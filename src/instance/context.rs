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
    /// An open-ended period
    Forever,
}

/// XBRL context combining entity, period, and optional dimensions
#[derive(Debug, Clone)]
pub struct Context {
    pub id: String,
    pub entity: EntityIdentifier,
    pub period: Period,
    pub dimensions: HashMap<String, String>,
    pub segment_elements: Vec<String>,
    pub scenario_elements: Vec<String>,
    pub segment_has_instance_descendant: bool,
    pub scenario_has_instance_descendant: bool,
}

impl Context {
    pub fn new(id: String, entity: EntityIdentifier, period: Period) -> Self {
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
