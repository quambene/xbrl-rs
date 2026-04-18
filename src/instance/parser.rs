use crate::{
    NamespacePrefix, NamespaceUri, QName, XbrlError,
    xml::{self, ArcroleRef, RoleRef, SchemaRef, parse_qname},
};
use quick_xml::{
    Reader,
    events::{BytesStart, Event, attributes::Attributes},
};
use std::{
    collections::HashMap,
    fs::File,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};

/// An `xbrli:context` element as parsed from the instance document.
#[derive(Debug, PartialEq, Eq)]
pub struct RawContext {
    /// Id attribute of the context.
    pub id: String,
    /// Entity definition for the context.
    pub entity: RawEntity,
    /// Period definition for the context.
    pub period: RawPeriod,
    /// Scenario dimensions for the context.
    pub scenario_dimensions: Vec<RawDimension>,
}

/// An `xbrli:entity` element as parsed from the instance document.
#[derive(Debug, PartialEq, Eq)]
pub struct RawEntity {
    /// Identifier for the entity, typically a legal entity identifier (LEI).
    pub identifier: String,
    /// Scheme for the entity identifier, typically a URI that defines the
    /// syntax and semantics of the identifier (e.g.
    /// "http://standards.iso.org/iso/17442" for LEIs).
    pub scheme: String,
    /// Segment dimensions for the entity.
    pub segment_dimensions: Vec<RawDimension>,
}

/// An `xbrli:period` element as parsed from the instance document.
#[derive(Debug, PartialEq, Eq)]
pub enum RawPeriod {
    Instant(String),
    Duration {
        start_date: String,
        end_date: String,
    },
    Forever,
}

/// A dimension defined in a `scenario` or `segment` element.
#[derive(Debug, PartialEq, Eq)]
pub struct RawDimension {
    /// QName of the dimension
    pub dimension: QName,
    /// QName of the member
    pub member: QName,
}

#[derive(Debug, PartialEq, Eq)]
pub struct RawUnit {
    /// Unique ID of the unit as specified in the instance document.
    pub id: String,
    /// For a simple unit, this will be the only measure. For a divide unit,
    /// this is the numerator.
    pub numerator: Vec<QName>,
    /// For a simple unit, this will be empty. For a divide unit, this is the
    /// denominator.
    pub denominator: Vec<QName>,
}

/// A fact in the instance document, which can be either an item or a tuple.
#[derive(Debug, PartialEq, Eq)]
pub enum RawFact {
    Item(RawItemFact),
    Tuple(RawTupleFact),
}

