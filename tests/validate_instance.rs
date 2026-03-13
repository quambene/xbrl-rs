//! Integration tests for XBRL instance validation.

use quick_xml::Reader;
use std::{
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
    str::FromStr,
};
use xbrl_rs::{InstanceDocument, TaxonomySet};

const INSTANCE_BASE: &str = "test_data/instances";
const TAXONOMY_ENTRY_POINT: &str = "test_data/taxonomies";

fn parse_instance(path: &Path) -> InstanceDocument {
    let file = File::open(path).expect("failed to open instance file");
    let reader = Reader::from_reader(BufReader::new(file));

    InstanceDocument::from_xml(reader).expect("failed to parse instance")
}

fn discover_taxonomy(instance: &InstanceDocument, entry_point: &str) -> TaxonomySet {
    let entry_point = PathBuf::from_str(entry_point).unwrap();
    TaxonomySet::discover(instance.schema_refs().to_vec(), entry_point).unwrap()
}

#[test]
#[cfg_attr(not(feature = "taxonomy-test"), ignore)]
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
#[cfg_attr(not(feature = "taxonomy-test"), ignore)]
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
