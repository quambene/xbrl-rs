use crate::XbrlError;
use quick_xml::{
    Reader,
    events::{BytesStart, Event},
};
use rust_decimal::Decimal;
use std::{io::BufRead, path::PathBuf, str};

/// A locator in a presentation, calculation, or definition link.
#[derive(Debug, PartialEq, Eq)]
pub struct Locator {
    /// The label of the locator, used to reference it in arcs.
    pub label: String,
    /// The href of the locator, pointing to the element in the taxonomy.
    pub href: String,
}

/// A resource in a label link.
#[derive(Debug, PartialEq, Eq)]
pub struct LabelResource {
    /// The label of the resource, used to reference it in arcs.
    pub label: String,
    /// The role of the resource, used to specify the type of label.
    pub role: Option<String>,
    /// The language of the resource (e.g., "en", "de").
    pub lang: String,
    /// The text content of the resource, containing the label information.
    pub text: String,
}

/// A resource in a reference link.
///
/// Note: The XBRL specification does not define a specific structure for
/// reference resources, but they are typically used to provide additional
/// information about the referenced elements, such as citations or
/// explanations. Therefore, we can use a similar structure to `LabelResource`,
/// but without parsing the text content, since it can be arbitrary and may
/// contain complex XML content.
#[derive(Debug, PartialEq, Eq)]
pub struct ReferenceResource {
    /// The label of the resource, used to reference it in arcs.
    pub label: String,
    /// The role of the resource, used to specify the type of reference.
    pub role: Option<String>,
}

/// A parent-child relationship from a presentation linkbase.
#[derive(Debug, Clone, PartialEq)]
pub struct PresentationArc {
    /// Parent concept element ID.
    pub from: String,
    /// Child concept element ID.
    pub to: String,
    /// Display order among siblings.
    pub order: Option<Decimal>,
    /// The preferred label role URI, if specified.
    pub preferred_label: Option<String>,
}

/// A summation-item relationship from a calculation linkbase.
#[derive(Debug, Clone, PartialEq)]
pub struct CalculationArc {
    /// Parent (summation) concept element ID.
    pub from: String,
    /// Child (contributing item) concept element ID.
    pub to: String,
    /// Display order among siblings.
    pub order: Option<Decimal>,
    /// Weight factor (typically 1.0 or -1.0).
    pub weight: Decimal,
}

/// A dimensional relationship from a definition linkbase.
#[derive(Debug, Clone, PartialEq)]
pub struct DefinitionArc {
    /// Source concept element ID.
    pub from: String,
    /// Target concept element ID.
    pub to: String,
    /// Display/processing order.
    pub order: Option<Decimal>,
    /// The arc role URI.
    pub arcrole: String,
}

/// An arc in a label link, connecting a locator to a resource.
#[derive(Debug, PartialEq, Eq)]
pub struct LabelArc {
    /// The label of the source locator, referencing the `xlink:label` of a
    /// `loc` element.
    pub from: String,
    /// The label of the target resource, referencing the `xlink:label` of a
    /// `resource` element.
    pub to: String,
}

/// An arc in a reference link, connecting a locator to a resource.
#[derive(Debug, PartialEq, Eq)]
pub struct ReferenceArc {
    /// The label of the source locator, referencing the `xlink:label` of a
    /// `loc` element.
    pub from: String,
    /// The label of the target resource, referencing the `xlink:label` of a
    /// `resource` element.
    pub to: String,
}

/// A presentation link, containing locators and arcs.
#[derive(Debug, PartialEq)]
pub struct PresentationLink {
    /// The role of the presentation link, used to specify the type of
    /// presentation.
    pub role: String,
    /// The locators in the presentation link, used to reference elements in the
    /// taxonomy.
    pub locators: Vec<Locator>,
    /// The arcs in the presentation link, used to specify the relationships
    /// between locators.
    pub arcs: Vec<PresentationArc>,
}

/// A calculation link, containing locators and arcs.
#[derive(Debug, PartialEq)]
pub struct CalculationLink {
    /// The role of the calculation link, used to specify the type of
    /// calculation.
    pub role: String,
    /// The locators in the calculation link, used to reference elements in the
    /// taxonomy.
    pub locators: Vec<Locator>,
    /// The arcs in the calculation link, used to specify the relationships
    /// between locators.
    pub arcs: Vec<CalculationArc>,
}

/// A definition link, containing locators and arcs.
#[derive(Debug, PartialEq)]
pub struct DefinitionLink {
    /// The role of the definition link, used to specify the type of
    /// relationship.
    pub role: String,
    /// The locators in the definition link, used to reference elements in the
    /// taxonomy.
    pub locators: Vec<Locator>,
    /// The arcs in the definition link, used to specify the relationships
    /// between locators.
    pub arcs: Vec<DefinitionArc>,
}

