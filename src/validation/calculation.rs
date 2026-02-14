//! Calculation linkbase validation.
//!
//! For each summation-item relationship, checks whether the reported parent
//! value equals the weighted sum of reported children values, within a
//! rounding tolerance derived from the `decimals` attribute.

use super::{Severity, ValidationResult};
use crate::{Fact, TaxonomySet, XbrlInstance};
use std::collections::HashMap;

/// Run calculation consistency checks for all roles.
pub(super) fn validate_calculations(
    instance: &XbrlInstance,
    taxonomy: &TaxonomySet,
    result: &mut ValidationResult,
) {
    let fact_index = build_fact_index(instance, taxonomy);

    for (role, arcs) in taxonomy.calculations() {
        // Group arcs by parent (from) to collect all children
        let mut parent_children: HashMap<&str, Vec<(&str, f64)>> = HashMap::new();
        for arc in arcs {
            parent_children
                .entry(&arc.from)
                .or_default()
                .push((&arc.to, arc.weight));
        }

        for (parent_id, children) in &parent_children {
            let Some(parent_facts) = fact_index.get(*parent_id) else {
                continue;
            };

            for (ctx_unit_key, parent_fact) in parent_facts {
                let Some(parent_value) = parse_numeric(parent_fact.value()) else {
                    continue;
                };

                let mut weighted_sum = 0.0;
                let mut any_child_found = false;

                for (child_id, weight) in children {
                    if let Some(child_facts) = fact_index.get(*child_id)
                        && let Some(child_fact) = child_facts.get(ctx_unit_key)
                        && let Some(child_value) = parse_numeric(child_fact.value())
                    {
                        weighted_sum += weight * child_value;
                        any_child_found = true;
                    }
                }

                if !any_child_found {
                    continue;
                }

                let tolerance = rounding_tolerance(parent_fact.decimals());
                let diff = (parent_value - weighted_sum).abs();

                if diff > tolerance {
                    result.add(
                        Severity::Warning,
                        "calc.summation_inconsistency",
                        format!(
                            "Calculation inconsistency in role '{role}': '{parent_id}' \
                             reported {parent_value} but children sum to {weighted_sum} \
                             (diff={diff:.2}, tolerance={tolerance:.2})",
                        ),
                        Some(parent_fact.concept()),
                        Some(parent_fact.context_ref()),
                    );
                }
            }
        }
    }
}

/// Key for grouping facts: (context_ref, unit_ref or "").
type CtxUnitKey = (String, String);

/// Build an index: element_id -> { (ctx, unit) -> &Fact }.
///
/// Only includes non-nil facts whose local_name maps to a known element with an id.
fn build_fact_index<'a>(
    instance: &'a XbrlInstance,
    taxonomy: &TaxonomySet,
) -> HashMap<String, HashMap<CtxUnitKey, &'a Fact>> {
    let mut index: HashMap<String, HashMap<CtxUnitKey, &'a Fact>> = HashMap::new();

    for fact in instance.facts() {
        if fact.is_nil() {
            continue;
        }
        let local_name = fact.local_name();
        if let Some(element) = taxonomy.find_element(local_name)
            && let Some(ref id) = element.id
        {
            let key = (
                fact.context_ref().to_string(),
                fact.unit_ref().unwrap_or("").to_string(),
            );
            index.entry(id.clone()).or_default().insert(key, fact);
        }
    }

    index
}

/// Parse a string as f64, returning None for empty or non-numeric values.
fn parse_numeric(value: &str) -> Option<f64> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    trimmed.parse::<f64>().ok()
}

/// Compute rounding tolerance from the `decimals` attribute.
///
/// - `decimals="2"` → tolerance 0.005
/// - `decimals="0"` → tolerance 0.5
/// - `decimals="-3"` → tolerance 500
/// - `decimals="INF"` → tolerance 0 (exact)
fn rounding_tolerance(decimals: Option<&str>) -> f64 {
    let Some(dec_str) = decimals else {
        return 1.0;
    };
    if dec_str == "INF" {
        return 0.0;
    }
    match dec_str.parse::<i32>() {
        Ok(d) => 0.5 * 10.0_f64.powi(-d),
        Err(_) => 1.0,
    }
}
