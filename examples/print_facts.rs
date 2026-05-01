//! Print all facts of an XBRL instance as a labelled document tree.
//!
//! Facts are grouped by presentation section (extended link role) and shown
//! in their presentation hierarchy with labels and indentation.
//!
//! Usage:
//!     cargo run --example print_facts

use std::path::{Path, PathBuf};
use xbrl_rs::{InstanceDocument, ItemFact, ROLE_LABEL, ROLE_TERSE, TaxonomySet, TreeNode};

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

/// Resolve terse label first, then standard label, then fall back to concept ID.
fn resolve_label<'a>(node: &'a TreeNode<'a>, lang: &str) -> &'a str {
    if let Some(label) = node
        .labels
        .iter()
        .find(|label| label.lang == lang && label.role == ROLE_TERSE)
    {
        return label.text.as_str();
    }
    if let Some(label) = node
        .labels
        .iter()
        .find(|label| label.lang == lang && label.role == ROLE_LABEL)
    {
        return label.text.as_str();
    }
    node.concept_name
}

fn print_node(
    node: &TreeNode,
    facts: &[&ItemFact],
    lang: &str,
    w_label: usize,
    w_concept: usize,
    w_value: usize,
) {
    let indent = "  ".repeat(node.depth);
    let label = resolve_label(node, lang);
    let label_col = truncate(&format!("{indent}{label}"), w_label);
    let concept_col = truncate(node.concept_name, w_concept);
    let level = node.depth;

    if node.fact_indices.is_empty() {
        println!(
            "| {:<w_concept$} | {:>5} | {:<w_label$} | {:>w_value$} | {:<6} | {:<16} |",
            concept_col, level, label_col, "", "", "",
        );
    } else {
        for &idx in &node.fact_indices {
            let fact = &facts[idx];

            if fact.is_nil() {
                continue;
            }

            println!(
                "| {:<w_concept$} | {:>5} | {:<w_label$} | {:>w_value$} | {:<6} | {:<16} |",
                concept_col,
                level,
                label_col,
                truncate(fact.value(), w_value),
                fact.unit_ref().unwrap_or(""),
                truncate(fact.context_ref(), 16),
            );
        }
    }

    for child in &node.children {
        print_node(child, facts, lang, w_label, w_concept, w_value);
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse instance
    let instance = InstanceDocument::from_file(Path::new(INSTANCE_PATH))?;

    // Discover taxonomy
    let schema_refs: Vec<String> = instance.schema_refs().to_vec();
    let entry_point = PathBuf::from(TAXONOMY_ENTRY_POINT);
    let taxonomy = TaxonomySet::discover(schema_refs, entry_point)?;

    // Build document view
    let view = instance.view(&taxonomy);
    let item_facts = instance.item_facts();

    let w_concept = 40;
    let w_level = 5;
    let w_label = 80;
    let w_value = 20;
    let w_unit = 6;
    let w_context = 16;

    for section in &view.sections {
        let role_label = section.role.rsplit('/').next().unwrap_or(section.role);
        println!("\n=== {role_label} ===");
        println!(
            "| {:<w_concept$} | {:>w_level$} | {:<w_label$} | {:>w_value$} | {:<w_unit$} | {:<w_context$} |",
            "CONCEPT", "LEVEL", "LABEL", "VALUE", "UNIT", "CONTEXT"
        );
        println!(
            "|-{:-<w_concept$}-|-{:-<w_level$}-|-{:-<w_label$}-|-{:-<w_value$}-|-{:-<w_unit$}-|-{:-<w_context$}-|",
            "", "", "", "", "", ""
        );

        for node in &section.nodes {
            print_node(node, &item_facts, LANG, w_label, w_concept, w_value);
        }
    }

    let total_facts = item_facts.len();
    let nil_facts = item_facts.iter().filter(|f| f.is_nil()).count();
    println!("\n{} facts total ({} nil)", total_facts, nil_facts);

    Ok(())
}
