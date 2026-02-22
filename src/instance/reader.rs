//! XBRL instance XML reader (deserialization).

use crate::{
    Context, ContextId, EntityIdentifier, Fact, Period, XbrlInstance,
    error::{Result, XbrlError},
    instance::{
        FootnoteArc, FootnoteLink, FootnoteLocator, FootnoteResource, NamespacePrefix, Unit,
        UnitId, unit::UnitMeasure,
    },
};
use quick_xml::{
    Reader,
    escape::unescape,
    events::{BytesStart, Event, attributes::Attributes},
};
use std::{collections::HashMap, io};

fn local_name(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}

fn name_matches(name: &str, expected_local: &str) -> bool {
    local_name(name) == expected_local
}

/// Parse an XBRL instance document from XML content.
///
/// The raw XML may contain a wrapper around the `<xbrli:xbrl>` element; this
/// function handles extraction automatically.
pub(crate) fn read_xml<R>(reader: &mut Reader<R>) -> Result<XbrlInstance>
where
    R: io::BufRead,
{
    reader.config_mut().trim_text_start = true;
    reader.config_mut().trim_text_end = true;

    let mut instance = XbrlInstance::default();
    let mut buf = Vec::new();
    let mut inside_xbrl = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let name = e.name();
                let name_str = String::from_utf8_lossy(name.as_ref());

                // Detect XBRL root element and extract namespaces
                if name_matches(&name_str, "xbrl") {
                    inside_xbrl = true;
                    extract_namespaces(&e.attributes(), &mut instance);
                    instance.set_root_xml_lang(
                        get_attribute(&e.attributes(), b"xml:lang")
                            .or_else(|| get_attribute_local(&e.attributes(), "lang")),
                    );
                }

                if !inside_xbrl {
                    buf.clear();
                    continue;
                }

                // Parse different XBRL elements
                if name_matches(&name_str, "schemaRef") {
                    if let Some(href) = get_attribute(&e.attributes(), b"xlink:href")
                        .or_else(|| get_attribute_local(&e.attributes(), "href"))
                    {
                        let resolved_href = get_attribute(&e.attributes(), b"xml:base")
                            .or_else(|| get_attribute_local(&e.attributes(), "base"))
                            .map(|xml_base| resolve_xml_base_href(&xml_base, &href))
                            .unwrap_or(href);
                        instance.add_schema_ref(resolved_href);
                    }
                } else if name_matches(&name_str, "roleRef") {
                    if let Some(role_uri) = get_attribute(&e.attributes(), b"roleURI")
                        .or_else(|| get_attribute_local(&e.attributes(), "roleURI"))
                    {
                        instance.add_role_ref(role_uri);
                    }
                } else if name_matches(&name_str, "arcroleRef") {
                    if let Some(arcrole_uri) = get_attribute(&e.attributes(), b"arcroleURI")
                        .or_else(|| get_attribute_local(&e.attributes(), "arcroleURI"))
                    {
                        instance.add_arcrole_ref(arcrole_uri);
                    }
                } else if name_matches(&name_str, "context") {
                    let namespaces = instance.namespaces().clone();
                    let context = parse_context(reader, &e, &namespaces)?;
                    instance.add_context(context);
                } else if name_matches(&name_str, "unit") {
                    let namespaces = instance.namespaces().clone();
                    let unit = parse_unit(reader, &e, &namespaces)?;
                    instance.add_unit(unit);
                } else if name_matches(&name_str, "footnoteLink") {
                    let footnote_link = parse_footnote_link(reader, &e)?;
                    instance.add_footnote_link(footnote_link);
                } else if inside_xbrl
                    && is_fact_element(&name_str)
                    && let Some(fact) = parse_fact(reader, &e, &name_str)?
                {
                    instance.add_fact(fact);
                }
            }
            Ok(Event::End(e)) => {
                let name_bytes = e.name();
                let name_str = String::from_utf8_lossy(name_bytes.as_ref());
                if name_matches(&name_str, "xbrl") {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => {
                return Err(XbrlError::XmlParse {
                    position: reader.buffer_position(),
                    element: None,
                    source: err,
                });
            }
            _ => {}
        }
        buf.clear();
    }

    Ok(instance)
}

