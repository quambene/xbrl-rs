use roxmltree::Document;
use std::{
    io::Cursor,
    path::{Path, PathBuf},
    str::FromStr,
};
use xbrl_rs::{Fact, InstanceDocument, TaxonomySet, XmlReader, XmlWriter};

const TAXONOMY_ENTRY_POINT: &str = "test_data/taxonomies";

#[test]
fn write_empty_instance() {
    let entry_point = PathBuf::from_str(TAXONOMY_ENTRY_POINT).unwrap();
    let gcd = "http://www.xbrl.de/taxonomies/de-gcd-2020-04-01/de-gcd-2020-04-01-shell.xsd";
    let gaap = "http://www.xbrl.de/taxonomies/de-gaap-ci-2020-04-01/de-gaap-ci-2020-04-01-shell-fiscal.xsd";
    let taxonomy =
        TaxonomySet::discover(vec![gcd.to_owned(), gaap.to_owned()], entry_point).unwrap();

    let mut instance = InstanceDocument::default();

    for url in taxonomy.schema_refs().keys() {
        instance.add_schema_ref(url.to_string());
    }

    for schema in taxonomy.schemas().values() {
        for (prefix, uri) in &schema.namespaces {
            instance.add_namespace(prefix.clone(), uri.clone());
        }
    }

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

#[test]
fn write_roundtrip_preserves_tuple_children() {
    let path = Path::new("test_data/examples/HandelsbilanzLandwirt_GmbH.xml");
    let source = std::fs::read_to_string(path).unwrap();

    let mut reader = XmlReader::from_str(&source);
    let instance = InstanceDocument::from_xml(&mut reader).unwrap();

    let original_item_count = instance.item_fact_count();

    let mut writer: XmlWriter<Vec<u8>> = XmlWriter::new(Vec::new());
    instance.to_xml(&mut writer).unwrap();
    let xml = writer.into_inner();

    let mut reparsed_reader = XmlReader::from_reader(Cursor::new(xml));
    let reparsed = InstanceDocument::from_xml(&mut reparsed_reader).unwrap();

    assert_eq!(reparsed.item_fact_count(), original_item_count);

    let has_shareholder_tuple = reparsed
        .facts()
        .iter()
        .any(|fact| matches!(fact, Fact::Tuple(tuple) if tuple.concept() == "de-gcd:genInfo.company.id.shareholder"));
    assert!(has_shareholder_tuple);
}
