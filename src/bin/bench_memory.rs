/// Memory benchmark binary for xbrl-rs.
///
/// Loads a single taxonomy entry point + instance parsing + validation so that
/// memory usage can be measured externally via `/usr/bin/time -v`
///
/// Usage:
///     # Peak RSS: /usr/bin/time -v cargo run --release --bin bench_memory
use std::path::{Path, PathBuf};
use std::str::FromStr;
use xbrl_rs::{InstanceDocument, TaxonomySet};

const TAXONOMY_ENTRY_POINT: &str = "test_data/taxonomies";
const INSTANCE_PATH: &str = "test_data/instances/balance_sheet_v64.xml";

fn schema_refs_2020() -> Vec<String> {
    vec![
        "http://www.xbrl.de/taxonomies/de-bra-2020-04-01/de-bra-2020-04-01-shell-fiscal.xsd"
            .to_owned(),
    ]
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let entry_point = PathBuf::from_str(TAXONOMY_ENTRY_POINT)?;
    let taxonomy = TaxonomySet::discover(schema_refs_2020(), entry_point)?;
    let instance = InstanceDocument::from_file(Path::new(INSTANCE_PATH))?;
    let result = instance.validate(&taxonomy);

    // Print summary
    eprintln!("Taxonomy schemas : {}", taxonomy.schemas().len());
    eprintln!("Instance facts   : {}", instance.item_facts().len());
    eprintln!("Validation errors: {}", result.errors().len());

    Ok(())
}
