//! Schema validation: checks that facts conform to the DTS element definitions
//! and XBRL specification structural rules.

use super::{Severity, ValidationResult, value::PreparedFactValues};
use crate::{
    DeclaredAccuracy, ExpandedName, Fact, InstanceDocument, ItemFact, Period, TaxonomySet,
    TupleFact, Unit,
    taxonomy::{Compositor, Concept, MaxOccurs, PeriodType, TupleChild, XbrlType},
};
use rust_decimal::Decimal;
use std::{
    collections::{HashMap, HashSet},
    str::FromStr,
};

const NS_XBRLI: &str = "http://www.xbrl.org/2003/instance";
const NS_ISO4217: &str = "http://www.xbrl.org/2003/iso4217";
const ROLE_LINK: &str = "http://www.xbrl.org/2003/role/link";
const ROLE_FOOTNOTE: &str = "http://www.xbrl.org/2003/role/footnote";
const ROLE_LABEL: &str = "http://www.xbrl.org/2003/role/label";
const ARCROLE_FACT_FOOTNOTE: &str = "http://www.xbrl.org/2003/arcrole/fact-footnote";
const ARCROLE_ESSENCE_ALIAS: &str = "http://www.xbrl.org/2003/arcrole/essence-alias";

/// Run all schema-level validation checks.
pub(super) fn validate_schema(
    instance: &InstanceDocument,
    taxonomy: &TaxonomySet,
    _prepared: &PreparedFactValues,
    result: &mut ValidationResult,
) {
    validate_contexts(instance, taxonomy, result);
    validate_footnotes(instance, result);
    validate_instance_refs(instance, result);
    validate_essence_alias_units(instance, taxonomy, result);

    for fact in instance.facts() {
        validate_fact(fact, None, instance, taxonomy, result);
    }
}

/// Validate uniqueness of instance-level `roleRef` and `arcroleRef` declarations.
///
/// Adds an error when the same role URI or arcrole URI is declared more than
/// once in the instance document.
fn validate_instance_refs(instance: &InstanceDocument, result: &mut ValidationResult) {
    let mut role_uris = HashSet::new();
    for role_uri in instance.role_refs() {
        if !role_uris.insert(role_uri.as_str()) {
            result.add(
                Severity::Error,
                "spec.duplicate_role_ref",
                format!("Duplicate roleRef for roleURI '{}'", role_uri),
                None,
                None,
            );
        }
    }

    let mut arcrole_uris = HashSet::new();
    for arcrole_uri in instance.arcrole_refs() {
        if !arcrole_uris.insert(arcrole_uri.as_str()) {
            result.add(
                Severity::Error,
                "spec.duplicate_arcrole_ref",
                format!("Duplicate arcroleRef for arcroleURI '{}'", arcrole_uri),
                None,
                None,
            );
        }
    }
}

/// Validate essence-alias relationships by comparing units of matched facts.
///
/// For each definition arc with arcrole `essence-alias`, this checks facts in
/// the same context and reports an error when non-nil source/target facts use
/// semantically different units.
fn validate_essence_alias_units(
    instance: &InstanceDocument,
    taxonomy: &TaxonomySet,
    result: &mut ValidationResult,
) {
    if taxonomy.definitions().is_empty() {
        return;
    }

    let mut facts_by_concept_name: HashMap<ExpandedName, Vec<&ItemFact>> = HashMap::new();

    for fact in instance.item_facts() {
        if let Some(concept) = taxonomy.find_concept(fact.concept_name()) {
            facts_by_concept_name
                .entry(concept.name.clone())
                .or_default()
                .push(fact);
        }
    }

    for arcs in taxonomy.definitions().values() {
        for arc in arcs {
            if arc.arcrole.as_str() != ARCROLE_ESSENCE_ALIAS {
                continue;
            }

            let Some(essence_facts) = facts_by_concept_name.get(&arc.from) else {
                continue;
            };
            let Some(alias_facts) = facts_by_concept_name.get(&arc.to) else {
                continue;
            };

            for essence_fact in essence_facts {
                if essence_fact.is_nil() {
                    continue;
                }
                let Some(essence_unit_ref) = essence_fact.unit_ref() else {
                    continue;
                };
                let Some(essence_unit) = instance.get_unit(essence_unit_ref) else {
                    continue;
                };

                for alias_fact in alias_facts {
                    if alias_fact.is_nil() || essence_fact.context_ref() != alias_fact.context_ref()
                    {
                        continue;
                    }

                    let Some(alias_unit_ref) = alias_fact.unit_ref() else {
                        continue;
                    };
                    let Some(alias_unit) = instance.get_unit(alias_unit_ref) else {
                        continue;
                    };

                    if !units_semantically_equal(essence_unit, alias_unit) {
                        result.add(
                            Severity::Error,
                            "spec.essence_alias_unit_mismatch",
                            format!(
                                "Essence-alias facts '{}' and '{}' in context '{}' must have equal units",
                                essence_fact.concept_name(),
                                alias_fact.concept_name(),
                                essence_fact.context_ref()
                            ),
                            Some(essence_fact.concept_name().to_string()),
                            Some(essence_fact.context_ref().to_string()),
                        );
                    }
                }
            }
        }
    }
}