fn resolve_xml_base_href(xml_base: &str, href: &str) -> String {
    if href.contains("://") || href.starts_with('/') {
        return href.to_string();
    }

    let base = xml_base.trim();
    if base.is_empty() {
        return href.to_string();
    }

    if base.ends_with('/') {
        format!("{base}{href}")
    } else {
        format!("{base}/{href}")
    }
}

/// Extract namespace declarations from the xbrl element.
fn extract_namespaces(attributes: &Attributes, instance: &mut XbrlInstance) {
    for attr in attributes.clone().flatten() {
        let key = String::from_utf8_lossy(attr.key.as_ref());
        if key == "xmlns" {
            let uri = String::from_utf8_lossy(&attr.value).to_string();
            instance.add_namespace(String::new(), uri);
        } else if key.starts_with("xmlns:") {
            let prefix = key.strip_prefix("xmlns:").unwrap_or("");
            let uri = String::from_utf8_lossy(&attr.value).to_string();
            instance.add_namespace(prefix.to_string(), uri);
        }
    }
}

/// Parse a context element.
fn parse_context<R: std::io::BufRead>(
    reader: &mut Reader<R>,
    start_element: &quick_xml::events::BytesStart,
    namespaces: &HashMap<NamespacePrefix, String>,
) -> Result<Context> {
    let id = get_attribute(&start_element.attributes(), b"id").ok_or_else(|| {
        XbrlError::MissingAttribute {
            element: "context".to_string(),
            attribute: "id".to_string(),
        }
    })?;

    let mut entity_scheme = None;
    let mut entity_value = None;
    let mut period_instant = None;
    let mut period_start = None;
    let mut period_end = None;
    let mut period_forever = false;
    let mut dimensions = Vec::new();
    let mut segment_elements = Vec::new();
    let mut scenario_elements = Vec::new();
    let mut segment_has_instance_descendant = false;
    let mut scenario_has_instance_descendant = false;
    let mut in_segment = false;
    let mut in_scenario = false;

    let mut buf = Vec::new();
    let mut depth = 1;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                depth += 1;
                let name_bytes = e.name();
                let name = String::from_utf8_lossy(name_bytes.as_ref());

                if name_matches(&name, "segment") {
                    in_segment = true;
                } else if name_matches(&name, "scenario") {
                    in_scenario = true;
                } else if in_segment || in_scenario {
                    let ns = resolve_element_namespace(&name, &e.attributes(), namespaces);
                    if ns.as_deref() == Some("http://www.xbrl.org/2003/instance") {
                        if in_segment {
                            segment_has_instance_descendant = true;
                        } else {
                            scenario_has_instance_descendant = true;
                        }
                    }

                    if in_segment {
                        segment_elements.push(name.to_string());
                    } else {
                        scenario_elements.push(name.to_string());
                    }
                }

                if name_matches(&name, "identifier") {
                    entity_scheme = get_attribute(&e.attributes(), b"scheme");
                    let mut text_buf = Vec::new();
                    if let Ok(Event::Text(t)) = reader.read_event_into(&mut text_buf) {
                        let text_str = std::str::from_utf8(t.as_ref())?;
                        entity_value = Some(unescape(text_str)?.into_owned());
                    }
                } else if name_matches(&name, "instant") {
                    let mut text_buf = Vec::new();
                    if let Ok(Event::Text(t)) = reader.read_event_into(&mut text_buf) {
                        let text_str = std::str::from_utf8(t.as_ref())?;
                        period_instant = Some(unescape(text_str)?.into_owned());
                    }
                } else if name_matches(&name, "startDate") {
                    let mut text_buf = Vec::new();
                    if let Ok(Event::Text(t)) = reader.read_event_into(&mut text_buf) {
                        let text_str = std::str::from_utf8(t.as_ref())?;
                        period_start = Some(unescape(text_str)?.into_owned());
                    }
                } else if name_matches(&name, "endDate") {
                    let mut text_buf = Vec::new();
                    if let Ok(Event::Text(t)) = reader.read_event_into(&mut text_buf) {
                        let text_str = std::str::from_utf8(t.as_ref())?;
                        period_end = Some(unescape(text_str)?.into_owned());
                    }
                } else if name_matches(&name, "forever") {
                    period_forever = true;
                } else if name_matches(&name, "explicitMember") {
                    let dim = get_attribute(&e.attributes(), b"dimension");
                    let mut text_buf = Vec::new();
                    if let (Some(dimension), Ok(Event::Text(t))) =
                        (dim, reader.read_event_into(&mut text_buf))
                    {
                        let text_str = std::str::from_utf8(t.as_ref())?;
                        dimensions.push((dimension, unescape(text_str)?.into_owned()));
                    }
                }
            }
            Ok(Event::Empty(e)) => {
                let name_bytes = e.name();
                let name = String::from_utf8_lossy(name_bytes.as_ref());
                if name_matches(&name, "forever") {
                    period_forever = true;
                }

                if in_segment || in_scenario {
                    let ns = resolve_element_namespace(&name, &e.attributes(), namespaces);
                    if ns.as_deref() == Some("http://www.xbrl.org/2003/instance") {
                        if in_segment {
                            segment_has_instance_descendant = true;
                        } else {
                            scenario_has_instance_descendant = true;
                        }
                    }

                    if in_segment {
                        segment_elements.push(name.to_string());
                    } else {
                        scenario_elements.push(name.to_string());
                    }
                }

                if name_matches(&name, "explicitMember") {
                    let dim = get_attribute(&e.attributes(), b"dimension");
                    if let Some(dimension) = dim {
                        dimensions.push((dimension, String::new()));
                    }
                }
            }
            Ok(Event::End(e)) => {
                let name_bytes = e.name();
                let name = String::from_utf8_lossy(name_bytes.as_ref());
                if name_matches(&name, "segment") {
                    in_segment = false;
                } else if name_matches(&name, "scenario") {
                    in_scenario = false;
                }
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => {
                return Err(XbrlError::XmlParse {
                    position: reader.buffer_position(),
                    element: Some("context".to_string()),
                    source: err,
                });
            }
            _ => {}
        }
        buf.clear();
    }

    let entity = EntityIdentifier {
        scheme: entity_scheme.unwrap_or_default(),
        value: entity_value.unwrap_or_default(),
    };

    // Determine period type
    let period = if period_forever {
        Period::Forever
    } else if let Some(instant) = period_instant {
        Period::Instant { date: instant }
    } else if let (Some(start), Some(end)) = (period_start, period_end) {
        Period::Duration { start, end }
    } else {
        Period::Instant {
            date: String::new(),
        }
    };

    let mut context = Context::new(ContextId::from(id), entity, period);
    for (dim, member) in dimensions {
        context.add_dimension(dim, member);
    }
    context.segment_elements = segment_elements;
    context.scenario_elements = scenario_elements;
    context.segment_has_instance_descendant = segment_has_instance_descendant;
    context.scenario_has_instance_descendant = scenario_has_instance_descendant;

    Ok(context)
}

