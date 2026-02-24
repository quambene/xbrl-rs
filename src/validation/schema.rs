//! Schema validation: checks that facts conform to the DTS element definitions
//! and XBRL specification structural rules.

use super::{Severity, ValidationResult};
use crate::{
    Fact, InstanceDocument, Period, TaxonomySet,
    taxonomy::{ElementDefinition, PeriodType},
};
use std::collections::{HashMap, HashSet};

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
    result: &mut ValidationResult,
) {
    validate_contexts(instance, taxonomy, result);
    validate_footnotes(instance, result);
    validate_instance_refs(instance, result);
    validate_essence_alias_units(instance, taxonomy, result);

    for fact in instance.facts() {
        validate_fact(fact, instance, taxonomy, result);
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

    let mut facts_by_element_id: HashMap<String, Vec<&Fact>> = HashMap::new();
    for fact in instance.facts() {
        if let Some(element) = taxonomy.find_element(fact.local_name())
            && let Some(id) = element.id.as_ref()
        {
            facts_by_element_id
                .entry(id.clone())
                .or_default()
                .push(fact);
        }
    }

    for arcs in taxonomy.definitions().values() {
        for arc in arcs {
            if arc.arcrole != ARCROLE_ESSENCE_ALIAS {
                continue;
            }

            let Some(essence_facts) = facts_by_element_id.get(&arc.from) else {
                continue;
            };
            let Some(alias_facts) = facts_by_element_id.get(&arc.to) else {
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
                                essence_fact.concept(),
                                alias_fact.concept(),
                                essence_fact.context_ref()
                            ),
                            Some(essence_fact.concept()),
                            Some(essence_fact.context_ref()),
                        );
                    }
                }
            }
        }
    }
}

