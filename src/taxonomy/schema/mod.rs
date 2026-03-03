mod concept;
mod parser;
mod reader;
mod resolver;
mod validation;

use crate::{
    error::{Result, XbrlError},
    instance::Decimals,
};
pub use concept::{
    Balance, BaseSubstitutionGroup, Concept, MaxOccurs, PeriodType, SubstitutionGroup,
    TupleChildRef, XbrlBase, XbrlType,
};
use quick_xml::Reader;
use std::{
    collections::{HashMap, HashSet},
    fmt, io,
    path::{Path, PathBuf},
    str::FromStr,
};

/// The allowed cycle direction for an arcrole (`cyclesAllowed` attribute).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CyclesAllowed {
    /// Any cycles are allowed.
    Any,
    /// Only undirected cycles are allowed.
    Undirected,
    /// No cycles are allowed.
    None,
}

impl FromStr for CyclesAllowed {
    type Err = XbrlError;

    fn from_str(str: &str) -> Result<Self> {
        match str {
            "any" => Ok(Self::Any),
            "undirected" => Ok(Self::Undirected),
            "none" => Ok(Self::None),
            _ => Err(XbrlError::ParseError {
                expected: "CyclesAllowed",
                value: str.to_owned(),
            }),
        }
    }
}

impl fmt::Display for CyclesAllowed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Any => f.write_str("any"),
            Self::Undirected => f.write_str("undirected"),
            Self::None => f.write_str("none"),
        }
    }
}

/// A `link:roleType` definition from a taxonomy schema.
#[derive(Debug, Clone)]
pub struct RoleType {
    /// The id attribute (e.g., "role_balanceSheet").
    pub id: String,
    /// The roleURI attribute.
    pub role_uri: String,
    /// The human-readable definition (child `link:definition` text).
    pub definition: Option<String>,
    /// Which link types this role is used on (child `link:usedOn` texts).
    pub used_on: Vec<String>,
}

/// A `link:arcroleType` definition from a taxonomy schema.
#[derive(Debug, Clone)]
pub struct ArcroleType {
    /// The id attribute.
    pub id: String,
    /// The arcroleURI attribute.
    pub arcrole_uri: String,
    /// The human-readable definition.
    pub definition: Option<String>,
    /// Which link types this arcrole is used on.
    pub used_on: Vec<String>,
    /// The cycles-allowed attribute.
    pub cycles_allowed: Option<CyclesAllowed>,
}

/// A `link:linkbaseRef` found in a schema's `xs:annotation/xs:appinfo`.
#[derive(Debug, Clone)]
pub struct LinkbaseRef {
    /// The xlink:href value (relative path to the linkbase file).
    pub href: String,
    /// The xlink:role (e.g., <http://www.xbrl.org/2003/role/labelLinkbaseRef>).
    pub role: Option<String>,
    /// The xlink:arcrole (typically <http://www.w3.org/1999/xlink/properties/linkbase>).
    pub arcrole: Option<String>,
    /// The xlink:title.
    pub title: Option<String>,
}

/// An `xs:import` reference in a schema.
#[derive(Debug, Clone)]
pub struct SchemaImport {
    /// Namespace URI declared by `xs:import/@namespace`.
    pub namespace: String,
    /// Optional schema location from `xs:import/@schemaLocation`.
    pub schema_location: Option<String>,
}

/// An `xs:include` reference in a schema.
#[derive(Debug, Clone)]
pub struct SchemaInclude {
    pub schema_location: String,
}

/// Declared accuracy constraints for an XBRL item type.
///
/// Holds the `decimals` and/or `precision` fixed/default values declared on
/// a named type's attribute restrictions (e.g. `xbrli:decimalItemType`).
#[derive(Debug, Clone, Default)]
pub struct DeclaredAccuracy {
    /// Declared `decimals` constraint, if any.
    pub decimals: Option<Decimals>,
    /// Declared `precision` constraint, if any.
    pub precision: Option<Decimals>,
}

/// A parsed taxonomy schema (.xsd) file.
#[derive(Debug)]
pub struct TaxonomySchema {
    /// Absolute file path of this schema.
    pub file_path: PathBuf,
    /// The targetNamespace of this schema.
    pub target_namespace: Option<String>,
    /// Namespace declarations (prefix -> URI).
    pub namespaces: HashMap<String, String>,
    /// `xs:import` references.
    pub imports: Vec<SchemaImport>,
    /// `xs:include` references.
    pub includes: Vec<SchemaInclude>,
    /// `link:linkbaseRef` entries.
    pub linkbase_refs: Vec<LinkbaseRef>,
    /// Locations referenced by `xsi:schemaLocation` and
    /// `xsi:noNamespaceSchemaLocation` attributes.
    pub schema_location_refs: Vec<String>,
    /// `link:roleType` definitions.
    pub role_types: Vec<RoleType>,
    /// `link:arcroleType` definitions.
    pub arcrole_types: Vec<ArcroleType>,
    /// `xs:element` definitions.
    pub concepts: Vec<Concept>,
    /// Named simple/complex type derivations: type name -> base QName.
    pub type_bases: HashMap<String, String>,
    /// Named types with declared decimals/precision attributes (fixed/default) on restrictions.
    pub type_declared_accuracy: HashMap<String, DeclaredAccuracy>,
}

