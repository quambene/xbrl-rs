use super::{Context, ContextId, Fact, InstanceDocument, ItemFact, TupleFact, Unit};
use crate::{
    ExpandedName, NamespacePrefix, NamespaceUri, PresentationArc, RoleUri, TaxonomySet,
    taxonomy::{Concept, ElementParticle, GroupParticle, Particle, PeriodType},
};
use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
};

const ARCROLE_ALL: &str = "http://xbrl.org/int/dim/arcrole/all";
const ARCROLE_NOT_ALL: &str = "http://xbrl.org/int/dim/arcrole/notAll";
const ARCROLE_DOMAIN_MEMBER: &str = "http://xbrl.org/int/dim/arcrole/domain-member";

/// Builds a template instance document based on the taxonomy structure, with
/// nil facts for all concrete items and tuples. The generated instance includes
/// all namespaces and schema refs declared in the taxonomy, and the provided
/// contexts and units.
///
/// Tuple facts are emitted according to the schema structure defined by tuple
/// content models. For exclusive single-choice tuple models, only a nil tuple
/// placeholder is emitted without any children, since the template is intended
/// to be fully populated by the user.
///
/// Dimensional items (members of dimensional base sets) are emitted with all
/// applicable dimensional contexts for their period type. If no applicable
/// dimensional contexts are provided for a dimensional item’s period type, that
/// item is omitted.
pub(crate) fn build_instance(
    taxonomy: &TaxonomySet,
    namespaces: HashMap<NamespacePrefix, NamespaceUri>,
    instant_context: Context,
    duration_context: Context,
    dimensional_instant_contexts: Vec<Context>,
    dimensional_duration_contexts: Vec<Context>,
    units: &[Unit],
) -> InstanceDocument {
    let mut instance = InstanceDocument::default();

    for (prefix, uri) in namespaces {
        instance.add_namespace(prefix, uri);
    }

    for schema_url in taxonomy.schema_refs().keys() {
        instance.add_schema_ref(schema_url.to_string());
    }

    let instant_context_ref = instant_context.id.clone();
    let duration_context_ref = duration_context.id.clone();

    instance.add_context(instant_context);
    instance.add_context(duration_context);

    let dimensional_instant_context_refs = dimensional_instant_contexts
        .iter()
        .map(|ctx| ctx.id.clone())
        .collect::<Vec<_>>();
    let dimensional_duration_context_refs = dimensional_duration_contexts
        .iter()
        .map(|ctx| ctx.id.clone())
        .collect::<Vec<_>>();

    for context in dimensional_instant_contexts {
        instance.add_context(context);
    }

    for context in dimensional_duration_contexts {
        instance.add_context(context);
    }

    for unit in units {
        instance.add_unit(unit.clone());
    }

    // Build a deterministic schema graph using tuple content models. The
    // traversal order is schema discovery order + in-schema declaration
    // order + particle child order.
    let mut recursion_path: HashSet<ExpandedName> = HashSet::new();
    let mut emitted_items: HashSet<ExpandedName> = HashSet::new();
    let mut emitted_dimensional_items: HashSet<(ExpandedName, ContextId)> = HashSet::new();
    let mut emitted_tuples: HashSet<ExpandedName> = HashSet::new();
    let concepts = taxonomy.concepts().collect::<Vec<_>>();
    let dimensional_hypercube_items = dimensional_hypercube_items(taxonomy, &[]);
    let schema_index = build_schema_child_index(&concepts, taxonomy);
    let roots = schema_roots(&concepts, &schema_index);
    let mut seeded_nodes: HashSet<ExpandedName> = HashSet::new();
    let skip_items: HashSet<ExpandedName> = HashSet::new();

    for root in roots {
        seeded_nodes.insert(root.clone());
        let mut hoisted: Vec<Fact> = Vec::new();
        populate_from_tree(
            &schema_index,
            root,
            taxonomy,
            &instant_context_ref,
            &duration_context_ref,
            &dimensional_instant_context_refs,
            &dimensional_duration_context_refs,
            units,
            &mut instance.facts,
            &mut emitted_items,
            &mut emitted_dimensional_items,
            &mut emitted_tuples,
            &mut recursion_path,
            None,
            &dimensional_hypercube_items,
            &skip_items,
            &mut hoisted,
        );
        instance.facts.extend(hoisted);
    }

    for concept in &concepts {
        if !schema_participates(concept, &schema_index) {
            continue;
        }

        if seeded_nodes.contains(&concept.name) {
            continue;
        }

        // Skip items already emitted as tuple children during the root traversal
        // above. Without this guard the fallback loop would re-emit them at the
        // top level, producing duplicate facts.
        if emitted_items.contains(&concept.name) {
            continue;
        }

        let mut hoisted: Vec<Fact> = Vec::new();

        populate_from_tree(
            &schema_index,
            &concept.name,
            taxonomy,
            &instant_context_ref,
            &duration_context_ref,
            &dimensional_instant_context_refs,
            &dimensional_duration_context_refs,
            units,
            &mut instance.facts,
            &mut emitted_items,
            &mut emitted_dimensional_items,
            &mut emitted_tuples,
            &mut recursion_path,
            None,
            &dimensional_hypercube_items,
            &skip_items,
            &mut hoisted,
        );

        instance.facts.extend(hoisted);
    }

    instance
}

