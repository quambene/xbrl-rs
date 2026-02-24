//! Dimensional validation using the definition linkbase.
//!
//! Checks that dimension members used in instance contexts are declared as
//! valid members in the definition linkbase's domain-member relationships.

use super::{Severity, ValidationResult};
use crate::{InstanceDocument, TaxonomySet};
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

    for (ctx_id, context) in instance.contexts() {
        for (dimension, member) in &context.dimensions {
            let dim_id = qname_to_element_id(dimension);
            let member_id = qname_to_element_id(member);

            if let Some(allowed_members) = valid_members.get(&dim_id)
                && !allowed_members.contains(&member_id)
            {
                result.add(
                    Severity::Error,
                    "dim.invalid_member",
                    format!(
                        "Context '{ctx_id}': dimension '{dimension}' has member \
                         '{member}' which is not a valid domain member in the taxonomy"
                    ),
                    None,
                    Some(ctx_id),
                );
            }
        }
    }
}

/// Build a map from dimension element ID to the set of valid member element IDs.
///
/// Traverses `dimension-domain` and `domain-member` arcs to collect the full
/// tree of allowed members for each dimension.
fn build_valid_dimension_members(taxonomy: &TaxonomySet) -> HashMap<String, HashSet<String>> {
    let mut dim_domains: HashMap<String, HashSet<String>> = HashMap::new();
    let mut domain_members: HashMap<String, HashSet<String>> = HashMap::new();

    for arcs in taxonomy.definitions().values() {
        for arc in arcs {
            match arc.arcrole.as_str() {
                ARCROLE_DIMENSION_DOMAIN => {
                    dim_domains
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

    let mut result: HashMap<String, HashSet<String>> = HashMap::new();

    for (dim_id, domains) in &dim_domains {
        let mut members = HashSet::new();
        let mut queue: Vec<&str> = domains.iter().map(|s| s.as_str()).collect();

        while let Some(current) = queue.pop() {
            if !members.insert(current.to_string()) {
                continue;
            }
            if let Some(children) = domain_members.get(current) {
                for child in children {
                    queue.push(child);
                }
            }
        }

        result.insert(dim_id.clone(), members);
    }

    result
}

/// Convert a QName like "prefix:local.name" to an element ID like "prefix_local.name".
fn qname_to_element_id(qname: &str) -> String {
    if let Some((prefix, local)) = qname.split_once(':') {
        format!("{prefix}_{local}")
    } else {
        qname.to_string()
    }
}
