//! Integration tests for taxonomy-centric views.

use std::{path::PathBuf, str::FromStr};
use xbrl_rs::{ConceptView, TaxonomySet, TaxonomyView};

const TAXONOMY_ENTRY_POINT: &str = "test_data/taxonomies";

#[test]
fn taxonomy_view_empty_taxonomy() {
    let taxonomy = TaxonomySet::default();
    let view = TaxonomyView::build(&taxonomy);

    assert!(view.sections.is_empty());
}

#[test]
#[cfg_attr(not(feature = "taxonomy-test"), ignore)]
fn taxonomy_and_concept_views_on_discovered_dts() {
    let schema_refs = [
        "http://www.xbrl.de/taxonomies/de-gcd-2020-04-01/de-gcd-2020-04-01-shell.xsd",
        "http://www.xbrl.de/taxonomies/de-gaap-ci-2020-04-01/de-gaap-ci-2020-04-01-shell-fiscal.xsd",
    ]
    .into_iter()
    .map(|href| href.to_owned())
    .collect();

    let entry_point = PathBuf::from_str(TAXONOMY_ENTRY_POINT).unwrap();
    let dts = TaxonomySet::discover(schema_refs, entry_point).unwrap();

    let taxonomy_view = TaxonomyView::build(&dts);
    assert!(!taxonomy_view.sections.is_empty());
    assert_eq!(taxonomy_view.sections.len(), dts.presentations().len());
    assert!(taxonomy_view.sections.iter().any(|section| !section.nodes.is_empty()));

    let concept = dts
        .elements()
        .into_iter()
        .find(|concept| concept.name.local_name == "bs.ass")
        .unwrap();
    let concept_view = ConceptView::build(concept, &dts);

    assert_eq!(concept_view.concept.name.local_name, "bs.ass");
    assert!(
        !concept_view.presentation_parents.is_empty() || !concept_view.presentation_children.is_empty()
    );
}
