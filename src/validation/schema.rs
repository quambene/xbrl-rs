//! Schema validation: checks that facts conform to the DTS element definitions
//! and XBRL specification structural rules.

use super::{Severity, ValidationResult};
use crate::{Fact, Period, TaxonomySet, XbrlInstance, taxonomy::ElementDefinition};

/// Run all schema-level validation checks.
pub(super) fn validate_schema(
    instance: &XbrlInstance,
    taxonomy: &TaxonomySet,
    result: &mut ValidationResult,
) {
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

    // 5. Numeric facts: unit reference and decimals
    if is_numeric_type(element) && !fact.is_nil() {
        if fact.unit_ref().is_none() {
            result.add(
                Severity::Error,
                "spec.numeric_no_unit",
                format!("Numeric fact '{local_name}' has no unitRef"),
                Some(concept),
                Some(ctx_ref),
            );
        }

        if fact.decimals().is_none() {
            result.add(
                Severity::Error,
                "spec.numeric_no_decimals",
                format!("Numeric fact '{local_name}' has no decimals attribute"),
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
