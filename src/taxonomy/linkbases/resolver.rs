use crate::{
    ConceptId, ExpandedName, RoleUri, XbrlError,
    taxonomy::{
        Concept,
        linkbases::parser::{LabelResource, RawLinkbases, ReferenceResource},
    },
    xml::ArcroleUri,
};
use indexmap::IndexMap;
use rust_decimal::Decimal;
use std::collections::HashMap;

/// A regulatory/legal reference for a taxonomy concept.
#[derive(Debug, Clone, PartialEq)]
pub struct Reference {
    /// The reference role URI.
    pub role: String,
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

/// A resolved presentation arc between two concepts.
#[derive(Debug, Clone, PartialEq)]
pub struct PresentationArc {
    /// Parent concept of the relationship.
    pub from: ExpandedName,
    /// Child concept of the relationship.
    pub to: ExpandedName,
    /// Display order among siblings.
    pub order: Option<Decimal>,
    /// Preferred label role URI if present.
    pub preferred_label: Option<RoleUri>,
    /// Arcrole URI (normally parent-child for presentation).
    pub arcrole: ArcroleUri,
}

/// A resolved calculation arc between two concepts.
#[derive(Debug, Clone, PartialEq)]
pub struct CalculationArc {
    /// Source concept of the relationship.
    pub from: ExpandedName,
    /// Target concept of the relationship.
    pub to: ExpandedName,
    /// Display order among siblings.
    pub order: Option<Decimal>,
    /// Weight of the relationship (e.g., 1 or -1).
    pub weight: Decimal,
    /// Arcrole URI (normally summation-item for calculation).
    pub arcrole: ArcroleUri,
}

/// A resolved definition arc between two concepts.
#[derive(Debug, Clone, PartialEq)]
pub struct DefinitionArc {
    /// Source concept of the relationship.
    pub from: ExpandedName,
    /// Target concept of the relationship.
    pub to: ExpandedName,
    /// Display order among siblings.
    pub order: Option<Decimal>,
    /// Arcrole URI (normally parent-child for definition).
    pub arcrole: ArcroleUri,
}

/// Resolved linkbase data suitable for use in `TaxonomySet`.
#[derive(Debug, Default)]
pub struct Linkbases {
    /// Concept labels parsed from label linkbase files.
    /// Keyed by concept element ID (e.g., "de-gaap-ci_bs.ass").
    pub labels_by_id: HashMap<ConceptId, Vec<Label>>,
    /// Concept labels indexed by resolved concept name (ExpandedName) for quick
    /// lookup during validation.
    pub labels_by_name: HashMap<ExpandedName, Vec<Label>>,
    /// Presentation arcs grouped by role URI, in the order roles were first
    /// encountered during schema discovery.
    pub presentations: IndexMap<RoleUri, Vec<PresentationArc>>,
    /// Calculation arcs grouped by role URI.
    pub calculations: HashMap<RoleUri, Vec<CalculationArc>>,
    /// Definition arcs grouped by role URI.
    pub definitions: HashMap<RoleUri, Vec<DefinitionArc>>,
    /// Concept references parsed from reference linkbase files.
    /// Keyed by concept element ID.
    pub references: HashMap<ConceptId, Vec<Reference>>,
}

// TODO: use concepts_by_name if concepts_by_id lookup fails (e.g. if locators
// reference xs:element/@name instead of @id).
/// Resolve locator references from a linkbase and merge them into the provided
/// accumulator maps. Linkbases are resolved as follows:
///     RawPresentationArc.from ("de-gaap-ci_bs.ass.fixAss")
///         ↓ lookup in locators (xlink:label → xlink:href)
///         ↓ extract fragment (#de-gaap-ci_bs.ass.fixAss)
///         ↓ find schema element (xs:element/@id or @name)
///         ↓ Concept.name (ExpandedName)
///     ResolvedPresentationArc.from = ExpandedName
pub fn resolve_linkbases(
    linkbases: RawLinkbases,
    concepts_by_id: &HashMap<ConceptId, &Concept>,
) -> Result<Linkbases, XbrlError> {
    let mut labels_by_id: HashMap<ConceptId, Vec<Label>> = HashMap::new();
    let mut labels_by_name: HashMap<ExpandedName, Vec<Label>> = HashMap::new();
    let mut presentations: IndexMap<RoleUri, Vec<PresentationArc>> = IndexMap::new();
    let mut calculations: HashMap<RoleUri, Vec<CalculationArc>> = HashMap::new();
    let mut definitions: HashMap<RoleUri, Vec<DefinitionArc>> = HashMap::new();
    let mut references: HashMap<ConceptId, Vec<Reference>> = HashMap::new();

    for link in linkbases.label_links {
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
                labels_by_id
                    .entry(concept_id.into())
                    .or_default()
                    .push(Label {
                        role: resource.role.clone().unwrap_or_default(),
                        lang: resource.lang.clone(),
                        text: resource.text.clone(),
                    });

                if let Some(concept) = concepts_by_id.get(&ConceptId::from(concept_id.to_string()))
                {
                    labels_by_name
                        .entry(concept.name.clone())
                        .or_default()
                        .push(Label {
                            role: resource.role.clone().unwrap_or_default(),
                            lang: resource.lang.clone(),
                            text: resource.text.clone(),
                        });
                }
            }
        }
    }

    for link in linkbases.presentation_links {
        // locator label -> href fragment (concept ID)
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
                let from_fragment = locator_map.get(arc.from.as_str())?;
                let from_concept = concepts_by_id.get(&ConceptId::from(*from_fragment))?;
                let to_fragment = locator_map.get(arc.to.as_str())?;
                let to_concept = concepts_by_id.get(&ConceptId::from(*to_fragment))?;

                Some(PresentationArc {
                    from: from_concept.name.clone(),
                    to: to_concept.name.clone(),
                    order: arc.order,
                    preferred_label: arc.preferred_label.clone(),
                    arcrole: arc.arcrole.clone(),
                })
            })
            .collect();

        if !arcs.is_empty() {
            presentations
                .entry(link.role.into())
                .or_default()
                .extend(arcs);
        }
    }

    for link in linkbases.calculation_links {
        // locator label -> href fragment (concept ID)
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
                let from_fragment = locator_map.get(arc.from.as_str())?;
                let from_concept = concepts_by_id.get(&ConceptId::from(*from_fragment))?;
                let to_fragment = locator_map.get(arc.to.as_str())?;
                let to_concept = concepts_by_id.get(&ConceptId::from(*to_fragment))?;

                Some(CalculationArc {
                    from: from_concept.name.clone(),
                    to: to_concept.name.clone(),
                    order: arc.order,
                    weight: arc.weight,
                    arcrole: arc.arcrole.clone(),
                })
            })
            .collect();

        if !arcs.is_empty() {
            calculations
                .entry(link.role.into())
                .or_default()
                .extend(arcs);
        }
    }

    for link in linkbases.definition_links {
        // locator label -> href fragment (concept ID)
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
                let from_fragment = locator_map.get(arc.from.as_str())?;
                let from_concept = concepts_by_id.get(&ConceptId::from(*from_fragment))?;
                let to_fragment = locator_map.get(arc.to.as_str())?;
                let to_concept = concepts_by_id.get(&ConceptId::from(*to_fragment))?;

                Some(DefinitionArc {
                    from: from_concept.name.clone(),
                    to: to_concept.name.clone(),
                    order: arc.order,
                    arcrole: arc.arcrole.clone(),
                })
            })
            .collect();

        if !arcs.is_empty() {
            definitions
                .entry(link.role.into())
                .or_default()
                .extend(arcs);
        }
    }

    for link in linkbases.reference_links {
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
                    .entry(concept_id.into())
                    .or_default()
                    .push(Reference {
                        role: resource.role.clone().unwrap_or_default(),
                    });
            }
        }
    }

    Ok(Linkbases {
        labels_by_id,
        labels_by_name,
        presentations,
        calculations,
        definitions,
        references,
    })
}

/// Extract the fragment (after `#`) from an xlink:href.
fn href_fragment(href: &str) -> Option<&str> {
    href.split_once('#').map(|(_, frag)| frag)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_presentation_arc() {
        todo!()
    }

    #[test]
    fn test_resolve_calculation_arc() {
        todo!()
    }

    #[test]
    fn test_resolve_definition_arc() {
        todo!()
    }

    #[test]
    fn test_resolve_labels() {
        todo!()
    }

    #[test]
    fn test_resolve_references() {
        todo!()
    }
}
