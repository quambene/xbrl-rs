mod linkbases;
#[cfg(feature = "download")]
mod loader;
mod schema;
mod taxonomy_set;
mod types;

use crate::{error::Result, taxonomy::types::ParsedQName};
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
    ArcroleType, Balance, BaseSubstitutionGroup, Concept, CyclesAllowed, DeclaredAccuracy,
    LinkbaseRef, MaxOccurs, PeriodType, RoleType, SchemaImport, SchemaInclude, SubstitutionGroup,
    TaxonomySchema, TupleChildRef, XbrlBase, XbrlType,
};
pub use taxonomy_set::TaxonomySet;
pub use types::{ConceptId, QName, RoleUri, SchemaRefUrl};

pub(crate) fn split_qname(name: &[u8]) -> Result<ParsedQName<'_>> {
    let decoded = std::str::from_utf8(name)?;
    if let Some((namespace, local)) = decoded.split_once(':') {
        return Ok(ParsedQName { namespace, local });
    }

    Ok(ParsedQName {
        namespace: "",
        local: decoded,
    })
}
