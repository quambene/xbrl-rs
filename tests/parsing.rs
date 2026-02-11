//! Integration tests for parsing XBRL instance files.

use xbrl_rs::{XbrlParser, extract_xbrl};

fn parse_file(path: &str) -> xbrl_rs::XbrlInstance {
    let xml = std::fs::read_to_string(path).expect("failed to read file");
    let xbrl = extract_xbrl(&xml);
    XbrlParser::new().parse(xbrl).expect("failed to parse")
}

// v6.4

#[test]
fn v64_balance_sheet_restaurateur() {
    let instance = parse_file("test_data/samples/ebilanz/v6.4/HandelsbilanzGastronom_PersG.xml");
    assert!(!instance.facts().is_empty());
    assert!(!instance.contexts().is_empty());
    assert!(!instance.units().is_empty());
}

#[test]
fn v64_balance_sheet_farmer() {
    let instance = parse_file("test_data/samples/ebilanz/v6.4/HandelsbilanzLandwirt_GmbH.xml");
    assert!(!instance.facts().is_empty());
    assert!(!instance.contexts().is_empty());
    assert!(!instance.units().is_empty());
}

#[test]
fn v64_tax_balance_car_dealer() {
    let instance =
        parse_file("test_data/samples/ebilanz/v6.4/SteuerbilanzAutoverkaeufer_PersG.xml");
    assert!(!instance.facts().is_empty());
    assert!(!instance.contexts().is_empty());
    assert!(!instance.units().is_empty());
}

// v6.5

#[test]
fn v65_balance_sheet_restaurateur() {
    let instance = parse_file("test_data/samples/ebilanz/v6.5/HandelsbilanzGastronom_PersG.xml");
    assert!(!instance.facts().is_empty());
    assert!(!instance.contexts().is_empty());
    assert!(!instance.units().is_empty());
}

#[test]
fn v65_balance_sheet_farmer() {
    let instance = parse_file("test_data/samples/ebilanz/v6.5/HandelsbilanzLandwirt_GmbH.xml");
    assert!(!instance.facts().is_empty());
    assert!(!instance.contexts().is_empty());
    assert!(!instance.units().is_empty());
}

#[test]
fn v65_tax_balance_car_dealer() {
    let instance =
        parse_file("test_data/samples/ebilanz/v6.5/SteuerbilanzAutoverkaeufer_PersG.xml");
    assert!(!instance.facts().is_empty());
    assert!(!instance.contexts().is_empty());
    assert!(!instance.units().is_empty());
}
