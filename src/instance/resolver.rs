//! Resolves a [`RawInstance`] into a high-level [`InstanceDocument`].
//!
//! This module converts the raw, string-based types produced by the parser into
//! the domain types used by the rest of the library.

use super::{
    Context, ContextId, EntityIdentifier, Fact, FootnoteArc, FootnoteLink, FootnoteLocator,
    FootnoteResource, InstanceDocument, ItemFact, NamespacePrefix, Period, TupleFact, Unit, UnitId,
    parser::{
        RawContext, RawFact, RawFootnoteLink, RawInstance, RawItemFact, RawPeriod, RawTupleFact,
        RawUnit,
    },
    unit::known_unit_namespace,
};
use crate::{ExpandedName, QName, XbrlError, instance::NamespaceUri};
use std::collections::HashMap;

/// Resolve a [`RawInstance`] into an [`InstanceDocument`].
pub(crate) fn resolve_instance(raw: RawInstance) -> Result<InstanceDocument, XbrlError> {
    let namespaces = &raw.namespaces;
    let schema_refs = raw
        .schema_refs
        .into_iter()
        .map(|schema_ref| schema_ref.href)
        .collect();
    let contexts = resolve_contexts(raw.contexts, namespaces)?;
    let units = resolve_units(raw.units, namespaces)?;
    let facts = resolve_facts(raw.facts, namespaces)?;
    let footnote_links = resolve_footnote_links(raw.footnote_links);

    let mut instance = InstanceDocument::new(
        schema_refs,
        contexts,
        units,
        facts,
        namespaces.clone(),
        footnote_links,
    );

    for role_ref in raw.role_refs {
        instance.add_role_ref(role_ref.role_uri);
    }
    for arcrole_ref in raw.arcrole_refs {
        instance.add_arcrole_ref(arcrole_ref.arcrole_uri);
    }

    Ok(instance)
}

fn resolve_contexts(
    raw: Vec<RawContext>,
    namespaces: &HashMap<NamespacePrefix, NamespaceUri>,
) -> Result<HashMap<ContextId, Context>, XbrlError> {
    raw.into_iter()
        .map(|raw_context| {
            let id = ContextId::from(raw_context.id);
            let entity = EntityIdentifier {
                scheme: raw_context.entity.scheme,
                value: raw_context.entity.identifier,
            };
            let period = match raw_context.period {
                RawPeriod::Instant(date) => Period::Instant { date },
                RawPeriod::Duration {
                    start_date,
                    end_date,
                } => Period::Duration {
                    start: start_date,
                    end: end_date,
                },
                RawPeriod::Forever => Period::Forever,
            };
            let mut context = Context::new(id.clone(), entity, period);

            for dim in raw_context.scenario_dimensions {
                let dimension = resolve_measure(&dim.dimension, namespaces)?;
                let member = resolve_measure(&dim.member, namespaces)?;
                context.add_dimension(dimension, member);
            }

            Ok((id, context))
        })
        .collect()
}

fn resolve_units(
    raw: Vec<RawUnit>,
    namespaces: &HashMap<NamespacePrefix, NamespaceUri>,
) -> Result<HashMap<UnitId, Unit>, XbrlError> {
    raw.into_iter()
        .map(|raw_unit| {
            let id = UnitId::from(raw_unit.id);
            let numerator = raw_unit
                .numerator
                .into_iter()
                .map(|qname| resolve_measure(&qname, namespaces))
                .collect::<Result<Vec<_>, _>>()?;
            let denominator = raw_unit
                .denominator
                .into_iter()
                .map(|qname| resolve_measure(&qname, namespaces))
                .collect::<Result<Vec<_>, _>>()?;
            let unit = Unit {
                id: id.clone(),
                numerator,
                denominator,
            };

            Ok((id, unit))
        })
        .collect()
}

