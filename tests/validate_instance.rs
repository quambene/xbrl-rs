//! Integration tests for XBRL instance validation.

use std::path::Path;
use xbrl_rs::{TaxonomySet, XbrlParser, XbrlValidator, extract_xbrl};

const INSTANCE_BASE: &str = "test_data/instances/ebilanz";
const TAXONOMY_BASE: &str = "test_data/taxonomies/german-gaap";

fn parse_instance(path: &Path) -> xbrl_rs::XbrlInstance {
    let xml = std::fs::read_to_string(path).expect("failed to read instance file");
    let xbrl = extract_xbrl(&xml);
    XbrlParser::new()
        .parse(xbrl)
        .expect("failed to parse instance")
}

fn discover_taxonomy_v64() -> TaxonomySet {
    let gcd_path =
        Path::new(TAXONOMY_BASE).join("v6.4/de-gcd-2020-04-01/de-gcd-2020-04-01-shell.xsd");
    let gaap_path = Path::new(TAXONOMY_BASE)
        .join("v6.4/de-gaap-ci-2020-04-01/de-gaap-ci-2020-04-01-shell-fiscal.xsd");

    TaxonomySet::discover(&[&gcd_path, &gaap_path]).expect("failed to discover taxonomy")
}

fn discover_taxonomy_v65() -> TaxonomySet {
    let gcd_path =
        Path::new(TAXONOMY_BASE).join("v6.5/de-gcd-2021-04-14/de-gcd-2021-04-14-shell.xsd");
    let gaap_path = Path::new(TAXONOMY_BASE)
        .join("v6.5/de-gaap-ci-2021-04-14/de-gaap-ci-2021-04-14-shell-fiscal.xsd");

    TaxonomySet::discover(&[&gcd_path, &gaap_path]).expect("failed to discover taxonomy")
}

#[test]
fn validate_instance_v64_balance_sheet_restaurateur() {
    let path = Path::new(INSTANCE_BASE).join("v6.4/HandelsbilanzGastronom_PersG.xml");
    let instance = parse_instance(&path);
    let taxonomy = discover_taxonomy_v64();

    let result = XbrlValidator::new(&instance, &taxonomy).validate_all();

    assert!(result.is_valid());
}

#[test]
fn validate_instance_v64_balance_sheet_farmer() {
    let path = Path::new(INSTANCE_BASE).join("v6.4/HandelsbilanzLandwirt_GmbH.xml");
    let instance = parse_instance(&path);
    let taxonomy = discover_taxonomy_v64();

    let result = XbrlValidator::new(&instance, &taxonomy).validate_all();

    assert!(result.is_valid());
}

#[test]
fn validate_instance_v64_tax_balance_sheet_car_dealer() {
    let path = Path::new(INSTANCE_BASE).join("v6.4/SteuerbilanzAutoverkaeufer_PersG.xml");
    let instance = parse_instance(&path);
    let taxonomy = discover_taxonomy_v64();

    let result = XbrlValidator::new(&instance, &taxonomy).validate_all();

    assert!(result.is_valid());
}

#[test]
fn validate_instance_v65_balance_sheet_restaurateur() {
    let path = Path::new(INSTANCE_BASE).join("v6.5/HandelsbilanzGastronom_PersG.xml");
    let instance = parse_instance(&path);
    let taxonomy = discover_taxonomy_v65();

    let result = XbrlValidator::new(&instance, &taxonomy).validate_all();

    assert!(result.is_valid());
}

#[test]
fn validate_instance_v65_balance_sheet_farmer() {
    let path = Path::new(INSTANCE_BASE).join("v6.5/HandelsbilanzLandwirt_GmbH.xml");
    let instance = parse_instance(&path);
    let taxonomy = discover_taxonomy_v65();

    let result = XbrlValidator::new(&instance, &taxonomy).validate_all();

    assert!(result.is_valid());
}

#[test]
fn validate_instance_v65_tax_balance_sheet_car_dealer() {
    let path = Path::new(INSTANCE_BASE).join("v6.5/SteuerbilanzAutoverkaeufer_PersG.xml");
    let instance = parse_instance(&path);
    let taxonomy = discover_taxonomy_v65();

    let result = XbrlValidator::new(&instance, &taxonomy).validate_all();

    assert!(result.is_valid());
}
