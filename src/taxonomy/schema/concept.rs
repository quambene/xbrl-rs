use crate::{
    ConceptId,
    error::{Result, XbrlError},
    taxonomy::QName,
};
use std::{fmt, str::FromStr};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XbrlBase {
    Monetary,
    Decimal,
    Integer,
    Boolean,
    String,
    Date,
    DateTime,
    Pure,
    QName,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XbrlType {
    /// Actual schema type
    pub name: QName,
    /// Resolved semantic base
    pub base: XbrlBase,
}

impl FromStr for XbrlType {
    type Err = XbrlError;

    fn from_str(value: &str) -> Result<Self> {
        let value = value.trim();
        if value.is_empty() {
            return Err(XbrlError::ParseError {
                expected: "XbrlType",
                value: value.to_owned(),
            });
        }

        let name = if let Some((namespace, local)) = value.split_once(':') {
            QName {
                namespace: namespace.to_owned(),
                local: local.to_owned(),
            }
        } else {
            QName {
                namespace: String::new(),
                local: value.to_owned(),
            }
        };

        let lower = name.local.to_ascii_lowercase();
        let base = if lower.contains("monetary") {
            XbrlBase::Monetary
        } else if lower.contains("decimal")
            || lower.contains("float")
            || lower.contains("double")
            || lower.contains("shares")
            || lower.contains("fraction")
            || lower.contains("percent")
            || lower.contains("pershare")
        {
            XbrlBase::Decimal
        } else if lower.contains("integer") {
            XbrlBase::Integer
        } else if lower.contains("boolean") {
            XbrlBase::Boolean
        } else if lower.contains("datetime") {
            XbrlBase::DateTime
        } else if lower.contains("date") {
            XbrlBase::Date
        } else if lower.contains("pure") {
            XbrlBase::Pure
        } else if lower.contains("qname") {
            XbrlBase::QName
        } else {
            XbrlBase::String
        };

        Ok(Self { name, base })
    }
}

/// The substitution group of an element (e.g., "xbrli:item", "xbrli:tuple",
/// etc.).
///
/// XBRL 2.1 defines the following substitution groups:
/// - `xbrli:item`: Concrete item concepts that can appear as facts in an
///   instance document.
/// - `xbrli:tuple`: Tuple concepts that can contain other elements as children.
/// dimension domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BaseSubstitutionGroup {
    Item,
    Tuple,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubstitutionGroup {
    pub name: QName,
    pub base: BaseSubstitutionGroup,
}

impl FromStr for SubstitutionGroup {
    type Err = XbrlError;

    fn from_str(value: &str) -> Result<Self> {
        let value = value.trim();
        if value.is_empty() {
            return Err(XbrlError::ParseError {
                expected: "SubstitutionGroup",
                value: value.to_owned(),
            });
        }

        let name = QName::from_prefixed(value);
        let base = match name.local.as_str() {
            "item" => BaseSubstitutionGroup::Item,
            "tuple" => BaseSubstitutionGroup::Tuple,
            _ => BaseSubstitutionGroup::Item,
        };

        Ok(Self { name, base })
    }
}

impl SubstitutionGroup {
    pub fn item() -> Self {
        Self {
            name: QName {
                namespace: "xbrli".to_owned(),
                local: "item".to_owned(),
            },
            base: BaseSubstitutionGroup::Item,
        }
    }

    pub fn tuple() -> Self {
        Self {
            name: QName {
                namespace: "xbrli".to_owned(),
                local: "tuple".to_owned(),
            },
            base: BaseSubstitutionGroup::Tuple,
        }
    }

    pub fn is_item(&self) -> bool {
        self.name.local == "item"
    }

    pub fn is_tuple(&self) -> bool {
        self.name.local == "tuple"
    }
}

/// Dimensional XBRL 1.0 concepts (Hypercube, Dimension, Domain, DomainMember).
#[allow(dead_code)]
pub enum DimensionalType {
    Hypercube,
    Dimension,
    Domain,
    DomainMember,
}

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
    /// The element's id attribute (e.g., "de-gaap-ci_bs.ass.fixAss"), derived from full QName.
    pub id: ConceptId,
    /// The element's qualified name (e.g., "de-gaap-ci:bs.ass.fixAss").
    pub qname: QName,
    /// The XSD type (e.g., "xbrli:monetaryItemType").
    pub data_type: XbrlType,
    /// Substitution group (e.g., "xbrli:item", "xbrli:tuple").
    pub substitution_group: SubstitutionGroup,
    /// The XBRL period type ("instant" or "duration").
    pub period_type: Option<PeriodType>,
    /// The XBRL balance type ("debit" or "credit").
    pub balance: Option<Balance>,
    /// Whether this element is nillable.
    pub nillable: bool,
    /// Whether this element is abstract.
    pub is_abstract: bool,
    /// For tuple elements: the child elements declared via `xs:element[@ref]`
    /// inside the tuple's inline `xs:complexType`. Empty for non-tuple elements.
    pub tuple_children: Vec<TupleChildRef>,
}

impl Concept {
    /// Returns `true` if this element is an XBRL tuple (`substitutionGroup="xbrli:tuple"`).
    pub fn is_tuple(&self) -> bool {
        self.substitution_group.is_tuple()
    }

    /// Returns `true` if this element is a concrete (non-abstract) item fact.
    /// Such elements are the only ones that should appear as facts in an instance document.
    pub fn is_concrete_item(&self) -> bool {
        !self.is_abstract && self.period_type.is_some()
    }
}
