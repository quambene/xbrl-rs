//! Integration tests for parsing the full DTS (Discoverable Taxonomy Set)
//! across all available taxonomy versions.

use std::{path::PathBuf, str::FromStr};
use xbrl_rs::{Balance, ExpandedName, NamespaceUri, PeriodType, TaxonomySet};

const TAXONOMY_ENTRY_POINT: &str = "test_data/taxonomies";

fn assert_dts(dts: &TaxonomySet) {
    // GCD elements
    assert!(
        dts.find_concept(&ExpandedName {
            namespace_uri: NamespaceUri::from("http://www.xbrl.de/taxonomies/de-gcd-2020-04-01"),
            local_name: "genInfo".to_string()
        })
        .is_some(),
        "Expected genInfo from de-gcd"
    );

    // GAAP-CI elements
    let bs_ass = dts
        .find_concept(&ExpandedName {
            namespace_uri: NamespaceUri::from(
                "http://www.xbrl.de/taxonomies/de-gaap-ci-2020-04-01",
            ),
            local_name: "bs.ass".to_string(),
        })
        .expect("bs.ass not found");
    assert_eq!(bs_ass.period_type, Some(PeriodType::Instant));
    assert_eq!(bs_ass.balance, Some(Balance::Debit));
    assert!(bs_ass.nillable);

    assert!(
        dts.elements().len() > 1000,
        "Expected >1000 elements, got {}",
        dts.elements().len()
    );

    // Linkbases
    assert!(!dts.labels_map().is_empty(), "Expected labels");
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

// -- Version mismatch --

#[test]
#[cfg_attr(not(feature = "taxonomy-test"), ignore)]
fn discover_version_mismatch_returns_error() {
    let schema_refs = vec![
        "http://www.xbrl.de/taxonomies/de-gcd-2020-04-01/de-gcd-2020-04-01-shell.xsd".to_owned(),
        "http://www.xbrl.de/taxonomies/de-gaap-ci-2021-04-14/de-gaap-ci-2021-04-14-shell-fiscal.xsd"
            .to_owned(),
    ];
    let entry_point = PathBuf::from_str(TAXONOMY_ENTRY_POINT).unwrap();
    let result = TaxonomySet::discover(schema_refs, entry_point);
    assert!(result.is_err());
}

// -- 2020-04-01 (v6.4) --

#[test]
#[cfg_attr(not(feature = "taxonomy-test"), ignore)]
fn discover_full_dts_2020() {
    let schema_refs = [
        "http://www.xbrl.de/taxonomies/de-gcd-2020-04-01/de-gcd-2020-04-01-shell.xsd",
        "http://www.xbrl.de/taxonomies/de-gaap-ci-2020-04-01/de-gaap-ci-2020-04-01-shell-fiscal.xsd",
        "http://www.xbrl.de/taxonomies/de-bra-2020-04-01/de-bra-2020-04-01-shell-fiscal.xsd",
        "http://www.xbrl.de/taxonomies/de-fi-2020-04-01/de-fi-2020-04-01-shell-staffelform-fiscal.xsd",
        "http://www.xbrl.de/taxonomies/de-ins-2020-04-01/de-ins-2020-04-01-shell-fiscal.xsd",
        "http://www.xbrl.de/taxonomies/de-pi-2020-04-01/de-pi-2020-04-01-shell-staffelform-fiscal.xsd",
    ]
    .into_iter()
    .map(|href| href.to_owned())
    .collect();
    let entry_point = PathBuf::from_str(TAXONOMY_ENTRY_POINT).unwrap();
    let dts = TaxonomySet::discover(schema_refs, entry_point).unwrap();

    assert_dts(&dts);
}

// -- 2021-04-14 (v6.5) --

#[test]
#[cfg_attr(not(feature = "taxonomy-test"), ignore)]
fn discover_full_dts_2021() {
    let schema_refs = [
        "http://www.xbrl.de/taxonomies/de-gcd-2021-04-14/de-gcd-2021-04-14-shell.xsd",
        "http://www.xbrl.de/taxonomies/de-gaap-ci-2021-04-14/de-gaap-ci-2021-04-14-shell-fiscal.xsd",
        "http://www.xbrl.de/taxonomies/de-bra-2021-04-14/de-bra-2021-04-14-shell-fiscal.xsd",
        "http://www.xbrl.de/taxonomies/de-fi-2021-04-14/de-fi-2021-04-14-shell-staffelform-fiscal.xsd",
        "http://www.xbrl.de/taxonomies/de-ins-2021-04-14/de-ins-2021-04-14-shell-fiscal.xsd",
        "http://www.xbrl.de/taxonomies/de-pi-2021-04-14/de-pi-2021-04-14-shell-staffelform-fiscal.xsd",
    ]
    .into_iter()
    .map(|href| href.to_owned())
    .collect();
    let entry_point = PathBuf::from_str(TAXONOMY_ENTRY_POINT).unwrap();
    let dts = TaxonomySet::discover(schema_refs, entry_point).unwrap();

    assert_dts(&dts);
}
