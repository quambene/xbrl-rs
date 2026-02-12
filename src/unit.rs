//! XBRL unit definitions
//!
//! Units define the measurement unit for numeric facts (e.g., EUR, USD, pure).

/// Represents a unit of measurement
#[derive(Debug, Clone, PartialEq)]
pub struct Unit {
    /// Unique ID of the unit.
    pub id: String,
    /// Unit of measure, e.g., "iso4217:EUR" for Euro or "xbrli:pure" for
    /// dimensionless.
    pub measure: String,
}

impl Unit {
    pub fn new(id: String, measure: String) -> Self {
        Self { id, measure }
    }

    /// Check if this is a currency unit
    pub fn is_currency(&self) -> bool {
        self.measure.contains("iso4217:")
    }

    /// Get the currency code if this is a currency unit
    pub fn currency_code(&self) -> Option<&str> {
        if self.is_currency() {
            self.measure.strip_prefix("iso4217:")
        } else {
            None
        }
    }

    /// Check if this is a pure (dimensionless) unit
    pub fn is_pure(&self) -> bool {
        self.measure == "xbrli:pure"
    }
}
