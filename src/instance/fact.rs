//! XBRL fact definitions
//!
//! Facts are the actual data values in an XBRL instance document.

use crate::{ExpandedName, XbrlError};
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
pub enum Fact {
    /// An item fact with a value and optional unit reference.
    Item(ItemFact),
    /// A tuple fact that can contain child facts (both item and tuple facts).
    /// Needed to preserve the full document structure.
    Tuple(TupleFact),
}

/// Represents a single item fact (data point) in an XBRL instance.
#[derive(Debug, Clone)]
pub struct ItemFact {
    /// The the resolved concept name (e.g. "de-gaap-ci:bs.ass.fixAss").
    concept_name: ExpandedName,
    /// Optional XML id attribute
    id: Option<String>,
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

impl ItemFact {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: Option<String>,
        concept_name: ExpandedName,
        context_ref: String,
        unit_ref: Option<String>,
        value: String,
        is_nil: bool,
        decimals: Option<Decimals>,
        precision: Option<Decimals>,
    ) -> Self {
        Self {
            id,
            concept_name,
            context_ref,
            unit_ref,
            value,
            is_nil,
            decimals,
            precision,
        }
    }

    pub fn concept_name(&self) -> &ExpandedName {
        &self.concept_name
    }

    pub fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }

    pub fn set_id(&mut self, id: String) {
        self.id = Some(id);
    }

    pub fn clear_id(&mut self) {
        self.id = None;
    }

    pub fn context_ref(&self) -> &str {
        &self.context_ref
    }

    pub fn set_context_ref(&mut self, context_ref: String) {
        self.context_ref = context_ref;
    }

    pub fn unit_ref(&self) -> Option<&str> {
        self.unit_ref.as_deref()
    }

    pub fn set_unit_ref(&mut self, unit_ref: Option<String>) {
        self.unit_ref = unit_ref;
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

    pub fn clear_decimals(&mut self) {
        self.decimals = None;
    }

    pub fn precision(&self) -> Option<&Decimals> {
        self.precision.as_ref()
    }

    pub fn set_precision(&mut self, precision: Decimals) {
        self.precision = Some(precision);
    }

    pub fn clear_precision(&mut self) {
        self.precision = None;
    }
}

/// Represents a tuple fact that can contain child facts.
#[derive(Debug, Clone)]
pub struct TupleFact {
    /// Optional XML id attribute
    id: Option<String>,
    /// The resolved concept name (e.g.
    /// "de-gcd:genInfo.company.id.shareholder").
    concept_name: ExpandedName,
    /// Whether the tuple is nil (xsi:nil="true"). A nil tuple has no children
    /// and its content model constraints (e.g. minOccurs) do not apply.
    is_nil: bool,
    /// Nested child facts (item and tuple facts)
    children: Vec<Fact>,
}

impl TupleFact {
    pub fn new(concept_name: ExpandedName) -> Self {
        Self {
            id: None,
            concept_name,
            is_nil: false,
            children: Vec::new(),
        }
    }

    pub fn concept_name(&self) -> &ExpandedName {
        &self.concept_name
    }

    pub fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }

    pub fn set_id(&mut self, id: String) {
        self.id = Some(id);
    }

    pub fn is_nil(&self) -> bool {
        self.is_nil
    }

    pub fn set_nil(&mut self, is_nil: bool) {
        self.is_nil = is_nil;
    }

    pub fn children(&self) -> &[Fact] {
        &self.children
    }

    pub fn children_mut(&mut self) -> &mut Vec<Fact> {
        &mut self.children
    }

    pub fn add_child(&mut self, child: Fact) {
        self.children.push(child);
    }
}

impl Fact {
    pub fn item(
        concept: ExpandedName,
        context_ref: String,
        unit_ref: Option<String>,
        value: String,
    ) -> Self {
        Self::Item(ItemFact::new(
            None,
            concept,
            context_ref,
            unit_ref,
            value,
            false,
            None,
            None,
        ))
    }

    pub fn tuple(concept_name: ExpandedName) -> Self {
        Self::Tuple(TupleFact::new(concept_name))
    }

    pub fn concept_name(&self) -> &ExpandedName {
        match self {
            Self::Item(fact) => fact.concept_name(),
            Self::Tuple(fact) => fact.concept_name(),
        }
    }

    pub fn id(&self) -> Option<&str> {
        match self {
            Self::Item(fact) => fact.id(),
            Self::Tuple(fact) => fact.id(),
        }
    }

    pub fn as_item(&self) -> Option<&ItemFact> {
        match self {
            Self::Item(fact) => Some(fact),
            Self::Tuple(_) => None,
        }
    }

    pub fn as_item_mut(&mut self) -> Option<&mut ItemFact> {
        match self {
            Self::Item(fact) => Some(fact),
            Self::Tuple(_) => None,
        }
    }

    pub fn as_tuple(&self) -> Option<&TupleFact> {
        match self {
            Self::Item(_) => None,
            Self::Tuple(fact) => Some(fact),
        }
    }

    pub fn as_tuple_mut(&mut self) -> Option<&mut TupleFact> {
        match self {
            Self::Item(_) => None,
            Self::Tuple(fact) => Some(fact),
        }
    }

    pub fn walk_items<'a>(&'a self, out: &mut Vec<&'a ItemFact>) {
        match self {
            Self::Item(fact) => out.push(fact),
            Self::Tuple(fact) => {
                for child in &fact.children {
                    child.walk_items(out);
                }
            }
        }
    }

    pub fn walk_items_mut<'a>(&'a mut self, out: &mut Vec<&'a mut ItemFact>) {
        match self {
            Self::Item(fact) => out.push(fact),
            Self::Tuple(fact) => {
                for child in &mut fact.children {
                    child.walk_items_mut(out);
                }
            }
        }
    }

    pub fn count_items(&self) -> usize {
        match self {
            Self::Item(_) => 1,
            Self::Tuple(tuple_fact) => tuple_fact
                .children
                .iter()
                .map(|fact| fact.count_items())
                .sum(),
        }
    }
}