#[derive(Debug, PartialEq, Eq)]
pub struct RawItemFact {
    /// QName of the corresponding concept
    pub name: QName,
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
    /// xsi:nil attribute
    pub is_nil: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub struct RawTupleFact {
    /// QName of the corresponding concept
    pub name: QName,
    /// id attribute
    pub id: Option<String>,
    /// xsi:nil attribute
    pub is_nil: bool,
    /// Child facts (items or nested tuples)
    pub children: Vec<RawFact>,
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

#[derive(Debug, PartialEq, Eq, Default)]
pub struct RawInstance {
    /// Namespace declarations (prefix -> URI)
    pub namespaces: HashMap<NamespacePrefix, NamespaceUri>,
    /// Schema references
    pub schema_refs: Vec<SchemaRef>,
    /// Role references
    ///
    /// Usually defined in the linkbase document, but can also be present in the
    /// instance document.
    pub role_refs: Vec<RoleRef>,
    /// Arcrole references
    ///
    /// Usually defined in the linkbase document, but can also be present in the
    /// instance document.
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
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        namespaces: HashMap<NamespacePrefix, NamespaceUri>,
        schema_refs: Vec<SchemaRef>,
        role_refs: Vec<RoleRef>,
        arcrole_refs: Vec<ArcroleRef>,
        contexts: Vec<RawContext>,
        units: Vec<RawUnit>,
        facts: Vec<RawFact>,
        footnote_links: Vec<RawFootnoteLink>,
    ) -> Self {
        Self {
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

/// The parser for XBRL instance documents.
pub struct InstanceParser<R> {
    /// The XML reader for the instance document.
    reader: Reader<R>,
    /// Path of the currently parsed instance file if available. Used for error
    /// reporting.
    path: Option<PathBuf>,
    /// Flag to indicate if the root element is an XBRL instance element.
    is_xbrl_root: bool,
}

impl InstanceParser<BufReader<File>> {
    /// Creates a new `InstanceParser` from the given file path.
    pub fn from_file(path: &Path) -> Result<Self, XbrlError> {
        let file = File::open(path).map_err(|err| XbrlError::FileOpen {
            path: path.to_path_buf(),
            source: err,
        })?;
        let reader = Reader::from_reader(BufReader::new(file));

        Ok(Self {
            path: Some(path.to_path_buf()),
            reader,
            is_xbrl_root: false,
        })
    }
}

impl<R: BufRead> InstanceParser<R> {
    /// Creates a new `InstanceParser` with the given reader and file path.
    pub fn new(reader: Reader<R>, path: Option<PathBuf>, is_xbrl_root: bool) -> Self {
        Self {
            reader,
            path,
            is_xbrl_root,
        }
    }

    /// Sets whether the parser enforces `<xbrli:xbrl>` as the document root.
    /// When `true`, any element encountered before `<xbrli:xbrl>` is an error.
    /// When `false` (default), non-XBRL wrapper elements are silently skipped.
    pub fn xbrl_root(mut self, is_xbrl_root: bool) -> Self {
        self.is_xbrl_root = is_xbrl_root;
        self
    }

    /// Creates a new `InstanceParser` from the given reader.
    pub fn from_reader(reader: R) -> Self {
        let mut reader = Reader::from_reader(reader);
        reader.config_mut().trim_text_start = true;
        reader.config_mut().trim_text_end = true;
        Self::new(reader, None, false)
    }

    /// Parses an XBRL instance document from the reader. Path is used for error
    /// reporting.
    pub fn parse(&mut self) -> Result<RawInstance, XbrlError> {
        let mut instance = RawInstance::default();
        let mut has_instance_root = false;
        let mut buf = Vec::new();

        loop {
            match self.reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref event)) => {
                    let event_name = event.name();
                    let local_name = event_name.local_name();
                    let attributes = event.attributes();

                    match local_name.as_ref() {
                        b"xbrl" => {
                            has_instance_root = true;
                            self.parse_instance_root(&mut instance, attributes)?;
                        }
                        _ if !has_instance_root => {
                            if self.is_xbrl_root {
                                return Err(XbrlError::InvalidInstanceDocument {
                                    path: self.path.clone(),
                                    reason: "expected <xbrli:xbrl> as root element".to_string(),
                                });
                            }
                        }
                        b"schemaRef" => self.parse_schema_ref(&mut instance, attributes)?,
                        b"roleRef" => self.parse_role_ref(&mut instance, attributes)?,
                        b"arcroleRef" => self.parse_arcrole_ref(&mut instance, attributes)?,
                        b"context" => self.parse_context(&mut instance, attributes)?,
                        b"unit" => self.parse_unit(&mut instance, attributes)?,
                        b"footnoteLink" => self.parse_footnote_link(&mut instance, attributes)?,
                        _ if Self::is_fact_element(local_name.as_ref()) => {
                            self.parse_fact(&mut instance, event)?;
                        }
                        _ => {}
                    }
                }
                Ok(Event::Empty(ref event)) => {
                    let local_name = event.name().local_name();
                    let attributes = event.attributes();

                    match local_name.as_ref() {
                        b"xbrl" => {
                            has_instance_root = true;
                            self.parse_instance_root(&mut instance, attributes)?;
                        }
                        _ if !has_instance_root => {
                            if self.is_xbrl_root {
                                return Err(XbrlError::InvalidInstanceDocument {
                                    path: self.path.clone(),
                                    reason: "expected <xbrli:xbrl> as root element".to_string(),
                                });
                            }
                        }
                        b"schemaRef" => self.parse_schema_ref(&mut instance, attributes)?,
                        b"roleRef" => self.parse_role_ref(&mut instance, attributes)?,
                        b"arcroleRef" => self.parse_arcrole_ref(&mut instance, attributes)?,
                        _ if Self::is_fact_element(local_name.as_ref()) => {
                            let fact = self.parse_empty_fact(event)?;
                            instance.facts.push(fact);
                        }
                        _ => {}
                    }
                }
                Ok(Event::End(ref event)) if event.name().local_name().as_ref() == b"xbrl" => {
                    break;
                }
                Ok(Event::End(_)) => {}
                Ok(Event::Text(_)) => {}
                Ok(Event::Eof) => break,
                Err(err) => {
                    return Err(XbrlError::XmlParse {
                        path: self.path.clone(),
                        position: self.reader.buffer_position(),
                        element: Some("schema".to_string()),
                        source: err,
                    });
                }
                _ => {}
            }
        }

        if !has_instance_root {
            return Err(XbrlError::InvalidInstanceDocument {
                path: self.path.clone(),
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
                path: self.path.clone(),
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
                instance.namespaces.insert(
                    NamespacePrefix::from(namespace_prefix),
                    NamespaceUri::from(uri.into_owned()),
                );
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
                path: self.path.clone(),
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
                path: self.path.clone(),
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
                path: self.path.clone(),
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
    ///
    /// `xbrli:segment` and `xbrli:scenario` elements are parsed as dimensional
    /// containers. `xbrli:segment` is always a child of `xbrli:entity`, while
    /// `xbrli:scenario` is a direct child of `xbrli:context`.
    fn parse_context(
        &mut self,
        instance: &mut RawInstance,
        attributes: Attributes,
    ) -> Result<(), XbrlError> {
        let mut id = None;

        for attribute in attributes {
            let attribute = attribute.map_err(|err| XbrlError::XmlParse {
                path: self.path.clone(),
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
        let mut scenario_dimensions = Vec::new();
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
                        self.parse_dimensional_container(&mut scenario_dimensions)?;
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
            scenario_dimensions,
        });

        Ok(())
    }

    /// Parse the `xbrli:entity` element to extract the entity identifier and
    /// scheme.
    fn parse_entity(&mut self) -> Result<RawEntity, XbrlError> {
        let mut identifier = None;
        let mut scheme = None;
        let mut segment_dimensions = Vec::new();
        let mut buf = Vec::new();

        loop {
            match self.reader.read_event_into(&mut buf)? {
                Event::Start(ref event) => match event.local_name().as_ref() {
                    b"identifier" => {
                        for attribute in event.attributes() {
                            let attribute = attribute.map_err(|err| XbrlError::XmlParse {
                                path: self.path.clone(),
                                position: self.reader.buffer_position(),
                                element: Some("identifier".to_string()),
                                source: err.into(),
                            })?;

                            if attribute.key.local_name().as_ref() == b"scheme" {
                                let value =
                                    attribute.decode_and_unescape_value(self.reader.decoder())?;
                                scheme = Some(value.into_owned());
                            }
                        }
                    }
                    b"segment" => {
                        self.parse_dimensional_container(&mut segment_dimensions)?;
                    }
                    _ => {}
                },
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
            segment_dimensions,
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
                                path: self.path.clone(),
                                position: self.reader.buffer_position(),
                                element: Some("explicitMember".to_string()),
                                source: err.into(),
                            })?;

                            if attribute.key.local_name().as_ref() == b"dimension" {
                                let value =
                                    attribute.decode_and_unescape_value(self.reader.decoder())?;
                                dimension = Some(parse_qname(&value));
                            }
                        }

                        if let Some(dimension) = dimension {
                            let mut member_buf = Vec::new();

                            if let Event::Text(ref text) =
                                self.reader.read_event_into(&mut member_buf)?
                            {
                                let member = text.xml_content().map_err(quick_xml::Error::from)?;
                                let member = parse_qname(member.trim());
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
                path: self.path.clone(),
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
        let mut numerator = Vec::new();
        let mut denominator = Vec::new();
        let mut buf = Vec::new();

        loop {
            match self.reader.read_event_into(&mut buf)? {
                Event::Start(ref event) => match event.local_name().as_ref() {
                    b"measure" => {
                        let mut text_buf = Vec::new();
                        if let Event::Text(ref text) = self.reader.read_event_into(&mut text_buf)? {
                            let value = text.xml_content().map_err(quick_xml::Error::from)?;
                            let qname = xml::parse_qname(&value);
                            numerator.push(qname);
                        }
                    }
                    b"divide" => {
                        self.parse_unit_divide(&mut numerator, &mut denominator)?;
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
            numerator,
            denominator,
        });

        Ok(())
    }

    /// Parse the `divide` element inside a `unit` to extract the numerator and
    /// denominator measures.
    fn parse_unit_divide(
        &mut self,
        numerator: &mut Vec<QName>,
        denominator: &mut Vec<QName>,
    ) -> Result<(), XbrlError> {
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
                            let qname = xml::parse_qname(&value);
                            if in_numerator {
                                numerator.push(qname);
                            } else if in_denominator {
                                denominator.push(qname);
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

        Ok(())
    }

    /// Check if a local element name represents a fact (as opposed to a
    /// structural XBRL element like context, unit, schemaRef, etc.).
    fn is_fact_element(local_name: &[u8]) -> bool {
        !matches!(
            local_name,
            b"xbrl"
                | b"context"
                | b"unit"
                | b"schemaRef"
                | b"roleRef"
                | b"arcroleRef"
                | b"identifier"
                | b"entity"
                | b"period"
                | b"instant"
                | b"startDate"
                | b"endDate"
                | b"scenario"
                | b"segment"
                | b"explicitMember"
                | b"measure"
                | b"footnoteLink"
                | b"footnote"
                | b"footnoteArc"
                | b"loc"
                | b"forever"
                | b"unitNumerator"
                | b"unitDenominator"
                | b"divide"
        )
    }

    /// Parse a fact element (item or tuple).
    ///
    /// If `contextRef` is present the element is an item fact; otherwise it is
    /// a tuple fact whose children are parsed recursively.
    fn parse_fact(
        &mut self,
        instance: &mut RawInstance,
        event: &BytesStart,
    ) -> Result<(), XbrlError> {
        let fact = self.parse_fact_recursive(event)?;

        if let Some(fact) = fact {
            instance.facts.push(fact);
        }

        Ok(())
    }

    /// Recursively parse a single fact element, returning `None` for
    /// self-closing elements without `contextRef` that have no children
    /// (empty tuples are still returned).
    fn parse_fact_recursive(&mut self, event: &BytesStart) -> Result<Option<RawFact>, XbrlError> {
        let name = parse_qname(std::str::from_utf8(event.name().as_ref())?);

        let mut context_ref = None;
        let mut unit_ref = None;
        let mut decimals = None;
        let mut precision = None;
        let mut id = None;
        let mut is_nil = false;

        for attribute in event.attributes() {
            let attribute = attribute.map_err(|err| XbrlError::XmlParse {
                path: self.path.clone(),
                position: self.reader.buffer_position(),
                element: Some(name.to_string()),
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
                b"nil" => is_nil = value.as_ref() == "true",
                _ => {}
            }
        }

        if let Some(context_ref) = context_ref {
            let mut value = String::new();
            let mut buf = Vec::new();

            // Item fact: read text value until closing tag
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

            // Remove newline characters and trim whitespace from the fact
            // value.
            let value = value.trim().to_owned();

            Ok(Some(RawFact::Item(RawItemFact {
                name,
                value,
                context_ref,
                unit_ref,
                decimals,
                precision,
                id,
                is_nil,
            })))
        } else {
            let mut children = Vec::new();
            let mut buf = Vec::new();

            // Tuple fact: recursively parse child facts until closing tag
            loop {
                match self.reader.read_event_into(&mut buf)? {
                    Event::Start(ref child_event) => {
                        if Self::is_fact_element(child_event.name().local_name().as_ref())
                            && let Some(child) = self.parse_fact_recursive(child_event)?
                        {
                            children.push(child);
                        }
                    }
                    Event::Empty(ref child_event) => {
                        if Self::is_fact_element(child_event.name().local_name().as_ref()) {
                            children.push(self.parse_empty_fact(child_event)?);
                        }
                    }
                    Event::End(ref end) if end.name().as_ref() == event.name().as_ref() => break,
                    Event::Eof => break,
                    _ => {}
                }
                buf.clear();
            }

            Ok(Some(RawFact::Tuple(RawTupleFact {
                name,
                id,
                is_nil,
                children,
            })))
        }
    }

    /// Parse a self-closing (empty) fact element.
    fn parse_empty_fact(&mut self, event: &BytesStart) -> Result<RawFact, XbrlError> {
        let name = parse_qname(std::str::from_utf8(event.name().as_ref())?);

        let mut context_ref = None;
        let mut unit_ref = None;
        let mut decimals = None;
        let mut precision = None;
        let mut id = None;
        let mut is_nil = false;

        for attribute in event.attributes() {
            let attribute = attribute.map_err(|err| XbrlError::XmlParse {
                path: self.path.clone(),
                position: self.reader.buffer_position(),
                element: Some(name.to_string()),
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
                b"nil" => is_nil = value.as_ref() == "true",
                _ => {}
            }
        }

        if let Some(context_ref) = context_ref {
            Ok(RawFact::Item(RawItemFact {
                name,
                value: String::new(),
                context_ref,
                unit_ref,
                decimals,
                precision,
                id,
                is_nil,
            }))
        } else {
            Ok(RawFact::Tuple(RawTupleFact {
                name,
                id,
                is_nil,
                children: Vec::new(),
            }))
        }
    }

    /// Parse the `link:footnoteLink` element to extract the footnote link.
    fn parse_footnote_link(
        &mut self,
        instance: &mut RawInstance,
        attributes: Attributes,
    ) -> Result<(), XbrlError> {
        let mut role = String::new();

        for attribute in attributes {
            let attribute = attribute.map_err(|err| XbrlError::XmlParse {
                path: self.path.clone(),
                position: self.reader.buffer_position(),
                element: Some("footnoteLink".to_string()),
                source: err.into(),
            })?;

            if attribute.key.local_name().as_ref() == b"role" {
                let value = attribute.decode_and_unescape_value(self.reader.decoder())?;
                role = value.into_owned();
            }
        }

        let mut locators = Vec::new();
        let mut arcs = Vec::new();
        let mut footnotes = Vec::new();
        let mut buf = Vec::new();

        loop {
            match self.reader.read_event_into(&mut buf)? {
                Event::Start(ref event) | Event::Empty(ref event) => {
                    match event.local_name().as_ref() {
                        b"loc" => {
                            let mut label = None;
                            let mut href = None;

                            for attribute in event.attributes() {
                                let attribute = attribute.map_err(|err| XbrlError::XmlParse {
                                    path: self.path.clone(),
                                    position: self.reader.buffer_position(),
                                    element: Some("loc".to_string()),
                                    source: err.into(),
                                })?;
                                let local_name = attribute.key.local_name();
                                let value =
                                    attribute.decode_and_unescape_value(self.reader.decoder())?;

                                match local_name.as_ref() {
                                    b"label" => label = Some(value.into_owned()),
                                    b"href" => href = Some(value.into_owned()),
                                    _ => {}
                                }
                            }

                            if let (Some(label), Some(href)) = (label, href) {
                                locators.push(Locator { label, href });
                            }
                        }
                        b"footnoteArc" => {
                            let mut from = None;
                            let mut to = None;

                            for attribute in event.attributes() {
                                let attribute = attribute.map_err(|err| XbrlError::XmlParse {
                                    path: self.path.clone(),
                                    position: self.reader.buffer_position(),
                                    element: Some("footnoteArc".to_string()),
                                    source: err.into(),
                                })?;
                                let local_name = attribute.key.local_name();
                                let value =
                                    attribute.decode_and_unescape_value(self.reader.decoder())?;

                                match local_name.as_ref() {
                                    b"from" => from = Some(value.into_owned()),
                                    b"to" => to = Some(value.into_owned()),
                                    _ => {}
                                }
                            }

                            if let (Some(from), Some(to)) = (from, to) {
                                arcs.push(FootnoteArc { from, to });
                            }
                        }
                        b"footnote" => {
                            let mut label = None;
                            let mut lang = None;

                            for attribute in event.attributes() {
                                let attribute = attribute.map_err(|err| XbrlError::XmlParse {
                                    path: self.path.clone(),
                                    position: self.reader.buffer_position(),
                                    element: Some("footnote".to_string()),
                                    source: err.into(),
                                })?;
                                let local_name = attribute.key.local_name();
                                let value =
                                    attribute.decode_and_unescape_value(self.reader.decoder())?;

                                match local_name.as_ref() {
                                    b"label" => label = Some(value.into_owned()),
                                    b"lang" => lang = Some(value.into_owned()),
                                    _ => {}
                                }
                            }

                            // Read footnote text content
                            let mut text = String::new();
                            let mut text_buf = Vec::new();
                            loop {
                                match self.reader.read_event_into(&mut text_buf)? {
                                    Event::Text(ref t) => {
                                        let decoded =
                                            t.xml_content().map_err(quick_xml::Error::from)?;
                                        text.push_str(&decoded);
                                    }
                                    Event::End(ref e) if e.local_name().as_ref() == b"footnote" => {
                                        break;
                                    }
                                    Event::Eof => break,
                                    _ => {}
                                }
                                text_buf.clear();
                            }

                            if let Some(label) = label {
                                footnotes.push(FootnoteResource {
                                    label,
                                    lang,
                                    text: text.trim().to_string(),
                                });
                            }
                        }
                        _ => {}
                    }
                }
                Event::End(ref event) if event.local_name().as_ref() == b"footnoteLink" => {
                    break;
                }
                Event::Eof => break,
                _ => {}
            }
            buf.clear();
        }

        instance.footnote_links.push(RawFootnoteLink {
            role,
            locators,
            arcs,
            footnotes,
        });

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_matches::assert_matches;
    use std::str::FromStr;

    #[test]
    fn test_parse_non_instance_root() {
        let xml = r#"<root>
                                <xbrli:xbrl xmlns:xbrli="http://www.xbrl.org/2003/instance"
                                    xmlns:ifrs="http://xbrl.ifrs.org/taxonomy/2023">
                                </xbrli:xbrl>
                            </root>"#;
        let mut parser = InstanceParser::from_reader(xml.as_bytes());
        let instance = parser.parse().unwrap();

        assert_eq!(instance.namespaces.len(), 2);
        assert_eq!(
            instance
                .namespaces
                .get(&NamespacePrefix::from("xbrli"))
                .unwrap(),
            &NamespaceUri::from("http://www.xbrl.org/2003/instance")
        );
        assert_eq!(
            instance
                .namespaces
                .get(&NamespacePrefix::from("ifrs"))
                .unwrap(),
            &NamespaceUri::from("http://xbrl.ifrs.org/taxonomy/2023")
        );
    }

    #[test]
    fn test_parse_non_instance_root_strict() {
        let xml = r#"<root>
                                <xbrli:xbrl xmlns:xbrli="http://www.xbrl.org/2003/instance"
                                    xmlns:ifrs="http://xbrl.ifrs.org/taxonomy/2023">
                                </xbrli:xbrl>
                            </root>"#;
        let mut parser = InstanceParser::from_reader(xml.as_bytes()).xbrl_root(true);
        let res = parser.parse();

        assert_matches!(res, Err(XbrlError::InvalidInstanceDocument { reason, .. }) if reason == "expected <xbrli:xbrl> as root element");
    }

    #[test]
    fn test_parse_instance_root() {
        let xml = r#"<xbrli:xbrl xmlns:xbrli="http://www.xbrl.org/2003/instance"
                            xmlns:ifrs="http://xbrl.ifrs.org/taxonomy/2023">
                        </xbrli:xbrl>"#;
        let mut parser = InstanceParser::from_reader(xml.as_bytes());
        let instance = parser.parse().unwrap();

        assert_eq!(instance.namespaces.len(), 2);
        assert_eq!(
            instance
                .namespaces
                .get(&NamespacePrefix::from("xbrli"))
                .unwrap(),
            &NamespaceUri::from("http://www.xbrl.org/2003/instance")
        );
        assert_eq!(
            instance
                .namespaces
                .get(&NamespacePrefix::from("ifrs"))
                .unwrap(),
            &NamespaceUri::from("http://xbrl.ifrs.org/taxonomy/2023")
        );
    }

    #[test]
    fn test_parse_schema_ref() {
        let xml = r#"<xbrli:xbrl xmlns:xbrli="http://www.xbrl.org/2003/instance"
                            xmlns:ifrs="http://xbrl.ifrs.org/taxonomy/2023">
                            <link:schemaRef xlink:href="ifrs.xsd" />
                        </xbrli:xbrl>"#;
        let mut parser = InstanceParser::from_reader(xml.as_bytes());
        let instance = parser.parse().unwrap();

        assert_eq!(instance.schema_refs.len(), 1);
        assert_eq!(instance.schema_refs[0].href, "ifrs.xsd");
    }

    #[test]
    fn test_parse_role_ref() {
        let xml = r#"<xbrli:xbrl xmlns:xbrli="http://www.xbrl.org/2003/instance"
                            xmlns:ifrs="http://xbrl.ifrs.org/taxonomy/2023">
                            <link:roleRef roleURI="http://example.com/role" xlink:href="role.xml" />
                        </xbrli:xbrl>"#;
        let mut parser = InstanceParser::from_reader(xml.as_bytes());
        let instance = parser.parse().unwrap();

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
        let mut parser = InstanceParser::from_reader(xml.as_bytes());
        let instance = parser.parse().unwrap();

        assert_eq!(instance.arcrole_refs.len(), 1);
        assert_eq!(
            instance.arcrole_refs[0].arcrole_uri,
            "http://example.com/arcrole"
        );
        assert_eq!(instance.arcrole_refs[0].href, "arcrole.xml");
    }

    #[test]
    fn test_parse_context() {
        let xml = r#"<xbrli:xbrl
                                xmlns:xbrli="http://www.xbrl.org/2003/instance"
                                xmlns:xbrldi="http://xbrl.org/2006/xbrldi"
                                xmlns:ifrs="http://xbrl.ifrs.org/taxonomy/2023">
                                <context id="c1">
                                    <entity>
                                        <identifier scheme="http://example.com">ABC</identifier>
                                        <segment>
                                            <xbrldi:explicitMember dimension="ifrs:OperatingSegmentsAxis">
                                                ifrs:EuropeSegmentMember
                                            </xbrldi:explicitMember>
                                        </segment>
                                    </entity>
                                    <period>
                                        <instant>2024-12-31</instant>
                                    </period>
                                    <scenario>
                                        <xbrldi:explicitMember dimension="ifrs:ProductsAndServicesAxis">
                                            ifrs:SoftwareMember
                                        </xbrldi:explicitMember>
                                    </scenario>
                                </context>
                            </xbrli:xbrl>"#;
        let mut parser = InstanceParser::from_reader(xml.as_bytes());
        let instance = parser.parse().unwrap();

        assert_eq!(instance.contexts.len(), 1);
        let context = &instance.contexts[0];
        assert_eq!(
            context,
            &RawContext {
                id: "c1".to_string(),
                entity: RawEntity {
                    identifier: "ABC".to_string(),
                    scheme: "http://example.com".to_string(),
                    segment_dimensions: vec![RawDimension {
                        dimension: QName::from_str("ifrs:OperatingSegmentsAxis").unwrap(),
                        member: QName::from_str("ifrs:EuropeSegmentMember").unwrap(),
                    }],
                },
                period: RawPeriod::Instant("2024-12-31".to_string()),
                scenario_dimensions: vec![RawDimension {
                    dimension: QName::from_str("ifrs:ProductsAndServicesAxis").unwrap(),
                    member: QName::from_str("ifrs:SoftwareMember").unwrap(),
                }],
            }
        );
    }

    #[test]
    fn test_parse_unit() {
        let xml = r#"<xbrli:xbrl xmlns:xbrli="http://www.xbrl.org/2003/instance"
                            xmlns:ifrs="http://xbrl.ifrs.org/taxonomy/2023">
                            <unit id="u1">
                                <measure>iso4217:EUR</measure>
                            </unit>
                        </xbrli:xbrl>"#;
        let mut parser = InstanceParser::from_reader(xml.as_bytes());
        let instance = parser.parse().unwrap();

        assert_eq!(instance.units.len(), 1);
        let unit = &instance.units[0];
        assert_eq!(
            unit,
            &RawUnit {
                id: "u1".to_string(),
                numerator: vec![QName::from_str("iso4217:EUR").unwrap()],
                denominator: vec![],
            }
        );
    }

    #[test]
    fn test_parse_unit_divide() {
        let xml = r#"<xbrli:xbrl xmlns:xbrli="http://www.xbrl.org/2003/instance"
                                xmlns:ifrs="http://xbrl.ifrs.org/taxonomy/2023">
                                <xbrli:unit id="USD_per_share">
                                    <xbrli:divide>
                                        <xbrli:unitNumerator>
                                            <xbrli:measure>iso4217:USD</xbrli:measure>
                                        </xbrli:unitNumerator>
                                        <xbrli:unitDenominator>
                                            <xbrli:measure>xbrli:shares</xbrli:measure>
                                        </xbrli:unitDenominator>
                                    </xbrli:divide>
                                </xbrli:unit>
                            </xbrli:xbrl>"#;
        let mut parser = InstanceParser::from_reader(xml.as_bytes());
        let instance = parser.parse().unwrap();

        assert_eq!(instance.units.len(), 1);
        let unit = &instance.units[0];
        assert_eq!(
            unit,
            &RawUnit {
                id: "USD_per_share".to_string(),
                numerator: vec![QName::from_str("iso4217:USD").unwrap()],
                denominator: vec![QName::from_str("xbrli:shares").unwrap()],
            }
        );
    }

    #[test]
    fn test_parse_item_fact() {
        let xml = r#"<xbrli:xbrl xmlns:xbrli="http://www.xbrl.org/2003/instance"
                            xmlns:ifrs="http://xbrl.ifrs.org/taxonomy/2023">
                            <ifrs:Revenue contextRef="c1" unitRef="u1" decimals="-3">
                                1200000
                            </ifrs:Revenue>
                        </xbrli:xbrl>"#;
        let mut parser = InstanceParser::from_reader(xml.as_bytes());
        let instance = parser.parse().unwrap();

        assert_eq!(instance.facts.len(), 1);
        let fact = &instance.facts[0];
        assert_matches!(fact, RawFact::Item(fact) => {
            assert_eq!(fact.name.to_string(), "ifrs:Revenue");
            assert_eq!(fact.value, "1200000");
            assert_eq!(fact.context_ref, "c1");
            assert_eq!(fact.unit_ref.as_deref(), Some("u1"));
            assert_eq!(fact.decimals.as_deref(), Some("-3"));
            assert!(!fact.is_nil);
        });
    }

    #[test]
    fn test_parse_tuple_fact() {
        let xml = r#"<xbrli:xbrl xmlns:xbrli="http://www.xbrl.org/2003/instance"
                                xmlns:t="http://example.com/taxonomy">
                                <t:Address>
                                    <t:Street contextRef="c1">Main Street</t:Street>
                                    <t:City contextRef="c1">Berlin</t:City>
                                </t:Address>
                            </xbrli:xbrl>"#;
        let mut parser = InstanceParser::from_reader(xml.as_bytes());
        let instance = parser.parse().unwrap();

        assert_eq!(instance.facts.len(), 1);
        let fact = &instance.facts[0];
        assert_matches!(fact, RawFact::Tuple(tuple) => {
            assert_eq!(tuple.name.to_string(), "t:Address");
            assert!(!tuple.is_nil);
            assert_eq!(tuple.children.len(), 2);

            assert_matches!(&tuple.children[0], RawFact::Item(item) => {
                assert_eq!(item.name.to_string(), "t:Street");
                assert_eq!(item.value, "Main Street");
                assert_eq!(item.context_ref, "c1");
            });
            assert_matches!(&tuple.children[1], RawFact::Item(item) => {
                assert_eq!(item.name.to_string(), "t:City");
                assert_eq!(item.value, "Berlin");
                assert_eq!(item.context_ref, "c1");
            });
        });
    }

    #[test]
    fn test_parse_nested_tuple() {
        let xml = r#"<xbrli:xbrl xmlns:xbrli="http://www.xbrl.org/2003/instance"
                                xmlns:t="http://example.com/taxonomy">
                                <t:Outer>
                                    <t:Inner>
                                        <t:Value contextRef="c1">42</t:Value>
                                    </t:Inner>
                                </t:Outer>
                            </xbrli:xbrl>"#;
        let mut parser = InstanceParser::from_reader(xml.as_bytes());
        let instance = parser.parse().unwrap();

        assert_eq!(instance.facts.len(), 1);
        let fact = &instance.facts[0];
        assert_matches!(fact, RawFact::Tuple(outer) => {
            assert_eq!(outer.name.to_string(), "t:Outer");
            assert!(!outer.is_nil);
            assert_eq!(outer.children.len(), 1);

            assert_matches!(&outer.children[0], RawFact::Tuple(inner) => {
                assert_eq!(inner.name.to_string(), "t:Inner");
                assert!(!inner.is_nil);
                assert_eq!(inner.children.len(), 1);

                assert_matches!(&inner.children[0], RawFact::Item(item) => {
                    assert_eq!(item.name.to_string(), "t:Value");
                    assert_eq!(item.value, "42");
                    assert_eq!(item.context_ref, "c1");
                });
            });
        });
    }

    #[test]
    fn test_parse_nil_item_fact() {
        let xml = r#"<xbrli:xbrl xmlns:xbrli="http://www.xbrl.org/2003/instance"
                            xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
                            xmlns:ifrs="http://xbrl.ifrs.org/taxonomy/2023">
                            <ifrs:Revenue contextRef="c1" xsi:nil="true" />
                        </xbrli:xbrl>"#;
        let mut parser = InstanceParser::from_reader(xml.as_bytes());
        let instance = parser.parse().unwrap();

        assert_eq!(instance.facts.len(), 1);
        match &instance.facts[0] {
            RawFact::Item(fact) => {
                assert_eq!(fact.name.to_string(), "ifrs:Revenue");
                assert!(fact.is_nil);
                assert_eq!(fact.value, "");
                assert_eq!(fact.context_ref, "c1");
            }
            RawFact::Tuple(_) => panic!("expected item fact"),
        }
    }

    #[test]
    fn test_parse_empty_tuple() {
        let xml = r#"<xbrli:xbrl xmlns:xbrli="http://www.xbrl.org/2003/instance"
                            xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
                            xmlns:t="http://example.com/taxonomy">
                            <t:Address xsi:nil="true" />
                        </xbrli:xbrl>"#;
        let mut parser = InstanceParser::from_reader(xml.as_bytes());
        let instance = parser.parse().unwrap();

        assert_eq!(instance.facts.len(), 1);
        match &instance.facts[0] {
            RawFact::Tuple(tuple) => {
                assert_eq!(tuple.name.to_string(), "t:Address");
                assert!(tuple.is_nil);
                assert!(tuple.children.is_empty());
            }
            RawFact::Item(_) => panic!("expected tuple fact"),
        }
    }

    #[test]
    fn test_parse_footnote_link() {
        let xml = r##"<xbrli:xbrl xmlns:xbrli="http://www.xbrl.org/2003/instance"
                            xmlns:ifrs="http://xbrl.ifrs.org/taxonomy/2023">
                            <link:footnoteLink role="http://example.com/footnote">
                                <link:loc xlink:label="loc1" xlink:href="#c1" />
                                <link:footnote xlink:label="fn1" xml:lang="en">
                                    This is a footnote.
                                </link:footnote>
                                <link:footnoteArc xlink:from="loc1" xlink:to="fn1" />
                            </link:footnoteLink>
                        </xbrli:xbrl>"##;
        let mut parser = InstanceParser::from_reader(xml.as_bytes());
        let instance = parser.parse().unwrap();

        assert_eq!(instance.footnote_links.len(), 1);
        let footnote_link = &instance.footnote_links[0];
        assert_eq!(
            footnote_link,
            &RawFootnoteLink {
                role: "http://example.com/footnote".to_string(),
                locators: vec![Locator {
                    label: "loc1".to_string(),
                    href: "#c1".to_string(),
                }],
                arcs: vec![FootnoteArc {
                    from: "loc1".to_string(),
                    to: "fn1".to_string(),
                }],
                footnotes: vec![FootnoteResource {
                    label: "fn1".to_string(),
                    lang: Some("en".to_string()),
                    text: "This is a footnote.".to_string(),
                }],
            }
        );
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
        let mut parser = InstanceParser::from_reader(xml.as_bytes());
        let instance = parser.parse().unwrap();

        assert_eq!(instance.contexts.len(), 1);
        assert_eq!(instance.units.len(), 1);
        assert_eq!(instance.facts.len(), 1);

        assert_eq!(
            instance,
            RawInstance {
                namespaces: {
                    let mut namespaces = HashMap::new();
                    namespaces.insert("xbrli".into(), "http://www.xbrl.org/2003/instance".into());
                    namespaces.insert("ifrs".into(), "http://xbrl.ifrs.org/taxonomy/2023".into());
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
                        segment_dimensions: vec![],
                    },
                    period: RawPeriod::Instant("2024-12-31".to_string()),
                    scenario_dimensions: vec![],
                }],
                units: vec![RawUnit {
                    id: "u1".to_string(),
                    numerator: vec![QName::from_str("iso4217:EUR").unwrap()],
                    denominator: vec![],
                }],
                facts: vec![RawFact::Item(RawItemFact {
                    name: QName::from_str("ifrs:Revenue").unwrap(),
                    value: "1200000".to_string(),
                    context_ref: "c1".to_string(),
                    unit_ref: Some("u1".to_string()),
                    decimals: Some("-3".to_string()),
                    precision: None,
                    id: None,
                    is_nil: false,
                })],
                footnote_links: vec![],
            }
        );
    }
}
