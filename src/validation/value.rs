use super::ValidationResult;
use crate::{Concept, InstanceDocument, ItemFact, TaxonomySet};
use chrono::{NaiveDate, NaiveDateTime};
use rust_decimal::Decimal;
use std::{collections::HashMap, str::FromStr};

#[derive(Debug, Clone)]
pub(super) enum FactValue {
    Text(String),
    Numeric(Decimal),
    Boolean(bool),
    Date(NaiveDate),
    DateTime(NaiveDateTime),
}

#[derive(Default)]
pub(super) struct PreparedFactValues {
    values: HashMap<*const ItemFact, Option<FactValue>>,
}

impl PreparedFactValues {
    pub(super) fn insert(&mut self, fact: &ItemFact, value: Option<FactValue>) {
        self.values.insert(fact as *const ItemFact, value);
    }

    pub(super) fn get(&self, fact: &ItemFact) -> Option<&Option<FactValue>> {
        self.values.get(&(fact as *const ItemFact))
    }

    pub(super) fn numeric_value(&self, fact: &ItemFact) -> Option<Decimal> {
        match self.get(fact)? {
            Some(FactValue::Numeric(value)) => Some(*value),
            _ => None,
        }
    }

    #[allow(dead_code)]
    pub(super) fn text_value(&self, fact: &ItemFact) -> Option<&str> {
        match self.get(fact)? {
            Some(FactValue::Text(value)) => Some(value),
            _ => None,
        }
    }

    #[allow(dead_code)]
    pub(super) fn boolean_value(&self, fact: &ItemFact) -> Option<bool> {
        match self.get(fact)? {
            Some(FactValue::Boolean(value)) => Some(*value),
            _ => None,
        }
    }

    #[allow(dead_code)]
    pub(super) fn date_value(&self, fact: &ItemFact) -> Option<NaiveDate> {
        match self.get(fact)? {
            Some(FactValue::Date(value)) => Some(*value),
            _ => None,
        }
    }

    #[allow(dead_code)]
    pub(super) fn datetime_value(&self, fact: &ItemFact) -> Option<NaiveDateTime> {
        match self.get(fact)? {
            Some(FactValue::DateTime(value)) => Some(*value),
            _ => None,
        }
    }
}

enum ExpectedValueKind {
    Numeric,
    Boolean,
    Date,
    DateTime,
    Other,
}

pub(super) fn prepare_fact_values(
    instance: &InstanceDocument,
    taxonomy: &TaxonomySet,
    _result: &mut ValidationResult,
) -> PreparedFactValues {
    let mut prepared = PreparedFactValues::default();

    for fact in instance.item_facts() {
        if fact.is_nil() {
            prepared.insert(fact, None);
            continue;
        }

        let Some(element) = taxonomy.find_element(fact.local_name()) else {
            prepared.insert(fact, Some(FactValue::Text(fact.value().to_string())));
            continue;
        };

        let trimmed = fact.value().trim();
        let parsed = match expected_value_kind(element, taxonomy) {
            ExpectedValueKind::Numeric => parse_numeric_compatible(trimmed)
                .map(FactValue::Numeric)
                .or_else(|| Some(FactValue::Text(fact.value().to_string()))),
            ExpectedValueKind::Boolean => parse_boolean(trimmed)
                .map(FactValue::Boolean)
                .or_else(|| Some(FactValue::Text(fact.value().to_string()))),
            ExpectedValueKind::Date => parse_xsd_date(trimmed)
                .map(FactValue::Date)
                .or_else(|| Some(FactValue::Text(fact.value().to_string()))),
            ExpectedValueKind::DateTime => parse_xsd_datetime(trimmed)
                .map(FactValue::DateTime)
                .or_else(|| Some(FactValue::Text(fact.value().to_string()))),
            ExpectedValueKind::Other => Some(FactValue::Text(fact.value().to_string())),
        };

        prepared.insert(fact, parsed);
    }

    prepared
}

fn parse_numeric_compatible(value: &str) -> Option<Decimal> {
    if value.is_empty() {
        return None;
    }

    if let Ok(decimal) = Decimal::from_str(value) {
        return Some(decimal);
    }

    Decimal::from_scientific(value).ok()
}

fn expected_value_kind(element: &Concept, taxonomy: &TaxonomySet) -> ExpectedValueKind {
    let Some(type_name) = element.type_name.as_deref() else {
        return ExpectedValueKind::Other;
    };

    for base in [
        "monetaryItemType",
        "decimalItemType",
        "floatItemType",
        "doubleItemType",
        "integerItemType",
        "sharesItemType",
        "pureItemType",
        "fractionItemType",
    ] {
        if taxonomy.is_type_derived_from(type_name, base) {
            return ExpectedValueKind::Numeric;
        }
    }

    if taxonomy.is_type_derived_from(type_name, "booleanItemType") {
        return ExpectedValueKind::Boolean;
    }

    if taxonomy.is_type_derived_from(type_name, "dateTimeItemType") {
        return ExpectedValueKind::DateTime;
    }

    if taxonomy.is_type_derived_from(type_name, "dateItemType") {
        return ExpectedValueKind::Date;
    }

    let lower = type_name.to_ascii_lowercase();
    if lower.contains("monetary")
        || lower.contains("decimal")
        || lower.contains("float")
        || lower.contains("double")
        || lower.contains("integer")
        || lower.contains("shares")
        || lower.contains("pure")
        || lower.contains("percent")
        || lower.contains("pershare")
    {
        return ExpectedValueKind::Numeric;
    }

    if lower.contains("boolean") {
        return ExpectedValueKind::Boolean;
    }

    if lower.contains("datetime") {
        return ExpectedValueKind::DateTime;
    }

    if lower.contains("date") {
        return ExpectedValueKind::Date;
    }

    ExpectedValueKind::Other
}

fn parse_boolean(value: &str) -> Option<bool> {
    if value.eq_ignore_ascii_case("true") || value == "1" {
        return Some(true);
    }
    if value.eq_ignore_ascii_case("false") || value == "0" {
        return Some(false);
    }
    None
}

fn parse_xsd_date(value: &str) -> Option<NaiveDate> {
    let normalized = strip_xsd_timezone(value);
    NaiveDate::parse_from_str(normalized, "%Y-%m-%d").ok()
}

fn parse_xsd_datetime(value: &str) -> Option<NaiveDateTime> {
    let normalized = strip_xsd_timezone(value);
    NaiveDateTime::parse_from_str(normalized, "%Y-%m-%dT%H:%M:%S%.f")
        .or_else(|_| NaiveDateTime::parse_from_str(normalized, "%Y-%m-%dT%H:%M:%S"))
        .ok()
}

fn strip_xsd_timezone(value: &str) -> &str {
    let trimmed = value.trim();
    if let Some(without_z) = trimmed.strip_suffix('Z') {
        return without_z;
    }

    if trimmed.len() >= 6 {
        let tz_marker_index = trimmed.len() - 6;
        let bytes = trimmed.as_bytes();
        if (bytes[tz_marker_index] == b'+' || bytes[tz_marker_index] == b'-')
            && bytes.get(tz_marker_index + 3) == Some(&b':')
        {
            return &trimmed[..tz_marker_index];
        }
    }

    trimmed
}
