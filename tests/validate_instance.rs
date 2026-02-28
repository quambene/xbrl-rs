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

fn parse_instance(path: &Path) -> InstanceDocument {
    let file = File::open(path).expect("failed to open instance file");
    let mut reader = Reader::from_reader(BufReader::new(file));

    InstanceDocument::from_xml(&mut reader).expect("failed to parse instance")
}

fn discover_taxonomy(instance: &InstanceDocument, entry_point: &str) -> TaxonomySet {
    let entry_point = PathBuf::from_str(entry_point).unwrap();
    TaxonomySet::discover(instance.schema_refs().to_vec(), entry_point).unwrap()
}

fn case_instance(case: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("test_data")
        .join("cases")
        .join(case)
        .join("instance.xml")
}

fn discover_case_taxonomy(instance: &InstanceDocument, case: &str) -> TaxonomySet {
    let entry_point = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("test_data")
        .join("cases")
        .join(case);
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
    let path = case_instance("validation_tuple_base");
    let mut instance = parse_instance(&path);
    let taxonomy = discover_case_taxonomy(&instance, "validation_tuple_base");

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
    let path = case_instance("validation_tuple_base");
    let mut instance = parse_instance(&path);
    let taxonomy = discover_case_taxonomy(&instance, "validation_tuple_base");

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
