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

// TODO: key labels and reference by concept name
/// Resolved linkbase data suitable for use in `TaxonomySet`.
///
/// Labels and references are keyed by concept ID to provide metadata for
/// concepts during validation. Presentation, calculation, and definition arcs
/// are keyed by role URI to preserve the grouping from the linkbase files.
///
/// Linkbases are resolved as follows (e.g., for presentation arcs):
///    1. RawPresentationArc::from ("de-gaap-ci_bs.ass.fixAss")
///    2. Lookup in locators (xlink:label → xlink:href)
///    3. Extract fragment (#de-gaap-ci_bs.ass.fixAss)
///    4. Find schema element (xs:element/@id or @name), i.e. the resolved
///       Concept::name
///    5. PresentationArc::from
///       ("{http://www.xbrl.de/taxonomies/de-gaap-ci/role/balanceSheet}bs.ass.fixAss")
#[derive(Debug, Default)]
pub struct Linkbases {
    /// Presentation arcs grouped by role URI, in the order roles were first
    /// encountered during schema discovery.
    pub presentations: IndexMap<RoleUri, Vec<PresentationArc>>,
    /// Calculation arcs grouped by role URI.
    pub calculations: HashMap<RoleUri, Vec<CalculationArc>>,
    /// Definition arcs grouped by role URI.
    pub definitions: HashMap<RoleUri, Vec<DefinitionArc>>,
    /// Concept labels parsed from label linkbase files.
    /// Keyed by resolved concept name.
    pub labels: HashMap<ExpandedName, Vec<Label>>,
    /// Concept references parsed from reference linkbase files.
    /// Keyed by concept element ID.
    pub references: HashMap<ConceptId, Vec<Reference>>,
}

