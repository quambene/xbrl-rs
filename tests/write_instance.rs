use roxmltree::Document;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    str::FromStr,
};
use xbrl_rs::{
    Context, ContextId, EntityIdentifier, ExpandedName, InstanceDocument, InstanceWriter,
    NamespacePrefix, NamespaceUri, Period, TaxonomySet, Unit, UnitId, XmlWriter,
};

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

#[test]
#[cfg_attr(not(feature = "taxonomy-test"), ignore)]
fn generate_instance() {
    // Discover the taxonomy from the local test data
    let entry_point = PathBuf::from(TAXONOMY_ENTRY_POINT);
    let taxonomy = TaxonomySet::discover(
        vec![GCD_SCHEMA.to_owned(), GAAP_SCHEMA.to_owned()],
        entry_point,
    )
    .unwrap();
    let namespaces: HashMap<NamespacePrefix, NamespaceUri> = HashMap::from_iter([
        ("xbrli".into(), "http://www.xbrl.org/2003/instance".into()),
        ("link".into(), "http://www.xbrl.org/2003/linkbase".into()),
        ("xlink".into(), "http://www.w3.org/1999/xlink".into()),
        (
            "xsi".into(),
            "http://www.w3.org/2001/XMLSchema-instance".into(),
        ),
        ("iso4217".into(), "http://www.xbrl.org/2003/iso4217".into()),
        (
            "de-gcd".into(),
            "http://www.xbrl.de/taxonomies/de-gcd-2020-04-01".into(),
        ),
        (
            "de-gaap-ci".into(),
            "http://www.xbrl.de/taxonomies/de-gaap-ci-2020-04-01".into(),
        ),
    ]);

    // Define an instant context (balance-sheet date) and a duration context (fiscal year)
    let entity = EntityIdentifier {
        scheme: "http://example.com/id".to_owned(),
        value: "0000000000000".to_owned(),
    };
    let instant_ctx = Context::new(
        ContextId::from("I-2020"),
        entity.clone(),
        Period::Instant {
            date: "2020-12-31".to_owned(),
        },
    );
    let duration_ctx = Context::new(
        ContextId::from("D-2020"),
        entity,
        Period::Duration {
            start: "2020-01-01".to_owned(),
            end: "2020-12-31".to_owned(),
        },
    );

    // Define units: monetary (EUR) and pure (for dimensionless numeric items)
    let monetary_unit = Unit::new(
        UnitId::from("EUR"),
        vec![ExpandedName {
            namespace_uri: NamespaceUri::from("http://www.xbrl.org/2003/iso4217"),
            local_name: "EUR".to_owned(),
        }],
        vec![],
    );
    let pure_unit = Unit::new(
        UnitId::from("pure"),
        vec![ExpandedName {
            namespace_uri: NamespaceUri::from("http://www.xbrl.org/2003/instance"),
            local_name: "pure".to_owned(),
        }],
        vec![],
    );

    // Build the instance from the taxonomy.
    let instance = InstanceDocument::from_taxonomy(
        &taxonomy,
        namespaces,
        instant_ctx,
        duration_ctx,
        vec![],
        vec![],
        &[monetary_unit, pure_unit],
    );

    // Validate the generated XBRL
    let res = instance.validate(&taxonomy);
    assert!(
        res.errors().is_empty(),
        "Validation errors: {:?}",
        res.errors()
    );
    assert!(res.is_valid());
    assert!(
        res.warnings().is_empty(),
        "Validation warnings: {:?}",
        res.warnings()
    );

    // Deserialize fixture from XML
    let fixture_path = Path::new("test_data/instances/generated_instance.xml");
    let expected_instance = InstanceDocument::from_file(fixture_path).unwrap();

    // Fix the fixture
    // let file = std::fs::File::create(fixture_path).unwrap();
    // let mut writer = InstanceWriter::new(XmlWriter::new(file), true);
    // writer.write(&instance).unwrap();

    let mut role_refs = instance.role_refs().to_vec();
    role_refs.sort();
    let mut expected_role_refs = expected_instance.role_refs().to_vec();
    expected_role_refs.sort();

    assert_eq!(instance.namespaces(), expected_instance.namespaces());
    assert_eq!(instance.schema_refs(), expected_instance.schema_refs());
    assert_eq!(role_refs, expected_role_refs);
    assert_eq!(instance.arcrole_refs(), expected_instance.arcrole_refs());
    assert_eq!(
        instance.contexts().len(),
        expected_instance.contexts().len()
    );
    assert_eq!(instance.units(), expected_instance.units());
    assert_eq!(instance.facts().len(), expected_instance.facts().len());
    assert_eq!(
        instance.footnote_links().len(),
        expected_instance.footnote_links().len()
    );
}
