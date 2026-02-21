//! Schema validation: checks that facts conform to the DTS element definitions
//! and XBRL specification structural rules.

use super::{Severity, ValidationResult};
use crate::{Fact, Period, TaxonomySet, XbrlInstance, taxonomy::ElementDefinition};

const NS_XBRLI: &str = "http://www.xbrl.org/2003/instance";
const NS_ISO4217: &str = "http://www.xbrl.org/2003/iso4217";

/// Run all schema-level validation checks.
pub(super) fn validate_schema(
    instance: &XbrlInstance,
    taxonomy: &TaxonomySet,
    result: &mut ValidationResult,
) {
    validate_contexts(instance, taxonomy, result);

    for fact in instance.facts() {
        validate_fact(fact, instance, taxonomy, result);
    }
}

fn validate_fact(
    fact: &Fact,
    instance: &XbrlInstance,
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
    if is_numeric_type(element) {
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

        if has_decimals && has_precision {
            result.add(
                Severity::Error,
                "spec.numeric_decimals_precision_mutual_exclusion",
                format!("Numeric fact '{local_name}' specifies both decimals and precision"),
                Some(concept),
                Some(ctx_ref),
            );
        } else if !fact.is_nil() && !has_decimals && !has_precision {
            result.add(
                Severity::Error,
                "spec.numeric_missing_accuracy",
                format!("Numeric fact '{local_name}' must specify either decimals or precision"),
                Some(concept),
                Some(ctx_ref),
            );
        }

        if fact.is_nil() && (has_decimals || has_precision) {
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
            (period_type.as_str(), &context.period),
            ("instant", Period::Instant { .. }) | ("duration", Period::Duration { .. })
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
        validate_unit_constraints(fact, element, unit, result);
    }
}

fn validate_contexts(
    instance: &XbrlInstance,
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
    result: &mut ValidationResult,
) {
    let concept = fact.concept();
    let ctx_ref = fact.context_ref();

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

    let type_name = element.type_name.as_deref().unwrap_or("").to_lowercase();

    if type_name.contains("monetary") {
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

    if type_name.contains("shares") {
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
fn is_numeric_type(element: &ElementDefinition) -> bool {
    let Some(ref type_name) = element.type_name else {
        return false;
    };
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
