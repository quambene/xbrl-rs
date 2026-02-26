use super::schema::{DeclaredAccuracy, ElementDefinition, RoleType, TaxonomySchema};
use crate::{
    error::{Result, XbrlError},
    taxonomy::linkbases::{
        calculation::{self, CalculationArc},
        definition::{self, DefinitionArc},
        label::{self, Label},
        presentation::{self, PresentationArc},
        reference::{self, Reference},
    },
};
use indexmap::IndexMap;
use log::warn;
use quick_xml::Reader;
use std::{
    borrow::Borrow,
    collections::{HashMap, HashSet, VecDeque},
    fmt, fs,
    io::BufReader,
    ops::Deref,
    path::{Path, PathBuf},
};

/// Strongly-typed key used by [`TaxonomySet`] maps previously keyed by
/// plain strings.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SchemaRefUrl(String);

impl SchemaRefUrl {
    /// Returns the key as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for SchemaRefUrl {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for SchemaRefUrl {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl Deref for SchemaRefUrl {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl AsRef<str> for SchemaRefUrl {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Borrow<str> for SchemaRefUrl {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for SchemaRefUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Concept element identifier used in label/reference maps
/// (e.g. `de-gaap-ci_bs.ass`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ConceptId(String);

impl ConceptId {
    /// Returns the concept identifier as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for ConceptId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for ConceptId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl Deref for ConceptId {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl AsRef<str> for ConceptId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Borrow<str> for ConceptId {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for ConceptId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Extended link role URI used in presentation/calculation/definition maps.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RoleUri(String);

impl RoleUri {
    /// Returns the role URI as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for RoleUri {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for RoleUri {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl Deref for RoleUri {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl AsRef<str> for RoleUri {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Borrow<str> for RoleUri {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for RoleUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The complete Discoverable Taxonomy Set (DTS).
///
/// Built by following all schema imports, includes, and linkbase references
/// starting from one or more entry point schemas.
#[derive(Debug, Default)]
pub struct TaxonomySet {
    /// The directory of the taxonomy files, used to resolve relative
    /// references.
    entry_point: PathBuf,
    /// Entry point schema URLs mapped to their resolved local paths, in the
    /// order they were passed to [`TaxonomySet::discover`].
    schema_refs: IndexMap<SchemaRefUrl, PathBuf>,
    /// All schemas in the DTS, keyed by their canonical absolute path.
    schemas: HashMap<PathBuf, TaxonomySchema>,
    /// All linkbase file paths discovered (canonical absolute paths).
    linkbase_paths: Vec<PathBuf>,
    /// Concept labels parsed from label linkbase files.
    /// Keyed by concept element ID (e.g., "de-gaap-ci_bs.ass").
    labels: HashMap<ConceptId, Vec<Label>>,
    /// Presentation arcs grouped by role URI, in the order roles were first
    /// encountered during schema discovery.
    presentations: IndexMap<RoleUri, Vec<PresentationArc>>,
    /// Calculation arcs grouped by role URI.
    calculations: HashMap<RoleUri, Vec<CalculationArc>>,
    /// Definition arcs grouped by role URI.
    definitions: HashMap<RoleUri, Vec<DefinitionArc>>,
    /// Concept references parsed from reference linkbase files.
    /// Keyed by concept element ID.
    references: HashMap<ConceptId, Vec<Reference>>,
    /// Maps each role URI to the schema file that defines it (`link:roleType`).
    role_source_schema: HashMap<RoleUri, PathBuf>,
    /// Taxonomy version extracted from the schema ref URLs.
    /// German-style taxonomies yield a date (e.g. `"2020-04-01"`); US GAAP
    /// taxonomies yield a year (e.g. `"2023"`). `None` if neither pattern
    /// matches.
    version: Option<String>,
}

impl TaxonomySet {
    #[cfg(test)]
    pub(crate) fn from_test_schemas(
        entry_point: PathBuf,
        schema_refs: Vec<String>,
        schemas: Vec<TaxonomySchema>,
    ) -> Self {
        let mut schema_refs_map: IndexMap<SchemaRefUrl, PathBuf> = IndexMap::new();
        let mut schemas_map: HashMap<PathBuf, TaxonomySchema> = HashMap::new();

        for schema in schemas {
            let path = schema.file_path.clone();
            schemas_map.insert(path, schema);
        }

        for schema_ref in schema_refs {
            let resolved = entry_point.join(strip_prefix(&schema_ref));
            schema_refs_map.insert(schema_ref.into(), resolved);
        }

        Self {
            entry_point,
            schema_refs: schema_refs_map,
            schemas: schemas_map,
            linkbase_paths: Vec::new(),
            labels: HashMap::new(),
            presentations: IndexMap::new(),
            calculations: HashMap::new(),
            definitions: HashMap::new(),
            references: HashMap::new(),
            role_source_schema: HashMap::new(),
            version: None,
        }
    }

