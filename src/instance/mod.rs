//! XBRL instance document representation

mod context;
mod fact;
mod footnote;
mod parser;
mod resolver;
mod template;
mod typed;
mod unit;
mod view;
mod writer;

use crate::{
    NamespacePrefix, NamespaceUri, TaxonomySet,
    error::Result,
    validation::{self, ValidationResult},
};
pub use context::{Context, ContextId, EntityIdentifier, Period};
pub use fact::{Decimals, Fact, ItemFact, TupleFact};
pub use footnote::{FootnoteArc, FootnoteLink, FootnoteLocator, FootnoteResource};
pub use parser::InstanceParser;
use quick_xml::{Reader, Writer};
pub use resolver::resolve_instance;
use std::{collections::HashMap, fs::File, io, path::Path};
pub use typed::{FactValue, TypedFact, TypedInstanceDocument, TypedItemFact, TypedTupleFact};
pub use unit::{Unit, UnitId};
pub use view::{DocumentView, SectionView, TreeNode};
pub use writer::InstanceWriter;

/// Represents a complete XBRL instance document
#[derive(Debug, Default)]
pub struct InstanceDocument {
    /// Namespace prefixes used in the document (e.g. "xbrli" ->
    /// "http://www.xbrl.org/2003/instance")
    namespaces: HashMap<NamespacePrefix, NamespaceUri>,
    /// Schema references (xlink:href values from link:schemaRef elements)
    schema_refs: Vec<String>,
    /// roleURI values from roleRef elements in the instance.
    role_refs: Vec<String>,
    /// arcroleURI values from arcroleRef elements in the instance.
    arcrole_refs: Vec<String>,
    /// All contexts in the instance
    contexts: HashMap<ContextId, Context>,
    /// All units in the instance
    units: HashMap<UnitId, Unit>,
    /// Top-level facts in the instance (item and tuple facts)
    facts: Vec<Fact>,
    /// Footnote links found in the instance.
    footnote_links: Vec<FootnoteLink>,
}

