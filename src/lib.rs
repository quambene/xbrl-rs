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
pub use instance::{Context, EntityIdentifier, Fact, Period, Unit, XbrlInstance};
pub use quick_xml::{Reader as XmlReader, Writer as XmlWriter};
#[cfg(feature = "download")]
pub use taxonomy::TaxonomyLoader;
pub use taxonomy::{
    ArcroleType, CalculationArc, ConceptId, DefinitionArc, ElementDefinition, Label,
    LinkbaseLocator, LinkbaseRef, PresentationArc, Reference, ReferencePart, RoleType, RoleUri,
    SchemaImport, SchemaInclude, SchemaRefUrl, TaxonomySchema, TaxonomySet,
};
pub use validation::{Severity, ValidationMessage, ValidationResult};