/// A reference link, containing locators, arcs, and resources.
#[derive(Debug, PartialEq, Eq)]
pub struct ReferenceLink {
    /// The role of the reference link, used to specify the type of
    /// relationship.
    pub role: String,
    /// The locators in the reference link, used to reference elements in the
    /// taxonomy.
    pub locators: Vec<Locator>,
    /// The arcs in the reference link, used to specify the relationships
    /// between locators.
    pub arcs: Vec<ReferenceArc>,
    /// The resources in the reference link, used to provide additional
    /// information about the referenced elements.
    pub references: Vec<ReferenceResource>,
}

/// A label link, containing locators, arcs, and resources.
///
/// Locators reference concepts in the taxonomy, arcs connect locators to label
/// resources, and label resources contain the actual label text and metadata:
/// ```text
/// Concept (in schema)
///       ↑
///    locator
///       │
///       │ arc
///       ▼
///    resource
///       ↓
///  human-readable content
/// ```
#[derive(Debug, PartialEq, Eq)]
pub struct LabelLink {
    /// The role of the label link, used to specify the type of
    /// relationship.
    pub role: String,
    /// The locators in the label link, used to reference elements in the
    /// taxonomy.
    pub locators: Vec<Locator>,
    /// The arcs in the label link, used to specify the relationships
    /// between locators.
    pub arcs: Vec<LabelArc>,
    /// The labels in the label link, used to provide additional
    /// information about the referenced elements.
    pub labels: Vec<LabelResource>,
}

/// The complete linkbase, containing all types of links.
#[derive(Debug, PartialEq)]
pub struct Linkbases {
    /// The presentation links in the linkbase, used to specify the presentation
    /// of elements in the taxonomy.
    pub presentation_links: Vec<PresentationLink>,
    /// The calculation links in the linkbase, used to specify the calculations
    /// between elements in the taxonomy.
    pub calculation_links: Vec<CalculationLink>,
    /// The definition links in the linkbase, used to specify the relationships
    /// between elements in the taxonomy.
    pub definition_links: Vec<DefinitionLink>,
    /// The label links in the linkbase, used to specify the labels of elements
    /// in the taxonomy.
    pub label_links: Vec<LabelLink>,
    /// The reference links in the linkbase, used to specify the references of
    /// elements in the taxonomy.
    pub reference_links: Vec<ReferenceLink>,
}

impl Linkbases {
    /// Creates a new `Linkbase` with the given links.
    pub fn new(
        presentation_links: Vec<PresentationLink>,
        calculation_links: Vec<CalculationLink>,
        definition_links: Vec<DefinitionLink>,
        label_links: Vec<LabelLink>,
        reference_links: Vec<ReferenceLink>,
    ) -> Self {
        Self {
            presentation_links,
            calculation_links,
            definition_links,
            label_links,
            reference_links,
        }
    }
}

impl Default for Linkbases {
    fn default() -> Self {
        Self {
            presentation_links: Vec::new(),
            calculation_links: Vec::new(),
            definition_links: Vec::new(),
            label_links: Vec::new(),
            reference_links: Vec::new(),
        }
    }
}

/// Parses a string into a Decimal.
fn parse_decimal(value: &str) -> Result<Decimal, XbrlError> {
    value.parse::<Decimal>().map_err(|_| XbrlError::ParseError {
        expected: "floating point number",
        value: value.to_string(),
    })
}

/// The parser for XBRL linkbase documents.
pub struct LinkbaseParser<R> {
    /// Path of the currently parsed linkbase file, used for error reporting.
    path: PathBuf,
    /// The XML reader for the linkbase document.
    reader: Reader<R>,
}

impl<R: BufRead> LinkbaseParser<R> {
    /// Creates a new `LinkbaseParser` with the given reader and file path.
    pub fn new(reader: R, path: PathBuf) -> Self {
        let mut reader = Reader::from_reader(reader);
        reader.config_mut().trim_text_start = true;
        reader.config_mut().trim_text_end = true;

        Self { path, reader }
    }

    pub fn parse_linkbase(&mut self, linkbase: &mut Linkbases) -> Result<(), XbrlError> {
        let mut buf = Vec::new();

        loop {
            match self.reader.read_event_into(&mut buf)? {
                quick_xml::events::Event::Start(event) => match event.local_name().as_ref() {
                    b"presentationLink" => {
                        let link = self.parse_presentation_link(event)?;
                        linkbase.presentation_links.push(link);
                    }

                    b"calculationLink" => {
                        let link = self.parse_calculation_link(event)?;
                        linkbase.calculation_links.push(link);
                    }

                    b"definitionLink" => {
                        let link = self.parse_definition_link(event)?;
                        linkbase.definition_links.push(link);
                    }

                    b"labelLink" => {
                        let link = self.parse_label_link(event)?;
                        linkbase.label_links.push(link);
                    }

                    b"referenceLink" => {
                        let link = self.parse_reference_link(event)?;
                        linkbase.reference_links.push(link);
                    }

                    _ => {}
                },
                Event::Eof => break,

                _ => {}
            }

            buf.clear();
        }

        Ok(())
    }

