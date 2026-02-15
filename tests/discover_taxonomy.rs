//! Integration tests for parsing the full DTS (Discoverable Taxonomy Set)
//! across all available taxonomy versions.

use std::path::Path;
use xbrl_rs::{EntryPoint, TaxonomySet};

const TAXONOMY_BASE: &str = "test_data/taxonomies";
const TAXONOMY_URL_BASE: &str = "http://www.xbrl.de/taxonomies";

fn discover(entry_points: &[&str]) -> TaxonomySet {
    let entries: Vec<_> = entry_points
        .iter()
        .map(|path| {
            EntryPoint::new(
                format!("{TAXONOMY_URL_BASE}/{path}"),
                Path::new(TAXONOMY_BASE).join(path),
            )
        })
        .collect();
    TaxonomySet::discover(&entries).expect("failed to discover taxonomy")
}

fn assert_dts(dts: &TaxonomySet) {
    // GCD elements
    assert!(
        dts.find_element("genInfo").is_some(),
        "Expected genInfo from de-gcd"
    );

    // GAAP-CI elements
    let bs_ass = dts.find_element("bs.ass").expect("bs.ass not found");
    assert_eq!(bs_ass.period_type.as_deref(), Some("instant"));
    assert_eq!(bs_ass.balance.as_deref(), Some("debit"));
    assert!(bs_ass.nillable);

    assert!(
        dts.elements().len() > 1000,
        "Expected >1000 elements, got {}",
        dts.elements().len()
    );

    // Linkbases
    assert!(!dts.labels().is_empty(), "Expected labels");
    assert!(!dts.presentations().is_empty(), "Expected presentations");
    assert!(!dts.calculations().is_empty(), "Expected calculations");
    assert!(!dts.definitions().is_empty(), "Expected definitions");
    assert!(!dts.references().is_empty(), "Expected references");

    // Role types
    assert!(
        dts.role_types().len() > 10,
        "Expected >10 role types, got {}",
        dts.role_types().len()
    );
}

// -- 2020-04-01 (v6.4) --

#[test]
fn discover_full_dts_2020() {
    let dts = discover(&[
        "de-gcd-2020-04-01/de-gcd-2020-04-01-shell.xsd",
        "de-gaap-ci-2020-04-01/de-gaap-ci-2020-04-01-shell-fiscal.xsd",
        "de-bra-2020-04-01/de-bra-2020-04-01-shell-fiscal.xsd",
        "de-fi-2020-04-01/de-fi-2020-04-01-shell-staffelform-fiscal.xsd",
        "de-ins-2020-04-01/de-ins-2020-04-01-shell-fiscal.xsd",
        "de-pi-2020-04-01/de-pi-2020-04-01-shell-staffelform-fiscal.xsd",
    ]);

    assert_dts(&dts);
}

// -- 2021-04-14 (v6.5) --

#[test]
fn discover_full_dts_2021() {
    let dts = discover(&[
        "de-gcd-2021-04-14/de-gcd-2021-04-14-shell.xsd",
        "de-gaap-ci-2021-04-14/de-gaap-ci-2021-04-14-shell-fiscal.xsd",
        "de-bra-2021-04-14/de-bra-2021-04-14-shell-fiscal.xsd",
        "de-fi-2021-04-14/de-fi-2021-04-14-shell-staffelform-fiscal.xsd",
        "de-ins-2021-04-14/de-ins-2021-04-14-shell-fiscal.xsd",
        "de-pi-2021-04-14/de-pi-2021-04-14-shell-staffelform-fiscal.xsd",
    ]);

    assert_dts(&dts);
}

// -- 2022-05-02 (v6.7) --

#[test]
fn discover_full_dts_2022() {
    let dts = discover(&[
        "de-gcd-2022-05-02/de-gcd-2022-05-02-shell.xsd",
        "de-gaap-ci-2022-05-02/de-gaap-ci-2022-05-02-shell-fiscal.xsd",
        "de-bra-2022-05-02/de-bra-2022-05-02-shell-fiscal.xsd",
        "de-fi-2022-05-02/de-fi-2022-05-02-shell-staffelform-fiscal.xsd",
        "de-ins-2022-05-02/de-ins-2022-05-02-shell-fiscal.xsd",
        "de-pi-2022-05-02/de-pi-2022-05-02-shell-staffelform-fiscal.xsd",
    ]);

    assert_dts(&dts);
}
