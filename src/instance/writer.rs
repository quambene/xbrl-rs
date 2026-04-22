//! XBRL instance XML writer (serialization).

use crate::{
    Context, Fact, InstanceDocument, ItemFact, NamespacePrefix, NamespaceUri, Period, QName,
    TupleFact, error::Result, instance::Unit,
};
use quick_xml::{
    Writer,
    events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event},
};
use std::{collections::HashMap, io};

/// The writer for XBRL instance documents.
pub struct InstanceWriter<W> {
    /// The XML writer for the instance document.
    writer: Writer<W>,
    /// Flag to indicate if the root element is an XBRL instance element.
    is_xbrl_root: bool,
}

impl<W: io::Write> InstanceWriter<W> {
    pub fn new(writer: Writer<W>, is_xbrl_root: bool) -> Self {
        Self {
            writer,
            is_xbrl_root,
        }
    }

    /// Consume the writer and return the underlying writer.
    pub fn into_inner(self) -> W {
        self.writer.into_inner()
    }

    /// Serialize [`InstanceDocument`] to an XBRL XML document.
    ///
    /// When `xbrl_root` is `true`, the XML declaration is omitted (e.g. when embedding
    /// the XBRL element inside an outer document).
    pub fn write(&mut self, instance: &InstanceDocument) -> Result<()> {
        // XML declaration is only needed if the root element is <xbrli:xbrl>.
        if self.is_xbrl_root {
            self.writer
                .write_event(Event::Decl(BytesDecl::new("1.0", Some("utf-8"), None)))?;
        }

        let prefix_to_uri = instance.namespaces();
        let uri_to_prefix: HashMap<NamespaceUri, NamespacePrefix> = prefix_to_uri
            .iter()
            .map(|(prefix, uri)| (uri.clone(), prefix.clone()))
            .collect();
        let mut namespaces: Vec<_> = prefix_to_uri.iter().collect();
        namespaces.sort_by_key(|(prefix, _)| *prefix);

        // <xbrli:xbrl> root element with namespace declarations
        let mut root = BytesStart::new("xbrli:xbrl");

        for (prefix, uri) in &namespaces {
            let attr_name = format!("xmlns:{prefix}");
            root.push_attribute((attr_name.as_str(), uri.as_str()));
        }

        self.writer.write_event(Event::Start(root))?;

        // <link:schemaRef> elements
        for href in instance.schema_refs() {
            let mut elem = BytesStart::new("link:schemaRef");
            elem.push_attribute(("xlink:type", "simple"));
            elem.push_attribute(("xlink:href", href.as_str()));
            self.writer.write_event(Event::Empty(elem))?;
        }

        // <link:roleRef> elements
        for role_uri in instance.role_refs() {
            let mut elem = BytesStart::new("link:roleRef");
            elem.push_attribute(("roleURI", role_uri.as_str()));
            elem.push_attribute(("xlink:type", "simple"));
            elem.push_attribute(("xlink:href", ""));
            self.writer.write_event(Event::Empty(elem))?;
        }

        // <link:arcroleRef> elements
        for arcrole_uri in instance.arcrole_refs() {
            let mut elem = BytesStart::new("link:arcroleRef");
            elem.push_attribute(("arcroleURI", arcrole_uri.as_str()));
            elem.push_attribute(("xlink:type", "simple"));
            elem.push_attribute(("xlink:href", ""));
            self.writer.write_event(Event::Empty(elem))?;
        }

        // <xbrli:context> elements
        let mut ctx_sorted: Vec<_> = instance.contexts().iter().collect();
        ctx_sorted.sort_by_key(|(id, _)| *id);
        for (_, context) in &ctx_sorted {
            write_context(&mut self.writer, context, &uri_to_prefix)?;
        }

        // <xbrli:unit> elements
        let mut unit_sorted: Vec<_> = instance.units().iter().collect();
        unit_sorted.sort_by_key(|(id, _)| *id);
        for (_, unit) in &unit_sorted {
            write_unit(&mut self.writer, unit, &uri_to_prefix)?;
        }

        for fact in instance.facts() {
            write_fact(&mut self.writer, fact, &uri_to_prefix)?;
        }

        // </xbrli:xbrl>
        self.writer
            .write_event(Event::End(BytesEnd::new("xbrli:xbrl")))?;

        Ok(())
    }
}

