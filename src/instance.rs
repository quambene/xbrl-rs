//! XBRL instance document representation

use crate::{Context, Fact, TaxonomySet, Unit, validation};
use crate::{reader, validation::ValidationResult, writer};
use anyhow::Result;
use std::collections::HashMap;

/// Represents a complete XBRL instance document
#[derive(Debug)]
pub struct XbrlInstance {
    /// Schema references (xlink:href values from link:schemaRef elements)
    schema_refs: Vec<String>,
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
            schema_refs: Vec::new(),
            contexts: HashMap::new(),
            units: HashMap::new(),
            facts: Vec::new(),
            namespaces: HashMap::new(),
        }
    }

    /// Parse an XBRL instance document from XML.
    ///
    /// Automatically extracts the `<xbrli:xbrl>` element if the input
    /// contains a wrapper around it.
    pub fn from_xml(xml: &str) -> Result<Self> {
        reader::parse_xml(xml)
    }

    /// Validate this instance against a taxonomy.
    pub fn validate(&self, taxonomy: &TaxonomySet) -> ValidationResult {
        validation::validate_all(self, taxonomy)
    }

    /// Serialize this instance to an XBRL XML document.
    pub fn to_xml(&self) -> Result<String, anyhow::Error> {
        writer::write_xml(self)
    }

    /// Add a schema reference (xlink:href from a link:schemaRef element)
    pub fn add_schema_ref(&mut self, href: String) {
        self.schema_refs.push(href);
    }

    /// Get all schema references declared in the instance document.
    pub fn schema_refs(&self) -> &[String] {
        &self.schema_refs
    }

    /// Extract relative path suffixes from schema reference URLs.
    ///
    /// Strips the URL scheme, host, and leading `/taxonomies/` segment to
    /// produce paths suitable for joining with a local taxonomy directory.
    ///
    /// For example:
    /// `http://www.xbrl.de/taxonomies/de-gcd-2020-04-01/de-gcd-2020-04-01-shell.xsd`
    /// becomes `de-gcd-2020-04-01/de-gcd-2020-04-01-shell.xsd`.
    pub fn schema_ref_paths(&self) -> Vec<&str> {
        self.schema_refs
            .iter()
            .map(|href| {
                // Find the path portion after "://" + host
                let path = href
                    .find("://")
                    .and_then(|i| href[i + 3..].find('/'))
                    .map(|i| &href[href.find("://").unwrap() + 3 + i..])
                    .unwrap_or(href);
                // Strip leading "/taxonomies/" if present
                path.strip_prefix("/taxonomies/")
                    .or_else(|| path.strip_prefix("/"))
                    .unwrap_or(path)
            })
            .collect()
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

    /// Get all facts mutably
    pub fn facts_mut(&mut self) -> &mut [Fact] {
        &mut self.facts
    }

    /// Add a namespace prefix mapping
    pub fn add_namespace(&mut self, prefix: String, uri: String) {
        self.namespaces.insert(prefix, uri);
    }

    /// Get namespace URI for a prefix
    pub fn get_namespace(&self, prefix: &str) -> Option<&str> {
        self.namespaces.get(prefix).map(|s| s.as_str())
    }

    /// Get all namespace prefix mappings
    pub fn namespaces(&self) -> &HashMap<String, String> {
        &self.namespaces
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
