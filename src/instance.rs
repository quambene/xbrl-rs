//! XBRL instance document representation

use crate::{Context, Fact, Unit};
use std::collections::HashMap;

/// Represents a complete XBRL instance document
#[derive(Debug)]
pub struct XbrlInstance {
    /// All contexts in the instance
    contexts: HashMap<String, Context>,
    /// All units in the instance
    units: HashMap<String, Unit>,
    /// All facts in the instance
    facts: Vec<Fact>,
    /// Namespace prefixes used in the document
    namespaces: HashMap<String, String>,
}

impl Default for XbrlInstance {
    fn default() -> Self {
        Self::new()
    }
}

impl XbrlInstance {
    pub fn new() -> Self {
        Self {
            contexts: HashMap::new(),
            units: HashMap::new(),
            facts: Vec::new(),
            namespaces: HashMap::new(),
        }
    }

    /// Add a context to the instance
    pub fn add_context(&mut self, context: Context) {
        self.contexts.insert(context.id.clone(), context);
    }

    /// Get a context by ID
    pub fn get_context(&self, id: &str) -> Option<&Context> {
        self.contexts.get(id)
    }

    /// Add a unit to the instance
    pub fn add_unit(&mut self, unit: Unit) {
        self.units.insert(unit.id.clone(), unit);
    }

    /// Get a unit by ID
    pub fn get_unit(&self, id: &str) -> Option<&Unit> {
        self.units.get(id)
    }

    /// Add a fact to the instance
    pub fn add_fact(&mut self, fact: Fact) {
        self.facts.push(fact);
    }

    /// Get all facts
    pub fn facts(&self) -> &[Fact] {
        &self.facts
    }

    /// Get facts for a specific concept
    pub fn facts_by_concept(&self, concept: &str) -> Vec<&Fact> {
        self.facts
            .iter()
            .filter(|f| f.concept() == concept)
            .collect()
    }

    /// Add a namespace prefix mapping
    pub fn add_namespace(&mut self, prefix: String, uri: String) {
        self.namespaces.insert(prefix, uri);
    }

    /// Get namespace URI for a prefix
    pub fn get_namespace(&self, prefix: &str) -> Option<&str> {
        self.namespaces.get(prefix).map(|s| s.as_str())
    }

    /// Get all contexts
    pub fn contexts(&self) -> &HashMap<String, Context> {
        &self.contexts
    }

    /// Get all units
    pub fn units(&self) -> &HashMap<String, Unit> {
        &self.units
    }
}