fn parse_footnote_link<R: io::BufRead>(
    reader: &mut Reader<R>,
    start_element: &BytesStart,
) -> Result<FootnoteLink> {
    let mut link = FootnoteLink {
        role: get_attribute(&start_element.attributes(), b"xlink:role")
            .or_else(|| get_attribute_local(&start_element.attributes(), "role")),
        xml_lang: get_attribute(&start_element.attributes(), b"xml:lang")
            .or_else(|| get_attribute_local(&start_element.attributes(), "lang")),
        ..FootnoteLink::default()
    };

    let mut depth = 1;
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                depth += 1;
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let local = local_name(&name).to_string();

                if local == "loc" || is_locator_like(&e.attributes()) {
                    link.locators.push(FootnoteLocator {
                        element_local_name: local,
                        label: get_attribute(&e.attributes(), b"xlink:label")
                            .or_else(|| get_attribute_local(&e.attributes(), "label")),
                        href: get_attribute(&e.attributes(), b"xlink:href")
                            .or_else(|| get_attribute_local(&e.attributes(), "href")),
                    });
                } else if local == "footnote" {
                    link.footnotes.push(FootnoteResource {
                        label: get_attribute(&e.attributes(), b"xlink:label")
                            .or_else(|| get_attribute_local(&e.attributes(), "label")),
                        id: get_attribute(&e.attributes(), b"id"),
                        role: get_attribute(&e.attributes(), b"xlink:role")
                            .or_else(|| get_attribute_local(&e.attributes(), "role")),
                        xml_lang: get_attribute(&e.attributes(), b"xml:lang")
                            .or_else(|| get_attribute_local(&e.attributes(), "lang")),
                    });
                } else if local.ends_with("Arc") || is_arc_like(&e.attributes()) {
                    link.arcs.push(FootnoteArc {
                        from: get_attribute(&e.attributes(), b"xlink:from")
                            .or_else(|| get_attribute_local(&e.attributes(), "from")),
                        to: get_attribute(&e.attributes(), b"xlink:to")
                            .or_else(|| get_attribute_local(&e.attributes(), "to")),
                        arcrole: get_attribute(&e.attributes(), b"xlink:arcrole")
                            .or_else(|| get_attribute_local(&e.attributes(), "arcrole")),
                    });
                }
            }
            Ok(Event::Empty(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let local = local_name(&name).to_string();

                if local == "loc" || is_locator_like(&e.attributes()) {
                    link.locators.push(FootnoteLocator {
                        element_local_name: local,
                        label: get_attribute(&e.attributes(), b"xlink:label")
                            .or_else(|| get_attribute_local(&e.attributes(), "label")),
                        href: get_attribute(&e.attributes(), b"xlink:href")
                            .or_else(|| get_attribute_local(&e.attributes(), "href")),
                    });
                } else if local == "footnote" {
                    link.footnotes.push(FootnoteResource {
                        label: get_attribute(&e.attributes(), b"xlink:label")
                            .or_else(|| get_attribute_local(&e.attributes(), "label")),
                        id: get_attribute(&e.attributes(), b"id"),
                        role: get_attribute(&e.attributes(), b"xlink:role")
                            .or_else(|| get_attribute_local(&e.attributes(), "role")),
                        xml_lang: get_attribute(&e.attributes(), b"xml:lang")
                            .or_else(|| get_attribute_local(&e.attributes(), "lang")),
                    });
                } else if local.ends_with("Arc") || is_arc_like(&e.attributes()) {
                    link.arcs.push(FootnoteArc {
                        from: get_attribute(&e.attributes(), b"xlink:from")
                            .or_else(|| get_attribute_local(&e.attributes(), "from")),
                        to: get_attribute(&e.attributes(), b"xlink:to")
                            .or_else(|| get_attribute_local(&e.attributes(), "to")),
                        arcrole: get_attribute(&e.attributes(), b"xlink:arcrole")
                            .or_else(|| get_attribute_local(&e.attributes(), "arcrole")),
                    });
                }
            }
            Ok(Event::End(_)) => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => {
                return Err(XbrlError::XmlParse {
                    position: reader.buffer_position(),
                    element: Some("footnoteLink".to_string()),
                    source: err,
                });
            }
            _ => {}
        }
        buf.clear();
    }

    Ok(link)
}

