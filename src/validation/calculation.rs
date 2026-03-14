//! Calculation linkbase validation.
//!
//! For each summation-item relationship, checks whether the reported parent
//! value equals the weighted sum of reported children values, within a
//! rounding tolerance derived from the `decimals` attribute.

use super::{Severity, ValidationResult, value::PreparedFactValues};
use crate::{
    Context, Decimals, DeclaredAccuracy, InstanceDocument, ItemFact, Period, TaxonomySet, Unit,
};
use rust_decimal::{Decimal, RoundingStrategy};
use std::{collections::HashMap, str::FromStr};

/// Run calculation consistency checks for all roles.
pub(super) fn validate_calculations(
    instance: &InstanceDocument,
    taxonomy: &TaxonomySet,
    prepared: &PreparedFactValues,
    result: &mut ValidationResult,
) {
    let fact_index = build_fact_index(instance, taxonomy);

    for (role, arcs) in taxonomy.calculations() {
        // Group arcs by parent (from) to collect all children
        let mut parent_children: HashMap<&str, Vec<(&str, Decimal)>> = HashMap::new();
        for arc in arcs {
            let Ok(weight) = Decimal::from_str(&arc.weight.to_string()) else {
                continue;
            };
            parent_children
                .entry(&arc.from)
                .or_default()
                .push((&arc.to, weight));
        }

        for (parent_id, children) in &parent_children {
            let Some(parent_facts) = fact_index.get(*parent_id) else {
                continue;
            };

            for (ctx_unit_key, parent_fact_entry) in parent_facts {
                if parent_fact_entry.is_duplicate {
                    continue;
                }

                let parent_fact = parent_fact_entry.fact;
                let Some(parent_value) = prepared.numeric_value(parent_fact) else {
                    continue;
                };

                let parent_acc = effective_accuracy(parent_fact);
                let parent_effective_value = apply_effective_accuracy(
                    parent_fact.value(),
                    parent_value,
                    parent_acc.decimals.as_ref(),
                    parent_acc.precision.as_ref(),
                );

                let mut weighted_sum = Decimal::ZERO;
                let mut any_child_found = false;
                let mut duplicate_child_found = false;

                for (child_id, weight) in children {
                    if let Some(child_facts) = fact_index.get(*child_id)
                        && let Some(child_fact_entry) = child_facts.get(ctx_unit_key)
                    {
                        if child_fact_entry.is_duplicate {
                            duplicate_child_found = true;
                            break;
                        }

                        let child_fact = child_fact_entry.fact;
                        let Some(child_value) = prepared.numeric_value(child_fact) else {
                            continue;
                        };

                        let child_acc = effective_accuracy(child_fact);
                        let child_effective_value = apply_effective_accuracy(
                            child_fact.value(),
                            child_value,
                            child_acc.decimals.as_ref(),
                            child_acc.precision.as_ref(),
                        );

                        weighted_sum += weight * child_effective_value;
                        any_child_found = true;
                    }
                }

                if duplicate_child_found || !any_child_found {
                    continue;
                }

                let weighted_sum_effective = apply_effective_accuracy(
                    &weighted_sum.to_string(),
                    weighted_sum,
                    parent_acc.decimals.as_ref(),
                    parent_acc.precision.as_ref(),
                );
                let diff = (parent_effective_value - weighted_sum_effective).abs();

                if diff > Decimal::new(1, 9) {
                    result.add(
                        Severity::Error,
                        "calc.summation_inconsistency",
                        format!(
                            "Calculation inconsistency in role '{role}': '{parent_id}' \
                             reported effective value {parent_effective_value} but children sum to {weighted_sum_effective}",
                        ),
                        Some(parent_fact.concept_name()),
                        Some(parent_fact.context_ref()),
                    );
                }
            }
        }
    }
}

// TODO: determine `DeclaredAccuracy`
fn effective_accuracy(fact: &ItemFact) -> DeclaredAccuracy {
    let decimals = fact.decimals().cloned();
    let precision = fact.precision().cloned();
    if decimals.is_some() || precision.is_some() {
        return DeclaredAccuracy {
            decimals,
            precision,
        };
    }

    DeclaredAccuracy::default()
}

