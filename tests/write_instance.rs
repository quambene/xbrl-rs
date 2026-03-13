use roxmltree::Document;
use std::{path::PathBuf, str::FromStr};
use xbrl_rs::{
    Context, ContextId, EntityIdentifier, InstanceDocument, Period, TaxonomySet, Unit, UnitId,
    XmlReader, XmlWriter,
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
#[cfg_attr(not(feature = "taxonomy-test"), ignore)]
fn generate_instance() {
    // 1. Discover the taxonomy from the local test data
    let entry_point = PathBuf::from(TAXONOMY_ENTRY_POINT);
    let taxonomy = TaxonomySet::discover(
        vec![GCD_SCHEMA.to_owned(), GAAP_SCHEMA.to_owned()],
        entry_point,
    )
    .unwrap();

    // 2. Define an instant context (balance-sheet date) and a duration context (fiscal year)
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

    // 3. Define units: monetary (EUR) and pure (for dimensionless numeric items)
    let monetary_unit = Unit::new(UnitId::from("EUR"), "iso4217:EUR".to_owned());
    let pure_unit = Unit::new(UnitId::from("pure"), "xbrli:pure".to_owned());

    // 4. Build the instance from the taxonomy.
    let mut instance = InstanceDocument::from_taxonomy(
        &taxonomy,
        instant_ctx,
        duration_ctx,
        &[monetary_unit, pure_unit],
    );
    assert_eq!(instance.item_fact_count(), 3609);

    // 5. Register namespace declarations from all discovered schemas
    for schema in taxonomy.schemas().values() {
        for (prefix, uri) in &schema.namespaces {
            instance.add_namespace(prefix.clone(), uri.clone());
        }
    }

    // 6. Validate the generated XBRL
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

    // 7. Serialize to XML
    let mut writer: XmlWriter<Vec<u8>> = XmlWriter::new(Vec::new());
    instance.to_xml(&mut writer).unwrap();
    let xml = String::from_utf8(writer.into_inner()).unwrap();

    let reader = XmlReader::from_str(&xml);
    let instance_from_xml = InstanceDocument::from_xml(reader).unwrap();

    assert_eq!(instance.schema_refs(), instance_from_xml.schema_refs());
    assert_eq!(instance.role_refs(), instance_from_xml.role_refs());
    assert_eq!(instance.arcrole_refs(), instance_from_xml.arcrole_refs());
    assert_eq!(instance.namespaces(), instance_from_xml.namespaces());
    assert_eq!(instance.root_xml_lang(), instance_from_xml.root_xml_lang());
    assert_eq!(instance.document_name(), instance_from_xml.document_name());
    assert_eq!(
        instance.contexts().len(),
        instance_from_xml.contexts().len()
    );
    assert_eq!(instance.units(), instance_from_xml.units());
    assert_eq!(instance.facts().len(), instance_from_xml.facts().len());
    assert_eq!(
        instance.footnote_links().len(),
        instance_from_xml.footnote_links().len()
    );
}
