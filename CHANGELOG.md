<!-- markdownlint-disable MD041 -->

## Unreleased

- added
- changed
- removed

## v0.2.0 (unrelesed)

- added
  - Add benchmarks in `/benches`
  - Check version compatibility of schemas
  - Add `LinkbaseLocator`
  - Add `precision` to `Fact`
  - Validate `Context`
  - Parse and validate `FootnoteLink`
  - Add `root_xml_lang`, `role_refs`, and `arcrole_refs` to `XbrlInstance`
  - Add `type_bases` and `type_declared_accuracy` to `TaxonomySchema`
  - Add `Period::Forever`
  - Validate context, instance refs, and essence alias units
  - Add newtypes for `SchemaRefUrl`, `ConceptId`, and `RoleUri`
  - Add `TaxonomySchema::from_xml_unchecked` and `TaxonomySchema::validate`
- changed
  - Rename `TaxonomySchema::parse` to `TaxonomySchema::from_xml`
  - Use `BufReader` for parsing schemas and linkbases
  - Put `TaxonomyLoader` behind feature flag `download`
  - Put `env:logger` behind feature flag `logger`

## v0.1.1 (2026-02-18)

- removed
  - Exclude `/test_data` and `/bin` folders from publishing

## v0.1.0 (2026-02-18)

- added
  - Add `XbrlInstance`, `TaxonomySchema`, `TaxonomySet`, and `TaxonomyLoader`
