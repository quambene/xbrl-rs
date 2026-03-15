mod linkbases;
#[cfg(feature = "download")]
mod loader;
mod schema;
mod taxonomy_set;

pub use linkbases::{
    locator::LinkbaseLocator,
    parser::{CalculationArc, DefinitionArc, LinkbaseParser, PresentationArc},
    resolver::{Label, Reference, ReferencePart},
};
#[cfg(feature = "download")]
pub use loader::TaxonomyLoader;
pub use schema::{
    Balance, BaseSubstitutionGroup, Compositor, Concept, DeclaredAccuracy, MaxOccurs, PeriodType,
    RoleType, SchemaParser, SubstitutionGroup, TaxonomySchema, TupleChild, XbrlType,
};
pub use taxonomy_set::TaxonomySet;
