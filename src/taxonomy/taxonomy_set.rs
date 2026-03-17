use super::schema::TaxonomySchema;
use crate::{
    CalculationArc, ConceptId, DefinitionArc, ExpandedName, Label, PresentationArc, Reference,
    RoleUri, SchemaRefUrl,
    error::{Result, XbrlError},
    taxonomy::{
        RoleType,
        linkbases::{
            parser::{LinkbaseParser, RawLinkbases},
            resolver::{self, Linkbases},
        },
        schema::Concept,
    },
};
use indexmap::{IndexMap, IndexSet};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    fs, io,
    path::{Path, PathBuf},
};

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
    /// Resolved linkbase data merged from all linkbase files.
    linkbases: Linkbases,
    /// Maps each role URI to the schema file that defines it (`link:roleType`).
    role_source_schema: HashMap<RoleUri, PathBuf>,
    /// Taxonomy version extracted from the schema ref URLs.
    /// German-style taxonomies yield a date (e.g. `"2020-04-01"`); US GAAP
    /// taxonomies yield a year (e.g. `"2023"`). `None` if neither pattern
    /// matches.
    version: Option<String>,
}

impl TaxonomySet {
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
        let mut linkbase_set: IndexSet<PathBuf> = IndexSet::new();

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
            let schema = TaxonomySchema::from_file(&path)?;
            let schema_dir = path.parent().unwrap_or(Path::new("."));

            // Collect linkbase refs
            for lbref in &schema.linkbase_refs {
                if let Some(resolved) = resolve_local_path(schema_dir, &lbref.href) {
                    if !resolved.exists() {
                        return Err(XbrlError::FileRead {
                            path: resolved,
                            context: "linkbase referenced from schema".to_string(),
                            source: io::Error::new(
                                io::ErrorKind::NotFound,
                                "referenced linkbase file does not exist",
                            ),
                        });
                    }
                    let canonical =
                        fs::canonicalize(&resolved).map_err(|err| XbrlError::FileRead {
                            path: resolved.clone(),
                            context: "linkbase referenced from schema".to_string(),
                            source: err,
                        })?;
                    linkbase_set.insert(canonical);
                }
            }

            // Follow xs:import schemaLocation
            for import in &schema.imports {
                if let Some(ref location) = import.schema_location
                    && let Some(resolved) = resolve_local_path(schema_dir, location)
                    && resolved.exists()
                    && let Ok(canonical) = std::fs::canonicalize(&resolved)
                    && visited.insert(canonical.clone())
                {
                    queue.push_back(canonical);
                }
            }

            // Follow xs:include schemaLocation
            for include in &schema.includes {
                if let Some(resolved) = resolve_local_path(schema_dir, &include.schema_location)
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
        let mut linkbases = RawLinkbases::default();

        for path in &linkbase_paths {
            let mut parser = LinkbaseParser::from_file(path)?;
            parser.parse_linkbase(&mut linkbases)?;
        }

        let concepts_by_id = schemas
            .values()
            .flat_map(|schema| &schema.concepts)
            .filter_map(|concept| concept.id.clone().map(|id| (ConceptId::from(id), concept)))
            .collect::<HashMap<_, _>>();

        let linkbases = resolver::resolve_linkbases(linkbases, &concepts_by_id)?;

        // Build role → source schema map
        let mut role_source_schema: HashMap<RoleUri, PathBuf> = HashMap::new();
        for (path, schema) in &schemas {
            for role_type in &schema.role_types {
                role_source_schema
                    .entry(role_type.role_uri.clone())
                    .or_insert_with(|| path.clone());
            }
        }

        let taxonomy = TaxonomySet {
            entry_point,
            schema_refs: schema_refs_map,
            schemas,
            linkbase_paths,
            linkbases,
            role_source_schema,
            version,
        };

        Ok(taxonomy)
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
    pub fn elements(&self) -> Vec<&Concept> {
        self.schemas.values().flat_map(|s| &s.concepts).collect()
    }

    /// Get all role type definitions across all schemas in the DTS.
    pub fn role_types(&self) -> Vec<&RoleType> {
        self.schemas.values().flat_map(|s| &s.role_types).collect()
    }

    /// Find an element definition by name across all schemas.
    pub fn find_concept(&self, name: &ExpandedName) -> Option<&Concept> {
        self.schemas
            .values()
            .flat_map(|schema| &schema.concepts)
            .find(|concept| &concept.name == name)
    }

    /// Find an element definition by its ID attribute (e.g., `de-gaap-ci_bs.ass`).
    pub fn find_concept_by_id(&self, id: &str) -> Option<&Concept> {
        self.schemas
            .values()
            .flat_map(|schema| &schema.concepts)
            .find(|concept| concept.id.as_deref() == Some(id))
    }

    /// Find the tuple element that directly contains the given concept, if any.
    ///
    /// A concept belongs to a tuple when its `substitutionGroup` points to an abstract
    /// head element that is listed as an `xs:element[@ref]` inside the tuple's inline
    /// `xs:complexType`. Only one level of indirection is resolved (direct parent tuple).
    pub fn find_parent_tuple(&self, concept_id: &str) -> Option<&Concept> {
        let element = self.find_concept_by_id(concept_id)?;
        // Find a tuple whose xs:complexType references this child element QName
        self.schemas
            .values()
            .flat_map(|schema| &schema.concepts)
            .find(|concept| {
                concept.is_tuple()
                    && concept
                        .tuple_children
                        .iter()
                        .any(|tuple_child| tuple_child.name.local_name == element.name.local_name)
            })
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

            ancestors.push(parent_tuple.id.clone().unwrap_or_default().to_string());
            current = parent_tuple.id.clone().unwrap_or_default().to_string();
        }

        ancestors.reverse();
        ancestors
    }

