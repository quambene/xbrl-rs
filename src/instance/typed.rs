use super::{
    Context, ContextId, Decimals, Fact, FootnoteLink, InstanceDocument, ItemFact, TupleFact, Unit,
    UnitId,
};
use crate::{
    Concept, ExpandedName, NamespacePrefix, NamespaceUri, TaxonomySet, ValueError, XbrlError,
    XbrlType,
};
use chrono::{NaiveDate, NaiveDateTime};
use rust_decimal::Decimal;
use std::{collections::HashMap, str::FromStr};

/// A typed representation of an XBRL instance document.
#[derive(Debug, Clone)]
pub struct TypedInstanceDocument {
    /// Namespace prefixes used in the document.
    namespaces: HashMap<NamespacePrefix, NamespaceUri>,
    /// Schema references from link:schemaRef elements.
    schema_refs: Vec<String>,
    /// Role references declared in the instance.
    role_refs: Vec<String>,
    /// Arcrole references declared in the instance.
    arcrole_refs: Vec<String>,
    /// All contexts in the instance.
    contexts: HashMap<ContextId, Context>,
    /// All units in the instance.
    units: HashMap<UnitId, Unit>,
    /// Top-level typed facts.
    facts: Vec<TypedFact>,
    /// Footnote links found in the instance.
    footnote_links: Vec<FootnoteLink>,
}

impl TypedInstanceDocument {
    /// Converts a raw `InstanceDocument` into a `TypedInstanceDocument` where
    /// `Fact`s are replaced by `TypedFact`s.
    pub fn from_instance(
        instance: InstanceDocument,
        taxonomy: &TaxonomySet,
    ) -> Result<Self, XbrlError> {
        let InstanceDocument {
            namespaces,
            schema_refs,
            role_refs,
            arcrole_refs,
            contexts,
            units,
            facts,
            footnote_links,
        } = instance;

        let facts = convert_facts(&facts, taxonomy, &contexts, &units, &namespaces)?;

        Ok(Self {
            namespaces,
            schema_refs,
            role_refs,
            arcrole_refs,
            contexts,
            units,
            facts,
            footnote_links,
        })
    }

    pub fn facts(&self) -> &[TypedFact] {
        &self.facts
    }

    pub fn contexts(&self) -> &HashMap<ContextId, Context> {
        &self.contexts
    }

    pub fn units(&self) -> &HashMap<UnitId, Unit> {
        &self.units
    }

    pub fn schema_refs(&self) -> &[String] {
        &self.schema_refs
    }

    pub fn role_refs(&self) -> &[String] {
        &self.role_refs
    }

    pub fn arcrole_refs(&self) -> &[String] {
        &self.arcrole_refs
    }

    pub fn namespaces(&self) -> &HashMap<NamespacePrefix, NamespaceUri> {
        &self.namespaces
    }

    pub fn footnote_links(&self) -> &[FootnoteLink] {
        &self.footnote_links
    }
}

/// Represents a typed fact in the instance, which can be either an item or a tuple.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum TypedFact {
    Item(TypedItemFact),
    Tuple(TypedTupleFact),
}

#[derive(Debug, Clone)]
pub struct TypedItemFact {
    /// Resolved concept
    pub concept: Concept,
    /// Optional XML id
    pub id: Option<String>,
    /// Resolved context
    pub context: Context,
    /// Resolved unit (if numeric).
    pub unit: Option<Unit>,
    /// Typed and validated value. Nil facts are represented as `None`.
    pub value: Option<FactValue>,
    /// Decimals attribute (if numeric).
    pub decimals: Option<Decimals>,
    /// Precision attribute (if numeric).
    pub precision: Option<Decimals>,
}

#[derive(Debug, Clone)]
pub struct TypedTupleFact {
    pub concept: Concept,
    pub id: Option<String>,
    pub children: Vec<TypedFact>,
}

#[derive(Debug, Clone)]
pub enum FactValue {
    Text(String),
    Boolean(bool),
    Integer(i64),
    Decimal(Decimal),
    Date(NaiveDate),
    DateTime(NaiveDateTime),
    QName(ExpandedName),
}

fn convert_facts(
    facts: &[Fact],
    taxonomy: &TaxonomySet,
    contexts: &HashMap<ContextId, Context>,
    units: &HashMap<UnitId, Unit>,
    namespaces: &HashMap<NamespacePrefix, NamespaceUri>,
) -> Result<Vec<TypedFact>, XbrlError> {
    facts
        .iter()
        .map(|fact| convert_fact(fact, taxonomy, contexts, units, namespaces))
        .collect()
}

