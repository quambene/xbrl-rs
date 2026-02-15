# xbrl-rs

A Rust library for parsing and validating XBRL (eXtensible Business Reporting Language) documents.

- [What is XBRL?](#what-is-xbrl)
- [Features](#features)
- [Usage](#usage)
- [Testing](#testing)
- [References](#references)

## What is XBRL?

XBRL (eXtensible Business Reporting Language) is a standard for financial reporting.

The main components of XBRL are:

- taxonomy: the dictionary and rulebook that defines how financial and business
  information is labeled, structured, and related
- instance document: a company's actual reported data, tagged using the taxonomy
- DTS (Discoverable Taxonomy Set): the full set of taxonomy files that define
  all concepts referenced in an instance

## Features

- Instance Document: contexts (entity + period), units, facts, dimensions (explicit & typed)
- Schema (XSD): concept definitions (items/tuples), data types, period type
  (instant/duration), balance (debit/credit), abstract & nillable flags
- Linkbases: presentation, calculation, definition, labels, references
- DTS: import resolution, linkbase discovery/loading
- Validation: validate XBRL instance against XBRL taxonomy, calculation checks,
  dimensional validation

## Usage

``` rust
// Parse XBRL instance document from XML file
let xml_file = File::open("/path/to/financial_report.xml").unwrap();
let mut reader = XmlReader::from_reader(BufReader::new(xml_file));
let xbrl_instance = XbrlInstance::from_xml(&mut reader).unwrap();

// Write XBRL instance document to XML file
let mut xml_file = File::create("financial_report.xml")?;
let mut writer = XmlWriter::new(xml_file);
xbrl_instance.to_xml(&mut writer).unwrap();

// Validate XBRL instance document against XBRL taxonomy
let schema_refs = xbrl_instance.schema_refs();
let taxonomy = TaxonomySet::discover(schema_refs.to_vec(), "/path/to/taxonomies").unwrap();
let validation_result = xbrl_instance.validate(&taxonomy);
```

## Testing

```bash
# Run unit tests
cargo test --lib

# Run integration tests
cargo test --test '*'
```

## References

- [XBRL Specifications](https://specifications.xbrl.org/specifications.html)
- [XBRL Deutschland e.V.](https://www.xbrl.de/)
