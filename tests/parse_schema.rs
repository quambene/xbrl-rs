//! Integration tests for parsing taxonomy schema files.

use assert_matches::assert_matches;
use quick_xml::Reader;
use std::{fs::File, io::BufReader, path::Path};
use xbrl_rs::{TaxonomySchema, XbrlError};

const SCHEMA_BASE: &str = "test_data/schemas";

fn parse_schema_unchecked(path: &Path) -> Result<TaxonomySchema, XbrlError> {
    let file = File::open(path).expect("failed to open schema file");
    let mut reader = Reader::from_reader(BufReader::new(file));
    TaxonomySchema::from_xml_unchecked(path, &mut reader)
}

#[test]
fn from_xml_unchecked_parses_minimal_valid_schema() {
    let path = Path::new(SCHEMA_BASE).join("minimal_valid_schema.xsd");
    let schema = parse_schema_unchecked(&path).expect("schema should parse");

    assert_eq!(
        schema.target_namespace.as_deref(),
        Some("http://example.com/taxonomy")
    );
    assert_eq!(schema.imports.len(), 1);
    assert_eq!(schema.elements.len(), 1);
    assert_eq!(schema.elements[0].name, "Cash");
}

#[test]
fn from_xml_unchecked_requires_schema_root() {
    let path = Path::new(SCHEMA_BASE).join("invalid_missing_schema_root.xml");
    let res = parse_schema_unchecked(&path);

    assert_matches!(res, Err(XbrlError::InvalidSchemaDocument { reason, .. }) => {
        assert!(reason.contains("missing <schema> root element"));
    });
}

#[test]
fn from_xml_unchecked_accepts_arcrole_used_on_when_qnames_are_not_s_equal() {
    let path = Path::new(SCHEMA_BASE).join("arcrole_used_on_not_s_equal.xsd");
    let parsed = parse_schema_unchecked(&path);

    assert!(parsed.is_ok());
}

#[test]
fn from_xml_unchecked_rejects_arcrole_used_on_when_qnames_are_s_equal() {
    let path = Path::new(SCHEMA_BASE).join("arcrole_used_on_s_equal_duplicate.xsd");
    let parsed = parse_schema_unchecked(&path);

    assert_matches!(parsed, Err(XbrlError::InvalidSchemaDocument { reason, .. }) => {
        assert!(reason.contains("duplicate s-equal usedOn"));
    });
}

#[test]
fn from_xml_unchecked_accepts_role_used_on_when_qnames_are_not_s_equal() {
    let path = Path::new(SCHEMA_BASE).join("role_used_on_not_s_equal.xsd");
    let parsed = parse_schema_unchecked(&path);

    assert!(parsed.is_ok());
}

#[test]
fn from_xml_unchecked_rejects_role_used_on_when_qnames_are_s_equal() {
    let path = Path::new(SCHEMA_BASE).join("role_used_on_s_equal_duplicate.xsd");
    let parsed = parse_schema_unchecked(&path);

    assert_matches!(parsed, Err(XbrlError::InvalidSchemaDocument { reason, .. }) => {
        assert!(reason.contains("duplicate s-equal usedOn"));
    });
}
