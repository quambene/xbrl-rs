//! Parser for XBRL documents with support for:
//! - XBRL instance documents
//! - XBRL taxonomy schemas
//! - XBRL taxonomy linkbases (presentation, calculation, definition, label,
//!   reference)

mod error;
mod instance;
mod taxonomy;
pub(crate) mod validation;

pub use error::{LinkbaseType, Result, XbrlError};
pub use instance::{
    Context, ContextId, Decimals, DocumentView, EntityIdentifier, Fact, FootnoteArc, FootnoteLink,
    FootnoteLocator, FootnoteResource, InstanceDocument, ItemFact, NamespacePrefix, Period,
    SectionView, TreeNode, TupleFact, Unit, UnitId,
};
pub use quick_xml::{Reader as XmlReader, Writer as XmlWriter};
#[cfg(feature = "download")]
pub use taxonomy::TaxonomyLoader;
pub use taxonomy::{
    ArcroleType, Balance, CalculationArc, ConceptId, CyclesAllowed, DeclaredAccuracy,
    DefinitionArc, ElementDefinition, Label, LinkbaseLocator, LinkbaseRef, PeriodType,
    PresentationArc, Reference, ReferencePart, RoleType, RoleUri, SchemaImport, SchemaInclude,
    MaxOccurs, SchemaRefUrl, TaxonomySchema, TaxonomySet, TupleChildRef,
};
pub use validation::{Severity, ValidationMessage, ValidationResult};
