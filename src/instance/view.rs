//! Document view built from the presentation linkbase.

use super::fact::ItemFact;
use crate::{PresentationNetwork, TaxonomySet, taxonomy::Label};
use std::collections::HashMap;

/// A hierarchical view of an XBRL document organised by presentation sections.
#[derive(Debug)]
pub struct DocumentView<'a> {
    /// One section per extended link role found in the presentation linkbase.
    pub sections: Vec<SectionView<'a>>,
}

impl<'a> DocumentView<'a> {
    /// Build a document view from a flat facts slice and a taxonomy.
    ///
    /// `facts` is read once to map concept IDs to their positions; no
    /// references into the slice are retained. The returned view borrows
    /// only from `taxonomy`.
    pub fn build(facts: &[&ItemFact], taxonomy: &'a TaxonomySet) -> Self {
        build_view(facts, taxonomy)
    }
}

/// One report section (extended link role) from the presentation linkbase.
#[derive(Debug)]
pub struct SectionView<'a> {
    /// The extended link role URI identifying this section.
    pub role: &'a str,
    /// Root nodes of the presentation tree for this section.
    pub nodes: Vec<TreeNode<'a>>,
}

/// A single node in the presentation hierarchy.
#[derive(Debug)]
pub struct TreeNode<'a> {
    /// Concept element ID (e.g. `de-gaap-ci_bs.ass`).
    pub concept_id: &'a str,
    /// All labels for this concept. The caller selects the desired language
    /// and role (e.g. `terseLabel`, `label`).
    pub labels: &'a [Label],
    /// Depth in the tree; root nodes have depth 0.
    pub depth: usize,
    /// Indices into the `InstanceDocument::facts()` slice for facts whose concept
    /// maps to this element ID.
    ///
    /// Storing indices rather than references means the view's lifetime is tied
    /// only to the taxonomy, leaving the instance free to be mutably borrowed
    /// while the view is alive.
    pub fact_indices: Vec<usize>,
    /// Child nodes, ordered by the presentation arc `order` attribute.
    pub children: Vec<TreeNode<'a>>,
}

/// Build a [`DocumentView`] by walking the presentation linkbase and
/// attaching instance facts and taxonomy labels to each node.
///
/// `facts` is borrowed for index-building only; no references into the slice
/// are retained, so the returned `DocumentView<'a>` borrows only from
/// `taxonomy`.
pub fn build_view<'a>(facts: &[&ItemFact], taxonomy: &'a TaxonomySet) -> DocumentView<'a> {
    // Index facts by their element ID → position in the facts slice.
    let mut fact_index: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, fact) in facts.iter().enumerate() {
        fact_index.entry(fact.concept_id()).or_default().push(i);
    }

    let mut sections = Vec::with_capacity(taxonomy.presentations().len());

    for (role, network) in taxonomy.presentations() {
        let nodes = network
            .roots()
            .iter()
            .flat_map(|root| build_nodes(network, root.as_str(), 0, taxonomy, &fact_index))
            .collect();
        sections.push(SectionView {
            role: role.as_str(),
            nodes,
        });
    }

    DocumentView { sections }
}