impl TaxonomySchema {
    /// Parse a taxonomy schema from an XML reader without semantic validation.
    pub fn from_xml_unchecked<R: io::BufRead>(path: &Path, reader: &mut Reader<R>) -> Result<Self> {
        reader::read_schema(path, reader)
    }

    /// Parse a taxonomy schema from an XML reader with semantic validation.
    pub fn from_xml<R: io::BufRead>(path: &Path, reader: &mut Reader<R>) -> Result<Self> {
        let schema = Self::from_xml_unchecked(path, reader)?;
        schema.validate()?;
        Ok(schema)
    }

    /// Validate schema-level XBRL constraints.
    pub fn validate(&self) -> Result<()> {
        validation::validate(self)
    }

    fn is_monetary_type(&self, type_name: &str) -> bool {
        if local_name(type_name) == "monetaryItemType" {
            return true;
        }

        let mut current = type_name.to_string();
        let mut visited: HashSet<String> = HashSet::new();

        while visited.insert(current.clone()) {
            let Some(base) = self.type_bases.get(&current) else {
                return false;
            };

            if local_name(base) == "monetaryItemType" {
                return true;
            }

            current = base.clone();
        }

        false
    }

    fn is_known_complex_item_type(&self, type_name: &str) -> bool {
        let local = local_name(type_name);
        !local.ends_with("ItemType")
    }
}

/// Extract the local name from a possibly prefixed XML name.
fn local_name(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}

fn is_ncname(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };

    if !(first == '_' || first.is_ascii_alphabetic()) {
        return false;
    }

    chars.all(|ch| ch == '_' || ch == '-' || ch == '.' || ch.is_ascii_alphanumeric())
}

fn is_absolute_uri(value: &str) -> bool {
    if value.is_empty() || value.starts_with('#') {
        return false;
    }

    let Some((scheme, _rest)) = value.split_once(':') else {
        return false;
    };

    !scheme.is_empty()
        && scheme.chars().enumerate().all(|(index, ch)| {
            ch.is_ascii_alphabetic()
                || (index > 0 && matches!(ch, '+' | '-' | '.'))
                || (index > 0 && ch.is_ascii_digit())
        })
}

#[cfg(test)]
mod tests {
    use super::{Concept, RoleType, TaxonomySchema};
    use crate::{
        ConceptId, QName, SubstitutionGroup, XbrlBase, XbrlError, XbrlType,
        taxonomy::schema::concept::{Balance, PeriodType},
    };
    use assert_matches::assert_matches;
    use std::collections::HashMap;

    fn test_concept(
        local_name: &str,
        type_local: &str,
        substitution_group: SubstitutionGroup,
        period_type: Option<PeriodType>,
        balance: Option<Balance>,
    ) -> Concept {
        Concept {
            id: ConceptId::from(local_name),
            qname: QName {
                namespace: "".to_string(),
                local: local_name.to_string(),
            },
            data_type: XbrlType {
                name: QName {
                    namespace: "xbrli".to_string(),
                    local: type_local.to_string(),
                },
                base: if type_local.contains("monetary") {
                    XbrlBase::Monetary
                } else {
                    XbrlBase::String
                },
            },
            substitution_group,
            nillable: true,
            is_abstract: false,
            period_type,
            balance,
            tuple_children: Vec::new(),
        }
    }

    #[test]
    fn validate_requires_period_type_on_items() {
        let schema = TaxonomySchema {
            file_path: "test.xsd".into(),
            target_namespace: Some("http://example.com/taxonomy".to_string()),
            namespaces: HashMap::new(),
            imports: vec![],
            includes: vec![],
            linkbase_refs: vec![],
            schema_location_refs: vec![],
            role_types: vec![],
            arcrole_types: vec![],
            concepts: vec![test_concept(
                "MissingPeriodType",
                "stringItemType",
                SubstitutionGroup::item(),
                None,
                None,
            )],
            type_bases: HashMap::new(),
            type_declared_accuracy: HashMap::new(),
        };

        let res = schema.validate();

        assert_matches!(res, Err(XbrlError::InvalidSchemaDocument { reason, .. }) => {
            assert!(reason.contains("missing xbrli:periodType"));
        });
    }

