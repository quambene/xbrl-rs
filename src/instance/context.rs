//! XBRL context definitions
//!
//! Contexts define the reporting entity and time period for facts.

use crate::ExpandedName;
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
#[derive(Debug, Clone, PartialEq)]
pub struct Context {
    /// The `id` attribute of the `<xbrli:context>` element.
    pub id: ContextId,
    /// The reporting entity (e.g., a company identified by a LEI).
    pub entity: EntityIdentifier,
    /// The time period this context applies to (e.g., an instant or duration).
    pub period: Period,
    /// Optional dimensions (e.g., segment, scenario) as a map from dimension
    /// name to member name.
    pub dimensions: HashMap<ExpandedName, ExpandedName>,
    /// Track which dimensions were defined in segments.
    pub segment_elements: Vec<ExpandedName>,
    /// Track which dimensions were defined in scenarios.   
    pub scenario_elements: Vec<ExpandedName>,
    /// True if any segment element has an instance descendant (directly or via
    /// substitution group).
    pub segment_has_instance_descendant: bool,
    /// True if any scenario element has an instance descendant (directly or via
    /// substitution group).
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

    pub fn add_dimension(&mut self, dimension: ExpandedName, member: ExpandedName) {
        self.dimensions.insert(dimension, member);
    }
}
