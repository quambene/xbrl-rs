//! Document view built from the presentation linkbase.

use super::fact::{Fact, ItemFact};
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
    /// Build a document view from instance facts and a taxonomy.
    ///
    /// `fact_indices` in the returned view are item-only indices in depth-first
    /// order, matching [`InstanceDocument::set_fact_value`] / `set_fact_nil`.
    pub fn build(facts: &[Fact], taxonomy: &'a TaxonomySet) -> Self {
        let mut item_facts = Vec::new();
        let mut parents = Vec::new();
        let mut tuple_fact_names = HashSet::new();

        for fact in facts {
            collect_item_facts_with_parent(fact, None, &mut item_facts, &mut parents);
            collect_tuple_fact_names(fact, &mut tuple_fact_names);
        }

        build_view(&item_facts, &parents, &tuple_fact_names, taxonomy)
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

/// Build a [`DocumentView`] from item-fact indices and tuple-presence context.
fn build_view<'a>(
    facts: &[&ItemFact],
    parents: &[Option<ExpandedName>],
    tuple_fact_names: &HashSet<ExpandedName>,
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
                    tuple_fact_names,
                    &mut visited,
                )
            })
            .collect();

        sections.push(SectionView { role, nodes });
    }

    DocumentView { sections }
}

/// Recursively collect item facts, tracking the parent tuple concept for each
/// fact.
fn collect_item_facts_with_parent<'a>(
    fact: &'a Fact,
    parent_tuple: Option<&ExpandedName>,
    facts: &mut Vec<&'a ItemFact>,
    parents: &mut Vec<Option<ExpandedName>>,
) {
    match fact {
        Fact::Item(item) => {
            facts.push(item);
            parents.push(parent_tuple.cloned());
        }
        Fact::Tuple(tuple) => {
            for child in tuple.children() {
                collect_item_facts_with_parent(child, Some(tuple.concept_name()), facts, parents);
            }
        }
    }
}

/// Recursively collect concept names of all tuple facts.
fn collect_tuple_fact_names(fact: &Fact, names: &mut HashSet<ExpandedName>) {
    match fact {
        Fact::Item(_) => {}
        Fact::Tuple(tuple) => {
            names.insert(tuple.concept_name().clone());
            for child in tuple.children() {
                collect_tuple_fact_names(child, names);
            }
        }
    }
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
#[allow(clippy::too_many_arguments)]
fn build_nodes<'a>(
    arc_index: &HashMap<&'a ExpandedName, Vec<&'a PresentationArc>>,
    parent_id: &'a ExpandedName,
    depth: usize,
    taxonomy: &'a TaxonomySet,
    fact_index: &HashMap<ExpandedName, Vec<usize>>,
    tuple_child_index: &HashMap<(ExpandedName, ExpandedName), Vec<usize>>,
    tuple_fact_names: &HashSet<ExpandedName>,
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
            tuple_fact_names,
            visited,
        );

        // Only include the node if it has facts or children; otherwise it's
        // just a concept with no reported facts.
        //
        // Tuple names are retained when present in the instance even if
        // they currently have no item descendants.
        if !fact_indices.is_empty() || !children.is_empty() || tuple_fact_names.contains(child_id) {
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
    use crate::{Fact, ItemFact, taxonomy::TaxonomySet};
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
        let view = DocumentView::build(&[], &taxonomy);
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
        let facts = vec![Fact::Item(fact)];

        let view = DocumentView::build(&facts, &taxonomy);

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
        let first_fact = facts[node_a.fact_indices[0]]
            .as_item()
            .expect("expected item fact");
        assert_eq!(first_fact.value(), "42");
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
        let view = DocumentView::build(&[], &taxonomy);
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
        let facts = [Fact::Item(fact_c_in_a), Fact::Item(fact_c_in_b)];
        // fact 0 is a child of TupleA, fact 1 is a child of TupleB
        let view = build_view(
            &[
                facts[0].as_item().expect("expected item fact"),
                facts[1].as_item().expect("expected item fact"),
            ],
            &[Some(tuple_a.clone()), Some(tuple_b.clone())],
            &HashSet::new(),
            &taxonomy,
        );

        assert_eq!(view.sections.len(), 1);
        let section = &view.sections[0];
        // TupleA and TupleB are roots; build_nodes returns their children,
        // so section.nodes contains one C-node per tuple parent.
        assert_eq!(section.nodes.len(), 2);

        let c_under_a = &section.nodes[0];
        assert_eq!(c_under_a.concept_name, "C");
        assert_eq!(c_under_a.fact_indices, vec![0]);
        let first = facts[c_under_a.fact_indices[0]]
            .as_item()
            .expect("expected item fact");
        assert_eq!(first.value(), "42");

        let c_under_b = &section.nodes[1];
        assert_eq!(c_under_b.concept_name, "C");
        assert_eq!(c_under_b.fact_indices, vec![1]);
        let second = facts[c_under_b.fact_indices[0]]
            .as_item()
            .expect("expected item fact");
        assert_eq!(second.value(), "99");
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
        let facts = [Fact::Item(fact_a), Fact::Item(fact_b)];
        let view = DocumentView::build(&facts, &taxonomy);

        let section = &view.sections[0];
        assert_eq!(section.nodes[0].concept_name, "a");
        assert_eq!(section.nodes[1].concept_name, "b");
    }

    #[test]
    fn tuple_presence_keeps_tuple_node_without_item_facts() {
        let role = "http://example.com/role/tuple-visibility".to_string();
        let root = ExpandedName::new("http://example.com/ns".into(), "root".into());
        let tuple = ExpandedName::new("http://example.com/ns".into(), "reportType".into());

        let arcs = vec![(
            role.clone(),
            PresentationArc {
                from: root,
                to: tuple.clone(),
                order: Some(Decimal::new(1, 0)),
                preferred_label: None,
                arcrole: "http://www.xbrl.org/2003/arcrole/parent-child".into(),
            },
        )];
        let taxonomy = create_taxonomy(arcs, vec![]);

        let facts: [&ItemFact; 0] = [];
        let parents: [Option<ExpandedName>; 0] = [];
        let tuple_concepts = HashSet::from([tuple.clone()]);

        let view = build_view(&facts, &parents, &tuple_concepts, &taxonomy);

        assert_eq!(view.sections.len(), 1);
        let section = &view.sections[0];
        assert_eq!(section.nodes.len(), 1);
        assert_eq!(section.nodes[0].concept_name, "reportType");
        assert!(section.nodes[0].fact_indices.is_empty());
    }

    #[test]
    fn tuple_without_presence_stays_hidden_when_empty() {
        let role = "http://example.com/role/tuple-hidden".to_string();
        let root = ExpandedName::new("http://example.com/ns".into(), "root".into());
        let tuple = ExpandedName::new("http://example.com/ns".into(), "reportType".into());

        let arcs = vec![(
            role,
            PresentationArc {
                from: root,
                to: tuple,
                order: Some(Decimal::new(1, 0)),
                preferred_label: None,
                arcrole: "http://www.xbrl.org/2003/arcrole/parent-child".into(),
            },
        )];
        let taxonomy = create_taxonomy(arcs, vec![]);

        let facts: [&ItemFact; 0] = [];
        let parents: [Option<ExpandedName>; 0] = [];
        let tuple_concepts = HashSet::new();

        let view = build_view(&facts, &parents, &tuple_concepts, &taxonomy);

        assert_eq!(view.sections.len(), 1);
        assert!(view.sections[0].nodes.is_empty());
    }
}