    /// Discover the DTS starting from one or more entry point schema files.
    ///
    /// Starts from the provided `entry_point` directory and follows the given
    /// `schema_refs` to find the initial schemas. Then recursively follows all
    /// `xs:import` and `xs:include` references to discover the full set of
    /// schemas in the DTS.
    ///
    /// Automatic download of taxonomy files is not supported. All referenced
    /// files must be present locally.
    pub fn discover(schema_refs: Vec<String>, entry_point: PathBuf) -> Result<Self> {
        let version = schema_refs.first().and_then(|url| extract_version(url));

        if schema_refs.len() > 1
            && let Some(ref expected) = version
        {
            for url in schema_refs.iter().skip(1) {
                if let Some(found) = extract_version(url)
                    && &found != expected
                {
                    return Err(XbrlError::VersionMismatch {
                        expected: expected.clone(),
                        found,
                        schema_ref: url.clone(),
                    });
                }
            }
        }

        let mut visited: HashSet<PathBuf> = HashSet::new();
        let mut queue: VecDeque<PathBuf> = VecDeque::new();
        let mut schemas: HashMap<PathBuf, TaxonomySchema> = HashMap::new();
        let mut schema_order: Vec<PathBuf> = Vec::new(); // BFS discovery order
        let mut linkbase_set: HashSet<PathBuf> = HashSet::new();

        let canonical_entry_point =
            fs::canonicalize(&entry_point).map_err(|err| XbrlError::FileRead {
                path: entry_point.clone(),
                context: "entry point".to_string(),
                source: err,
            })?;

        let mut schema_refs_map: IndexMap<SchemaRefUrl, PathBuf> = IndexMap::new();
        for url in &schema_refs {
            let canonical = canonical_entry_point.join(strip_prefix(url));
            schema_refs_map.insert(url.clone().into(), canonical.clone());
            if visited.insert(canonical.clone()) {
                queue.push_back(canonical);
            }
        }

        while let Some(path) = queue.pop_front() {
            let xml_file = fs::File::open(&path).map_err(|err| XbrlError::FileRead {
                path: path.clone(),
                context: "schema".to_string(),
                source: err,
            })?;
            let mut reader = Reader::from_reader(BufReader::new(xml_file));
            let schema = TaxonomySchema::from_xml(&path, &mut reader)?;
            let schema_dir = path.parent().unwrap_or(Path::new("."));

            // Collect linkbase refs
            for lbref in &schema.linkbase_refs {
                if let Some(resolved) = resolve_local_path(schema_dir, &lbref.href) {
                    if resolved.exists() {
                        if let Ok(canonical) = std::fs::canonicalize(&resolved) {
                            linkbase_set.insert(canonical);
                        }
                    } else {
                        warn!("Linkbase not found: {}", resolved.display());
                    }
                }
            }

            // Follow xs:import schemaLocation
            for imp in &schema.imports {
                if let Some(ref loc) = imp.schema_location
                    && let Some(resolved) = resolve_local_path(schema_dir, loc)
                    && resolved.exists()
                    && let Ok(canonical) = std::fs::canonicalize(&resolved)
                    && visited.insert(canonical.clone())
                {
                    queue.push_back(canonical);
                }
            }

            // Follow xs:include schemaLocation
            for inc in &schema.includes {
                if let Some(resolved) = resolve_local_path(schema_dir, &inc.schema_location)
                    && resolved.exists()
                    && let Ok(canonical) = std::fs::canonicalize(&resolved)
                    && visited.insert(canonical.clone())
                {
                    queue.push_back(canonical);
                }
            }

            schema_order.push(path.clone());
            schemas.insert(path, schema);
        }

        let linkbase_paths: Vec<PathBuf> = linkbase_set.into_iter().collect();

        // Collect linkbase paths by type from LinkbaseRef entries, iterating schemas
        // in BFS discovery order so that linkbases are parsed in entry-point order.
        let mut label_paths: HashSet<PathBuf> = HashSet::new();
        let mut presentation_paths: Vec<PathBuf> = Vec::new();
        let mut presentation_paths_seen: HashSet<PathBuf> = HashSet::new();
        let mut calculation_paths: HashSet<PathBuf> = HashSet::new();
        let mut definition_paths: HashSet<PathBuf> = HashSet::new();
        let mut reference_paths: HashSet<PathBuf> = HashSet::new();

        for schema_path in &schema_order {
            let schema = &schemas[schema_path];
            let schema_dir = schema.file_path.parent().unwrap_or(Path::new("."));
            for lbref in &schema.linkbase_refs {
                let role = lbref.role.as_deref().unwrap_or("");
                let Some(resolved) = resolve_local_path(schema_dir, &lbref.href) else {
                    continue;
                };
                if !resolved.exists() {
                    continue;
                }
                let Ok(canonical) = std::fs::canonicalize(&resolved) else {
                    continue;
                };

                if role.contains("presentationLinkbaseRef") {
                    if presentation_paths_seen.insert(canonical.clone()) {
                        presentation_paths.push(canonical);
                    }
                } else if role.contains("labelLinkbaseRef") {
                    label_paths.insert(canonical);
                } else if role.contains("calculationLinkbaseRef") {
                    calculation_paths.insert(canonical);
                } else if role.contains("definitionLinkbaseRef") {
                    definition_paths.insert(canonical);
                } else if role.contains("referenceLinkbaseRef") {
                    reference_paths.insert(canonical);
                }
            }
        }

        // Parse label linkbases
        let mut labels: HashMap<ConceptId, Vec<Label>> = HashMap::new();
        for path in &label_paths {
            let xml_file = fs::File::open(path).map_err(|err| XbrlError::FileRead {
                path: path.clone(),
                context: "label linkbase".to_string(),
                source: err,
            })?;
            let mut reader = Reader::from_reader(BufReader::new(xml_file));
            let parsed = label::parse_label_linkbase(&mut reader)?;
            for (id, mut vals) in parsed {
                labels.entry(id.into()).or_default().append(&mut vals);
            }
        }

        // Parse presentation linkbases in BFS schema order (entry-point order preserved)
        let mut presentations: IndexMap<RoleUri, Vec<PresentationArc>> = IndexMap::new();
        for path in &presentation_paths {
            let xml_file = fs::File::open(path).map_err(|err| XbrlError::FileRead {
                path: path.clone(),
                context: "presentation linkbase".to_string(),
                source: err,
            })?;
            let mut reader = Reader::from_reader(BufReader::new(xml_file));
            let parsed = presentation::parse_presentation_linkbase(&mut reader)?;
            for (role, mut arcs) in parsed {
                presentations
                    .entry(role.into())
                    .or_default()
                    .append(&mut arcs);
            }
        }

        // Parse calculation linkbases
        let mut calculations: HashMap<RoleUri, Vec<CalculationArc>> = HashMap::new();
        for path in &calculation_paths {
            let xml_file = fs::File::open(path).map_err(|err| XbrlError::FileRead {
                path: path.clone(),
                context: "calculation linkbase".to_string(),
                source: err,
            })?;
            let mut reader = Reader::from_reader(BufReader::new(xml_file));
            let parsed = calculation::parse_calculation_linkbase(&mut reader)?;

            for (role, mut arcs) in parsed {
                calculations
                    .entry(role.into())
                    .or_default()
                    .append(&mut arcs);
            }
        }

        // Parse definition linkbases
        let mut definitions: HashMap<RoleUri, Vec<DefinitionArc>> = HashMap::new();
        for path in &definition_paths {
            let xml_file = fs::File::open(path).map_err(|err| XbrlError::FileRead {
                path: path.clone(),
                context: "definition linkbase".to_string(),
                source: err,
            })?;
            let mut reader = Reader::from_reader(BufReader::new(xml_file));
            let parsed = definition::parse_definition_linkbase(&mut reader)?;

            for (role, mut arcs) in parsed {
                definitions
                    .entry(role.into())
                    .or_default()
                    .append(&mut arcs);
            }
        }

        // Build role → source schema map
        let mut role_source_schema: HashMap<RoleUri, PathBuf> = HashMap::new();
        for (path, schema) in &schemas {
            for role_type in &schema.role_types {
                role_source_schema
                    .entry(role_type.role_uri.clone().into())
                    .or_insert_with(|| path.clone());
            }
        }

        // Parse reference linkbases
        let mut references: HashMap<ConceptId, Vec<Reference>> = HashMap::new();
        for path in &reference_paths {
            let xml_file = fs::File::open(path).map_err(|err| XbrlError::FileRead {
                path: path.clone(),
                context: "reference linkbase".to_string(),
                source: err,
            })?;
            let mut reader = Reader::from_reader(BufReader::new(xml_file));
            let parsed = reference::parse_reference_linkbase(&mut reader)?;

            for (id, mut vals) in parsed {
                references.entry(id.into()).or_default().append(&mut vals);
            }
        }

        Ok(TaxonomySet {
            entry_point,
            schema_refs: schema_refs_map,
            schemas,
            linkbase_paths,
            labels,
            presentations,
            calculations,
            definitions,
            references,
            role_source_schema,
            version,
        })
    }

