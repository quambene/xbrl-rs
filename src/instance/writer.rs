//! XBRL instance XML writer (serialization).

use crate::{
    Context, Fact, InstanceDocument, ItemFact, Period, TupleFact, error::Result, instance::Unit,
};
use quick_xml::{
    Writer,
    events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event},
};
use std::io;

/// Serialize [`InstanceDocument`] to an XBRL XML document.
pub(crate) fn write_xml<W: io::Write>(
    writer: &mut Writer<W>,
    instance: &InstanceDocument,
) -> Result<()> {
    // XML declaration
    writer.write_event(Event::Decl(BytesDecl::new("1.0", Some("utf-8"), None)))?;

    // <xbrli:xbrl> root element with namespace declarations
    let mut root = BytesStart::new("xbrli:xbrl");
    root.push_attribute(("xmlns:xbrli", "http://www.xbrl.org/2003/instance"));
    root.push_attribute(("xmlns:link", "http://www.xbrl.org/2003/linkbase"));
    root.push_attribute(("xmlns:xlink", "http://www.w3.org/1999/xlink"));
    root.push_attribute(("xmlns:xsi", "http://www.w3.org/2001/XMLSchema-instance"));

    // Add user-defined namespace declarations
    let mut ns_sorted: Vec<_> = instance.namespaces().iter().collect();
    ns_sorted.sort_by_key(|(prefix, _)| *prefix);
    for (prefix, uri) in &ns_sorted {
        // Skip namespaces we already declared above
        if matches!(prefix.as_str(), "xbrli" | "link" | "xlink" | "xsi") {
            continue;
        }
        let attr_name = format!("xmlns:{prefix}");
        root.push_attribute((attr_name.as_str(), uri.as_str()));
    }
    writer.write_event(Event::Start(root))?;

    // <link:schemaRef> elements
    for href in instance.schema_refs() {
        let mut elem = BytesStart::new("link:schemaRef");
        elem.push_attribute(("xlink:type", "simple"));
        elem.push_attribute(("xlink:href", href.as_str()));
        writer.write_event(Event::Empty(elem))?;
    }

    // <xbrli:context> elements
    let mut ctx_sorted: Vec<_> = instance.contexts().iter().collect();
    ctx_sorted.sort_by_key(|(id, _)| *id);
    for (_, context) in &ctx_sorted {
        write_context(writer, context)?;
    }

    // <xbrli:unit> elements
    let mut unit_sorted: Vec<_> = instance.units().iter().collect();
    unit_sorted.sort_by_key(|(id, _)| *id);
    for (_, unit) in &unit_sorted {
        write_unit(writer, unit)?;
    }

    for fact in instance.facts() {
        write_fact(writer, fact)?;
    }

    // </xbrli:xbrl>
    writer.write_event(Event::End(BytesEnd::new("xbrli:xbrl")))?;

    Ok(())
}

