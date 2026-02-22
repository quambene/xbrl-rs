//! XBRL instance document representation

mod context;
mod fact;
mod footnote;
mod reader;
mod unit;
mod view;
mod writer;

use crate::{TaxonomySet, error::Result, validation, validation::ValidationResult};
pub use context::{Context, EntityIdentifier, Period};
pub use fact::Fact;
pub use footnote::{FootnoteArc, FootnoteLink, FootnoteLocator, FootnoteResource};
pub use view::{DocumentView, SectionView, TreeNode};
use quick_xml::{Reader, Writer};
use std::{collections::HashMap, io};
pub use unit::Unit;

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

    /// Build a hierarchical document view from the presentation linkbase.
    ///
    /// Each section corresponds to one extended link role in the presentation
    /// linkbase. Nodes within each section are ordered by the arc `order`
    /// attribute and annotated with labels for the requested `lang`. Facts
    /// from this instance are attached to their matching concept nodes.
    pub fn view<'a>(&'a self, taxonomy: &'a TaxonomySet, lang: &str) -> DocumentView<'a> {
        view::build_view(&self.facts, taxonomy, lang)
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

#[cfg(test)]
mod tests {
    use super::XbrlInstance;
    use crate::TaxonomySet;
    use quick_xml::Reader;

    #[test]
    fn from_xml_parses_basic_instance() {
        let xml = r#"
            <xbrli:xbrl
                xmlns:xbrli="http://www.xbrl.org/2003/instance"
                xmlns:link="http://www.xbrl.org/2003/linkbase"
                xmlns:xlink="http://www.w3.org/1999/xlink">
                <link:schemaRef
                    xlink:type="simple"
                    xlink:href="http://www.xbrl.de/taxonomies/de-gcd-2020-04-01/de-gcd-2020-04-01-shell.xsd"/>
            </xbrli:xbrl>
        "#;

        let mut reader = Reader::from_str(xml);
        let instance = XbrlInstance::from_xml(&mut reader).expect("instance should parse");

        assert_eq!(instance.schema_refs().len(), 1);
        assert!(instance.contexts().is_empty());
        assert!(instance.units().is_empty());
        assert!(instance.facts().is_empty());
    }

    #[test]
    fn validate_reports_duplicate_role_refs() {
        let taxonomy = TaxonomySet::default();
        let mut instance = XbrlInstance::default();
        let role_uri = "http://www.xbrl.org/2003/role/link".to_string();

        instance.add_role_ref(role_uri.clone());
        instance.add_role_ref(role_uri);

        let result = instance.validate(&taxonomy);

        assert!(!result.is_valid());
        assert!(
            result
                .errors()
                .iter()
                .any(|message| message.code == "spec.duplicate_role_ref")
        );
    }

    #[test]
    fn validate_reports_duplicate_arcrole_refs() {
        let taxonomy = TaxonomySet::default();
        let mut instance = XbrlInstance::default();
        let arcrole_uri = "http://www.xbrl.org/2003/arcrole/fact-footnote".to_string();

        instance.add_arcrole_ref(arcrole_uri.clone());
        instance.add_arcrole_ref(arcrole_uri);

        let result = instance.validate(&taxonomy);

        assert!(!result.is_valid());
        assert!(
            result
                .errors()
                .iter()
                .any(|message| message.code == "spec.duplicate_arcrole_ref")
        );
    }

    #[test]
    fn validate_accepts_unique_refs() {
        let taxonomy = TaxonomySet::default();
        let mut instance = XbrlInstance::default();

        instance.add_role_ref("http://www.xbrl.org/2003/role/link".to_string());
        instance.add_arcrole_ref("http://www.xbrl.org/2003/arcrole/fact-footnote".to_string());

        let result = instance.validate(&taxonomy);

        assert!(
            result.is_valid(),
            "unexpected errors: {:#?}",
            result.errors()
        );
        assert!(result.errors().is_empty());
    }

    #[test]
    fn from_xml_parses_role_and_arcrole_refs() {
        let xml = r#"
            <xbrli:xbrl
                xmlns:xbrli="http://www.xbrl.org/2003/instance"
                xmlns:link="http://www.xbrl.org/2003/linkbase"
                xmlns:xlink="http://www.w3.org/1999/xlink">
                <link:roleRef
                    roleURI="http://www.xbrl.org/2003/role/link"
                    xlink:type="simple"
                    xlink:href="dummy.xsd#role_link"/>
                <link:arcroleRef
                    arcroleURI="http://www.xbrl.org/2003/arcrole/fact-footnote"
                    xlink:type="simple"
                    xlink:href="dummy.xsd#arcrole_fact_footnote"/>
            </xbrli:xbrl>
        "#;

        let mut reader = Reader::from_str(xml);
        let instance = XbrlInstance::from_xml(&mut reader).expect("instance should parse");

        assert_eq!(instance.role_refs(), ["http://www.xbrl.org/2003/role/link"]);
        assert_eq!(
            instance.arcrole_refs(),
            ["http://www.xbrl.org/2003/arcrole/fact-footnote"]
        );
    }

    #[test]
    fn validate_reports_both_duplicate_role_and_arcrole_refs() {
        let taxonomy = TaxonomySet::default();
        let mut instance = XbrlInstance::default();

        instance.add_role_ref("http://www.xbrl.org/2003/role/link".to_string());
        instance.add_role_ref("http://www.xbrl.org/2003/role/link".to_string());
        instance.add_arcrole_ref("http://www.xbrl.org/2003/arcrole/fact-footnote".to_string());
        instance.add_arcrole_ref("http://www.xbrl.org/2003/arcrole/fact-footnote".to_string());

        let result = instance.validate(&taxonomy);

        assert!(!result.is_valid());
        assert!(
            result
                .errors()
                .iter()
                .any(|message| message.code == "spec.duplicate_role_ref")
        );
        assert!(
            result
                .errors()
                .iter()
                .any(|message| message.code == "spec.duplicate_arcrole_ref")
        );
    }
}
