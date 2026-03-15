//! Integration tests for parsing XBRL instance files.

use std::path::Path;
use xbrl_rs::InstanceDocument;

const INSTANCE_BASE: &str = "test_data/instances";

fn parse_instance(path: &Path) -> InstanceDocument {
    InstanceDocument::from_file(path).expect("failed to parse instance")
}

#[test]
fn balance_sheet_v64() {
    let path = Path::new(INSTANCE_BASE).join("balance_sheet_v64.xml");
    let instance = parse_instance(&path);
    assert!(!instance.facts().is_empty());
    assert!(!instance.contexts().is_empty());
    assert!(!instance.units().is_empty());
}

#[test]
fn balance_sheet_v65() {
    let path = Path::new(INSTANCE_BASE).join("balance_sheet_v65.xml");
    let instance = parse_instance(&path);
    assert!(!instance.facts().is_empty());
    assert!(!instance.contexts().is_empty());
    assert!(!instance.units().is_empty());
}