    /// Get the entry point directory of taxonomy files.
    pub fn entry_point(&self) -> &Path {
        &self.entry_point
    }

    /// Get the taxonomy version, if present.
    ///
    /// Returns a date string for German-style taxonomies (e.g. `"2020-04-01"`)
    /// or a year string for US GAAP (e.g. `"2023"`).
    pub fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }

    /// Get the entry point schema URLs and their resolved local paths, in declaration order.
    pub fn schema_refs(&self) -> &IndexMap<SchemaRefUrl, PathBuf> {
        &self.schema_refs
    }

    /// Get the resolved local path of the schema file that defines the given role URI.
    pub fn role_source_path(&self, role: &str) -> Option<&Path> {
        self.role_source_schema.get(role).map(PathBuf::as_path)
    }

    /// Get all schemas in the DTS.
    pub fn schemas(&self) -> &HashMap<PathBuf, TaxonomySchema> {
        &self.schemas
    }

    /// Get all discovered linkbase file paths.
    pub fn linkbase_paths(&self) -> &[PathBuf] {
        &self.linkbase_paths
    }

    /// Get all element definitions across all schemas in the DTS.
    pub fn elements(&self) -> Vec<&ElementDefinition> {
        self.schemas.values().flat_map(|s| &s.elements).collect()
    }

