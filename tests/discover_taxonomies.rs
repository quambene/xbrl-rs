use std::path::Path;
use xbrl_rs::{Label, TaxonomySet};

const TAXONOMY_BASE: &str = "test_data/taxonomies/german-gaap/v6.9";

#[test]
fn discover_gcd_shell() {
    let entry = format!("{TAXONOMY_BASE}/de-gcd-2025-04-01/de-gcd-2025-04-01-shell.xsd");
    let dts = TaxonomySet::discover(&[Path::new(&entry)]).unwrap();

    // shell -> de-gcd-2025-04-01.xsd -> hgbrole-2025-04-01.xsd
    // (HTTP import for xbrl-instance is skipped)
    assert!(
        dts.schemas().len() >= 3,
        "Expected at least 3 schemas, got {}",
        dts.schemas().len()
    );

    // GCD main has label + reference linkbases, shell has presentation
    assert!(
        !dts.linkbase_paths().is_empty(),
        "Expected linkbase paths to be discovered"
    );

    // GCD defines the role_gcd roleType
    let roles = dts.role_types();
    assert!(
        roles.iter().any(|r| r.id == "role_gcd"),
        "Expected role_gcd to be defined"
    );
}

#[test]
fn discover_gaap_ci_shell_fiscal() {
    let entry =
        format!("{TAXONOMY_BASE}/de-gaap-ci-2025-04-01/de-gaap-ci-2025-04-01-shell-fiscal.xsd");
    let dts = TaxonomySet::discover(&[Path::new(&entry)]).unwrap();

    // shell-fiscal -> shell -> de-gaap-ci.xsd + dimensions.xsd + hgbrole
    assert!(
        dts.schemas().len() >= 4,
        "Expected at least 4 schemas, got {}",
        dts.schemas().len()
    );

    // The main de-gaap-ci schema defines many elements
    let elements = dts.elements();
    assert!(
        elements.len() > 1000,
        "Expected >1000 elements, got {}",
        elements.len()
    );

    // Verify a known element
    let bs_ass = dts
        .find_element("bs.ass")
        .expect("Element bs.ass not found");
    assert_eq!(bs_ass.period_type.as_deref(), Some("instant"));
    assert_eq!(bs_ass.balance.as_deref(), Some("debit"));
    assert!(bs_ass.nillable);
}

#[test]
fn discover_bra_shell_fiscal() {
    let entry = format!("{TAXONOMY_BASE}/de-bra-2025-04-01/de-bra-2025-04-01-shell-fiscal.xsd");
    let dts = TaxonomySet::discover(&[Path::new(&entry)]).unwrap();

    // Cross-module: bra imports de-gaap-ci
    assert!(
        dts.schemas().len() >= 5,
        "Expected at least 5 schemas, got {}",
        dts.schemas().len()
    );

    // Verify a BRA-specific element exists
    let elem = dts
        .find_element("bs.ass.fixAss.tan.stayingWood")
        .expect("BRA element not found");
    assert_eq!(elem.balance.as_deref(), Some("debit"));

    // Verify GAAP-CI elements are also included (from imported schema)
    assert!(
        dts.find_element("bs.ass").is_some(),
        "GAAP-CI element bs.ass should be reachable"
    );
}

#[test]
fn discover_multiple_entry_points() {
    let gcd = format!("{TAXONOMY_BASE}/de-gcd-2025-04-01/de-gcd-2025-04-01-shell.xsd");
    let gaap =
        format!("{TAXONOMY_BASE}/de-gaap-ci-2025-04-01/de-gaap-ci-2025-04-01-shell-fiscal.xsd");
    let dts = TaxonomySet::discover(&[Path::new(&gcd), Path::new(&gaap)]).unwrap();

    // hgbrole is imported by de-gcd but should only appear once
    let hgbrole_count = dts
        .schemas()
        .keys()
        .filter(|p| {
            p.file_name()
                .and_then(|f| f.to_str())
                .is_some_and(|f| f.contains("hgbrole"))
        })
        .count();
    assert_eq!(hgbrole_count, 1, "hgbrole should appear exactly once");
}

#[test]
fn role_types_discovered() {
    let entry = format!("{TAXONOMY_BASE}/de-gaap-ci-2025-04-01/de-gaap-ci-2025-04-01-shell.xsd");
    let dts = TaxonomySet::discover(&[Path::new(&entry)]).unwrap();

    let roles = dts.role_types();
    assert!(
        roles.len() > 10,
        "Expected >10 roleTypes, got {}",
        roles.len()
    );

    // Check for a known role
    let balance_sheet = roles
        .iter()
        .find(|r| r.role_uri.contains("balanceSheet") && !r.role_uri.contains("Table"))
        .expect("balanceSheet role not found");
    assert_eq!(balance_sheet.id, "role_balanceSheet");
    assert!(balance_sheet.definition.is_some());
    assert!(!balance_sheet.used_on.is_empty());
}

#[test]
fn schema_by_namespace() {
    let entry = format!("{TAXONOMY_BASE}/de-gcd-2025-04-01/de-gcd-2025-04-01-shell.xsd");
    let dts = TaxonomySet::discover(&[Path::new(&entry)]).unwrap();

    let gcd = dts
        .schema_by_namespace("http://www.xbrl.de/taxonomies/de-gcd-2025-04-01")
        .expect("GCD schema not found by namespace");
    assert!(!gcd.elements.is_empty());
    assert!(gcd.elements.iter().any(|e| e.name == "genInfo"));
}

#[test]
fn parse_labels_for_gaap_ci() {
    let entry =
        format!("{TAXONOMY_BASE}/de-gaap-ci-2025-04-01/de-gaap-ci-2025-04-01-shell-fiscal.xsd");
    let dts = TaxonomySet::discover(&[Path::new(&entry)]).unwrap();

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
fn parse_labels_include_multiple_roles() {
    let entry = format!("{TAXONOMY_BASE}/de-gcd-2025-04-01/de-gcd-2025-04-01-shell.xsd");
    let dts = TaxonomySet::discover(&[Path::new(&entry)]).unwrap();

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
fn linkbase_refs_have_roles() {
    let entry = format!("{TAXONOMY_BASE}/de-gcd-2025-04-01/de-gcd-2025-04-01.xsd");
    let dts = TaxonomySet::discover(&[Path::new(&entry)]).unwrap();

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