/// Parse a unit element.
fn parse_unit<R: std::io::BufRead>(
    reader: &mut Reader<R>,
    start_element: &quick_xml::events::BytesStart,
    namespaces: &HashMap<NamespacePrefix, String>,
) -> Result<Unit> {
    let id = get_attribute(&start_element.attributes(), b"id").ok_or_else(|| {
        XbrlError::MissingAttribute {
            element: "unit".to_string(),
            attribute: "id".to_string(),
        }
    })?;

    let mut measure = String::new();
    let mut numerator_measures = Vec::new();
    let mut denominator_measures = Vec::new();
    let mut in_denominator = false;
    let mut unit_scope = namespaces.clone();
    unit_scope.extend(collect_local_xmlns(&start_element.attributes()));
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name_bytes = e.name();
                let name = String::from_utf8_lossy(name_bytes.as_ref());
                if name_matches(&name, "unitDenominator") {
                    in_denominator = true;
                } else if name_matches(&name, "measure") {
                    let mut scope = unit_scope.clone();
                    scope.extend(collect_local_xmlns(&e.attributes()));
                    let mut text_buf = Vec::new();
                    if let Ok(Event::Text(t)) = reader.read_event_into(&mut text_buf) {
                        let text_str = std::str::from_utf8(t.as_ref())?;
                        measure = unescape(text_str)?.into_owned();

                        let parsed = parse_measure(&measure, &scope);
                        if in_denominator {
                            denominator_measures.push(parsed);
                        } else {
                            numerator_measures.push(parsed);
                        }
                    }
                }
            }
            Ok(Event::Empty(e)) => {
                let name_bytes = e.name();
                let name = String::from_utf8_lossy(name_bytes.as_ref());
                if name_matches(&name, "measure") {
                    let mut scope = unit_scope.clone();
                    scope.extend(collect_local_xmlns(&e.attributes()));
                    let parsed = parse_measure("", &scope);
                    if in_denominator {
                        denominator_measures.push(parsed);
                    } else {
                        numerator_measures.push(parsed);
                    }
                }
            }
            Ok(Event::End(e)) => {
                let name_bytes = e.name();
                let name = String::from_utf8_lossy(name_bytes.as_ref());
                if name_matches(&name, "unitDenominator") {
                    in_denominator = false;
                }
                if name_matches(&name, "unit") {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => {
                return Err(XbrlError::XmlParse {
                    position: reader.buffer_position(),
                    element: Some("unit".to_string()),
                    source: err,
                });
            }
            _ => {}
        }
        buf.clear();
    }

    let mut unit = Unit::new(UnitId::from(id), measure);
    if !numerator_measures.is_empty() || !denominator_measures.is_empty() {
        unit.set_measures(numerator_measures, denominator_measures);
    }
    Ok(unit)
}