    /// Get all role type definitions across all schemas in the DTS.
    pub fn role_types(&self) -> Vec<&RoleType> {
        self.schemas.values().flat_map(|s| &s.role_types).collect()
    }

    /// Find an element definition by name across all schemas.
    pub fn find_element(&self, name: &str) -> Option<&ElementDefinition> {
        self.schemas
            .values()
            .flat_map(|s| &s.elements)
            .find(|e| e.name == name)
    }

    /// Find an element definition by its ID attribute (e.g., `de-gaap-ci_bs.ass`).
    pub fn find_element_by_id(&self, id: &str) -> Option<&ElementDefinition> {
        self.schemas
            .values()
            .flat_map(|s| &s.elements)
            .find(|e| e.id.as_deref() == Some(id))
    }

    /// Find the tuple element that directly contains the given concept, if any.
    ///
    /// A concept belongs to a tuple when its `substitutionGroup` points to an abstract
    /// head element that is listed as an `xs:element[@ref]` inside the tuple's inline
    /// `xs:complexType`. Only one level of indirection is resolved (direct parent tuple).
    pub fn find_parent_tuple(&self, concept_id: &str) -> Option<&ElementDefinition> {
        let element = self.find_element_by_id(concept_id)?;
        let parent_qname = element.substitution_group.as_deref()?;

        // Skip standard XBRL substitution groups — those don't belong to a custom tuple
        let local = parent_qname.rsplit(':').next().unwrap_or(parent_qname);
        if local == "item" || local == "tuple" {
            return None;
        }

        // Find a tuple whose xs:complexType references this abstract head QName
        self.schemas
            .values()
            .flat_map(|s| &s.elements)
            .find(|e| e.is_tuple() && e.tuple_children.iter().any(|c| c == parent_qname))
    }

