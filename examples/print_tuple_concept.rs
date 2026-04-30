//! Print tuple-choice information for one tuple concept.
//!
//! This example focuses on the tuple fact local name
//! `genInfo.report.id.reportElement` and prints all choice branches found in
//! its tuple content model.
//!
//! Usage:
//!     cargo run --example print_tuple_concept

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use xbrl_rs::{
    Concept, ConceptView, InstanceDocument, ROLE_LABEL, ROLE_TERSE, TaxonomySet, TupleParticleView,
};

const INSTANCE_PATH: &str = "test_data/instances/balance_sheet_v64.xml";
const TAXONOMY_ENTRY_POINT: &str = "test_data/taxonomies";
const TUPLE_LOCAL_NAME: &str = "genInfo.report.id.reportType";
const TUPLE_CONCEPT_ID: &str = "de-gcd_genInfo.report.id.reportType";
const REPORT_ELEMENTS_PREFIX: &str = "genInfo.report.id.reportType.reportType.";
const LANG: &str = "en";

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}~", &s[..max - 1])
    }
}

fn resolve_label_by_role<'a>(
    taxonomy: &'a TaxonomySet,
    concept: &'a Concept,
    role: &str,
    lang: &str,
) -> Option<&'a str> {
    let labels = taxonomy.labels(&concept.name).unwrap_or(&[]);

    labels
        .iter()
        .find(|label| label.lang == lang && label.role == role)
        .or_else(|| labels.iter().find(|label| label.role == role))
        .map(|label| label.text.as_str())
}

fn collect_report_elements<'a>(particle: &'a TupleParticleView<'a>, out: &mut Vec<&'a Concept>) {
    match particle {
        TupleParticleView::Element { element, .. } => {
            if let Some(concept) = element.concept
                && concept.name.local_name.starts_with(REPORT_ELEMENTS_PREFIX)
            {
                out.push(concept);
            }
        }
        TupleParticleView::Sequence { children, .. }
        | TupleParticleView::Choice { children, .. } => {
            for child in children {
                collect_report_elements(child, out);
            }
        }
        TupleParticleView::GroupRef { .. } => {}
        TupleParticleView::GroupDef { particle, .. } => collect_report_elements(particle, out),
    }
}

fn in_substitution_group(taxonomy: &TaxonomySet, concept: &Concept, head: &Concept) -> bool {
    let mut current = &concept.substitution_group.original;
    let mut seen = HashSet::new();

    while seen.insert(current) {
        if current == &head.name {
            return true;
        }

        let Some(parent) = taxonomy.find_concept(current) else {
            break;
        };
        current = &parent.substitution_group.original;
    }

    false
}

fn expand_report_element_heads<'a>(
    taxonomy: &'a TaxonomySet,
    concepts: Vec<&'a Concept>,
) -> Vec<&'a Concept> {
    let mut expanded = Vec::new();

    for concept in concepts {
        if concept.name.local_name.ends_with(".head") {
            for candidate in taxonomy.elements() {
                if !candidate
                    .name
                    .local_name
                    .starts_with(REPORT_ELEMENTS_PREFIX)
                    || candidate.name.local_name.ends_with(".head")
                {
                    continue;
                }
                if in_substitution_group(taxonomy, candidate, concept) {
                    expanded.push(candidate);
                }
            }
        } else {
            expanded.push(concept);
        }
    }

    expanded
}

fn find_target_tuple_concept<'a>(taxonomy: &'a TaxonomySet) -> Option<&'a Concept> {
    taxonomy
        .find_concept_by_id(TUPLE_CONCEPT_ID)
        .filter(|concept| concept.is_tuple())
        .or_else(|| {
            taxonomy
                .elements()
                .into_iter()
                .find(|concept| concept.is_tuple() && concept.name.local_name == TUPLE_LOCAL_NAME)
        })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse instance to get schema refs for the matching taxonomy version.
    let instance = InstanceDocument::from_file(Path::new(INSTANCE_PATH))?;

    // Discover taxonomy.
    let schema_refs: Vec<String> = instance.schema_refs().to_vec();
    let entry_point = PathBuf::from(TAXONOMY_ENTRY_POINT);
    let taxonomy = TaxonomySet::discover(schema_refs, entry_point)?;

    let concept = find_target_tuple_concept(&taxonomy).ok_or_else(|| {
        format!(
            "Tuple concept '{}' not found. Hint id tried: '{}'.",
            TUPLE_LOCAL_NAME, TUPLE_CONCEPT_ID
        )
    })?;

    let view = ConceptView::build(concept, &taxonomy);

    println!("=== Tuple Concept ===");
    println!("id: {}", view.concept.id.as_deref().unwrap_or("<none>"));
    println!("name: {}", view.concept.name.local_name);
    println!("namespace: {}", view.concept.name.namespace_uri.as_str());
    println!("tuple: {}", view.concept.is_tuple());

    println!("\n=== reportElements ===");
    match &view.tuple_content {
        Some(particle) => {
            let mut concepts = Vec::new();
            collect_report_elements(particle, &mut concepts);
            let concepts = expand_report_element_heads(&taxonomy, concepts);

            let mut seen = HashSet::new();
            let w_concept = 70;
            let w_terse = 45;
            let w_label = 60;

            println!(
                "| {:<w_concept$} | {:<w_terse$} | {:<w_label$} |",
                "CONCEPT", "TERSE LABEL", "FULL LABEL"
            );
            println!(
                "|-{:-<w_concept$}-|-{:-<w_terse$}-|-{:-<w_label$}-|",
                "", "", ""
            );

            for concept in concepts {
                if !seen.insert(concept.name.local_name.as_str()) {
                    continue;
                }
                let concept_col = truncate(concept.name.local_name.as_str(), w_concept);
                let terse_label =
                    resolve_label_by_role(&taxonomy, concept, ROLE_TERSE, LANG).unwrap_or("");
                let full_label =
                    resolve_label_by_role(&taxonomy, concept, ROLE_LABEL, LANG).unwrap_or("");
                let terse_col = truncate(terse_label, w_terse);
                let label_col = truncate(full_label, w_label);
                println!(
                    "| {:<w_concept$} | {:<w_terse$} | {:<w_label$} |",
                    concept_col, terse_col, label_col
                );
            }
        }
        None => {
            println!("No tuple content model available for this concept.");
        }
    }

    Ok(())
}
