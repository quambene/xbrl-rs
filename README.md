# xbrl-rs

[![latest version](https://img.shields.io/crates/v/xbrl-rs.svg?label=crates.io)](https://crates.io/crates/xbrl-rs)
[![documentation](https://img.shields.io/docsrs/xbrl-rs?label=docs.rs)](https://docs.rs/xbrl-rs)
[![build status](https://github.com/quambene/xbrl-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/quambene/xbrl-rs/actions/workflows/ci.yml)

A Rust library for parsing and validating XBRL documents.

- [What is XBRL?](#what-is-xbrl)
- [Features](#features)
- [Usage](#usage)
- [Testing](#testing)
- [References](#references)

## What is XBRL?

XBRL (eXtensible Business Reporting Language) is a standard for financial reporting.

The main concepts of XBRL are:

- **taxonomy**: the dictionary and rulebook that defines how financial and business
  information is labeled, structured, and related
- **instance document**: a company's actual reported data, referencing the taxonomy
- **discoverable taxonomy set (DTS)**: the full set of taxonomy files that define
  all concepts referenced in an instance

## Features

- XBRL Instance: parse facts, contexts, units
- XBRL Taxonomy: parse XSD schemas and linkbases (presentation, calculation, definition, labels, references)
- XBRL Validation: validate XBRL instance against XBRL taxonomy

## Usage

```rust
// Parse XBRL instance document from XML file
let xml_file = File::open("/path/to/financial_report.xml").unwrap();
let mut reader = XmlReader::from_reader(BufReader::new(xml_file));
let xbrl_instance = XbrlInstance::from_xml(&mut reader).unwrap();

// Write XBRL instance document to XML file
let mut xml_file = File::create("financial_report.xml")?;
let mut writer = XmlWriter::new(xml_file);
let xbrl_instance = XbrlInstance::default();
xbrl_instance.to_xml(&mut writer).unwrap();

// Validate XBRL instance document against XBRL taxonomy
let schema_refs = xbrl_instance.schema_refs();
let taxonomy_root = "/path/to/taxonomies";
let loader = TaxonomyLoader::new().unwrap();
loader
    .download_all(schema_refs.iter().map(String::as_str), taxonomy_root)
    .unwrap();
let taxonomy = TaxonomySet::discover(schema_refs.to_vec(), taxonomy_root.into()).unwrap();
let validation_result = xbrl_instance.validate(&taxonomy);
```

## Testing

```bash
# Run unit tests
cargo test --lib

# Run integration tests
cargo test --test '*'
```

Integration tests require the downloaded taxonomy files: `cargo run --bin download_taxonomies --release`

## Benchmarks

``` bash
# Rust (HTML report → target/criterion/taxonomy_discovery/report/index.html)
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
| arelle  | 1003.46 ms |      1x |
| xbrl-rs |   99.28 ms |     10x |

Benchmark `full_dts_2020`: 6 entry points combined into one DTS

| Library |  Mean time | Speedup |
| ------- | ---------: | ------: |
| arelle  | 3851.48 ms |      1x |
| xbrl-rs |  157.26 ms |   24.5x |

## References

- [XBRL International](https://www.xbrl.org)
- [XBRL Specifications](https://specifications.xbrl.org/specifications.html)
- [XBRL Deutschland e.V.](https://www.xbrl.de)