/// Builds a template instance document restricted to the given presentation
/// roles. Facts are emitted in presentation-arc order; concepts not covered by
/// any of the specified roles are omitted.
///
/// Tuple subtrees are always populated from the schema content model (same as
/// [`build_instance`]) so the tuple structure stays consistent regardless of
/// what the presentation linkbase says about tuple children.
#[allow(clippy::too_many_arguments)]
pub(crate) fn build_instance_from_sections(
    taxonomy: &TaxonomySet,
    roles: &[RoleUri],
    namespaces: HashMap<NamespacePrefix, NamespaceUri>,
    instant_context: Context,
    duration_context: Context,
    dimensional_instant_contexts: Vec<Context>,
    dimensional_duration_contexts: Vec<Context>,
    units: &[Unit],
    dimensional_hypercubes: &[ExpandedName],
) -> InstanceDocument {
    let mut instance = InstanceDocument::default();

    for (prefix, uri) in namespaces {
        instance.add_namespace(prefix, uri);
    }

    for schema_url in taxonomy.schema_refs().keys() {
        instance.add_schema_ref(schema_url.to_string());
    }

    let instant_context_ref = instant_context.id.clone();
    let duration_context_ref = duration_context.id.clone();

    instance.add_context(instant_context);
    instance.add_context(duration_context);

    let dimensional_instant_context_refs = dimensional_instant_contexts
        .iter()
        .map(|ctx| ctx.id.clone())
        .collect::<Vec<_>>();
    let dimensional_duration_context_refs = dimensional_duration_contexts
        .iter()
        .map(|ctx| ctx.id.clone())
        .collect::<Vec<_>>();

    for context in dimensional_instant_contexts {
        instance.add_context(context);
    }

    for context in dimensional_duration_contexts {
        instance.add_context(context);
    }

    for unit in units {
        instance.add_unit(unit.clone());
    }

    let target_dimensional_items = dimensional_hypercube_items(taxonomy, dimensional_hypercubes);
    // When a hypercube filter is active, facts from other hypercubes must not be
    // emitted at all — neither with dimensional nor with plain contexts. Compute
    // the "skip" set: all hypercube members minus the target ones.
    let skip_items: HashSet<ExpandedName> = if dimensional_hypercubes.is_empty() {
        HashSet::new()
    } else {
        let all_dimensional_items = dimensional_hypercube_items(taxonomy, &[]);
        all_dimensional_items
            .difference(&target_dimensional_items)
            .cloned()
            .collect()
    };
    let concepts = taxonomy.concepts().collect::<Vec<_>>();
    let schema_index = build_schema_child_index(&concepts, taxonomy);

    let mut emitted_items: HashSet<ExpandedName> = HashSet::new();
    let mut emitted_dimensional_items: HashSet<(ExpandedName, ContextId)> = HashSet::new();
    let mut emitted_tuples: HashSet<ExpandedName> = HashSet::new();
    let mut recursion_path: HashSet<ExpandedName> = HashSet::new();

    for role in roles {
        let Some(arcs) = taxonomy.presentation_arcs(role.as_str()) else {
            continue;
        };

        let mut arc_index: HashMap<&ExpandedName, Vec<&PresentationArc>> = HashMap::new();

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

        let roots = super::view::find_roots(arcs, &arc_index);

        for root in roots {
            recursion_path.clear();
            populate_from_presentation(
                &arc_index,
                &schema_index,
                root,
                taxonomy,
                &instant_context_ref,
                &duration_context_ref,
                &dimensional_instant_context_refs,
                &dimensional_duration_context_refs,
                units,
                &mut instance.facts,
                &mut emitted_items,
                &mut emitted_dimensional_items,
                &mut emitted_tuples,
                &mut recursion_path,
                &target_dimensional_items,
                &skip_items,
            );
        }
    }

    instance
}