fn write_context<W: std::io::Write>(
    writer: &mut Writer<W>,
    context: &Context,
    uri_to_prefix: &HashMap<NamespaceUri, NamespacePrefix>,
) -> Result<()> {
    let mut element = BytesStart::new("xbrli:context");
    element.push_attribute(("id", context.id.as_str()));
    writer.write_event(Event::Start(element))?;

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
            let dimension = QName {
                prefix: uri_to_prefix.get(&dimension.namespace_uri).cloned(),
                local_name: dimension.local_name.clone(),
            }
            .to_string();
            let member = QName {
                prefix: uri_to_prefix.get(&member.namespace_uri).cloned(),
                local_name: member.local_name.clone(),
            }
            .to_string();

            explicit.push_attribute(("dimension", dimension.as_str()));

            if member.is_empty() {
                writer.write_event(Event::Empty(explicit))?;
            } else {
                writer.write_event(Event::Start(explicit))?;
                writer.write_event(Event::Text(BytesText::new(&member)))?;
                writer.write_event(Event::End(BytesEnd::new("xbrldi:explicitMember")))?;
            }
        }
        writer.write_event(Event::End(BytesEnd::new("xbrli:scenario")))?;
    }

    writer.write_event(Event::End(BytesEnd::new("xbrli:context")))?;

    Ok(())
}

fn write_unit<W: io::Write>(
    writer: &mut Writer<W>,
    unit: &Unit,
    uri_to_prefix: &HashMap<NamespaceUri, NamespacePrefix>,
) -> Result<()> {
    let mut elem = BytesStart::new("xbrli:unit");
    elem.push_attribute(("id", unit.id.as_str()));
    writer.write_event(Event::Start(elem))?;

    if unit.has_denominator() {
        writer.write_event(Event::Start(BytesStart::new("xbrli:divide")))?;
        writer.write_event(Event::Start(BytesStart::new("xbrli:numerator")))?;

        for measure in &unit.numerator {
            let qname = QName {
                prefix: uri_to_prefix.get(&measure.namespace_uri).cloned(),
                local_name: measure.local_name.clone(),
            };
            writer.write_event(Event::Start(BytesStart::new("xbrli:measure")))?;
            writer.write_event(Event::Text(BytesText::new(&qname.to_string())))?;
            writer.write_event(Event::End(BytesEnd::new("xbrli:measure")))?;
        }

        writer.write_event(Event::End(BytesEnd::new("xbrli:numerator")))?;
        writer.write_event(Event::Start(BytesStart::new("xbrli:denominator")))?;

        for measure in &unit.denominator {
            let qname = QName {
                prefix: uri_to_prefix.get(&measure.namespace_uri).cloned(),
                local_name: measure.local_name.clone(),
            };
            writer.write_event(Event::Start(BytesStart::new("xbrli:measure")))?;
            writer.write_event(Event::Text(BytesText::new(&qname.to_string())))?;
            writer.write_event(Event::End(BytesEnd::new("xbrli:measure")))?;
        }

        writer.write_event(Event::End(BytesEnd::new("xbrli:denominator")))?;
        writer.write_event(Event::End(BytesEnd::new("xbrli:divide")))?;
    } else {
        for measure in &unit.numerator {
            let qname = QName {
                prefix: uri_to_prefix.get(&measure.namespace_uri).cloned(),
                local_name: measure.local_name.clone(),
            };
            writer.write_event(Event::Start(BytesStart::new("xbrli:measure")))?;
            writer.write_event(Event::Text(BytesText::new(&qname.to_string())))?;
            writer.write_event(Event::End(BytesEnd::new("xbrli:measure")))?;
        }
    }

    writer.write_event(Event::End(BytesEnd::new("xbrli:unit")))?;

    Ok(())
}