fn convert_fact(
    fact: &Fact,
    taxonomy: &TaxonomySet,
    contexts: &HashMap<ContextId, Context>,
    units: &HashMap<UnitId, Unit>,
    namespaces: &HashMap<NamespacePrefix, NamespaceUri>,
) -> Result<TypedFact, XbrlError> {
    match fact {
        Fact::Item(item) => Ok(TypedFact::Item(convert_item(
            item, taxonomy, contexts, units, namespaces,
        )?)),
        Fact::Tuple(tuple) => Ok(TypedFact::Tuple(convert_tuple(
            tuple, taxonomy, contexts, units, namespaces,
        )?)),
    }
}

fn convert_item(
    item: &ItemFact,
    taxonomy: &TaxonomySet,
    contexts: &HashMap<ContextId, Context>,
    units: &HashMap<UnitId, Unit>,
    namespaces: &HashMap<NamespacePrefix, NamespaceUri>,
) -> Result<TypedItemFact, XbrlError> {
    let concept = taxonomy
        .find_concept(item.concept_name())
        .ok_or_else(|| ValueError::UnknownConcept {
            concept: item.concept_name().to_string(),
        })?
        .clone();

    let context =
        contexts
            .get(item.context_ref())
            .cloned()
            .ok_or_else(|| ValueError::MissingContext {
                context_ref: item.context_ref().to_owned(),
            })?;

    let unit = match item.unit_ref() {
        Some(unit_ref) => {
            Some(
                units
                    .get(unit_ref)
                    .cloned()
                    .ok_or_else(|| ValueError::MissingUnit {
                        unit_ref: unit_ref.to_owned(),
                    })?,
            )
        }
        None => None,
    };

    let value = if item.is_nil() {
        None
    } else {
        Some(parse_fact_value(item.value(), &concept, namespaces)?)
    };

    Ok(TypedItemFact {
        concept: concept.to_owned(),
        id: item.id().map(str::to_owned),
        context,
        unit,
        value,
        decimals: item.decimals().cloned(),
        precision: item.precision().cloned(),
    })
}

fn convert_tuple(
    tuple: &TupleFact,
    taxonomy: &TaxonomySet,
    contexts: &HashMap<ContextId, Context>,
    units: &HashMap<UnitId, Unit>,
    namespaces: &HashMap<NamespacePrefix, NamespaceUri>,
) -> Result<TypedTupleFact, XbrlError> {
    let concept = taxonomy
        .find_concept(tuple.concept_name())
        .ok_or_else(|| ValueError::UnknownConcept {
            concept: tuple.concept_name().to_string(),
        })?
        .clone();

    let children = convert_facts(tuple.children(), taxonomy, contexts, units, namespaces)?;

    Ok(TypedTupleFact {
        concept,
        id: tuple.id().map(str::to_owned),
        children,
    })
}

pub(crate) fn parse_fact_value(
    raw: &str,
    concept: &Concept,
    namespaces: &HashMap<NamespacePrefix, NamespaceUri>,
) -> Result<FactValue, XbrlError> {
    let value = raw.trim();

    match concept.data_type {
        XbrlType::Boolean => parse_boolean(value).map(FactValue::Boolean),
        XbrlType::Integer => parse_integer(value).map(FactValue::Integer),
        XbrlType::Monetary
        | XbrlType::Decimal
        | XbrlType::Float
        | XbrlType::Double
        | XbrlType::Shares
        | XbrlType::Pure
        | XbrlType::Fraction
        | XbrlType::Percent
        | XbrlType::PerShare => parse_decimal_compatible(value).map(FactValue::Decimal),
        XbrlType::Date => parse_xsd_date(value).map(FactValue::Date),
        XbrlType::DateTime => parse_xsd_datetime(value).map(FactValue::DateTime),
        XbrlType::QName => parse_qname_value(value, namespaces).map(FactValue::QName),
        XbrlType::String | XbrlType::Simple(_) | XbrlType::Complex(_) => {
            Ok(FactValue::Text(value.to_owned()))
        }
    }
}

fn parse_boolean(value: &str) -> Result<bool, XbrlError> {
    if value.eq_ignore_ascii_case("true") || value == "1" {
        return Ok(true);
    }
    if value.eq_ignore_ascii_case("false") || value == "0" {
        return Ok(false);
    }

    Err(ValueError::InvalidBoolean {
        raw: value.to_owned(),
    }
    .into())
}

fn parse_integer(value: &str) -> Result<i64, XbrlError> {
    i64::from_str(value).map_err(|_| {
        ValueError::InvalidInteger {
            raw: value.to_owned(),
        }
        .into()
    })
}