fn units_semantically_equal(left: &Unit, right: &Unit) -> bool {
    let mut left_num = left.numerator.clone();
    let mut right_num = right.numerator.clone();
    let mut left_den = left.denominator.clone();
    let mut right_den = right.denominator.clone();

    left_num.sort();
    right_num.sort();
    left_den.sort();
    right_den.sort();

    left_num == right_num && left_den == right_den
}

/// Validate footnote link structure and cross-reference integrity.
///
/// Enforces role constraints, locator/resource requirements, `href` target
/// resolution, and `fact-footnote` arc endpoint correctness.
fn validate_footnotes(instance: &InstanceDocument, result: &mut ValidationResult) {
    if instance.footnote_links().is_empty() {
        return;
    }

    let context_ids: HashSet<&str> = instance.contexts().keys().map(|id| id.as_str()).collect();
    let unit_ids: HashSet<&str> = instance.units().keys().map(|id| id.as_str()).collect();
    let fact_ids: HashSet<&str> = instance
        .item_facts()
        .iter()
        .filter_map(|fact| fact.id())
        .collect();

    for footnote_link in instance.footnote_links() {
        if footnote_link.role.as_deref() == Some(ROLE_FOOTNOTE) {
            result.add(
                Severity::Error,
                "spec.invalid_footnote_link_role",
                "footnoteLink cannot use standard footnote role".to_string(),
                None,
                None,
            );
        }

        let mut loc_by_label: HashMap<&str, &str> = HashMap::new();
        let mut res_by_label: HashMap<&str, &crate::instance::FootnoteResource> = HashMap::new();

        for loc in &footnote_link.locators {
            if loc.element_local_name != "loc" {
                result.add(
                    Severity::Error,
                    "spec.invalid_custom_locator",
                    "footnoteLink contains custom locator element".to_string(),
                    None,
                    None,
                );
            }

            if let (Some(label), Some(href)) = (loc.label.as_deref(), loc.href.as_deref()) {
                loc_by_label.insert(label, href);
                if let Some((_file_part, target)) = href_target_id(href) {
                    if context_ids.contains(target) || unit_ids.contains(target) {
                        result.add(
                            Severity::Error,
                            "spec.invalid_footnote_href_target",
                            format!("Footnote locator href '{}' points to context/unit", href),
                            None,
                            None,
                        );
                    }
                    if !fact_ids.contains(target) {
                        result.add(
                            Severity::Error,
                            "spec.footnote_href_not_fact",
                            format!("Footnote locator href '{}' must resolve to a fact id", href),
                            None,
                            None,
                        );
                    }
                } else {
                    result.add(
                        Severity::Error,
                        "spec.invalid_footnote_href",
                        format!("Invalid footnote locator href '{}': missing fragment", href),
                        None,
                        None,
                    );
                }
            }
        }

        for resource in &footnote_link.footnotes {
            if let Some(label) = resource.label.as_deref() {
                res_by_label.insert(label, resource);
            }

            if resource.xml_lang.is_none() && footnote_link.xml_lang.is_none() {
                result.add(
                    Severity::Error,
                    "spec.footnote_missing_lang",
                    "Footnote resource is missing xml:lang (and no link-level xml:lang provided)"
                        .to_string(),
                    None,
                    None,
                );
            }

            if let Some(role) = resource.role.as_deref()
                && (role == ROLE_LINK || role == ROLE_LABEL)
            {
                result.add(
                    Severity::Error,
                    "spec.invalid_footnote_role",
                    format!("Footnote resource uses invalid standard role '{}'", role),
                    None,
                    None,
                );
            }
        }

        for arc in &footnote_link.arcs {
            if arc.arcrole.as_deref() == Some(ARCROLE_FACT_FOOTNOTE) {
                let Some(from) = arc.from.as_deref() else {
                    result.add(
                        Severity::Error,
                        "spec.missing_footnote_arc_from",
                        "fact-footnote arc is missing xlink:from".to_string(),
                        None,
                        None,
                    );
                    continue;
                };
                let Some(to) = arc.to.as_deref() else {
                    result.add(
                        Severity::Error,
                        "spec.missing_footnote_arc_to",
                        "fact-footnote arc is missing xlink:to".to_string(),
                        None,
                        None,
                    );
                    continue;
                };

                let Some(from_href) = loc_by_label.get(from).copied() else {
                    result.add(
                        Severity::Error,
                        "spec.arc_from_out_of_scope",
                        format!(
                            "fact-footnote arc from='{}' does not resolve to a locator",
                            from
                        ),
                        None,
                        None,
                    );
                    continue;
                };

                let Some((_, from_target)) = href_target_id(from_href) else {
                    result.add(
                        Severity::Error,
                        "spec.arc_from_invalid_href",
                        format!("Locator '{}' has invalid href '{}'", from, from_href),
                        None,
                        None,
                    );
                    continue;
                };

                if !fact_ids.contains(from_target) {
                    result.add(
                        Severity::Error,
                        "spec.arc_from_not_fact",
                        format!("fact-footnote arc from='{}' does not point to a fact", from),
                        None,
                        None,
                    );
                }

                let Some(to_resource) = res_by_label.get(to).copied() else {
                    result.add(
                        Severity::Error,
                        "spec.arc_to_out_of_scope",
                        format!(
                            "fact-footnote arc to='{}' does not resolve to footnote resource",
                            to
                        ),
                        None,
                        None,
                    );
                    continue;
                };

                if let Some(role) = to_resource.role.as_deref()
                    && role != ROLE_FOOTNOTE
                {
                    result.add(
                        Severity::Error,
                        "spec.invalid_referenced_footnote_role",
                        format!(
                            "Referenced footnote resource must have role '{}' when role is provided",
                            ROLE_FOOTNOTE
                        ),
                        None,
                        None,
                    );
                }
            }
        }
    }
}