/// Recursively build tree nodes for all children of `parent_id`.
fn build_nodes<'a>(
    network: &'a PresentationNetwork,
    parent_id: &'a str,
    depth: usize,
    taxonomy: &'a TaxonomySet,
    fact_index: &HashMap<String, Vec<usize>>,
) -> Vec<TreeNode<'a>> {
    // Children are already sorted by `order` in the network.
    let children = network.children_of(parent_id);

    let mut nodes = Vec::with_capacity(children.len());

    for child_id in children {
        let child_str: &'a str = child_id.as_str();
        let labels = taxonomy.labels_for(child_str).unwrap_or(&[]);
        let fact_indices = fact_index.get(child_str).cloned().unwrap_or_default();
        let grandchildren = build_nodes(network, child_str, depth + 1, taxonomy, fact_index);

        nodes.push(TreeNode {
            concept_id: child_str,
            labels,
            depth,
            fact_indices,
            children: grandchildren,
        });
    }

    nodes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ItemFact,
        taxonomy::{Label, PresentationArc, TaxonomySet},
    };
    use rust_decimal::Decimal;

    fn create_taxonomy(
        arcs: Vec<(String, PresentationArc)>,
        labels: Vec<(String, Label)>,
    ) -> TaxonomySet {
        let mut taxonomy = TaxonomySet::default();
        // Group arcs by role, preserving insertion order.
        let mut role_order: Vec<String> = Vec::new();
        let mut by_role: HashMap<String, Vec<PresentationArc>> = HashMap::new();

        for (role, arc) in arcs {
            if !by_role.contains_key(&role) {
                role_order.push(role.clone());
            }
            by_role.entry(role).or_default().push(arc);
        }

        for role in role_order {
            taxonomy.add_presentation_arcs(role.clone(), by_role.remove(&role).unwrap());
        }

        for (concept_id, label) in labels {
            taxonomy.add_label(concept_id, label);
        }

        taxonomy
    }

    #[test]
    fn build_view_empty_taxonomy() {
        let taxonomy = TaxonomySet::default();
        let view = build_view(&[], &taxonomy);
        assert!(view.sections.is_empty());
    }

    #[test]
    fn build_view_single_section_with_hierarchy() {
        let role = "http://example.com/role/bs".to_string();
        let arcs = vec![
            (
                role.clone(),
                PresentationArc {
                    from: "root".to_string(),
                    to: "child_a".to_string(),
                    order: Some(Decimal::new(1, 0)),
                },
            ),
            (
                role.clone(),
                PresentationArc {
                    from: "root".to_string(),
                    to: "child_b".to_string(),
                    order: Some(Decimal::new(2, 0)),
                },
            ),
            (
                role.clone(),
                PresentationArc {
                    from: "child_a".to_string(),
                    to: "grandchild".to_string(),
                    order: Some(Decimal::new(1, 0)),
                },
            ),
        ];
        let labels = vec![(
            "child_a".to_string(),
            Label {
                role: "http://www.xbrl.org/2003/role/label".to_string(),
                lang: "en".to_string(),
                text: "Child A".to_string(),
            },
        )];
        let taxonomy = create_taxonomy(arcs, labels);

        // Use a QName without a prefix so concept_id() == "child_a" directly,
        // matching the element ID used in the presentation arcs above.
        let fact = ItemFact::new(
            "child_a".to_string(), // no prefix → concept_id() == "child_a"
            "ctx1".to_string(),
            None,
            "42".to_string(),
        );
        let facts = vec![&fact];

        let view = build_view(&facts, &taxonomy);

        assert_eq!(view.sections.len(), 1);
        let section = &view.sections[0];
        assert_eq!(section.role, role);

        // "root" is the root; its children are child_a and child_b
        assert_eq!(section.nodes.len(), 2);

        let node_a = &section.nodes[0];
        assert_eq!(node_a.concept_id, "child_a");
        assert_eq!(node_a.labels.len(), 1);
        assert_eq!(node_a.labels[0].text, "Child A");
        assert_eq!(node_a.labels[0].lang, "en");
        assert_eq!(node_a.depth, 0);
        assert_eq!(node_a.fact_indices.len(), 1);
        assert_eq!(facts[node_a.fact_indices[0]].value(), "42");
        assert_eq!(node_a.children.len(), 1);

        let grandchild = &node_a.children[0];
        assert_eq!(grandchild.concept_id, "grandchild");
        assert_eq!(grandchild.depth, 1);
        assert!(grandchild.labels.is_empty());

        let node_b = &section.nodes[1];
        assert_eq!(node_b.concept_id, "child_b");
        assert!(node_b.fact_indices.is_empty());
    }

    #[test]
    fn build_view_acyclic_network() {
        // Verify build_view works correctly on a simple non-cyclic network.
        let role = "http://example.com/role/test".to_string();
        let arcs = vec![(
            role.clone(),
            PresentationArc {
                from: "root".to_string(),
                to: "child".to_string(),
                order: Some(Decimal::new(1, 0)),
            },
        )];
        let taxonomy = create_taxonomy(arcs, vec![]);
        let view = build_view(&[], &taxonomy);
        assert_eq!(view.sections.len(), 1);
        assert_eq!(view.sections[0].nodes.len(), 1);
        assert_eq!(view.sections[0].nodes[0].concept_id, "child");
    }

    #[test]
    fn sibling_order_respected() {
        let role = "http://example.com/role/order".to_string();
        let arcs = vec![
            (
                role.clone(),
                PresentationArc {
                    from: "root".to_string(),
                    to: "b".to_string(),
                    order: Some(Decimal::new(2, 0)),
                },
            ),
            (
                role.clone(),
                PresentationArc {
                    from: "root".to_string(),
                    to: "a".to_string(),
                    order: Some(Decimal::new(1, 0)),
                },
            ),
        ];
        let taxonomy = create_taxonomy(arcs, vec![]);
        let view = build_view(&[], &taxonomy);

        let section = &view.sections[0];
        assert_eq!(section.nodes[0].concept_id, "a");
        assert_eq!(section.nodes[1].concept_id, "b");
    }
}