impl InstanceDocument {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        schema_refs: Vec<String>,
        contexts: HashMap<ContextId, Context>,
        units: HashMap<UnitId, Unit>,
        facts: Vec<Fact>,
        namespaces: HashMap<NamespacePrefix, NamespaceUri>,
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
            footnote_links,
        }
    }

    /// Create a new instance pre-wired to a known taxonomy.
    ///
    /// - Registers all schema refs and role refs from the taxonomy
    /// - Adds both contexts and all provided units
    /// - Pre-populates nil facts for concepts in taxonomy schemas, preserving
    ///   tuple nesting and child order from XSD content models
    /// - For tuples with an exclusive single-choice content model, emits the
    ///   tuple as `xsi:nil=true` (without pre-populated choice children)
    /// - Assigns each fact the correct `unitRef` based on its XSD type:
    ///   monetary → first currency unit, shares → first shares unit,
    ///   other numeric → first pure unit, non-numeric → no unitRef
    /// - Skips concepts that participate in dimensional hypercube base sets.
    ///   Those facts require dimensional contexts and are not safe to emit into
    ///   a plain instant/duration context template.
    ///
    /// Build the [`DocumentView`] once after this call, then fill values
    /// in-place via [`set_fact_value`] without rebuilding the view.
    pub fn from_taxonomy(
        taxonomy: &TaxonomySet,
        namespaces: HashMap<NamespacePrefix, NamespaceUri>,
        instant_context: Context,
        duration_context: Context,
        units: &[Unit],
    ) -> Self {
        template::build_instance(
            taxonomy,
            namespaces,
            instant_context,
            duration_context,
            units,
        )
    }

    /// Parse an XBRL instance document from the file at the given path.
    ///
    /// Automatically extracts the `<xbrli:xbrl>` element if the input
    /// contains a wrapper around it.
    pub fn from_file(path: &Path) -> Result<Self> {
        let mut parser = InstanceParser::from_file(path)?;
        let instance = parser.parse()?;
        let doc = resolver::resolve_instance(instance)?;
        Ok(doc)
    }

    /// Parse an XBRL instance document from the reader.
    ///
    /// Automatically extracts the `<xbrli:xbrl>` element if the input
    /// contains a wrapper around it.
    pub fn from_reader<R>(reader: R) -> Result<Self>
    where
        R: io::BufRead,
    {
        let mut parser = InstanceParser::from_reader(reader);
        let instance = parser.parse()?;
        let doc = resolver::resolve_instance(instance)?;
        Ok(doc)
    }

    /// Parse an XBRL instance document from the XML reader.
    ///
    /// Automatically extracts the `<xbrli:xbrl>` element if the input
    /// contains a wrapper around it.
    pub fn from_xml_reader<R>(reader: Reader<R>) -> Result<Self>
    where
        R: io::BufRead,
    {
        let mut parser = InstanceParser::new(reader, None, false);
        let instance = parser.parse()?;
        let doc = resolver::resolve_instance(instance)?;
        Ok(doc)
    }

    /// Validate this instance against a taxonomy.
    pub fn validate(&self, taxonomy: &TaxonomySet) -> ValidationResult {
        validation::validate_all(self, taxonomy)
    }

    /// Convert this instance into a typed instance document.
    pub fn type_instance(self, taxonomy: &TaxonomySet) -> Result<TypedInstanceDocument> {
        TypedInstanceDocument::from_instance(self, taxonomy)
    }

    /// Convenience wrapper for [`DocumentView::build`] using this instance's
    /// full fact tree.
    ///
    /// The returned view is item-indexed (`fact_indices`) and keeps tuple
    /// concepts visible when corresponding tuple facts are present.
    pub fn view<'a>(&self, taxonomy: &'a TaxonomySet) -> DocumentView<'a> {
        DocumentView::build(self.facts(), taxonomy)
    }

    /// Serialize this instance to an XML file at the given path.
    pub fn to_file(&self, path: &Path) -> Result<()> {
        let file = File::create(path)?;
        self.to_writer(file)?;
        Ok(())
    }

    /// Serialize this instance to an XBRL XML document using a writer.
    pub fn to_writer<W>(&self, writer: W) -> Result<()>
    where
        W: io::Write,
    {
        let mut writer = InstanceWriter::new(Writer::new(writer), false);
        writer.write(self)
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

    /// Get all top-level facts.
    pub fn facts(&self) -> &[Fact] {
        &self.facts
    }

    /// Get all top-level facts mutably.
    pub fn facts_mut(&mut self) -> &mut [Fact] {
        &mut self.facts
    }

    /// Get all item facts in depth-first order.
    pub fn item_facts(&self) -> Vec<&ItemFact> {
        let mut out = Vec::new();
        for fact in &self.facts {
            fact.walk_items(&mut out);
        }
        out
    }

    /// Number of item facts in the instance (including nested tuple descendants).
    pub fn item_fact_count(&self) -> usize {
        self.facts.iter().map(|fact| fact.count_items()).sum()
    }

    /// Set the value of a fact by its index (from [`DocumentView`] fact_indices).
    /// Clears nil status.
    ///
    /// # Panics
    /// Panics if `index` is out of bounds.
    pub fn set_fact_value(&mut self, index: usize, value: String) {
        let mut current_index = 0usize;

        for fact in &mut self.facts {
            if Self::set_item_value_by_index(fact, index, &value, &mut current_index) {
                return;
            }
        }

        panic!("fact index out of bounds: {index}");
    }

    /// Set the nil status of a fact by its index (from [`DocumentView`] fact_indices).
    /// When setting nil=true, also clears the value.
    ///
    /// # Panics
    /// Panics if `index` is out of bounds.
    pub fn set_fact_nil(&mut self, index: usize, is_nil: bool) {
        let mut current_index = 0usize;

        for fact in &mut self.facts {
            if Self::set_item_nil_by_index(fact, index, is_nil, &mut current_index) {
                return;
            }
        }

        panic!("fact index out of bounds: {index}");
    }

    /// Sets `xsi:nil` on a tuple fact within all matching tuple instances.
    ///
    /// This is a mutation-only helper. It does not check taxonomy/schema
    /// compatibility; call [`validate`] explicitly after mutation.
    ///
    /// Returns the number of tuple instances that were mutated.
    pub fn set_tuple_fact_nil(&mut self, tuple_local_name: &str, is_nil: bool) -> Result<usize> {
        let mut changed = 0usize;

        for fact in &mut self.facts {
            changed += Self::set_tuple_fact_nil_in_fact(fact, tuple_local_name, is_nil);
        }

        Ok(changed)
    }

    /// Adds one tuple child within all matching tuple instances.
    ///
    /// Behavior:
    /// - if the child already exists, no change is made
    /// - if it does not exist, a new child is added using `child_fact`
    /// - if the tuple itself is nil and a child is added, it is set to non-nil
    ///
    /// This is a mutation-only helper. It does not check taxonomy/schema
    /// compatibility; call [`validate`] explicitly after mutation.
    ///
    /// Returns the number of tuple instances that were mutated.
    pub fn add_tuple_child(
        &mut self,
        tuple_local_name: &str,
        child_fact: &ItemFact,
    ) -> Result<usize> {
        let mut changed = 0usize;

        for fact in &mut self.facts {
            changed += Self::add_tuple_child_in_fact(fact, tuple_local_name, child_fact);
        }

        Ok(changed)
    }

    /// Removes one tuple child within all matching tuple instances.
    ///
    /// Behavior:
    /// - removes all item children whose local name matches
    ///   `child_local_name`
    /// - leaves all other children unchanged
    ///
    /// This is a mutation-only helper. It does not check taxonomy/schema
    /// compatibility; call [`validate`] explicitly after mutation.
    ///
    /// Returns the number of tuple instances that were mutated.
    pub fn remove_tuple_child(
        &mut self,
        tuple_local_name: &str,
        child_local_name: &str,
    ) -> Result<usize> {
        let mut changed = 0usize;

        for fact in &mut self.facts {
            changed += Self::remove_tuple_child_in_fact(fact, tuple_local_name, child_local_name);
        }

        Ok(changed)
    }

    /// Sets `xsi:nil` on a tuple child within all matching tuple instances.
    ///
    /// This is a mutation-only helper. It does not check taxonomy/schema
    /// compatibility; call [`validate`] explicitly after mutation.
    ///
    /// Returns the number of tuple instances that were mutated.
    pub fn set_tuple_child_nil(
        &mut self,
        tuple_local_name: &str,
        child_local_name: &str,
        is_nil: bool,
    ) -> Result<usize> {
        let mut changed = 0usize;

        for fact in &mut self.facts {
            changed +=
                Self::set_tuple_child_nil_in_fact(fact, tuple_local_name, child_local_name, is_nil);
        }

        Ok(changed)
    }

    /// Add a namespace prefix mapping
    pub fn add_namespace(&mut self, prefix: NamespacePrefix, uri: NamespaceUri) {
        self.namespaces.insert(prefix, uri);
    }

    /// Get namespace URI for a prefix
    pub fn get_namespace(&self, prefix: &str) -> Option<&str> {
        self.namespaces.get(prefix).map(|s| s.as_str())
    }

    /// Get all namespace prefix mappings
    pub fn namespaces(&self) -> &HashMap<NamespacePrefix, NamespaceUri> {
        &self.namespaces
    }

    /// Add a footnote link to the instance.
    pub fn add_footnote_link(&mut self, footnote_link: FootnoteLink) {
        self.footnote_links.push(footnote_link);
    }

    /// Get all footnote links in the instance.
    pub fn footnote_links(&self) -> &[FootnoteLink] {
        &self.footnote_links
    }

    /// Get all contexts.
    pub fn contexts(&self) -> &HashMap<ContextId, Context> {
        &self.contexts
    }

    /// Get a mutable reference to all contexts.
    pub fn contexts_mut(&mut self) -> &mut HashMap<ContextId, Context> {
        &mut self.contexts
    }

    /// Get all units.
    pub fn units(&self) -> &HashMap<UnitId, Unit> {
        &self.units
    }

    /// Get a mutable reference to all units.
    pub fn units_mut(&mut self) -> &mut HashMap<UnitId, Unit> {
        &mut self.units
    }

    /// Set the value of a fact by its index (from [`DocumentView`]
    /// fact_indices).
    fn set_item_nil_by_index(
        fact: &mut Fact,
        target_index: usize,
        is_nil: bool,
        current_index: &mut usize,
    ) -> bool {
        match fact {
            Fact::Item(item) => {
                if *current_index == target_index {
                    item.set_nil(is_nil);
                    if is_nil {
                        item.set_value(String::new());
                    }
                    true
                } else {
                    *current_index += 1;
                    false
                }
            }
            Fact::Tuple(tuple) => {
                for child in tuple.children_mut() {
                    if Self::set_item_nil_by_index(child, target_index, is_nil, current_index) {
                        return true;
                    }
                }
                false
            }
        }
    }

    fn set_item_value_by_index(
        fact: &mut Fact,
        target_index: usize,
        value: &str,
        current_index: &mut usize,
    ) -> bool {
        match fact {
            Fact::Item(item) => {
                if *current_index == target_index {
                    item.set_value(value.to_owned());
                    item.set_nil(false);
                    true
                } else {
                    *current_index += 1;
                    false
                }
            }
            Fact::Tuple(tuple) => {
                for child in tuple.children_mut() {
                    if Self::set_item_value_by_index(child, target_index, value, current_index) {
                        return true;
                    }
                }
                false
            }
        }
    }

    /// Sets `xsi:nil` on a tuple fact within a fact.
    ///
    /// This is a mutation-only helper. It does not check taxonomy/schema
    /// compatibility; call [`validate`] explicitly after mutation.
    ///
    /// Returns the number of tuple instances that were mutated.
    fn set_tuple_fact_nil_in_fact(fact: &mut Fact, tuple_local_name: &str, is_nil: bool) -> usize {
        match fact {
            Fact::Item(_) => 0,
            Fact::Tuple(tuple) => {
                let mut changed = 0usize;

                if tuple.concept_name().local_name == tuple_local_name {
                    tuple.set_nil(is_nil);
                    changed += 1;
                }

                for child in tuple.children_mut() {
                    changed += Self::set_tuple_fact_nil_in_fact(child, tuple_local_name, is_nil);
                }

                changed
            }
        }
    }

    /// Adds one tuple child within a fact.
    ///
    /// This is a mutation-only helper. It does not check taxonomy/schema
    /// compatibility; call [`validate`] explicitly after mutation.
    ///
    /// Returns the number of tuple instances that were mutated.
    fn add_tuple_child_in_fact(
        fact: &mut Fact,
        tuple_local_name: &str,
        child_fact: &ItemFact,
    ) -> usize {
        match fact {
            Fact::Item(_) => 0,
            Fact::Tuple(tuple) => {
                let mut changed = 0usize;

                if tuple.concept_name().local_name == tuple_local_name
                    && Self::add_tuple_child_in_tuple(tuple, child_fact)
                {
                    changed += 1;
                }

                for child in tuple.children_mut() {
                    changed += Self::add_tuple_child_in_fact(child, tuple_local_name, child_fact);
                }

                changed
            }
        }
    }

    /// Adds one tuple child within a tuple fact.
    fn add_tuple_child_in_tuple(tuple: &mut TupleFact, child_fact: &ItemFact) -> bool {
        let child_local_name = child_fact.concept_name().local_name.as_str();
        let has_matching_child = tuple.children().iter().any(|fact| {
            matches!(fact, Fact::Item(item) if item.concept_name().local_name == child_local_name)
        });

        // Explicit no-op when the child already exists.
        if has_matching_child {
            return false;
        }

        // A nil tuple has no active content. Adding a child makes it non-nil again.
        if tuple.is_nil() {
            tuple.set_nil(false);
        }

        let children = tuple.children_mut();

        let mut added = child_fact.clone();
        added.set_nil(false);
        children.push(Fact::Item(added));
        true
    }

    /// Removes one tuple child within a fact.
    ///
    /// This is a mutation-only helper. It does not check taxonomy/schema
    /// compatibility; call [`validate`] explicitly after mutation.
    ///
    /// Returns the number of tuple instances that were mutated.
    fn remove_tuple_child_in_fact(
        fact: &mut Fact,
        tuple_local_name: &str,
        child_local_name: &str,
    ) -> usize {
        match fact {
            Fact::Item(_) => 0,
            Fact::Tuple(tuple) => {
                let mut changed = 0usize;

                if tuple.concept_name().local_name == tuple_local_name
                    && Self::remove_tuple_child_in_tuple(tuple, child_local_name)
                {
                    changed += 1;
                }

                for child in tuple.children_mut() {
                    changed +=
                        Self::remove_tuple_child_in_fact(child, tuple_local_name, child_local_name);
                }

                changed
            }
        }
    }

    /// Removes all matching tuple item children within a tuple fact.
    fn remove_tuple_child_in_tuple(tuple: &mut TupleFact, child_local_name: &str) -> bool {
        let children = tuple.children_mut();
        let original_len = children.len();

        children.retain(|fact| {
            !matches!(fact, Fact::Item(item) if item.concept_name().local_name == child_local_name)
        });

        original_len != children.len()
    }

    /// Sets `xsi:nil` on a tuple child within a tuple fact.
    ///
    /// This is a mutation-only helper. It does not check taxonomy/schema
    /// compatibility; call [`validate`] explicitly after mutation.
    ///
    /// Returns true if the fact was mutated.
    ///
    /// Note: this only sets nil on existing children; it does not add new nil
    /// children if the target child does not already exist.
    ///
    /// If the same child appears multiple times within the same tuple, all
    /// occurrences will be updated.
    fn set_tuple_child_nil_in_fact(
        fact: &mut Fact,
        tuple_local_name: &str,
        child_local_name: &str,
        is_nil: bool,
    ) -> usize {
        match fact {
            Fact::Item(_) => 0,
            Fact::Tuple(tuple) => {
                let mut changed = 0usize;

                if tuple.concept_name().local_name == tuple_local_name
                    && Self::set_tuple_child_nil_in_tuple(tuple, child_local_name, is_nil)
                {
                    changed += 1;
                }

                for child in tuple.children_mut() {
                    changed += Self::set_tuple_child_nil_in_fact(
                        child,
                        tuple_local_name,
                        child_local_name,
                        is_nil,
                    );
                }

                changed
            }
        }
    }

    /// Sets `xsi:nil` on a tuple child within a tuple fact.
    fn set_tuple_child_nil_in_tuple(
        tuple: &mut TupleFact,
        child_local_name: &str,
        is_nil: bool,
    ) -> bool {
        let mut touched = false;

        for fact in tuple.children_mut() {
            if let Fact::Item(item) = fact
                && item.concept_name().local_name == child_local_name
            {
                item.set_nil(is_nil);
                if is_nil {
                    item.set_value(String::new());
                }
                touched = true;
            }
        }

        touched
    }
}

