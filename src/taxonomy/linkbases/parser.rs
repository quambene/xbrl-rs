use crate::XbrlError;
use quick_xml::{
    Reader,
    events::{BytesStart, Event},
};
use rust_decimal::Decimal;
use std::{io::BufRead, path::PathBuf};

/// A locator in a presentation, calculation, or definition link.
#[derive(Debug, PartialEq, Eq)]
pub struct Locator {
    /// The label of the locator, used to reference it in arcs.
    pub label: String,
    /// The href of the locator, pointing to the element in the taxonomy.
    pub href: String,
}

/// A resource in a label or reference link.
#[derive(Debug, PartialEq, Eq)]
pub struct Resource {
    /// The label of the resource, used to reference it in arcs.
    pub label: String,
    /// The role of the resource, used to specify the type of label or
    /// reference.
    pub role: Option<String>,
    /// The text content of the resource, containing the label or reference
    /// information.
    pub text: String,
}

/// An arc in a presentation link, connecting two locators.
#[derive(Debug, PartialEq, Eq)]
pub struct PresentationArc {
    /// The label of the source locator, referencing the `xlink:label` of a
    /// `loc` element.
    pub from: String,
    /// The label of the target locator, referencing the `xlink:label` of a
    /// `loc` element.
    pub to: String,
    /// The order of the arc, used to specify the sequence of presentation.
    pub order: Option<Decimal>,
    /// The preferred label of the arc, used to specify which label to use when
    /// multiple labels are available for the same element.
    pub preferred_label: Option<String>,
}

/// An arc in a calculation link, connecting two locators.
#[derive(Debug, PartialEq, Eq)]
pub struct CalculationArc {
    /// The label of the source locator, referencing the `xlink:label` of a
    /// `loc` element.
    pub from: String,
    /// The label of the target locator, referencing the `xlink:label` of a
    /// `loc` element.
    pub to: String,
    /// The order of the arc, used to specify the sequence of calculation.
    pub order: Option<Decimal>,
    /// The weight of the arc, used to specify the contribution of the source
    /// element to the target element.
    pub weight: Decimal,
}

/// An arc in a definition link, connecting two locators.
#[derive(Debug, PartialEq, Eq)]
pub struct DefinitionArc {
    /// The label of the source locator, referencing the `xlink:label` of a
    /// `loc` element.
    pub from: String,
    /// The label of the target locator, referencing the `xlink:label` of a
    /// `loc` element.
    pub to: String,
    /// The role of the arc, used to specify the type of relationship between
    /// the source and target elements.
    pub arcrole: String,
    /// The order of the arc, used to specify the sequence of definition.
    pub order: Option<Decimal>,
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
#[derive(Debug, PartialEq, Eq)]
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
#[derive(Debug, PartialEq, Eq)]
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
#[derive(Debug, PartialEq, Eq)]
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
    pub references: Vec<Resource>,
}

/// A label link, containing locators, arcs, and resources.
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
    pub labels: Vec<Resource>,
}

/// The complete linkbase, containing all types of links.
#[derive(Debug, PartialEq, Eq)]
pub struct Linkbase {
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

impl Linkbase {
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

impl Default for Linkbase {
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

    pub fn parse_linkbase(&mut self, linkbase: &mut Linkbase) -> Result<(), XbrlError> {
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
                        // Parse locator
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
                                    let value = attribute
                                        .decode_and_unescape_value(self.reader.decoder())?;
                                    label = Some(value.to_string())
                                }
                                b"xlink:href" => {
                                    let value = attribute
                                        .decode_and_unescape_value(self.reader.decoder())?;
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
    fn parse_calculation_link(&mut self, event: BytesStart) -> Result<CalculationLink, XbrlError> {
        todo!()
    }

    /// Parses a `definitionLink` element and its children.
    fn parse_definition_link(&mut self, event: BytesStart) -> Result<DefinitionLink, XbrlError> {
        todo!()
    }

    /// Parses a `labelLink` element and its children.
    fn parse_label_link(&mut self, event: BytesStart) -> Result<LabelLink, XbrlError> {
        todo!()
    }

    /// Parses a `referenceLink` element and its children.
    fn parse_reference_link(&mut self, event: BytesStart) -> Result<ReferenceLink, XbrlError> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_matches::assert_matches;

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
        let mut linkbase = Linkbase::default();
        parser.parse_linkbase(&mut linkbase).unwrap();

        assert_eq!(linkbase.presentation_links.len(), 1);
        assert_eq!(linkbase.calculation_links.len(), 0);
        assert_eq!(linkbase.definition_links.len(), 0);
        assert_eq!(linkbase.label_links.len(), 0);
        assert_eq!(linkbase.reference_links.len(), 0);

        let presentation_link = &linkbase.presentation_links[0];
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
}
