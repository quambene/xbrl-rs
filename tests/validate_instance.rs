//! Integration tests for XBRL instance validation.

use quick_xml::Reader;
use std::{
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
    str::FromStr,
};
use xbrl_rs::{TaxonomySet, XbrlInstance};

const INSTANCE_BASE: &str = "test_data/instances/ebilanz";
const TAXONOMY_ENTRY_POINT: &str = "test_data/taxonomies";

fn parse_instance(path: &Path) -> XbrlInstance {
    let file = File::open(path).expect("failed to open instance file");
    let mut reader = Reader::from_reader(BufReader::new(file));

    XbrlInstance::from_xml(&mut reader).expect("failed to parse instance")
}

#[test]
fn validate_instance_v64_balance_sheet_restaurateur() {
    let path = Path::new(INSTANCE_BASE).join("v6.4/HandelsbilanzGastronom_PersG.xml");
    let instance = parse_instance(&path);
    let entry_point = PathBuf::from_str(TAXONOMY_ENTRY_POINT).unwrap();
    let taxonomy = TaxonomySet::discover(instance.schema_refs().to_vec(), entry_point).unwrap();

    let result = instance.validate(&taxonomy);

    assert!(result.is_valid());
}

#[test]
fn validate_instance_v64_balance_sheet_farmer() {
    let path = Path::new(INSTANCE_BASE).join("v6.4/HandelsbilanzLandwirt_GmbH.xml");
    let instance = parse_instance(&path);
    let entry_point = PathBuf::from_str(TAXONOMY_ENTRY_POINT).unwrap();
    let taxonomy = TaxonomySet::discover(instance.schema_refs().to_vec(), entry_point).unwrap();

    let result = instance.validate(&taxonomy);

    assert!(result.is_valid());
}

#[test]
fn validate_instance_v64_tax_balance_sheet_car_dealer() {
    let path = Path::new(INSTANCE_BASE).join("v6.4/SteuerbilanzAutoverkaeufer_PersG.xml");
    let instance = parse_instance(&path);
    let entry_point = PathBuf::from_str(TAXONOMY_ENTRY_POINT).unwrap();
    let taxonomy = TaxonomySet::discover(instance.schema_refs().to_vec(), entry_point).unwrap();

    let result = instance.validate(&taxonomy);

    assert!(result.is_valid());
}

#[test]
fn validate_instance_v65_balance_sheet_restaurateur() {
    let path = Path::new(INSTANCE_BASE).join("v6.5/HandelsbilanzGastronom_PersG.xml");
    let instance = parse_instance(&path);
    let entry_point = PathBuf::from_str(TAXONOMY_ENTRY_POINT).unwrap();
    let taxonomy = TaxonomySet::discover(instance.schema_refs().to_vec(), entry_point).unwrap();

    let result = instance.validate(&taxonomy);

    assert!(result.is_valid());
}

#[test]
fn validate_instance_v65_balance_sheet_farmer() {
    let path = Path::new(INSTANCE_BASE).join("v6.5/HandelsbilanzLandwirt_GmbH.xml");
    let instance = parse_instance(&path);
    let entry_point = PathBuf::from_str(TAXONOMY_ENTRY_POINT).unwrap();
    let taxonomy = TaxonomySet::discover(instance.schema_refs().to_vec(), entry_point).unwrap();

    let result = instance.validate(&taxonomy);

    assert!(result.is_valid());
}

#[test]
fn validate_instance_v65_tax_balance_sheet_car_dealer() {
    let path = Path::new(INSTANCE_BASE).join("v6.5/SteuerbilanzAutoverkaeufer_PersG.xml");
    let instance = parse_instance(&path);
    let entry_point = PathBuf::from_str(TAXONOMY_ENTRY_POINT).unwrap();
    let taxonomy = TaxonomySet::discover(instance.schema_refs().to_vec(), entry_point).unwrap();

    let result = instance.validate(&taxonomy);

    assert!(result.is_valid());
}