#[cfg(test)]
mod tests {
    use super::{Fact, InstanceDocument, ItemFact, TaxonomySet, TupleFact};
    use crate::{ExpandedName, NamespaceUri};

    fn expanded_name(local_name: &str) -> ExpandedName {
        ExpandedName::new(
            NamespaceUri::from("http://example.com/ns"),
            local_name.to_owned(),
        )
    }

    fn item(local_name: &str, value: &str, is_nil: bool) -> Fact {
        Fact::Item(ItemFact::new(
            None,
            expanded_name(local_name),
            "D-2020".to_owned(),
            None,
            value.to_owned(),
            is_nil,
            None,
            None,
        ))
    }

    fn tuple(local_name: &str, children: Vec<Fact>) -> Fact {
        let mut tuple = TupleFact::new(expanded_name(local_name));

        for child in children {
            tuple.add_child(child);
        }

        Fact::Tuple(tuple)
    }

    #[test]
    fn type_instance_fails_for_unknown_concept() {
        let taxonomy = TaxonomySet::default();
        let mut instance = InstanceDocument::default();

        instance.add_fact(item("unknownConcept", "1", false));

        let result = instance.type_instance(&taxonomy);
        assert!(result.is_err());
    }

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

        let instance =
            InstanceDocument::from_reader(xml.as_bytes()).expect("instance should parse");