fn write_fact<W: io::Write>(
    writer: &mut Writer<W>,
    fact: &Fact,
    namespaces: &HashMap<NamespaceUri, NamespacePrefix>,
) -> Result<()> {
    match fact {
        Fact::Item(item) => write_item_fact(writer, item, namespaces),
        Fact::Tuple(tuple) => write_tuple_fact(writer, tuple, namespaces),
    }
}

fn write_tuple_fact<W: io::Write>(
    writer: &mut Writer<W>,
    fact: &TupleFact,
    namespaces: &HashMap<NamespaceUri, NamespacePrefix>,
) -> Result<()> {
    let concept_name = fact.concept_name();
    let prefix = namespaces.get(&concept_name.namespace_uri);
    let local_name = &concept_name.local_name;
    let concept_name = QName {
        prefix: prefix.cloned(),
        local_name: local_name.clone(),
    }
    .to_string();
    let mut element = BytesStart::new(&concept_name);

    if let Some(id) = fact.id() {
        element.push_attribute(("id", id));
    }

    if fact.is_nil() {
        element.push_attribute(("xsi:nil", "true"));
        writer.write_event(Event::Empty(element))?;
        return Ok(());
    }

    if fact.children().is_empty() {
        writer.write_event(Event::Empty(element))?;
        return Ok(());
    }

    writer.write_event(Event::Start(element))?;
    for child in fact.children() {
        write_fact(writer, child, namespaces)?;
    }
    writer.write_event(Event::End(BytesEnd::new(concept_name)))?;
    Ok(())
}

fn write_item_fact<W: std::io::Write>(
    writer: &mut Writer<W>,
    fact: &ItemFact,
    namespaces: &HashMap<NamespaceUri, NamespacePrefix>,
) -> Result<()> {
    let concept_name = fact.concept_name();
    let prefix = namespaces.get(&concept_name.namespace_uri);
    let local_name = &concept_name.local_name;
    let concept_name = QName {
        prefix: prefix.cloned(),
        local_name: local_name.clone(),
    }
    .to_string();
    let mut element = BytesStart::new(&concept_name);

    if let Some(id) = fact.id() {
        element.push_attribute(("id", id));
    }

    element.push_attribute(("contextRef", fact.context_ref()));

    if let Some(unit_ref) = fact.unit_ref() {
        element.push_attribute(("unitRef", unit_ref));
    }

    if !fact.is_nil() {
        if let Some(decimals) = fact.decimals() {
            element.push_attribute(("decimals", decimals.to_string().as_str()));
        }

        if let Some(precision) = fact.precision() {
            element.push_attribute(("precision", precision.to_string().as_str()));
        }
    }

    if fact.is_nil() {
        element.push_attribute(("xsi:nil", "true"));
        writer.write_event(Event::Empty(element))?;
    } else {
        writer.write_event(Event::Start(element))?;
        writer.write_event(Event::Text(BytesText::new(fact.value())))?;
        writer.write_event(Event::End(BytesEnd::new(&concept_name)))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use quick_xml::Writer;

    fn write_instance(instance: &InstanceDocument, is_xbrl_root: bool) -> String {
        let mut output = Vec::new();
        let mut writer = InstanceWriter::new(Writer::new(&mut output), is_xbrl_root);
        writer.write(instance).unwrap();
        String::from_utf8(output).unwrap()
    }

    #[test]
    fn test_write_instance_xbrl_root() {
        let instance = InstanceDocument::default();
        let output = write_instance(&instance, true);
        assert!(output.starts_with("<?xml version=\"1.0\" encoding=\"utf-8\"?>"));
    }

    #[test]
    fn test_write_instance_non_xbrl_root() {
        let instance = InstanceDocument::default();
        let output = write_instance(&instance, false);
        assert!(!output.starts_with("<?xml version=\"1.0\" encoding=\"utf-8\"?>"));
        assert!(output.starts_with("<xbrli:xbrl"));
    }
}
