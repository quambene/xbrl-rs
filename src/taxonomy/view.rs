//! Read-only taxonomy-centric views for presentation trees and single concepts.

use super::{
    Concept, GroupParticle, Label, Occurrence, Particle, PresentationArc, Reference, TaxonomySet,
};
use crate::ExpandedName;
use rust_decimal::Decimal;
use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
};

/// A hierarchical view of taxonomy concepts organized by presentation role.
#[derive(Debug)]
pub struct TaxonomyView<'a> {
    /// One section per extended link role found in the presentation linkbase.
    pub sections: Vec<TaxonomySectionView<'a>>,
}

impl<'a> TaxonomyView<'a> {
    /// Build a taxonomy-wide presentation view across all roles.
    pub fn build(taxonomy: &'a TaxonomySet) -> Self {
        build_taxonomy_view(taxonomy)
    }

    /// Build a taxonomy presentation view for one role URI.
    pub fn build_for_role(taxonomy: &'a TaxonomySet, role: &'a str) -> Option<Self> {
        taxonomy.presentation_arcs(role).map(|arcs| TaxonomyView {
            sections: vec![build_section(role, arcs, taxonomy)],
        })
    }
}

/// One presentation role section in a taxonomy view.
#[derive(Debug)]
pub struct TaxonomySectionView<'a> {
    /// The extended link role URI identifying this section.
    pub role: &'a str,
    /// Root concept nodes for this presentation section.
    pub nodes: Vec<TaxonomyTreeNode<'a>>,
}

/// A concept node in a taxonomy presentation tree.
#[derive(Debug)]
pub struct TaxonomyTreeNode<'a> {
    /// The concept represented by this node.
    pub concept: &'a Concept,
    /// Labels for this concept.
    pub labels: &'a [Label],
    /// Depth in the tree. Root nodes have depth 0.
    pub depth: usize,
    /// Child concepts, ordered by presentation arc order.
    pub children: Vec<TaxonomyTreeNode<'a>>,
}

/// A focused view of a single concept and its relationships.
#[derive(Debug)]
pub struct ConceptView<'a> {
    /// The concept represented by this view.
    pub concept: &'a Concept,
    /// Labels for this concept.
    pub labels: &'a [Label],
    /// References for this concept, keyed by concept id when available.
    pub references: &'a [Reference],
    /// Direct tuple parent concept when this concept belongs to a tuple.
    pub parent_tuple: Option<&'a Concept>,
    /// Tuple ancestor chain from root tuple to direct tuple parent.
    pub tuple_ancestors: Vec<&'a Concept>,
    /// Presentation parents of this concept across roles.
    pub presentation_parents: Vec<PresentationRelationView<'a>>,
    /// Presentation children of this concept across roles.
    pub presentation_children: Vec<PresentationRelationView<'a>>,
    /// Tuple content model projected for display, when this concept is a tuple.
    pub tuple_content: Option<TupleParticleView<'a>>,
}

impl<'a> ConceptView<'a> {
    /// Build a concept view from a concept reference.
    pub fn build(concept: &'a Concept, taxonomy: &'a TaxonomySet) -> Self {
        build_concept_view(concept, taxonomy)
    }

    /// Build a concept view from a concept qualified name.
    pub fn build_from_name(concept_name: &ExpandedName, taxonomy: &'a TaxonomySet) -> Option<Self> {
        taxonomy
            .find_concept(concept_name)
            .map(|concept| build_concept_view(concept, taxonomy))
    }

    /// Build a concept view from a concept id attribute.
    pub fn build_from_id(concept_id: &str, taxonomy: &'a TaxonomySet) -> Option<Self> {
        taxonomy
            .find_concept_by_id(concept_id)
            .map(|concept| build_concept_view(concept, taxonomy))
    }
}

/// A presentation relationship from or to a concept in a given role.
#[derive(Debug)]
pub struct PresentationRelationView<'a> {
    /// The role URI where this relationship is defined.
    pub role: &'a str,
    /// The related concept name used by the arc endpoint.
    pub concept_name: &'a ExpandedName,
    /// The resolved related concept when available in the DTS.
    pub concept: Option<&'a Concept>,
    /// Arc order among siblings.
    pub order: Option<Decimal>,
    /// Preferred label role for this edge, if present.
    pub preferred_label: Option<&'a str>,
}

