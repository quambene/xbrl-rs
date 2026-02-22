//! XBRL fact definitions
//!
//! Facts are the actual data values in an XBRL instance document.

use crate::XbrlError;
use std::{fmt, str::FromStr};

/// The numeric accuracy attribute value for a fact (`decimals` or `precision`).
///
/// Per XBRL 2.1, the value is either `"INF"` (exact, no rounding) or an integer
/// representing the number of decimal places (for `decimals`) or significant
/// digits (for `precision`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decimals {
    /// An exact value; no rounding tolerance (`"INF"`).
    Infinite,
    /// Finite accuracy: the integer value of the `decimals` or `precision` attribute.
    Finite(i32),
}

impl FromStr for Decimals {
    type Err = XbrlError;

    fn from_str(str: &str) -> Result<Self, Self::Err> {
        if str.eq_ignore_ascii_case("INF") {
            return Ok(Self::Infinite);
        }
        str.parse::<i32>()
            .map(Self::Finite)
            .map_err(|_| XbrlError::ParseError {
                expected: "Decimals",
                value: str.to_owned(),
            })
    }
}

impl fmt::Display for Decimals {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Infinite => f.write_str("INF"),
            Self::Finite(n) => write!(f, "{n}"),
        }
    }
}

/// Represents a single fact (data point) in an XBRL instance
#[derive(Debug, Clone)]
pub struct Fact {
    /// Optional XML id attribute
    id: Option<String>,
    /// The concept name (e.g. "de-gaap-ci:bs.ass.fixAss")
    concept: String,
    /// Reference to the context ID
    context_ref: String,
    /// Optional reference to the unit ID
    unit_ref: Option<String>,
    /// The value of the fact
    value: String,
    /// Whether the fact is nil (xsi:nil="true")
    is_nil: bool,
    /// Decimals attribute for numeric facts
    decimals: Option<Decimals>,
    /// Precision attribute for numeric facts
    precision: Option<Decimals>,
}

impl Fact {
    pub fn new(
        concept: String,
        context_ref: String,
        unit_ref: Option<String>,
        value: String,
    ) -> Self {
        Self {
            id: None,
            concept,
            context_ref,
            unit_ref,
            value,
            is_nil: false,
            decimals: None,
            precision: None,
        }
    }

    pub fn concept(&self) -> &str {
        &self.concept
    }

    pub fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }

    pub fn set_id(&mut self, id: String) {
        self.id = Some(id);
    }

    pub fn context_ref(&self) -> &str {
        &self.context_ref
    }

    pub fn unit_ref(&self) -> Option<&str> {
        self.unit_ref.as_deref()
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn is_nil(&self) -> bool {
        self.is_nil
    }

    pub fn set_value(&mut self, value: String) {
        self.value = value;
    }

    pub fn set_nil(&mut self, is_nil: bool) {
        self.is_nil = is_nil;
    }

    pub fn decimals(&self) -> Option<&Decimals> {
        self.decimals.as_ref()
    }

    pub fn set_decimals(&mut self, decimals: Decimals) {
        self.decimals = Some(decimals);
    }

    pub fn precision(&self) -> Option<&Decimals> {
        self.precision.as_ref()
    }

    pub fn set_precision(&mut self, precision: Decimals) {
        self.precision = Some(precision);
    }

    /// Extract the namespace prefix from the concept
    pub fn namespace_prefix(&self) -> Option<&str> {
        self.concept.split(':').next()
    }

    /// Extract the local name from the concept (without namespace prefix)
    pub fn local_name(&self) -> &str {
        self.concept.split(':').nth(1).unwrap_or(&self.concept)
    }

    /// Convert the concept QName to element ID format used in taxonomy linkbases.
    ///
    /// Replaces the first `:` with `_`, e.g. `de-gaap-ci:bs.ass` →
    /// `de-gaap-ci_bs.ass`. This matches the `id` attribute convention in XSD
    /// where colons are not valid in XML ID values.
    pub fn concept_id(&self) -> String {
        self.concept.replacen(':', "_", 1)
    }
}