        assert_eq!(instance.schema_refs().len(), 1);
        assert!(instance.contexts().is_empty());
        assert!(instance.units().is_empty());
        assert!(instance.facts().is_empty());
    }

    #[test]
    fn validate_reports_duplicate_role_refs() {
        let taxonomy = TaxonomySet::default();
        let mut instance = InstanceDocument::default();
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
        let mut instance = InstanceDocument::default();
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
        let mut instance = InstanceDocument::default();

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

        let instance =
            InstanceDocument::from_reader(xml.as_bytes()).expect("instance should parse");

        assert_eq!(instance.role_refs(), ["http://www.xbrl.org/2003/role/link"]);
        assert_eq!(
            instance.arcrole_refs(),
            ["http://www.xbrl.org/2003/arcrole/fact-footnote"]
        );
    }

    #[test]
    fn validate_reports_both_duplicate_role_and_arcrole_refs() {
        let taxonomy = TaxonomySet::default();
        let mut instance = InstanceDocument::default();

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

    #[test]
    fn remove_tuple_child_removes_existing_child() {
        let mut instance = InstanceDocument::default();
        instance.add_fact(tuple(
            "genInfo.report.id.specialAccountingStandard",
            vec![
                item("genInfo.report.id.specialAccountingStandard.K", "", false),
                item("genInfo.report.id.specialAccountingStandard.RKV", "", true),
            ],
        ));

        let changed = instance
            .remove_tuple_child(
                "genInfo.report.id.specialAccountingStandard",
                "genInfo.report.id.specialAccountingStandard.RKV",
            )
            .expect("remove should succeed");

        assert_eq!(changed, 1);

        let facts = instance.item_facts();
        assert_eq!(facts.len(), 1);
        assert!(facts.iter().any(|fact| {
            fact.concept_name().local_name == "genInfo.report.id.specialAccountingStandard.K"
        }));
        assert!(!facts.iter().any(|fact| {
            fact.concept_name().local_name == "genInfo.report.id.specialAccountingStandard.RKV"
        }));
    }

    #[test]
    fn add_tuple_child_adds_new_child_when_missing() {
        let mut instance = InstanceDocument::default();
        instance.add_fact(tuple(
            "genInfo.report.id.specialAccountingStandard",
            vec![item(
                "genInfo.report.id.specialAccountingStandard.K",
                "",
                false,
            )],
        ));

        let changed = instance
            .add_tuple_child(
                "genInfo.report.id.specialAccountingStandard",
                &ItemFact::new(
                    None,
                    expanded_name("genInfo.report.id.specialAccountingStandard.RKV"),
                    "D-2020".to_owned(),
                    None,
                    String::new(),
                    false,
                    None,
                    None,
                ),
            )
            .expect("add should succeed");

        assert_eq!(changed, 1);

        let facts = instance.item_facts();
        assert_eq!(facts.len(), 2);
        assert!(facts.iter().any(|fact| {
            fact.concept_name().local_name == "genInfo.report.id.specialAccountingStandard.RKV"
                && !fact.is_nil()
        }));
        assert!(facts.iter().any(|fact| {
            fact.concept_name().local_name == "genInfo.report.id.specialAccountingStandard.K"
                && !fact.is_nil()
        }));
    }

    #[test]
    fn add_tuple_child_does_nothing_when_child_exists() {
        let mut instance = InstanceDocument::default();
        instance.add_fact(tuple(
            "genInfo.report.id.specialAccountingStandard",
            vec![
                item("genInfo.report.id.specialAccountingStandard.K", "", false),
                item("genInfo.report.id.specialAccountingStandard.RKV", "", true),
            ],
        ));

        let changed = instance
            .add_tuple_child(
                "genInfo.report.id.specialAccountingStandard",
                &ItemFact::new(
                    None,
                    expanded_name("genInfo.report.id.specialAccountingStandard.RKV"),
                    "D-2020".to_owned(),
                    None,
                    String::new(),
                    false,
                    None,
                    None,
                ),
            )
            .expect("add should succeed");

        assert_eq!(changed, 0);

        let facts = instance.item_facts();
        assert_eq!(facts.len(), 2);
        let rkv = facts
            .iter()
            .find(|fact| {
                fact.concept_name().local_name == "genInfo.report.id.specialAccountingStandard.RKV"
            })
            .expect("RKV child should exist");
        assert!(rkv.is_nil());
    }

    #[test]
    fn set_tuple_child_nil_sets_nil_and_clears_value() {
        let mut instance = InstanceDocument::default();
        instance.add_fact(tuple(
            "genInfo.report.id.reportElement",
            vec![item(
                "genInfo.report.id.reportElement.reportElements.BVV",
                "present",
                false,
            )],
        ));

        let changed = instance
            .set_tuple_child_nil(
                "genInfo.report.id.reportElement",
                "genInfo.report.id.reportElement.reportElements.BVV",
                true,
            )
            .expect("nil update should succeed");

        assert_eq!(changed, 1);

        let facts = instance.item_facts();
        let bvv = facts
            .iter()
            .find(|fact| {
                fact.concept_name().local_name
                    == "genInfo.report.id.reportElement.reportElements.BVV"
            })
            .expect("BVV child should exist");
        assert!(bvv.is_nil());
        assert_eq!(bvv.value(), "");
    }

    #[test]
    fn set_tuple_fact_nil() {
        let mut instance = InstanceDocument::default();
        instance.add_fact(tuple(
            "genInfo.report.id.reportType",
            vec![item(
                "genInfo.report.id.reportType.reportType.JA",
                "",
                false,
            )],
        ));

        let changed = instance
            .set_tuple_fact_nil("genInfo.report.id.reportType", true)
            .expect("tuple nil update should succeed");

        assert_eq!(changed, 1);
        let tuple_fact = match &instance.facts()[0] {
            Fact::Tuple(tuple) => tuple,
            _ => panic!("expected tuple fact"),
        };
        assert!(tuple_fact.is_nil());
    }
}