/// A projected tuple content-model particle for display.
#[derive(Debug)]
pub enum TupleParticleView<'a> {
    /// An element particle.
    Element {
        /// Element-local view information.
        element: TupleElementView<'a>,
        /// Occurrence constraints.
        occurs: &'a Occurrence,
    },
    /// A sequence compositor.
    Sequence {
        /// Child particles.
        children: Vec<TupleParticleView<'a>>,
        /// Occurrence constraints.
        occurs: &'a Occurrence,
    },
    /// A choice compositor.
    Choice {
        /// Child particles.
        children: Vec<TupleParticleView<'a>>,
        /// Occurrence constraints.
        occurs: &'a Occurrence,
    },
    /// A group reference.
    GroupRef {
        /// Referenced group local name.
        name: &'a str,
        /// Occurrence constraints.
        occurs: &'a Occurrence,
    },
    /// An inline group definition.
    GroupDef {
        /// Group name, if present.
        name: Option<&'a str>,
        /// Wrapped particle.
        particle: Box<TupleParticleView<'a>>,
        /// Occurrence constraints.
        occurs: &'a Occurrence,
    },
}

/// A tuple element projection.
#[derive(Debug)]
pub struct TupleElementView<'a> {
    /// Allowed element local name.
    pub local_name: &'a str,
    /// Resolved concept for this element, if present in the taxonomy.
    pub concept: Option<&'a Concept>,
}

fn build_taxonomy_view<'a>(taxonomy: &'a TaxonomySet) -> TaxonomyView<'a> {
    let sections = taxonomy
        .presentations()
        .iter()
        .map(|(role, arcs)| build_section(role.as_str(), arcs, taxonomy))
        .collect();

    TaxonomyView { sections }
}

fn build_section<'a>(
    role: &'a str,
    arcs: &'a [PresentationArc],
    taxonomy: &'a TaxonomySet,
) -> TaxonomySectionView<'a> {
    let concept_index = taxonomy
        .elements()
        .into_iter()
        .map(|concept| (&concept.name, concept))
        .collect::<HashMap<_, _>>();

    let mut arc_index: HashMap<&'a ExpandedName, Vec<&'a PresentationArc>> = HashMap::new();
    for arc in arcs {
        arc_index.entry(&arc.from).or_default().push(arc);
    }

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
    let mut nodes = Vec::with_capacity(roots.len());

    for root_id in roots {
        if let Some(node) = build_node(
            &concept_index,
            &arc_index,
            root_id,
            0,
            taxonomy,
            &mut visited,
        ) {
            nodes.push(node);
        }
    }

    TaxonomySectionView { role, nodes }
}

