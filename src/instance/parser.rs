use crate::XbrlError;
use quick_xml::{
    Reader,
    events::{Event, attributes::Attributes},
};
use std::{collections::HashMap, io::BufRead, path::PathBuf};

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
    pub role_refs: Vec<RoleRef>,
    /// Arcrole references
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
                            self.parse_xbrl_root(&mut instance, attributes)?;
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

    fn parse_xbrl_root(
        &mut self,
        instance: &mut RawInstance,
        attributes: Attributes,
    ) -> Result<(), XbrlError> {
        todo!()
    }

    fn parse_schema_ref(
        &mut self,
        instance: &mut RawInstance,
        attributes: Attributes,
    ) -> Result<(), XbrlError> {
        todo!()
    }

    fn parse_role_ref(
        &mut self,
        instance: &mut RawInstance,
        attributes: Attributes,
    ) -> Result<(), XbrlError> {
        todo!()
    }

    fn parse_arcrole_ref(
        &mut self,
        instance: &mut RawInstance,
        attributes: Attributes,
    ) -> Result<(), XbrlError> {
        todo!()
    }

    fn parse_context(
        &mut self,
        instance: &mut RawInstance,
        attributes: Attributes,
    ) -> Result<(), XbrlError> {
        todo!()
    }

    fn parse_unit(
        &mut self,
        instance: &mut RawInstance,
        attributes: Attributes,
    ) -> Result<(), XbrlError> {
        todo!()
    }

    fn parse_fact(
        &mut self,
        instance: &mut RawInstance,
        event: &quick_xml::events::BytesStart,
    ) -> Result<(), XbrlError> {
        todo!()
    }

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
    }
}