fn href_target_id(href: &str) -> Option<(Option<&str>, &str)> {
    if let Some(rest) = href.strip_prefix('#') {
        return (!rest.is_empty()).then_some((None, rest));
    }
    let (file_part, fragment) = href.split_once('#')?;
    if file_part.is_empty() || fragment.is_empty() {
        return None;
    }
    Some((Some(file_part), fragment))
}

/// Validate a fact against its concept definition recursively, checking content
/// model constraints for tuples and concept-level restrictions for items.
fn validate_fact(
    fact: &Fact,
    parent_tuple: Option<&Concept>,
    instance: &InstanceDocument,
    taxonomy: &TaxonomySet,
    result: &mut ValidationResult,
) {
    match fact {
        Fact::Item(item_fact) => {
            let concept_name = item_fact.concept_name();

            if let Some(parent_tuple) = parent_tuple
                && let Some(child_concept) = taxonomy.find_concept(concept_name)
            {
                validate_tuple_child(child_concept, parent_tuple, taxonomy, result);
            }
            validate_item_fact(item_fact, instance, taxonomy, result);
        }
        Fact::Tuple(tuple_fact) => {
            let tuple_element = validate_tuple_fact(tuple_fact, parent_tuple, taxonomy, result);

            for child in tuple_fact.children() {
                validate_fact(child, tuple_element, instance, taxonomy, result);
            }
        }
    }
}

