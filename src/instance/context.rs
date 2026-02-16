//! XBRL context definitions
//!
//! Contexts define the reporting entity and time period for facts.

use std::collections::HashMap;

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
}

/// XBRL context combining entity, period, and optional dimensions
#[derive(Debug, Clone)]
pub struct Context {
    pub id: String,
    pub entity: EntityIdentifier,
    pub period: Period,
    pub dimensions: HashMap<String, String>,
}

impl Context {
    pub fn new(id: String, entity: EntityIdentifier, period: Period) -> Self {
        Self {
            id,
            entity,
            period,
            dimensions: HashMap::new(),
        }
    }

    pub fn add_dimension(&mut self, dimension: String, member: String) {
        self.dimensions.insert(dimension, member);
    }
}
