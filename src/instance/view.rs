//! Document view built from the presentation linkbase.

use super::fact::ItemFact;
use crate::{
    TaxonomySet,
    taxonomy::{Label, PresentationArc},
};
use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
};

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

    let roles = taxonomy
        .presentations()
        .iter()
        .map(|(role, arcs)| (role.as_str(), arcs))
        .collect::<Vec<_>>();

    let mut sections = Vec::with_capacity(roles.len());

    for (role, arcs) in roles {
        let roots = find_roots(arcs);
        let mut visited: HashSet<&'a str> = HashSet::new();
        let nodes = roots
            .iter()
            .flat_map(|root_id| build_nodes(arcs, root_id, 0, taxonomy, &fact_index, &mut visited))
            .collect();

        sections.push(SectionView { role, nodes });
    }

    DocumentView { sections }
}

/// Find root concept IDs: those that appear as `from` but never as `to`.
pub(super) fn find_roots<'a>(arcs: &'a [PresentationArc]) -> Vec<&'a str> {
    let to_set: HashSet<&str> = arcs.iter().map(|a| a.to.as_str()).collect();
    let mut seen: HashSet<&str> = HashSet::new();
    let mut roots: Vec<&'a str> = Vec::new();

    for arc in arcs {
        let from = arc.from.as_str();
        if !to_set.contains(from) && seen.insert(from) {
            roots.push(from);
        }
    }

    // Order roots by their minimum outgoing arc order.
    roots.sort_by(|a, b| {
        let min_order = |id: &&str| -> f64 {
            arcs.iter()
                .filter(|arc| arc.from.as_str() == *id)
                .filter_map(|arc| arc.order)
                .fold(f64::INFINITY, f64::min)
        };
        min_order(a)
            .partial_cmp(&min_order(b))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    roots
}

/// Recursively build tree nodes for all children of `parent_id`.
fn build_nodes<'a>(
    arcs: &'a [PresentationArc],
    parent_id: &'a str,
    depth: usize,
    taxonomy: &'a TaxonomySet,
    fact_index: &HashMap<String, Vec<usize>>,
    visited: &mut HashSet<&'a str>,
) -> Vec<TreeNode<'a>> {
    if !visited.insert(parent_id) {
        // Cycle detected — skip this branch.
        return Vec::new();
    }

    let mut children_arcs: Vec<&'a PresentationArc> = arcs
        .iter()
        .filter(|a| a.from.as_str() == parent_id)
        .collect();
    children_arcs.sort_by(|a, b| match (a.order, b.order) {
        (Some(x), Some(y)) => x.partial_cmp(&y).unwrap_or(Ordering::Equal),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    });

    let mut nodes = Vec::with_capacity(children_arcs.len());

    for arc in children_arcs {
        let child_id = arc.to.as_str();
        let labels = taxonomy.labels_for(child_id).unwrap_or(&[]);
        let fact_indices = fact_index.get(child_id).cloned().unwrap_or_default();
        let children = build_nodes(arcs, child_id, depth + 1, taxonomy, fact_index, visited);

        nodes.push(TreeNode {
            concept_id: child_id,
            labels,
            depth,
            fact_indices,
            children,
        });
    }

    visited.remove(parent_id);

    nodes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ItemFact,
        taxonomy::{Label, PresentationArc, TaxonomySet},
    };

    fn create_taxonomy(
        arcs: Vec<(String, PresentationArc)>,
        labels: Vec<(String, Label)>,
    ) -> TaxonomySet {
        let mut taxonomy = TaxonomySet::default();
        for (role, arc) in arcs {
            taxonomy.add_presentation_arc(role, arc);
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
                    order: Some(1.0),
                },
            ),
            (
                role.clone(),
                PresentationArc {
                    from: "root".to_string(),
                    to: "child_b".to_string(),
                    order: Some(2.0),
                },
            ),
            (
                role.clone(),
                PresentationArc {
                    from: "child_a".to_string(),
                    to: "grandchild".to_string(),
                    order: Some(1.0),
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
    fn build_view_cycle_protection() {
        let role = "http://example.com/role/cycle".to_string();
        // a -> b -> a  (cycle)
        let arcs = vec![
            (
                role.clone(),
                PresentationArc {
                    from: "a".to_string(),
                    to: "b".to_string(),
                    order: Some(1.0),
                },
            ),
            (
                role.clone(),
                PresentationArc {
                    from: "b".to_string(),
                    to: "a".to_string(),
                    order: Some(1.0),
                },
            ),
        ];
        let taxonomy = create_taxonomy(arcs, vec![]);
        // Should not hang or panic.
        let view = build_view(&[], &taxonomy);
        assert_eq!(view.sections.len(), 1);
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
                    order: Some(2.0),
                },
            ),
            (
                role.clone(),
                PresentationArc {
                    from: "root".to_string(),
                    to: "a".to_string(),
                    order: Some(1.0),
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
