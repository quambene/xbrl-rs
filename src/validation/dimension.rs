//! Dimensional validation using the definition linkbase.
//!
//! Checks that dimension members used in instance contexts are declared as
//! valid members in the definition linkbase's domain-member relationships.

use super::{Severity, ValidationResult};
use crate::{ExpandedName, InstanceDocument, TaxonomySet};
use std::collections::{HashMap, HashSet};

/// Well-known XBRL Dimensions arcrole URIs.
const ARCROLE_DOMAIN_MEMBER: &str = "http://xbrl.org/int/dim/arcrole/domain-member";
const ARCROLE_DIMENSION_DOMAIN: &str = "http://xbrl.org/int/dim/arcrole/dimension-domain";

pub(super) fn validate_dimensions(
    instance: &InstanceDocument,
    taxonomy: &TaxonomySet,
    result: &mut ValidationResult,
) {
    let valid_members = build_valid_dimension_members(taxonomy);

    if valid_members.is_empty() {
        return;
    }

    for (context_id, context) in instance.contexts() {
        for (dimension, member) in &context.dimensions {
            if let Some(allowed_members) = valid_members.get(dimension)
                && !allowed_members.contains(member)
            {
                result.add(
                    Severity::Error,
                    "dim.invalid_member",
                    format!(
                        "Context '{context_id}': dimension '{dimension}' has member \
                         '{member}' which is not a valid domain member in the taxonomy"
                    ),
                    None,
                    Some(context_id.to_string()),
                );
            }
        }
    }
}

/// Build a map from dimension element ID to the set of valid member element IDs.
///
/// Traverses `dimension-domain` and `domain-member` arcs to collect the full
/// tree of allowed members for each dimension.
fn build_valid_dimension_members(
    taxonomy: &TaxonomySet,
) -> HashMap<ExpandedName, HashSet<ExpandedName>> {
    let mut dimension_domains: HashMap<ExpandedName, HashSet<ExpandedName>> = HashMap::new();
    let mut domain_members: HashMap<ExpandedName, HashSet<ExpandedName>> = HashMap::new();

    for arcs in taxonomy.definitions().values() {
        for arc in arcs {
            match arc.arcrole.as_str() {
                ARCROLE_DIMENSION_DOMAIN => {
                    dimension_domains
                        .entry(arc.from.clone())
                        .or_default()
                        .insert(arc.to.clone());
                }
                ARCROLE_DOMAIN_MEMBER => {
                    domain_members
                        .entry(arc.from.clone())
                        .or_default()
                        .insert(arc.to.clone());
                }
                _ => {}
            }
        }
    }

    let mut result: HashMap<ExpandedName, HashSet<ExpandedName>> = HashMap::new();

    for (dimension_id, dimension_domains) in &dimension_domains {
        let mut members = HashSet::new();
        let mut queue = dimension_domains.iter().collect::<Vec<_>>();

        while let Some(current) = queue.pop() {
            if !members.insert(current.clone()) {
                continue;
            }
            if let Some(children) = domain_members.get(current) {
                for child in children {
                    queue.push(child);
                }
            }
        }

        result.insert(dimension_id.clone(), members);
    }

    result
}
