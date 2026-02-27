pub(crate) struct QName<'a> {
    pub namespace: &'a str,
    pub local_name: &'a str,
}

pub(crate) fn split_qname(name: &[u8]) -> QName<'_> {
    let decoded = std::str::from_utf8(name).unwrap_or("");
    if let Some((namespace, local_name)) = decoded.split_once(':') {
        return QName {
            namespace,
            local_name,
        };
    }

    QName {
        namespace: "",
        local_name: decoded,
    }
}

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
