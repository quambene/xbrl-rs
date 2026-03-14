mod parser;
mod resolver;
mod validation;

use crate::{error::Result, instance::Decimals};
pub use parser::{ArcroleType, LinkbaseRef, RoleType, SchemaImport, SchemaInclude, SchemaParser};
pub use resolver::{
    BaseSubstitutionGroup, Concept, MaxOccurs, SubstitutionGroup, TupleChild, XbrlType,
};
use std::{
    collections::HashMap,
    io,
    path::{Path, PathBuf},
};

/// The XBRL balance type for a monetary taxonomy element (`xbrli:balance` attribute).
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Balance {
    /// An asset or expense concept (increases on the debit side).
    Debit,
    /// A liability, equity, or income concept (increases on the credit side).
    Credit,
}

/// The XBRL period type for a taxonomy element (`xbrli:periodType` attribute).
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum PeriodType {
    /// The element reports a value at a specific point in time.
    Instant,
    /// The element reports a value over a time range.
    Duration,
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
    /// Tuple definitions: tuple element ID -> child element references.
    pub tuple_defs: HashMap<String, TupleChild>,
    /// Named simple/complex type derivations: type name -> base QName.
    pub type_bases: HashMap<String, String>,
    /// Named types with declared decimals/precision attributes (fixed/default) on restrictions.
    pub type_declared_accuracy: HashMap<String, DeclaredAccuracy>,
}

impl TaxonomySchema {
    /// Parse a taxonomy schema from an XML reader without semantic validation.
    pub fn from_xml_unchecked<R: io::BufRead>(path: &Path, reader: R) -> Result<Self> {
        let mut parser = SchemaParser::new(reader, path.to_path_buf());
        let schema = parser.parse_schema()?;
        let concepts = resolver::resolve_concepts(&schema);

        Ok(TaxonomySchema {
            file_path: path.to_path_buf(),
            target_namespace: schema.target_namespace,
            namespaces: schema.namespaces,
            imports: schema.imports,
            includes: schema.includes,
            linkbase_refs: schema.linkbase_refs,
            // TODO: parse actual schema location refs from `xsi:schemaLocation`
            // and `xsi:noNamespaceSchemaLocation` attributes
            schema_location_refs: Vec::new(),
            // TODO: parse actual roleType and arcroleType definitions from
            // linkbase schema documents
            role_types: Vec::new(),
            // TODO: parse actual roleType and arcroleType definitions from
            // linkbase schema documents
            arcrole_types: Vec::new(),
            concepts,
            // TODO: move tuple definitions from `Concept::tuple_children` to this
            // top-level map during concept resolution
            tuple_defs: HashMap::new(),
            type_bases: HashMap::new(),
            type_declared_accuracy: HashMap::new(),
        })
    }

    /// Parse a taxonomy schema from an XML reader with semantic validation.
    pub fn from_xml<R: io::BufRead>(path: &Path, reader: R) -> Result<Self> {
        let schema = Self::from_xml_unchecked(path, reader)?;
        schema.validate()?;
        Ok(schema)
    }

    /// Validate schema-level XBRL constraints.
    pub fn validate(&self) -> Result<()> {
        validation::validate(self)
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
    use super::{Concept, TaxonomySchema};
    use crate::{
        Balance, ExpandedName, NamespaceUri, PeriodType, XbrlError,
        taxonomy::{
            BaseSubstitutionGroup, RoleType,
            schema::resolver::{SubstitutionGroup, XbrlType},
        },
        xml::QName,
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
            id: None,
            name: ExpandedName {
                namespace_uri: NamespaceUri::from(""),
                local_name: local_name.to_string(),
            },
            data_type: XbrlType::Simple(type_local.to_string()),
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
                SubstitutionGroup {
                    base: BaseSubstitutionGroup::Item,
                    original: QName {
                        prefix: None,
                        local_name: "item".to_string(),
                    },
                },
                None,
                None,
            )],
            tuple_defs: HashMap::new(),
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
                SubstitutionGroup {
                    base: BaseSubstitutionGroup::Item,
                    original: QName {
                        prefix: None,
                        local_name: "item".to_string(),
                    },
                },
                Some(PeriodType::Duration),
                Some(Balance::Credit),
            )],
            tuple_defs: HashMap::new(),
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
                SubstitutionGroup {
                    base: BaseSubstitutionGroup::Tuple,
                    original: QName {
                        prefix: None,
                        local_name: "tuple".to_string(),
                    },
                },
                Some(PeriodType::Duration),
                None,
            )],
            tuple_defs: HashMap::new(),
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
                SubstitutionGroup {
                    base: BaseSubstitutionGroup::Tuple,
                    original: QName {
                        prefix: None,
                        local_name: "tuple".to_string(),
                    },
                },
                None,
                Some(Balance::Credit),
            )],
            tuple_defs: HashMap::new(),
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
            tuple_defs: HashMap::new(),
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
                SubstitutionGroup {
                    base: BaseSubstitutionGroup::Item,
                    original: QName {
                        prefix: None,
                        local_name: "item".to_string(),
                    },
                },
                Some(PeriodType::Instant),
                Some(Balance::Debit),
            )],
            tuple_defs: HashMap::new(),
            type_bases: HashMap::new(),
            type_declared_accuracy: HashMap::new(),
        };

        assert!(schema.validate().is_ok());
    }
}