/// Walk one node of the presentation tree and emit facts.
///
/// - Concrete tuple  → delegates to [`populate_from_tree`] (schema content model owns children).
/// - Concrete item   → emits a nil [`ItemFact`]; recurses into presentation children.
/// - Abstract / grouping → recurses into presentation children.
///
/// `skip_items` lists concepts that belong to hypercubes for which no dimensional
/// contexts were provided. These are marked as emitted but not pushed as facts.
#[allow(clippy::too_many_arguments)]
fn populate_from_presentation(
    arc_index: &HashMap<&ExpandedName, Vec<&PresentationArc>>,
    schema_index: &HashMap<ExpandedName, Vec<ExpandedName>>,
    concept_name: &ExpandedName,
    taxonomy: &TaxonomySet,
    instant_ctx: &ContextId,
    duration_ctx: &ContextId,
    dimensional_instant_ctxs: &[ContextId],
    dimensional_duration_ctxs: &[ContextId],
    units: &[Unit],
    facts: &mut Vec<Fact>,
    emitted_items: &mut HashSet<ExpandedName>,
    emitted_dimensional_items: &mut HashSet<(ExpandedName, ContextId)>,
    emitted_tuples: &mut HashSet<ExpandedName>,
    recursion_path: &mut HashSet<ExpandedName>,
    dimensional_hypercube_items: &HashSet<ExpandedName>,
    skip_items: &HashSet<ExpandedName>,
) {
    if !recursion_path.insert(concept_name.clone()) {
        return;
    }

    if let Some(concept) = taxonomy.find_concept(concept_name) {
        if concept.is_tuple() && !concept.is_abstract {
            // Delegate entirely to schema-based traversal so tuple children
            // follow the xs:complexType content model. Use a fresh
            // recursion_path so the schema walk's visited set doesn't
            // contaminate the outer presentation walk.
            let mut tuple_recursion_path: HashSet<ExpandedName> = HashSet::new();
            let mut hoisted: Vec<Fact> = Vec::new();
            populate_from_tree(
                schema_index,
                concept_name,
                taxonomy,
                instant_ctx,
                duration_ctx,
                dimensional_instant_ctxs,
                dimensional_duration_ctxs,
                units,
                facts,
                emitted_items,
                emitted_dimensional_items,
                emitted_tuples,
                &mut tuple_recursion_path,
                None,
                dimensional_hypercube_items,
                skip_items,
                &mut hoisted,
            );
            facts.extend(hoisted);
            recursion_path.remove(concept_name);
            return;
        }

        if !concept.is_abstract
            && let Some(ref period_type) = concept.period_type
        {
            if dimensional_hypercube_items.contains(concept_name) {
                let context_refs = dimensional_context_refs_for_period(
                    period_type,
                    dimensional_instant_ctxs,
                    dimensional_duration_ctxs,
                );
                if context_refs.is_empty() {
                    recursion_path.remove(concept_name);
                    return;
                }

                for context_ref in context_refs {
                    if !emitted_dimensional_items
                        .insert((concept_name.clone(), context_ref.clone()))
                    {
                        continue;
                    }

                    let mut fact = ItemFact::new(
                        None,
                        concept.name.clone(),
                        context_ref.to_string(),
                        unit_ref_for_concept(concept, units),
                        String::new(),
                        true,
                        None,
                        None,
                    );
                    fact.set_nil(true);
                    facts.push(Fact::Item(fact));
                }
            } else if skip_items.contains(concept_name) {
                // Member of a hypercube we have no contexts for: omit but mark
                // as emitted so the fallback traversal doesn't re-emit it.
                emitted_items.insert(concept_name.clone());
            } else if emitted_items.insert(concept_name.clone()) {
                let context_ref =
                    default_context_ref_for_period(period_type, instant_ctx, duration_ctx);
                let mut fact = ItemFact::new(
                    None,
                    concept.name.clone(),
                    context_ref.to_string(),
                    unit_ref_for_concept(concept, units),
                    String::new(),
                    true,
                    None,
                    None,
                );
                fact.set_nil(true);
                facts.push(Fact::Item(fact));
            }
        }
    }

    // Recurse into presentation children for abstract concepts, concrete items,
    // and concepts not found in the schema.
    let children = arc_index
        .get(concept_name)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    for arc in children {
        populate_from_presentation(
            arc_index,
            schema_index,
            &arc.to,
            taxonomy,
            instant_ctx,
            duration_ctx,
            dimensional_instant_ctxs,
            dimensional_duration_ctxs,
            units,
            facts,
            emitted_items,
            emitted_dimensional_items,
            emitted_tuples,
            recursion_path,
            dimensional_hypercube_items,
            skip_items,
        );
    }

    recursion_path.remove(concept_name);
}

