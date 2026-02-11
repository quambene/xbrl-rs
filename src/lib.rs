//! Parser for XBRL documents with support for:
//! - XBRL instance documents
//! - Context and period information
//! - Unit definitions
//!
//! # Example
//!
//! ```no_run
//! use xbrl_rs::{XbrlParser, extract_xbrl};
//!
//! let xml_content = std::fs::read_to_string("instance.xml")?;
//! let xbrl = extract_xbrl(&xml_content);
//! let parser = XbrlParser::new();
//! let instance = parser.parse(xbrl)?;
//!
//! for fact in instance.facts() {
//!     println!("{}: {}", fact.concept(), fact.value());
//! }
//!
//! # Ok::<(), anyhow::Error>(())
//! ```

mod context;
mod fact;
mod instance;
mod parser;
mod unit;

pub use context::{Context, EntityIdentifier, Period};
pub use fact::Fact;
pub use instance::XbrlInstance;
pub use parser::XbrlParser;
pub use unit::Unit;

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
