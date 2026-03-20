use std::{fs, path::Path, path::PathBuf};
use xbrl_rs::{DocumentView, InstanceDocument, ItemFact, TaxonomySet, TreeNode};

const TAXONOMY_ENTRY_POINT: &str = "test_data/taxonomies";
const LANG: &str = "en";
const ROLE_TERSE: &str = "http://www.xbrl.org/2003/role/terseLabel";
const ROLE_LABEL: &str = "http://www.xbrl.org/2003/role/label";

/// Render a deterministic CSV snapshot of the instance view.
pub fn render_view(view: &DocumentView, item_facts: &[&ItemFact], lang: &str) -> String {
    let mut out = String::new();
    out.push_str("role,concept,level,label,value,unit,context\n");

    for section in &view.sections {
        for node in &section.nodes {
            write_node_rows(&mut out, section.role, node, &item_facts, lang);
        }
    }

    out
}

fn resolve_label<'a>(node: &'a TreeNode<'a>, lang: &str) -> Option<&'a str> {
    if let Some(label) = node
        .labels
        .iter()
        .find(|label| label.lang == lang && label.role == ROLE_TERSE)
    {
        return Some(label.text.as_str());
    }

    if let Some(label) = node
        .labels
        .iter()
        .find(|label| label.lang == lang && label.role == ROLE_LABEL)
    {
        return Some(label.text.as_str());
    }

    None
}

fn write_node_rows(
    out: &mut String,
    section_role: &str,
    node: &TreeNode,
    facts: &[&ItemFact],
    lang: &str,
) {
    let label = resolve_label(node, lang);
    let indented_label = format!("{}{}", "  ".repeat(node.depth), label.unwrap_or_default());
    let level = node.depth.to_string();

    if node.fact_indices.is_empty() {
        write_csv_row(
            out,
            &[
                section_role,
                node.concept_name,
                level.as_str(),
                indented_label.as_str(),
                "",
                "",
                "",
            ],
        );
    } else {
        for &idx in &node.fact_indices {
            let fact = facts[idx];
            if fact.is_nil() {
                continue;
            }

            write_csv_row(
                out,
                &[
                    section_role,
                    node.concept_name,
                    level.as_str(),
                    indented_label.as_str(),
                    fact.value(),
                    fact.unit_ref().unwrap_or(""),
                    fact.context_ref(),
                ],
            );
        }
    }

    for child in &node.children {
        write_node_rows(out, section_role, child, facts, lang);
    }
}

fn write_csv_row(out: &mut String, fields: &[&str]) {
    for (idx, field) in fields.iter().enumerate() {
        if idx > 0 {
            out.push(',');
        }
        write_csv_field(out, field);
    }
    out.push('\n');
}

fn write_csv_field(out: &mut String, field: &str) {
    out.push('"');
    for ch in field.chars() {
        if ch == '"' {
            out.push('"');
            out.push('"');
        } else {
            out.push(ch);
        }
    }
    out.push('"');
}

fn normalize_newlines(value: &str) -> String {
    value.replace("\r\n", "\n")
}

#[test]
#[cfg_attr(not(feature = "taxonomy-test"), ignore)]
fn view_instance_v64() {
    let instance_path = Path::new("test_data/instances/balance_sheet_v64.xml");
    let fixture_path = Path::new("test_data/views/balance_sheet_v64.csv");
    let taxonomy_path = PathBuf::from(TAXONOMY_ENTRY_POINT);

    let instance = InstanceDocument::from_file(instance_path).unwrap();
    let schema_refs = instance.schema_refs().to_vec();
    let taxonomy = TaxonomySet::discover(schema_refs, taxonomy_path).unwrap();
    let view = instance.view(&taxonomy);
    let item_facts = instance.item_facts();

    let actual = normalize_newlines(&render_view(&view, &item_facts, LANG));
    let expected = normalize_newlines(&fs::read_to_string(fixture_path).unwrap());

    assert_eq!(expected, actual);
}

#[test]
#[cfg_attr(not(feature = "taxonomy-test"), ignore)]
fn view_instance_v65() {
    let instance_path = Path::new("test_data/instances/balance_sheet_v65.xml");
    let fixture_path = Path::new("test_data/views/balance_sheet_v65.csv");
    let taxonomy_path = PathBuf::from(TAXONOMY_ENTRY_POINT);

    let instance = InstanceDocument::from_file(instance_path).unwrap();
    let schema_refs = instance.schema_refs().to_vec();
    let taxonomy = TaxonomySet::discover(schema_refs, taxonomy_path).unwrap();
    let view = instance.view(&taxonomy);
    let item_facts = instance.item_facts();

    let actual = normalize_newlines(&render_view(&view, &item_facts, LANG));
    let expected = normalize_newlines(&fs::read_to_string(fixture_path).unwrap());

    assert_eq!(expected, actual);
}
