//! Calculation linkbase validation.
//!
//! For each summation-item relationship, checks whether the reported parent
//! value equals the weighted sum of reported children values, within a
//! rounding tolerance derived from the `decimals` attribute.

use super::{Severity, ValidationResult};
use crate::{Context, Fact, Period, TaxonomySet, Unit, XbrlInstance};
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

                let tolerance = rounding_tolerance(
                    parent_fact.value(),
                    parent_fact.decimals(),
                    parent_fact.precision(),
                );
                let diff = (parent_value - weighted_sum).abs();

                if diff > tolerance {
                    result.add(
                        Severity::Error,
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
type CtxUnitKey = (ContextKey, UnitKey);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ContextKey {
    entity_scheme: String,
    entity_value: String,
    period: PeriodKey,
    dimensions: Vec<(String, String)>,
    segment_elements: Vec<String>,
    scenario_elements: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum PeriodKey {
    Instant(String),
    Duration(String, String),
    Forever,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct UnitKey {
    numerator: Vec<(Option<String>, String)>,
    denominator: Vec<(Option<String>, String)>,
}

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
            && let Some(context) = instance.get_context(fact.context_ref())
        {
            let Some(key) = fact_semantic_key(instance, fact, context) else {
                continue;
            };
            index.entry(id.clone()).or_default().insert(key, fact);
        }
    }

    index
}

fn fact_semantic_key(
    instance: &XbrlInstance,
    fact: &Fact,
    context: &Context,
) -> Option<CtxUnitKey> {
    let context_key = context_key(context);

    let unit_key = if let Some(unit_ref) = fact.unit_ref() {
        let unit = instance.get_unit(unit_ref)?;
        unit_key(unit)
    } else {
        UnitKey {
            numerator: Vec::new(),
            denominator: Vec::new(),
        }
    };

    Some((context_key, unit_key))
}

fn context_key(context: &Context) -> ContextKey {
    let period = match &context.period {
        Period::Instant { date } => PeriodKey::Instant(date.trim().to_string()),
        Period::Duration { start, end } => {
            PeriodKey::Duration(start.trim().to_string(), end.trim().to_string())
        }
        Period::Forever => PeriodKey::Forever,
    };

    let mut dimensions: Vec<(String, String)> = context
        .dimensions
        .iter()
        .map(|(dimension, member)| (dimension.clone(), member.clone()))
        .collect();
    dimensions.sort();

    ContextKey {
        entity_scheme: context.entity.scheme.trim().to_string(),
        entity_value: context.entity.value.trim().to_string(),
        period,
        dimensions,
        segment_elements: context.segment_elements.clone(),
        scenario_elements: context.scenario_elements.clone(),
    }
}

fn unit_key(unit: &Unit) -> UnitKey {
    let mut numerator: Vec<(Option<String>, String)> = unit
        .numerator_measures
        .iter()
        .map(|measure| {
            (
                measure.namespace_uri.clone(),
                measure.local_name.to_ascii_lowercase(),
            )
        })
        .collect();
    numerator.sort();

    let mut denominator: Vec<(Option<String>, String)> = unit
        .denominator_measures
        .iter()
        .map(|measure| {
            (
                measure.namespace_uri.clone(),
                measure.local_name.to_ascii_lowercase(),
            )
        })
        .collect();
    denominator.sort();

    UnitKey {
        numerator,
        denominator,
    }
}

/// Parse a string as f64, returning None for empty or non-numeric values.
fn parse_numeric(value: &str) -> Option<f64> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    trimmed.parse::<f64>().ok()
}

/// Compute rounding tolerance from `decimals` or inferred from `precision`.
///
/// - `decimals="2"` → tolerance 0.005
/// - `decimals="0"` → tolerance 0.5
/// - `decimals="-3"` → tolerance 500
/// - `decimals="INF"` → tolerance 0 (exact)
fn rounding_tolerance(value: &str, decimals: Option<&str>, precision: Option<&str>) -> f64 {
    if let Some(dec_str) = decimals {
        if dec_str == "INF" {
            return 0.0;
        }
        return match dec_str.parse::<i32>() {
            Ok(d) => 0.5 * 10.0_f64.powi(-d),
            Err(_) => 1.0,
        };
    }

    if let Some(prec_str) = precision {
        if prec_str == "INF" {
            return 0.0;
        }

        if let (Ok(p), Some(v)) = (prec_str.parse::<i32>(), parse_numeric(value)) {
            if v == 0.0 {
                return 0.0;
            }
            let magnitude = v.abs().log10().floor() as i32;
            let inferred_decimals = p - magnitude - 1;
            return 0.5 * 10.0_f64.powi(-inferred_decimals);
        }
    }

    1.0
}
