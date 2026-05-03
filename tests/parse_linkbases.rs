use rust_decimal::Decimal;
use std::{path::PathBuf, str::FromStr};
use xbrl_rs::{ExpandedName, Label, NamespaceUri, PeriodType, TaxonomySet};

const TAXONOMY_ENTRY_POINT: &str = "test_data/taxonomies";

#[test]
#[cfg_attr(not(feature = "taxonomy-test"), ignore)]
fn schema_by_namespace() {
    let entry_point = PathBuf::from_str(TAXONOMY_ENTRY_POINT).unwrap();
    let gcd = "http://www.xbrl.de/taxonomies/de-gcd-2020-04-01/de-gcd-2020-04-01-shell.xsd";
    let dts = TaxonomySet::discover(vec![gcd.to_owned()], entry_point).unwrap();

    let gcd = dts
        .schema_by_namespace("http://www.xbrl.de/taxonomies/de-gcd-2020-04-01")
        .expect("GCD schema not found by namespace");
    assert!(!gcd.concepts.is_empty());
    assert!(
        gcd.concepts
            .iter()
            .any(|concept| concept.name.local_name == "genInfo")
    );
}

#[test]
#[cfg_attr(not(feature = "taxonomy-test"), ignore)]
fn parse_labels_linkbase() {
    let entry_point = PathBuf::from_str(TAXONOMY_ENTRY_POINT).unwrap();
    let gaap = "http://www.xbrl.de/taxonomies/de-gaap-ci-2020-04-01/de-gaap-ci-2020-04-01-shell-fiscal.xsd";
    let dts = TaxonomySet::discover(vec![gaap.to_owned()], entry_point).unwrap();

    assert!(
        !dts.labels_map().is_empty(),
        "Expected labels to be parsed from label linkbases"
    );

    // bs.ass should have labels in German and English
    let bs_ass_labels = dts
        .labels(&ExpandedName {
            namespace_uri: NamespaceUri::from(
                "http://www.xbrl.de/taxonomies/de-gaap-ci-2020-04-01",
            ),
            local_name: "bs.ass".to_string(),
        })
        .expect("Expected labels for de-gaap-ci_bs.ass");

    assert!(bs_ass_labels.contains(&Label {
        role: "http://www.xbrl.org/2003/role/terseLabel".to_owned(),
        lang: "en".to_owned(),
        text: "Total assets".to_owned(),
    }));
    assert!(bs_ass_labels.contains(&Label {
        role: "http://www.xbrl.org/2003/role/label".to_owned(),
        lang: "en".to_owned(),
        text: "Balance sheet, total assets".to_owned(),
    }));
    assert!(bs_ass_labels.contains(&Label {
        role: "http://www.xbrl.org/2003/role/terseLabel".to_owned(),
        lang: "de".to_owned(),
        text: "Summe Aktiva".to_owned(),
    }));
    assert!(bs_ass_labels.contains(&Label {
        role: "http://www.xbrl.org/2003/role/label".to_owned(),
        lang: "de".to_owned(),
        text: "Bilanzsumme, Summe Aktiva".to_owned(),
    }));
    assert!(bs_ass_labels.contains(&Label {
        role: "http://www.xbrl.org/2003/role/definitionGuidance".to_owned(),
        lang: "de".to_owned(),
        text: "Dieser Wert muss der Bilanzsumme, Summe Passiva entsprechen".to_owned(),
    }));
}

#[test]
#[cfg_attr(not(feature = "taxonomy-test"), ignore)]
fn parse_labels_linkbase_multiple_roles() {
    let entry_point = PathBuf::from_str(TAXONOMY_ENTRY_POINT).unwrap();
    let gcd = "http://www.xbrl.de/taxonomies/de-gcd-2020-04-01/de-gcd-2020-04-01-shell.xsd";
    let dts = TaxonomySet::discover(vec![gcd.to_owned()], entry_point).unwrap();

    // Find a concept that has both a standard label and documentation
    let concept_labels = dts.labels_map();

    let has_multiple_roles = concept_labels.values().any(|labels| {
        let has_label = labels.iter().any(|l| l.role.ends_with("/label"));
        let has_doc = labels.iter().any(|l| l.role.ends_with("/documentation"));
        has_label && has_doc
    });
    assert!(
        has_multiple_roles,
        "Expected at least one concept with both label and documentation roles"
    );
}

