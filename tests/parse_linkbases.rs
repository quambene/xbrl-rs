use std::path::Path;
use xbrl_rs::{EntryPoint, Label, TaxonomySet};

const TAXONOMY_BASE: &str = "test_data/taxonomies/german-gaap/v6.9";
const TAXONOMY_URL_BASE: &str = "http://www.xbrl.de/taxonomies";

fn entry_point(relative_path: &str) -> EntryPoint {
    EntryPoint::new(
        format!("{TAXONOMY_URL_BASE}/{relative_path}"),
        Path::new(TAXONOMY_BASE).join(relative_path),
    )
}

#[test]
fn schema_by_namespace() {
    let dts = TaxonomySet::discover(&[entry_point(
        "de-gcd-2025-04-01/de-gcd-2025-04-01-shell.xsd",
    )])
    .unwrap();

    let gcd = dts
        .schema_by_namespace("http://www.xbrl.de/taxonomies/de-gcd-2025-04-01")
        .expect("GCD schema not found by namespace");
    assert!(!gcd.elements.is_empty());
    assert!(gcd.elements.iter().any(|e| e.name == "genInfo"));
}

#[test]
fn parse_labels_linkbase() {
    let dts = TaxonomySet::discover(&[entry_point(
        "de-gaap-ci-2025-04-01/de-gaap-ci-2025-04-01-shell-fiscal.xsd",
    )])
    .unwrap();

    assert!(
        !dts.labels().is_empty(),
        "Expected labels to be parsed from label linkbases"
    );

    // bs.ass should have labels in German and English
    let bs_ass_labels = dts
        .labels_for("de-gaap-ci_bs.ass")
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
fn parse_labels_linkbase_multiple_roles() {
    let dts = TaxonomySet::discover(&[entry_point(
        "de-gcd-2025-04-01/de-gcd-2025-04-01-shell.xsd",
    )])
    .unwrap();

    // Find a concept that has both a standard label and documentation
    let concept_labels = dts.labels();

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
fn parse_presentation_linkbase() {
    let dts = TaxonomySet::discover(&[entry_point(
        "de-gaap-ci-2025-04-01/de-gaap-ci-2025-04-01-shell-fiscal.xsd",
    )])
    .unwrap();

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
        bs_arcs
            .iter()
            .any(|a| a.from == "de-gaap-ci_bs.ass" && a.to == "de-gaap-ci_bs.ass.fixAss"),
        "Expected bs.ass -> bs.ass.fixAss presentation arc"
    );
}

#[test]
fn parse_calculation_linkbase() {
    let dts = TaxonomySet::discover(&[entry_point(
        "de-gaap-ci-2025-04-01/de-gaap-ci-2025-04-01-shell-fiscal.xsd",
    )])
    .unwrap();

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
        .find(|a| a.from == "de-gaap-ci_bs.ass" && a.to == "de-gaap-ci_bs.ass.fixAss")
        .expect("Expected bs.ass -> bs.ass.fixAss calculation arc");
    assert_eq!(child_arc.weight, 1.0);
}

#[test]
fn parse_definition_linkbase() {
    let dts = TaxonomySet::discover(&[entry_point(
        "de-gaap-ci-2025-04-01/de-gaap-ci-2025-04-01-shell-fiscal.xsd",
    )])
    .unwrap();

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
fn parse_reference_linkbase() {
    let dts = TaxonomySet::discover(&[entry_point(
        "de-gaap-ci-2025-04-01/de-gaap-ci-2025-04-01-shell-fiscal.xsd",
    )])
    .unwrap();

    assert!(
        !dts.references().is_empty(),
        "Expected references to be parsed"
    );

    // bs.ass should have an HGB reference
    let bs_refs = dts
        .references_for("de-gaap-ci_bs.ass")
        .expect("Expected references for de-gaap-ci_bs.ass");

    let has_hgb = bs_refs
        .iter()
        .any(|r| r.parts.iter().any(|p| p.name == "Name" && p.value == "HGB"));
    assert!(has_hgb, "Expected HGB reference for bs.ass");
}

#[test]
fn linkbase_refs_have_roles() {
    let dts =
        TaxonomySet::discover(&[entry_point("de-gcd-2025-04-01/de-gcd-2025-04-01.xsd")]).unwrap();

    // The GCD main schema has label and reference linkbaseRefs
    let gcd = dts
        .schema_by_namespace("http://www.xbrl.de/taxonomies/de-gcd-2025-04-01")
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
fn find_element_by_id() {
    let dts = TaxonomySet::discover(&[entry_point(
        "de-gaap-ci-2025-04-01/de-gaap-ci-2025-04-01-shell-fiscal.xsd",
    )])
    .unwrap();

    let elem = dts
        .find_element_by_id("de-gaap-ci_bs.ass")
        .expect("Expected to find element by ID");
    assert_eq!(elem.name, "bs.ass");
    assert_eq!(elem.period_type.as_deref(), Some("instant"));
    assert!(!elem.is_abstract);
}

#[test]
fn qualified_name() {
    let dts = TaxonomySet::discover(&[entry_point(
        "de-gaap-ci-2025-04-01/de-gaap-ci-2025-04-01-shell-fiscal.xsd",
    )])
    .unwrap();

    assert_eq!(
        dts.qualified_name("de-gaap-ci_bs.ass").as_deref(),
        Some("de-gaap-ci:bs.ass")
    );
    assert_eq!(
        dts.qualified_name("de-gaap-ci_bs.ass.fixAss").as_deref(),
        Some("de-gaap-ci:bs.ass.fixAss")
    );
    assert_eq!(dts.qualified_name("nonexistent_element"), None);
}
