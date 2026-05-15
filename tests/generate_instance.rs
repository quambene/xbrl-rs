use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};
use xbrl_rs::{
    Context, ContextId, EntityIdentifier, ExpandedName, InstanceDocument, NamespacePrefix,
    NamespaceUri, Period, RoleUri, TaxonomySet, Unit, UnitId,
};

const TAXONOMY_ENTRY_POINT: &str = "test_data/taxonomies";
const GCD_SCHEMA: &str =
    "http://www.xbrl.de/taxonomies/de-gcd-2020-04-01/de-gcd-2020-04-01-shell.xsd";
const GAAP_SCHEMA: &str =
    "http://www.xbrl.de/taxonomies/de-gaap-ci-2020-04-01/de-gaap-ci-2020-04-01-shell-fiscal.xsd";

#[test]
#[cfg_attr(not(feature = "taxonomy-test"), ignore)]
fn generate_instance_from_taxonomy() {
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
    let fixture_path = Path::new("test_data/instances/instance_from_taxonomy.xml");
    let expected_instance = InstanceDocument::from_file(fixture_path).unwrap();

    // Fix the fixture
    // let file = std::fs::File::create(fixture_path).unwrap();
    // let mut writer = xbrl_rs::InstanceWriter::new(xbrl_rs::XmlWriter::new(file), true);
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

#[test]
#[cfg_attr(not(feature = "taxonomy-test"), ignore)]
fn generate_instance_from_sections() {
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
    let roles = &[RoleUri::from(
        "http://www.xbrl.de/taxonomies/de-gaap-ci/role/balanceSheet",
    )];

    // Build the instance from the taxonomy.
    let instance = InstanceDocument::from_sections(
        &taxonomy,
        roles,
        namespaces,
        instant_ctx,
        duration_ctx,
        vec![],
        vec![],
        &[monetary_unit, pure_unit],
        &[],
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
    let fixture_path = Path::new("test_data/instances/instance_from_sections.xml");
    let expected_instance = InstanceDocument::from_file(fixture_path).unwrap();

    // Fix the fixture
    // let file = std::fs::File::create(fixture_path).unwrap();
    // let mut writer: xbrl_rs::InstanceWriter<_> =
    //     xbrl_rs::InstanceWriter::new(xbrl_rs::XmlWriter::new(file), true);
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
