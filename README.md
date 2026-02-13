# xbrl-rs

A Rust library for parsing and validating XBRL (eXtensible Business Reporting Language) documents.

- [What is XBRL?](#what-is-xbrl)
- [Testing](#testing)
- [References](#references)

## What is XBRL?

XBRL (eXtensible Business Reporting Language) is a standard for digital business
and financial reporting.

The main components of XBRL are:

- taxonomy: the dictionary and rulebook that defines how financial and business
  information is labeled, structured, and related
- instance document: a company's actual reported data, tagged using the taxonomy
- DTS (Discoverable Taxonomy Set): the full set of taxonomy files that define
  all concepts referenced in an instance

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
