use std::{borrow::Borrow, fmt, ops::Deref};

/// Strongly-typed key used by [`TaxonomySet`] maps previously keyed by
/// plain strings.
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

/// Extended link role URI used in presentation/calculation/definition maps.
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