/// Key for grouping facts: (context_ref, unit_ref or "").
type CtxUnitKey = (ContextKey, UnitKey);

#[derive(Debug, Clone, Copy)]
struct IndexedFact<'a> {
    fact: &'a ItemFact,
    is_duplicate: bool,
}

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
    instance: &'a InstanceDocument,
    taxonomy: &TaxonomySet,
) -> HashMap<String, HashMap<CtxUnitKey, IndexedFact<'a>>> {
    let mut index: HashMap<String, HashMap<CtxUnitKey, IndexedFact<'a>>> = HashMap::new();

    for fact in instance.item_facts() {
        if fact.is_nil() {
            continue;
        }
        let local_name = fact.local_name();
        if let Some(element) = taxonomy.find_concept(local_name)
            && let Some(context) = instance.get_context(fact.context_ref())
        {
            let Some(key) = fact_semantic_key(instance, fact, context) else {
                continue;
            };
            let element_index = index
                .entry(element.id.clone().unwrap_or_default().to_string())
                .or_default();
            if let Some(existing) = element_index.get_mut(&key) {
                existing.is_duplicate = true;
            } else {
                element_index.insert(
                    key,
                    IndexedFact {
                        fact,
                        is_duplicate: false,
                    },
                );
            }
        }
    }

    index
}

fn fact_semantic_key(
    instance: &InstanceDocument,
    fact: &ItemFact,
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
                measure.qname.local_name.to_ascii_lowercase(),
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
                measure.qname.local_name.to_ascii_lowercase(),
            )
        })
        .collect();
    denominator.sort();

    UnitKey {
        numerator,
        denominator,
    }
}

/// Compute rounding tolerance from `decimals` or inferred from `precision`.
///
/// - `decimals="2"` → tolerance 0.005
/// - `decimals="0"` → tolerance 0.5
/// - `decimals="-3"` → tolerance 500
/// - `decimals="INF"` → tolerance 0 (exact)
fn apply_effective_accuracy(
    raw_value: &str,
    parsed_value: Decimal,
    decimals: Option<&Decimals>,
    precision: Option<&Decimals>,
) -> Decimal {
    if let Some(dec) = decimals {
        return match dec {
            Decimals::Infinite => parsed_value,
            Decimals::Finite(n) => round_to_decimals_ties_even(parsed_value, *n),
        };
    }

    if let Some(prec) = precision {
        return match prec {
            Decimals::Infinite => parsed_value,
            Decimals::Finite(n) => {
                let inferred_decimals = infer_decimals_from_precision(parsed_value, *n);
                round_to_decimals_ties_even(parsed_value, inferred_decimals)
            }
        };
    }

    if let Ok(parsed_again) = Decimal::from_str(raw_value.trim()) {
        return parsed_again;
    }

    if let Ok(parsed_again) = Decimal::from_scientific(raw_value.trim()) {
        return parsed_again;
    }

    parsed_value
}

fn infer_decimals_from_precision(value: Decimal, precision: i32) -> i32 {
    if value.is_zero() {
        precision - 1
    } else {
        let normalized = value.abs().normalize().to_string();
        let unsigned = normalized.strip_prefix('-').unwrap_or(&normalized);
        let (integer_part, fraction_part) = unsigned.split_once('.').unwrap_or((unsigned, ""));

        let integer_no_leading = integer_part.trim_start_matches('0');
        let magnitude = if !integer_no_leading.is_empty() {
            integer_no_leading.len() as i32 - 1
        } else {
            let leading_zeros = fraction_part.chars().take_while(|ch| *ch == '0').count() as i32;
            -(leading_zeros + 1)
        };

        precision - magnitude - 1
    }
}

fn round_to_decimals_ties_even(value: Decimal, decimals: i32) -> Decimal {
    if decimals >= 0 {
        return value
            .round_dp_with_strategy(decimals as u32, RoundingStrategy::MidpointNearestEven);
    }

    let shift = decimal_power_of_ten((-decimals) as u32);
    (value / shift).round_dp_with_strategy(0, RoundingStrategy::MidpointNearestEven) * shift
}

fn decimal_power_of_ten(exp: u32) -> Decimal {
    let mut value = Decimal::ONE;
    for _ in 0..exp {
        value *= Decimal::TEN;
    }
    value
}
