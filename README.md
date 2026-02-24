# xbrl-rs

[![latest version](https://img.shields.io/crates/v/xbrl-rs.svg?label=crates.io)](https://crates.io/crates/xbrl-rs)
[![documentation](https://img.shields.io/docsrs/xbrl-rs?label=docs.rs)](https://docs.rs/xbrl-rs)
[![build status](https://github.com/quambene/xbrl-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/quambene/xbrl-rs/actions/workflows/ci.yml)

A Rust library for parsing and validating XBRL documents.

- [What is XBRL?](#what-is-xbrl)
- [Features](#features)
- [Usage](#usage)
- [Testing](#testing)
- [Conformance](#conformance)
- [Benchmarks](#benchmarks)
- [References](#references)

## What is XBRL?

XBRL (eXtensible Business Reporting Language) is a standard for financial reporting.

The main concepts of XBRL are:

- **taxonomy**: the dictionary and rulebook that defines how financial and business
  information is labeled, structured, and related
- **instance document**: a company's actual reported data
- **discoverable taxonomy set (DTS)**: the full set of taxonomy files that define
  all concepts referenced in an instance

## Features

- XBRL Instance: parse facts, contexts, units
- XBRL Taxonomy: parse XSD schemas and linkbases (presentation, calculation, definition, labels, references)
- XBRL Validation: validate XBRL instance against XBRL taxonomy

## Usage

To download XBRL taxonomies, feature `download` needs to be enabled.

```rust
// Parse instance document from XML file
let xml_file = File::open("/path/to/financial_report.xml").unwrap();
let mut reader = XmlReader::from_reader(BufReader::new(xml_file));
let instance = InstanceDocument::from_xml(&mut reader).unwrap();

// Write instance document to XML file
let mut xml_file = File::create("financial_report.xml")?;
let mut writer = XmlWriter::new(xml_file);
let instance = InstanceDocument::default();
instance.to_xml(&mut writer).unwrap();

// Validate instance document against taxonomy
let schema_refs = instance.schema_refs();
let taxonomy_root = "/path/to/taxonomies";
let loader = TaxonomyLoader::new().unwrap();
loader
    .download_all(schema_refs.iter().map(String::as_str), taxonomy_root)
    .unwrap();
let taxonomy = TaxonomySet::discover(schema_refs.to_vec(), taxonomy_root.into()).unwrap();
let validation_result = instance.validate(&taxonomy);
```

## Testing

Integration tests require the downloaded taxonomy files: `cargo run --bin download_taxonomies --release`

```bash
# Run unit tests
cargo test --lib

# Run integration tests
cargo test --test '*'
```

## Conformance

Conformance tests are based on the [XBRL International Conformance
Suite](https://specifications.xbrl.org/release-history-base-spec-conformance-suite.html)
(2025-07-16). The downloaded test suite is required in
`test_data/conformance` to run the conformance tests.

``` bash
# Run conformance tests
cargo test conformance_suite -- --ignored --nocapture
```

| Category                  |  Passed |  Failed | Skipped |   Total | Pass Rate |
| ------------------------- | ------: | ------: | ------: | ------: | --------: |
| 100-schema                |      76 |       0 |       0 |      76 |      100% |
| 200-linkbase              |     127 |      77 |       0 |     204 |       62% |
| 300-instance              |     251 |      54 |       0 |     305 |       82% |
| 400-misc                  |       4 |       7 |       0 |      11 |       36% |
| arc-duplication           |       1 |       3 |       0 |       4 |       25% |
| uniqueParticleAttribution |       4 |       2 |       0 |       6 |       67% |
| **TOTAL**                 | **463** | **143** |   **0** | **606** |   **76%** |

## Benchmarks

The package manager `uv` is required to run the Python benchmarks.

``` bash
# Rust (HTML report is generated in target/criterion/report/index.html)
cargo bench

# Python (uv creates venv in ~/.cache/uv and installs arelle-release)
uv run benches/taxonomy_parsing.py
```

Benchmarked libraries:

- [Arelle](https://pypi.org/project/arelle-release) (v2.38.13)
- [xbrl-rs](https://crates.io/crates/xbrl-rs) (v0.1.1)

DTS discovery for German HGB taxonomies (2020-04-01, ~50 MB of XSD/XML files).

Benchmark `single_dts_2020`: 1 entry point

| Library |  Mean time | Speedup |
| ------- | ---------: | ------: |
| Arelle  | 1003.46 ms |      1x |
| xbrl-rs |  106.40 ms |      9x |

Benchmark `full_dts_2020`: 6 entry points combined into one DTS

| Library |  Mean time | Speedup |
| ------- | ---------: | ------: |
| Arelle  | 3851.48 ms |      1x |
| xbrl-rs |  174.32 ms |     22x |

## References

- [XBRL International](https://www.xbrl.org)
- [XBRL Specifications](https://specifications.xbrl.org/specifications.html)
- [XBRL Deutschland e.V.](https://www.xbrl.de)
