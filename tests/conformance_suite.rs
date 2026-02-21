//! XBRL 2.1 Conformance Suite integration test.
//!
//! Runs the XBRL International conformance test suite located at
//! `test_data/conformance/`. The test is `#[ignore]` by default so it does not
//! block CI; run it explicitly with:
//!
//! ```text
//! cargo test conformance_suite -- --ignored --nocapture
//! ```

use quick_xml::{Reader, events::Event};
use std::{
    collections::HashMap,
    fs::File,
    io::{BufReader, Write},
    path::{Path, PathBuf},
};
use xbrl_rs::{LinkbaseLocator, TaxonomySet, XbrlInstance};

// ---------------------------------------------------------------------------
// Data model
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum FileKind {
    Xsd,
    Linkbase,
    Instance,
    TaxonomyPackage,
}

#[derive(Debug, Clone, PartialEq)]
enum Expected {
    Valid,
    Invalid,
}

#[derive(Debug, Clone)]
struct DataFile {
    kind: FileKind,
    /// Filename relative to the testcase directory.
    path: String,
    /// Whether this file is marked `readMeFirst=true` in the testcase XML.
    read_me_first: bool,
}

#[derive(Debug, Clone)]
struct Variation {
    id: String,
    name: String,
    expected: Expected,
    data_files: Vec<DataFile>,
}

#[derive(Debug)]
struct TestCase {
    name: String,
    /// Absolute path to the testcase XML file.
    path: PathBuf,
    variations: Vec<Variation>,
}

// ---------------------------------------------------------------------------
// Suite index parser
// ---------------------------------------------------------------------------