    /// Parses a `presentationLink` element and its children.
    fn parse_presentation_link(
        &mut self,
        start: BytesStart,
    ) -> Result<PresentationLink, XbrlError> {
        // Extract the `xlink:role` attribute
        let mut role = None;
        for attribute in start.attributes() {
            let attribute = attribute.map_err(|err| XbrlError::XmlParse {
                position: self.reader.buffer_position(),
                element: Some("presentationLink".to_string()),
                source: err.into(),
            })?;

            if attribute.key.as_ref() == b"xlink:role" {
                let value = attribute.decode_and_unescape_value(self.reader.decoder())?;
                role = Some(value.to_string());
            }
        }
        let role = role.ok_or_else(|| XbrlError::ParseError {
            expected: "xlink:role on presentationLink",
            value: "".to_string(),
        })?;

        let mut locators = Vec::new();
        let mut arcs = Vec::new();
        let mut buf = Vec::new();

        loop {
            match self.reader.read_event_into(&mut buf)? {
                Event::Start(event) | Event::Empty(event) => match event.local_name().as_ref() {
                    b"loc" => {
                        let locator = self.parse_locator(&event)?;
                        locators.push(locator);
                    }
                    b"presentationArc" => {
                        // Parse arc
                        let mut from = None;
                        let mut to = None;
                        let mut order = None;
                        let mut preferred_label = None;

                        for attribute in event.attributes() {
                            let attribute = attribute.map_err(|err| XbrlError::XmlParse {
                                position: self.reader.buffer_position(),
                                element: Some("presentationLink".to_string()),
                                source: err.into(),
                            })?;

                            match attribute.key.as_ref() {
                                b"xlink:from" => {
                                    let value = attribute
                                        .decode_and_unescape_value(self.reader.decoder())?;
                                    from = Some(value.to_string())
                                }
                                b"xlink:to" => {
                                    let value = attribute
                                        .decode_and_unescape_value(self.reader.decoder())?;
                                    to = Some(value.to_string())
                                }
                                b"order" => {
                                    let value = attribute
                                        .decode_and_unescape_value(self.reader.decoder())?;
                                    order = Some(parse_decimal(&value)?);
                                }
                                b"preferredLabel" => {
                                    let value = attribute
                                        .decode_and_unescape_value(self.reader.decoder())?;
                                    preferred_label = Some(value.to_string())
                                }
                                _ => {}
                            }
                        }

                        let arc = PresentationArc {
                            from: from.ok_or_else(|| XbrlError::ParseError {
                                expected: "xlink:from on presentationArc",
                                value: "".to_string(),
                            })?,
                            to: to.ok_or_else(|| XbrlError::ParseError {
                                expected: "xlink:to on presentationArc",
                                value: "".to_string(),
                            })?,
                            order,
                            preferred_label,
                        };
                        arcs.push(arc);
                    }

                    _ => {}
                },
                Event::End(event) => {
                    if event.name() == start.name() {
                        break;
                    }
                }
                Event::Eof => {
                    return Err(XbrlError::ParseError {
                        expected: "presentationLink end tag",
                        value: "".to_string(),
                    });
                }

                _ => {}
            }

            buf.clear();
        }

        Ok(PresentationLink {
            role,
            locators,
            arcs,
        })
    }

