//! XBRL context definitions
//!
//! Contexts define the reporting entity and time period for facts.

use std::{borrow::Borrow, collections::HashMap, fmt, ops::Deref};

/// Type-safe identifier for an XBRL context (the `id` attribute on
/// `<xbrli:context>` elements).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ContextId(String);

impl ContextId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for ContextId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for ContextId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl Deref for ContextId {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl AsRef<str> for ContextId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Borrow<str> for ContextId {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for ContextId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Identifies the reporting entity
#[derive(Debug, Clone, PartialEq)]
pub struct EntityIdentifier {
    pub scheme: String,
    pub value: String,
}

/// Time period for a context (instant or duration)
#[derive(Debug, Clone, PartialEq)]
pub enum Period {
    /// A specific point in time
    Instant { date: String },
    /// A duration between two dates
    Duration { start: String, end: String },
    /// An open-ended period
    Forever,
}

/// XBRL context combining entity, period, and optional dimensions
#[derive(Debug, Clone)]
pub struct Context {
    pub id: ContextId,
    pub entity: EntityIdentifier,
    pub period: Period,
    pub dimensions: HashMap<String, String>,
    pub segment_elements: Vec<String>,
    pub scenario_elements: Vec<String>,
    pub segment_has_instance_descendant: bool,
    pub scenario_has_instance_descendant: bool,
}

impl Context {
    pub fn new(id: ContextId, entity: EntityIdentifier, period: Period) -> Self {
        Self {
            id,
            entity,
            period,
            dimensions: HashMap::new(),
            segment_elements: Vec::new(),
            scenario_elements: Vec::new(),
            segment_has_instance_descendant: false,
            scenario_has_instance_descendant: false,
        }
    }

    pub fn add_dimension(&mut self, dimension: String, member: String) {
        self.dimensions.insert(dimension, member);
    }
}
