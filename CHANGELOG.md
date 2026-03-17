<!-- markdownlint-disable MD041 -->

## Unreleased

- added
- changed
- removed

## v0.2.0 (unrelesed)

- added
  - Add benchmarks in `/benches`
  - Check version compatibility of schemas
  - Add `precision` to `Fact`
  - Validate `Context`
  - Parse and validate `FootnoteLink`
  - Add `root_xml_lang`, `role_refs`, and `arcrole_refs` to `InstanceDocument`
  - Add `type_bases` and `type_declared_accuracy` to `TaxonomySchema`
  - Add `Period::Forever`
  - Validate context, instance refs, and essence alias units
  - Add `SchemaRefUrl`, `ConceptId`, `RoleUri`, `ArcroleUri`, `ContextId`, `NamespacePrefix`, `NamespaceUri`,
    `UnitId`, `Decimals`, `Balance`, `CyclesAllowed`, `PeriodType`, `FactValue`, `ExpandedName`
  - Add `TaxonomySchema::from_xml_unchecked` and `TaxonomySchema::validate`
  - Add `DocumentView` and implement `InstanceDocument::view`
  - Add `LinkbaseLocator`
  - Add `example/print_facts.rs`
  - Implement `InstanceDocument::from_taxonomy` and `InstanceDocument::from_file`
  - Parse and validate `TupleFact`
  - Parse and validate `TupleChildRef`
  - Add feature flag `taxonomy-test` and `conformance-test`
- changed
  - Refactor reader to `SchemaParser`, `LinkbaseParser`, and `InstanceParser`
  - Rename `TaxonomySchema::parse` to `TaxonomySchema::from_reader`
  - Use `BufReader` for parsing schemas and linkbases
  - Put `TaxonomyLoader` behind feature flag `download`
  - Put `env::logger` behind feature flag `logger`
  - Rename `XbrlInstance` to `InstanceDocument`
  - Rename `ElementDefinition` to `Concept`

## v0.1.1 (2026-02-18)

- removed
  - Exclude `/test_data` and `/bin` folders from publishing

## v0.1.0 (2026-02-18)

- added
  - Add `XbrlInstance`, `TaxonomySchema`, `TaxonomySet`, and `TaxonomyLoader`
