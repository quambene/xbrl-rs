//! XBRL unit definitions
//!
//! Units define the measurement unit for numeric facts (e.g., EUR, USD, pure).

/// A parsed unit measure QName with namespace resolution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitMeasure {
    pub qname: String,
    pub prefix: Option<String>,
    pub local_name: String,
    pub namespace_uri: Option<String>,
}

/// Represents a unit of measurement
#[derive(Debug, Clone, PartialEq)]
pub struct Unit {
    /// Unique ID of the unit.
    pub id: String,
    /// Unit of measure, e.g., "iso4217:EUR" for Euro or "xbrli:pure" for
    /// dimensionless.
    pub measure: String,
    /// Numerator measures (or all measures when no divide is present).
    pub numerator_measures: Vec<UnitMeasure>,
    /// Denominator measures when divide is present.
    pub denominator_measures: Vec<UnitMeasure>,
}

impl Unit {
    pub fn new(id: String, measure: String) -> Self {
        let (prefix, local_name) = parse_qname(&measure);
        let first = UnitMeasure {
            qname: measure.clone(),
            prefix,
            local_name,
            namespace_uri: None,
        };

        Self {
            id,
            measure,
            numerator_measures: vec![first],
            denominator_measures: Vec::new(),
        }
    }

    pub fn set_measures(
        &mut self,
        numerator_measures: Vec<UnitMeasure>,
        denominator_measures: Vec<UnitMeasure>,
    ) {
        self.measure = numerator_measures
            .first()
            .map(|m| m.qname.clone())
            .unwrap_or_default();
        self.numerator_measures = numerator_measures;
        self.denominator_measures = denominator_measures;
    }

    pub fn has_denominator(&self) -> bool {
        !self.denominator_measures.is_empty()
    }

    pub fn has_single_measure_no_divide(&self) -> bool {
        self.numerator_measures.len() == 1 && self.denominator_measures.is_empty()
    }

    pub fn primary_measure(&self) -> Option<&UnitMeasure> {
        self.numerator_measures.first()
    }

    /// Check if this is a currency unit
    pub fn is_currency(&self) -> bool {
        self.primary_measure()
            .and_then(|measure| measure.namespace_uri.as_deref())
            .is_some_and(|namespace| namespace == "http://www.xbrl.org/2003/iso4217")
    }

    /// Get the currency code if this is a currency unit
    pub fn currency_code(&self) -> Option<&str> {
        self.primary_measure()
            .map(|measure| measure.local_name.as_str())
    }

    /// Check if this is a pure (dimensionless) unit
    pub fn is_pure(&self) -> bool {
        self.primary_measure().is_some_and(|measure| {
            measure.namespace_uri.as_deref() == Some("http://www.xbrl.org/2003/instance")
                && measure.local_name == "pure"
        })
    }
}

fn parse_qname(value: &str) -> (Option<String>, String) {
    if let Some((prefix, local)) = value.split_once(':') {
        (Some(prefix.to_string()), local.to_string())
    } else {
        (None, value.to_string())
    }
}