/// Recursively walk one node of the presentation tree and emit facts.
///
/// - Concrete tuple -> push a [`TupleFact`] and recurse into its children.
/// - Concrete item  -> push an [`ItemFact`] (nil placeholder).  If the item
///   is not a valid schema child of the enclosing tuple (per its
///   `xs:complexType` content model) it is pushed to `hoisted` instead,
///   which `from_taxonomy` appends to the top-level facts after all sections
///   have been traversed.
/// - Abstract / grouping -> recurse into children at the same level.
#[allow(clippy::too_many_arguments)]
fn populate_from_tree(
    schema_index: &HashMap<ExpandedName, Vec<ExpandedName>>,
    concept_name: &ExpandedName,
    taxonomy: &TaxonomySet,
    instant_ctx: &ContextId,
    duration_ctx: &ContextId,
    dimensional_instant_ctxs: &[ContextId],
    dimensional_duration_ctxs: &[ContextId],
    units: &[Unit],
    facts: &mut Vec<Fact>,
    emitted_items: &mut HashSet<ExpandedName>,
    emitted_dimensional_items: &mut HashSet<(ExpandedName, ContextId)>,
    emitted_tuples: &mut HashSet<ExpandedName>,
    recursion_path: &mut HashSet<ExpandedName>,
    parent_tuple_element: Option<&Concept>,
    dimensional_hypercube_items: &HashSet<ExpandedName>,
    skip_items: &HashSet<ExpandedName>,
    hoisted: &mut Vec<Fact>,
) {
    if !recursion_path.insert(concept_name.clone()) {
        return; // cycle guard within current recursion branch
    }

    let children = schema_index
        .get(concept_name)
        .map(Vec::as_slice)
        .unwrap_or(&[]);

    if let Some(concept) = taxonomy.find_concept(concept_name) {
        if concept.is_tuple() && !concept.is_abstract {
            if emitted_tuples.insert(concept_name.clone()) {
                let mut tuple = TupleFact::new(concept.name.clone());

                // For exclusive single-choice tuple models, generate a nil
                // tuple placeholder.
                if tuple_uses_nil_template(concept) {
                    tuple.set_nil(true);
                    facts.push(Fact::Tuple(tuple));

                    // Skip materialization of this tuple's presentation
                    // descendants and mark them as emitted so fallback
                    // traversal does not emit them at top level.
                    let mut skipped_visited: HashSet<ExpandedName> = HashSet::new();
                    for child_name in children {
                        mark_schema_subtree_as_emitted(
                            schema_index,
                            child_name,
                            emitted_items,
                            emitted_tuples,
                            &mut skipped_visited,
                        );
                    }

                    recursion_path.remove(concept_name);
                    return;
                }

                facts.push(Fact::Tuple(tuple));

                let tuple_children = match facts.last_mut() {
                    Some(Fact::Tuple(tuple)) => tuple.children_mut(),
                    _ => unreachable!(),
                };

                for child_name in children {
                    populate_from_tree(
                        schema_index,
                        child_name,
                        taxonomy,
                        instant_ctx,
                        duration_ctx,
                        dimensional_instant_ctxs,
                        dimensional_duration_ctxs,
                        units,
                        tuple_children,
                        emitted_items,
                        emitted_dimensional_items,
                        emitted_tuples,
                        recursion_path,
                        Some(concept),
                        dimensional_hypercube_items,
                        skip_items,
                        hoisted,
                    );
                }
            }
            recursion_path.remove(concept_name);
            return;
        }

        if !concept.is_abstract
            && let Some(ref period_type) = concept.period_type
        {
            if dimensional_hypercube_items.contains(concept_name) {
                let context_refs = dimensional_context_refs_for_period(
                    period_type,
                    dimensional_instant_ctxs,
                    dimensional_duration_ctxs,
                );

                if context_refs.is_empty() {
                    recursion_path.remove(concept_name);
                    return;
                }

                for context_ref in context_refs {
                    if !emitted_dimensional_items
                        .insert((concept_name.clone(), context_ref.clone()))
                    {
                        continue;
                    }

                    let mut fact = ItemFact::new(
                        None,
                        concept.name.clone(),
                        context_ref.to_string(),
                        unit_ref_for_concept(concept, units),
                        String::new(),
                        true,
                        None,
                        None,
                    );
                    fact.set_nil(true);

                    // Items not allowed by the tuple's content model are hoisted to
                    // the top level so they still appear in the generated template.
                    if let Some(parent_el) = parent_tuple_element
                        && !item_allowed_in_tuple(parent_el, concept, taxonomy)
                    {
                        hoisted.push(Fact::Item(fact));
                    } else {
                        facts.push(Fact::Item(fact));
                    }
                }
            } else if skip_items.contains(concept_name) {
                // Member of a hypercube we have no contexts for: omit but mark
                // as emitted so the fallback traversal doesn't re-emit it.
                emitted_items.insert(concept_name.clone());
            } else {
                // Mark as emitted so `populate_from_presentation` does not re-emit
                // this concept as a top-level fact later.
                emitted_items.insert(concept_name.clone());

                let context_ref =
                    default_context_ref_for_period(period_type, instant_ctx, duration_ctx);
                let mut fact = ItemFact::new(
                    None,
                    concept.name.clone(),
                    context_ref.to_string(),
                    unit_ref_for_concept(concept, units),
                    String::new(),
                    true,
                    None,
                    None,
                );
                fact.set_nil(true);

                // Items not allowed by the tuple's content model are hoisted to
                // the top level so they still appear in the generated template.
                if let Some(parent_el) = parent_tuple_element
                    && !item_allowed_in_tuple(parent_el, concept, taxonomy)
                {
                    hoisted.push(Fact::Item(fact));
                } else {
                    facts.push(Fact::Item(fact));
                }
            }
        }
    }

    // Recurse children at the same level for abstract/grouping parents.
    for child_name in children {
        populate_from_tree(
            schema_index,
            child_name,
            taxonomy,
            instant_ctx,
            duration_ctx,
            dimensional_instant_ctxs,
            dimensional_duration_ctxs,
            units,
            facts,
            emitted_items,
            emitted_dimensional_items,
            emitted_tuples,
            recursion_path,
            parent_tuple_element,
            dimensional_hypercube_items,
            skip_items,
            hoisted,
        );
    }

    recursion_path.remove(concept_name);
}

