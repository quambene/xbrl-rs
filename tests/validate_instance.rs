//! Integration tests for XBRL instance validation.

use std::path::Path;
use xbrl_rs::{EntryPoint, TaxonomySet, XbrlParser, XbrlValidator, extract_xbrl};

const INSTANCE_BASE: &str = "test_data/instances/ebilanz";
const TAXONOMY_BASE: &str = "test_data/taxonomies";

fn parse_instance(path: &Path) -> xbrl_rs::XbrlInstance {
    let xml = std::fs::read_to_string(path).expect("failed to read instance file");
    let xbrl = extract_xbrl(&xml);
    XbrlParser::new()
        .parse(xbrl)
        .expect("failed to parse instance")
}

fn discover_from_instance(instance: &xbrl_rs::XbrlInstance) -> TaxonomySet {
    let entry_points: Vec<_> = instance
        .schema_refs()
        .iter()
        .zip(instance.schema_ref_paths())
        .map(|(href, rel_path)| {
            EntryPoint::new(href.clone(), Path::new(TAXONOMY_BASE).join(rel_path))
        })
        .collect();
    TaxonomySet::discover(&entry_points).expect("failed to discover taxonomy")
}

#[test]
fn validate_instance_v64_balance_sheet_restaurateur() {
    let path = Path::new(INSTANCE_BASE).join("v6.4/HandelsbilanzGastronom_PersG.xml");
    let instance = parse_instance(&path);
    let taxonomy = discover_from_instance(&instance);

    let result = XbrlValidator::new(&instance, &taxonomy).validate_all();

    assert!(result.is_valid());
}

#[test]
fn validate_instance_v64_balance_sheet_farmer() {
    let path = Path::new(INSTANCE_BASE).join("v6.4/HandelsbilanzLandwirt_GmbH.xml");
    let instance = parse_instance(&path);
    let taxonomy = discover_from_instance(&instance);

    let result = XbrlValidator::new(&instance, &taxonomy).validate_all();

    assert!(result.is_valid());
}

#[test]
fn validate_instance_v64_tax_balance_sheet_car_dealer() {
    let path = Path::new(INSTANCE_BASE).join("v6.4/SteuerbilanzAutoverkaeufer_PersG.xml");
    let instance = parse_instance(&path);
    let taxonomy = discover_from_instance(&instance);

    let result = XbrlValidator::new(&instance, &taxonomy).validate_all();

    assert!(result.is_valid());
}

#[test]
fn validate_instance_v65_balance_sheet_restaurateur() {
    let path = Path::new(INSTANCE_BASE).join("v6.5/HandelsbilanzGastronom_PersG.xml");
    let instance = parse_instance(&path);
    let taxonomy = discover_from_instance(&instance);

    let result = XbrlValidator::new(&instance, &taxonomy).validate_all();

    assert!(result.is_valid());
}

#[test]
fn validate_instance_v65_balance_sheet_farmer() {
    let path = Path::new(INSTANCE_BASE).join("v6.5/HandelsbilanzLandwirt_GmbH.xml");
    let instance = parse_instance(&path);
    let taxonomy = discover_from_instance(&instance);

    let result = XbrlValidator::new(&instance, &taxonomy).validate_all();

    assert!(result.is_valid());
}

#[test]
fn validate_instance_v65_tax_balance_sheet_car_dealer() {
    let path = Path::new(INSTANCE_BASE).join("v6.5/SteuerbilanzAutoverkaeufer_PersG.xml");
    let instance = parse_instance(&path);
    let taxonomy = discover_from_instance(&instance);

    let result = XbrlValidator::new(&instance, &taxonomy).validate_all();

    assert!(result.is_valid());
}