fn units_semantically_equal(left: &crate::instance::Unit, right: &crate::instance::Unit) -> bool {
    let mut left_num: Vec<(Option<&str>, &str)> = left
        .numerator_measures
        .iter()
        .map(|measure| {
            (
                measure.namespace_uri.as_deref(),
                measure.local_name.as_str(),
            )
        })
        .collect();
    let mut right_num: Vec<(Option<&str>, &str)> = right
        .numerator_measures
        .iter()
        .map(|measure| {
            (
                measure.namespace_uri.as_deref(),
                measure.local_name.as_str(),
            )
        })
        .collect();
    left_num.sort();
    right_num.sort();

    let mut left_den: Vec<(Option<&str>, &str)> = left
        .denominator_measures
        .iter()
        .map(|measure| {
            (
                measure.namespace_uri.as_deref(),
                measure.local_name.as_str(),
            )
        })
        .collect();
    let mut right_den: Vec<(Option<&str>, &str)> = right
        .denominator_measures
        .iter()
        .map(|measure| {
            (
                measure.namespace_uri.as_deref(),
                measure.local_name.as_str(),
            )
        })
        .collect();
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
    let fact_ids: HashSet<&str> = instance.facts().iter().filter_map(Fact::id).collect();

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
                if let Some((file_part, target)) = href_target_id(href) {
                    if let (Some(file_part), Some(document_name)) = (
                        file_part.filter(|s| !s.is_empty()),
                        instance.document_name(),
                    ) && file_part != document_name
                    {
                        result.add(
                            Severity::Error,
                            "spec.footnote_href_out_of_scope",
                            format!(
                                "Footnote locator href '{}' points to another document '{}'; expected '{}'",
                                href, file_part, document_name
                            ),
                            None,
                            None,
                        );
                    }

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

            if resource.xml_lang.is_none()
                && footnote_link.xml_lang.is_none()
                && instance.root_xml_lang().is_none()
            {
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

fn validate_fact(
    fact: &Fact,
    instance: &InstanceDocument,
    taxonomy: &TaxonomySet,
    result: &mut ValidationResult,
) {
    let local_name = fact.local_name();
    let concept = fact.concept();
    let ctx_ref = fact.context_ref();

    // 1. Context reference must be valid
    let context = instance.get_context(ctx_ref);
    if context.is_none() {
        result.add(
            Severity::Error,
            "spec.invalid_context_ref",
            format!("Fact '{concept}' references unknown context '{ctx_ref}'"),
            Some(concept),
            Some(ctx_ref),
        );
    }

    // 2. Concept must exist in the DTS
    let Some(element) = taxonomy.find_element(local_name) else {
        result.add(
            Severity::Error,
            "schema.concept_not_found",
            format!("Fact concept '{local_name}' not found in taxonomy"),
            Some(concept),
            Some(ctx_ref),
        );
        return;
    };

    // 3. Abstract elements cannot be reported
    if element.is_abstract {
        result.add(
            Severity::Error,
            "schema.abstract_concept",
            format!("Fact reports abstract concept '{local_name}'"),
            Some(concept),
            Some(ctx_ref),
        );
    }

    // 4. Nillable check — if fact is nil, element must be nillable
    if fact.is_nil() && !element.nillable {
        result.add(
            Severity::Error,
            "schema.nil_not_allowed",
            format!("Fact '{local_name}' is nil but element is not nillable"),
            Some(concept),
            Some(ctx_ref),
        );
    }

    // 5. Numeric facts: unit reference and decimals/precision constraints
    if is_numeric_type(element, taxonomy) {
        if fact.unit_ref().is_none() {
            result.add(
                Severity::Error,
                "spec.numeric_no_unit",
                format!("Numeric fact '{local_name}' has no unitRef"),
                Some(concept),
                Some(ctx_ref),
            );
        }

        let has_decimals = fact.decimals().is_some();
        let has_precision = fact.precision().is_some();
        let has_declared_accuracy = element.type_name.as_deref().is_some_and(|type_name| {
            let acc = taxonomy.type_declared_accuracy(type_name);
            acc.decimals.is_some() || acc.precision.is_some()
        });

        if has_decimals && has_precision {
            result.add(
                Severity::Error,
                "spec.numeric_decimals_precision_mutual_exclusion",
                format!("Numeric fact '{local_name}' specifies both decimals and precision"),
                Some(concept),
                Some(ctx_ref),
            );
        } else if !fact.is_nil() && !has_decimals && !has_precision && !has_declared_accuracy {
            result.add(
                Severity::Error,
                "spec.numeric_missing_accuracy",
                format!("Numeric fact '{local_name}' must specify either decimals or precision"),
                Some(concept),
                Some(ctx_ref),
            );
        }

        if fact.is_nil() && (has_decimals || has_precision || has_declared_accuracy) {
            result.add(
                Severity::Error,
                "spec.nil_fact_has_accuracy",
                format!("Nil numeric fact '{local_name}' must not specify decimals or precision"),
                Some(concept),
                Some(ctx_ref),
            );
        }
    }

    // Unit reference must resolve if present
    if let Some(unit_ref) = fact.unit_ref()
        && instance.get_unit(unit_ref).is_none()
    {
        result.add(
            Severity::Error,
            "spec.invalid_unit_ref",
            format!("Fact '{local_name}' references unknown unit '{unit_ref}'"),
            Some(concept),
            Some(ctx_ref),
        );
    }

    // 6. Period type check
    if let (Some(period_type), Some(context)) = (&element.period_type, context) {
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
                    "Fact '{local_name}' requires {period_type} period but context '{ctx_ref}' has {actual}",
                ),
                Some(concept),
                Some(ctx_ref),
            );
        }
    }

    // 7. Unit semantics for specific item types
    if let Some(unit_ref) = fact.unit_ref()
        && let Some(unit) = instance.get_unit(unit_ref)
    {
        validate_unit_constraints(fact, element, unit, taxonomy, result);
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
    for (ctx_id, context) in instance.contexts() {
        if context.segment_has_instance_descendant {
            result.add(
                Severity::Error,
                "spec.segment_contains_xbrli",
                format!(
                    "Context '{ctx_id}' has a segment descendant in the XBRL instance namespace"
                ),
                None,
                Some(ctx_id),
            );
        }

        if context.scenario_has_instance_descendant {
            result.add(
                Severity::Error,
                "spec.scenario_contains_xbrli",
                format!(
                    "Context '{ctx_id}' has a scenario descendant in the XBRL instance namespace"
                ),
                None,
                Some(ctx_id),
            );
        }

        for qname in context
            .segment_elements
            .iter()
            .chain(context.scenario_elements.iter())
        {
            let local = qname.rsplit(':').next().unwrap_or(qname);
            if let Some(element) = taxonomy.find_element(local)
                && element
                    .substitution_group
                    .as_deref()
                    .is_some_and(|sg| sg.contains("xbrli:item") || sg.contains("xbrli:tuple"))
            {
                result.add(
                    Severity::Error,
                    "spec.context_contains_xbrl_item",
                    format!(
                        "Context '{ctx_id}' contains '{qname}' which is in an XBRL substitution group"
                    ),
                    None,
                    Some(ctx_id),
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
                    "Context '{ctx_id}' has start date '{start}' that is not earlier than end date '{end}'"
                ),
                None,
                Some(ctx_id),
            );
        }
    }
}

fn validate_unit_constraints(
    fact: &Fact,
    element: &ElementDefinition,
    unit: &crate::instance::Unit,
    taxonomy: &TaxonomySet,
    result: &mut ValidationResult,
) {
    let concept = fact.concept();
    let ctx_ref = fact.context_ref();

    if !unit.denominator_measures.is_empty() {
        let mut numerator_counts: HashMap<(Option<&str>, &str), usize> = HashMap::new();
        for measure in &unit.numerator_measures {
            let key = (
                measure.namespace_uri.as_deref(),
                measure.local_name.as_str(),
            );
            *numerator_counts.entry(key).or_default() += 1;
        }

        for measure in &unit.denominator_measures {
            let key = (
                measure.namespace_uri.as_deref(),
                measure.local_name.as_str(),
            );
            if let Some(count) = numerator_counts.get(&key)
                && *count > 0
            {
                result.add(
                    Severity::Error,
                    "spec.unit_not_simplest_form",
                    format!(
                        "Fact '{}' uses unit '{}' that is not in simplest form (canceling measure '{}')",
                        fact.local_name(),
                        unit.id,
                        measure.qname
                    ),
                    Some(concept),
                    Some(ctx_ref),
                );
                break;
            }
        }
    }

    for measure in unit
        .numerator_measures
        .iter()
        .chain(unit.denominator_measures.iter())
    {
        if measure.namespace_uri.as_deref() == Some(NS_XBRLI)
            && measure.local_name != "pure"
            && measure.local_name != "shares"
        {
            result.add(
                Severity::Error,
                "spec.invalid_xbrli_measure_local_name",
                format!(
                    "Fact '{}' uses invalid measure '{}' in XBRL instance namespace",
                    fact.local_name(),
                    measure.qname
                ),
                Some(concept),
                Some(ctx_ref),
            );
            return;
        }
    }

    let is_monetary = element
        .type_name
        .as_deref()
        .is_some_and(|t| taxonomy.is_type_derived_from(t, "monetaryItemType"));
    let is_shares = element
        .type_name
        .as_deref()
        .is_some_and(|t| taxonomy.is_type_derived_from(t, "sharesItemType"));

    if is_monetary {
        if !unit.has_single_measure_no_divide() {
            result.add(
                Severity::Error,
                "spec.invalid_monetary_unit_shape",
                format!(
                    "Monetary fact '{}' must use exactly one non-divide measure",
                    fact.local_name()
                ),
                Some(concept),
                Some(ctx_ref),
            );
            return;
        }

        if let Some(measure) = unit.primary_measure() {
            let is_iso = measure.namespace_uri.as_deref() == Some(NS_ISO4217);
            let is_code = measure.local_name.len() == 3
                && measure.local_name.chars().all(|ch| ch.is_ascii_uppercase());
            if !is_iso || !is_code {
                result.add(
                    Severity::Error,
                    "spec.invalid_monetary_measure",
                    format!(
                        "Monetary fact '{}' must use ISO4217 currency measure, got '{}'",
                        fact.local_name(),
                        measure.qname
                    ),
                    Some(concept),
                    Some(ctx_ref),
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
                    fact.local_name()
                ),
                Some(concept),
                Some(ctx_ref),
            );
            return;
        }

        if let Some(measure) = unit.primary_measure()
            && !(measure.namespace_uri.as_deref() == Some(NS_XBRLI)
                && measure.local_name == "shares")
        {
            result.add(
                Severity::Error,
                "spec.invalid_shares_measure",
                format!(
                    "Shares fact '{}' must use xbrli:shares measure, got '{}'",
                    fact.local_name(),
                    measure.qname
                ),
                Some(concept),
                Some(ctx_ref),
            );
        }
    }
}

fn period_order_is_valid(start: &str, end: &str) -> bool {
    let s = start.trim();
    let e = end.trim();
    !s.is_empty() && !e.is_empty() && s < e
}

/// Determine whether an element definition is numeric based on its XSD type name.
fn is_numeric_type(element: &ElementDefinition, taxonomy: &TaxonomySet) -> bool {
    let Some(ref type_name) = element.type_name else {
        return false;
    };

    for base in [
        "monetaryItemType",
        "decimalItemType",
        "floatItemType",
        "doubleItemType",
        "integerItemType",
        "sharesItemType",
        "pureItemType",
        "fractionItemType",
    ] {
        if taxonomy.is_type_derived_from(type_name, base) {
            return true;
        }
    }

    let t = type_name.to_lowercase();
    t.contains("monetary")
        || t.contains("decimal")
        || t.contains("float")
        || t.contains("double")
        || t.contains("integer")
        || t.contains("shares")
        || t.contains("pure")
        || t.contains("percent")
        || t.contains("pershare")
}
