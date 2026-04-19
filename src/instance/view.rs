//! Document view built from the presentation linkbase.

use super::fact::ItemFact;
use crate::{ExpandedName, Label, PresentationArc, TaxonomySet};
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

    /// Build a document view with tuple-parent context for each fact.
    ///
    /// `parents` must be the same length as `facts`. Each element is the
    /// concept name of the direct tuple parent of the corresponding fact, or
    /// `None` if the fact is a top-level item.
    ///
    /// When a parent is known, fact lookup in each tree node prefers the
    /// scoped `(parent_concept, child_concept)` key so that items shared
    /// across multiple tuple types each receive only their own facts.
    pub fn build_with_parents(
        facts: &[&ItemFact],
        parents: &[Option<ExpandedName>],
        taxonomy: &'a TaxonomySet,
    ) -> Self {
        build_view_with_context(facts, parents, taxonomy)
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
    /// Concept name (e.g. `bs.ass`).
    pub concept_name: &'a str,
    /// All labels for this concept. The caller selects the desired language
    /// and role (e.g. `terseLabel`, `label`).
    pub labels: &'a [Label],
    /// Depth in the tree; root nodes have depth 0.
    pub depth: usize,
    /// Indices into the `InstanceDocument::facts()` slice for facts whose concept
    /// maps to this concept name.
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
    let parents = vec![None; facts.len()];
    build_view_with_context(facts, &parents, taxonomy)
}

/// Build a [`DocumentView`] like [`build_view`], but also accepts a parallel
/// `parents` slice that records the direct tuple-parent concept of each fact
/// (or `None` for top-level items).
///
/// When a parent is known, facts are indexed by `(parent_concept, child_concept)`
/// so each tree node receives only the fact that belongs to its tuple context
/// rather than all facts for that concept across all tuple instances.
pub fn build_view_with_context<'a>(
    facts: &[&ItemFact],
    parents: &[Option<ExpandedName>],
    taxonomy: &'a TaxonomySet,
) -> DocumentView<'a> {
    let mut fact_index: HashMap<ExpandedName, Vec<usize>> = HashMap::new();
    let mut tuple_child_index: HashMap<(ExpandedName, ExpandedName), Vec<usize>> = HashMap::new();

    for (i, (fact, parent)) in facts.iter().zip(parents.iter()).enumerate() {
        let concept = fact.concept_name().clone();
        fact_index.entry(concept.clone()).or_default().push(i);
        if let Some(parent) = parent {
            tuple_child_index
                .entry((parent.clone(), concept))
                .or_default()
                .push(i);
        }
    }

    let roles = taxonomy
        .presentations()
        .iter()
        .map(|(role, arcs)| (role.as_str(), arcs))
        .collect::<Vec<_>>();

    let mut sections = Vec::with_capacity(roles.len());

    for (role, arcs) in roles {
        let mut arc_index: HashMap<&'a ExpandedName, Vec<&'a PresentationArc>> = HashMap::new();

        for arc in arcs {
            arc_index.entry(&arc.from).or_default().push(arc);
        }

        // Sort children by `order` up front so `build_nodes` never needs to
        // re-sort.
        for children in arc_index.values_mut() {
            children.sort_by(|a, b| match (a.order, b.order) {
                (Some(x), Some(y)) => x.cmp(&y),
                (Some(_), None) => Ordering::Less,
                (None, Some(_)) => Ordering::Greater,
                (None, None) => Ordering::Equal,
            });
        }

        let roots = find_roots(arcs, &arc_index);
        let mut visited: HashSet<&'a ExpandedName> = HashSet::new();
        let nodes = roots
            .iter()
            .flat_map(|root_id| {
                build_nodes(
                    &arc_index,
                    root_id,
                    0,
                    taxonomy,
                    &fact_index,
                    &tuple_child_index,
                    &mut visited,
                )
            })
            .collect();

        sections.push(SectionView { role, nodes });
    }

    DocumentView { sections }
}

