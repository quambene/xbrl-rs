//! Print a single concept view with labels, relationships, and tuple content.
//!
//! Usage:
//!     cargo run --example print_concept

use std::path::{Path, PathBuf};
use xbrl_rs::{
    ConceptView, InstanceDocument, Label, ROLE_LABEL, ROLE_TERSE, TaxonomySet, TupleParticleView,
};

const INSTANCE_PATH: &str = "test_data/instances/balance_sheet_v64.xml";
const TAXONOMY_ENTRY_POINT: &str = "test_data/taxonomies";
const CONCEPT_ID: &str = "de-gaap-ci_bs.ass";
const LANG: &str = "en";

/// Resolve terse label first, then standard label, then fallback.
fn resolve_label<'a>(labels: &'a [Label], fallback: &'a str, lang: &str) -> &'a str {
    if let Some(label) = labels
        .iter()
        .find(|label| label.lang == lang && label.role == ROLE_TERSE)
    {
        return label.text.as_str();
    }
    if let Some(label) = labels
        .iter()
        .find(|label| label.lang == lang && label.role == ROLE_LABEL)
    {
        return label.text.as_str();
    }
    fallback
}

fn format_occurs(min: u32, max: Option<u32>) -> String {
    match max {
        Some(max) => format!("{min}..{max}"),
        None => format!("{min}..unbounded"),
    }
}

fn print_tuple_particle(particle: &TupleParticleView, depth: usize) {
    let indent = "  ".repeat(depth);
    match particle {
        TupleParticleView::Element { element, occurs } => {
            let concept_name = element
                .concept
                .map(|concept| concept.name.local_name.as_str())
                .unwrap_or(element.local_name);
            println!(
                "{indent}- element: {} [{}]",
                concept_name,
                format_occurs(occurs.min, occurs.max)
            );
        }
        TupleParticleView::Sequence { children, occurs } => {
            println!(
                "{indent}- sequence [{}]",
                format_occurs(occurs.min, occurs.max)
            );
            for child in children {
                print_tuple_particle(child, depth + 1);
            }
        }
        TupleParticleView::Choice { children, occurs } => {
            println!(
                "{indent}- choice [{}]",
                format_occurs(occurs.min, occurs.max)
            );
            for child in children {
                print_tuple_particle(child, depth + 1);
            }
        }
        TupleParticleView::GroupRef { name, occurs } => {
            println!(
                "{indent}- group-ref: {} [{}]",
                name,
                format_occurs(occurs.min, occurs.max)
            );
        }
        TupleParticleView::GroupDef {
            name,
            particle,
            occurs,
        } => {
            let group_name = name.unwrap_or("<anonymous>");
            println!(
                "{indent}- group-def: {} [{}]",
                group_name,
                format_occurs(occurs.min, occurs.max)
            );
            print_tuple_particle(particle, depth + 1);
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse instance to get schema refs for the matching taxonomy version.
    let instance = InstanceDocument::from_file(Path::new(INSTANCE_PATH))?;

    // Discover taxonomy.
    let schema_refs: Vec<String> = instance.schema_refs().to_vec();
    let entry_point = PathBuf::from(TAXONOMY_ENTRY_POINT);
    let taxonomy = TaxonomySet::discover(schema_refs, entry_point)?;

    // Build concept view.
    let view = ConceptView::build_from_id(CONCEPT_ID, &taxonomy)
        .ok_or_else(|| format!("Concept id '{}' not found in taxonomy", CONCEPT_ID))?;

    let local_name = view.concept.name.local_name.as_str();
    let label = resolve_label(view.labels, local_name, LANG);

    println!("=== ConceptView ===");
    println!("id: {}", view.concept.id.as_deref().unwrap_or("<none>"));
    println!("name: {}", local_name);
    println!("namespace: {}", view.concept.name.namespace_uri.as_str());
    println!("label({}): {}", LANG, label);
    println!("tuple: {}", view.concept.is_tuple());
    println!("abstract: {}", view.concept.is_abstract);
    println!("nillable: {}", view.concept.nillable);
    println!("period_type: {:?}", view.concept.period_type);
    println!("balance: {:?}", view.concept.balance);
    println!("references: {}", view.references.len());
    println!("presentation parents: {}", view.presentation_parents.len());
    println!(
        "presentation children: {}",
        view.presentation_children.len()
    );

    match view.parent_tuple {
        Some(parent) => println!("direct parent tuple: {}", parent.name.local_name),
        None => println!("direct parent tuple: <none>"),
    }

    if view.tuple_ancestors.is_empty() {
        println!("tuple ancestors: <none>");
    } else {
        let chain = view
            .tuple_ancestors
            .iter()
            .map(|concept| concept.name.local_name.as_str())
            .collect::<Vec<_>>()
            .join(" -> ");
        println!("tuple ancestors: {}", chain);
    }

    println!("\n-- Presentation Parents --");
    if view.presentation_parents.is_empty() {
        println!("<none>");
    } else {
        for relation in &view.presentation_parents {
            let related = relation
                .concept
                .map(|concept| concept.name.local_name.as_str())
                .unwrap_or(relation.concept_name.local_name.as_str());
            println!(
                "role={} related={} order={:?}",
                relation.role, related, relation.order
            );
        }
    }

    println!("\n-- Presentation Children --");
    if view.presentation_children.is_empty() {
        println!("<none>");
    } else {
        for relation in &view.presentation_children {
            let related = relation
                .concept
                .map(|concept| concept.name.local_name.as_str())
                .unwrap_or(relation.concept_name.local_name.as_str());
            println!(
                "role={} related={} order={:?}",
                relation.role, related, relation.order
            );
        }
    }

    println!("\n-- Tuple Content Model --");
    match &view.tuple_content {
        Some(particle) => print_tuple_particle(particle, 0),
        None => println!("<none>"),
    }

    Ok(())
}