    /// Map an element ID from a schema to the qualified concept name used in
    /// instance facts.
    ///
    /// For example, `de-gaap-ci_bs.ass` becomes `de-gaap-ci:bs.ass`. Returns
    /// `None` if the element is not found.
    pub fn qualified_name(&self, element_id: &str) -> Option<ExpandedName> {
        for schema in self.schemas.values() {
            if let Some(concept) = schema
                .concepts
                .iter()
                .find(|concept| concept.id.as_deref() == Some(element_id))
            {
                return Some(concept.name.clone());
            }
        }

        None
    }

    /// Get all concept labels.
    pub fn labels_map(&self) -> &HashMap<ExpandedName, Vec<Label>> {
        &self.linkbases.labels
    }

    /// Get labels for a specific concept by its name.
    pub fn labels(&self, concept_name: &ExpandedName) -> Option<&[Label]> {
        self.linkbases
            .labels
            .get(concept_name)
            .map(|labels| labels.as_slice())
    }

    /// Get all presentation arcs grouped by role URI, in entry-point discovery order.
    pub fn presentations(&self) -> &IndexMap<RoleUri, Vec<PresentationArc>> {
        &self.linkbases.presentations
    }

    /// Get presentation arcs for a specific role URI.
    pub fn presentation_arcs(&self, role: &str) -> Option<&[PresentationArc]> {
        self.linkbases
            .presentations
            .get(role)
            .map(|arcs| arcs.as_slice())
    }

    /// Get all calculation arcs grouped by role URI.
    pub fn calculations(&self) -> &HashMap<RoleUri, Vec<CalculationArc>> {
        &self.linkbases.calculations
    }

    /// Get calculation arcs for a specific role URI.
    pub fn calculation_arcs(&self, role: &str) -> Option<&[CalculationArc]> {
        self.linkbases
            .calculations
            .get(role)
            .map(|arcs| arcs.as_slice())
    }

    /// Get all definition arcs grouped by role URI.
    pub fn definitions(&self) -> &HashMap<RoleUri, Vec<DefinitionArc>> {
        &self.linkbases.definitions
    }

    /// Get definition arcs for a specific role URI.
    pub fn definition_arcs(&self, role: &str) -> Option<&[DefinitionArc]> {
        self.linkbases
            .definitions
            .get(role)
            .map(|arcs| arcs.as_slice())
    }

    /// Get all concept references.
    pub fn references(&self) -> &HashMap<ConceptId, Vec<Reference>> {
        &self.linkbases.references
    }

    /// Get references for a specific concept by its element ID.
    pub fn references_for(&self, concept_id: &str) -> Option<&[Reference]> {
        self.linkbases
            .references
            .get(concept_id)
            .map(|references| references.as_slice())
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
        self.linkbases
            .presentations
            .entry(role.into())
            .or_default()
            .push(arc);
    }

    /// Insert a label for a concept name.
    pub fn add_label(&mut self, concept_name: ExpandedName, label: Label) {
        self.linkbases
            .labels
            .entry(concept_name)
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
