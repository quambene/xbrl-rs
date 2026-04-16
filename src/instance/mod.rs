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
    ExpandedName, NamespacePrefix, NamespaceUri, PresentationArc, TaxonomySet,
    error::Result,
    taxonomy::{Concept, ElementParticle, Particle, PeriodType},
    validation::{self, ValidationResult},
};
pub use context::{Context, ContextId, EntityIdentifier, Period};
pub use fact::{Decimals, Fact, ItemFact, TupleFact};
pub use footnote::{FootnoteArc, FootnoteLink, FootnoteLocator, FootnoteResource};
pub use parser::InstanceParser;
use quick_xml::{Reader, Writer};
use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
    fs::File,
    io,
    path::Path,
};
pub use unit::{Unit, UnitId};
pub use view::{DocumentView, SectionView, TreeNode};

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
        namespaces: HashMap<NamespacePrefix, NamespaceUri>,
        instant_context: Context,
        duration_context: Context,
        units: &[Unit],
    ) -> Self {
        let mut instance = Self::default();

        for (prefix, uri) in namespaces {
            instance.add_namespace(prefix, uri);
        }

        for schema_url in taxonomy.schema_refs().keys() {
            instance.add_schema_ref(schema_url.to_string());
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
        let mut recursion_path: HashSet<ExpandedName> = HashSet::new();
        let mut emitted_items: HashSet<ExpandedName> = HashSet::new();
        let mut emitted_tuples: HashSet<ExpandedName> = HashSet::new();

        for arcs in taxonomy.presentations().values() {
            let mut arc_index: HashMap<&ExpandedName, Vec<&PresentationArc>> = HashMap::new();

            for arc in arcs {
                arc_index.entry(&arc.from).or_default().push(arc);
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
            let mut seeded_nodes: HashSet<&ExpandedName> = HashSet::new();

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

            let mut remaining_nodes = arcs
                .iter()
                .flat_map(|arc| [&arc.from, &arc.to])
                .filter(|concept_name| !seeded_nodes.contains(concept_name))
                .collect::<Vec<_>>();
            remaining_nodes.sort_unstable();
            remaining_nodes.dedup();

            for concept_name in remaining_nodes {
                let mut hoisted: Vec<Fact> = Vec::new();
                Self::populate_from_tree(
                    &arc_index,
                    concept_name,
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

    /// Convenience wrapper for [`DocumentView::build`] using this instance's item facts.
    pub fn view<'a>(&self, taxonomy: &'a TaxonomySet) -> DocumentView<'a> {
        let item_facts = self.item_facts();
        DocumentView::build(&item_facts, taxonomy)
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
        let mut writer = Writer::new(writer);
        writer::write_xml(&mut writer, self)
    }

    /// Serialize this instance to an XBRL XML document using an XML writer.
    pub fn to_xml_writer<W>(&self, writer: &mut Writer<W>) -> Result<()>
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
        arc_index: &HashMap<&ExpandedName, Vec<&PresentationArc>>,
        concept_name: &ExpandedName,
        taxonomy: &TaxonomySet,
        instant_ctx: &ContextId,
        duration_ctx: &ContextId,
        units: &[Unit],
        facts: &mut Vec<Fact>,
        emitted_items: &mut HashSet<ExpandedName>,
        emitted_tuples: &mut HashSet<ExpandedName>,
        recursion_path: &mut HashSet<ExpandedName>,
        parent_tuple_element: Option<&Concept>,
        hoisted: &mut Vec<Fact>,
    ) {
        if !recursion_path.insert(concept_name.clone()) {
            return; // cycle guard within current recursion branch
        }

        // Children are already sorted by `order`.
        let children = arc_index
            .get(concept_name)
            .map(Vec::as_slice)
            .unwrap_or(&[]);

        if let Some(concept) = taxonomy.find_concept(concept_name) {
            if concept.is_tuple() && !concept.is_abstract {
                if emitted_tuples.insert(concept_name.clone()) {
                    facts.push(Fact::Tuple(TupleFact::new(concept.name.clone())));

                    let tuple_children = match facts.last_mut() {
                        Some(Fact::Tuple(tuple)) => tuple.children_mut(),
                        _ => unreachable!(),
                    };

                    for arc in children {
                        Self::populate_from_tree(
                            arc_index,
                            &arc.to,
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
                recursion_path.remove(concept_name);
                return;
            }

            if !concept.is_abstract
                && let Some(ref period_type) = concept.period_type
            {
                let context_ref = match period_type {
                    PeriodType::Duration => duration_ctx,
                    PeriodType::Instant => instant_ctx,
                };

                if emitted_items.insert(concept_name.clone()) {
                    let mut fact = ItemFact::new(
                        None,
                        concept.name.clone(),
                        context_ref.to_string(),
                        unit_ref_for_concept(concept, units),
                        String::new(),
                        true,
                        None,
                        None,
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
                &arc.to,
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

        recursion_path.remove(concept_name);
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
/// A child is allowed when `parent_element.content_model` is `None` (no explicit
/// content model) or when the child's element name or substitution-group ancestry
/// matches an element particle in the content model.
fn item_allowed_in_tuple(
    parent_element: &Concept,
    child_element: &Concept,
    taxonomy: &TaxonomySet,
) -> bool {
    let Some(model) = &parent_element.content_model else {
        return true;
    };
    matches_particle_model(model, child_element, taxonomy)
}

/// Returns `true` if `child_element` satisfies any element particle in `model`,
/// either by a direct name match or via substitution-group ancestry.
fn matches_particle_model(
    model: &Particle,
    child_element: &Concept,
    taxonomy: &TaxonomySet,
) -> bool {
    model
        .elements()
        .iter()
        .any(|element_particle| matches_element_particle(element_particle, child_element, taxonomy))
}

/// Returns `true` if `child_element` satisfies the element particle, either
/// by a direct name match or via its substitution-group ancestry chain.
fn matches_element_particle(
    element_particle: &ElementParticle,
    child_element: &Concept,
    taxonomy: &TaxonomySet,
) -> bool {
    let allowed_local = match element_particle {
        ElementParticle::Ref(qname) => qname.local_name.as_str(),
        ElementParticle::Decl(declaration) => declaration.name.as_str(),
    };

    if child_element.name.local_name == allowed_local {
        return true;
    }

    // Walk the substitution group ancestry: if the child's substitution group
    // (or any ancestor in the chain) matches the declared element particle, the
    // element is a valid substitute.
    let mut current = child_element;

    loop {
        let parent_substitution_group = &current.substitution_group.original;

        if parent_substitution_group.local_name == allowed_local {
            return true;
        }

        match taxonomy.find_concept(parent_substitution_group) {
            Some(parent) => current = parent,
            None => break,
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::{InstanceDocument, TaxonomySet};

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
}