    #[test]
    fn validate_rejects_balance_on_non_monetary_item() {
        let schema = TaxonomySchema {
            file_path: "test.xsd".into(),
            target_namespace: Some("http://example.com/taxonomy".to_string()),
            namespaces: HashMap::new(),
            imports: vec![],
            includes: vec![],
            linkbase_refs: vec![],
            schema_location_refs: vec![],
            role_types: vec![],
            arcrole_types: vec![],
            concepts: vec![test_concept(
                "NonMonetaryWithBalance",
                "stringItemType",
                SubstitutionGroup::item(),
                Some(PeriodType::Duration),
                Some(Balance::Credit),
            )],
            type_bases: HashMap::new(),
            type_declared_accuracy: HashMap::new(),
        };

        let res = schema.validate();

        assert_matches!(res, Err(XbrlError::InvalidSchemaDocument { reason, .. }) => {
            assert!(reason.contains("not monetaryItemType-derived"));
        });
    }

    #[test]
    fn validate_rejects_tuple_with_period_type() {
        let schema = TaxonomySchema {
            file_path: "test.xsd".into(),
            target_namespace: Some("http://example.com/taxonomy".to_string()),
            namespaces: HashMap::new(),
            imports: vec![],
            includes: vec![],
            linkbase_refs: vec![],
            schema_location_refs: vec![],
            role_types: vec![],
            arcrole_types: vec![],
            concepts: vec![test_concept(
                "TupleWithPeriodType",
                "stringItemType",
                SubstitutionGroup::tuple(),
                Some(PeriodType::Duration),
                None,
            )],
            type_bases: HashMap::new(),
            type_declared_accuracy: HashMap::new(),
        };

        let res = schema.validate();

        assert_matches!(res, Err(XbrlError::InvalidSchemaDocument { reason, .. }) => {
            assert!(reason.contains("must not declare xbrli:periodType"));
        });
    }

    #[test]
    fn validate_rejects_tuple_with_balance() {
        let schema = TaxonomySchema {
            file_path: "test.xsd".into(),
            target_namespace: Some("http://example.com/taxonomy".to_string()),
            namespaces: HashMap::new(),
            imports: vec![],
            includes: vec![],
            linkbase_refs: vec![],
            schema_location_refs: vec![],
            role_types: vec![],
            arcrole_types: vec![],
            concepts: vec![test_concept(
                "TupleWithBalance",
                "stringItemType",
                SubstitutionGroup::tuple(),
                None,
                Some(Balance::Credit),
            )],
            type_bases: HashMap::new(),
            type_declared_accuracy: HashMap::new(),
        };

        let res = schema.validate();

        assert_matches!(res, Err(XbrlError::InvalidSchemaDocument { reason, .. }) => {
            assert!(reason.contains("must not declare xbrli:balance"));
        });
    }

    #[test]
    fn validate_rejects_role_type_with_invalid_ncname_id() {
        let schema = TaxonomySchema {
            file_path: "test.xsd".into(),
            target_namespace: Some("http://example.com/taxonomy".to_string()),
            namespaces: HashMap::new(),
            imports: vec![],
            includes: vec![],
            linkbase_refs: vec![],
            schema_location_refs: vec![],
            role_types: vec![RoleType {
                id: "1invalid-id".to_string(),
                role_uri: "http://example.com/role".to_string(),
                definition: None,
                used_on: vec![],
            }],
            arcrole_types: vec![],
            concepts: vec![],
            type_bases: HashMap::new(),
            type_declared_accuracy: HashMap::new(),
        };

        let res = schema.validate();

        assert_matches!(res, Err(XbrlError::InvalidSchemaDocument { reason, .. }) => {
            assert!(reason.contains("roleType id"));
            assert!(reason.contains("NCName"));
        });
    }

    #[test]
    fn validate_accepts_monetary_item_with_balance_and_period_type() {
        let schema = TaxonomySchema {
            file_path: "test.xsd".into(),
            target_namespace: Some("http://example.com/taxonomy".to_string()),
            namespaces: HashMap::new(),
            imports: vec![],
            includes: vec![],
            linkbase_refs: vec![],
            schema_location_refs: vec![],
            role_types: vec![],
            arcrole_types: vec![],
            concepts: vec![test_concept(
                "Cash",
                "monetaryItemType",
                SubstitutionGroup::item(),
                Some(PeriodType::Instant),
                Some(Balance::Debit),
            )],
            type_bases: HashMap::new(),
            type_declared_accuracy: HashMap::new(),
        };

        assert!(schema.validate().is_ok());
    }
}