#[test]
#[cfg_attr(not(feature = "taxonomy-test"), ignore)]
fn parse_presentation_linkbase() {
    let entry_point = PathBuf::from_str(TAXONOMY_ENTRY_POINT).unwrap();
    let gaap = "http://www.xbrl.de/taxonomies/de-gaap-ci-2020-04-01/de-gaap-ci-2020-04-01-shell-fiscal.xsd";
    let dts = TaxonomySet::discover(vec![gaap.to_owned()], entry_point).unwrap();

    assert!(
        !dts.presentations().is_empty(),
        "Expected presentation arcs to be parsed"
    );

    // Balance sheet role should have parent-child arcs
    let bs_role = "http://www.xbrl.de/taxonomies/de-gaap-ci/role/balanceSheet";
    let bs_arcs = dts
        .presentation_arcs(bs_role)
        .expect("Expected presentation arcs for balanceSheet role");

    // bs.ass -> bs.ass.fixAss should be a known parent-child relationship
    assert!(
        bs_arcs.iter().any(|arc| arc.from
            == ExpandedName {
                namespace_uri: NamespaceUri::from(
                    "http://www.xbrl.de/taxonomies/de-gaap-ci-2020-04-01"
                ),
                local_name: "bs.ass".to_string()
            }
            && arc.to
                == ExpandedName {
                    namespace_uri: NamespaceUri::from(
                        "http://www.xbrl.de/taxonomies/de-gaap-ci-2020-04-01"
                    ),
                    local_name: "bs.ass.fixAss".to_string()
                }),
        "Expected bs.ass -> bs.ass.fixAss presentation arc"
    );
}

#[test]
#[cfg_attr(not(feature = "taxonomy-test"), ignore)]
fn parse_calculation_linkbase() {
    let entry_point = PathBuf::from_str(TAXONOMY_ENTRY_POINT).unwrap();
    let gaap = "http://www.xbrl.de/taxonomies/de-gaap-ci-2020-04-01/de-gaap-ci-2020-04-01-shell-fiscal.xsd";
    let dts = TaxonomySet::discover(vec![gaap.to_owned()], entry_point).unwrap();

    assert!(
        !dts.calculations().is_empty(),
        "Expected calculation arcs to be parsed"
    );

    let bs_role = "http://www.xbrl.de/taxonomies/de-gaap-ci/role/balanceSheet";
    let bs_arcs = dts
        .calculation_arcs(bs_role)
        .expect("Expected calculation arcs for balanceSheet role");

    // bs.ass should sum its children with weight 1
    let child_arc = bs_arcs
        .iter()
        .find(|arc| {
            arc.from
                == ExpandedName {
                    namespace_uri: NamespaceUri::from(
                        "http://www.xbrl.de/taxonomies/de-gaap-ci-2020-04-01",
                    ),
                    local_name: "bs.ass".to_string(),
                }
                && arc.to
                    == ExpandedName {
                        namespace_uri: NamespaceUri::from(
                            "http://www.xbrl.de/taxonomies/de-gaap-ci-2020-04-01",
                        ),
                        local_name: "bs.ass.fixAss".to_string(),
                    }
        })
        .expect("Expected bs.ass -> bs.ass.fixAss calculation arc");
    assert_eq!(child_arc.weight, Decimal::ONE);
}

#[test]
#[cfg_attr(not(feature = "taxonomy-test"), ignore)]
fn parse_definition_linkbase() {
    let entry_point = PathBuf::from_str(TAXONOMY_ENTRY_POINT).unwrap();
    let gaap = "http://www.xbrl.de/taxonomies/de-gaap-ci-2020-04-01/de-gaap-ci-2020-04-01-shell-fiscal.xsd";
    let dts = TaxonomySet::discover(vec![gaap.to_owned()], entry_point).unwrap();

    assert!(
        !dts.definitions().is_empty(),
        "Expected definition arcs to be parsed"
    );

    // At least one role should have domain-member arcs
    let has_domain_member = dts
        .definitions()
        .values()
        .any(|arcs| arcs.iter().any(|a| a.arcrole.contains("domain-member")));
    assert!(
        has_domain_member,
        "Expected at least one domain-member definition arc"
    );
}

