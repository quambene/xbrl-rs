//! Parser for XBRL documents with support for:
//! - XBRL instance documents
//! - XBRL taxonomy schemas
//! ```

mod instance;
mod taxonomy;
pub(crate) mod validation;

pub use instance::{Context, EntityIdentifier, Fact, Period, Unit, XbrlInstance};
pub use quick_xml::{Reader as XmlReader, Writer as XmlWriter};
pub use taxonomy::{
    ArcroleType, CalculationArc, DefinitionArc, ElementDefinition, Label, LinkbaseRef,
    PresentationArc, Reference, ReferencePart, RoleType, SchemaImport, SchemaInclude,
    TaxonomySchema, TaxonomySet,
};
pub use validation::{Severity, ValidationMessage, ValidationResult};
