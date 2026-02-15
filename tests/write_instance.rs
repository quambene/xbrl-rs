use roxmltree::Document;
use std::{path::PathBuf, str::FromStr};
use xbrl_rs::{TaxonomySet, XmlWriter};

const TAXONOMY_ENTRY_POINT: &str = "test_data/taxonomies";

#[test]
fn write_empty_instance() {
    let entry_point = PathBuf::from_str(TAXONOMY_ENTRY_POINT).unwrap();
    let gcd = "http://www.xbrl.de/taxonomies/de-gcd-2020-04-01/de-gcd-2020-04-01-shell.xsd";
    let gaap = "http://www.xbrl.de/taxonomies/de-gaap-ci-2020-04-01/de-gaap-ci-2020-04-01-shell-fiscal.xsd";
    let taxonomy =
        TaxonomySet::discover(vec![gcd.to_owned(), gaap.to_owned()], entry_point).unwrap();
    let instance = taxonomy.create_instance();

    // Validate the generated XBRL
    let res = instance.validate(&taxonomy);
    assert!(res.is_valid());

    let mut writer: XmlWriter<Vec<u8>> = XmlWriter::new(Vec::new());
    instance.to_xml(&mut writer).unwrap();
    let xml = String::from_utf8(writer.into_inner()).unwrap();

    // Parse the generated XML
    let doc = Document::parse(&xml);

    assert!(doc.is_ok());
}
