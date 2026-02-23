mod linkbases;
#[cfg(feature = "download")]
mod loader;
mod schema;
mod taxonomy_set;

pub use linkbases::{
    calculation::CalculationArc,
    definition::DefinitionArc,
    label::Label,
    presentation::PresentationArc,
    reader::LinkbaseLocator,
    reference::{Reference, ReferencePart},
};
#[cfg(feature = "download")]
pub use loader::TaxonomyLoader;
pub use schema::{
    ArcroleType, Balance, CyclesAllowed, DeclaredAccuracy, ElementDefinition, LinkbaseRef,
    PeriodType, RoleType, SchemaImport, SchemaInclude, TaxonomySchema,
};
pub use taxonomy_set::{ConceptId, RoleUri, SchemaRefUrl, TaxonomySet};
