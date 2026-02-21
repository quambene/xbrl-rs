//! XBRL fact definitions
//!
//! Facts are the actual data values in an XBRL instance document.

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
    decimals: Option<String>,
    /// Precision attribute for numeric facts
    precision: Option<String>,
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

    pub fn decimals(&self) -> Option<&str> {
        self.decimals.as_deref()
    }

    pub fn set_decimals(&mut self, decimals: String) {
        self.decimals = Some(decimals);
    }

    pub fn precision(&self) -> Option<&str> {
        self.precision.as_deref()
    }

    pub fn set_precision(&mut self, precision: String) {
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
}
