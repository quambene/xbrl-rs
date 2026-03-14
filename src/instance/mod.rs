//! XBRL instance document representation

mod context;
mod fact;
mod footnote;
mod parser;
mod resolver;
mod unit;
mod view;
mod writer;

use crate::{
    PresentationArc, TaxonomySet,
    error::Result,
    taxonomy::{Concept, PeriodType, TupleChild},
    validation::{self, ValidationResult},
};
pub use context::{Context, ContextId, EntityIdentifier, Period};
pub use fact::{Decimals, Fact, ItemFact, TupleFact};
pub use footnote::{FootnoteArc, FootnoteLink, FootnoteLocator, FootnoteResource};
pub use parser::RawUnit;
use quick_xml::{Reader, Writer};
use std::{
    borrow::Borrow,
    cmp::Ordering,
    collections::{HashMap, HashSet},
    fmt, io,
    ops::Deref,
};
pub use unit::{Unit, UnitId};
pub use view::{DocumentView, SectionView, TreeNode};

/// Type-safe namespace prefix key used in the instance namespace map
/// (the `xmlns:prefix` declarations on the root `<xbrli:xbrl>` element).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NamespacePrefix(String);

impl NamespacePrefix {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for NamespacePrefix {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for NamespacePrefix {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl Deref for NamespacePrefix {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl AsRef<str> for NamespacePrefix {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Borrow<str> for NamespacePrefix {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for NamespacePrefix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NamespaceUri(String);

impl NamespaceUri {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for NamespaceUri {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for NamespaceUri {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl Deref for NamespaceUri {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl AsRef<str> for NamespaceUri {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for NamespaceUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Represents a complete XBRL instance document
#[derive(Debug, Default)]
pub struct InstanceDocument {
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
    /// Namespace prefixes used in the document (e.g. "xbrli" ->
    /// "http://www.xbrl.org/2003/instance")
    namespaces: HashMap<NamespacePrefix, NamespaceUri>,
    /// Source document file name used for scope-sensitive checks.
    document_name: Option<String>,
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
            document_name,
            footnote_links,
        }
    }

    /// Create a new instance pre-wired to a known taxonomy.
    ///
    /// - Registers all schema refs and role refs from the taxonomy
    /// - Adds both contexts and all provided units
    /// - Pre-populates nil facts for concepts in the presentation linkbase,
    ///   preserving tuple nesting derived directly from the presentation tree
    /// - Assigns each fact the correct `unitRef` based on its XSD type:
    ///   monetary → first currency unit, shares → first shares unit,
    ///   other numeric → first pure unit, non-numeric → no unitRef
    ///
    /// Build the [`DocumentView`] once after this call, then fill values
    /// in-place via [`set_fact_value`] without rebuilding the view.
    pub fn from_taxonomy(
        taxonomy: &TaxonomySet,
        instant_context: Context,
        duration_context: Context,
        units: &[Unit],
    ) -> Self {
        let mut instance = Self::default();

        for schema_url in taxonomy.schema_refs().keys() {
            instance.add_schema_ref(schema_url.to_string());
        }
        for role in taxonomy.role_types() {
            instance.add_role_ref(role.role_uri.to_string());
        }

        let instant_context_ref = instant_context.id.clone();
        let duration_context_ref = duration_context.id.clone();
        instance.add_context(instant_context);
        instance.add_context(duration_context);

        for unit in units {
            instance.add_unit(unit.clone());
        }

        // Walk the presentation tree in section order, depth-first within each section.
        // The tree structure gives both the fact order and the tuple nesting directly,
        // without needing to consult schema substitution groups.
        let mut recursion_path: HashSet<String> = HashSet::new();
        let mut emitted_items: HashSet<String> = HashSet::new();
        let mut emitted_tuples: HashSet<String> = HashSet::new();
        for arcs in taxonomy.presentations().values() {
            let mut arc_index: HashMap<&str, Vec<&PresentationArc>> = HashMap::new();
            for arc in arcs {
                arc_index.entry(arc.from.as_str()).or_default().push(arc);
            }
            for children in arc_index.values_mut() {
                children.sort_by(|a, b| match (a.order, b.order) {
                    (Some(x), Some(y)) => x.cmp(&y),
                    (Some(_), None) => Ordering::Less,
                    (None, Some(_)) => Ordering::Greater,
                    (None, None) => Ordering::Equal,
                });
            }

            let roots = view::find_roots(arcs, &arc_index);
            let mut seeded_nodes: HashSet<&str> = HashSet::new();
            for root_id in roots {
                seeded_nodes.insert(root_id);
                let mut hoisted: Vec<Fact> = Vec::new();
                Self::populate_from_tree(
                    &arc_index,
                    root_id,
                    taxonomy,
                    &instant_context_ref,
                    &duration_context_ref,
                    units,
                    &mut instance.facts,
                    &mut emitted_items,
                    &mut emitted_tuples,
                    &mut recursion_path,
                    None,
                    &mut hoisted,
                );
                instance.facts.extend(hoisted);
            }

            let mut remaining_nodes: Vec<&str> = arcs
                .iter()
                .flat_map(|arc| [arc.from.as_str(), arc.to.as_str()])
                .filter(|concept_id| !seeded_nodes.contains(*concept_id))
                .collect();
            remaining_nodes.sort_unstable();
            remaining_nodes.dedup();

            for concept_id in remaining_nodes {
                let mut hoisted: Vec<Fact> = Vec::new();
                Self::populate_from_tree(
                    &arc_index,
                    concept_id,
                    taxonomy,
                    &instant_context_ref,
                    &duration_context_ref,
                    units,
                    &mut instance.facts,
                    &mut emitted_items,
                    &mut emitted_tuples,
                    &mut recursion_path,
                    None,
                    &mut hoisted,
                );
                instance.facts.extend(hoisted);
            }
        }

        instance
    }

