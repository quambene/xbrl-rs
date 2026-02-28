use serde::Deserialize;
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Deserialize)]
pub struct CaseManifest {
    pub name: String,
    #[serde(rename = "type")]
    pub case_type: String,
    pub expected: String,
    pub instance: String,
}

pub fn discover_case_dirs(root: &Path) -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    let entries = fs::read_dir(root).expect("failed to read cases directory");
    for entry in entries {
        let entry = entry.expect("failed to read case directory entry");
        let path = entry.path();
        if path.is_dir() {
            dirs.push(path);
        }
    }

    dirs.sort();
    dirs
}

pub fn load_manifest(case_dir: &Path) -> CaseManifest {
    let manifest_path = case_dir.join("manifest.json");
    let content = fs::read_to_string(&manifest_path).expect("failed to read manifest.json");
    serde_json::from_str(&content).expect("failed to parse manifest.json")
}
