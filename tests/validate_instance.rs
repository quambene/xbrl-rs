//! Integration tests for XBRL instance validation.

use quick_xml::Reader;
use std::{
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
    str::FromStr,
};
use xbrl_rs::{Fact, InstanceDocument, TaxonomySet};

const INSTANCE_BASE: &str = "test_data/instances";
const TAXONOMY_ENTRY_POINT: &str = "test_data/taxonomies";
const SCHEMA_ENTRY_POINT: &str = "test_data/schemas";

fn parse_instance(path: &Path) -> InstanceDocument {
    let file = File::open(path).expect("failed to open instance file");
    let mut reader = Reader::from_reader(BufReader::new(file));

    InstanceDocument::from_xml(&mut reader).expect("failed to parse instance")
}

fn discover_taxonomy(instance: &InstanceDocument, entry_point: &str) -> TaxonomySet {
    let entry_point = PathBuf::from_str(entry_point).unwrap();
    TaxonomySet::discover(instance.schema_refs().to_vec(), entry_point).unwrap()
}

#[test]
fn validate_instance_balance_sheet_v64() {
    let path = Path::new(INSTANCE_BASE).join("balance_sheet_v64.xml");
    let instance = parse_instance(&path);
    let taxonomy = discover_taxonomy(&instance, TAXONOMY_ENTRY_POINT);

    let result = instance.validate(&taxonomy);

    assert!(result.is_valid());
    assert!(result.errors().is_empty(), "errors: {:#?}", result.errors());
    assert!(
        result.warnings().is_empty(),
        "warnings: {:#?}",
        result.warnings()
    );
}

#[test]
fn validate_instance_balance_sheet_v65() {
    let path = Path::new(INSTANCE_BASE).join("balance_sheet_v65.xml");
    let instance = parse_instance(&path);
    let taxonomy = discover_taxonomy(&instance, TAXONOMY_ENTRY_POINT);

    let result = instance.validate(&taxonomy);

    assert!(result.is_valid());
    assert!(result.errors().is_empty(), "errors: {:#?}", result.errors());
    assert!(
        result.warnings().is_empty(),
        "warnings: {:#?}",
        result.warnings()
    );
}

#[test]
fn validates_unknown_tuple_concept() {
    let path = Path::new(INSTANCE_BASE).join("validation_tuple_base.xml");
    let mut instance = parse_instance(&path);
    let taxonomy = discover_taxonomy(&instance, SCHEMA_ENTRY_POINT);

    instance.add_fact(Fact::tuple("de-gcd:doesNotExistTuple".to_string()));

    let result = instance.validate(&taxonomy);
    assert!(
        result
            .errors()
            .iter()
            .any(|error| error.code == "schema.concept_not_found")
    );
}

#[test]
fn validates_non_tuple_concept_used_as_tuple() {
    let path = Path::new(INSTANCE_BASE).join("validation_tuple_base.xml");
    let mut instance = parse_instance(&path);
    let taxonomy = discover_taxonomy(&instance, SCHEMA_ENTRY_POINT);

    instance.add_fact(Fact::tuple("my:city".to_string()));

    let result = instance.validate(&taxonomy);
    assert!(
        result
            .errors()
            .iter()
            .any(|error| error.code == "schema.tuple_requires_tuple_concept")
    );
}

#[test]
fn validates_tuple_child_not_allowed() {
    let path = Path::new(INSTANCE_BASE).join("validation_tuple_strict_invalid_child.xml");
    let instance = parse_instance(&path);
    let taxonomy = discover_taxonomy(&instance, SCHEMA_ENTRY_POINT);

    let result = instance.validate(&taxonomy);
    assert!(
        result
            .errors()
            .iter()
            .any(|error| error.code == "schema.tuple_child_not_allowed")
    );
}

#[test]
fn validates_tuple_missing_required_child() {
    let path = Path::new(INSTANCE_BASE).join("validation_tuple_missing_required_child.xml");
    let instance = parse_instance(&path);
    let taxonomy = discover_taxonomy(&instance, SCHEMA_ENTRY_POINT);

    let result = instance.validate(&taxonomy);
    assert!(
        result
            .errors()
            .iter()
            .any(|error| error.code == "schema.tuple_missing_required_child"),
        "expected schema.tuple_missing_required_child error, got: {:?}",
        result.errors()
    );
}

#[test]
fn validates_tuple_min_occurs_underflow() {
    let path = Path::new(INSTANCE_BASE).join("validation_tuple_cardinality_min_violation.xml");
    let instance = parse_instance(&path);
    let taxonomy = discover_taxonomy(&instance, SCHEMA_ENTRY_POINT);

    let result = instance.validate(&taxonomy);
    assert!(
        result
            .errors()
            .iter()
            .any(|error| error.code == "schema.tuple_missing_required_child"),
        "expected schema.tuple_missing_required_child error, got: {:?}",
        result.errors()
    );
}

#[test]
fn validates_tuple_max_occurs_exceeded() {
    let path = Path::new(INSTANCE_BASE).join("validation_tuple_cardinality_max_violation.xml");
    let instance = parse_instance(&path);
    let taxonomy = discover_taxonomy(&instance, SCHEMA_ENTRY_POINT);

    let result = instance.validate(&taxonomy);
    assert!(
        result
            .errors()
            .iter()
            .any(|error| error.code == "schema.tuple_child_not_allowed"),
        "expected schema.tuple_child_not_allowed error, got: {:?}",
        result.errors()
    );
}

#[test]
fn accepts_tuple_children_within_cardinality_bounds() {
    let path = Path::new(INSTANCE_BASE).join("validation_tuple_cardinality_valid.xml");
    let instance = parse_instance(&path);
    let taxonomy = discover_taxonomy(&instance, SCHEMA_ENTRY_POINT);

    let result = instance.validate(&taxonomy);
    assert!(
        !result
            .errors()
            .iter()
            .any(|error| error.code == "schema.tuple_missing_required_child"),
        "unexpected min-occurs errors: {:#?}",
        result.errors()
    );
    assert!(
        !result
            .errors()
            .iter()
            .any(|error| error.code == "schema.tuple_child_not_allowed"),
        "unexpected max-occurs errors: {:#?}",
        result.errors()
    );
}

#[test]
fn accepts_tuple_concept_derived_by_substitution_group() {
    let path = Path::new(INSTANCE_BASE).join("validation_tuple_base.xml");
    let instance = parse_instance(&path);
    let taxonomy = discover_taxonomy(&instance, SCHEMA_ENTRY_POINT);

    let result = instance.validate(&taxonomy);

    assert!(
        !result
            .errors()
            .iter()
            .any(|error| error.code == "schema.tuple_requires_tuple_concept"),
        "unexpected tuple classification errors: {:#?}",
        result.errors()
    );
}

#[test]
fn tolerates_invalid_numeric_lexical_value_for_compatibility() {
    let path = Path::new(INSTANCE_BASE).join("balance_sheet_v64.xml");
    let mut instance = parse_instance(&path);
    let taxonomy = discover_taxonomy(&instance, TAXONOMY_ENTRY_POINT);

    let numeric_fact_index = instance
        .item_facts()
        .iter()
        .position(|fact| fact.unit_ref().is_some())
        .expect("expected at least one numeric fact");

    instance.set_fact_value(numeric_fact_index, "not-a-number".to_string());

    let result = instance.validate(&taxonomy);

    assert!(
        !result
            .errors()
            .iter()
            .any(|error| error.code == "schema.invalid_numeric_lexical"),
        "did not expect eager lexical numeric errors, got: {:#?}",
        result.errors()
    );
}
