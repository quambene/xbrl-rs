//! XBRL instance document representation

mod context;
mod fact;
mod reader;
mod unit;
mod writer;

use crate::{TaxonomySet, error::Result, validation, validation::ValidationResult};
pub use context::{Context, EntityIdentifier, Period};
pub use fact::Fact;
use quick_xml::{Reader, Writer};
use std::{collections::HashMap, io};
pub use unit::Unit;

/// A single `link:footnoteLink` extended link in an XBRL instance.
#[derive(Debug, Clone, Default)]
pub struct FootnoteLink {
    /// Optional `xlink:role` on the footnote link.
    pub role: Option<String>,
    /// Optional `xml:lang` inherited by contained footnote resources.
    pub xml_lang: Option<String>,
    /// Locator resources (`link:loc` or custom locator-like elements).
    pub locators: Vec<FootnoteLocator>,
    /// Footnote resources (`link:footnote`).
    pub footnotes: Vec<FootnoteResource>,
    /// Arcs connecting locators and footnote resources.
    pub arcs: Vec<FootnoteArc>,
}

/// A locator within a footnote link, usually a `link:loc` element.
#[derive(Debug, Clone)]
pub struct FootnoteLocator {
    /// Local name of the locator element (e.g. `loc` or a custom element).
    pub element_local_name: String,
    /// Optional `xlink:label` used for arc endpoints.
    pub label: Option<String>,
    /// Optional `xlink:href` target, typically a same-document fragment.
    pub href: Option<String>,
}

/// A footnote resource within a footnote link (`link:footnote`).
#[derive(Debug, Clone)]
pub struct FootnoteResource {
    /// Optional `xlink:label` used for arc endpoints.
    pub label: Option<String>,
    /// Optional XML `id` of the footnote resource.
    pub id: Option<String>,
    /// Optional `xlink:role` of the resource.
    pub role: Option<String>,
    /// Optional `xml:lang` for the footnote text content.
    pub xml_lang: Option<String>,
}

/// An arc in a footnote link (for example `link:footnoteArc`).
#[derive(Debug, Clone)]
pub struct FootnoteArc {
    /// Optional `xlink:from` label.
    pub from: Option<String>,
    /// Optional `xlink:to` label.
    pub to: Option<String>,
    /// Optional `xlink:arcrole` of the relationship.
    pub arcrole: Option<String>,
}

/// Represents a complete XBRL instance document
#[derive(Debug, Default)]
pub struct XbrlInstance {
    /// Schema references (xlink:href values from link:schemaRef elements)
    schema_refs: Vec<String>,
    /// roleURI values from roleRef elements in the instance.
    role_refs: Vec<String>,
    /// arcroleURI values from arcroleRef elements in the instance.
    arcrole_refs: Vec<String>,
    /// All contexts in the instance
    contexts: HashMap<String, Context>,
    /// All units in the instance
    units: HashMap<String, Unit>,
    /// All facts in the instance
    facts: Vec<Fact>,
    /// Namespace prefixes used in the document (e.g. "xbrli" ->
    /// "http://www.xbrl.org/2003/instance")
    namespaces: HashMap<String, String>,
    /// xml:lang value declared on the root xbrl element, if present.
    root_xml_lang: Option<String>,
    /// Source document file name used for scope-sensitive checks.
    document_name: Option<String>,
    /// Footnote links found in the instance.
    footnote_links: Vec<FootnoteLink>,
}

impl XbrlInstance {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        schema_refs: Vec<String>,
        contexts: HashMap<String, Context>,
        units: HashMap<String, Unit>,
        facts: Vec<Fact>,
        namespaces: HashMap<String, String>,
        root_xml_lang: Option<String>,
        document_name: Option<String>,
        footnote_links: Vec<FootnoteLink>,
    ) -> Self {
        Self {
            schema_refs,
            role_refs: Vec::new(),
            arcrole_refs: Vec::new(),
            contexts,
            units,
            facts,
            namespaces,
            root_xml_lang,
            document_name,
            footnote_links,
        }
    }

    /// Parse an XBRL instance document from XML.
    ///
    /// Automatically extracts the `<xbrli:xbrl>` element if the input
    /// contains a wrapper around it.
    pub fn from_xml<R>(reader: &mut Reader<R>) -> Result<Self>
    where
        R: io::BufRead,
    {
        reader::read_xml(reader)
    }

    /// Validate this instance against a taxonomy.
    pub fn validate(&self, taxonomy: &TaxonomySet) -> ValidationResult {
        validation::validate_all(self, taxonomy)
    }

    /// Serialize this instance to an XBRL XML document.
    pub fn to_xml<W>(&self, writer: &mut Writer<W>) -> Result<()>
    where
        W: io::Write,
    {
        writer::write_xml(writer, self)
    }

    /// Add a schema reference (xlink:href from a link:schemaRef element)
    pub fn add_schema_ref(&mut self, href: String) {
        self.schema_refs.push(href);
    }

    /// Get all schema references declared in the instance document.
    pub fn schema_refs(&self) -> &[String] {
        &self.schema_refs
    }

    /// Add a role reference URI from a roleRef element.
    pub fn add_role_ref(&mut self, role_uri: String) {
        self.role_refs.push(role_uri);
    }

    /// Get all role reference URIs declared in the instance document.
    pub fn role_refs(&self) -> &[String] {
        &self.role_refs
    }

    /// Add an arcrole reference URI from an arcroleRef element.
    pub fn add_arcrole_ref(&mut self, arcrole_uri: String) {
        self.arcrole_refs.push(arcrole_uri);
    }

    /// Get all arcrole reference URIs declared in the instance document.
    pub fn arcrole_refs(&self) -> &[String] {
        &self.arcrole_refs
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

    pub fn set_root_xml_lang(&mut self, xml_lang: Option<String>) {
        self.root_xml_lang = xml_lang;
    }

    pub fn root_xml_lang(&self) -> Option<&str> {
        self.root_xml_lang.as_deref()
    }

    pub fn set_document_name(&mut self, document_name: Option<String>) {
        self.document_name = document_name;
    }

    pub fn document_name(&self) -> Option<&str> {
        self.document_name.as_deref()
    }

    pub fn add_footnote_link(&mut self, footnote_link: FootnoteLink) {
        self.footnote_links.push(footnote_link);
    }

    pub fn footnote_links(&self) -> &[FootnoteLink] {
        &self.footnote_links
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
