mod linkbases;
#[cfg(feature = "download")]
mod loader;
mod schema;
mod taxonomy_set;
mod types;

pub use linkbases::{
    locator::LinkbaseLocator,
    parser::{CalculationArc, DefinitionArc, PresentationArc},
    resolver::{Label, Reference, ReferencePart},
};
#[cfg(feature = "download")]
pub use loader::TaxonomyLoader;
pub use schema::{
    Balance, BaseSubstitutionGroup, Concept, DeclaredAccuracy, MaxOccurs, PeriodType, RoleType,
    SubstitutionGroup, TaxonomySchema, TupleChild, XbrlType,
};
pub use taxonomy_set::TaxonomySet;
pub use types::{ConceptId, RoleUri, SchemaRefUrl};