    /// Parses a `calculationLink` element and its children.
    fn parse_calculation_link(&mut self, start: BytesStart) -> Result<CalculationLink, XbrlError> {
        // Extract the `xlink:role` attribute
        let mut role = None;
        for attribute in start.attributes() {
            let attribute = attribute.map_err(|err| XbrlError::XmlParse {
                position: self.reader.buffer_position(),
                element: Some("calculationLink".to_string()),
                source: err.into(),
            })?;

            if attribute.key.as_ref() == b"xlink:role" {
                let value = attribute.decode_and_unescape_value(self.reader.decoder())?;
                role = Some(value.to_string());
            }
        }
        let role = role.ok_or_else(|| XbrlError::ParseError {
            expected: "xlink:role on calculationLink",
            value: "".to_string(),
        })?;

        let mut locators = Vec::new();
        let mut arcs = Vec::new();
        let mut buf = Vec::new();

        loop {
            match self.reader.read_event_into(&mut buf)? {
                Event::Start(event) | Event::Empty(event) => match event.local_name().as_ref() {
                    b"loc" => {
                        let locator = self.parse_locator(&event)?;
                        locators.push(locator);
                    }
                    b"calculationArc" => {
                        // Parse a calculation arc
                        let mut from = None;
                        let mut to = None;
                        let mut order = None;
                        let mut weight = None;

                        for attribute in event.attributes() {
                            let attribute = attribute.map_err(|err| XbrlError::XmlParse {
                                position: self.reader.buffer_position(),
                                element: Some("calculationLink".to_string()),
                                source: err.into(),
                            })?;

                            match attribute.key.as_ref() {
                                b"xlink:from" => {
                                    let value = attribute
                                        .decode_and_unescape_value(self.reader.decoder())?;
                                    from = Some(value.to_string())
                                }
                                b"xlink:to" => {
                                    let value = attribute
                                        .decode_and_unescape_value(self.reader.decoder())?;
                                    to = Some(value.to_string())
                                }
                                b"order" => {
                                    let value = attribute
                                        .decode_and_unescape_value(self.reader.decoder())?;
                                    order = Some(parse_decimal(&value)?);
                                }
                                b"weight" => {
                                    let value = attribute
                                        .decode_and_unescape_value(self.reader.decoder())?;
                                    weight = Some(parse_decimal(&value)?);
                                }
                                _ => {}
                            }
                        }

                        arcs.push(CalculationArc {
                            from: from.ok_or_else(|| XbrlError::ParseError {
                                expected: "xlink:from on calculationArc",
                                value: "".to_string(),
                            })?,
                            to: to.ok_or_else(|| XbrlError::ParseError {
                                expected: "xlink:to on calculationArc",
                                value: "".to_string(),
                            })?,
                            order,
                            weight: weight.ok_or_else(|| XbrlError::ParseError {
                                expected: "weight on calculationArc",
                                value: "".to_string(),
                            })?,
                        });
                    }
                    _ => {}
                },
                Event::End(event) => {
                    if event.name() == start.name() {
                        break;
                    }
                }
                Event::Eof => {
                    return Err(XbrlError::ParseError {
                        expected: "calculationLink end tag",
                        value: "".to_string(),
                    });
                }
                _ => {}
            }

            buf.clear();
        }

        Ok(CalculationLink {
            role,
            locators,
            arcs,
        })
    }

    /// Parses a `definitionLink` element and its children.
    fn parse_definition_link(&mut self, start: BytesStart) -> Result<DefinitionLink, XbrlError> {
        // Extract the `xlink:role` attribute
        let mut role = None;
        for attribute in start.attributes() {
            let attribute = attribute.map_err(|err| XbrlError::XmlParse {
                position: self.reader.buffer_position(),
                element: Some("definitionLink".to_string()),
                source: err.into(),
            })?;

            if attribute.key.as_ref() == b"xlink:role" {
                let value = attribute.decode_and_unescape_value(self.reader.decoder())?;
                role = Some(value.to_string());
            }
        }
        let role = role.ok_or_else(|| XbrlError::ParseError {
            expected: "xlink:role on definitionLink",
            value: "".to_string(),
        })?;

        let mut locators = Vec::new();
        let mut arcs = Vec::new();
        let mut buf = Vec::new();

        loop {
            match self.reader.read_event_into(&mut buf)? {
                Event::Start(event) | Event::Empty(event) => match event.local_name().as_ref() {
                    b"loc" => {
                        let locator = self.parse_locator(&event)?;
                        locators.push(locator);
                    }
                    b"definitionArc" => {
                        let mut from = None;
                        let mut to = None;
                        let mut arcrole = None;
                        let mut order = None;

                        for attribute in event.attributes() {
                            let attribute = attribute.map_err(|err| XbrlError::XmlParse {
                                position: self.reader.buffer_position(),
                                element: Some("definitionArc".to_string()),
                                source: err.into(),
                            })?;

                            match attribute.key.as_ref() {
                                b"xlink:from" => {
                                    let value = attribute
                                        .decode_and_unescape_value(self.reader.decoder())?;
                                    from = Some(value.to_string());
                                }
                                b"xlink:to" => {
                                    let value = attribute
                                        .decode_and_unescape_value(self.reader.decoder())?;
                                    to = Some(value.to_string());
                                }
                                b"xlink:arcrole" => {
                                    let value = attribute
                                        .decode_and_unescape_value(self.reader.decoder())?;
                                    arcrole = Some(value.to_string());
                                }
                                b"order" => {
                                    let value = attribute
                                        .decode_and_unescape_value(self.reader.decoder())?;
                                    order = Some(parse_decimal(&value)?);
                                }
                                _ => {}
                            }
                        }

                        arcs.push(DefinitionArc {
                            from: from.ok_or_else(|| XbrlError::ParseError {
                                expected: "xlink:from on definitionArc",
                                value: "".to_string(),
                            })?,
                            to: to.ok_or_else(|| XbrlError::ParseError {
                                expected: "xlink:to on definitionArc",
                                value: "".to_string(),
                            })?,
                            arcrole: arcrole.ok_or_else(|| XbrlError::ParseError {
                                expected: "xlink:arcrole on definitionArc",
                                value: "".to_string(),
                            })?,
                            order,
                        });
                    }
                    _ => {}
                },
                Event::End(event) => {
                    if event.name() == start.name() {
                        break;
                    }
                }
                Event::Eof => {
                    return Err(XbrlError::ParseError {
                        expected: "definitionLink end tag",
                        value: "".to_string(),
                    });
                }
                _ => {}
            }

            buf.clear();
        }

        Ok(DefinitionLink {
            role,
            locators,
            arcs,
        })
    }

