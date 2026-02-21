//! XBRL instance XML reader (deserialization).

use crate::{
    Context, EntityIdentifier, Fact, Period, XbrlInstance,
    error::{Result, XbrlError},
    instance::Unit,
};
use quick_xml::{
    Reader,
    escape::unescape,
    events::{Event, attributes::Attributes},
};
use std::io;

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
                        instance.add_schema_ref(href);
                    }
                } else if name_matches(&name_str, "context") {
                    let context = parse_context(reader, &e)?;
                    instance.add_context(context);
                } else if name_matches(&name_str, "unit") {
                    let unit = parse_unit(reader, &e)?;
                    instance.add_unit(unit);
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

/// Extract namespace declarations from the xbrl element.
fn extract_namespaces(attributes: &Attributes, instance: &mut XbrlInstance) {
    for attr in attributes.clone().flatten() {
        let key = String::from_utf8_lossy(attr.key.as_ref());
        if key.starts_with("xmlns:") {
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
    let mut dimensions = Vec::new();

    let mut buf = Vec::new();
    let mut depth = 1;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                depth += 1;
                let name_bytes = e.name();
                let name = String::from_utf8_lossy(name_bytes.as_ref());

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
                if name_matches(&name, "explicitMember") {
                    let dim = get_attribute(&e.attributes(), b"dimension");
                    if let Some(dimension) = dim {
                        dimensions.push((dimension, String::new()));
                    }
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
    let period = if let Some(instant) = period_instant {
        Period::Instant { date: instant }
    } else if let (Some(start), Some(end)) = (period_start, period_end) {
        Period::Duration { start, end }
    } else {
        Period::Instant {
            date: String::new(),
        }
    };

    let mut context = Context::new(id, entity, period);
    for (dim, member) in dimensions {
        context.add_dimension(dim, member);
    }

    Ok(context)
}

/// Parse a unit element.
fn parse_unit<R: std::io::BufRead>(
    reader: &mut Reader<R>,
    start_element: &quick_xml::events::BytesStart,
) -> Result<Unit> {
    let id = get_attribute(&start_element.attributes(), b"id").ok_or_else(|| {
        XbrlError::MissingAttribute {
            element: "unit".to_string(),
            attribute: "id".to_string(),
        }
    })?;

    let mut measure = String::new();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name_bytes = e.name();
                let name = String::from_utf8_lossy(name_bytes.as_ref());
                if name_matches(&name, "measure") {
                    let mut text_buf = Vec::new();
                    if let Ok(Event::Text(t)) = reader.read_event_into(&mut text_buf) {
                        let text_str = std::str::from_utf8(t.as_ref())?;
                        measure = unescape(text_str)?.into_owned();
                    }
                }
            }
            Ok(Event::End(e)) => {
                let name_bytes = e.name();
                let name = String::from_utf8_lossy(name_bytes.as_ref());
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

    Ok(Unit::new(id, measure))
}

/// Parse a fact element.
fn parse_fact<R: std::io::BufRead>(
    reader: &mut Reader<R>,
    start_element: &quick_xml::events::BytesStart,
    concept: &str,
) -> Result<Option<Fact>> {
    let context_ref = get_attribute(&start_element.attributes(), b"contextRef");
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

fn get_attribute_local(attributes: &Attributes, local: &str) -> Option<String> {
    for attr in attributes.clone().flatten() {
        let key = String::from_utf8_lossy(attr.key.as_ref());
        if key.rsplit(':').next() == Some(local) {
            return attr.unescape_value().ok().map(|v| v.to_string());
        }
    }
    None
}