fn write_context<W: std::io::Write>(writer: &mut Writer<W>, context: &Context) -> Result<()> {
    let mut elem = BytesStart::new("xbrli:context");
    elem.push_attribute(("id", context.id.as_str()));
    writer.write_event(Event::Start(elem))?;

    // Entity
    writer.write_event(Event::Start(BytesStart::new("xbrli:entity")))?;
    let mut identifier = BytesStart::new("xbrli:identifier");
    identifier.push_attribute(("scheme", context.entity.scheme.as_str()));
    writer.write_event(Event::Start(identifier))?;
    writer.write_event(Event::Text(BytesText::new(&context.entity.value)))?;
    writer.write_event(Event::End(BytesEnd::new("xbrli:identifier")))?;
    writer.write_event(Event::End(BytesEnd::new("xbrli:entity")))?;

    // Period
    writer.write_event(Event::Start(BytesStart::new("xbrli:period")))?;
    match &context.period {
        Period::Instant { date } => {
            writer.write_event(Event::Start(BytesStart::new("xbrli:instant")))?;
            writer.write_event(Event::Text(BytesText::new(date)))?;
            writer.write_event(Event::End(BytesEnd::new("xbrli:instant")))?;
        }
        Period::Duration { start, end } => {
            writer.write_event(Event::Start(BytesStart::new("xbrli:startDate")))?;
            writer.write_event(Event::Text(BytesText::new(start)))?;
            writer.write_event(Event::End(BytesEnd::new("xbrli:startDate")))?;
            writer.write_event(Event::Start(BytesStart::new("xbrli:endDate")))?;
            writer.write_event(Event::Text(BytesText::new(end)))?;
            writer.write_event(Event::End(BytesEnd::new("xbrli:endDate")))?;
        }
        Period::Forever => {
            writer.write_event(Event::Empty(BytesStart::new("xbrli:forever")))?;
        }
    }
    writer.write_event(Event::End(BytesEnd::new("xbrli:period")))?;

    // Dimensions (scenario)
    if !context.dimensions.is_empty() {
        writer.write_event(Event::Start(BytesStart::new("xbrli:scenario")))?;
        let mut dim_sorted: Vec<_> = context.dimensions.iter().collect();
        dim_sorted.sort_by_key(|(dim, _)| *dim);
        for (dimension, member) in &dim_sorted {
            let mut explicit = BytesStart::new("xbrldi:explicitMember");
            explicit.push_attribute(("dimension", dimension.as_str()));
            if member.is_empty() {
                writer.write_event(Event::Empty(explicit))?;
            } else {
                writer.write_event(Event::Start(explicit))?;
                writer.write_event(Event::Text(BytesText::new(member)))?;
                writer.write_event(Event::End(BytesEnd::new("xbrldi:explicitMember")))?;
            }
        }
        writer.write_event(Event::End(BytesEnd::new("xbrli:scenario")))?;
    }

    writer.write_event(Event::End(BytesEnd::new("xbrli:context")))?;

    Ok(())
}

fn write_unit<W: std::io::Write>(writer: &mut Writer<W>, unit: &Unit) -> Result<()> {
    let mut elem = BytesStart::new("xbrli:unit");
    elem.push_attribute(("id", unit.id.as_str()));
    writer.write_event(Event::Start(elem))?;

    writer.write_event(Event::Start(BytesStart::new("xbrli:measure")))?;
    writer.write_event(Event::Text(BytesText::new(&unit.measure)))?;
    writer.write_event(Event::End(BytesEnd::new("xbrli:measure")))?;

    writer.write_event(Event::End(BytesEnd::new("xbrli:unit")))?;

    Ok(())
}

fn write_fact<W: std::io::Write>(writer: &mut Writer<W>, fact: &Fact) -> Result<()> {
    match fact {
        Fact::Item(item) => write_item_fact(writer, item),
        Fact::Tuple(tuple) => write_tuple_fact(writer, tuple),
    }
}

fn write_tuple_fact<W: std::io::Write>(writer: &mut Writer<W>, fact: &TupleFact) -> Result<()> {
    let concept = fact.concept();
    let mut elem = BytesStart::new(concept);
    if let Some(id) = fact.id() {
        elem.push_attribute(("id", id));
    }

    if fact.children().is_empty() {
        writer.write_event(Event::Empty(elem))?;
        return Ok(());
    }

    writer.write_event(Event::Start(elem))?;
    for child in fact.children() {
        write_fact(writer, child)?;
    }
    writer.write_event(Event::End(BytesEnd::new(concept)))?;
    Ok(())
}

fn write_item_fact<W: std::io::Write>(writer: &mut Writer<W>, fact: &ItemFact) -> Result<()> {
    let concept = fact.concept();
    let mut elem = BytesStart::new(concept);
    if let Some(id) = fact.id() {
        elem.push_attribute(("id", id));
    }
    elem.push_attribute(("contextRef", fact.context_ref()));
    if let Some(unit_ref) = fact.unit_ref() {
        elem.push_attribute(("unitRef", unit_ref));
    }
    if let Some(decimals) = fact.decimals() {
        elem.push_attribute(("decimals", decimals.to_string().as_str()));
    }
    if let Some(precision) = fact.precision() {
        elem.push_attribute(("precision", precision.to_string().as_str()));
    }
    if fact.is_nil() {
        elem.push_attribute(("xsi:nil", "true"));
        writer.write_event(Event::Empty(elem))?;
    } else {
        writer.write_event(Event::Start(elem))?;
        writer.write_event(Event::Text(BytesText::new(fact.value())))?;
        writer.write_event(Event::End(BytesEnd::new(concept)))?;
    }

    Ok(())
}