/// Parse a fact element.
fn parse_fact<R: io::BufRead>(
    reader: &mut Reader<R>,
    start_element: &quick_xml::events::BytesStart,
    concept: &str,
) -> Result<Option<Fact>> {
    for attr in start_element.attributes().flatten() {
        let key = String::from_utf8_lossy(attr.key.as_ref());
        if key.rsplit(':').next() == Some("periodType") {
            return Err(XbrlError::Io(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("fact '{concept}' contains forbidden attribute '{key}'"),
            )));
        }
    }

    let context_ref = get_attribute(&start_element.attributes(), b"contextRef");
    let id = get_attribute(&start_element.attributes(), b"id");
    let unit_ref = get_attribute(&start_element.attributes(), b"unitRef");
    let is_nil = get_attribute(&start_element.attributes(), b"xsi:nil")
        .map(|v| v == "true")
        .unwrap_or(false);
    let decimals = get_attribute(&start_element.attributes(), b"decimals");
    let precision = get_attribute(&start_element.attributes(), b"precision");

    // Only process if we have a context reference
    let context_ref = match context_ref {
        Some(cr) => cr,
        None => return Ok(None),
    };

    let mut value = String::new();

    // Read the text content
    let mut text_buf = Vec::new();
    match reader.read_event_into(&mut text_buf) {
        Ok(Event::Text(t)) => {
            let text_str = std::str::from_utf8(t.as_ref())?;
            value = unescape(text_str)?.into_owned();
        }
        Ok(Event::End(_)) => {
            // Empty element
        }
        _ => {}
    }

    let mut fact = Fact::new(concept.to_string(), context_ref, unit_ref, value);
    if let Some(id) = id {
        fact.set_id(id);
    }
    fact.set_nil(is_nil);
    if let Some(dec) = decimals {
        fact.set_decimals(dec);
    }
    if let Some(prec) = precision {
        fact.set_precision(prec);
    }

    Ok(Some(fact))
}

