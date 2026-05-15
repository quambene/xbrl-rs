use roxmltree::Document;
use std::{path::PathBuf, str::FromStr};
use xbrl_rs::{InstanceDocument, InstanceWriter, TaxonomySet, XmlWriter};

const TAXONOMY_ENTRY_POINT: &str = "test_data/taxonomies";
const GCD_SCHEMA: &str =
    "http://www.xbrl.de/taxonomies/de-gcd-2020-04-01/de-gcd-2020-04-01-shell.xsd";
const GAAP_SCHEMA: &str =
    "http://www.xbrl.de/taxonomies/de-gaap-ci-2020-04-01/de-gaap-ci-2020-04-01-shell-fiscal.xsd";

#[test]
#[cfg_attr(not(feature = "taxonomy-test"), ignore)]
fn write_empty_instance() {
    let entry_point = PathBuf::from_str(TAXONOMY_ENTRY_POINT).unwrap();
    let taxonomy = TaxonomySet::discover(
        vec![GCD_SCHEMA.to_owned(), GAAP_SCHEMA.to_owned()],
        entry_point,
    )
    .unwrap();
    let namespaces = [
        ("xbrli", "http://www.xbrl.org/2003/instance"),
        ("link", "http://www.xbrl.org/2003/linkbase"),
        ("xlink", "http://www.w3.org/1999/xlink"),
        ("xsi", "http://www.w3.org/2001/XMLSchema-instance"),
    ];

    let mut instance = InstanceDocument::default();

    for namespace in namespaces {
        instance.add_namespace(namespace.0.into(), namespace.1.into());
    }

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

    let mut writer = InstanceWriter::new(XmlWriter::new(Vec::new()), false);

    writer.write(&instance).unwrap();
    let xml = String::from_utf8(writer.into_inner()).unwrap();

    // Parse the generated XML
    let doc = Document::parse(&xml);

    assert!(doc.is_ok(), "Failed to parse XML: {:?}", doc.err());
}
