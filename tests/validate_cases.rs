//! Manifest-driven integration tests for XBRL instance validation cases.

mod manifest_loader;

use manifest_loader::{discover_case_dirs, load_manifest};
use std::path::{Path, PathBuf};
use xbrl_rs::{InstanceDocument, TaxonomySet};

fn parse_instance(path: &Path) -> InstanceDocument {
    InstanceDocument::from_file(path).expect("failed to parse instance")
}

fn discover_taxonomy(instance: &InstanceDocument, entry_point: &Path) -> TaxonomySet {
    TaxonomySet::discover(instance.schema_refs().to_vec(), entry_point.to_path_buf()).expect(
        &format!(
            "failed to discover taxonomy for instance with entry point '{}'",
            entry_point.display()
        ),
    )
}

fn case_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("test_data")
        .join("cases")
}

#[test]
fn validate_cases_from_manifest() {
    let root = case_root();
    let case_dirs = discover_case_dirs(&root);

    assert!(
        !case_dirs.is_empty(),
        "expected at least one test case in test_data/cases"
    );

    let mut failures = Vec::new();

    for case_dir in case_dirs {
        let manifest = load_manifest(&case_dir);
        let instance_path = case_dir.join(&manifest.instance);

        let instance = parse_instance(&instance_path);
        let taxonomy = discover_taxonomy(&instance, &case_dir);
        let result = instance.validate(&taxonomy);

        let expected_valid = matches!(manifest.expected.as_str(), "pass" | "valid");

        if result.is_valid() != expected_valid {
            failures.push(format!(
                "case='{}' type='{}' expected='{}' got_valid='{}' errors={:#?}",
                manifest.name,
                manifest.case_type,
                manifest.expected,
                result.is_valid(),
                result.errors()
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "manifest-driven case failures:\n{}",
        failures.join("\n\n")
    );
}
