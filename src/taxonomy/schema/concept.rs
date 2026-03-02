use crate::{
    error::{Result, XbrlError},
    taxonomy::schema::local_name,
};
use std::{fmt, str::FromStr};

/// The XBRL period type for a taxonomy element (`xbrli:periodType` attribute).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PeriodType {
    /// The element reports a value at a specific point in time.
    Instant,
    /// The element reports a value over a time range.
    Duration,
}

impl FromStr for PeriodType {
    type Err = XbrlError;

    fn from_str(str: &str) -> Result<Self> {
        match str {
            "instant" => Ok(Self::Instant),
            "duration" => Ok(Self::Duration),
            _ => Err(XbrlError::ParseError {
                expected: "PeriodType",
                value: str.to_owned(),
            }),
        }
    }
}

impl fmt::Display for PeriodType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Instant => f.write_str("instant"),
            Self::Duration => f.write_str("duration"),
        }
    }
}

/// The XBRL balance type for a monetary taxonomy element (`xbrli:balance` attribute).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Balance {
    /// An asset or expense concept (increases on the debit side).
    Debit,
    /// A liability, equity, or income concept (increases on the credit side).
    Credit,
}

impl FromStr for Balance {
    type Err = XbrlError;

    fn from_str(str: &str) -> Result<Self> {
        match str {
            "debit" => Ok(Self::Debit),
            "credit" => Ok(Self::Credit),
            _ => Err(XbrlError::ParseError {
                expected: "Balance",
                value: str.to_owned(),
            }),
        }
    }
}

impl fmt::Display for Balance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Debit => f.write_str("debit"),
            Self::Credit => f.write_str("credit"),
        }
    }
}

/// Maximum occurrences of a child element in a tuple's content model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MaxOccurs {
    /// A finite upper bound (e.g., `maxOccurs="1"`).
    Bounded(u32),
    /// No upper bound (`maxOccurs="unbounded"`).
    Unbounded,
}

/// A child element reference declared inside a tuple's `xs:complexType`.
#[derive(Debug, Clone, PartialEq)]
pub struct TupleChildRef {
    /// The qualified name of the referenced element (e.g., `"my:street"`).
    pub qname: String,
    /// Minimum occurrences from the `minOccurs` attribute; defaults to `1` per
    /// the XSD spec.
    pub min_occurs: u32,
    /// Maximum occurrences from the `maxOccurs` attribute; defaults to
    /// `MaxOccurs::Bounded(1)` per the XSD spec.
    pub max_occurs: MaxOccurs,
}

/// An `xs:element` definition from a taxonomy schema.
#[derive(Debug, Clone)]
pub struct Concept {
    /// The element's local name (e.g., "bs.ass.fixAss").
    pub name: String,
    /// The element's id attribute (e.g., "de-gaap-ci_bs.ass.fixAss").
    pub id: Option<String>,
    /// The XSD type (e.g., "xbrli:monetaryItemType").
    pub type_name: Option<String>,
    /// Substitution group (e.g., "xbrli:item", "xbrli:tuple").
    pub substitution_group: Option<String>,
    /// Whether this element is nillable.
    pub nillable: bool,
    /// Whether this element is abstract.
    pub is_abstract: bool,
    /// The XBRL period type ("instant" or "duration").
    pub period_type: Option<PeriodType>,
    /// The XBRL balance type ("debit" or "credit").
    pub balance: Option<Balance>,
    /// For tuple elements: the child elements declared via `xs:element[@ref]`
    /// inside the tuple's inline `xs:complexType`. Empty for non-tuple elements.
    pub tuple_children: Vec<TupleChildRef>,
}

impl Concept {
    /// Returns `true` if this element is an XBRL tuple (`substitutionGroup="xbrli:tuple"`).
    pub fn is_tuple(&self) -> bool {
        self.substitution_group
            .as_deref()
            .is_some_and(|s| local_name(s) == "tuple")
    }

    /// Returns `true` if this element is a concrete (non-abstract) item fact.
    /// Such elements are the only ones that should appear as facts in an instance document.
    pub fn is_concrete_item(&self) -> bool {
        !self.is_abstract && self.period_type.is_some()
    }
}