fn validate_tuple_fact<'a>(
    fact: &TupleFact,
    parent_tuple: Option<&'a Concept>,
    taxonomy: &'a TaxonomySet,
    result: &mut ValidationResult,
) -> Option<&'a Concept> {
    let concept_name = fact.concept_name();

    let Some(concept) = taxonomy.find_concept(concept_name) else {
        result.add(
            Severity::Error,
            "schema.concept_not_found",
            format!(
                "Fact '{}' concept not found in taxonomy",
                concept_name.local_name
            ),
            Some(concept_name.to_string()),
            None,
        );
        return None;
    };

    if let Some(parent_tuple) = parent_tuple
        && let Some(child_concept) = taxonomy.find_concept(concept_name)
    {
        validate_tuple_child(child_concept, parent_tuple, taxonomy, result);
    }

    if !concept.is_tuple() {
        result.add(
            Severity::Error,
            "schema.tuple_requires_tuple_concept",
            format!(
                "Tuple fact reports non-tuple concept '{}'",
                concept_name.local_name
            ),
            Some(concept_name.to_string()),
            None,
        );
        return None;
    }

    if concept.is_abstract() {
        result.add(
            Severity::Error,
            "schema.abstract_concept",
            format!(
                "Fact reports abstract concept '{}'",
                concept_name.local_name
            ),
            Some(concept_name.to_string()),
            None,
        );
    }

    validate_required_tuple_children(fact, concept, taxonomy, result);

    Some(concept)
}

fn validate_tuple_child(
    child_concept: &Concept,
    parent_tuple: &Concept,
    taxonomy: &TaxonomySet,
    result: &mut ValidationResult,
) {
    if parent_tuple.tuple_children.is_empty() {
        return;
    }

    if tuple_allows_child(parent_tuple, &child_concept.name, taxonomy) {
        return;
    }

    result.add(
        Severity::Error,
        "schema.tuple_child_not_allowed",
        format!(
            "Fact '{}' is not allowed as child of tuple '{}'",
            child_concept.name.local_name, parent_tuple.name.local_name
        ),
        Some(child_concept.name.to_string()),
        None,
    );
}

fn tuple_allows_child(
    parent_tuple: &Concept,
    child_concept_name: &ExpandedName,
    taxonomy: &TaxonomySet,
) -> bool {
    parent_tuple
        .tuple_children
        .iter()
        .any(|child_ref| tuple_child_ref_matches_concept(child_ref, child_concept_name, taxonomy))
}

fn validate_required_tuple_children(
    fact: &TupleFact,
    concept: &Concept,
    taxonomy: &TaxonomySet,
    result: &mut ValidationResult,
) {
    // A nil tuple has no content; content model constraints do not apply.
    if fact.is_nil() {
        return;
    }

    // For xs:choice compositors, any one of the declared children satisfies
    // the content model. Per-child minOccurs/maxOccurs checks don't apply
    // in the same way as for xs:sequence.
    if concept.compositor == Some(Compositor::Choice) {
        return;
    }

    for child_ref in &concept.tuple_children {
        let count = fact
            .children()
            .iter()
            .filter(|child| {
                tuple_child_ref_matches_concept(child_ref, child.concept_name(), taxonomy)
            })
            .count() as u32;

        if count < child_ref.min_occurs {
            result.add(
                Severity::Error,
                "schema.tuple_missing_required_child",
                format!(
                    "Tuple '{}' requires at least {} occurrence(s) of child '{}' but found {}",
                    fact.concept_name(),
                    child_ref.min_occurs,
                    child_ref.name.local_name,
                    count
                ),
                Some(fact.concept_name().to_string()),
                None,
            );
        }

        let has_ambiguous_choice_default =
            child_ref.min_occurs == 0 && matches!(child_ref.max_occurs, MaxOccurs::Bounded(1));

        if !has_ambiguous_choice_default
            && let MaxOccurs::Bounded(max_occurs) = child_ref.max_occurs
            && count > max_occurs
        {
            result.add(
                Severity::Error,
                "schema.tuple_child_not_allowed",
                format!(
                    "Tuple '{}' allows at most {} occurrence(s) of child '{}' but found {}",
                    fact.concept_name(),
                    max_occurs,
                    child_ref.name.local_name,
                    count
                ),
                Some(fact.concept_name().to_string()),
                None,
            );
        }
    }
}