#[test]
#[cfg_attr(not(feature = "taxonomy-test"), ignore)]
fn parse_reference_linkbase() {
    let entry_point = PathBuf::from_str(TAXONOMY_ENTRY_POINT).unwrap();
    let gcd = "http://www.xbrl.de/taxonomies/de-gcd-2020-04-01/de-gcd-2020-04-01-shell.xsd";
    let gaap = "http://www.xbrl.de/taxonomies/de-gaap-ci-2020-04-01/de-gaap-ci-2020-04-01-shell-fiscal.xsd";
    let dts = TaxonomySet::discover(vec![gcd.to_owned(), gaap.to_owned()], entry_point).unwrap();

    assert!(
        !dts.references().is_empty(),
        "Expected references to be parsed"
    );

    // bs.ass should have an HGB reference
    let bs_refs = dts.references_for("de-gaap-ci_bs.ass");
    assert!(bs_refs.is_some());

    let references = dts.references();
    assert!(
        references.values().flatten().any(|reference| {
            reference
                .parts
                .iter()
                .any(|part| part.name.ends_with(":fiscalRequirement") && part.value == "Mussfeld")
        }),
        "Expected at least one parsed fiscalRequirement=Mussfeld reference part"
    );
}

#[test]
#[cfg_attr(not(feature = "taxonomy-test"), ignore)]
fn linkbase_refs_have_roles() {
    let entry_point = PathBuf::from_str(TAXONOMY_ENTRY_POINT).unwrap();
    let gcd = "http://www.xbrl.de/taxonomies/de-gcd-2020-04-01/de-gcd-2020-04-01-shell.xsd";
    let dts = TaxonomySet::discover(vec![gcd.to_owned()], entry_point).unwrap();

    // The GCD main schema has label and reference linkbaseRefs
    let gcd = dts
        .schema_by_namespace("http://www.xbrl.de/taxonomies/de-gcd-2020-04-01")
        .unwrap();

    let label_refs: Vec<_> = gcd
        .linkbase_refs
        .iter()
        .filter(|lr| {
            lr.role
                .as_deref()
                .is_some_and(|r| r.contains("labelLinkbaseRef"))
        })
        .collect();
    assert!(
        label_refs.len() >= 2,
        "Expected at least 2 label linkbaseRefs (de + en), got {}",
        label_refs.len()
    );
}

#[test]
#[cfg_attr(not(feature = "taxonomy-test"), ignore)]
fn find_element_by_id() {
    let entry_point = PathBuf::from_str(TAXONOMY_ENTRY_POINT).unwrap();
    let gaap = "http://www.xbrl.de/taxonomies/de-gaap-ci-2020-04-01/de-gaap-ci-2020-04-01-shell-fiscal.xsd";
    let dts = TaxonomySet::discover(vec![gaap.to_owned()], entry_point).unwrap();

    let concept = dts
        .find_concept_by_id("de-gaap-ci_bs.ass")
        .expect("Expected to find element by ID");
    assert_eq!(concept.name.local_name, "bs.ass");
    assert_eq!(concept.period_type, Some(PeriodType::Instant));
    assert!(!concept.is_abstract);
}

#[test]
#[cfg_attr(not(feature = "taxonomy-test"), ignore)]
fn qualified_name() {
    let entry_point = PathBuf::from_str(TAXONOMY_ENTRY_POINT).unwrap();
    let gaap = "http://www.xbrl.de/taxonomies/de-gaap-ci-2020-04-01/de-gaap-ci-2020-04-01-shell-fiscal.xsd";
    let dts = TaxonomySet::discover(vec![gaap.to_owned()], entry_point).unwrap();

    assert_eq!(
        dts.qualified_name("de-gaap-ci_bs.ass"),
        Some(ExpandedName {
            namespace_uri: "http://www.xbrl.de/taxonomies/de-gaap-ci-2020-04-01".into(),
            local_name: "bs.ass".to_string()
        })
    );
    assert_eq!(
        dts.qualified_name("de-gaap-ci_bs.ass.fixAss"),
        Some(ExpandedName {
            namespace_uri: "http://www.xbrl.de/taxonomies/de-gaap-ci-2020-04-01".into(),
            local_name: "bs.ass.fixAss".to_string()
        })
    );
    assert_eq!(dts.qualified_name("nonexistent_element"), None);
}
