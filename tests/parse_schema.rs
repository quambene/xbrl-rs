//! Integration tests for parsing taxonomy schema files.

use assert_matches::assert_matches;
use std::path::Path;
use xbrl_rs::{TaxonomySchema, XbrlError};

const SCHEMA_BASE: &str = "test_data/schemas";

#[test]
fn from_xml_unchecked_parses_minimal_valid_schema() {
    let path = Path::new(SCHEMA_BASE).join("minimal_valid_schema.xsd");
    let schema = TaxonomySchema::from_file_unchecked(&path).unwrap();

    assert_eq!(
        schema.target_namespace.as_deref(),
        Some("http://example.com/taxonomy")
    );
    assert_eq!(schema.imports.len(), 1);
    assert_eq!(schema.concepts.len(), 1);
    assert_eq!(schema.concepts[0].name.local_name, "Cash");
}

#[test]
fn from_xml_unchecked_accepts_arcrole_used_on_when_qnames_are_not_s_equal() {
    let path = Path::new(SCHEMA_BASE).join("arcrole_used_on_not_s_equal.xsd");
    let parsed = TaxonomySchema::from_file_unchecked(&path);

    assert!(parsed.is_ok());
}

#[test]
fn from_xml_unchecked_accepts_role_used_on_when_qnames_are_not_s_equal() {
    let path = Path::new(SCHEMA_BASE).join("role_used_on_not_s_equal.xsd");
    let parsed = TaxonomySchema::from_file_unchecked(&path);

    assert!(parsed.is_ok());
}

#[test]
fn from_xml_unchecked_rejects_arcrole_used_on_when_qnames_are_s_equal() {
    let path = Path::new(SCHEMA_BASE).join("arcrole_used_on_s_equal_duplicate.xsd");
    let parsed = TaxonomySchema::from_file_unchecked(&path);

    assert_matches!(parsed, Err(XbrlError::InvalidSchemaDocument { reason, .. }) => {
        assert!(reason.contains("duplicate s-equal usedOn"));
    });
}

#[test]
fn from_xml_unchecked_rejects_role_used_on_when_qnames_are_s_equal() {
    let path = Path::new(SCHEMA_BASE).join("role_used_on_s_equal_duplicate.xsd");
    let parsed = TaxonomySchema::from_file_unchecked(&path);

    assert_matches!(parsed, Err(XbrlError::InvalidSchemaDocument { reason, .. }) => {
        assert!(reason.contains("duplicate s-equal usedOn"));
    });
}
