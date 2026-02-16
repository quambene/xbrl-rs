//! Parser for XBRL documents with support for:
//! - XBRL instance documents
//! - XBRL taxonomy schemas
//! ```

mod context;
mod fact;
mod instance;
pub(crate) mod reader;
mod taxonomy;
mod unit;
pub(crate) mod validation;
pub(crate) mod writer;

pub use context::{Context, EntityIdentifier, Period};
pub use fact::Fact;
pub use instance::XbrlInstance;
pub use quick_xml::{Reader as XmlReader, Writer as XmlWriter};
pub use taxonomy::{
    ArcroleType, CalculationArc, DefinitionArc, ElementDefinition, Label, LinkbaseRef,
    PresentationArc, Reference, ReferencePart, RoleType, SchemaImport, SchemaInclude,
    TaxonomySchema, TaxonomySet,
};
pub use unit::Unit;
pub use validation::{Severity, ValidationMessage, ValidationResult};