/// Check if an element name represents a fact.
fn is_fact_element(name: &str) -> bool {
    let local = local_name(name);

    // Facts are elements with namespace prefixes from taxonomies
    name.contains(':')
        && local != "xbrl"
        && local != "context"
        && local != "unit"
        && local != "schemaRef"
        && local != "identifier"
        && local != "entity"
        && local != "period"
        && local != "instant"
        && local != "startDate"
        && local != "endDate"
        && local != "scenario"
        && local != "explicitMember"
        && local != "measure"
        && local != "footnoteLink"
        && local != "footnote"
        && local != "loc"
        && !local.ends_with("Arc")
}

fn is_locator_like(attributes: &Attributes) -> bool {
    get_attribute(attributes, b"xlink:type")
        .or_else(|| get_attribute_local(attributes, "type"))
        .as_deref()
        == Some("locator")
}

fn is_arc_like(attributes: &Attributes) -> bool {
    get_attribute(attributes, b"xlink:type")
        .or_else(|| get_attribute_local(attributes, "type"))
        .as_deref()
        == Some("arc")
}

/// Extract an attribute value from attributes.
fn get_attribute(attributes: &Attributes, key: &[u8]) -> Option<String> {
    for attr in attributes.clone().flatten() {
        if attr.key.as_ref() == key {
            return attr.unescape_value().ok().map(|v| v.to_string());
        }
    }
    None
}

fn collect_local_xmlns(attributes: &Attributes) -> HashMap<NamespacePrefix, String> {
    let mut out = HashMap::new();
    for attr in attributes.clone().flatten() {
        let key = String::from_utf8_lossy(attr.key.as_ref());
        let value = attr
            .unescape_value()
            .map(|v| v.to_string())
            .unwrap_or_default();
        if key == "xmlns" {
            out.insert(NamespacePrefix::from(""), value);
        } else if let Some(prefix) = key.strip_prefix("xmlns:") {
            out.insert(NamespacePrefix::from(prefix), value);
        }
    }
    out
}

fn resolve_element_namespace(
    name: &str,
    attributes: &Attributes,
    root_namespaces: &HashMap<NamespacePrefix, String>,
) -> Option<String> {
    let local = collect_local_xmlns(attributes);
    if let Some((prefix, _)) = name.split_once(':') {
        local
            .get(prefix)
            .cloned()
            .or_else(|| root_namespaces.get(prefix).cloned())
    } else {
        local
            .get("")
            .cloned()
            .or_else(|| root_namespaces.get("").cloned())
    }
}

fn parse_measure(qname: &str, namespace_scope: &HashMap<NamespacePrefix, String>) -> UnitMeasure {
    let (prefix, local_name) = if let Some((prefix, local)) = qname.split_once(':') {
        (Some(prefix.to_string()), local.to_string())
    } else {
        (None, qname.to_string())
    };

    let namespace_uri = if let Some(prefix) = prefix.as_deref() {
        namespace_scope.get(prefix).cloned()
    } else {
        namespace_scope.get("").cloned()
    };

    UnitMeasure {
        qname: qname.to_string(),
        prefix,
        local_name,
        namespace_uri,
    }
}

fn get_attribute_local(attributes: &Attributes, local: &str) -> Option<String> {
    for attr in attributes.clone().flatten() {
        let key = String::from_utf8_lossy(attr.key.as_ref());
        if key.rsplit(':').next() == Some(local) {
            return attr.unescape_value().ok().map(|v| v.to_string());
        }
    }
    None
}
