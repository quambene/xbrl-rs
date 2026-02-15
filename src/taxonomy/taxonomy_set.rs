use super::{
    calculation::{self, CalculationArc},
    definition::{self, DefinitionArc},
    label::{self, Label},
    presentation::{self, PresentationArc},
    reference::{self, Reference},
    schema::{ElementDefinition, RoleType, TaxonomySchema},
};
use crate::XbrlInstance;
use anyhow::{Context, Result};
use log::warn;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::{Path, PathBuf},
};

/// An entry point schema of a DTS, combining its public URL with a local path.
#[derive(Debug, Clone)]
pub struct EntryPoint {
    /// The public URL used in `link:schemaRef` elements
    /// (e.g., `http://www.xbrl.de/taxonomies/de-gcd-2020-04-01/de-gcd-2020-04-01-shell.xsd`).
    pub href: String,
    /// The local file system path to the schema file.
    pub local_path: PathBuf,
}

impl EntryPoint {
    pub fn new(href: impl Into<String>, local_path: impl Into<PathBuf>) -> Self {
        Self {
            href: href.into(),
            local_path: local_path.into(),
        }
    }
}

/// The complete Discoverable Taxonomy Set (DTS).
///
/// Built by following all schema imports, includes, and linkbase references
/// starting from one or more entry point schemas.
#[derive(Debug)]
pub struct TaxonomySet {
    /// The entry point schemas of this DTS.
    entry_points: Vec<EntryPoint>,
    /// All schemas in the DTS, keyed by their canonical absolute path.
    schemas: HashMap<PathBuf, TaxonomySchema>,
    /// All linkbase file paths discovered (canonical absolute paths).
    linkbase_paths: Vec<PathBuf>,
    /// Concept labels parsed from label linkbase files.
    /// Keyed by concept element ID (e.g., "de-gaap-ci_bs.ass").
    labels: HashMap<String, Vec<Label>>,
    /// Presentation arcs grouped by role URI.
    presentations: HashMap<String, Vec<PresentationArc>>,
    /// Calculation arcs grouped by role URI.
    calculations: HashMap<String, Vec<CalculationArc>>,
    /// Definition arcs grouped by role URI.
    definitions: HashMap<String, Vec<DefinitionArc>>,
    /// Concept references parsed from reference linkbase files.
    /// Keyed by concept element ID.
    references: HashMap<String, Vec<Reference>>,
}

impl TaxonomySet {
    /// Discover the DTS starting from one or more entry point schema files.
    pub fn discover(entry_points: &[EntryPoint]) -> Result<Self> {
        let mut visited: HashSet<PathBuf> = HashSet::new();
        let mut queue: VecDeque<PathBuf> = VecDeque::new();
        let mut schemas: HashMap<PathBuf, TaxonomySchema> = HashMap::new();
        let mut linkbase_set: HashSet<PathBuf> = HashSet::new();

        // Seed the queue with entry points
        for entry in entry_points {
            let canonical = std::fs::canonicalize(&entry.local_path).with_context(|| {
                format!(
                    "Failed to resolve entry point: {}",
                    entry.local_path.display()
                )
            })?;
            if visited.insert(canonical.clone()) {
                queue.push_back(canonical);
            }
        }

        while let Some(path) = queue.pop_front() {
            let xml_content = std::fs::read_to_string(&path)
                .with_context(|| format!("Failed to read schema: {}", path.display()))?;

            let schema = TaxonomySchema::parse(&path, &xml_content)
                .with_context(|| format!("Failed to parse schema: {}", path.display()))?;

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

            schemas.insert(path, schema);
        }

        let linkbase_paths: Vec<PathBuf> = linkbase_set.into_iter().collect();

        // Collect linkbase paths by type from LinkbaseRef entries.
        let mut label_paths: HashSet<PathBuf> = HashSet::new();
        let mut presentation_paths: HashSet<PathBuf> = HashSet::new();
        let mut calculation_paths: HashSet<PathBuf> = HashSet::new();
        let mut definition_paths: HashSet<PathBuf> = HashSet::new();
        let mut reference_paths: HashSet<PathBuf> = HashSet::new();

        for schema in schemas.values() {
            let schema_dir = schema.file_path.parent().unwrap_or(Path::new("."));
            for lbref in &schema.linkbase_refs {
                let role = lbref.role.as_deref().unwrap_or("");
                let set = if role.contains("labelLinkbaseRef") {
                    &mut label_paths
                } else if role.contains("presentationLinkbaseRef") {
                    &mut presentation_paths
                } else if role.contains("calculationLinkbaseRef") {
                    &mut calculation_paths
                } else if role.contains("definitionLinkbaseRef") {
                    &mut definition_paths
                } else if role.contains("referenceLinkbaseRef") {
                    &mut reference_paths
                } else {
                    continue;
                };

                if let Some(resolved) = resolve_local_path(schema_dir, &lbref.href)
                    && resolved.exists()
                    && let Ok(canonical) = std::fs::canonicalize(&resolved)
                {
                    set.insert(canonical);
                }
            }
        }

        // Parse label linkbases
        let mut labels: HashMap<String, Vec<Label>> = HashMap::new();
        for path in &label_paths {
            let xml = std::fs::read_to_string(path)
                .with_context(|| format!("Failed to read label linkbase: {}", path.display()))?;
            let parsed = label::parse_label_linkbase(&xml)
                .with_context(|| format!("Failed to parse label linkbase: {}", path.display()))?;
            for (id, mut vals) in parsed {
                labels.entry(id).or_default().append(&mut vals);
            }
        }

        // Parse presentation linkbases
        let mut presentations: HashMap<String, Vec<PresentationArc>> = HashMap::new();
        for path in &presentation_paths {
            let xml = std::fs::read_to_string(path).with_context(|| {
                format!("Failed to read presentation linkbase: {}", path.display())
            })?;
            let parsed = presentation::parse_presentation_linkbase(&xml).with_context(|| {
                format!("Failed to parse presentation linkbase: {}", path.display())
            })?;
            for (role, mut arcs) in parsed {
                presentations.entry(role).or_default().append(&mut arcs);
            }
        }

        // Parse calculation linkbases
        let mut calculations: HashMap<String, Vec<CalculationArc>> = HashMap::new();
        for path in &calculation_paths {
            let xml = std::fs::read_to_string(path).with_context(|| {
                format!("Failed to read calculation linkbase: {}", path.display())
            })?;
            let parsed = calculation::parse_calculation_linkbase(&xml).with_context(|| {
                format!("Failed to parse calculation linkbase: {}", path.display())
            })?;
            for (role, mut arcs) in parsed {
                calculations.entry(role).or_default().append(&mut arcs);
            }
        }

        // Parse definition linkbases
        let mut definitions: HashMap<String, Vec<DefinitionArc>> = HashMap::new();
        for path in &definition_paths {
            let xml = std::fs::read_to_string(path).with_context(|| {
                format!("Failed to read definition linkbase: {}", path.display())
            })?;
            let parsed = definition::parse_definition_linkbase(&xml).with_context(|| {
                format!("Failed to parse definition linkbase: {}", path.display())
            })?;
            for (role, mut arcs) in parsed {
                definitions.entry(role).or_default().append(&mut arcs);
            }
        }

        // Parse reference linkbases
        let mut references: HashMap<String, Vec<Reference>> = HashMap::new();
        for path in &reference_paths {
            let xml = std::fs::read_to_string(path).with_context(|| {
                format!("Failed to read reference linkbase: {}", path.display())
            })?;
            let parsed = reference::parse_reference_linkbase(&xml).with_context(|| {
                format!("Failed to parse reference linkbase: {}", path.display())
            })?;
            for (id, mut vals) in parsed {
                references.entry(id).or_default().append(&mut vals);
            }
        }

        let entry_points = entry_points.to_vec();

        Ok(TaxonomySet {
            entry_points,
            schemas,
            linkbase_paths,
            labels,
            presentations,
            calculations,
            definitions,
            references,
        })
    }

