use crate::XbrlError;
use std::{
    borrow::Borrow,
    fmt::{self, Display},
    ops::Deref,
    str::FromStr,
};

/// The `link:schemaRef` element in an XML document, referencing a schema
/// definition.
#[derive(Debug, PartialEq, Eq)]
pub struct SchemaRef {
    /// The `xlink:href` attribute value pointing to the schema file.
    pub href: String,
}

/// The `link:roleRef` element in an XML document, referencing a role
/// definition.
#[derive(Debug, PartialEq, Eq)]
pub struct RoleRef {
    /// The `roleURI` attribute value identifying the role.
    pub role_uri: String,
    /// The `xlink:href` attribute value pointing to the role definition.
    pub href: String,
}

/// The `link:arcroleRef` element in an XML document, referencing an arcrole
/// definition.
#[derive(Debug, PartialEq, Eq)]
pub struct ArcroleRef {
    /// The `arcroleURI` attribute value identifying the arcrole.
    pub arcrole_uri: String,
    /// The `xlink:href` attribute value pointing to the arcrole definition.
    pub href: String,
}

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

/// Identifier for schema ref urls (e.g.
/// <http://www.xbrl.de/taxonomies/de-gcd-2020-04-01/de-gcd-2020-04-01-shell.xsd>).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SchemaRefUrl(String);

impl SchemaRefUrl {
    /// Returns the key as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for SchemaRefUrl {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for SchemaRefUrl {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl Deref for SchemaRefUrl {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl AsRef<str> for SchemaRefUrl {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Borrow<str> for SchemaRefUrl {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for SchemaRefUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Extended link role URI used in presentation/calculation/definition maps
/// (e.g. <http://www.xbrl.de/taxonomies/de-gaap-ci/role/balanceSheet> or
/// <http://www.xbrl.org/2003/role/label>).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RoleUri(String);

impl RoleUri {
    /// Returns the role URI as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for RoleUri {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for RoleUri {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl Deref for RoleUri {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl AsRef<str> for RoleUri {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Borrow<str> for RoleUri {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for RoleUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Arc role URI used in presentation/calculation/definition maps (e.g.
/// <http://www.xbrl.org/2003/arcrole/parent-child>).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ArcroleUri(String);

impl ArcroleUri {
    /// Returns the arcrole URI as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for ArcroleUri {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for ArcroleUri {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl Deref for ArcroleUri {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl AsRef<str> for ArcroleUri {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Display for ArcroleUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Concept element identifier used in label/reference maps
/// (e.g. `de-gaap-ci_bs.ass`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ConceptId(String);

impl ConceptId {
    /// Returns the concept identifier as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for ConceptId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for ConceptId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl Deref for ConceptId {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl AsRef<str> for ConceptId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Borrow<str> for ConceptId {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for ConceptId {
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

/// The display name of a concept (e.g.
/// "{http://xbrl.org/2003/instance}Revenue").
impl Display for ExpandedName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{{{}}}{}", self.namespace_uri, self.local_name)
    }
}
