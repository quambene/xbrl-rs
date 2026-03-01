pub(crate) mod calculation;
pub(crate) mod definition;
pub(crate) mod label;
pub(crate) mod presentation;
pub(crate) mod reader;
pub(crate) mod reference;

/// Extract the local name (part after the last `:`) from a prefixed XML name.
pub(super) fn local_name(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}