/// Parse `xbrl.xml` and return the list of testcase URIs.
fn parse_suite_index(path: &Path) -> Vec<String> {
    let file = File::open(path).expect("xbrl.xml not found");
    let mut reader = Reader::from_reader(BufReader::new(file));
    reader.config_mut().trim_text(true);

    let mut uris = Vec::new();
    let mut buf = Vec::new();

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(e)) | Ok(Event::Start(e)) => {
                let local = e.name().local_name();
                if local.as_ref() == b"testcase" {
                    for attr in e.attributes().flatten() {
                        if attr.key.local_name().as_ref() == b"uri" {
                            let uri = String::from_utf8_lossy(&attr.value).to_string();
                            uris.push(uri);
                        }
                    }
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    uris
}

// ---------------------------------------------------------------------------
// Testcase file parser
// ---------------------------------------------------------------------------

/// Parse a single testcase XML file into a [`TestCase`].
fn parse_testcase(path: &Path) -> Result<TestCase, String> {
    let file = File::open(path).map_err(|e| format!("open {}: {e}", path.display()))?;
    let mut reader = Reader::from_reader(BufReader::new(file));
    reader.config_mut().trim_text(true);

    let mut tc_name = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let mut variations: Vec<Variation> = Vec::new();

    // Parser state
    let mut current_var: Option<Variation> = None;
    let mut in_data = false;
    let mut current_file_kind: Option<FileKind> = None;
    let mut current_read_me_first = false;

    let mut buf = Vec::new();

    loop {
        buf.clear();
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                let local = e.name().local_name();
                let attrs = collect_attrs(e.attributes());

                match local.as_ref() {
                    b"testcase" => {
                        if let Some(n) = attrs.get("name") {
                            tc_name = n.clone();
                        }
                    }
                    b"variation" => {
                        current_var = Some(Variation {
                            id: attrs.get("id").cloned().unwrap_or_default(),
                            name: attrs.get("name").cloned().unwrap_or_default(),
                            expected: Expected::Valid,
                            data_files: Vec::new(),
                        });
                    }
                    b"data" if current_var.is_some() => {
                        in_data = true;
                    }
                    b"xsd" if in_data => {
                        current_file_kind = Some(FileKind::Xsd);
                        current_read_me_first = attrs
                            .get("readMeFirst")
                            .map(|v| v == "true")
                            .unwrap_or(false);
                    }
                    b"linkbase" if in_data => {
                        current_file_kind = Some(FileKind::Linkbase);
                        current_read_me_first = attrs
                            .get("readMeFirst")
                            .map(|v| v == "true")
                            .unwrap_or(false);
                    }
                    b"instance" if in_data => {
                        current_file_kind = Some(FileKind::Instance);
                        current_read_me_first = attrs
                            .get("readMeFirst")
                            .map(|v| v == "true")
                            .unwrap_or(false);
                    }
                    b"taxonomyPackage" if in_data => {
                        current_file_kind = Some(FileKind::TaxonomyPackage);
                        current_read_me_first = attrs
                            .get("readMeFirst")
                            .map(|v| v == "true")
                            .unwrap_or(false);
                    }
                    b"result" if current_var.is_some() => {
                        let expected = match attrs.get("expected").map(String::as_str) {
                            Some("valid") => Expected::Valid,
                            _ => Expected::Invalid,
                        };
                        if let Some(ref mut v) = current_var {
                            v.expected = expected;
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(e)) => {
                if let (Some(kind), Some(var)) = (current_file_kind.take(), &mut current_var) {
                    if in_data {
                        let text = String::from_utf8_lossy(e.as_ref()).trim().to_string();
                        if !text.is_empty() {
                            var.data_files.push(DataFile {
                                kind,
                                path: text,
                                read_me_first: current_read_me_first,
                            });
                        }
                        current_file_kind = None;
                    } else {
                        current_file_kind = Some(kind);
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let local = e.name().local_name();
                match local.as_ref() {
                    b"variation" => {
                        if let Some(v) = current_var.take() {
                            variations.push(v);
                        }
                    }
                    b"data" => {
                        in_data = false;
                        current_file_kind = None;
                    }
                    b"xsd" | b"linkbase" | b"instance" | b"taxonomyPackage" => {
                        // If we got End before Text (empty element handled as Start+End),
                        // discard the pending kind.
                        current_file_kind = None;
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }

    Ok(TestCase {
        name: tc_name,
        path: path.to_path_buf(),
        variations,
    })
}

/// Collect XML attributes into a `HashMap<String, String>`.
fn collect_attrs(attrs: quick_xml::events::attributes::Attributes<'_>) -> HashMap<String, String> {
    attrs
        .flatten()
        .map(|a| {
            let key = String::from_utf8_lossy(a.key.local_name().as_ref()).to_string();
            let val = String::from_utf8_lossy(&a.value).to_string();
            (key, val)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Variation runner
// ---------------------------------------------------------------------------

#[derive(Debug, PartialEq)]
enum Outcome {
    Valid,
    Invalid,
    Skipped,
}

/// Run a single variation and return whether the result is valid, invalid, or
/// skipped (unsupported variation type).
fn run_variation(variation: &Variation, base_dir: &Path) -> Outcome {
    // Classify by the primary (readMeFirst=true) file kind.
    let primary = variation.data_files.iter().find(|file| file.read_me_first);

    match primary.map(|file| &file.kind) {
        Some(FileKind::Instance) => run_instance_variation(variation, base_dir),
        Some(FileKind::Xsd) => run_schema_variation(variation, base_dir),
        Some(FileKind::Linkbase) => run_linkbase_variation(variation, base_dir),
        Some(FileKind::TaxonomyPackage) => Outcome::Skipped,
        None => {
            // No readMeFirst=true — if there is exactly one XSD, treat it as
            // the primary schema target (some testcases omit readMeFirst).
            let xsds: Vec<_> = variation
                .data_files
                .iter()
                .filter(|file| file.kind == FileKind::Xsd)
                .collect();
            let instances: Vec<_> = variation
                .data_files
                .iter()
                .filter(|file| file.kind == FileKind::Instance)
                .collect();

            if !instances.is_empty() {
                run_instance_variation(variation, base_dir)
            } else if !xsds.is_empty() {
                run_schema_variation(variation, base_dir)
            } else {
                Outcome::Skipped
            }
        }
    }
}

/// Run an instance-primary variation: parse the instance, discover the
/// taxonomy from accompanying XSD files, then validate.
fn run_instance_variation(variation: &Variation, base_dir: &Path) -> Outcome {
    // Find the primary instance (readMeFirst=true, or the first instance).
    let instance_file = variation
        .data_files
        .iter()
        .find(|f| f.kind == FileKind::Instance && f.read_me_first)
        .or_else(|| {
            variation
                .data_files
                .iter()
                .find(|f| f.kind == FileKind::Instance)
        });

    let Some(instance_file) = instance_file else {
        return Outcome::Skipped;
    };

    let instance_path = base_dir.join(&instance_file.path);

    // Parse the instance document.
    let instance = match File::open(&instance_path) {
        Ok(f) => {
            let mut reader = Reader::from_reader(BufReader::new(f));
            match XbrlInstance::from_xml(&mut reader) {
                Ok(i) => i,
                Err(_) => return Outcome::Invalid,
            }
        }
        Err(_) => return Outcome::Invalid,
    };

    // Collect companion XSD schema refs.
    let xsd_refs: Vec<String> = variation
        .data_files
        .iter()
        .filter(|f| f.kind == FileKind::Xsd)
        .map(|f| f.path.clone())
        .collect();

    // Merge: schema refs from the instance itself + explicit XSD files.
    let mut schema_refs: Vec<String> = instance
        .schema_refs()
        .iter()
        .map(|s| {
            // If the schema ref looks like a relative path (no scheme), keep it;
            // otherwise use as-is so strip_prefix in TaxonomySet works.
            s.to_string()
        })
        .collect();
    for xsd in &xsd_refs {
        if !schema_refs.contains(xsd) {
            schema_refs.push(xsd.clone());
        }
    }

    // If there are no schema refs at all, skip (can't discover a taxonomy).
    if schema_refs.is_empty() {
        return Outcome::Skipped;
    }

    // Discover taxonomy.
    let taxonomy = match TaxonomySet::discover(schema_refs, base_dir.to_path_buf()) {
        Ok(t) => t,
        Err(_) => return Outcome::Invalid,
    };

    // Validate.
    let result = instance.validate(&taxonomy);
    if result.is_valid() {
        Outcome::Valid
    } else {
        Outcome::Invalid
    }
}

/// Run a linkbase-primary variation.
///
/// 1. Parses the primary linkbase file and validates all locator hrefs for
///    illegal pointer syntax (xpointer(), xmlns() schemes, empty href in
///    standard links).  Any illegal syntax → Invalid.
/// 2. If a companion XSD is present, delegates to [`run_schema_variation`]
///    for the full taxonomy-discovery check (the XSD's `linkbaseRef` causes
///    the linkbase to be loaded transitively).
fn run_linkbase_variation(variation: &Variation, base_dir: &Path) -> Outcome {
    let lb_file = variation
        .data_files
        .iter()
        .find(|f| f.kind == FileKind::Linkbase && f.read_me_first);

    let Some(lb_file) = lb_file else {
        return Outcome::Skipped;
    };

    let lb_path = base_dir.join(&lb_file.path);

    // Open and parse the linkbase, then validate every locator href.
    let locators = match File::open(&lb_path) {
        Ok(f) => {
            let mut reader = Reader::from_reader(BufReader::new(f));
            match LinkbaseLocator::parse(&mut reader) {
                Ok(locs) => locs,
                Err(_) => return Outcome::Invalid,
            }
        }
        Err(_) => return Outcome::Invalid,
    };
    for loc in &locators {
        if loc.validate().is_err() {
            return Outcome::Invalid;
        }
    }

    // If there is a companion XSD, verify that the full DTS can be discovered.
    // (The XSD typically carries a linkbaseRef that points back to the primary
    // linkbase, so this also exercises loading the linkbase through the normal
    // taxonomy-discovery path.)
    let has_xsd = variation.data_files.iter().any(|f| f.kind == FileKind::Xsd);
    if has_xsd {
        run_schema_variation(variation, base_dir)
    } else {
        Outcome::Valid
    }
}

/// Run an XSD-primary variation: try to discover the taxonomy from the named
/// schema files. Error → invalid, success → valid.
fn run_schema_variation(variation: &Variation, base_dir: &Path) -> Outcome {
    let schema_refs: Vec<String> = variation
        .data_files
        .iter()
        .filter(|f| f.kind == FileKind::Xsd)
        .map(|f| f.path.clone())
        .collect();

    if schema_refs.is_empty() {
        return Outcome::Skipped;
    }

    match TaxonomySet::discover(schema_refs, base_dir.to_path_buf()) {
        Ok(_) => Outcome::Valid,
        Err(_) => Outcome::Invalid,
    }
}

// ---------------------------------------------------------------------------
// Test entry point
// ---------------------------------------------------------------------------

/// Extract the category name from a testcase path, e.g.
/// `Common/300-instance/301-idScope.xml` → `"300-instance"`.
fn category(testcase: &TestCase) -> String {
    testcase
        .path
        .parent()
        .and_then(|path| path.file_name())
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

#[test]
#[ignore]
fn conformance_suite() {
    let suite_root = Path::new("test_data/conformance");
    assert!(
        suite_root.exists(),
        "conformance suite not found at {suite_root:?}"
    );

    let testcase_uris = parse_suite_index(&suite_root.join("xbrl.xml"));
    assert!(!testcase_uris.is_empty(), "no testcases found in xbrl.xml");

    let testcases: Vec<TestCase> = testcase_uris
        .iter()
        .filter_map(|uri| {
            let path = suite_root.join(uri);
            match parse_testcase(&path) {
                Ok(testcase) => Some(testcase),
                Err(err) => {
                    eprintln!("WARNING: could not parse testcase {uri}: {err}");
                    None
                }
            }
        })
        .collect();

    // Per-category counters.
    let mut cat_pass: HashMap<String, usize> = HashMap::new();
    let mut cat_fail: HashMap<String, usize> = HashMap::new();
    let mut cat_skip: HashMap<String, usize> = HashMap::new();

    let mut failures: Vec<String> = Vec::new();

    for testcase in &testcases {
        let category = category(testcase);
        let base_dir = testcase.path.parent().unwrap_or(Path::new("."));

        for variation in &testcase.variations {
            let outcome = run_variation(variation, base_dir);

            let expected_outcome = match variation.expected {
                Expected::Valid => Outcome::Valid,
                Expected::Invalid => Outcome::Invalid,
            };

            match outcome {
                Outcome::Skipped => {
                    *cat_skip.entry(category.clone()).or_default() += 1;
                }
                ref outcome if *outcome == expected_outcome => {
                    *cat_pass.entry(category.clone()).or_default() += 1;
                }
                _ => {
                    *cat_fail.entry(category.clone()).or_default() += 1;
                    failures.push(format!(
                        "  [{category} / {} / {} {}] expected={:?}, got={:?}",
                        testcase.name, variation.id, variation.name, variation.expected, outcome
                    ));
                }
            }
        }
    }

    // Print report.
    println!("\nXBRL 2.1 Conformance Suite Results");
    println!("====================================");

    let mut categories: Vec<&String> = cat_pass
        .keys()
        .chain(cat_fail.keys())
        .chain(cat_skip.keys())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    categories.sort();

    let mut total_pass = 0usize;
    let mut total_fail = 0usize;
    let mut total_skip = 0usize;

    for category in &categories {
        let passed = *cat_pass.get(*category).unwrap_or(&0);
        let failed = *cat_fail.get(*category).unwrap_or(&0);
        let skipped = *cat_skip.get(*category).unwrap_or(&0);
        let total = passed + failed + skipped;
        println!(
            "  {category:<20}: {passed:>4} passed, {failed:>4} failed, {skipped:>4} skipped / {total:>4} total"
        );
        total_pass += passed;
        total_fail += failed;
        total_skip += skipped;
    }

    let grand_total = total_pass + total_fail + total_skip;
    println!(
        "  {:<20}  {:>4} passed, {:>4} failed, {:>4} skipped / {:>4} total",
        "TOTAL", total_pass, total_fail, total_skip, grand_total
    );

    if !failures.is_empty() {
        println!("\nFAILURES ({}):", failures.len());
        for line in &failures {
            println!("{line}");
        }
    }

    // Write summary CSV.
    let csv_path = Path::new("test_data/conformance_results.csv");

    if let Ok(mut csv) = File::create(csv_path) {
        writeln!(csv, "category,passed,failed,skipped,total").unwrap();
        for category in &categories {
            let passed = *cat_pass.get(*category).unwrap_or(&0);
            let failed = *cat_fail.get(*category).unwrap_or(&0);
            let skipped = *cat_skip.get(*category).unwrap_or(&0);
            let total = passed + failed + skipped;
            writeln!(csv, "{category},{passed},{failed},{skipped},{total}").unwrap();
        }
        writeln!(
            csv,
            "TOTAL,{total_pass},{total_fail},{total_skip},{grand_total}"
        )
        .unwrap();
        println!("Results written to {}", csv_path.display());
    }

    println!();
}