fn default_context_ref_for_period<'a>(
    period_type: &PeriodType,
    instant_ctx: &'a ContextId,
    duration_ctx: &'a ContextId,
) -> &'a ContextId {
    match period_type {
        PeriodType::Duration => duration_ctx,
        PeriodType::Instant => instant_ctx,
    }
}

fn dimensional_context_refs_for_period<'a>(
    period_type: &PeriodType,
    dimensional_instant_ctxs: &'a [ContextId],
    dimensional_duration_ctxs: &'a [ContextId],
) -> &'a [ContextId] {
    match period_type {
        PeriodType::Duration => dimensional_duration_ctxs,
        PeriodType::Instant => dimensional_instant_ctxs,
    }
}

/// Collect concepts that belong to dimensional base sets and therefore require
/// dimensional contexts.
///
/// When `hypercube_filter` is non-empty, only concepts that are domain members
/// of the listed hypercubes are returned. Pass an empty slice to collect from
/// all hypercubes.
fn dimensional_hypercube_items(
    taxonomy: &TaxonomySet,
    hypercube_filter: &[ExpandedName],
) -> HashSet<ExpandedName> {
    let filter_set: HashSet<&ExpandedName> = hypercube_filter.iter().collect();
    let use_filter = !filter_set.is_empty();
    let mut result: HashSet<ExpandedName> = HashSet::new();

    for arcs in taxonomy.definitions().values() {
        if use_filter {
            let has_target = arcs.iter().any(|arc| {
                (arc.arcrole.as_str() == ARCROLE_ALL || arc.arcrole.as_str() == ARCROLE_NOT_ALL)
                    && filter_set.contains(&arc.to)
            });

            if !has_target {
                continue;
            }
        }

        let mut domain_children: HashMap<ExpandedName, Vec<ExpandedName>> = HashMap::new();
        let mut roots: Vec<ExpandedName> = Vec::new();

        for arc in arcs {
            match arc.arcrole.as_str() {
                ARCROLE_ALL | ARCROLE_NOT_ALL => {
                    roots.push(arc.from.clone());
                }
                ARCROLE_DOMAIN_MEMBER => {
                    domain_children
                        .entry(arc.from.clone())
                        .or_default()
                        .push(arc.to.clone());
                }
                _ => {}
            }
        }

        let mut stack = roots;
        let mut visited: HashSet<ExpandedName> = HashSet::new();

        while let Some(current) = stack.pop() {
            if !visited.insert(current.clone()) {
                continue;
            }

            if let Some(children) = domain_children.get(&current) {
                stack.extend(children.iter().cloned());
            }
        }

        result.extend(visited);
    }

    result
}

/// Determine the correct `unitRef` string for an element based on its XSD type.
///
/// - Monetary items  -> first currency unit (`is_currency()`)
/// - Shares items    -> first shares unit (`is_shares()`)
/// - Other numeric   -> first pure unit (`is_pure()`)
/// - Non-numeric     -> `None` (unitRef forbidden by the XBRL spec)
fn unit_ref_for_concept(concept: &Concept, units: &[Unit]) -> Option<String> {
    let type_name = &concept.data_type;

    if type_name.is_monetary() {
        return units
            .iter()
            .find(|u| u.is_currency())
            .map(|u| u.id.to_string());
    }

    if type_name.is_shares() {
        return units
            .iter()
            .find(|u| u.is_shares())
            .map(|u| u.id.to_string());
    }

    if type_name.is_numeric() {
        return units.iter().find(|u| u.is_pure()).map(|u| u.id.to_string());
    }

    None
}

