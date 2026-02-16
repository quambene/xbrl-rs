//! Integration tests for parsing XBRL instance files.

use quick_xml::Reader;
use std::{fs::File, io::BufReader, path::Path};
use xbrl_rs::XbrlInstance;

const INSTANCE_BASE: &str = "test_data/instances/ebilanz";

fn parse_instance(path: &Path) -> XbrlInstance {
    let file = File::open(path).expect("failed to open instance file");
    let mut reader = Reader::from_reader(BufReader::new(file));

    XbrlInstance::from_xml(&mut reader).expect("failed to parse instance")
}

// v6.4

#[test]
fn v64_balance_sheet_restaurateur() {
    let path = Path::new(INSTANCE_BASE).join("v6.4/HandelsbilanzGastronom_PersG.xml");
    let instance = parse_instance(&path);
    assert!(!instance.facts().is_empty());
    assert!(!instance.contexts().is_empty());
    assert!(!instance.units().is_empty());
}

#[test]
fn v64_balance_sheet_farmer() {
    let path = Path::new(INSTANCE_BASE).join("v6.4/HandelsbilanzLandwirt_GmbH.xml");
    let instance = parse_instance(&path);
    assert!(!instance.facts().is_empty());
    assert!(!instance.contexts().is_empty());
    assert!(!instance.units().is_empty());
}

#[test]
fn v64_tax_balance_sheet_car_dealer() {
    let path = Path::new(INSTANCE_BASE).join("v6.4/SteuerbilanzAutoverkaeufer_PersG.xml");
    let instance = parse_instance(&path);
    assert!(!instance.facts().is_empty());
    assert!(!instance.contexts().is_empty());
    assert!(!instance.units().is_empty());
}

// v6.5

#[test]
fn v65_balance_sheet_restaurateur() {
    let path = Path::new(INSTANCE_BASE).join("v6.5/HandelsbilanzGastronom_PersG.xml");
    let instance = parse_instance(&path);
    assert!(!instance.facts().is_empty());
    assert!(!instance.contexts().is_empty());
    assert!(!instance.units().is_empty());
}

#[test]
fn v65_balance_sheet_farmer() {
    let path = Path::new(INSTANCE_BASE).join("v6.5/HandelsbilanzLandwirt_GmbH.xml");
    let instance = parse_instance(&path);
    assert!(!instance.facts().is_empty());
    assert!(!instance.contexts().is_empty());
    assert!(!instance.units().is_empty());
}

#[test]
fn v65_tax_balance_sheet_car_dealer() {
    let path = Path::new(INSTANCE_BASE).join("v6.4/SteuerbilanzAutoverkaeufer_PersG.xml");
    let instance = parse_instance(&path);
    assert!(!instance.facts().is_empty());
    assert!(!instance.contexts().is_empty());
    assert!(!instance.units().is_empty());
}