/// Find root concept IDs: those that appear as from but never as to.
fn find_roots<'a>(
    arcs: &'a [PresentationArc],
    arc_index: &HashMap<&'a ExpandedName, Vec<&'a PresentationArc>>,
) -> Vec<&'a ExpandedName> {
    let to_set: HashSet<&ExpandedName> = arcs.iter().map(|arc| &arc.to).collect();
    let mut roots: Vec<&'a ExpandedName> = arc_index
        .keys()
        .copied()
        .filter(|from| !to_set.contains(*from))
        .collect();

    roots.sort_by(|a, b| {
        let min_order = |id: &&ExpandedName| {
            arc_index
                .get(*id)
                .and_then(|children| children.iter().filter_map(|arc| arc.order).min())
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

fn build_node<'a>(
    concept_index: &HashMap<&'a ExpandedName, &'a Concept>,
    arc_index: &HashMap<&'a ExpandedName, Vec<&'a PresentationArc>>,
    concept_id: &'a ExpandedName,
    depth: usize,
    taxonomy: &'a TaxonomySet,
    visited: &mut HashSet<&'a ExpandedName>,
) -> Option<TaxonomyTreeNode<'a>> {
    if !visited.insert(concept_id) {
        return None;
    }

    let concept = concept_index.get(concept_id).copied()?;
    let labels = taxonomy.labels(concept_id).unwrap_or(&[]);

    let child_arcs = arc_index.get(concept_id).map(Vec::as_slice).unwrap_or(&[]);
    let mut children = Vec::with_capacity(child_arcs.len());

    for arc in child_arcs {
        if let Some(child_node) = build_node(
            concept_index,
            arc_index,
            &arc.to,
            depth + 1,
            taxonomy,
            visited,
        ) {
            children.push(child_node);
        }
    }

    visited.remove(concept_id);

    Some(TaxonomyTreeNode {
        concept,
        labels,
        depth,
        children,
    })
}

fn build_concept_view<'a>(concept: &'a Concept, taxonomy: &'a TaxonomySet) -> ConceptView<'a> {
    let labels = taxonomy.labels(&concept.name).unwrap_or(&[]);
    let references = concept
        .id
        .as_deref()
        .and_then(|id| taxonomy.references_for(id))
        .unwrap_or(&[]);
    let parent_tuple = concept
        .id
        .as_deref()
        .and_then(|id| taxonomy.find_parent_tuple(id));
    let tuple_ancestors = concept
        .id
        .as_deref()
        .map(|id| {
            taxonomy
                .tuple_ancestor_ids(id)
                .into_iter()
                .filter_map(|ancestor_id| taxonomy.find_concept_by_id(&ancestor_id))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let mut presentation_parents = Vec::new();
    let mut presentation_children = Vec::new();

    for (role, arcs) in taxonomy.presentations() {
        for arc in arcs {
            if arc.to == concept.name {
                presentation_parents.push(PresentationRelationView {
                    role: role.as_str(),
                    concept_name: &arc.from,
                    concept: taxonomy.find_concept(&arc.from),
                    order: arc.order,
                    preferred_label: arc.preferred_label.as_ref().map(|x| x.as_ref()),
                });
            }

            if arc.from == concept.name {
                presentation_children.push(PresentationRelationView {
                    role: role.as_str(),
                    concept_name: &arc.to,
                    concept: taxonomy.find_concept(&arc.to),
                    order: arc.order,
                    preferred_label: arc.preferred_label.as_ref().map(|x| x.as_ref()),
                });
            }
        }
    }

    let tuple_content = concept
        .content_model
        .as_ref()
        .map(|particle| project_particle(particle, taxonomy));

    ConceptView {
        concept,
        labels,
        references,
        parent_tuple,
        tuple_ancestors,
        presentation_parents,
        presentation_children,
        tuple_content,
    }
}

fn project_particle<'a>(
    particle: &'a Particle,
    taxonomy: &'a TaxonomySet,
) -> TupleParticleView<'a> {
    match particle {
        Particle::Element { element, occurs } => {
            let local_name = element.local_name();
            TupleParticleView::Element {
                element: TupleElementView {
                    local_name,
                    concept: find_concept_by_local_name(taxonomy, local_name),
                },
                occurs,
            }
        }
        Particle::Sequence { children, occurs } => TupleParticleView::Sequence {
            children: children
                .iter()
                .map(|child| project_particle(child, taxonomy))
                .collect(),
            occurs,
        },
        Particle::Choice { children, occurs } => TupleParticleView::Choice {
            children: children
                .iter()
                .map(|child| project_particle(child, taxonomy))
                .collect(),
            occurs,
        },
        Particle::Group { group, occurs } => match group {
            GroupParticle::Ref(qname) => TupleParticleView::GroupRef {
                name: &qname.local_name,
                occurs,
            },
            GroupParticle::Def(group_def) => TupleParticleView::GroupDef {
                name: group_def
                    .name
                    .as_ref()
                    .map(|qname| qname.local_name.as_str()),
                particle: Box::new(project_particle(&group_def.particle, taxonomy)),
                occurs,
            },
        },
    }
}

fn find_concept_by_local_name<'a>(
    taxonomy: &'a TaxonomySet,
    local_name: &str,
) -> Option<&'a Concept> {
    taxonomy
        .elements()
        .into_iter()
        .find(|concept| concept.name.local_name == local_name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ExpandedName, PresentationArc, taxonomy::TaxonomySet};
    use rust_decimal::Decimal;

    #[test]
    fn taxonomy_view_empty_taxonomy() {
        let taxonomy = TaxonomySet::default();
        let view = TaxonomyView::build(&taxonomy);

        assert!(view.sections.is_empty());
    }

    #[test]
    fn taxonomy_view_missing_role_returns_none() {
        let taxonomy = TaxonomySet::default();
        let view = TaxonomyView::build_for_role(&taxonomy, "http://example.com/role");

        assert!(view.is_none());
    }

    #[test]
    fn taxonomy_view_section_created_for_existing_role_without_concepts() {
        let mut taxonomy = TaxonomySet::default();
        taxonomy.add_presentation_arc(
            "http://example.com/role".to_string(),
            PresentationArc {
                from: ExpandedName::new("http://example.com/ns".into(), "root".into()),
                to: ExpandedName::new("http://example.com/ns".into(), "child".into()),
                order: Some(Decimal::new(1, 0)),
                preferred_label: None,
                arcrole: "http://www.xbrl.org/2003/arcrole/parent-child".into(),
            },
        );

        let view = TaxonomyView::build(&taxonomy);

        assert_eq!(view.sections.len(), 1);
        assert_eq!(view.sections[0].role, "http://example.com/role");
        // No concept definitions were added, so nodes are skipped.
        assert!(view.sections[0].nodes.is_empty());
    }

    #[test]
    fn concept_view_build_from_name_returns_none_for_missing_concept() {
        let taxonomy = TaxonomySet::default();
        let view = ConceptView::build_from_name(
            &ExpandedName::new("http://example.com/ns".into(), "missing".into()),
            &taxonomy,
        );

        assert!(view.is_none());
    }

    #[test]
    fn concept_view_build_from_id_returns_none_for_missing_concept() {
        let taxonomy = TaxonomySet::default();
        let view = ConceptView::build_from_id("missing", &taxonomy);

        assert!(view.is_none());
    }
}
