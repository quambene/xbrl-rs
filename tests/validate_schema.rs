//! Integration tests for parsing taxonomy schema files.

use assert_matches::assert_matches;
use std::path::Path;
use xbrl_rs::{TaxonomySchema, XbrlError};

const SCHEMA_BASE: &str = "test_data/schemas";

#[test]
fn validate_accepts_arcrole_used_on_when_qnames_are_not_s_equal() {
    let path = Path::new(SCHEMA_BASE).join("arcrole_used_on_not_s_equal.xsd");
    let parsed = TaxonomySchema::from_file(&path);

    assert!(
        parsed.is_ok(),
        "Expected successful parse, got error: {:?}",
        parsed.err()
    );
}

#[test]
fn validate_accepts_role_used_on_when_qnames_are_not_s_equal() {
    let path = Path::new(SCHEMA_BASE).join("role_used_on_not_s_equal.xsd");
    let parsed = TaxonomySchema::from_file(&path);

    assert!(
        parsed.is_ok(),
        "Expected successful parse, got error: {:?}",
        parsed.err()
    );
}

#[test]
fn validate_rejects_arcrole_used_on_when_qnames_are_s_equal() {
    let path = Path::new(SCHEMA_BASE).join("arcrole_used_on_s_equal_duplicate.xsd");
    let parsed = TaxonomySchema::from_file(&path);

    assert_matches!(parsed, Err(XbrlError::InvalidSchemaDocument { reason, .. }) => {
        assert!(reason.contains("duplicate s-equal usedOn"));
    });
}

#[test]
fn validate_rejects_role_used_on_when_qnames_are_s_equal() {
    let path = Path::new(SCHEMA_BASE).join("role_used_on_s_equal_duplicate.xsd");
    let parsed = TaxonomySchema::from_file(&path);

    assert_matches!(parsed, Err(XbrlError::InvalidSchemaDocument { reason, .. }) => {
        assert!(reason.contains("duplicate s-equal usedOn"));
    });
}