    /// Create an empty [`XbrlInstance`] pre-populated with schema references
    /// and namespace declarations from this DTS.
    pub fn create_instance(&self) -> XbrlInstance {
        let mut instance = XbrlInstance::new();

        for entry in &self.entry_points {
            instance.add_schema_ref(entry.href.clone());
        }

        for schema in self.schemas.values() {
            for (prefix, uri) in &schema.namespaces {
                instance.add_namespace(prefix.clone(), uri.clone());
            }
        }

        instance
    }

    /// Get the entry point schemas.
    pub fn entry_points(&self) -> &[EntryPoint] {
        &self.entry_points
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

    /// Map an element ID to the qualified concept name used in instance facts.
    ///
    /// For example, `de-gaap-ci_bs.ass` becomes `de-gaap-ci:bs.ass`.
    /// Returns `None` if the element is not found or its schema has no
    /// target namespace with a matching prefix.
    pub fn qualified_name(&self, element_id: &str) -> Option<String> {
        for schema in self.schemas.values() {
            if let Some(elem) = schema.elements.iter().find(|e| e.id.as_deref() == Some(element_id)) {
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
    pub fn labels(&self) -> &HashMap<String, Vec<Label>> {
        &self.labels
    }

    /// Get labels for a specific concept by its element ID (e.g., "de-gaap-ci_bs.ass").
    pub fn labels_for(&self, concept_id: &str) -> Option<&[Label]> {
        self.labels.get(concept_id).map(|v| v.as_slice())
    }

    /// Get all presentation arcs grouped by role URI.
    pub fn presentations(&self) -> &HashMap<String, Vec<PresentationArc>> {
        &self.presentations
    }

    /// Get presentation arcs for a specific role URI.
    pub fn presentation_arcs(&self, role: &str) -> Option<&[PresentationArc]> {
        self.presentations.get(role).map(|v| v.as_slice())
    }

    /// Get all calculation arcs grouped by role URI.
    pub fn calculations(&self) -> &HashMap<String, Vec<CalculationArc>> {
        &self.calculations
    }

    /// Get calculation arcs for a specific role URI.
    pub fn calculation_arcs(&self, role: &str) -> Option<&[CalculationArc]> {
        self.calculations.get(role).map(|v| v.as_slice())
    }

    /// Get all definition arcs grouped by role URI.
    pub fn definitions(&self) -> &HashMap<String, Vec<DefinitionArc>> {
        &self.definitions
    }

    /// Get definition arcs for a specific role URI.
    pub fn definition_arcs(&self, role: &str) -> Option<&[DefinitionArc]> {
        self.definitions.get(role).map(|v| v.as_slice())
    }

    /// Get all concept references.
    pub fn references(&self) -> &HashMap<String, Vec<Reference>> {
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

/// Resolve a relative reference to a local file path.
/// Returns `None` for HTTP/HTTPS URLs.
fn resolve_local_path(base_dir: &Path, reference: &str) -> Option<PathBuf> {
    if reference.contains("://") {
        return None;
    }
    Some(base_dir.join(reference))
}
