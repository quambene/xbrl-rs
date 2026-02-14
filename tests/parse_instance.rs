//! Integration tests for parsing XBRL instance files.

use std::path::Path;
use xbrl_rs::{XbrlParser, extract_xbrl};

const INSTANCE_BASE: &str = "test_data/instances/ebilanz";

fn parse_file(path: &Path) -> xbrl_rs::XbrlInstance {
    let xml = std::fs::read_to_string(path).expect("failed to read file");
    let xbrl = extract_xbrl(&xml);
    XbrlParser::new().parse(xbrl).expect("failed to parse")
}

// v6.4

#[test]
fn v64_balance_sheet_restaurateur() {
    let path = Path::new(INSTANCE_BASE).join("v6.4/HandelsbilanzGastronom_PersG.xml");
    let instance = parse_file(&path);
    assert!(!instance.facts().is_empty());
    assert!(!instance.contexts().is_empty());
    assert!(!instance.units().is_empty());
}

#[test]
fn v64_balance_sheet_farmer() {
    let path = Path::new(INSTANCE_BASE).join("v6.4/HandelsbilanzLandwirt_GmbH.xml");
    let instance = parse_file(&path);
    assert!(!instance.facts().is_empty());
    assert!(!instance.contexts().is_empty());
    assert!(!instance.units().is_empty());
}

#[test]
fn v64_tax_balance_sheet_car_dealer() {
    let path = Path::new(INSTANCE_BASE).join("v6.4/SteuerbilanzAutoverkaeufer_PersG.xml");
    let instance = parse_file(&path);
    assert!(!instance.facts().is_empty());
    assert!(!instance.contexts().is_empty());
    assert!(!instance.units().is_empty());
}

// v6.5

#[test]
fn v65_balance_sheet_restaurateur() {
    let path = Path::new(INSTANCE_BASE).join("v6.5/HandelsbilanzGastronom_PersG.xml");
    let instance = parse_file(&path);
    assert!(!instance.facts().is_empty());
    assert!(!instance.contexts().is_empty());
    assert!(!instance.units().is_empty());
}

#[test]
fn v65_balance_sheet_farmer() {
    let path = Path::new(INSTANCE_BASE).join("v6.5/HandelsbilanzLandwirt_GmbH.xml");
    let instance = parse_file(&path);
    assert!(!instance.facts().is_empty());
    assert!(!instance.contexts().is_empty());
    assert!(!instance.units().is_empty());
}

#[test]
fn v65_tax_balance_sheet_car_dealer() {
    let path = Path::new(INSTANCE_BASE).join("v6.4/SteuerbilanzAutoverkaeufer_PersG.xml");
    let instance = parse_file(&path);
    assert!(!instance.facts().is_empty());
    assert!(!instance.contexts().is_empty());
    assert!(!instance.units().is_empty());
}