/// Resolve a QName measure string (e.g. `iso4217:EUR`) into a [`ExpandedName`]
/// by looking up the namespace URI in the document's namespace map.
fn resolve_measure(
    qname: &QName,
    namespaces: &HashMap<NamespacePrefix, NamespaceUri>,
) -> Result<ExpandedName, XbrlError> {
    let namespace_uri = if let Some(prefix) = qname.prefix.as_deref() {
        namespaces
            .get(prefix)
            .cloned()
            .or_else(|| known_unit_namespace(Some(prefix)).map(NamespaceUri::from))
    } else {
        None
    }
    .ok_or_else(|| XbrlError::ParseError {
        expected: "QName with prefix",
        value: qname.to_string(),
    })?;

    Ok(ExpandedName {
        namespace_uri,
        local_name: qname.local_name.clone(),
    })
}

fn resolve_facts(
    raw: Vec<RawFact>,
    namespaces: &HashMap<NamespacePrefix, NamespaceUri>,
) -> Result<Vec<Fact>, XbrlError> {
    raw.into_iter()
        .map(|raw_fact| resolve_fact(raw_fact, namespaces))
        .collect()
}

fn resolve_fact(
    raw: RawFact,
    namespaces: &HashMap<NamespacePrefix, NamespaceUri>,
) -> Result<Fact, XbrlError> {
    match raw {
        RawFact::Item(item) => resolve_item_fact(item, namespaces).map(Fact::Item),
        RawFact::Tuple(tuple) => resolve_tuple_fact(tuple, namespaces).map(Fact::Tuple),
    }
}

fn resolve_item_fact(
    raw: RawItemFact,
    namespaces: &HashMap<NamespacePrefix, NamespaceUri>,
) -> Result<ItemFact, XbrlError> {
    let concept_name = resolve_measure(&raw.name, namespaces)?;
    let mut fact = ItemFact::new(
        None,
        concept_name,
        raw.context_ref,
        raw.unit_ref,
        raw.value,
        false,
        None,
        None,
    );

    if let Some(id) = raw.id {
        fact.set_id(id);
    }

    fact.set_nil(raw.is_nil);

    if let Some(value) = raw.decimals {
        fact.set_decimals(value.parse()?);
    }
    if let Some(value) = raw.precision {
        fact.set_precision(value.parse()?);
    }

    Ok(fact)
}

fn resolve_tuple_fact(
    raw: RawTupleFact,
    namespaces: &HashMap<NamespacePrefix, NamespaceUri>,
) -> Result<TupleFact, XbrlError> {
    let concept_name = resolve_measure(&raw.name, namespaces)?;
    let mut tuple = TupleFact::new(concept_name);

    if let Some(id) = raw.id {
        tuple.set_id(id);
    }

    tuple.set_nil(raw.is_nil);

    for child in raw.children {
        tuple.add_child(resolve_fact(child, namespaces)?);
    }

    Ok(tuple)
}

