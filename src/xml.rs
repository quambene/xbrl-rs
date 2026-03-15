use crate::XbrlError;
use std::{
    borrow::Borrow,
    fmt::{self, Display},
    ops::Deref,
    str::FromStr,
};

/// Type-safe namespace prefix key (e.g. `xmlns:xbrli` declaration on the root
/// `<xbrli:xbrl>` element).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NamespacePrefix(String);

impl NamespacePrefix {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for NamespacePrefix {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for NamespacePrefix {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl Deref for NamespacePrefix {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl AsRef<str> for NamespacePrefix {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Borrow<str> for NamespacePrefix {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for NamespacePrefix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Type-safe namespace URI key (e.g.
/// xmlns:xbrli="http://www.xbrl.org/2003/instance" declarations on the root
/// `<xbrli:xbrl>` element).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NamespaceUri(String);

impl NamespaceUri {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for NamespaceUri {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for NamespaceUri {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl Deref for NamespaceUri {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl AsRef<str> for NamespaceUri {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for NamespaceUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Represents a qualified name in the XML document (e.g.,
/// "xbrli:monetaryItemType").
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct QName {
    /// The namespace prefix (e.g., "xbrli") if present.
    pub prefix: Option<NamespacePrefix>,
    /// The local name (e.g., "monetaryItemType").
    pub local_name: String,
}

impl Display for QName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(prefix) = &self.prefix {
            write!(f, "{}:{}", prefix, self.local_name)
        } else {
            f.write_str(&self.local_name)
        }
    }
}

impl FromStr for QName {
    type Err = XbrlError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(parse_qname(s))
    }
}

/// Parses a QName string (e.g., "xbrli:monetaryItemType") into a `QName`
/// struct.
pub fn parse_qname(value: &str) -> QName {
    if let Some(idx) = value.find(':') {
        QName {
            prefix: Some(NamespacePrefix::from(&value[..idx])),
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

/// The element's resolved name, based on unique namespace uri instead of
/// prefix.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ExpandedName {
    /// The namespace URI (e.g., "http://xbrl.org/2003/instance").
    pub namespace_uri: NamespaceUri,
    /// The local name (e.g., "Revenue").
    pub local_name: String,
}

impl ExpandedName {
    pub fn new(namespace_uri: String, local_name: String) -> Self {
        Self {
            namespace_uri: namespace_uri.into(),
            local_name,
        }
    }
}

impl Display for ExpandedName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{{{}}}{}", self.namespace_uri, self.local_name)
    }
}
