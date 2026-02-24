//! Integration tests for parsing XBRL instance files.

use quick_xml::Reader;
use std::{fs::File, io::BufReader, path::Path};
use xbrl_rs::InstanceDocument;

const INSTANCE_BASE: &str = "test_data/instances";

fn parse_instance(path: &Path) -> InstanceDocument {
    let file = File::open(path).expect("failed to open instance file");
    let mut reader = Reader::from_reader(BufReader::new(file));

    InstanceDocument::from_xml(&mut reader).expect("failed to parse instance")
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
