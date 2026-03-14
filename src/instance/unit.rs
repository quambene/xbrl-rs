//! XBRL unit definitions
//!
//! Units define the measurement unit for numeric facts (e.g., EUR, USD, pure).

use crate::ExpandedName;
use std::{borrow::Borrow, fmt, ops::Deref};

const NS_XBRLI: &str = "http://www.xbrl.org/2003/instance";
const NS_ISO4217: &str = "http://www.xbrl.org/2003/iso4217";

/// Type-safe identifier for an XBRL unit (the `id` attribute on
/// `<xbrli:unit>` elements).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct UnitId(String);

impl UnitId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for UnitId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for UnitId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl Deref for UnitId {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl AsRef<str> for UnitId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Borrow<str> for UnitId {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for UnitId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Represents a unit of measurement
#[derive(Debug, Clone, PartialEq)]
pub struct Unit {
    /// Unique ID of the unit as specified in the instance document.
    pub id: UnitId,
    /// Numerator measures of the unit. For simple units, this contains all
    /// measures; for divide units, this is the numerator.
    pub numerator: Vec<ExpandedName>,
    /// Denominator measures of the unit.
    /// Empty for simple units, or populated for divide units.
    pub denominator: Vec<ExpandedName>,
}

impl Unit {
    pub fn new(id: UnitId, numerator: Vec<ExpandedName>, denominator: Vec<ExpandedName>) -> Self {
        Self {
            id,
            numerator,
            denominator,
        }
    }

    pub fn set_measures(
        &mut self,
        numerator_measures: Vec<ExpandedName>,
        denominator_measures: Vec<ExpandedName>,
    ) {
        self.numerator = numerator_measures;
        self.denominator = denominator_measures;
    }

    pub fn has_denominator(&self) -> bool {
        !self.denominator.is_empty()
    }

    pub fn primary_measure(&self) -> Option<&ExpandedName> {
        self.numerator.first()
    }

    pub fn has_single_measure_no_divide(&self) -> bool {
        self.numerator.len() == 1 && self.denominator.is_empty()
    }

    /// Check if this is a currency unit
    pub fn is_currency(&self) -> bool {
        self.primary_measure()
            .map(|measure| measure.namespace_uri.as_str() == "http://www.xbrl.org/2003/iso4217")
            .unwrap_or(false)
    }

    /// Get the currency code if this is a currency unit
    pub fn currency_code(&self) -> Option<&str> {
        self.primary_measure()
            .map(|measure| measure.local_name.as_str())
    }

    /// Check if this is a pure (dimensionless) unit
    pub fn is_pure(&self) -> bool {
        self.primary_measure().is_some_and(|measure| {
            measure.namespace_uri.as_str() == "http://www.xbrl.org/2003/instance"
                && measure.local_name == "pure"
        })
    }

    /// Check if this is a shares unit
    pub fn is_shares(&self) -> bool {
        self.primary_measure().is_some_and(|measure| {
            measure.namespace_uri.as_str() == "http://www.xbrl.org/2003/instance"
                && measure.local_name == "shares"
        })
    }
}

pub(crate) fn known_unit_namespace(prefix: Option<&str>) -> Option<&'static str> {
    match prefix {
        Some("iso4217") => Some(NS_ISO4217),
        Some("xbrli") => Some(NS_XBRLI),
        _ => None,
    }
}