/// Returns `true` if `child_element` is a valid schema child of `parent_element`.
///
/// A child is allowed when `parent_element.content_model` is `None` (no explicit
/// content model) or when the child's element name or substitution-group ancestry
/// matches an element particle in the content model.
fn item_allowed_in_tuple(
    parent_element: &Concept,
    child_element: &Concept,
    taxonomy: &TaxonomySet,
) -> bool {
    let Some(model) = &parent_element.content_model else {
        return true;
    };

    if matches_particle_model(model, child_element, taxonomy) {
        return true;
    }

    // Group references are not currently resolved into concrete particles in
    // the semantic model. When they are present, avoid false-negative child
    // rejection and keep generated tuple children in place.
    particle_contains_group_ref(model)
}

/// Returns `true` when the tuple should be emitted as tuple-level nil in
/// `from_taxonomy`, because its content model is an exclusive single-choice.
fn tuple_uses_nil_template(concept: &Concept) -> bool {
    if !concept.is_tuple() {
        return false;
    }

    concept
        .content_model
        .as_ref()
        .is_some_and(is_exclusive_single_choice_particle)
}

/// Marks all concepts reachable from `start` in the presentation arc graph as
/// emitted, so fallback traversal does not materialize them later.
fn mark_schema_subtree_as_emitted(
    schema_index: &HashMap<ExpandedName, Vec<ExpandedName>>,
    start: &ExpandedName,
    emitted_items: &mut HashSet<ExpandedName>,
    emitted_tuples: &mut HashSet<ExpandedName>,
    visited: &mut HashSet<ExpandedName>,
) {
    if !visited.insert(start.clone()) {
        return;
    }

    emitted_items.insert(start.clone());
    emitted_tuples.insert(start.clone());

    if let Some(children) = schema_index.get(start) {
        for child_name in children {
            mark_schema_subtree_as_emitted(
                schema_index,
                child_name,
                emitted_items,
                emitted_tuples,
                visited,
            );
        }
    }
}

/// Builds an index of parent concept name -> child concept names based on tuple
/// content models in the taxonomy.
fn build_schema_child_index(
    concepts: &[&Concept],
    taxonomy: &TaxonomySet,
) -> HashMap<ExpandedName, Vec<ExpandedName>> {
    let mut index: HashMap<ExpandedName, Vec<ExpandedName>> = HashMap::new();

    for parent in concepts {
        if !parent.is_tuple() {
            continue;
        }
        let Some(model) = &parent.content_model else {
            continue;
        };

        let mut children: Vec<ExpandedName> = Vec::new();
        collect_model_children(model, parent, concepts, &mut children);

        children.retain(|child| child != &parent.name);
        children.dedup();

        if children.is_empty() {
            children = fallback_presentation_children(parent, taxonomy);
        }

        if !children.is_empty() {
            index.insert(parent.name.clone(), children);
        }
    }

    index
}

/// Recursively collects allowed child concept names from a particle model.
fn collect_model_children(
    particle: &Particle,
    parent: &Concept,
    concepts: &[&Concept],
    out: &mut Vec<ExpandedName>,
) {
    match particle {
        Particle::Element { element, .. } => {
            let allowed_local = match element {
                ElementParticle::Ref(qname) => qname.local_name.as_str(),
                ElementParticle::Decl(declaration) => declaration.name.as_str(),
            };

            let mut same_namespace_matches = Vec::new();
            let mut any_namespace_matches = Vec::new();

            for concept in concepts {
                let direct_local_match = concept.name.local_name == allowed_local;
                let direct_substitution_match =
                    concept.substitution_group.original.local_name == allowed_local;

                if !(direct_local_match || direct_substitution_match) {
                    continue;
                }

                if concept.name.namespace_uri == parent.name.namespace_uri {
                    same_namespace_matches.push(concept.name.clone());
                } else {
                    any_namespace_matches.push(concept.name.clone());
                }
            }

            if same_namespace_matches.is_empty() {
                out.extend(any_namespace_matches);
            } else {
                out.extend(same_namespace_matches);
            }
        }
        Particle::Sequence { children, .. } | Particle::Choice { children, .. } => {
            for child in children {
                collect_model_children(child, parent, concepts, out);
            }
        }
        Particle::Group { group, .. } => {
            if let GroupParticle::Def(group_def) = group {
                collect_model_children(&group_def.particle, parent, concepts, out);
            }
        }
    }
}

