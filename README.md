# xbrl-rs

[![latest version](https://img.shields.io/crates/v/xbrl-rs.svg?label=crates.io)](https://crates.io/crates/xbrl-rs)
[![documentation](https://img.shields.io/docsrs/xbrl-rs?label=docs.rs)](https://docs.rs/xbrl-rs)
[![build status](https://github.com/quambene/xbrl-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/quambene/xbrl-rs/actions/workflows/ci.yml)

A Rust library for parsing and validating XBRL documents. This library expects
UTF-8 encoded XML files.

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
- XBRL View: view facts of an XBRL instance, formatted according to the presentation linkbase

## Usage

To download XBRL taxonomies, feature `download` needs to be enabled.

```rust
// Parse instance document from XML file
let instance = InstanceDocument::from_file("/path/to/financial_report.xml").unwrap();

// Write instance document to XML file
let instance = InstanceDocument::default();
instance.to_file("financial_report.xml").unwrap();

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

Some integration tests require the downloaded taxonomy files in `test_data/taxonomies`: `cargo run --bin download_taxonomies --release`

```bash
# Run unit tests
cargo test --lib

# Run integration tests
cargo test --test '*'

# Run integration tests with full taxonomy
cargo test --features taxonomy-test
```

## Conformance

Conformance tests are based on the [XBRL International Conformance
Suite](https://specifications.xbrl.org/release-history-base-spec-conformance-suite.html)
(2025-07-16). The downloaded test suite is required in
`test_data/conformance` to run the conformance tests.

``` bash
# Run conformance tests
cargo test conformance_suite --features conformance-test
```

| Category                  |  Passed |  Failed | Skipped |   Total | Pass Rate |
| ------------------------- | ------: | ------: | ------: | ------: | --------: |
| 100-schema                |      61 |      15 |       0 |      76 |       80% |
| 200-linkbase              |     117 |      87 |       0 |     204 |       57% |
| 300-instance              |     228 |      77 |       0 |     305 |       75% |
| 400-misc                  |       4 |       7 |       0 |      11 |       36% |
| arc-duplication           |       1 |       3 |       0 |       4 |       25% |
| uniqueParticleAttribution |       4 |       2 |       0 |       6 |       67% |
| **TOTAL**                 | **417** | **189** |   **0** | **606** |   **69%** |

## Benchmarks

The package manager `uv` is required to run the Python benchmarks.

``` bash
# Rust (HTML report is generated in target/criterion/report/index.html)
cargo bench

# Python (uv creates venv in ~/.cache/uv and installs arelle-release)
uv run benches/bench_xbrl.py
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