fn resolve_footnote_links(raw: Vec<RawFootnoteLink>) -> Vec<FootnoteLink> {
    raw.into_iter()
        .map(|raw_link| {
            let role = if raw_link.role.is_empty() {
                None
            } else {
                Some(raw_link.role)
            };

            let locators = raw_link
                .locators
                .into_iter()
                .map(|loc| FootnoteLocator {
                    element_local_name: "loc".to_string(),
                    label: Some(loc.label),
                    href: Some(loc.href),
                })
                .collect();

            let footnotes = raw_link
                .footnotes
                .into_iter()
                .map(|res| FootnoteResource {
                    label: Some(res.label),
                    id: None,
                    role: None,
                    xml_lang: res.lang,
                })
                .collect();

            let arcs = raw_link
                .arcs
                .into_iter()
                .map(|arc| FootnoteArc {
                    from: Some(arc.from),
                    to: Some(arc.to),
                    arcrole: None,
                })
                .collect();

            FootnoteLink {
                role,
                xml_lang: None,
                locators,
                footnotes,
                arcs,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Decimals,
        instance::parser::{RawDimension, RawEntity},
        xml::{ArcroleRef, RoleRef, SchemaRef},
    };
    use assert_matches::assert_matches;
    use std::str::FromStr;

    #[test]
    fn resolve_empty_instance() {
        let raw = RawInstance::default();
        let doc = resolve_instance(raw).unwrap();

        assert!(doc.schema_refs().is_empty());
        assert!(doc.contexts().is_empty());
        assert!(doc.units().is_empty());
        assert!(doc.facts().is_empty());
        assert!(doc.footnote_links().is_empty());
    }

    #[test]
    fn resolve_schema_and_role_refs() {
        let mut raw = RawInstance::default();
        raw.schema_refs.push(SchemaRef {
            href: "taxonomy.xsd".to_string(),
        });
        raw.role_refs.push(RoleRef {
            role_uri: "http://example.com/role".to_string(),
            href: "role.xsd".to_string(),
        });
        raw.arcrole_refs.push(ArcroleRef {
            arcrole_uri: "http://example.com/arcrole".to_string(),
            href: "arcrole.xsd".to_string(),
        });

        let doc = resolve_instance(raw).unwrap();

        assert_eq!(doc.schema_refs(), ["taxonomy.xsd"]);
        assert_eq!(doc.role_refs(), ["http://example.com/role"]);
        assert_eq!(doc.arcrole_refs(), ["http://example.com/arcrole"]);
    }

    #[test]
    fn resolve_namespaces() {
        let mut raw = RawInstance::default();
        raw.namespaces
            .insert("xbrli".into(), "http://www.xbrl.org/2003/instance".into());

        let doc = resolve_instance(raw).unwrap();

        assert_eq!(
            doc.namespaces(),
            &HashMap::from_iter([(
                NamespacePrefix::from("xbrli"),
                NamespaceUri::from("http://www.xbrl.org/2003/instance")
            )])
        );
    }

    #[test]
    fn resolve_context_instant() {
        let mut raw = RawInstance::default();
        raw.namespaces.insert(
            NamespacePrefix::from("dim"),
            NamespaceUri::from("http://www.example.com"),
        );
        raw.contexts.push(RawContext {
            id: "c1".to_string(),
            entity: RawEntity {
                identifier: "ABC".to_string(),
                scheme: "http://example.com".to_string(),
                segment_dimensions: vec![],
            },
            period: RawPeriod::Instant("2024-12-31".to_string()),
            scenario_dimensions: vec![RawDimension {
                dimension: QName::from_str("dim:Axis").unwrap(),
                member: QName::from_str("dim:Member").unwrap(),
            }],
        });

        let doc = resolve_instance(raw).unwrap();
        let ctx = doc.get_context("c1").unwrap();

        assert_eq!(
            ctx,
            &Context {
                id: ContextId::from("c1"),
                entity: EntityIdentifier {
                    scheme: "http://example.com".to_string(),
                    value: "ABC".to_string(),
                },
                period: Period::Instant {
                    date: "2024-12-31".to_string(),
                },
                dimensions: HashMap::from_iter([(
                    ExpandedName {
                        namespace_uri: NamespaceUri::from("http://www.example.com"),
                        local_name: "Axis".to_string(),
                    },
                    ExpandedName {
                        namespace_uri: NamespaceUri::from("http://www.example.com"),
                        local_name: "Member".to_string(),
                    }
                )]),
                segment_elements: vec![],
                scenario_elements: vec![],
                segment_has_instance_descendant: false,
                scenario_has_instance_descendant: false
            }
        );
    }

    #[test]
    fn resolve_context_duration() {
        let mut raw = RawInstance::default();
        raw.contexts.push(RawContext {
            id: "d1".to_string(),
            entity: RawEntity {
                identifier: "XYZ".to_string(),
                scheme: "http://example.com".to_string(),
                segment_dimensions: vec![],
            },
            period: RawPeriod::Duration {
                start_date: "2024-01-01".to_string(),
                end_date: "2024-12-31".to_string(),
            },
            scenario_dimensions: vec![],
        });

        let doc = resolve_instance(raw).unwrap();
        let ctx = doc.get_context("d1").unwrap();

        assert_eq!(
            ctx,
            &Context {
                id: ContextId::from("d1"),
                entity: EntityIdentifier {
                    scheme: "http://example.com".to_string(),
                    value: "XYZ".to_string(),
                },
                period: Period::Duration {
                    start: "2024-01-01".to_string(),
                    end: "2024-12-31".to_string(),
                },
                dimensions: HashMap::new(),
                segment_elements: vec![],
                scenario_elements: vec![],
                segment_has_instance_descendant: false,
                scenario_has_instance_descendant: false
            }
        );
    }

    #[test]
    fn resolve_unit_simple_measure() {
        let mut raw = RawInstance::default();
        raw.namespaces
            .insert("iso4217".into(), "http://www.xbrl.org/2003/iso4217".into());
        raw.units.push(RawUnit {
            id: "u1".to_string(),
            numerator: vec![QName::from_str("iso4217:EUR").unwrap()],
            denominator: vec![],
        });

        let doc = resolve_instance(raw).unwrap();
        let unit = doc.get_unit("u1").unwrap();

        assert_eq!(
            unit,
            &Unit {
                id: UnitId::from("u1"),
                numerator: vec![ExpandedName {
                    namespace_uri: NamespaceUri::from("http://www.xbrl.org/2003/iso4217"),
                    local_name: "EUR".to_string(),
                }],
                denominator: vec![],
            }
        );
    }

    #[test]
    fn resolve_unit_divide() {
        let mut raw = RawInstance::default();
        raw.namespaces
            .insert("iso4217".into(), "http://www.xbrl.org/2003/iso4217".into());
        raw.namespaces
            .insert("xbrli".into(), "http://www.xbrl.org/2003/instance".into());
        raw.units.push(RawUnit {
            id: "u2".to_string(),
            numerator: vec![QName::from_str("iso4217:EUR").unwrap()],
            denominator: vec![QName::from_str("xbrli:shares").unwrap()],
        });

        let doc = resolve_instance(raw).unwrap();
        let unit = doc.get_unit("u2").unwrap();

        assert_eq!(
            unit,
            &Unit {
                id: UnitId::from("u2"),
                numerator: vec![ExpandedName {
                    namespace_uri: NamespaceUri::from("http://www.xbrl.org/2003/iso4217"),
                    local_name: "EUR".to_string(),
                }],
                denominator: vec![ExpandedName {
                    namespace_uri: NamespaceUri::from("http://www.xbrl.org/2003/instance"),
                    local_name: "shares".to_string(),
                }],
            }
        );
    }

    #[test]
    fn resolve_item_fact_with_decimals() {
        let mut raw = RawInstance::default();
        raw.namespaces.insert(
            NamespacePrefix::from("ifrs"),
            NamespaceUri::from("http://example.com"),
        );
        raw.facts.push(RawFact::Item(RawItemFact {
            name: QName::from_str("ifrs:Revenue").unwrap(),
            value: "1200000".to_string(),
            context_ref: "c1".to_string(),
            unit_ref: Some("u1".to_string()),
            decimals: Some("-3".to_string()),
            precision: None,
            id: Some("f1".to_string()),
            is_nil: false,
        }));

        let doc = resolve_instance(raw).unwrap();
        let fact = doc.facts()[0].as_item().unwrap();

        assert_eq!(fact.id(), Some("f1"));
        assert_eq!(
            fact.concept_name().to_string(),
            "{http://example.com}Revenue"
        );
        assert_eq!(fact.context_ref(), "c1");
        assert_eq!(fact.unit_ref(), Some("u1"));
        assert_eq!(fact.value(), "1200000");
        assert_eq!(fact.decimals(), Some(&Decimals::Finite(-3)));
        assert_eq!(fact.precision(), None);
        assert!(!fact.is_nil());
    }

    #[test]
    fn resolve_tuple_fact_with_children() {
        let mut raw = RawInstance::default();
        raw.namespaces.insert(
            NamespacePrefix::from("t"),
            NamespaceUri::from("http://example.com"),
        );
        raw.facts.push(RawFact::Tuple(RawTupleFact {
            name: QName::from_str("t:Address").unwrap(),
            id: None,
            is_nil: false,
            children: vec![
                RawFact::Item(RawItemFact {
                    name: QName::from_str("t:Street").unwrap(),
                    value: "Main St".to_string(),
                    context_ref: "c1".to_string(),
                    unit_ref: None,
                    decimals: None,
                    precision: None,
                    id: None,
                    is_nil: false,
                }),
                RawFact::Item(RawItemFact {
                    name: QName::from_str("t:City").unwrap(),
                    value: "Berlin".to_string(),
                    context_ref: "c1".to_string(),
                    unit_ref: None,
                    decimals: None,
                    precision: None,
                    id: None,
                    is_nil: false,
                }),
            ],
        }));

        let doc = resolve_instance(raw).unwrap();
        let tuple = doc.facts()[0].as_tuple().unwrap();

        assert_eq!(tuple.id(), None);
        assert_eq!(
            tuple.concept_name().to_string(),
            "{http://example.com}Address"
        );
        assert_eq!(tuple.children().len(), 2);
        assert_matches!(&tuple.children()[0], Fact::Item(item) => {
            assert_eq!(item.concept_name().to_string(), "{http://example.com}Street");
            assert_eq!(item.value(), "Main St");
            assert_eq!(item.context_ref(), "c1");
            assert_eq!(item.unit_ref(), None);
            assert_eq!(item.decimals(), None);
            assert_eq!(item.precision(), None);
            assert!(!item.is_nil());
        } );
        assert_matches!(&tuple.children()[1], Fact::Item(item) => {
            assert_eq!(item.concept_name().to_string(), "{http://example.com}City");
            assert_eq!(item.value(), "Berlin");
            assert_eq!(item.context_ref(), "c1");
            assert_eq!(item.unit_ref(), None);
            assert_eq!(item.decimals(), None);
            assert_eq!(item.precision(), None);
            assert!(!item.is_nil());
        } );
    }

    #[test]
    fn resolve_nil_fact() {
        let mut raw = RawInstance::default();
        raw.namespaces.insert(
            NamespacePrefix::from("ifrs"),
            NamespaceUri::from("http://example.com"),
        );
        raw.facts.push(RawFact::Item(RawItemFact {
            name: QName::from_str("ifrs:Revenue").unwrap(),
            value: String::new(),
            context_ref: "c1".to_string(),
            unit_ref: None,
            decimals: None,
            precision: None,
            id: None,
            is_nil: true,
        }));

        let doc = resolve_instance(raw).unwrap();
        let fact = doc.facts()[0].as_item().unwrap();

        assert!(fact.is_nil());
        assert_eq!(fact.value(), "");
        assert_eq!(
            fact.concept_name().to_string(),
            "{http://example.com}Revenue"
        );
    }

    #[test]
    fn resolve_unit_known_namespace_fallback() {
        let mut raw = RawInstance::default();
        // No iso4217 in namespaces: should fall back to known_unit_namespace
        raw.units.push(RawUnit {
            id: "u1".to_string(),
            numerator: vec![QName::from_str("iso4217:USD").unwrap()],
            denominator: vec![],
        });

        let doc = resolve_instance(raw).unwrap();
        let unit = doc.get_unit("u1").unwrap();

        assert_eq!(
            unit,
            &Unit {
                id: UnitId::from("u1"),
                numerator: vec![ExpandedName {
                    namespace_uri: NamespaceUri::from("http://www.xbrl.org/2003/iso4217"),
                    local_name: "USD".to_string(),
                }],
                denominator: vec![],
            }
        );
    }
}
