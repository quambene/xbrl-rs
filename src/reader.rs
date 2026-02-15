//! XBRL instance XML reader (deserialization).

use crate::{Context, EntityIdentifier, Fact, Period, Unit, XbrlInstance};
use anyhow::{Context as _, Result};
use quick_xml::{
    Reader,
    escape::unescape,
    events::{Event, attributes::Attributes},
};

/// Parse an XBRL instance document from XML content.
///
/// The raw XML may contain a wrapper around the `<xbrli:xbrl>` element; this
/// function handles extraction automatically.
pub(crate) fn parse_xml(xml_content: &str) -> Result<XbrlInstance> {
    let xml_content = extract_xbrl(xml_content);

    let mut reader = Reader::from_str(xml_content);
    reader.config_mut().trim_text_start = true;
    reader.config_mut().trim_text_end = true;

    let mut instance = XbrlInstance::new();
    let mut buf = Vec::new();
    let mut inside_xbrl = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let name = e.name();
                let name_str = String::from_utf8_lossy(name.as_ref());

                // Detect XBRL root element and extract namespaces
                if name_str.ends_with(":xbrl") || name_str == "xbrl" {
                    inside_xbrl = true;
                    extract_namespaces(&e.attributes(), &mut instance);
                }

                if !inside_xbrl {
                    buf.clear();
                    continue;
                }

                // Parse different XBRL elements
                if name_str.ends_with(":schemaRef") {
                    if let Some(href) = get_attribute(&e.attributes(), b"xlink:href") {
                        instance.add_schema_ref(href);
                    }
                } else if name_str.ends_with(":context") {
                    let context = parse_context(&mut reader, &e)?;
                    instance.add_context(context);
                } else if name_str.ends_with(":unit") {
                    let unit = parse_unit(&mut reader, &e)?;
                    instance.add_unit(unit);
                } else if inside_xbrl
                    && is_fact_element(&name_str)
                    && let Some(fact) = parse_fact(&mut reader, &e, &name_str)?
                {
                    instance.add_fact(fact);
                }
            }
            Ok(Event::End(e)) => {
                let name_bytes = e.name();
                let name_str = String::from_utf8_lossy(name_bytes.as_ref());
                if name_str.ends_with(":xbrl") || name_str == "xbrl" {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "Error parsing XBRL at position {}: {}",
                    reader.buffer_position(),
                    e
                ));
            }
            _ => {}
        }
        buf.clear();
    }

    Ok(instance)
}

/// Extract the `<xbrli:xbrl>` content from an XML document.
///
/// If no wrapper is detected the input is returned unchanged.
fn extract_xbrl(xml: &str) -> &str {
    if let Some(start) = xml.find("<xbrli:xbrl")
        && let Some(end) = xml.rfind("</xbrli:xbrl>")
    {
        return &xml[start..end + "</xbrli:xbrl>".len()];
    }

    xml
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
    let id = get_attribute(&start_element.attributes(), b"id")
        .context("Context missing id attribute")?;

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

                if name.ends_with(":identifier") {
                    entity_scheme = get_attribute(&e.attributes(), b"scheme");
                    let mut text_buf = Vec::new();
                    if let Ok(Event::Text(t)) = reader.read_event_into(&mut text_buf) {
                        let text_str = std::str::from_utf8(t.as_ref())?;
                        entity_value = Some(unescape(text_str)?.into_owned());
                    }
                } else if name.ends_with(":instant") {
                    let mut text_buf = Vec::new();
                    if let Ok(Event::Text(t)) = reader.read_event_into(&mut text_buf) {
                        let text_str = std::str::from_utf8(t.as_ref())?;
                        period_instant = Some(unescape(text_str)?.into_owned());
                    }
                } else if name.ends_with(":startDate") {
                    let mut text_buf = Vec::new();
                    if let Ok(Event::Text(t)) = reader.read_event_into(&mut text_buf) {
                        let text_str = std::str::from_utf8(t.as_ref())?;
                        period_start = Some(unescape(text_str)?.into_owned());
                    }
                } else if name.ends_with(":endDate") {
                    let mut text_buf = Vec::new();
                    if let Ok(Event::Text(t)) = reader.read_event_into(&mut text_buf) {
                        let text_str = std::str::from_utf8(t.as_ref())?;
                        period_end = Some(unescape(text_str)?.into_owned());
                    }
                } else if name.ends_with(":explicitMember") {
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
                if name.ends_with(":explicitMember") {
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
            Err(e) => return Err(anyhow::anyhow!("Error parsing context: {}", e)),
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
    let id = get_attribute(&start_element.attributes(), b"id")
        .context("Unit missing id attribute")?;

    let mut measure = String::new();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                let name_bytes = e.name();
                let name = String::from_utf8_lossy(name_bytes.as_ref());
                if name.ends_with(":measure") {
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
                if name.ends_with(":unit") {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(anyhow::anyhow!("Error parsing unit: {}", e)),
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

    Ok(Some(fact))
}

/// Check if an element name represents a fact.
fn is_fact_element(name: &str) -> bool {
    // Facts are elements with namespace prefixes from taxonomies
    name.contains(':')
        && !name.ends_with(":xbrl")
        && !name.ends_with(":context")
        && !name.ends_with(":unit")
        && !name.ends_with(":schemaRef")
        && !name.ends_with(":identifier")
        && !name.ends_with(":entity")
        && !name.ends_with(":period")
        && !name.ends_with(":instant")
        && !name.ends_with(":startDate")
        && !name.ends_with(":endDate")
        && !name.ends_with(":scenario")
        && !name.ends_with(":explicitMember")
        && !name.ends_with(":measure")
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