    /// Parses a `labelLink` element and its children.
    fn parse_label_link(&mut self, start: BytesStart) -> Result<LabelLink, XbrlError> {
        // Extract the `xlink:role` attribute
        let mut role = String::new();

        for attribute in start.attributes() {
            let attribute = attribute.map_err(|err| XbrlError::XmlParse {
                position: self.reader.buffer_position(),
                element: Some("labelLink".to_string()),
                source: err.into(),
            })?;

            if attribute.key.as_ref() == b"xlink:role" {
                let value = attribute.decode_and_unescape_value(self.reader.decoder())?;
                role = value.to_string();
            }
        }

        let mut locators = Vec::new();
        let mut arcs = Vec::new();
        let mut labels = Vec::new();
        let mut buf = Vec::new();

        loop {
            match self.reader.read_event_into(&mut buf)? {
                Event::Start(event) | Event::Empty(event) => match event.local_name().as_ref() {
                    b"loc" => {
                        let locator = self.parse_locator(&event)?;
                        locators.push(locator);
                    }
                    b"labelArc" => {
                        let mut from = None;
                        let mut to = None;

                        for attribute in event.attributes() {
                            let attribute = attribute.map_err(|err| XbrlError::XmlParse {
                                position: self.reader.buffer_position(),
                                element: Some("labelArc".to_string()),
                                source: err.into(),
                            })?;

                            match attribute.key.as_ref() {
                                b"xlink:from" => {
                                    let value = attribute
                                        .decode_and_unescape_value(self.reader.decoder())?;
                                    from = Some(value.to_string());
                                }
                                b"xlink:to" => {
                                    let value = attribute
                                        .decode_and_unescape_value(self.reader.decoder())?;
                                    to = Some(value.to_string());
                                }
                                _ => {}
                            }
                        }

                        arcs.push(LabelArc {
                            from: from.ok_or_else(|| XbrlError::ParseError {
                                expected: "xlink:from on labelArc",
                                value: "".to_string(),
                            })?,
                            to: to.ok_or_else(|| XbrlError::ParseError {
                                expected: "xlink:to on labelArc",
                                value: "".to_string(),
                            })?,
                        });
                    }
                    b"label" => {
                        let mut label = None;
                        let mut label_role = None;
                        let mut label_lang = None;

                        for attribute in event.attributes() {
                            let attribute = attribute.map_err(|err| XbrlError::XmlParse {
                                position: self.reader.buffer_position(),
                                element: Some("label".to_string()),
                                source: err.into(),
                            })?;

                            match attribute.key.as_ref() {
                                b"xlink:label" => {
                                    let value = attribute
                                        .decode_and_unescape_value(self.reader.decoder())?;
                                    label = Some(value.to_string());
                                }
                                b"xlink:role" => {
                                    let value = attribute
                                        .decode_and_unescape_value(self.reader.decoder())?;
                                    label_role = Some(value.to_string());
                                }
                                b"xml:lang" => {
                                    let value = attribute
                                        .decode_and_unescape_value(self.reader.decoder())?;
                                    label_lang = Some(value.to_string());
                                }
                                _ => {}
                            }
                        }

                        let mut text_buf = Vec::new();
                        let bytes_text = self
                            .reader
                            .read_text_into(event.to_end().name(), &mut text_buf)?;
                        let text = str::from_utf8(bytes_text.as_ref())
                            .map_err(|err| XbrlError::Utf8(err))?
                            .trim()
                            .to_string();

                        labels.push(LabelResource {
                            label: label.ok_or_else(|| XbrlError::ParseError {
                                expected: "xlink:label on label",
                                value: "".to_string(),
                            })?,
                            role: label_role,
                            lang: label_lang.unwrap_or_default(),
                            text,
                        });
                    }
                    _ => {}
                },
                Event::End(event) => {
                    if event.name() == start.name() {
                        break;
                    }
                }
                Event::Eof => {
                    return Err(XbrlError::ParseError {
                        expected: "labelLink end tag",
                        value: "".to_string(),
                    });
                }
                _ => {}
            }

            buf.clear();
        }

        Ok(LabelLink {
            role,
            locators,
            arcs,
            labels,
        })
    }

