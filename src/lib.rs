//! Parser for XBRL documents with support for:
//! - XBRL instance documents
//! - XBRL taxonomy schemas
//! ```

mod context;
mod fact;
mod instance;
mod parser;
mod taxonomy;
mod unit;
mod validation;

pub use context::{Context, EntityIdentifier, Period};
pub use fact::Fact;
pub use instance::XbrlInstance;
pub use parser::XbrlParser;
pub use taxonomy::{
    ArcroleType, CalculationArc, DefinitionArc, ElementDefinition, EntryPoint, Label,
    LinkbaseRef, PresentationArc, Reference, ReferencePart, RoleType, SchemaImport,
    SchemaInclude, TaxonomySchema, TaxonomySet,
};
pub use unit::Unit;
pub use validation::{Severity, ValidationMessage, ValidationResult, XbrlValidator};

/// Extract the `<xbrli:xbrl>` content from an XML document.
///
/// If no wrapper is detected the input is returned unchanged.
pub fn extract_xbrl(xml: &str) -> &str {
    if let Some(start) = xml.find("<xbrli:xbrl")
        && let Some(end) = xml.rfind("</xbrli:xbrl>")
    {
        return &xml[start..end + "</xbrli:xbrl>".len()];
    }

    xml
}