/// Check if a tuple child reference matches a given concept, considering direct
/// matches and substitution group relationships.
fn tuple_child_ref_matches_concept(
    child_ref: &TupleChild,
    child_concept_name: &ExpandedName,
    taxonomy: &TaxonomySet,
) -> bool {
    let Some(child_concept) = taxonomy.find_concept(child_concept_name) else {
        return false;
    };

    let allowed_local = &child_ref.name.local_name;

    // Direct match
    if &child_concept.name.local_name == allowed_local {
        return true;
    }

    // Walk substitution group chain: if the child substitutes for the declared
    // child ref (directly or transitively), it is allowed.
    let mut current = &child_concept.substitution_group.original;
    let mut seen = HashSet::new();

    while seen.insert(current) {
        if &current.local_name == allowed_local {
            return true;
        }
        let Some(parent) = taxonomy.find_concept(current) else {
            break;
        };
        current = &parent.substitution_group.original;
    }

    false
}

fn validate_item_fact(
    fact: &ItemFact,
    instance: &InstanceDocument,
    taxonomy: &TaxonomySet,
    result: &mut ValidationResult,
) {
    let concept_name = fact.concept_name();
    let ctx_ref = fact.context_ref();

    // 1. Context reference must be valid
    let context = instance.get_context(ctx_ref);
    if context.is_none() {
        result.add(
            Severity::Error,
            "spec.invalid_context_ref",
            format!("Fact '{concept_name}' references unknown context '{ctx_ref}'"),
            Some(concept_name.to_string()),
            Some(ctx_ref.to_string()),
        );
    }

    // 2. Concept must exist in the DTS
    let Some(concept) = taxonomy.find_concept(concept_name) else {
        result.add(
            Severity::Error,
            "schema.concept_not_found",
            format!(
                "Fact concept '{}' not found in taxonomy",
                concept_name.local_name
            ),
            Some(concept_name.to_string()),
            Some(ctx_ref.to_string()),
        );
        return;
    };

    // 3. Abstract elements cannot be reported
    if concept.is_abstract {
        result.add(
            Severity::Error,
            "schema.abstract_concept",
            format!(
                "Fact reports abstract concept '{}'",
                concept_name.local_name
            ),
            Some(concept_name.to_string()),
            Some(ctx_ref.to_string()),
        );
    }

    // 4. Nillable check: if fact is nil, element must be nillable
    if fact.is_nil() && !concept.nillable {
        result.add(
            Severity::Error,
            "schema.nil_not_allowed",
            format!(
                "Fact '{}' is nil but element is not nillable",
                concept_name.local_name
            ),
            Some(concept_name.to_string()),
            Some(ctx_ref.to_string()),
        );
    }

    // 5. Numeric facts: unit reference and decimals/precision constraints
    if concept.data_type.is_numeric() {
        if fact.unit_ref().is_none() {
            result.add(
                Severity::Error,
                "spec.numeric_no_unit",
                format!("Numeric fact '{}' has no unitRef", concept_name.local_name),
                Some(concept_name.to_string()),
                Some(ctx_ref.to_string()),
            );
        }

        let has_decimals = fact.decimals().is_some();
        let has_precision = fact.precision().is_some();
        let acc = DeclaredAccuracy::default();
        let has_declared_accuracy = acc.decimals.is_some() || acc.precision.is_some();

        if has_decimals && has_precision {
            result.add(
                Severity::Error,
                "spec.numeric_decimals_precision_mutual_exclusion",
                format!(
                    "Numeric fact '{}' specifies both decimals and precision",
                    concept_name.local_name
                ),
                Some(concept_name.to_string()),
                Some(ctx_ref.to_string()),
            );
        } else if !fact.is_nil() && !has_decimals && !has_precision && !has_declared_accuracy {
            result.add(
                Severity::Error,
                "spec.numeric_missing_accuracy",
                format!(
                    "Numeric fact '{}' must specify either decimals or precision",
                    concept_name.local_name
                ),
                Some(concept_name.to_string()),
                Some(ctx_ref.to_string()),
            );
        }

        if fact.is_nil() && (has_decimals || has_precision || has_declared_accuracy) {
            result.add(
                Severity::Error,
                "spec.nil_fact_has_accuracy",
                format!(
                    "Nil numeric fact '{}' must not specify decimals or precision",
                    concept_name.local_name
                ),
                Some(concept_name.to_string()),
                Some(ctx_ref.to_string()),
            );
        }

        if !fact.is_nil() && !is_valid_numeric_lexical(fact.value()) {
            result.add(
                Severity::Error,
                "schema.invalid_numeric_lexical",
                format!(
                    "Numeric fact '{}' has invalid lexical value '{}'",
                    concept_name.local_name,
                    fact.value()
                ),
                Some(concept_name.to_string()),
                Some(ctx_ref.to_string()),
            );
        }
    }

    // Non-numeric facts must not have a unitRef
    if !concept.data_type.is_numeric() && fact.unit_ref().is_some() {
        result.add(
            Severity::Error,
            "spec.non_numeric_has_unit",
            format!(
                "Non-numeric fact '{}' must not have unitRef",
                concept_name.local_name
            ),
            Some(concept_name.to_string()),
            Some(ctx_ref.to_string()),
        );
    }

    // Unit reference must resolve if present
    if let Some(unit_ref) = fact.unit_ref()
        && instance.get_unit(unit_ref).is_none()
    {
        result.add(
            Severity::Error,
            "spec.invalid_unit_ref",
            format!(
                "Fact '{}' references unknown unit '{}'",
                concept_name.local_name, unit_ref
            ),
            Some(concept_name.to_string()),
            Some(ctx_ref.to_string()),
        );
    }

    // 6. Period type check
    if let (Some(period_type), Some(context)) = (&concept.period_type, context) {
        let period_matches = matches!(
            (period_type, &context.period),
            (PeriodType::Instant, Period::Instant { .. })
                | (PeriodType::Duration, Period::Duration { .. })
        );
        if !period_matches {
            let actual = match &context.period {
                Period::Instant { .. } => "instant",
                Period::Duration { .. } => "duration",
                Period::Forever => "forever",
            };
            result.add(
                Severity::Error,
                "schema.period_type_mismatch",
                format!(
                    "Fact '{}' requires {:?} period but context '{}' has {}",
                    concept_name.local_name, period_type, ctx_ref, actual
                ),
                Some(concept_name.to_string()),
                Some(ctx_ref.to_string()),
            );
        }
    }

    // 7. Unit semantics for specific item types
    if let Some(unit_ref) = fact.unit_ref()
        && let Some(unit) = instance.get_unit(unit_ref)
    {
        validate_unit_constraints(fact, concept, unit, result);
    }
}