    /// Parses a `referenceLink` element and its children.
    fn parse_reference_link(&mut self, start: BytesStart) -> Result<ReferenceLink, XbrlError> {
        // Extract the `xlink:role` attribute
        let mut role = String::new();

        for attribute in start.attributes() {
            let attribute = attribute.map_err(|err| XbrlError::XmlParse {
                position: self.reader.buffer_position(),
                element: Some("referenceLink".to_string()),
                source: err.into(),
            })?;

            if attribute.key.as_ref() == b"xlink:role" {
                let value = attribute.decode_and_unescape_value(self.reader.decoder())?;
                role = value.to_string();
            }
        }

        let mut locators = Vec::new();
        let mut arcs = Vec::new();
        let mut references = Vec::new();
        let mut buf = Vec::new();

        loop {
            match self.reader.read_event_into(&mut buf)? {
                Event::Empty(event) | Event::Start(event) => match event.local_name().as_ref() {
                    b"loc" => {
                        let locator = self.parse_locator(&event)?;
                        locators.push(locator);
                    }
                    b"referenceArc" => {
                        let mut from = None;
                        let mut to = None;

                        for attribute in event.attributes() {
                            let attribute = attribute.map_err(|err| XbrlError::XmlParse {
                                position: self.reader.buffer_position(),
                                element: Some("referenceArc".to_string()),
                                source: err.into(),
                            })?;

                            match attribute.key.as_ref() {
                                b"xlink:from" => {
                                    let value = attribute
                                        .decode_and_unescape_value(self.reader.decoder())?;
                                    from = Some(value.to_string());
                                }
                                b"xlink:to" => {
                                    let value = attribute
                                        .decode_and_unescape_value(self.reader.decoder())?;
                                    to = Some(value.to_string());
                                }
                                _ => {}
                            }
                        }

                        arcs.push(ReferenceArc {
                            from: from.ok_or_else(|| XbrlError::ParseError {
                                expected: "xlink:from on referenceArc",
                                value: "".to_string(),
                            })?,
                            to: to.ok_or_else(|| XbrlError::ParseError {
                                expected: "xlink:to on referenceArc",
                                value: "".to_string(),
                            })?,
                        });
                    }
                    b"reference" => {
                        let mut label = None;
                        let mut ref_role = None;

                        for attribute in event.attributes() {
                            let attribute = attribute.map_err(|err| XbrlError::XmlParse {
                                position: self.reader.buffer_position(),
                                element: Some("reference".to_string()),
                                source: err.into(),
                            })?;

                            match attribute.key.as_ref() {
                                b"xlink:label" => {
                                    let value = attribute
                                        .decode_and_unescape_value(self.reader.decoder())?;
                                    label = Some(value.to_string());
                                }
                                b"xlink:role" => {
                                    let value = attribute
                                        .decode_and_unescape_value(self.reader.decoder())?;
                                    ref_role = Some(value.to_string());
                                }
                                _ => {}
                            }
                        }

                        references.push(ReferenceResource {
                            label: label.ok_or_else(|| XbrlError::ParseError {
                                expected: "xlink:label on reference",
                                value: "".to_string(),
                            })?,
                            role: ref_role,
                        });
                    }
                    _ => {}
                },
                Event::End(event) => {
                    if event.name() == start.name() {
                        break;
                    }
                }
                Event::Eof => {
                    return Err(XbrlError::ParseError {
                        expected: "referenceLink end tag",
                        value: "".to_string(),
                    });
                }
                _ => {}
            }

            buf.clear();
        }

        Ok(ReferenceLink {
            role,
            locators,
            arcs,
            references,
        })
    }

