//! Parser for XBRL documents with support for:
//! - XBRL instance documents
//! - XBRL taxonomy schemas
//! - XBRL taxonomy linkbases (presentation, calculation, definition, label,
//!   reference)

mod error;
mod instance;
mod taxonomy;
pub(crate) mod validation;
mod xml;

pub use error::{LinkbaseType, Result, XbrlError};
pub use instance::{
    Context, ContextId, Decimals, DocumentView, EntityIdentifier, Fact, FootnoteArc, FootnoteLink,
    FootnoteLocator, FootnoteResource, InstanceDocument, InstanceParser, ItemFact, Period,
    SectionView, TreeNode, TupleFact, Unit, UnitId,
};
pub use quick_xml::{Reader as XmlReader, Writer as XmlWriter};
#[cfg(feature = "download")]
pub use taxonomy::TaxonomyLoader;
pub use taxonomy::{
    Balance, CalculationArc, DeclaredAccuracy, DefinitionArc, Label, LinkbaseLocator,
    LinkbaseParser, PeriodType, PresentationArc, Reference, ReferencePart, SchemaParser,
    SubstitutionGroup, TaxonomySchema, TaxonomySet,
};
pub use validation::{Severity, ValidationMessage, ValidationResult};
pub use xml::{
    ConceptId, ExpandedName, NamespacePrefix, NamespaceUri, QName, RoleUri, SchemaRefUrl,
};
