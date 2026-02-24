//! Validation of XBRL instance documents against a taxonomy.

mod calculation;
mod dimension;
mod schema;

use crate::{InstanceDocument, TaxonomySet};

/// Severity level of a validation message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// A violation of a MUST-level XBRL rule. The instance is non-conformant.
    Error,
    /// A calculation inconsistency or soft check. The instance may still be
    /// technically valid but is likely incorrect.
    Warning,
}

/// A single validation finding.
#[derive(Debug, Clone)]
pub struct ValidationMessage {
    /// Error or Warning.
    pub severity: Severity,
    /// Machine-readable rule identifier (e.g., "schema.concept_not_found").
    pub code: String,
    /// Human-readable explanation.
    pub message: String,
    /// The concept name of the fact that triggered this, if applicable.
    pub concept: Option<String>,
    /// The context ref of the fact that triggered this, if applicable.
    pub context_ref: Option<String>,
}

/// Collected results of validating an instance document.
#[derive(Debug, Default)]
pub struct ValidationResult {
    pub messages: Vec<ValidationMessage>,
}

impl ValidationResult {
    pub fn new() -> Self {
        Self::default()
    }

    /// All messages with Error severity.
    pub fn errors(&self) -> Vec<&ValidationMessage> {
        self.messages
            .iter()
            .filter(|m| m.severity == Severity::Error)
            .collect()
    }

    /// All messages with Warning severity.
    pub fn warnings(&self) -> Vec<&ValidationMessage> {
        self.messages
            .iter()
            .filter(|m| m.severity == Severity::Warning)
            .collect()
    }

    /// True when no errors were found (warnings are acceptable).
    pub fn is_valid(&self) -> bool {
        !self.messages.iter().any(|m| m.severity == Severity::Error)
    }

    fn add(
        &mut self,
        severity: Severity,
        code: &str,
        message: String,
        concept: Option<&str>,
        context_ref: Option<&str>,
    ) {
        self.messages.push(ValidationMessage {
            severity,
            code: code.to_string(),
            message,
            concept: concept.map(str::to_string),
            context_ref: context_ref.map(str::to_string),
        });
    }
}

/// Run all validation checks: schema, calculation, and dimension.
pub fn validate_all(instance: &InstanceDocument, taxonomy: &TaxonomySet) -> ValidationResult {
    let mut result = ValidationResult::new();
    validate_schema(instance, taxonomy, &mut result);
    validate_calculations(instance, taxonomy, &mut result);
    validate_dimensions(instance, taxonomy, &mut result);
    result
}

/// Check that facts conform to the DTS element definitions and XBRL
/// specification structural rules.
pub fn validate_schema(
    instance: &InstanceDocument,
    taxonomy: &TaxonomySet,
    result: &mut ValidationResult,
) {
    schema::validate_schema(instance, taxonomy, result);
}

/// Check calculation linkbase consistency (summation-item relationships).
pub fn validate_calculations(
    instance: &InstanceDocument,
    taxonomy: &TaxonomySet,
    result: &mut ValidationResult,
) {
    calculation::validate_calculations(instance, taxonomy, result);
}

/// Check that dimension members in contexts are valid per the definition
/// linkbase.
pub fn validate_dimensions(
    instance: &InstanceDocument,
    taxonomy: &TaxonomySet,
    result: &mut ValidationResult,
) {
    dimension::validate_dimensions(instance, taxonomy, result);
}