    /// Parses a `loc` element and extracts the `xlink:label` and `xlink:href`
    /// attributes.
    fn parse_locator(&mut self, event: &BytesStart) -> Result<Locator, XbrlError> {
        let mut label = None;
        let mut href = None;

        for attribute in event.attributes() {
            let attribute = attribute.map_err(|err| XbrlError::XmlParse {
                position: self.reader.buffer_position(),
                element: Some("presentationLink".to_string()),
                source: err.into(),
            })?;

            match attribute.key.as_ref() {
                b"xlink:label" => {
                    let value = attribute.decode_and_unescape_value(self.reader.decoder())?;
                    label = Some(value.to_string())
                }
                b"xlink:href" => {
                    let value = attribute.decode_and_unescape_value(self.reader.decoder())?;
                    href = Some(value.to_string())
                }
                _ => {}
            }
        }
        let locator = Locator {
            label: label.ok_or_else(|| XbrlError::ParseError {
                expected: "xlink:label on loc",
                value: "".to_string(),
            })?,
            href: href.ok_or_else(|| XbrlError::ParseError {
                expected: "xlink:href on loc",
                value: "".to_string(),
            })?,
        };

        Ok(locator)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_linkbase_presentation_link() {
        let xml = r#"<link:linkbase xmlns:link="http://www.xbrl.org/2003/linkbase">
                                <link:presentationLink
                                    xlink:type="extended"
                                    xlink:role="http://example.com/role/balanceSheet">
                                    <link:loc
                                        xlink:type="locator"
                                        xlink:href="taxonomy.xsd#Assets"
                                        xlink:label="loc_assets" />
                                    <link:loc
                                        xlink:type="locator"
                                        xlink:href="taxonomy.xsd#Cash"
                                        xlink:label="loc_cash" />
                                    <link:presentationArc
                                        xlink:type="arc"
                                        xlink:from="loc_assets"
                                        xlink:to="loc_cash"
                                        order="1" />
                                </link:presentationLink>
                            </link:linkbase>"#;
        let mut parser = LinkbaseParser::new(xml.as_bytes(), PathBuf::from("test.xml"));
        let mut linkbases = Linkbases::default();
        parser.parse_linkbase(&mut linkbases).unwrap();

        assert_eq!(linkbases.presentation_links.len(), 1);
        assert_eq!(linkbases.calculation_links.len(), 0);
        assert_eq!(linkbases.definition_links.len(), 0);
        assert_eq!(linkbases.label_links.len(), 0);
        assert_eq!(linkbases.reference_links.len(), 0);

        let presentation_link = &linkbases.presentation_links[0];
        assert_eq!(
            presentation_link,
            &PresentationLink {
                role: "http://example.com/role/balanceSheet".to_string(),
                locators: vec![
                    Locator {
                        label: "loc_assets".to_string(),
                        href: "taxonomy.xsd#Assets".to_string(),
                    },
                    Locator {
                        label: "loc_cash".to_string(),
                        href: "taxonomy.xsd#Cash".to_string(),
                    },
                ],
                arcs: vec![PresentationArc {
                    from: "loc_assets".to_string(),
                    to: "loc_cash".to_string(),
                    order: Some(Decimal::new(1, 0)),
                    preferred_label: None,
                }],
            }
        );
    }

    #[test]
    fn test_parse_linkbase_calculation_link() {
        let xml = r#"<link:linkbase xmlns:link="http://www.xbrl.org/2003/linkbase">
                            <link:calculationLink xlink:type="extended" xlink:role="">
                                <link:calculationArc
                                    xlink:type="arc"
                                    xlink:from="loc_assets"
                                    xlink:to="loc_cash"
                                    weight="1"
                                    order="1" />
                            </link:calculationLink>
                        </link:linkbase>"#;
        let mut parser = LinkbaseParser::new(xml.as_bytes(), PathBuf::from("test.xml"));
        let mut linkbases = Linkbases::default();
        parser.parse_linkbase(&mut linkbases).unwrap();

        assert_eq!(linkbases.presentation_links.len(), 0);
        assert_eq!(linkbases.calculation_links.len(), 1);
        assert_eq!(linkbases.definition_links.len(), 0);
        assert_eq!(linkbases.label_links.len(), 0);
        assert_eq!(linkbases.reference_links.len(), 0);
        let calculation_link = &linkbases.calculation_links[0];
        assert_eq!(
            calculation_link,
            &CalculationLink {
                role: "".to_string(),
                locators: vec![],
                arcs: vec![CalculationArc {
                    from: "loc_assets".to_string(),
                    to: "loc_cash".to_string(),
                    order: Some(Decimal::new(1, 0)),
                    weight: Decimal::new(1, 0),
                }],
            }
        );
    }

    #[test]
    fn test_parse_linkbase_definition_link() {
        let xml = r#"<link:linkbase xmlns:link="http://www.xbrl.org/2003/linkbase">
                            <link:definitionLink xlink:type="extended" xlink:role="">
                                <link:definitionArc
                                    xlink:type="arc"
                                    xlink:arcrole="http://xbrl.org/int/dim/arcrole/domain-member"
                                    xlink:from="loc_domain"
                                    xlink:to="loc_member" />
                            </link:definitionLink>
                        </link:linkbase>"#;
        let mut parser = LinkbaseParser::new(xml.as_bytes(), PathBuf::from("test.xml"));
        let mut linkbases = Linkbases::default();
        parser.parse_linkbase(&mut linkbases).unwrap();

        assert_eq!(linkbases.presentation_links.len(), 0);
        assert_eq!(linkbases.calculation_links.len(), 0);
        assert_eq!(linkbases.definition_links.len(), 1);
        assert_eq!(linkbases.label_links.len(), 0);
        assert_eq!(linkbases.reference_links.len(), 0);
        let definition_link = &linkbases.definition_links[0];
        assert_eq!(
            definition_link,
            &DefinitionLink {
                role: "".to_string(),
                locators: vec![],
                arcs: vec![DefinitionArc {
                    from: "loc_domain".to_string(),
                    to: "loc_member".to_string(),
                    arcrole: "http://xbrl.org/int/dim/arcrole/domain-member".to_string(),
                    order: None,
                }],
            }
        );
    }

    #[test]
    fn test_parse_linkbase_label_link() {
        let xml = r#"<link:linkbase xmlns:link="http://www.xbrl.org/2003/linkbase">
                            <link:labelLink>
                                <link:loc
                                    xlink:type="locator"
                                    xlink:href="taxonomy.xsd#Assets"
                                    xlink:label="loc_assets" />
                                <link:label
                                    xlink:type="resource"
                                    xlink:label="lab_assets"
                                    xlink:role="http://www.xbrl.org/2003/role/label"
                                    xml:lang="en">
                                    Assets
                                </link:label>
                                <link:labelArc
                                    xlink:type="arc"
                                    xlink:from="loc_assets"
                                    xlink:to="lab_assets" />
                            </link:labelLink>
                        </link:linkbase>"#;
        let mut parser = LinkbaseParser::new(xml.as_bytes(), PathBuf::from("test.xml"));
        let mut linkbases = Linkbases::default();
        parser.parse_linkbase(&mut linkbases).unwrap();

        assert_eq!(linkbases.presentation_links.len(), 0);
        assert_eq!(linkbases.calculation_links.len(), 0);
        assert_eq!(linkbases.definition_links.len(), 0);
        assert_eq!(linkbases.label_links.len(), 1);
        assert_eq!(linkbases.reference_links.len(), 0);
        let label_link = &linkbases.label_links[0];
        assert_eq!(
            label_link,
            &LabelLink {
                role: "".to_string(),
                locators: vec![Locator {
                    label: "loc_assets".to_string(),
                    href: "taxonomy.xsd#Assets".to_string(),
                }],
                arcs: vec![LabelArc {
                    from: "loc_assets".to_string(),
                    to: "lab_assets".to_string(),
                }],
                labels: vec![LabelResource {
                    label: "lab_assets".to_string(),
                    role: Some("http://www.xbrl.org/2003/role/label".to_string()),
                    lang: "en".to_string(),
                    text: "Assets".to_string(),
                }],
            }
        );
    }

    #[test]
    fn test_parse_linkbase_reference_link() {
        let xml = r#"<link:linkbase xmlns:link="http://www.xbrl.org/2003/linkbase"
                            xmlns:xlink="http://www.w3.org/1999/xlink"
                            xmlns:my="http://example.com/my-taxonomy">
                            <!-- Extended link for references -->
                            <link:referenceLink xlink:type="extended" xlink:role="http://www.xbrl.org/2003/role/reference">
                                <!-- Locators point to concepts in the taxonomy -->
                                <link:loc xlink:type="locator" xlink:label="loc_assets" xlink:href="my-taxonomy.xsd#Assets" />
                                <link:loc xlink:type="locator" xlink:label="loc_cash" xlink:href="my-taxonomy.xsd#Cash" />
                                <!-- Arcs connect locators to resources -->
                                <link:referenceArc xlink:type="arc"
                                    xlink:from="loc_assets"
                                    xlink:to="ref_assets"
                                    order="1" />
                                <link:referenceArc xlink:type="arc"
                                    xlink:from="loc_cash"
                                    xlink:to="ref_cash"
                                    order="2" />
                                <!-- Resources provide textual references -->
                                <link:reference xlink:type="resource"
                                    xlink:label="ref_assets"
                                    xlink:role="http://www.xbrl.org/2003/role/statementRef">
                                    <link:content>Test content 1</link:content>
                                </link:reference>
                                <link:reference xlink:type="resource"
                                    xlink:label="ref_cash"
                                    xlink:role="http://www.xbrl.org/2003/role/statementRef">
                                    <link:content>Test content 2</link:content>
                                </link:reference>
                            </link:referenceLink>
                        </link:linkbase>"#;
        let mut parser = LinkbaseParser::new(xml.as_bytes(), PathBuf::from("test.xml"));
        let mut linkbases = Linkbases::default();
        parser.parse_linkbase(&mut linkbases).unwrap();

        assert_eq!(linkbases.presentation_links.len(), 0);
        assert_eq!(linkbases.calculation_links.len(), 0);
        assert_eq!(linkbases.definition_links.len(), 0);
        assert_eq!(linkbases.label_links.len(), 0);
        assert_eq!(linkbases.reference_links.len(), 1);
        let reference_link = &linkbases.reference_links[0];
        assert_eq!(
            reference_link,
            &ReferenceLink {
                role: "http://www.xbrl.org/2003/role/reference".to_string(),
                locators: vec![
                    Locator {
                        label: "loc_assets".to_string(),
                        href: "my-taxonomy.xsd#Assets".to_string(),
                    },
                    Locator {
                        label: "loc_cash".to_string(),
                        href: "my-taxonomy.xsd#Cash".to_string(),
                    },
                ],
                arcs: vec![
                    ReferenceArc {
                        from: "loc_assets".to_string(),
                        to: "ref_assets".to_string(),
                    },
                    ReferenceArc {
                        from: "loc_cash".to_string(),
                        to: "ref_cash".to_string(),
                    },
                ],
                references: vec![
                    ReferenceResource {
                        label: "ref_assets".to_string(),
                        role: Some("http://www.xbrl.org/2003/role/statementRef".to_string()),
                    },
                    ReferenceResource {
                        label: "ref_cash".to_string(),
                        role: Some("http://www.xbrl.org/2003/role/statementRef".to_string()),
                    },
                ],
            }
        );
    }
}
