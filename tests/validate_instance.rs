//! Integration tests for XBRL instance validation.

use quick_xml::Reader;
use std::{
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
    str::FromStr,
};
use xbrl_rs::{TaxonomySet, XbrlInstance};

const INSTANCE_BASE: &str = "test_data/instances";
const TAXONOMY_ENTRY_POINT: &str = "test_data/taxonomies";

fn parse_instance(path: &Path) -> XbrlInstance {
    let file = File::open(path).expect("failed to open instance file");
    let mut reader = Reader::from_reader(BufReader::new(file));

    XbrlInstance::from_xml(&mut reader).expect("failed to parse instance")
}

#[test]
fn validate_instance_balance_sheet_v64() {
    let path = Path::new(INSTANCE_BASE).join("balance_sheet_v64.xml");
    let instance = parse_instance(&path);
    let entry_point = PathBuf::from_str(TAXONOMY_ENTRY_POINT).unwrap();
    let taxonomy = TaxonomySet::discover(instance.schema_refs().to_vec(), entry_point).unwrap();

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
    let entry_point = PathBuf::from_str(TAXONOMY_ENTRY_POINT).unwrap();
    let taxonomy = TaxonomySet::discover(instance.schema_refs().to_vec(), entry_point).unwrap();

    let result = instance.validate(&taxonomy);

    assert!(result.is_valid());
    assert!(result.errors().is_empty(), "errors: {:#?}", result.errors());
    assert!(
        result.warnings().is_empty(),
        "warnings: {:#?}",
        result.warnings()
    );
}