    /// Find all tuple ancestor IDs from root tuple to direct parent tuple.
    pub fn tuple_ancestor_ids(&self, concept_id: &str) -> Vec<String> {
        let mut ancestors = Vec::new();
        let mut current = concept_id.to_string();
        let mut seen = HashSet::new();

        while seen.insert(current.clone()) {
            let Some(parent_tuple) = self.find_parent_tuple(&current) else {
                break;
            };
            let Some(parent_id) = parent_tuple.id.as_ref() else {
                break;
            };

            ancestors.push(parent_id.clone());
            current = parent_id.clone();
        }

        ancestors.reverse();
        ancestors
    }

    pub fn is_type_derived_from(&self, type_name: &str, target_base_local_name: &str) -> bool {
        let mut current = type_name.to_string();
        let mut seen = HashSet::new();

        loop {
            let current_local = current.rsplit(':').next().unwrap_or(current.as_str());
            if current_local == target_base_local_name {
                return true;
            }

            if !seen.insert(current.clone()) {
                return false;
            }

            let Some(next) = self.find_type_base(current_local) else {
                return false;
            };
            current = next;
        }
    }

    fn find_type_base(&self, type_local_name: &str) -> Option<String> {
        self.schemas
            .values()
            .find_map(|schema| schema.type_bases.get(type_local_name).cloned())
    }

    pub fn type_declared_accuracy(&self, type_name: &str) -> DeclaredAccuracy {
        let mut current = type_name.to_string();
        let mut seen = HashSet::new();

        loop {
            let current_local = current.rsplit(':').next().unwrap_or(current.as_str());

            let declared = self
                .schemas
                .values()
                .find_map(|schema| schema.type_declared_accuracy.get(current_local).cloned());

            if let Some(acc) = declared
                && (acc.decimals.is_some() || acc.precision.is_some())
            {
                return acc;
            }

            if !seen.insert(current.clone()) {
                return DeclaredAccuracy::default();
            }

            let Some(next) = self.find_type_base(current_local) else {
                return DeclaredAccuracy::default();
            };
            current = next;
        }
    }

    pub fn type_has_fixed_accuracy(&self, type_name: &str) -> bool {
        let acc = self.type_declared_accuracy(type_name);
        acc.decimals.is_some() || acc.precision.is_some()
    }

    /// Map an element ID to the qualified concept name used in instance facts.
    ///
    /// For example, `de-gaap-ci_bs.ass` becomes `de-gaap-ci:bs.ass`.
    /// Returns `None` if the element is not found or its schema has no
    /// target namespace with a matching prefix.
    pub fn qualified_name(&self, element_id: &str) -> Option<String> {
        for schema in self.schemas.values() {
            if let Some(elem) = schema
                .elements
                .iter()
                .find(|e| e.id.as_deref() == Some(element_id))
            {
                let target_ns = schema.target_namespace.as_deref()?;
                let prefix = schema
                    .namespaces
                    .iter()
                    .find(|(_, uri)| uri.as_str() == target_ns)
                    .map(|(prefix, _)| prefix)?;
                return Some(format!("{prefix}:{}", elem.name));
            }
        }
        None
    }

    /// Get all concept labels.
    pub fn labels(&self) -> &HashMap<ConceptId, Vec<Label>> {
        &self.labels
    }

    /// Get labels for a specific concept by its element ID (e.g., "de-gaap-ci_bs.ass").
    pub fn labels_for(&self, concept_id: &str) -> Option<&[Label]> {
        self.labels.get(concept_id).map(|v| v.as_slice())
    }

    /// Get all presentation arcs grouped by role URI, in entry-point discovery order.
    pub fn presentations(&self) -> &IndexMap<RoleUri, Vec<PresentationArc>> {
        &self.presentations
    }

    /// Get presentation arcs for a specific role URI.
    pub fn presentation_arcs(&self, role: &str) -> Option<&[PresentationArc]> {
        self.presentations.get(role).map(|v| v.as_slice())
    }

    /// Get all calculation arcs grouped by role URI.
    pub fn calculations(&self) -> &HashMap<RoleUri, Vec<CalculationArc>> {
        &self.calculations
    }