/// Validate context structure and period consistency.
///
/// Checks for prohibited XBRL-instance descendants in segment/scenario,
/// disallowed XBRL substitution-group elements inside context content, and
/// invalid duration period ordering (`startDate < endDate`).
fn validate_contexts(
    instance: &InstanceDocument,
    taxonomy: &TaxonomySet,
    result: &mut ValidationResult,
) {
    for (context_id, context) in instance.contexts() {
        if context.entity.scheme.trim().is_empty() {
            result.add(
                Severity::Error,
                "spec.identifier_missing_scheme",
                format!(
                    "Context '{context_id}' entity identifier is missing required @scheme attribute"
                ),
                None,
                Some(context_id.to_string()),
            );
        }

        if context.segment_has_instance_descendant {
            result.add(
                Severity::Error,
                "spec.segment_contains_xbrli",
                format!(
                    "Context '{context_id}' has a segment descendant in the XBRL instance namespace"
                ),
                None,
                Some(context_id.to_string()),
            );
        }

        if context.scenario_has_instance_descendant {
            result.add(
                Severity::Error,
                "spec.scenario_contains_xbrli",
                format!(
                    "Context '{context_id}' has a scenario descendant in the XBRL instance namespace"
                ),
                None,
                Some(context_id.to_string()),
            );
        }

        for segment_element in context
            .segment_elements
            .iter()
            .chain(context.scenario_elements.iter())
        {
            if let Some(concept) = taxonomy.find_concept(segment_element)
                && (concept.is_item() || concept.is_tuple())
            {
                result.add(
                    Severity::Error,
                    "spec.context_contains_xbrl_item",
                    format!(
                        "Context '{context_id}' contains '{}' which is in an XBRL substitution group",
                        segment_element.local_name
                    ),
                    None,
                    Some(context_id.to_string()),
                );
            }
        }

        if let Period::Duration { start, end } = &context.period
            && !period_order_is_valid(start, end)
        {
            result.add(
                Severity::Error,
                "spec.invalid_period_order",
                format!(
                    "Context '{context_id}' has start date '{start}' that is not earlier than end date '{end}'"
                ),
                None,
                Some(context_id.to_string()),
            );
        }
    }
}

