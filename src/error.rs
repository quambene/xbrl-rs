use std::{io, path::PathBuf};
use thiserror::Error;

/// The type of linkbase being parsed
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkbaseType {
    Label,
    Presentation,
    Calculation,
    Definition,
    Reference,
}

impl std::fmt::Display for LinkbaseType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LinkbaseType::Label => write!(f, "label"),
            LinkbaseType::Presentation => write!(f, "presentation"),
            LinkbaseType::Calculation => write!(f, "calculation"),
            LinkbaseType::Definition => write!(f, "definition"),
            LinkbaseType::Reference => write!(f, "reference"),
        }
    }
}

/// Error type for XBRL operations
#[derive(Error, Debug)]
pub enum XbrlError {
    /// Error parsing XML content
    #[error("Error parsing XML at position {position}{}{}: {source}", path.as_ref().map(|path| format!(" in file {}", path.display())).unwrap_or_default(), element.as_ref().map(|err| format!(" in element <{}>", err)).unwrap_or_default())]
    XmlParse {
        path: Option<PathBuf>,
        position: u64,
        element: Option<String>,
        #[source]
        source: quick_xml::Error,
    },

    /// Error parsing linkbase file
    #[error("Error parsing {linkbase_type} linkbase{}: {source}", file_path.as_ref().map(|path| format!(" from {}", path.display())).unwrap_or_default())]
    LinkbaseParse {
        linkbase_type: LinkbaseType,
        file_path: Option<PathBuf>,
        #[source]
        source: quick_xml::Error,
    },

    /// Error opening file
    #[error("Failed to open {context}: {}", path.display())]
    FileOpen {
        path: PathBuf,
        context: String,
        #[source]
        source: io::Error,
    },

    /// Error reading file
    #[error("Failed to read {context}: {}", path.display())]
    FileRead {
        path: PathBuf,
        context: String,
        #[source]
        source: io::Error,
    },

    /// Error writing file
    #[error("Failed to write file: {}", path.display())]
    FileWrite {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    /// Missing required XML attribute
    #[error("{element} missing required attribute: {attribute}")]
    MissingAttribute { element: String, attribute: String },

    /// Error discovering taxonomy
    #[error("Failed to discover taxonomy for schema ref '{schema_ref}' with entry point {}", entry_point.display())]
    TaxonomyDiscovery {
        schema_ref: String,
        entry_point: PathBuf,
        #[source]
        source: Box<XbrlError>,
    },

    /// Schema refs belong to different taxonomy versions
    #[error(
        "Schema refs have mismatched versions: expected '{expected}', \
         found '{found}' in '{schema_ref}'"
    )]
    VersionMismatch {
        expected: String,
        found: String,
        schema_ref: String,
    },

    /// Invalid XLink href in a linkbase (e.g. illegal pointer scheme).
    #[error("Invalid XLink href '{href}': {reason}")]
    InvalidHref { href: String, reason: String },

    /// Invalid schema document
    #[error("Invalid schema document '{}': {reason}", path.as_ref().map(|path| path.display().to_string()).unwrap_or_else(|| "unknown".to_string()))]
    InvalidSchemaDocument {
        path: Option<PathBuf>,
        reason: String,
    },

    // Invalid instance document
    #[error("Invalid instance document '{}': {reason}", path.as_ref().map(|path| path.display().to_string()).unwrap_or_else(|| "unknown".to_string()))]
    InvalidInstanceDocument {
        path: Option<PathBuf>,
        reason: String,
    },

    /// Invalid linkbase document
    #[error("Invalid linkbase document '{}': {reason}", path.as_ref().map(|path| path.display().to_string()).unwrap_or_else(|| "unknown".to_string()))]
    InvalidLinkbaseDocument {
        path: Option<PathBuf>,
        reason: String,
    },

    /// Error resolving linkbase references
    #[error("Error resolving linkbase references: {reason}")]
    InvalidLinkbaseResolution { reason: String },

    /// A string value could not be parsed as the expected XBRL type.
    #[error("invalid {expected} value '{value}'")]
    ParseError {
        expected: &'static str,
        value: String,
    },

    /// IO error
    #[error(transparent)]
    Io(#[from] io::Error),

    /// XML error
    #[error(transparent)]
    Xml(#[from] quick_xml::Error),

    /// UTF-8 encoding error
    #[error(transparent)]
    Utf8(#[from] std::str::Utf8Error),

    /// XML escape error
    #[error(transparent)]
    Escape(#[from] quick_xml::escape::EscapeError),
}

/// Result type alias for XBRL operations
pub type Result<T> = std::result::Result<T, XbrlError>;