/// Fallback child collection based on presentation relationships when no
/// explicit content model is defined for a tuple concept.
fn fallback_presentation_children(parent: &Concept, taxonomy: &TaxonomySet) -> Vec<ExpandedName> {
    let mut children: Vec<(ExpandedName, Option<rust_decimal::Decimal>)> = taxonomy
        .presentations()
        .values()
        .flat_map(|arcs| arcs.iter())
        .filter(|arc| arc.from == parent.name)
        .map(|arc| (arc.to.clone(), arc.order))
        .collect();

    children.sort_by(|a, b| match (a.1, b.1) {
        (Some(x), Some(y)) => x.cmp(&y),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    });

    let mut ordered = Vec::new();
    for (name, _) in children {
        if !ordered.contains(&name) {
            ordered.push(name);
        }
    }

    ordered
}

/// Returns the root concept names that participate in the schema (tuple or
/// concrete item concepts that are reachable from the presentation
/// relationships).
fn schema_roots<'a>(
    concepts: &'a [&'a Concept],
    schema_index: &HashMap<ExpandedName, Vec<ExpandedName>>,
) -> Vec<&'a ExpandedName> {
    let mut child_names: HashSet<&ExpandedName> = HashSet::new();

    for children in schema_index.values() {
        for child in children {
            child_names.insert(child);
        }
    }

    concepts
        .iter()
        .filter(|concept| schema_participates(concept, schema_index))
        .map(|concept| &concept.name)
        .filter(|name| !child_names.contains(name))
        .collect()
}

/// Returns `true` when the concept participates in the schema as a tuple or
/// concrete item, or as an abstract parent of other participating concepts.
fn schema_participates(
    concept: &Concept,
    schema_index: &HashMap<ExpandedName, Vec<ExpandedName>>,
) -> bool {
    concept.is_tuple()
        || concept.is_concrete_item()
        || (concept.is_abstract() && schema_index.contains_key(&concept.name))
}

/// Returns `true` when `model` effectively represents an exclusive choice
/// where at most one alternative can be selected.
///
/// Supported nested form:
/// - a direct `choice` with `maxOccurs=1`
/// - wrappers (`sequence`/`group`) with `maxOccurs=1` and exactly one child,
///   recursively ending in the same exclusive-choice shape
fn is_exclusive_single_choice_particle(model: &Particle) -> bool {
    match model {
        Particle::Choice { occurs, .. } => occurs.max == Some(1),
        Particle::Sequence { children, occurs } => {
            occurs.max == Some(1)
                && children.len() == 1
                && is_exclusive_single_choice_particle(&children[0])
        }
        Particle::Group { group, occurs } => {
            occurs.max == Some(1)
                && match group {
                    GroupParticle::Def(group_def) => {
                        is_exclusive_single_choice_particle(&group_def.particle)
                    }
                    GroupParticle::Ref(_) => false,
                }
        }
        Particle::Element { .. } => false,
    }
}

fn particle_contains_group_ref(model: &Particle) -> bool {
    match model {
        Particle::Element { .. } => false,
        Particle::Sequence { children, .. } | Particle::Choice { children, .. } => {
            children.iter().any(particle_contains_group_ref)
        }
        Particle::Group { group, .. } => match group {
            GroupParticle::Ref(_) => true,
            GroupParticle::Def(group_def) => particle_contains_group_ref(&group_def.particle),
        },
    }
}

/// Returns `true` if `child_element` satisfies any element particle in `model`,
/// either by a direct name match or via substitution-group ancestry.
fn matches_particle_model(
    model: &Particle,
    child_element: &Concept,
    taxonomy: &TaxonomySet,
) -> bool {
    model
        .elements()
        .iter()
        .any(|element_particle| matches_element_particle(element_particle, child_element, taxonomy))
}