/// Resolve locator references from a linkbase and merge them into the provided
/// accumulator maps.
pub fn resolve_linkbases(
    linkbases: RawLinkbases,
    concepts_by_id: &HashMap<ConceptId, &Concept>,
) -> Result<Linkbases, XbrlError> {
    let mut labels: HashMap<ExpandedName, Vec<Label>> = HashMap::new();
    let mut presentations: IndexMap<RoleUri, Vec<PresentationArc>> = IndexMap::new();
    let mut calculations: HashMap<RoleUri, Vec<CalculationArc>> = HashMap::new();
    let mut definitions: HashMap<RoleUri, Vec<DefinitionArc>> = HashMap::new();
    let mut references: HashMap<ConceptId, Vec<Reference>> = HashMap::new();

    for link in linkbases.presentation_links {
        // Map from locator label to href fragment (concept ID)
        let locator_map: HashMap<&str, &str> = link
            .locators
            .iter()
            .filter_map(|locator| {
                href_fragment(&locator.href).map(|fragment| (locator.label.as_str(), fragment))
            })
            .collect();
        let arcs: Vec<PresentationArc> = link
            .arcs
            .into_iter()
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
        // Map from locator label to href fragment (concept ID)
        let locator_map: HashMap<&str, &str> = link
            .locators
            .iter()
            .filter_map(|locator| {
                href_fragment(&locator.href).map(|fragment| (locator.label.as_str(), fragment))
            })
            .collect();
        let arcs: Vec<CalculationArc> = link
            .arcs
            .into_iter()
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
        // Map from locator label to href fragment (concept ID)
        let locator_map: HashMap<&str, &str> = link
            .locators
            .iter()
            .filter_map(|locator| {
                href_fragment(&locator.href).map(|fragment| (locator.label.as_str(), fragment))
            })
            .collect();
        let arcs: Vec<DefinitionArc> = link
            .arcs
            .into_iter()
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
                if let Some(concept) = concepts_by_id.get(&ConceptId::from(concept_id)) {
                    labels.entry(concept.name.clone()).or_default().push(Label {
                        role: resource.role.clone().unwrap_or_default(),
                        lang: resource.lang.clone(),
                        text: resource.text.clone(),
                    });
                } else {
                    return Err(XbrlError::InvalidLinkbaseResolution {
                        reason: format!(
                            "Label linkbase refers to unknown concept ID '{}'",
                            concept_id
                        ),
                    });
                }
            }
        }
    }

    for link in linkbases.reference_links {
        // Map from locator label to href fragment (concept ID)
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
        presentations,
        calculations,
        definitions,
        labels,
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
    use crate::taxonomy::{
        linkbases::parser::{
            CalculationLink, DefinitionLink, LabelLink, Locator, PresentationLink,
            RawCalculationArc, RawDefinitionArc, RawLabelArc, RawPresentationArc, RawReferenceArc,
            ReferenceLink,
        },
        schema::{BaseSubstitutionGroup, PeriodType, SubstitutionGroup, XbrlType},
    };

    fn create_concepts() -> Vec<Concept> {
        vec![
            Concept {
                name: ExpandedName::new("http://example.com".into(), "concept1".to_string()),
                id: Some("concept1".to_string()),
                data_type: XbrlType::Monetary,
                substitution_group: SubstitutionGroup {
                    base: BaseSubstitutionGroup::Item,
                    original: ExpandedName::new(
                        "http://www.xbrl.org/2003/instance".into(),
                        "item".to_string(),
                    ),
                },
                period_type: Some(PeriodType::Instant),
                balance: None,
                nillable: false,
                is_abstract: false,
                content_model: None,
            },
            Concept {
                name: ExpandedName::new("http://example.com".into(), "concept2".to_string()),
                id: Some("concept2".to_string()),
                data_type: XbrlType::Monetary,
                substitution_group: SubstitutionGroup {
                    base: BaseSubstitutionGroup::Item,
                    original: ExpandedName::new(
                        "http://www.xbrl.org/2003/instance".into(),
                        "item".to_string(),
                    ),
                },
                period_type: Some(PeriodType::Instant),
                balance: None,
                nillable: false,
                is_abstract: false,
                content_model: None,
            },
        ]
    }

    #[test]
    fn test_resolve_linkbases() {
        let concepts = create_concepts();
        let concepts_by_id = concepts
            .iter()
            .map(|concept| (ConceptId::from(concept.id.clone().unwrap()), concept))
            .collect::<HashMap<_, _>>();
        let raw_presentation = RawLinkbases {
            presentation_links: vec![PresentationLink {
                role: "http://example.com/role/presentation".into(),
                locators: vec![
                    Locator {
                        label: "loc1".into(),
                        href: "schema.xsd#concept1".into(),
                    },
                    Locator {
                        label: "loc2".into(),
                        href: "schema.xsd#concept2".into(),
                    },
                ],
                arcs: vec![RawPresentationArc {
                    from: "loc1".into(),
                    to: "loc2".into(),
                    order: Some(Decimal::new(1, 0)),
                    preferred_label: None,
                    arcrole: "http://www.xbrl.org/2003/arcrole/parent-child".into(),
                }],
            }],
            calculation_links: vec![CalculationLink {
                role: "http://example.com/role/calculation".into(),
                locators: vec![
                    Locator {
                        label: "loc1".into(),
                        href: "schema.xsd#concept1".into(),
                    },
                    Locator {
                        label: "loc2".into(),
                        href: "schema.xsd#concept2".into(),
                    },
                ],
                arcs: vec![RawCalculationArc {
                    from: "loc1".into(),
                    to: "loc2".into(),
                    order: Some(Decimal::new(1, 0)),
                    weight: Decimal::new(1, 0),
                    arcrole: "http://www.xbrl.org/2003/arcrole/summation-item".into(),
                }],
            }],
            definition_links: vec![DefinitionLink {
                role: "http://example.com/role/definition".into(),
                locators: vec![
                    Locator {
                        label: "loc1".into(),
                        href: "schema.xsd#concept1".into(),
                    },
                    Locator {
                        label: "loc2".into(),
                        href: "schema.xsd#concept2".into(),
                    },
                ],
                arcs: vec![RawDefinitionArc {
                    from: "loc1".into(),
                    to: "loc2".into(),
                    order: Some(Decimal::new(1, 0)),
                    arcrole: "http://www.xbrl.org/2003/arcrole/parent-child".into(),
                }],
            }],
            label_links: vec![LabelLink {
                role: "http://example.com/role/label".into(),
                locators: vec![Locator {
                    label: "loc1".into(),
                    href: "schema.xsd#concept1".into(),
                }],
                labels: vec![LabelResource {
                    label: "lab1".into(),
                    role: Some("http://www.xbrl.org/2003/role/label".into()),
                    lang: "en".into(),
                    text: "Concept 1 Label".into(),
                }],
                arcs: vec![RawLabelArc {
                    from: "loc1".into(),
                    to: "lab1".into(),
                }],
            }],
            reference_links: vec![ReferenceLink {
                role: "http://example.com/role/reference".into(),
                locators: vec![Locator {
                    label: "loc1".into(),
                    href: "schema.xsd#concept1".into(),
                }],
                references: vec![ReferenceResource {
                    label: "ref1".into(),
                    role: Some("http://www.xbrl.org/2003/role/reference".into()),
                }],
                arcs: vec![RawReferenceArc {
                    from: "loc1".into(),
                    to: "ref1".into(),
                }],
            }],
        };
        let linkbases = resolve_linkbases(raw_presentation, &concepts_by_id).unwrap();
        assert_eq!(linkbases.presentations.len(), 1);
        assert_eq!(linkbases.calculations.len(), 1);
        assert_eq!(linkbases.definitions.len(), 1);
        assert_eq!(linkbases.labels.len(), 1);
        assert_eq!(linkbases.references.len(), 1);

        let presentation_arc = &linkbases.presentations["http://example.com/role/presentation"][0];
        assert_eq!(
            presentation_arc,
            &PresentationArc {
                from: ExpandedName::new("http://example.com".into(), "concept1".to_string()),
                to: ExpandedName::new("http://example.com".into(), "concept2".to_string()),
                order: Some(Decimal::new(1, 0)),
                preferred_label: None,
                arcrole: "http://www.xbrl.org/2003/arcrole/parent-child".into(),
            }
        );

        let calculation_arc = &linkbases.calculations["http://example.com/role/calculation"][0];
        assert_eq!(
            calculation_arc,
            &CalculationArc {
                from: ExpandedName::new("http://example.com".into(), "concept1".to_string()),
                to: ExpandedName::new("http://example.com".into(), "concept2".to_string()),
                order: Some(Decimal::new(1, 0)),
                weight: Decimal::new(1, 0),
                arcrole: "http://www.xbrl.org/2003/arcrole/summation-item".into(),
            }
        );

        let definition_arc = &linkbases.definitions["http://example.com/role/definition"][0];
        assert_eq!(
            definition_arc,
            &DefinitionArc {
                from: ExpandedName::new("http://example.com".into(), "concept1".to_string()),
                to: ExpandedName::new("http://example.com".into(), "concept2".to_string()),
                order: Some(Decimal::new(1, 0)),
                arcrole: "http://www.xbrl.org/2003/arcrole/parent-child".into(),
            }
        );

        let label = &linkbases.labels
            [&ExpandedName::new("http://example.com".into(), "concept1".to_string())][0];
        assert_eq!(
            label,
            &Label {
                role: "http://www.xbrl.org/2003/role/label".into(),
                lang: "en".into(),
                text: "Concept 1 Label".into(),
            }
        );

        let reference = &linkbases.references[&ConceptId::from("concept1".to_string())][0];
        assert_eq!(
            reference,
            &Reference {
                role: "http://www.xbrl.org/2003/role/reference".into(),
            }
        );
    }
}