fn validate_unit_constraints(
    fact: &ItemFact,
    concept: &Concept,
    unit: &crate::instance::Unit,
    result: &mut ValidationResult,
) {
    let concept_name = fact.concept_name();
    let context_ref = fact.context_ref();

    if !unit.denominator.is_empty() {
        let mut numerator_counts: HashMap<(&str, &str), usize> = HashMap::new();
        for measure in &unit.numerator {
            let key = (measure.namespace_uri.as_str(), measure.local_name.as_str());
            *numerator_counts.entry(key).or_default() += 1;
        }

        for measure in &unit.denominator {
            let key = (measure.namespace_uri.as_str(), measure.local_name.as_str());
            if let Some(count) = numerator_counts.get(&key)
                && *count > 0
            {
                result.add(
                    Severity::Error,
                    "spec.unit_not_simplest_form",
                    format!(
                        "Fact '{}' uses unit '{}' that is not in simplest form (canceling measure '{}')",
                        fact.concept_name(),
                        unit.id,
                        measure
                    ),
                    Some(concept_name.to_string()),
                    Some(context_ref.to_string()),
                );
                break;
            }
        }
    }

    for measure in unit.numerator.iter().chain(unit.denominator.iter()) {
        if measure.namespace_uri.as_str() == NS_XBRLI
            && measure.local_name != "pure"
            && measure.local_name != "shares"
        {
            result.add(
                Severity::Error,
                "spec.invalid_xbrli_measure_local_name",
                format!(
                    "Fact '{}' uses invalid measure '{}' in XBRL instance namespace",
                    fact.concept_name(),
                    measure
                ),
                Some(concept_name.to_string()),
                Some(context_ref.to_string()),
            );
            return;
        }
    }

    let is_monetary = matches!(concept.data_type, XbrlType::Monetary);
    let is_shares = matches!(concept.data_type, XbrlType::Shares);

    if is_monetary {
        if !unit.has_single_measure_no_divide() {
            result.add(
                Severity::Error,
                "spec.invalid_monetary_unit_shape",
                format!(
                    "Monetary fact '{}' must use exactly one non-divide measure",
                    fact.concept_name()
                ),
                Some(concept_name.to_string()),
                Some(context_ref.to_string()),
            );
            return;
        }

        if let Some(measure) = unit.primary_measure() {
            let is_iso = measure.namespace_uri.as_str() == NS_ISO4217;
            let is_code = measure.local_name.len() == 3
                && measure
                    .local_name
                    .chars()
                    .all(|char| char.is_ascii_uppercase());
            if !is_iso || !is_code {
                result.add(
                    Severity::Error,
                    "spec.invalid_monetary_measure",
                    format!(
                        "Monetary fact '{}' must use ISO4217 currency measure, got '{}'",
                        fact.concept_name(),
                        measure
                    ),
                    Some(concept_name.to_string()),
                    Some(context_ref.to_string()),
                );
            }
        }
    }

    if is_shares {
        if !unit.has_single_measure_no_divide() {
            result.add(
                Severity::Error,
                "spec.invalid_shares_unit_shape",
                format!(
                    "Shares fact '{}' must use exactly one non-divide measure",
                    fact.concept_name()
                ),
                Some(concept_name.to_string()),
                Some(context_ref.to_string()),
            );
            return;
        }

        if let Some(measure) = unit.primary_measure()
            && !(measure.namespace_uri.as_str() == NS_XBRLI && measure.local_name == "shares")
        {
            result.add(
                Severity::Error,
                "spec.invalid_shares_measure",
                format!(
                    "Shares fact '{}' must use xbrli:shares measure, got '{}'",
                    fact.concept_name(),
                    measure
                ),
                Some(concept_name.to_string()),
                Some(context_ref.to_string()),
            );
        }
    }
}

fn period_order_is_valid(start: &str, end: &str) -> bool {
    let start = start.trim();
    let end = end.trim();
    !start.is_empty() && !end.is_empty() && start < end
}

fn is_valid_numeric_lexical(value: &str) -> bool {
    let trimmed = value.trim();
    !trimmed.is_empty()
        && (Decimal::from_str(trimmed).is_ok() || Decimal::from_scientific(trimmed).is_ok())
}
