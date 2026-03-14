use crate::XbrlError;

/// Represents a qualified name in the XML document (e.g.,
/// "xbrli:monetaryItemType").
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct QName {
    /// The namespace prefix (e.g., "xbrli") if present.
    pub prefix: Option<String>,
    /// The local name (e.g., "monetaryItemType").
    pub local_name: String,
}

/// Parses a QName string (e.g., "xbrli:monetaryItemType") into a `QName`
/// struct.
pub fn parse_qname(value: &str) -> QName {
    if let Some(idx) = value.find(':') {
        QName {
            prefix: Some(value[..idx].to_string()),
            local_name: value[idx + 1..].to_string(),
        }
    } else {
        QName {
            prefix: None,
            local_name: value.to_string(),
        }
    }
}

/// Parses a string into a u32.
pub fn parse_u32(value: &str) -> Result<u32, XbrlError> {
    value.parse::<u32>().map_err(|_| XbrlError::ParseError {
        expected: "integer",
        value: value.to_string(),
    })
}
