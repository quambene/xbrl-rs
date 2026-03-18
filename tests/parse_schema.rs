//! Integration tests for parsing taxonomy schema files.

use std::path::Path;
use xbrl_rs::TaxonomySchema;

const SCHEMA_BASE: &str = "test_data/schemas";

#[test]
fn parse_minimal_valid_schema() {
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