/// Returns `true` if `child_element` satisfies the element particle, either
/// by a direct name match or via its substitution-group ancestry chain.
fn matches_element_particle(
    element_particle: &ElementParticle,
    child_element: &Concept,
    taxonomy: &TaxonomySet,
) -> bool {
    let allowed_local = match element_particle {
        ElementParticle::Ref(qname) => qname.local_name.as_str(),
        ElementParticle::Decl(declaration) => declaration.name.as_str(),
    };

    if child_element.name.local_name == allowed_local {
        return true;
    }

    // Walk the substitution group ancestry: if the child's substitution group
    // (or any ancestor in the chain) matches the declared element particle, the
    // element is a valid substitute.
    let mut current = child_element;

    loop {
        let parent_substitution_group = &current.substitution_group.original;

        if parent_substitution_group.local_name == allowed_local {
            return true;
        }

        match taxonomy.find_concept(parent_substitution_group) {
            Some(parent) => current = parent,
            None => break,
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BaseSubstitutionGroup, Concept, ElementDecl, ElementParticle, ExpandedName, GroupDef,
        GroupParticle, NamespaceUri, Occurrence, Particle, PeriodType, SubstitutionGroup, XbrlType,
    };

    fn expanded_name(local_name: &str) -> ExpandedName {
        ExpandedName::new(
            NamespaceUri::from("http://example.com/ns"),
            local_name.to_owned(),
        )
    }

    #[test]
    fn unit_ref_for_pure_concept_prefers_pure_unit() {
        let concept = Concept {
            id: Some("pureConcept".to_owned()),
            name: expanded_name("pureConcept"),
            data_type: XbrlType::Pure,
            substitution_group: SubstitutionGroup {
                base: BaseSubstitutionGroup::Item,
                original: ExpandedName::new(
                    NamespaceUri::from("http://www.xbrl.org/2003/instance"),
                    "item".to_owned(),
                ),
            },
            period_type: Some(PeriodType::Instant),
            balance: None,
            nillable: true,
            is_abstract: false,
            content_model: None,
        };

        let units = vec![
            Unit::new(
                "EUR".into(),
                vec![ExpandedName::new(
                    NamespaceUri::from("http://www.xbrl.org/2003/iso4217"),
                    "EUR".to_owned(),
                )],
                vec![],
            ),
            Unit::new(
                "pure".into(),
                vec![ExpandedName::new(
                    NamespaceUri::from("http://www.xbrl.org/2003/instance"),
                    "pure".to_owned(),
                )],
                vec![],
            ),
        ];

        assert_eq!(
            unit_ref_for_concept(&concept, &units),
            Some("pure".to_owned())
        );
    }

    #[test]
    fn exclusive_choice_particle_detected() {
        let model = Particle::Choice {
            children: vec![
                Particle::Element {
                    element: ElementParticle::Decl(ElementDecl {
                        name: "A".to_owned(),
                        type_name: None,
                        inline_type: None,
                    }),
                    occurs: Occurrence {
                        min: 0,
                        max: Some(1),
                    },
                },
                Particle::Element {
                    element: ElementParticle::Decl(ElementDecl {
                        name: "B".to_owned(),
                        type_name: None,
                        inline_type: None,
                    }),
                    occurs: Occurrence {
                        min: 0,
                        max: Some(1),
                    },
                },
            ],
            occurs: Occurrence {
                min: 1,
                max: Some(1),
            },
        };

        assert!(is_exclusive_single_choice_particle(&model));
    }

    #[test]
    fn repeating_choice_particle_not_detected() {
        let model = Particle::Choice {
            children: vec![Particle::Element {
                element: ElementParticle::Decl(ElementDecl {
                    name: "A".to_owned(),
                    type_name: None,
                    inline_type: None,
                }),
                occurs: Occurrence {
                    min: 0,
                    max: Some(1),
                },
            }],
            occurs: Occurrence { min: 0, max: None },
        };

        assert!(!is_exclusive_single_choice_particle(&model));
    }

    #[test]
    fn nested_exclusive_choice_particle_detected() {
        let model = Particle::Sequence {
            children: vec![Particle::Group {
                group: GroupParticle::Def(GroupDef {
                    name: None,
                    particle: Box::new(Particle::Choice {
                        children: vec![Particle::Element {
                            element: ElementParticle::Decl(ElementDecl {
                                name: "A".to_owned(),
                                type_name: None,
                                inline_type: None,
                            }),
                            occurs: Occurrence {
                                min: 0,
                                max: Some(1),
                            },
                        }],
                        occurs: Occurrence {
                            min: 0,
                            max: Some(1),
                        },
                    }),
                }),
                occurs: Occurrence {
                    min: 1,
                    max: Some(1),
                },
            }],
            occurs: Occurrence {
                min: 1,
                max: Some(1),
            },
        };

        assert!(is_exclusive_single_choice_particle(&model));
    }

    #[test]
    fn sequence_with_multiple_children_not_detected_as_exclusive_choice() {
        let model = Particle::Sequence {
            children: vec![
                Particle::Element {
                    element: ElementParticle::Decl(ElementDecl {
                        name: "A".to_owned(),
                        type_name: None,
                        inline_type: None,
                    }),
                    occurs: Occurrence {
                        min: 0,
                        max: Some(1),
                    },
                },
                Particle::Element {
                    element: ElementParticle::Decl(ElementDecl {
                        name: "B".to_owned(),
                        type_name: None,
                        inline_type: None,
                    }),
                    occurs: Occurrence {
                        min: 0,
                        max: Some(1),
                    },
                },
            ],
            occurs: Occurrence {
                min: 1,
                max: Some(1),
            },
        };

        assert!(!is_exclusive_single_choice_particle(&model));
    }
}
