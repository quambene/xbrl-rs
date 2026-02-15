//! Create an empty XBRL instance document from a taxonomy.
//!
//! Walks the presentation linkbase for the balance sheet role and creates
//! nil facts for every non-abstract concept.
//!
//! Usage:
//!     cargo run --example create_empty_instance

use std::collections::HashSet;
use std::path::Path;
use xbrl_rs::{
    Context, EntityIdentifier, EntryPoint, Fact, Period, TaxonomySet, Unit, XbrlValidator,
};

const TAXONOMY_PATH_BASE: &str = "test_data/taxonomies/german-gaap/v6.9";
const TAXONOMY_URL_BASE: &str = "http://www.xbrl.de/taxonomies";

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let taxonomy = TaxonomySet::discover(&[
        EntryPoint::new(
            format!("{TAXONOMY_URL_BASE}/de-gcd-2025-04-01/de-gcd-2025-04-01-shell.xsd"),
            Path::new(TAXONOMY_PATH_BASE).join("de-gcd-2025-04-01/de-gcd-2025-04-01-shell.xsd"),
        ),
        EntryPoint::new(
            format!(
                "{TAXONOMY_URL_BASE}/de-gaap-ci-2025-04-01/de-gaap-ci-2025-04-01-shell-fiscal.xsd"
            ),
            Path::new(TAXONOMY_PATH_BASE)
                .join("de-gaap-ci-2025-04-01/de-gaap-ci-2025-04-01-shell-fiscal.xsd"),
        ),
    ])?;

    let mut instance = taxonomy.create_instance();

    // Contexts
    let entity = EntityIdentifier {
        scheme: "http://www.example.com/id".to_string(),
        value: "1234567890".to_string(),
    };
    instance.add_context(Context::new(
        "I".to_string(),
        entity.clone(),
        Period::Instant {
            date: "2025-12-31".to_string(),
        },
    ));
    instance.add_context(Context::new(
        "D".to_string(),
        entity,
        Period::Duration {
            start: "2025-01-01".to_string(),
            end: "2025-12-31".to_string(),
        },
    ));

    // Currency unit
    instance.add_unit(Unit::new("EUR".to_string(), "iso4217:EUR".to_string()));

    // Walk the balance sheet presentation tree and create nil facts
    let role = "http://www.xbrl.de/taxonomies/de-gaap-ci/role/balanceSheet";
    let arcs = taxonomy.presentation_arcs(role).unwrap_or_default();

    // Collect unique concept IDs from the presentation arcs
    let mut concept_ids = HashSet::new();
    for arc in arcs {
        concept_ids.insert(arc.from.as_str());
        concept_ids.insert(arc.to.as_str());
    }

    let mut fact_count = 0;
    for concept_id in &concept_ids {
        let elem = match taxonomy.find_element_by_id(concept_id) {
            Some(e) => e,
            None => continue,
        };

        // Skip abstract grouping concepts
        if elem.is_abstract {
            continue;
        }

        let concept = match taxonomy.qualified_name(concept_id) {
            Some(name) => name,
            None => continue,
        };

        // Context depends on period type
        let context_ref = match elem.period_type.as_deref() {
            Some("instant") => "I",
            _ => "D",
        };

        // Monetary types need a unit reference
        let is_monetary = elem
            .type_name
            .as_deref()
            .is_some_and(|t| t.contains("monetaryItemType"));
        let unit_ref = if is_monetary { Some("EUR") } else { None };

        let mut fact = Fact::new(
            concept,
            context_ref.to_string(),
            unit_ref.map(String::from),
            String::new(),
        );
        fact.set_nil(true);

        instance.add_fact(fact);
        fact_count += 1;
    }

    println!("Created instance with {fact_count} nil facts for role:");
    println!("  {role}");
    println!();
    println!("Schema refs: {}", instance.schema_refs().join(", "));
    println!("Contexts: {}", instance.contexts().len());
    println!("Units: {}", instance.units().len());
    println!();

    // Validate the created instance
    let result = XbrlValidator::new(&instance, &taxonomy).validate_all();
    if result.is_valid() {
        println!("Validation: PASSED");
    } else {
        println!("Validation errors:");
        for msg in result.errors() {
            println!("  - {}", msg.message);
        }
    }

    Ok(())
}
