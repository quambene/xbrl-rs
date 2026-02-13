use super::schema::{ElementDefinition, RoleType, TaxonomySchema};
use anyhow::{Context, Result};
use log::warn;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::{Path, PathBuf},
};

/// The complete Discoverable Taxonomy Set (DTS).
///
/// Built by following all schema imports, includes, and linkbase references
/// starting from one or more entry point schemas.
#[derive(Debug)]
pub struct TaxonomySet {
    /// All schemas in the DTS, keyed by their canonical absolute path.
    schemas: HashMap<PathBuf, TaxonomySchema>,
    /// All linkbase file paths discovered (canonical absolute paths).
    linkbase_paths: Vec<PathBuf>,
}

impl TaxonomySet {
    /// Discover the DTS starting from one or more entry point schema files.
    pub fn discover(entry_points: &[&Path]) -> Result<Self> {
        let mut visited: HashSet<PathBuf> = HashSet::new();
        let mut queue: VecDeque<PathBuf> = VecDeque::new();
        let mut schemas: HashMap<PathBuf, TaxonomySchema> = HashMap::new();
        let mut linkbase_set: HashSet<PathBuf> = HashSet::new();

        // Seed the queue with entry points
        for entry in entry_points {
            let canonical = std::fs::canonicalize(entry)
                .with_context(|| format!("Failed to resolve entry point: {}", entry.display()))?;
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

        Ok(TaxonomySet {
            schemas,
            linkbase_paths: linkbase_set.into_iter().collect(),
        })
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