fn parse_decimal_compatible(value: &str) -> Result<Decimal, XbrlError> {
    if value.is_empty() {
        return Err(ValueError::InvalidDecimal {
            raw: value.to_owned(),
        }
        .into());
    }

    if let Ok(decimal) = Decimal::from_str(value) {
        return Ok(decimal);
    }

    Decimal::from_scientific(value).map_err(|_| {
        ValueError::InvalidDecimal {
            raw: value.to_owned(),
        }
        .into()
    })
}

fn parse_xsd_date(value: &str) -> Result<NaiveDate, XbrlError> {
    let normalized = strip_xsd_timezone(value);
    NaiveDate::parse_from_str(normalized, "%Y-%m-%d").map_err(|_| {
        ValueError::InvalidDate {
            raw: value.to_owned(),
        }
        .into()
    })
}

fn parse_xsd_datetime(value: &str) -> Result<NaiveDateTime, XbrlError> {
    let normalized = strip_xsd_timezone(value);
    NaiveDateTime::parse_from_str(normalized, "%Y-%m-%dT%H:%M:%S%.f")
        .or_else(|_| NaiveDateTime::parse_from_str(normalized, "%Y-%m-%dT%H:%M:%S"))
        .map_err(|_| {
            ValueError::InvalidDateTime {
                raw: value.to_owned(),
            }
            .into()
        })
}

fn strip_xsd_timezone(value: &str) -> &str {
    let trimmed = value.trim();
    if let Some(without_z) = trimmed.strip_suffix('Z') {
        return without_z;
    }

    if trimmed.len() >= 6 {
        let tz_marker_index = trimmed.len() - 6;
        let bytes = trimmed.as_bytes();
        if (bytes[tz_marker_index] == b'+' || bytes[tz_marker_index] == b'-')
            && bytes.get(tz_marker_index + 3) == Some(&b':')
        {
            return &trimmed[..tz_marker_index];
        }
    }

    trimmed
}

fn parse_qname_value(
    value: &str,
    namespaces: &HashMap<NamespacePrefix, NamespaceUri>,
) -> Result<ExpandedName, XbrlError> {
    if value.is_empty() {
        return Err(ValueError::InvalidQName {
            raw: value.to_owned(),
            reason: "QName value is empty".to_owned(),
        }
        .into());
    }

    let (prefix, local_name) = match value.split_once(':') {
        Some((prefix, local_name)) => (Some(prefix), local_name),
        None => (None, value),
    };

    if local_name.is_empty() {
        return Err(ValueError::InvalidQName {
            raw: value.to_owned(),
            reason: "QName local name is empty".to_owned(),
        }
        .into());
    }

    let namespace_uri = match prefix {
        Some(prefix) => {
            namespaces
                .get(prefix)
                .cloned()
                .ok_or_else(|| ValueError::InvalidQName {
                    raw: value.to_owned(),
                    reason: format!("Unknown QName prefix '{prefix}'"),
                })?
        }
        None => namespaces
            .get("")
            .cloned()
            .unwrap_or_else(|| NamespaceUri::from("")),
    };

    Ok(ExpandedName::new(namespace_uri, local_name.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::{FactValue, parse_fact_value};
    use crate::{
        Balance, BaseSubstitutionGroup, Concept, ExpandedName, NamespaceUri, SubstitutionGroup,
        XbrlType,
    };
    use std::collections::HashMap;

    fn concept_with_type(data_type: XbrlType) -> Concept {
        Concept {
            id: Some("c1".to_owned()),
            name: ExpandedName::new(
                NamespaceUri::from("http://example.com"),
                "Concept".to_owned(),
            ),
            data_type,
            substitution_group: SubstitutionGroup {
                base: BaseSubstitutionGroup::Item,
                original: ExpandedName::new(
                    NamespaceUri::from("http://www.xbrl.org/2003/instance"),
                    "item".to_owned(),
                ),
            },
            period_type: None,
            balance: Some(Balance::Debit),
            nillable: true,
            is_abstract: false,
            content_model: None,
        }
    }

    #[test]
    fn parse_integer_value() {
        let concept = concept_with_type(XbrlType::Integer);
        let value = parse_fact_value("42", &concept, &HashMap::new()).expect("integer parse");
        assert!(matches!(value, FactValue::Integer(42)));
    }

    #[test]
    fn parse_invalid_boolean_value() {
        let concept = concept_with_type(XbrlType::Boolean);
        let value = parse_fact_value("truth", &concept, &HashMap::new());
        assert!(value.is_err());
    }
}