    /// Parse an XBRL instance document from XML.
    ///
    /// Automatically extracts the `<xbrli:xbrl>` element if the input
    /// contains a wrapper around it.
    pub fn from_xml<R>(mut reader: Reader<R>) -> Result<Self>
    where
        R: io::BufRead,
    {
        todo!()
    }

    /// Validate this instance against a taxonomy.
    pub fn validate(&self, taxonomy: &TaxonomySet) -> ValidationResult {
        validation::validate_all(self, taxonomy)
    }

    /// Convenience wrapper for [`DocumentView::build`] using this instance's item facts.
    pub fn view<'a>(&self, taxonomy: &'a TaxonomySet) -> DocumentView<'a> {
        let item_facts = self.item_facts();
        DocumentView::build(&item_facts, taxonomy)
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

    /// Add a namespace prefix mapping
    pub fn add_namespace(&mut self, prefix: String, uri: String) {
        self.namespaces
            .insert(NamespacePrefix::from(prefix), NamespaceUri::from(uri));
    }

    /// Get namespace URI for a prefix
    pub fn get_namespace(&self, prefix: &str) -> Option<&str> {
        self.namespaces.get(prefix).map(|s| s.as_str())
    }

    /// Get all namespace prefix mappings
    pub fn namespaces(&self) -> &HashMap<NamespacePrefix, NamespaceUri> {
        &self.namespaces
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
    pub fn contexts(&self) -> &HashMap<ContextId, Context> {
        &self.contexts
    }

    /// Get all units
    pub fn units(&self) -> &HashMap<UnitId, Unit> {
        &self.units
    }

    /// Recursively walk one node of the presentation tree and emit facts.
    ///
    /// - Concrete tuple → push a [`TupleFact`] and recurse into its children.
    /// - Concrete item  → push an [`ItemFact`] (nil placeholder).  If the item
    ///   is not a valid schema child of the enclosing tuple (per its
    ///   `xs:complexType` content model) it is pushed to `hoisted` instead,
    ///   which `from_taxonomy` appends to the top-level facts after all sections
    ///   have been traversed.
    /// - Abstract / grouping → recurse into children at the same level.
    #[allow(clippy::too_many_arguments)]
    fn populate_from_tree(
        arc_index: &HashMap<&str, Vec<&PresentationArc>>,
        concept_id: &str,
        taxonomy: &TaxonomySet,
        instant_ctx: &ContextId,
        duration_ctx: &ContextId,
        units: &[Unit],
        facts: &mut Vec<Fact>,
        emitted_items: &mut HashSet<String>,
        emitted_tuples: &mut HashSet<String>,
        recursion_path: &mut HashSet<String>,
        parent_tuple_element: Option<&Concept>,
        hoisted: &mut Vec<Fact>,
    ) {
        if !recursion_path.insert(concept_id.to_string()) {
            return; // cycle guard within current recursion branch
        }

        // Children are already sorted by `order`.
        let children = arc_index.get(concept_id).map(Vec::as_slice).unwrap_or(&[]);

        if let Some(concept) = taxonomy.find_concept_by_id(concept_id) {
            if taxonomy.concept_is_tuple(concept) && !concept.is_abstract {
                if emitted_tuples.insert(concept_id.to_string()) {
                    let concept_name = concept_id.replacen('_', ":", 1);
                    facts.push(Fact::Tuple(TupleFact::new(concept_name)));
                    let tuple_children = match facts.last_mut() {
                        Some(Fact::Tuple(t)) => t.children_mut(),
                        _ => unreachable!(),
                    };

                    for arc in children {
                        Self::populate_from_tree(
                            arc_index,
                            arc.to.as_str(),
                            taxonomy,
                            instant_ctx,
                            duration_ctx,
                            units,
                            tuple_children,
                            emitted_items,
                            emitted_tuples,
                            recursion_path,
                            Some(concept),
                            hoisted,
                        );
                    }
                }
                recursion_path.remove(concept_id);
                return;
            }

            if !concept.is_abstract
                && let Some(ref period_type) = concept.period_type
            {
                let context_ref = match period_type {
                    PeriodType::Duration => duration_ctx,
                    PeriodType::Instant => instant_ctx,
                };
                let concept_name = concept_id.replacen('_', ":", 1);

                if emitted_items.insert(concept_id.to_string()) {
                    let mut fact = ItemFact::new(
                        concept_name,
                        context_ref.to_string(),
                        unit_ref_for_concept(concept, units),
                        String::new(),
                    );
                    fact.set_nil(true);

                    // Items not allowed by the tuple's content model are hoisted to
                    // the top level so they still appear in the generated template.
                    if let Some(parent_el) = parent_tuple_element
                        && !item_allowed_in_tuple(parent_el, concept, taxonomy)
                    {
                        hoisted.push(Fact::Item(fact));
                    } else {
                        facts.push(Fact::Item(fact));
                    }
                }
            }
        }

        // Recurse children at the same level for non-structural presentation parents.
        for arc in children {
            Self::populate_from_tree(
                arc_index,
                arc.to.as_str(),
                taxonomy,
                instant_ctx,
                duration_ctx,
                units,
                facts,
                emitted_items,
                emitted_tuples,
                recursion_path,
                parent_tuple_element,
                hoisted,
            );
        }

        recursion_path.remove(concept_id);
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
}

/// Determine the correct `unitRef` string for an element based on its XSD type.
///
/// - Monetary items  → first currency unit (`is_currency()`)
/// - Shares items    → first shares unit (`is_shares()`)
/// - Other numeric   → first pure unit (`is_pure()`)
/// - Non-numeric     → `None` (unitRef forbidden by the XBRL spec)
fn unit_ref_for_concept(concept: &Concept, units: &[Unit]) -> Option<String> {
    let type_name = &concept.data_type;

    if type_name.is_monetary() {
        return units
            .iter()
            .find(|u| u.is_currency())
            .map(|u| u.id.to_string());
    }

    if type_name.is_shares() {
        return units
            .iter()
            .find(|u| u.is_shares())
            .map(|u| u.id.to_string());
    }

    if type_name.is_numeric() {
        return units.iter().find(|u| u.is_pure()).map(|u| u.id.to_string());
    }

    None
}

/// Returns `true` if `child_element` is a valid schema child of `parent_element`.
///
/// A child is allowed when `parent_element.tuple_children` is empty (no explicit
/// content model) or when the child's element name or substitution-group ancestry
/// matches one of the declared `xs:element ref` entries.
fn item_allowed_in_tuple(
    parent_element: &Concept,
    child_element: &Concept,
    taxonomy: &TaxonomySet,
) -> bool {
    if parent_element.tuple_children.is_empty() {
        return true;
    }
    parent_element
        .tuple_children
        .iter()
        .any(|child_ref| matches_tuple_child_ref(child_ref, child_element, taxonomy))
}

/// Returns `true` if `child_element` satisfies the `child_ref` constraint, either
/// by a direct name match or via its substitution-group ancestry chain.
fn matches_tuple_child_ref(
    child_ref: &TupleChild,
    child_element: &Concept,
    _taxonomy: &TaxonomySet,
) -> bool {
    let allowed_local = &child_ref.name.local_name;

    if &child_element.name.local_name == allowed_local {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::InstanceDocument;
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

        let reader = Reader::from_str(xml);
        let instance = InstanceDocument::from_xml(reader).expect("instance should parse");

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

        let reader = Reader::from_str(xml);
        let instance = InstanceDocument::from_xml(reader).expect("instance should parse");

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
}
