use crate::taxonomy::linkbases::parser::{
    CalculationArc, DefinitionArc, LabelResource, Linkbase, PresentationArc, ReferenceResource,
};
use indexmap::IndexMap;
use std::collections::HashMap;

/// A regulatory/legal reference for a taxonomy concept.
#[derive(Debug, Clone, PartialEq)]
pub struct Reference {
    /// The reference role URI.
    pub role: String,
    /// The reference parts (e.g., Name="HGB", Paragraph="242").
    pub parts: Vec<ReferencePart>,
}

/// A single key-value part within a reference.
#[derive(Debug, Clone, PartialEq)]
pub struct ReferencePart {
    /// The part name (local element name, e.g., "Name", "Paragraph").
    pub name: String,
    /// The part value (text content).
    pub value: String,
}

/// A human-readable label for a taxonomy concept.
#[derive(Debug, Clone, PartialEq)]
pub struct Label {
    /// The label role URI (e.g., `http://www.xbrl.org/2003/role/label`).
    pub role: String,
    /// The language code (e.g., "de", "en").
    pub lang: String,
    /// The label text.
    pub text: String,
}

/// Resolved linkbase data suitable for use in `TaxonomySet`.
pub(crate) struct ResolvedLinkbase {
    pub labels: HashMap<String, Vec<Label>>,
    pub presentations: IndexMap<String, Vec<PresentationArc>>,
    pub calculations: HashMap<String, Vec<CalculationArc>>,
    pub definitions: HashMap<String, Vec<DefinitionArc>>,
    pub references: HashMap<String, Vec<Reference>>,
}

/// Resolve locator references and convert into typed domain collections.
pub(crate) fn resolve_linkbase(linkbase: Linkbase) -> ResolvedLinkbase {
    let mut labels: HashMap<String, Vec<Label>> = HashMap::new();
    let mut presentations: IndexMap<String, Vec<PresentationArc>> = IndexMap::new();
    let mut calculations: HashMap<String, Vec<CalculationArc>> = HashMap::new();
    let mut definitions: HashMap<String, Vec<DefinitionArc>> = HashMap::new();
    let mut references: HashMap<String, Vec<Reference>> = HashMap::new();

    for link in linkbase.label_links {
        let locator_map: HashMap<&str, &str> = link
            .locators
            .iter()
            .filter_map(|locator| {
                href_fragment(&locator.href).map(|fragment| (locator.label.as_str(), fragment))
            })
            .collect();
        let resource_map: HashMap<&str, &LabelResource> = link
            .labels
            .iter()
            .map(|resource| (resource.label.as_str(), resource))
            .collect();

        for arc in &link.arcs {
            if let (Some(&concept_id), Some(&resource)) = (
                locator_map.get(arc.from.as_str()),
                resource_map.get(arc.to.as_str()),
            ) {
                labels
                    .entry(concept_id.to_string())
                    .or_default()
                    .push(Label {
                        role: resource.role.clone().unwrap_or_default(),
                        lang: resource.lang.clone(),
                        text: resource.text.clone(),
                    });
            }
        }
    }

    for link in linkbase.presentation_links {
        let locator_map: HashMap<&str, &str> = link
            .locators
            .iter()
            .filter_map(|locator| {
                href_fragment(&locator.href).map(|fragment| (locator.label.as_str(), fragment))
            })
            .collect();
        let arcs: Vec<PresentationArc> = link
            .arcs
            .iter()
            .filter_map(|arc| {
                Some(PresentationArc {
                    from: locator_map.get(arc.from.as_str())?.to_string(),
                    to: locator_map.get(arc.to.as_str())?.to_string(),
                    order: arc.order,
                    preferred_label: arc.preferred_label.clone(),
                })
            })
            .collect();

        if !arcs.is_empty() {
            presentations.entry(link.role).or_default().extend(arcs);
        }
    }

    for link in linkbase.calculation_links {
        let locator_map: HashMap<&str, &str> = link
            .locators
            .iter()
            .filter_map(|locator| {
                href_fragment(&locator.href).map(|fragment| (locator.label.as_str(), fragment))
            })
            .collect();
        let arcs: Vec<CalculationArc> = link
            .arcs
            .iter()
            .filter_map(|arc| {
                Some(CalculationArc {
                    from: locator_map.get(arc.from.as_str())?.to_string(),
                    to: locator_map.get(arc.to.as_str())?.to_string(),
                    order: arc.order,
                    weight: arc.weight,
                })
            })
            .collect();

        if !arcs.is_empty() {
            calculations.entry(link.role).or_default().extend(arcs);
        }
    }

    for link in linkbase.definition_links {
        let locator_map: HashMap<&str, &str> = link
            .locators
            .iter()
            .filter_map(|locator| {
                href_fragment(&locator.href).map(|fragment| (locator.label.as_str(), fragment))
            })
            .collect();
        let arcs: Vec<DefinitionArc> = link
            .arcs
            .iter()
            .filter_map(|arc| {
                Some(DefinitionArc {
                    from: locator_map.get(arc.from.as_str())?.to_string(),
                    to: locator_map.get(arc.to.as_str())?.to_string(),
                    order: arc.order,
                    arcrole: arc.arcrole.clone(),
                })
            })
            .collect();

        if !arcs.is_empty() {
            definitions.entry(link.role).or_default().extend(arcs);
        }
    }

    for link in linkbase.reference_links {
        let locator_map: HashMap<&str, &str> = link
            .locators
            .iter()
            .filter_map(|locator| {
                href_fragment(&locator.href).map(|fragment| (locator.label.as_str(), fragment))
            })
            .collect();
        let resource_map: HashMap<&str, &ReferenceResource> = link
            .references
            .iter()
            .map(|resource| (resource.label.as_str(), resource))
            .collect();

        for arc in &link.arcs {
            if let (Some(&concept_id), Some(&resource)) = (
                locator_map.get(arc.from.as_str()),
                resource_map.get(arc.to.as_str()),
            ) {
                references
                    .entry(concept_id.to_string())
                    .or_default()
                    .push(Reference {
                        role: resource.role.clone().unwrap_or_default(),
                        parts: resource
                            .parts
                            .iter()
                            .map(|(n, v)| ReferencePart {
                                name: n.clone(),
                                value: v.clone(),
                            })
                            .collect(),
                    });
            }
        }
    }

    ResolvedLinkbase {
        labels,
        presentations,
        calculations,
        definitions,
        references,
    }
}

/// Extract the fragment (after `#`) from an xlink:href.
fn href_fragment(href: &str) -> Option<&str> {
    href.split_once('#').map(|(_, frag)| frag)
}
