use crate::XbrlError;
use quick_xml::{
    Reader,
    events::{BytesStart, Event, attributes::Attributes},
};
use std::{collections::HashMap, io::BufRead, path::PathBuf, str::Bytes};

#[derive(Debug, PartialEq, Eq)]
pub struct SchemaRef {
    pub href: String,
}

#[derive(Debug, PartialEq, Eq)]
pub struct RoleRef {
    pub role_uri: String,
    pub href: String,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ArcroleRef {
    pub arcrole_uri: String,
    pub href: String,
}

#[derive(Debug, PartialEq, Eq)]
pub struct RawContext {
    pub id: String,
    pub entity: RawEntity,
    pub period: RawPeriod,
    pub dimensions: Vec<RawDimension>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct RawEntity {
    pub identifier: String,
    pub scheme: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum RawPeriod {
    Instant(String),
    Duration {
        start_date: String,
        end_date: String,
    },
    Forever,
}

#[derive(Debug, PartialEq, Eq)]
pub struct RawDimension {
    /// QName of the dimension
    pub dimension: String,
    /// QName of the member
    pub member: String,
}

#[derive(Debug, PartialEq, Eq)]
pub struct RawUnit {
    pub id: String,
    /// Measures like iso4217:EUR
    pub measures: Vec<String>,
    /// Divide unit (optional)
    pub divide: Option<RawUnitDivide>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct RawUnitDivide {
    pub numerator: Vec<String>,
    pub denominator: Vec<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct RawFact {
    /// QName of the concept
    pub name: String,
    /// Raw text value
    pub value: String,
    /// contextRef attribute
    pub context_ref: String,
    /// unitRef attribute
    pub unit_ref: Option<String>,
    /// decimals attribute
    pub decimals: Option<String>,
    /// precision attribute
    pub precision: Option<String>,
    /// id attribute
    pub id: Option<String>,
}

/// A locator in a footnote link, usually a `link:loc` element.
#[derive(Debug, PartialEq, Eq)]
pub struct Locator {
    /// Local name of the locator element (e.g. `loc` or a custom element).
    pub label: String,
    /// Optional `xlink:href` target, typically a same-document fragment.
    pub href: String,
}

#[derive(Debug, PartialEq, Eq)]
pub struct RawFootnoteLink {
    pub role: String,
    pub locators: Vec<Locator>,
    pub arcs: Vec<FootnoteArc>,
    pub footnotes: Vec<FootnoteResource>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct FootnoteArc {
    pub from: String,
    pub to: String,
}

#[derive(Debug, PartialEq, Eq)]
pub struct FootnoteResource {
    pub label: String,
    pub lang: Option<String>,
    pub text: String,
}

#[derive(Debug, PartialEq, Eq)]
pub struct RawInstance {
    /// Absolute file path of the instance document
    pub file_path: PathBuf,
    /// Namespace declarations (prefix -> URI)
    pub namespaces: HashMap<String, String>,
    /// Schema references
    pub schema_refs: Vec<SchemaRef>,
    /// Role references
    ///
    /// Usually defined in the linkbase, but can also be present in the instance
    /// document.
    pub role_refs: Vec<RoleRef>,
    /// Arcrole references
    ///
    /// Usually defined in the linkbase, but can also be present in the instance
    /// document.
    pub arcrole_refs: Vec<ArcroleRef>,
    /// Context definitions
    pub contexts: Vec<RawContext>,
    /// Unit definitions
    pub units: Vec<RawUnit>,
    /// All facts
    pub facts: Vec<RawFact>,
    /// Optional footnote links
    pub footnote_links: Vec<RawFootnoteLink>,
}

impl RawInstance {
    pub fn new(
        file_path: PathBuf,
        namespaces: HashMap<String, String>,
        schema_refs: Vec<SchemaRef>,
        role_refs: Vec<RoleRef>,
        arcrole_refs: Vec<ArcroleRef>,
        contexts: Vec<RawContext>,
        units: Vec<RawUnit>,
        facts: Vec<RawFact>,
        footnote_links: Vec<RawFootnoteLink>,
    ) -> Self {
        Self {
            file_path,
            namespaces,
            schema_refs,
            role_refs,
            arcrole_refs,
            contexts,
            units,
            facts,
            footnote_links,
        }
    }
}

impl Default for RawInstance {
    fn default() -> Self {
        Self {
            file_path: PathBuf::new(),
            namespaces: HashMap::new(),
            schema_refs: Vec::new(),
            role_refs: Vec::new(),
            arcrole_refs: Vec::new(),
            contexts: Vec::new(),
            units: Vec::new(),
            facts: Vec::new(),
            footnote_links: Vec::new(),
        }
    }
}

/// The parser for XBRL instance documents.
pub struct InstanceParser<R> {
    /// Path of the currently parsed instance file, used for error reporting.
    path: PathBuf,
    /// The XML reader for the instance document.
    reader: Reader<R>,
}

impl<R: BufRead> InstanceParser<R> {
    /// Creates a new `InstanceParser` with the given reader and file path.
    pub fn new(reader: R, path: PathBuf) -> Self {
        let mut reader = Reader::from_reader(reader);
        reader.config_mut().trim_text_start = true;
        reader.config_mut().trim_text_end = true;

        Self { path, reader }
    }

    /// Parses an XBRL instance document from the reader. Path is used for error
    /// reporting.
    pub fn parse_instance(&mut self) -> Result<RawInstance, XbrlError> {
        let mut instance = RawInstance::default();
        instance.file_path = self.path.clone();
        let mut has_instance_root = false;
        let mut buf = Vec::new();

        loop {
            match self.reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref event)) | Ok(Event::Empty(ref event)) => {
                    let event_name = event.name();
                    let local_name = event_name.local_name();
                    let attributes = event.attributes();

                    match local_name.as_ref() {
                        b"xbrl" => {
                            has_instance_root = true;
                            self.parse_instance_root(&mut instance, attributes)?;
                        }
                        b"schemaRef" => self.parse_schema_ref(&mut instance, attributes)?,
                        b"roleRef" => self.parse_role_ref(&mut instance, attributes)?,
                        b"arcroleRef" => self.parse_arcrole_ref(&mut instance, attributes)?,
                        b"context" => self.parse_context(&mut instance, attributes)?,
                        b"unit" => self.parse_unit(&mut instance, attributes)?,
                        b"footnoteLink" => self.parse_footnote_link(&mut instance, attributes)?,
                        _ => {
                            self.parse_fact(&mut instance, event)?;
                        }
                    }
                }
                Ok(Event::End(_)) => {}
                Ok(Event::Text(_)) => {}
                Ok(Event::Eof) => break,
                Err(err) => {
                    return Err(XbrlError::XmlParse {
                        position: self.reader.buffer_position(),
                        element: Some(format!("schema {}", self.path.display())),
                        source: err,
                    });
                }
                _ => {}
            }
        }

        if !has_instance_root {
            return Err(XbrlError::InvalidInstanceDocument {
                path: self.path.to_path_buf(),
                reason: "missing <xbrli:xbrl> root element".to_string(),
            });
        }

        Ok(instance)
    }

    /// Parses the root <xbrli:xbrl> element to extract namespace declarations.
    fn parse_instance_root(
        &mut self,
        instance: &mut RawInstance,
        attributes: Attributes,
    ) -> Result<(), XbrlError> {
        for attribute in attributes {
            let attribute = attribute.map_err(|err| XbrlError::XmlParse {
                position: self.reader.buffer_position(),
                element: Some("xbrl".to_string()),
                source: err.into(),
            })?;
            let key = attribute.key;

            if let Some(prefix) = key.prefix()
                && prefix.as_ref() == b"xmlns"
            {
                let local = key.local_name();
                let namespace_prefix = str::from_utf8(local.as_ref())?;
                let uri = attribute.decode_and_unescape_value(self.reader.decoder())?;
                instance
                    .namespaces
                    .insert(namespace_prefix.to_string(), uri.into_owned());
            }
        }

        Ok(())
    }

    /// Parse the `link:schemaRef` element to extract the schema reference.
    fn parse_schema_ref(
        &mut self,
        instance: &mut RawInstance,
        attributes: Attributes,
    ) -> Result<(), XbrlError> {
        for attribute in attributes {
            let attribute = attribute.map_err(|err| XbrlError::XmlParse {
                position: self.reader.buffer_position(),
                element: Some("schemaRef".to_string()),
                source: err.into(),
            })?;

            if attribute.key.local_name().as_ref() == b"href" {
                let value = attribute.decode_and_unescape_value(self.reader.decoder())?;
                instance.schema_refs.push(SchemaRef {
                    href: value.into_owned(),
                });
                return Ok(());
            }
        }

        Err(XbrlError::InvalidInstanceDocument {
            path: self.path.clone(),
            reason: "missing xlink:href in link:schemaRef".to_string(),
        })
    }

    /// Parse the `link:roleRef` element to extract the role reference.
    fn parse_role_ref(
        &mut self,
        instance: &mut RawInstance,
        attributes: Attributes,
    ) -> Result<(), XbrlError> {
        let mut role_uri = None;
        let mut href = None;

        for attribute in attributes {
            let attribute = attribute.map_err(|err| XbrlError::XmlParse {
                position: self.reader.buffer_position(),
                element: Some("roleRef".to_string()),
                source: err.into(),
            })?;
            let local_name = attribute.key.local_name();
            let value = attribute.decode_and_unescape_value(self.reader.decoder())?;

            match local_name.as_ref() {
                b"roleURI" => role_uri = Some(value.into_owned()),
                b"href" => href = Some(value.into_owned()),
                _ => {}
            }
        }

        instance.role_refs.push(RoleRef {
            role_uri: role_uri.ok_or_else(|| XbrlError::InvalidInstanceDocument {
                path: self.path.clone(),
                reason: "missing roleURI in link:roleRef".to_string(),
            })?,
            href: href.ok_or_else(|| XbrlError::InvalidInstanceDocument {
                path: self.path.clone(),
                reason: "missing xlink:href in link:roleRef".to_string(),
            })?,
        });

        Ok(())
    }

    /// Parse the `link:arcroleRef` element to extract the arcrole reference.
    fn parse_arcrole_ref(
        &mut self,
        instance: &mut RawInstance,
        attributes: Attributes,
    ) -> Result<(), XbrlError> {
        let mut arcrole_uri = None;
        let mut href = None;

        for attribute in attributes {
            let attribute = attribute.map_err(|err| XbrlError::XmlParse {
                position: self.reader.buffer_position(),
                element: Some("arcroleRef".to_string()),
                source: err.into(),
            })?;
            let local_name = attribute.key.local_name();
            let value = attribute.decode_and_unescape_value(self.reader.decoder())?;

            match local_name.as_ref() {
                b"arcroleURI" => arcrole_uri = Some(value.into_owned()),
                b"href" => href = Some(value.into_owned()),
                _ => {}
            }
        }

        instance.arcrole_refs.push(ArcroleRef {
            arcrole_uri: arcrole_uri.ok_or_else(|| XbrlError::InvalidInstanceDocument {
                path: self.path.clone(),
                reason: "missing arcroleURI in link:arcroleRef".to_string(),
            })?,
            href: href.ok_or_else(|| XbrlError::InvalidInstanceDocument {
                path: self.path.clone(),
                reason: "missing xlink:href in link:arcroleRef".to_string(),
            })?,
        });

        Ok(())
    }

    /// Parse the `xbrli:context` element to extract the context definition.
    fn parse_context(
        &mut self,
        instance: &mut RawInstance,
        attributes: Attributes,
    ) -> Result<(), XbrlError> {
        let mut id = None;

        for attribute in attributes {
            let attribute = attribute.map_err(|err| XbrlError::XmlParse {
                position: self.reader.buffer_position(),
                element: Some("context".to_string()),
                source: err.into(),
            })?;

            if attribute.key.local_name().as_ref() == b"id" {
                let value = attribute.decode_and_unescape_value(self.reader.decoder())?;
                id = Some(value.into_owned());
            }
        }

        let id = id.ok_or_else(|| XbrlError::InvalidInstanceDocument {
            path: self.path.clone(),
            reason: "missing id in xbrli:context".to_string(),
        })?;

        let mut entity = None;
        let mut period = None;
        let mut dimensions = Vec::new();
        let mut buf = Vec::new();

        loop {
            match self.reader.read_event_into(&mut buf)? {
                Event::Start(ref event) => match event.local_name().as_ref() {
                    b"entity" => {
                        entity = Some(self.parse_entity()?);
                    }
                    b"period" => {
                        period = Some(self.parse_period()?);
                    }
                    b"scenario" => {
                        self.parse_dimensional_container(&mut dimensions)?;
                    }
                    _ => {}
                },
                Event::End(ref event) if event.local_name().as_ref() == b"context" => break,
                Event::Eof => break,
                _ => {}
            }
            buf.clear();
        }

        instance.contexts.push(RawContext {
            id,
            entity: entity.ok_or_else(|| XbrlError::InvalidInstanceDocument {
                path: self.path.clone(),
                reason: "missing entity in xbrli:context".to_string(),
            })?,
            period: period.ok_or_else(|| XbrlError::InvalidInstanceDocument {
                path: self.path.clone(),
                reason: "missing period in xbrli:context".to_string(),
            })?,
            dimensions,
        });

        Ok(())
    }

    /// Parse the `xbrli:entity` element to extract the entity identifier and
    /// scheme.
    fn parse_entity(&mut self) -> Result<RawEntity, XbrlError> {
        let mut identifier = None;
        let mut scheme = None;
        let mut buf = Vec::new();

        loop {
            match self.reader.read_event_into(&mut buf)? {
                Event::Start(ref event) => {
                    match event.local_name().as_ref() {
                        b"identifier" => {
                            for attribute in event.attributes() {
                                let attribute = attribute.map_err(|err| XbrlError::XmlParse {
                                    position: self.reader.buffer_position(),
                                    element: Some("identifier".to_string()),
                                    source: err.into(),
                                })?;

                                if attribute.key.local_name().as_ref() == b"scheme" {
                                    let value = attribute
                                        .decode_and_unescape_value(self.reader.decoder())?;
                                    scheme = Some(value.into_owned());
                                }
                            }
                        }
                        b"segment" => {
                            // TODO: dimensions can appear in entity/segment
                        }
                        _ => {}
                    }
                }
                Event::Text(ref text) => {
                    if identifier.is_none() {
                        let value = text.xml_content().map_err(quick_xml::Error::from)?;
                        identifier = Some(value.into_owned());
                    }
                }
                Event::End(ref event) if event.local_name().as_ref() == b"entity" => break,
                Event::Eof => break,
                _ => {}
            }
            buf.clear();
        }

        Ok(RawEntity {
            identifier: identifier.ok_or_else(|| XbrlError::InvalidInstanceDocument {
                path: self.path.clone(),
                reason: "missing identifier in xbrli:entity".to_string(),
            })?,
            scheme: scheme.ok_or_else(|| XbrlError::InvalidInstanceDocument {
                path: self.path.clone(),
                reason: "missing scheme in xbrli:identifier".to_string(),
            })?,
        })
    }

    /// Parse the `xbrli:period` element to extract the period definition.
    fn parse_period(&mut self) -> Result<RawPeriod, XbrlError> {
        let mut instant = None;
        let mut start_date = None;
        let mut end_date = None;
        let mut is_forever = false;
        let mut current_tag: Option<String> = None;
        let mut buf = Vec::new();

        loop {
            match self.reader.read_event_into(&mut buf)? {
                Event::Start(ref event) | Event::Empty(ref event) => {
                    match event.local_name().as_ref() {
                        b"instant" => current_tag = Some("instant".to_string()),
                        b"startDate" => current_tag = Some("startDate".to_string()),
                        b"endDate" => current_tag = Some("endDate".to_string()),
                        b"forever" => is_forever = true,
                        _ => {}
                    }
                }
                Event::Text(ref text) => {
                    let value = text
                        .xml_content()
                        .map_err(quick_xml::Error::from)?
                        .into_owned();
                    match current_tag.as_deref() {
                        Some("instant") => instant = Some(value),
                        Some("startDate") => start_date = Some(value),
                        Some("endDate") => end_date = Some(value),
                        _ => {}
                    }
                }
                Event::End(ref event) => match event.local_name().as_ref() {
                    b"period" => break,
                    _ => current_tag = None,
                },
                Event::Eof => break,
                _ => {}
            }
            buf.clear();
        }

        if is_forever {
            Ok(RawPeriod::Forever)
        } else if let Some(instant) = instant {
            Ok(RawPeriod::Instant(instant))
        } else if let (Some(start_date), Some(end_date)) = (start_date, end_date) {
            Ok(RawPeriod::Duration {
                start_date,
                end_date,
            })
        } else {
            Err(XbrlError::InvalidInstanceDocument {
                path: self.path.clone(),
                reason: "invalid period in xbrli:context".to_string(),
            })
        }
    }

    /// Parse the dimensions defined in a `scenario` or `segment` element.
    fn parse_dimensional_container(
        &mut self,
        dimensions: &mut Vec<RawDimension>,
    ) -> Result<(), XbrlError> {
        let mut buf = Vec::new();

        loop {
            match self.reader.read_event_into(&mut buf)? {
                Event::Start(ref event) | Event::Empty(ref event) => {
                    if event.local_name().as_ref() == b"explicitMember" {
                        let mut dimension = None;

                        for attribute in event.attributes() {
                            let attribute = attribute.map_err(|err| XbrlError::XmlParse {
                                position: self.reader.buffer_position(),
                                element: Some("explicitMember".to_string()),
                                source: err.into(),
                            })?;

                            if attribute.key.local_name().as_ref() == b"dimension" {
                                let value =
                                    attribute.decode_and_unescape_value(self.reader.decoder())?;
                                dimension = Some(value.into_owned());
                            }
                        }

                        if let Some(dimension) = dimension {
                            let mut member_buf = Vec::new();

                            if let Event::Text(ref text) =
                                self.reader.read_event_into(&mut member_buf)?
                            {
                                let member = text
                                    .xml_content()
                                    .map_err(quick_xml::Error::from)?
                                    .into_owned();
                                dimensions.push(RawDimension { dimension, member });
                            }
                        }
                    }
                }
                Event::End(ref event)
                    if matches!(event.local_name().as_ref(), b"scenario" | b"segment") =>
                {
                    break;
                }
                Event::Eof => break,
                _ => {}
            }
            buf.clear();
        }

        Ok(())
    }

    /// Parse the `xbrli:unit` element to extract the unit definition, including
    /// measures and divide units.
    fn parse_unit(
        &mut self,
        instance: &mut RawInstance,
        attributes: Attributes,
    ) -> Result<(), XbrlError> {
        let mut id = None;

        for attribute in attributes {
            let attribute = attribute.map_err(|err| XbrlError::XmlParse {
                position: self.reader.buffer_position(),
                element: Some("unit".to_string()),
                source: err.into(),
            })?;

            if attribute.key.local_name().as_ref() == b"id" {
                let value = attribute.decode_and_unescape_value(self.reader.decoder())?;
                id = Some(value.into_owned());
            }
        }

        let id = id.ok_or_else(|| XbrlError::InvalidInstanceDocument {
            path: self.path.clone(),
            reason: "missing id in xbrli:unit".to_string(),
        })?;

        let mut measures = Vec::new();
        let mut divide = None;
        let mut buf = Vec::new();

        loop {
            match self.reader.read_event_into(&mut buf)? {
                Event::Start(ref event) => match event.local_name().as_ref() {
                    b"measure" => {
                        let mut text_buf = Vec::new();
                        if let Event::Text(ref text) = self.reader.read_event_into(&mut text_buf)? {
                            let value = text.xml_content().map_err(quick_xml::Error::from)?;
                            measures.push(value.into_owned());
                        }
                    }
                    b"divide" => {
                        divide = Some(self.parse_unit_divide()?);
                    }
                    _ => {}
                },
                Event::End(ref event) if event.local_name().as_ref() == b"unit" => break,
                Event::Eof => break,
                _ => {}
            }
            buf.clear();
        }

        instance.units.push(RawUnit {
            id,
            measures,
            divide,
        });

        Ok(())
    }

    /// Parse the `divide` element inside a `unit` to extract the numerator and
    /// denominator measures.
    fn parse_unit_divide(&mut self) -> Result<RawUnitDivide, XbrlError> {
        let mut numerator = Vec::new();
        let mut denominator = Vec::new();
        let mut in_numerator = false;
        let mut in_denominator = false;
        let mut buf = Vec::new();

        loop {
            match self.reader.read_event_into(&mut buf)? {
                Event::Start(ref event) => match event.local_name().as_ref() {
                    b"unitNumerator" => in_numerator = true,
                    b"unitDenominator" => in_denominator = true,
                    b"measure" => {
                        let mut text_buf = Vec::new();
                        if let Event::Text(ref text) = self.reader.read_event_into(&mut text_buf)? {
                            let value = text.xml_content().map_err(quick_xml::Error::from)?;
                            if in_numerator {
                                numerator.push(value.into_owned());
                            } else if in_denominator {
                                denominator.push(value.into_owned());
                            }
                        }
                    }
                    _ => {}
                },
                Event::End(ref event) => match event.local_name().as_ref() {
                    b"unitNumerator" => in_numerator = false,
                    b"unitDenominator" => in_denominator = false,
                    b"divide" => break,
                    _ => {}
                },
                Event::Eof => break,
                _ => {}
            }
            buf.clear();
        }

        Ok(RawUnitDivide {
            numerator,
            denominator,
        })
    }

    /// Parse a fact element.
    fn parse_fact(
        &mut self,
        instance: &mut RawInstance,
        event: &BytesStart,
    ) -> Result<(), XbrlError> {
        let name = std::str::from_utf8(event.name().as_ref())?.to_string();

        let mut context_ref = None;
        let mut unit_ref = None;
        let mut decimals = None;
        let mut precision = None;
        let mut id = None;

        for attribute in event.attributes() {
            let attribute = attribute.map_err(|err| XbrlError::XmlParse {
                position: self.reader.buffer_position(),
                element: Some(name.clone()),
                source: err.into(),
            })?;
            let local_name = attribute.key.local_name();
            let value = attribute.decode_and_unescape_value(self.reader.decoder())?;

            match local_name.as_ref() {
                b"contextRef" => context_ref = Some(value.into_owned()),
                b"unitRef" => unit_ref = Some(value.into_owned()),
                b"decimals" => decimals = Some(value.into_owned()),
                b"precision" => precision = Some(value.into_owned()),
                b"id" => id = Some(value.into_owned()),
                _ => {}
            }
        }

        // If contextRef is missing, it's not a fact element
        let context_ref = match context_ref {
            Some(context_ref) => context_ref,
            None => return Ok(()),
        };

        // Read the text value
        let mut value = String::new();
        let mut buf = Vec::new();

        loop {
            match self.reader.read_event_into(&mut buf)? {
                Event::Text(ref text) => {
                    let decoded = text.xml_content().map_err(quick_xml::Error::from)?;
                    value.push_str(&decoded);
                }
                Event::End(ref end) if end.name().as_ref() == event.name().as_ref() => break,
                Event::Eof => break,
                _ => {}
            }
            buf.clear();
        }

        instance.facts.push(RawFact {
            name,
            value,
            context_ref,
            unit_ref,
            decimals,
            precision,
            id,
        });

        Ok(())
    }

    /// Parse the `link:footnoteLink` element to extract the footnote link.
    fn parse_footnote_link(
        &mut self,
        instance: &mut RawInstance,
        attributes: Attributes,
    ) -> Result<(), XbrlError> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_instance_root() {
        let xml = r#"<xbrli:xbrl xmlns:xbrli="http://www.xbrl.org/2003/instance"
                            xmlns:ifrs="http://xbrl.ifrs.org/taxonomy/2023">
                        </xbrli:xbrl>"#;
        let mut parser = InstanceParser::new(xml.as_bytes(), PathBuf::from("test.xml"));
        let instance = parser.parse_instance().unwrap();

        assert_eq!(instance.namespaces.len(), 2);
        assert_eq!(
            instance.namespaces.get("xbrli").unwrap(),
            "http://www.xbrl.org/2003/instance"
        );
        assert_eq!(
            instance.namespaces.get("ifrs").unwrap(),
            "http://xbrl.ifrs.org/taxonomy/2023"
        );
    }

    #[test]
    fn test_parse_schema_ref() {
        let xml = r#"<xbrli:xbrl xmlns:xbrli="http://www.xbrl.org/2003/instance"
                            xmlns:ifrs="http://xbrl.ifrs.org/taxonomy/2023">
                            <link:schemaRef xlink:href="ifrs.xsd" />
                        </xbrli:xbrl>"#;
        let mut parser = InstanceParser::new(xml.as_bytes(), PathBuf::from("test.xml"));
        let instance = parser.parse_instance().unwrap();

        assert_eq!(instance.schema_refs.len(), 1);
        assert_eq!(instance.schema_refs[0].href, "ifrs.xsd");
    }

    #[test]
    fn test_parse_role_ref() {
        let xml = r#"<xbrli:xbrl xmlns:xbrli="http://www.xbrl.org/2003/instance"
                            xmlns:ifrs="http://xbrl.ifrs.org/taxonomy/2023">
                            <link:roleRef roleURI="http://example.com/role" xlink:href="role.xml" />
                        </xbrli:xbrl>"#;
        let mut parser = InstanceParser::new(xml.as_bytes(), PathBuf::from("test.xml"));
        let instance = parser.parse_instance().unwrap();

        assert_eq!(instance.role_refs.len(), 1);
        assert_eq!(instance.role_refs[0].role_uri, "http://example.com/role");
        assert_eq!(instance.role_refs[0].href, "role.xml");
    }

    #[test]
    fn test_parse_arcrole_ref() {
        let xml = r#"<xbrli:xbrl xmlns:xbrli="http://www.xbrl.org/2003/instance"
                            xmlns:ifrs="http://xbrl.ifrs.org/taxonomy/2023">
                            <link:arcroleRef arcroleURI="http://example.com/arcrole" xlink:href="arcrole.xml" />
                        </xbrli:xbrl>"#;
        let mut parser = InstanceParser::new(xml.as_bytes(), PathBuf::from("test.xml"));
        let instance = parser.parse_instance().unwrap();

        assert_eq!(instance.arcrole_refs.len(), 1);
        assert_eq!(
            instance.arcrole_refs[0].arcrole_uri,
            "http://example.com/arcrole"
        );
        assert_eq!(instance.arcrole_refs[0].href, "arcrole.xml");
    }

    #[test]
    fn test_parse_context() {
        let xml = r#"<xbrli:xbrl xmlns:xbrli="http://www.xbrl.org/2003/instance"
                            xmlns:ifrs="http://xbrl.ifrs.org/taxonomy/2023">
                            <context id="c1">
                                <entity>
                                    <identifier scheme="http://example.com">ABC</identifier>
                                </entity>
                                <period>
                                    <instant>2024-12-31</instant>
                                </period>
                            </context>
                        </xbrli:xbrl>"#;
        let mut parser = InstanceParser::new(xml.as_bytes(), PathBuf::from("test.xml"));
        let instance = parser.parse_instance().unwrap();

        assert_eq!(instance.contexts.len(), 1);
        let context = &instance.contexts[0];
        assert_eq!(context.id, "c1");
        assert_eq!(context.entity.identifier, "ABC");
        assert_eq!(context.entity.scheme, "http://example.com");
        assert_eq!(context.period, RawPeriod::Instant("2024-12-31".to_string()));
    }

    #[test]
    fn test_parse_unit() {
        let xml = r#"<xbrli:xbrl xmlns:xbrli="http://www.xbrl.org/2003/instance"
                            xmlns:ifrs="http://xbrl.ifrs.org/taxonomy/2023">
                            <unit id="u1">
                                <measure>iso4217:EUR</measure>
                            </unit>
                        </xbrli:xbrl>"#;
        let mut parser = InstanceParser::new(xml.as_bytes(), PathBuf::from("test.xml"));
        let instance = parser.parse_instance().unwrap();

        assert_eq!(instance.units.len(), 1);
        let unit = &instance.units[0];
        assert_eq!(unit.id, "u1");
        assert_eq!(unit.measures, vec!["iso4217:EUR".to_string()]);
    }

    #[test]
    fn test_parse_fact() {
        let xml = r#"<xbrli:xbrl xmlns:xbrli="http://www.xbrl.org/2003/instance"
                            xmlns:ifrs="http://xbrl.ifrs.org/taxonomy/2023">
                            <ifrs:Revenue contextRef="c1" unitRef="u1" decimals="-3">
                                1200000
                            </ifrs:Revenue>
                        </xbrli:xbrl>"#;
        let mut parser = InstanceParser::new(xml.as_bytes(), PathBuf::from("test.xml"));
        let instance = parser.parse_instance().unwrap();

        assert_eq!(instance.facts.len(), 1);
        let fact = &instance.facts[0];
        assert_eq!(fact.name, "ifrs:Revenue");
        assert_eq!(fact.value, "1200000");
        assert_eq!(fact.context_ref, "c1");
        assert_eq!(fact.unit_ref.as_deref(), Some("u1"));
        assert_eq!(fact.decimals.as_deref(), Some("-3"));
    }

    #[test]
    fn test_parse_footnote_link() {
        let xml = r#"<xbrli:xbrl xmlns:xbrli="http://www.xbrl.org/2003/instance"
                            xmlns:ifrs="http://xbrl.ifrs.org/taxonomy/2023">
                            <link:footnoteLink role="http://example.com/footnote">
                                <link:loc xlink:label="loc1" xlink:href="\#c1" />
                                <link:footnote xlink:label="fn1" xml:lang="en">
                                    This is a footnote.
                                </link:footnote>
                                <link:footnoteArc xlink:from="loc1" xlink:to="fn1" />
                            </link:footnoteLink>
                        </xbrli:xbrl>"#;
        let mut parser = InstanceParser::new(xml.as_bytes(), PathBuf::from("test.xml"));
        let instance = parser.parse_instance().unwrap();

        assert_eq!(instance.footnote_links.len(), 1);
        let footnote_link = &instance.footnote_links[0];
        assert_eq!(footnote_link.role, "http://example.com/footnote");
        assert_eq!(footnote_link.locators.len(), 1);
        assert_eq!(footnote_link.locators[0].label, "loc1");
        assert_eq!(footnote_link.locators[0].href, "#c1");
        assert_eq!(footnote_link.arcs.len(), 1);
        assert_eq!(footnote_link.arcs[0].from, "loc1");
        assert_eq!(footnote_link.arcs[0].to, "fn1");
        assert_eq!(footnote_link.footnotes.len(), 1);
        assert_eq!(footnote_link.footnotes[0].label, "fn1");
        assert_eq!(footnote_link.footnotes[0].lang.as_deref(), Some("en"));
        assert_eq!(footnote_link.footnotes[0].text, "This is a footnote.");
    }

    #[test]
    fn test_parse_instance() {
        let xml = r#"<xbrli:xbrl xmlns:xbrli="http://www.xbrl.org/2003/instance"
                            xmlns:ifrs="http://xbrl.ifrs.org/taxonomy/2023">
                            <link:schemaRef xlink:href="ifrs.xsd" />
                            <context id="c1">
                                <entity>
                                    <identifier scheme="http://example.com">ABC</identifier>
                                </entity>
                                <period>
                                    <instant>2024-12-31</instant>
                                </period>
                            </context>
                            <unit id="u1">
                                <measure>iso4217:EUR</measure>
                            </unit>
                            <ifrs:Revenue contextRef="c1" unitRef="u1" decimals="-3">
                                1200000
                            </ifrs:Revenue>
                        </xbrli:xbrl>"#;
        let mut parser = InstanceParser::new(xml.as_bytes(), PathBuf::from("test.xml"));
        let instance = parser.parse_instance().unwrap();

        assert_eq!(instance.contexts.len(), 1);
        assert_eq!(instance.units.len(), 1);
        assert_eq!(instance.facts.len(), 1);

        assert_eq!(
            instance,
            RawInstance {
                file_path: PathBuf::from("test.xml"),
                namespaces: {
                    let mut namespaces = HashMap::new();
                    namespaces.insert(
                        "xbrli".to_string(),
                        "http://www.xbrl.org/2003/instance".to_string(),
                    );
                    namespaces.insert(
                        "ifrs".to_string(),
                        "http://xbrl.ifrs.org/taxonomy/2023".to_string(),
                    );
                    namespaces
                },
                schema_refs: vec![SchemaRef {
                    href: "ifrs.xsd".to_string(),
                }],
                role_refs: vec![],
                arcrole_refs: vec![],
                contexts: vec![RawContext {
                    id: "c1".to_string(),
                    entity: RawEntity {
                        identifier: "ABC".to_string(),
                        scheme: "http://example.com".to_string(),
                    },
                    period: RawPeriod::Instant("2024-12-31".to_string()),
                    dimensions: vec![],
                }],
                units: vec![RawUnit {
                    id: "u1".to_string(),
                    measures: vec!["iso4217:EUR".to_string()],
                    divide: None,
                }],
                facts: vec![RawFact {
                    name: "ifrs:Revenue".to_string(),
                    value: "1200000".to_string(),
                    context_ref: "c1".to_string(),
                    unit_ref: Some("u1".to_string()),
                    decimals: Some("-3".to_string()),
                    precision: None,
                    id: None,
                }],
                footnote_links: vec![],
            }
        );
    }
}