/// Find root concept IDs: those that appear as `from` but never as `to`.
pub(super) fn find_roots<'a>(
    arcs: &'a [PresentationArc],
    arc_index: &HashMap<&'a ExpandedName, Vec<&'a PresentationArc>>,
) -> Vec<&'a ExpandedName> {
    let to_set: HashSet<&ExpandedName> = arcs.iter().map(|a| &a.to).collect();
    let mut seen: HashSet<&ExpandedName> = HashSet::new();
    let mut roots: Vec<&'a ExpandedName> = Vec::new();

    for arc in arcs {
        let from = &arc.from;

        if !to_set.contains(from) && seen.insert(from) {
            roots.push(from);
        }
    }

    // Order roots by their minimum outgoing arc order using the pre-built index.
    roots.sort_by(|a, b| {
        let min_order = |id: &&ExpandedName| {
            arc_index
                .get(*id)
                .and_then(|arcs| arcs.iter().filter_map(|a| a.order).min())
        };
        match (min_order(a), min_order(b)) {
            (Some(x), Some(y)) => x.cmp(&y),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        }
    });

    roots
}

/// Recursively build tree nodes for all children of `parent_id`.
fn build_nodes<'a>(
    arc_index: &HashMap<&'a ExpandedName, Vec<&'a PresentationArc>>,
    parent_id: &'a ExpandedName,
    depth: usize,
    taxonomy: &'a TaxonomySet,
    fact_index: &HashMap<ExpandedName, Vec<usize>>,
    tuple_child_index: &HashMap<(ExpandedName, ExpandedName), Vec<usize>>,
    visited: &mut HashSet<&'a ExpandedName>,
) -> Vec<TreeNode<'a>> {
    if !visited.insert(parent_id) {
        // The branch is skipped if a cycle is detected.
        return Vec::new();
    }

    // Children are already sorted by `order`
    let children_arcs = arc_index.get(parent_id).map(Vec::as_slice).unwrap_or(&[]);

    let mut nodes = Vec::with_capacity(children_arcs.len());

    for arc in children_arcs {
        let child_id = &arc.to;
        let labels = taxonomy.labels(child_id).unwrap_or_default();
        // Prefer scoped lookup by (parent_tuple, child) when available, so
        // each tree node only receives facts from its own tuple context.
        let fact_indices = tuple_child_index
            .get(&(parent_id.clone(), child_id.clone()))
            .or_else(|| fact_index.get(child_id))
            .cloned()
            .unwrap_or_default();
        let children = build_nodes(
            arc_index,
            child_id,
            depth + 1,
            taxonomy,
            fact_index,
            tuple_child_index,
            visited,
        );

        // Only include the node if it has facts or children; otherwise it's
        // just a concept with no reported facts.
        if !fact_indices.is_empty() || !children.is_empty() {
            nodes.push(TreeNode {
                concept_name: &child_id.local_name,
                labels,
                depth,
                fact_indices,
                children,
            });
        }
    }

    visited.remove(parent_id);

    nodes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ItemFact, taxonomy::TaxonomySet};
    use rust_decimal::Decimal;

    fn create_taxonomy(
        arcs: Vec<(String, PresentationArc)>,
        labels: Vec<(ExpandedName, Label)>,
    ) -> TaxonomySet {
        let mut taxonomy = TaxonomySet::default();
        for (role, arc) in arcs {
            taxonomy.add_presentation_arc(role, arc);
        }
        for (concept_name, label) in labels {
            taxonomy.add_label(concept_name, label);
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
                    from: ExpandedName::new("http://example.com/namespace".into(), "root".into()),
                    to: ExpandedName::new("http://example.com/namespace".into(), "child_a".into()),
                    order: Some(Decimal::new(1, 0)),
                    preferred_label: None,
                    arcrole: "http://www.xbrl.org/2003/arcrole/parent-child".into(),
                },
            ),
            (
                role.clone(),
                PresentationArc {
                    from: ExpandedName::new("http://example.com/namespace".into(), "root".into()),
                    to: ExpandedName::new("http://example.com/namespace".into(), "child_b".into()),
                    order: Some(Decimal::new(2, 0)),
                    preferred_label: None,
                    arcrole: "http://www.xbrl.org/2003/arcrole/parent-child".into(),
                },
            ),
            (
                role.clone(),
                PresentationArc {
                    from: ExpandedName::new(
                        "http://example.com/namespace".into(),
                        "child_a".into(),
                    ),
                    to: ExpandedName::new(
                        "http://example.com/namespace".into(),
                        "grandchild".into(),
                    ),
                    order: Some(Decimal::new(1, 0)),
                    preferred_label: None,
                    arcrole: "http://www.xbrl.org/2003/arcrole/parent-child".into(),
                },
            ),
        ];
        let labels = vec![(
            ExpandedName::new("http://example.com/namespace".into(), "child_a".into()),
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
            None,
            ExpandedName::new("http://example.com/namespace".into(), "child_a".into()),
            "ctx1".to_string(),
            None,
            "42".to_string(),
            false,
            None,
            None,
        );
        let facts = vec![&fact];

        let view = build_view(&facts, &taxonomy);

        assert_eq!(view.sections.len(), 1);
        let section = &view.sections[0];
        assert_eq!(section.role, role);

        // child_b has no facts so it is excluded; grandchild has no facts either.
        assert_eq!(section.nodes.len(), 1);

        let node_a = &section.nodes[0];
        assert_eq!(node_a.concept_name, "child_a");
        assert_eq!(node_a.labels.len(), 1);
        assert_eq!(node_a.labels[0].text, "Child A");
        assert_eq!(node_a.labels[0].lang, "en");
        assert_eq!(node_a.depth, 0);
        assert_eq!(node_a.fact_indices.len(), 1);
        assert_eq!(facts[node_a.fact_indices[0]].value(), "42");
        assert_eq!(node_a.children.len(), 0);
    }

    #[test]
    fn build_view_cycle_protection() {
        let role = "http://example.com/role/cycle".to_string();
        // a -> b -> a  (cycle)
        let arcs = vec![
            (
                role.clone(),
                PresentationArc {
                    from: ExpandedName::new("http://example.com/namespace".into(), "a".into()),
                    to: ExpandedName::new("http://example.com/namespace".into(), "b".into()),
                    order: Some(Decimal::new(1, 0)),
                    preferred_label: None,
                    arcrole: "http://www.xbrl.org/2003/arcrole/parent-child".into(),
                },
            ),
            (
                role.clone(),
                PresentationArc {
                    from: ExpandedName::new("http://example.com/namespace".into(), "b".into()),
                    to: ExpandedName::new("http://example.com/namespace".into(), "a".into()),
                    order: Some(Decimal::new(1, 0)),
                    preferred_label: None,
                    arcrole: "http://www.xbrl.org/2003/arcrole/parent-child".into(),
                },
            ),
        ];
        let taxonomy = create_taxonomy(arcs, vec![]);
        // Should not hang or panic.
        let view = build_view(&[], &taxonomy);
        assert_eq!(view.sections.len(), 1);
    }

    #[test]
    fn tuple_context_scopes_fact_indices() {
        // Same item concept C appears as child of two different tuple parents
        // (TupleA and TupleB) in the presentation linkbase.  The instance has
        // one fact for C inside TupleA and one inside TupleB.  Each tree node
        // for C should receive only the fact from its own tuple context.
        let ns = "http://example.com/ns";
        let tuple_a = ExpandedName::new(ns.into(), "TupleA".into());
        let tuple_b = ExpandedName::new(ns.into(), "TupleB".into());
        let item_c = ExpandedName::new(ns.into(), "C".into());
        let role = "http://example.com/role".to_string();

        let arcs = vec![
            (
                role.clone(),
                PresentationArc {
                    from: tuple_a.clone(),
                    to: item_c.clone(),
                    order: Some(Decimal::new(1, 0)),
                    preferred_label: None,
                    arcrole: "http://www.xbrl.org/2003/arcrole/parent-child".into(),
                },
            ),
            (
                role.clone(),
                PresentationArc {
                    from: tuple_b.clone(),
                    to: item_c.clone(),
                    order: Some(Decimal::new(1, 0)),
                    preferred_label: None,
                    arcrole: "http://www.xbrl.org/2003/arcrole/parent-child".into(),
                },
            ),
        ];
        let taxonomy = create_taxonomy(arcs, vec![]);

        let fact_c_in_a = ItemFact::new(
            None,
            item_c.clone(),
            "ctx1".to_string(),
            None,
            "42".to_string(),
            false,
            None,
            None,
        );
        let fact_c_in_b = ItemFact::new(
            None,
            item_c.clone(),
            "ctx1".to_string(),
            None,
            "99".to_string(),
            false,
            None,
            None,
        );
        let facts = [&fact_c_in_a, &fact_c_in_b];
        // fact 0 is a child of TupleA, fact 1 is a child of TupleB
        let parents = [Some(tuple_a.clone()), Some(tuple_b.clone())];

        let view = DocumentView::build_with_parents(&facts, &parents, &taxonomy);

        assert_eq!(view.sections.len(), 1);
        let section = &view.sections[0];
        // TupleA and TupleB are roots; build_nodes returns their children,
        // so section.nodes contains one C-node per tuple parent.
        assert_eq!(section.nodes.len(), 2);

        let c_under_a = &section.nodes[0];
        assert_eq!(c_under_a.concept_name, "C");
        assert_eq!(c_under_a.fact_indices, vec![0]);
        assert_eq!(facts[c_under_a.fact_indices[0]].value(), "42");

        let c_under_b = &section.nodes[1];
        assert_eq!(c_under_b.concept_name, "C");
        assert_eq!(c_under_b.fact_indices, vec![1]);
        assert_eq!(facts[c_under_b.fact_indices[0]].value(), "99");
    }

    #[test]
    fn sibling_order_respected() {
        let role = "http://example.com/role/order".to_string();
        let arcs = vec![
            (
                role.clone(),
                PresentationArc {
                    from: ExpandedName::new("http://example.com/namespace".into(), "root".into()),
                    to: ExpandedName::new("http://example.com/namespace".into(), "b".into()),
                    order: Some(Decimal::new(2, 0)),
                    preferred_label: None,
                    arcrole: "http://www.xbrl.org/2003/arcrole/parent-child".into(),
                },
            ),
            (
                role.clone(),
                PresentationArc {
                    from: ExpandedName::new("http://example.com/namespace".into(), "root".into()),
                    to: ExpandedName::new("http://example.com/namespace".into(), "a".into()),
                    order: Some(Decimal::new(1, 0)),
                    preferred_label: None,
                    arcrole: "http://www.xbrl.org/2003/arcrole/parent-child".into(),
                },
            ),
        ];
        let taxonomy = create_taxonomy(arcs, vec![]);
        let namespace = "http://example.com/namespace";
        let fact_a = ItemFact::new(
            None,
            ExpandedName::new(namespace.into(), "a".into()),
            "ctx1".to_string(),
            None,
            "1".to_string(),
            false,
            None,
            None,
        );
        let fact_b = ItemFact::new(
            None,
            ExpandedName::new(namespace.into(), "b".into()),
            "ctx1".to_string(),
            None,
            "2".to_string(),
            false,
            None,
            None,
        );
        let view = build_view(&[&fact_a, &fact_b], &taxonomy);

        let section = &view.sections[0];
        assert_eq!(section.nodes[0].concept_name, "a");
        assert_eq!(section.nodes[1].concept_name, "b");
    }
}
