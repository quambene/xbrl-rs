//! Print taxonomy presentation sections as a labelled concept tree.
//!
//! Concepts are grouped by presentation section (extended link role) and shown
//! in their presentation hierarchy with labels and indentation.
//!
//! Usage:
//!     cargo run --example print_taxonomy

use std::path::{Path, PathBuf};
use xbrl_rs::{
    InstanceDocument, Label, ROLE_LABEL, ROLE_TERSE, TaxonomySet, TaxonomyTreeNode, TaxonomyView,
};

const INSTANCE_PATH: &str = "test_data/instances/balance_sheet_v64.xml";
const TAXONOMY_ENTRY_POINT: &str = "test_data/taxonomies";
const LANG: &str = "en";

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}~", &s[..max - 1])
    }
}

/// Resolve terse label first, then standard label, then fall back to local name.
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

fn print_node(node: &TaxonomyTreeNode, lang: &str, w_label: usize, w_concept: usize) {
    let indent = "  ".repeat(node.depth);
    let local_name = node.concept.name.local_name.as_str();
    let label = resolve_label(node.labels, local_name, lang);
    let label_col = truncate(&format!("{indent}{label}"), w_label);
    let concept_col = truncate(local_name, w_concept);

    println!(
        "| {:<w_concept$} | {:>5} | {:<w_label$} |",
        concept_col, node.depth, label_col,
    );

    for child in &node.children {
        print_node(child, lang, w_label, w_concept);
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse instance to get schema refs for the matching taxonomy version.
    let instance = InstanceDocument::from_file(Path::new(INSTANCE_PATH))?;

    // Discover taxonomy.
    let schema_refs: Vec<String> = instance.schema_refs().to_vec();
    let entry_point = PathBuf::from(TAXONOMY_ENTRY_POINT);
    let taxonomy = TaxonomySet::discover(schema_refs, entry_point)?;

    // Build taxonomy view.
    let view = TaxonomyView::build(&taxonomy);

    let w_concept = 80;
    let w_level = 5;
    let w_label = 80;

    for section in &view.sections {
        let role_label = section.role.rsplit('/').next().unwrap_or(section.role);
        println!("\n=== {role_label} ===");
        println!(
            "| {:<w_concept$} | {:>w_level$} | {:<w_label$} |",
            "CONCEPT", "LEVEL", "LABEL"
        );
        println!(
            "|-{:-<w_concept$}-|-{:-<w_level$}-|-{:-<w_label$}-|",
            "", "", ""
        );

        for node in &section.nodes {
            print_node(node, LANG, w_label, w_concept);
        }
    }

    println!("\n{} presentation sections", view.sections.len());

    Ok(())
}