    /// Get calculation arcs for a specific role URI.
    pub fn calculation_arcs(&self, role: &str) -> Option<&[CalculationArc]> {
        self.calculations.get(role).map(|v| v.as_slice())
    }

    /// Get all definition arcs grouped by role URI.
    pub fn definitions(&self) -> &HashMap<RoleUri, Vec<DefinitionArc>> {
        &self.definitions
    }

    /// Get definition arcs for a specific role URI.
    pub fn definition_arcs(&self, role: &str) -> Option<&[DefinitionArc]> {
        self.definitions.get(role).map(|v| v.as_slice())
    }

    /// Get all concept references.
    pub fn references(&self) -> &HashMap<ConceptId, Vec<Reference>> {
        &self.references
    }

    /// Get references for a specific concept by its element ID.
    pub fn references_for(&self, concept_id: &str) -> Option<&[Reference]> {
        self.references.get(concept_id).map(|v| v.as_slice())
    }

    /// Get a schema by its target namespace.
    pub fn schema_by_namespace(&self, namespace: &str) -> Option<&TaxonomySchema> {
        self.schemas
            .values()
            .find(|s| s.target_namespace.as_deref() == Some(namespace))
    }
}

#[cfg(test)]
impl TaxonomySet {
    /// Insert a presentation arc for a role URI. Used in unit tests.
    pub fn add_presentation_arc(&mut self, role: String, arc: PresentationArc) {
        self.presentations.entry(role.into()).or_default().push(arc);
    }

    /// Insert a label for a concept ID. Used in unit tests.
    pub fn add_label(&mut self, concept_id: String, label: Label) {
        self.labels
            .entry(concept_id.into())
            .or_default()
            .push(label);
    }
}

/// Resolve a relative reference to a local file path.
/// Returns `None` for HTTP/HTTPS URLs.
fn resolve_local_path(base_dir: &Path, reference: &str) -> Option<PathBuf> {
    if reference.contains("://") {
        return None;
    }
    Some(base_dir.join(reference))
}

/// Extracts the taxonomy version from a schema ref URL.
///
/// Two patterns are recognized:
/// - German style: the first path segment ends with `YYYY-MM-DD`
///   (e.g. `de-gcd-2020-04-01` → `"2020-04-01"`)
/// - US GAAP style: a standalone 4-digit year appears as a path segment
///   (e.g. `us-gaap/2023/elts/…` → `"2023"`)
///
/// Returns `None` if neither pattern matches.
fn extract_version(url: &str) -> Option<String> {
    let stripped = strip_prefix(url);

    // German style: first segment ends with YYYY-MM-DD (e.g. de-gcd-2020-04-01).
    if let Some(segment) = stripped.split('/').next() {
        let parts: Vec<&str> = segment.split('-').collect();
        if parts.len() >= 3 {
            let tail = &parts[parts.len() - 3..];
            if tail[0].len() == 4
                && tail[1].len() == 2
                && tail[2].len() == 2
                && tail.iter().all(|p| p.chars().all(|c| c.is_ascii_digit()))
            {
                return Some(tail.join("-"));
            }
        }
    }

    // US GAAP style: standalone 4-digit year segment (e.g. us-gaap/2023/elts/…).
    for segment in stripped.split('/') {
        if segment.len() == 4 && segment.chars().all(|c| c.is_ascii_digit()) {
            return Some(segment.to_string());
        }
    }

    None
}

/// Strips the URL scheme, host, and leading `/taxonomies/` segment to
/// produce paths suitable for joining with a local taxonomy directory.
///
/// For example:
/// `http://www.xbrl.de/taxonomies/de-gcd-2020-04-01/de-gcd-2020-04-01-shell.xsd`
/// becomes `de-gcd-2020-04-01/de-gcd-2020-04-01-shell.xsd`.
pub fn strip_prefix(href: &str) -> &str {
    let path = href
        .find("://")
        .and_then(|i| href[i + 3..].find('/'))
        .map(|i| &href[href.find("://").unwrap() + 3 + i..])
        .unwrap_or(href);
    // Strip leading "/taxonomies/" if present
    path.strip_prefix("/taxonomies/")
        .or_else(|| path.strip_prefix("/"))
        .unwrap_or(path)
}
