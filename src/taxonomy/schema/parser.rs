use crate::XbrlError;
use quick_xml::{
    Reader,
    events::{
        Event,
        attributes::{self, Attributes},
    },
};
use std::{
    borrow::Cow,
    collections::HashMap,
    io::BufRead,
    path::{Path, PathBuf},
    str::FromStr,
};

/// Represents the different XML tags that can appear in `xs:appinfo` sections
/// of an XBRL schema document.
enum SchemaAppInfoTag {
    /// A `link:roleType` definition.
    RoleType,
    /// An `link:arcroleType` definition.
    ArcroleType,
    /// A `link:linkbaseRef` entry.
    LinkbaseRef,
}

/// Represents the different XML tags that can appear in XBRL linkbase
/// documents.
enum LinkbaseTag {
    /// A `link:linkbase` root element.
    Linkbase,
    /// A `link:presentationLink` element.
    PresentationLink,
    /// A `link:calculationLink` element.
    CalculationLink,
    /// A `link:definitionLink` element.
    DefinitionLink,
    /// A `link:labelLink` element.
    LabelLink,
    /// A `link:referenceLink` element.
    ReferenceLink,
    /// A `link:footnoteLink` element.
    FootnoteLink,
    /// A `link:loc` element (used in arcs to reference a concept by its QName).
    Loc,
    /// A `link:presentationArc` element.
    PresentationArc,
    /// A `link:calculationArc` element.
    CalculationArc,
    /// A `link:definitionArc` element.
    DefinitionArc,
    /// A `link:labelArc` element.
    LabelArc,
    /// A `link:referenceArc` element.
    ReferenceArc,
    /// A `link:footnoteArc` element.
    FootnoteArc,
    /// A `link:label` element.
    Label,
    /// A `link:reference` element.
    Reference,
    /// A `link:footnote` element.
    Footnote,
    /// A `link:roleRef` element.
    RoleRef,
    /// A `link:arcroleRef` element.
    ArcroleRef,
}

/// The XBRL balance type for a monetary taxonomy element (`xbrli:balance` attribute).
#[derive(Debug, PartialEq, Eq)]
pub enum Balance {
    /// An asset or expense concept (increases on the debit side).
    Debit,
    /// A liability, equity, or income concept (increases on the credit side).
    Credit,
}

/// The XBRL period type for a taxonomy element (`xbrli:periodType` attribute).
#[derive(Debug, PartialEq, Eq)]
pub enum PeriodType {
    /// The element reports a value at a specific point in time.
    Instant,
    /// The element reports a value over a time range.
    Duration,
}

/// Represents the `elementFormDefault` and `attributeFormDefault` values from
/// an XBRL schema's root `xs:schema` element.
pub enum FormDefault {
    Qualified,
    Unqualified,
}

/// Represents a raw parsed XBRL schema. Contains only the syntax-level data; no
/// resolved `Concept`s yet.
#[derive(Debug, PartialEq, Eq)]
pub struct RawSchema {
    /// Absolute file path of this schema.
    pub file_path: PathBuf,
    /// The targetNamespace of the schema.
    pub target_namespace: Option<String>,
    /// Namespace declarations (prefix -> URI).
    pub namespaces: HashMap<String, String>,
    /// `xs:import` references.
    pub imports: Vec<SchemaImport>,
    /// `xs:include` references.
    pub includes: Vec<SchemaInclude>,
    /// `link:linkbaseRef` entries.
    pub linkbase_refs: Vec<LinkbaseRef>,
    /// Parsed elements (`xs:element`) in this schema.
    pub elements: Vec<Element>,
    /// Parsed simple type definitions (`xs:simpleType`) in this schema.
    pub simple_types: Vec<SimpleType>,
    /// Parsed complex type definitions (`xs:complexType`) in this schema.
    pub complex_types: Vec<ComplexType>,
}

/// A parsed XML element from the schema (xs:element).
#[derive(Debug, PartialEq, Eq)]
pub struct Element {
    /// The element's local name (e.g., "bs.ass.fixAss").
    pub name: String,
    /// The element's id attribute (optional in XBRL).
    pub id: Option<String>,
    /// The type QName (e.g., "xbrli:monetaryItemType").
    pub type_name: Option<QName>,
    /// Substitution group (e.g., "xbrli:item", "xbrli:tuple").
    pub substitution_group: Option<QName>,
    /// Whether this element is nillable.
    pub is_nillable: bool,
    /// Whether this element is abstract.
    pub is_abstract: bool,
    /// The XBRL period type ("instant" or "duration").
    pub period_type: Option<PeriodType>,
    /// The XBRL balance ("debit" or "credit").
    pub balance: Option<Balance>,
}

/// A simple/complex type definition from the schema.
#[derive(Debug, PartialEq, Eq)]
pub struct TypeDefinition {
    /// The type's name.
    pub name: String,
    /// The base type name, if any.
    pub base: Option<String>,
    /// Additional restrictions or facets can be stored as key-value pairs.
    pub restrictions: HashMap<String, String>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum DerivationKind {
    Extension,
    Restriction,
}

#[derive(Debug, PartialEq, Eq)]
pub struct SimpleType {
    pub name: Option<String>,
    pub base: Option<QName>,
    pub enumerations: Vec<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct ComplexType {
    pub name: Option<String>,
    pub base: Option<QName>,
    pub derivation: Option<DerivationKind>,
}

/// Represents an `xs:import` in the schema.
#[derive(Debug, PartialEq, Eq)]
pub struct SchemaImport {
    /// Namespace being imported.
    pub namespace: String,
    /// Location of the imported schema file (from schemaLocation).
    pub schema_location: Option<String>,
}

/// Represents an `xs:include` in the schema.
#[derive(Debug, PartialEq, Eq)]
pub struct SchemaInclude {
    /// Location of the included schema file.
    pub schema_location: String,
}

/// Represents a `link:linkbaseRef` in the schema.
#[derive(Debug, PartialEq, Eq)]
pub struct LinkbaseRef {
    /// Href to the linkbase file.
    pub href: String,
    /// Role type of the linkbase (optional).
    pub role: Option<String>,
    /// Type of the linkbase (extended/simple).
    pub link_type: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct QName {
    pub prefix: Option<String>,
    pub local_name: String,
}

fn parse_qname(value: &str) -> QName {
    if let Some(idx) = value.find(':') {
        QName {
            prefix: Some(value[..idx].to_string()),
            local_name: value[idx + 1..].to_string(),
        }
    } else {
        QName {
            prefix: None,
            local_name: value.to_string(),
        }
    }
}

/// The parser for XBRL schema documents.
pub struct SchemaParser<R> {
    /// Path of the currently parsed schema file, used for error reporting.
    path: PathBuf,
    /// The XML reader for the schema document.
    reader: Reader<R>,
}

impl<R: BufRead> SchemaParser<R> {
    /// Creates a new `SchemaParser` with the given reader and file path.
    pub fn new(reader: R, path: PathBuf) -> Self {
        let mut reader = Reader::from_reader(reader);
        reader.config_mut().trim_text_start = true;
        reader.config_mut().trim_text_end = true;

        Self { path, reader }
    }

    /// Parses an XBRL schema document from the reader. Path is used for error
    /// reporting.
    pub fn parse_schema(&mut self) -> Result<RawSchema, XbrlError> {
        let mut schema = RawSchema {
            file_path: self.path.clone(),
            target_namespace: None,
            namespaces: HashMap::new(),
            imports: vec![],
            includes: vec![],
            linkbase_refs: vec![],
            elements: vec![],
            simple_types: vec![],
            complex_types: vec![],
        };

        let mut has_schema_root = false;
        let mut buf = Vec::new();

        loop {
            match self.reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref event)) | Ok(Event::Empty(ref event)) => {
                    let event_name = event.name();
                    let local_name = event_name.local_name();
                    let attributes = event.attributes();

                    match local_name.as_ref() {
                        b"schema" => {
                            has_schema_root = true;
                            self.parse_schema_root(&mut schema, attributes)?;
                        }
                        b"import" => self.parse_import(&mut schema, attributes)?,
                        b"include" => self.parse_include(&mut schema, attributes)?,
                        b"element" => self.parse_element(&mut schema, attributes)?,
                        b"simpleType" => {
                            let simple_type = self.parse_simple_type(attributes)?;
                            schema.simple_types.push(simple_type);
                        }
                        b"complexType" => {
                            let complex_type = self.parse_complex_type(attributes)?;
                            schema.complex_types.push(complex_type);
                        }
                        b"restriction" => self.parse_restriction(&mut schema, attributes)?,
                        b"extension" => self.parse_extension(&mut schema, attributes)?,
                        b"sequence" => self.parse_sequence(&mut schema, attributes)?,
                        b"choice" => self.parse_choice(&mut schema, attributes)?,
                        b"attribute" => self.parse_attribute(&mut schema, attributes)?,
                        b"annotation" => self.parse_annotation(&mut schema, attributes)?,
                        b"appinfo" => self.parse_appinfo(&mut schema, attributes)?,
                        b"redefine" => {
                            return Err(XbrlError::InvalidSchemaDocument {
                                path: self.path.clone(),
                                reason: "xsd:redefine is not allowed in taxonomy schemas"
                                    .to_string(),
                            });
                        }
                        other => {
                            return Err(XbrlError::InvalidSchemaDocument {
                                path: self.path.clone(),
                                reason: format!(
                                    "{} is not allowed in taxonomy schemas",
                                    String::from_utf8_lossy(other)
                                ),
                            });
                        }
                    }
                }
                Ok(Event::End(ref event)) => {}
                Ok(Event::Text(_)) => {
                    // TODO: parse `xs:annotation` and `xs:documentation`
                }
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

        if !has_schema_root {
            return Err(XbrlError::InvalidSchemaDocument {
                path: self.path.to_path_buf(),
                reason: "missing <schema> root element".to_string(),
            });
        }

        Ok(schema)
    }

    /// Parses the root `xs:schema` element.
    fn parse_schema_root(
        &mut self,
        schema: &mut RawSchema,
        attributes: Attributes,
    ) -> Result<(), XbrlError> {
        for attribute in attributes.flatten() {
            let local_name = attribute.key.local_name();
            let value = attribute.decode_and_unescape_value(self.reader.decoder())?;

            match local_name.as_ref() {
                b"targetNamespace" => {
                    schema.target_namespace = Some(value.to_string());
                }
                b"xmlns" => {
                    schema.namespaces.insert(
                        str::from_utf8(local_name.as_ref())?.to_string(),
                        value.to_string(),
                    );
                }
                // Not relevant for XBRL taxonomies.
                b"elementFormDefault" | b"attributeFormDefault" => continue,
                _ => {}
            }
        }

        Ok(())
    }

    /// Parses an `xs:import` element.
    fn parse_import(
        &mut self,
        schema: &mut RawSchema,
        attributes: Attributes,
    ) -> Result<(), XbrlError> {
        let mut namespace = None;
        let mut schema_location = None;

        for attribute in attributes.flatten() {
            let local_name = attribute.key.local_name();
            let value = attribute.decode_and_unescape_value(self.reader.decoder())?;

            match local_name.as_ref() {
                b"namespace" => namespace = Some(value.to_string()),
                b"schemaLocation" => schema_location = Some(value.to_string()),
                _ => {}
            }
        }

        schema.imports.push(SchemaImport {
            namespace: namespace.ok_or_else(|| XbrlError::InvalidSchemaDocument {
                path: self.path.clone(),
                reason: "missing namespace in xsd:import".to_string(),
            })?,
            schema_location,
        });

        Ok(())
    }

    /// Parses an `xs:include` element.
    fn parse_include(
        &mut self,
        schema: &mut RawSchema,
        attributes: Attributes,
    ) -> Result<(), XbrlError> {
        let mut schema_location = None;

        for attribute in attributes.flatten() {
            let local_name = attribute.key.local_name();
            let value = attribute.decode_and_unescape_value(self.reader.decoder())?;

            if local_name.as_ref() == b"schemaLocation" {
                schema_location = Some(value.to_string());
            }
        }

        schema.includes.push(SchemaInclude {
            schema_location: schema_location.ok_or_else(|| XbrlError::InvalidSchemaDocument {
                path: self.path.clone(),
                reason: "missing schemaLocation in xsd:include".to_string(),
            })?,
        });

        Ok(())
    }

    /// Parses an `xs:element` element.
    fn parse_element(
        &mut self,
        schema: &mut RawSchema,
        attributes: Attributes,
    ) -> Result<(), XbrlError> {
        let mut name = None;
        let mut id = None;
        let mut type_name = None;
        let mut substitution_group = None;
        let mut is_abstract = false;
        let mut is_nillable = false;
        let mut period_type = None;
        let mut balance = None;

        for attribute in attributes.flatten() {
            let qname = attribute.key;
            let local_name = qname.local_name();
            let value = attribute.decode_and_unescape_value(self.reader.decoder())?;

            match local_name.as_ref() {
                b"name" => name = Some(value.to_string()),
                b"id" => id = Some(value.to_string()),
                b"type" => type_name = Some(parse_qname(&value)),
                b"substitutionGroup" => substitution_group = Some(parse_qname(&value)),
                b"abstract" => is_abstract = value == "true",
                b"nillable" => is_nillable = value == "true",
                b"periodType" => {
                    period_type = match value.as_ref() {
                        "instant" => Some(PeriodType::Instant),
                        "duration" => Some(PeriodType::Duration),
                        _ => None,
                    }
                }
                b"balance" => {
                    balance = match value.as_ref() {
                        "debit" => Some(Balance::Debit),
                        "credit" => Some(Balance::Credit),
                        _ => None,
                    }
                }
                _ => {}
            }
        }

        let element = Element {
            name: name.ok_or_else(|| XbrlError::InvalidSchemaDocument {
                path: self.path.clone(),
                reason: "missing name in xsd:element".to_string(),
            })?,
            id: id.clone(),
            type_name,
            substitution_group,
            is_nillable,
            is_abstract,
            period_type,
            balance,
        };

        schema.elements.push(element);

        Ok(())
    }

    /// Parses an `xs:simpleType` element.
    fn parse_simple_type(&mut self, attributes: Attributes) -> Result<SimpleType, XbrlError> {
        let mut name = None;

        for attribute in attributes.flatten() {
            let qname = attribute.key;
            let local_name = qname.local_name();

            if local_name.as_ref() == b"name" {
                let value = attribute.decode_and_unescape_value(self.reader.decoder())?;
                name = Some(value.to_string());
            }
        }

        let mut base = None;
        let mut enumerations = Vec::new();
        let mut buf = Vec::new();

        loop {
            match self.reader.read_event_into(&mut buf)? {
                Event::Start(event) | Event::Empty(event) => {
                    let local_name = event.name().local_name();
                    match local_name.as_ref() {
                        b"restriction" => {
                            for attribute in event.attributes().flatten() {
                                if attribute.key.as_ref() == b"base" {
                                    let value = attribute
                                        .decode_and_unescape_value(self.reader.decoder())?;
                                    base = Some(parse_qname(&value));
                                }
                            }
                        }
                        b"enumeration" => {
                            for attribute in event.attributes().flatten() {
                                if attribute.key.as_ref() == b"value" {
                                    let value = attribute
                                        .decode_and_unescape_value(self.reader.decoder())?;
                                    enumerations.push(value.to_string());
                                }
                            }
                        }
                        _ => {}
                    }
                }
                Event::End(event) => {
                    let local_name = event.name().local_name();

                    if local_name.as_ref() == b"simpleType" {
                        break;
                    }
                }
                Event::Eof => break,
                _ => {}
            }

            buf.clear();
        }

        Ok(SimpleType {
            name,
            base,
            enumerations,
        })
    }

    /// Parses an `xs:complexType` element.
    fn parse_complex_type(&mut self, attributes: Attributes) -> Result<ComplexType, XbrlError> {
        todo!()
    }

    /// Parses an `xs:restriction` element.
    fn parse_restriction(
        &mut self,
        schema: &mut RawSchema,
        attributes: Attributes,
    ) -> Result<(), XbrlError> {
        todo!()
    }

    /// Parses an `xs:extension` element.
    fn parse_extension(
        &mut self,
        schema: &mut RawSchema,
        attributes: Attributes,
    ) -> Result<(), XbrlError> {
        todo!()
    }

    /// Parses an `xs:sequence` element.
    fn parse_sequence(
        &mut self,
        schema: &mut RawSchema,
        attributes: Attributes,
    ) -> Result<(), XbrlError> {
        todo!()
    }

    /// Parses an `xs:choice` element.
    fn parse_choice(
        &mut self,
        schema: &mut RawSchema,
        attributes: Attributes,
    ) -> Result<(), XbrlError> {
        todo!()
    }

    /// Parses an `xs:attribute` element.
    fn parse_attribute(
        &mut self,
        schema: &mut RawSchema,
        attributes: Attributes,
    ) -> Result<(), XbrlError> {
        todo!()
    }

    /// Parses an `xs:annotation` element.
    fn parse_annotation(
        &mut self,
        schema: &mut RawSchema,
        attributes: Attributes,
    ) -> Result<(), XbrlError> {
        todo!()
    }

    fn parse_appinfo(
        &mut self,
        schema: &mut RawSchema,
        attributes: Attributes,
    ) -> Result<(), XbrlError> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_import() {
        let xml = r#"<xsd:schema
                                xmlns:xsd="http://www.w3.org/2001/XMLSchema"
                                xmlns:xbrli="http://www.xbrl.org/2003/instance"
                                targetNamespace="http://example.com/taxonomy"
                                elementFormDefault="qualified">
                                <xsd:import
                                    namespace="http://www.xbrl.org/2003/instance"
                                    schemaLocation="http://www.xbrl.org/2003/xbrl-instance-2003-12-31.xsd" />
                            </xsd:schema>"#;
        let mut parser = SchemaParser::new(xml.as_bytes(), PathBuf::from("test.xsd"));
        let schema = parser.parse_schema().unwrap();

        let imports = &schema.imports;
        assert!(imports.len() == 1);
        let import = &imports[0];
        assert_eq!(import.namespace, "http://www.xbrl.org/2003/instance");
        assert_eq!(
            import.schema_location,
            Some("http://www.xbrl.org/2003/xbrl-instance-2003-12-31.xsd".to_string())
        );
    }

    #[test]
    fn test_parse_include() {
        let xml = r#"<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema"
                                targetNamespace="http://example.com"
                                xmlns="http://example.com">
                                <xs:include schemaLocation="test.xsd" />
                            </xs:schema>"#;
        let mut parser = SchemaParser::new(xml.as_bytes(), PathBuf::from("test.xsd"));
        let schema = parser.parse_schema().unwrap();

        let includes = &schema.includes;
        assert!(includes.len() == 1);
        let include = &includes[0];
        assert_eq!(include.schema_location, "test.xsd");
    }

    #[test]
    fn test_parse_element() {
        let xml = r#"<xsd:schema
                                xmlns:xsd="http://www.w3.org/2001/XMLSchema"
                                xmlns:xbrli="http://www.xbrl.org/2003/instance">
                                <xsd:element
                                    name="Revenue"
                                    id="Revenue"
                                    type="xbrli:monetaryItemType"
                                    substitutionGroup="xbrli:item"
                                    xbrli:periodType="duration"
                                    abstract="false"
                                    nillable="true" />
                            </xsd:schema>"#;
        let mut parser = SchemaParser::new(xml.as_bytes(), PathBuf::from("test.xsd"));
        let schema = parser.parse_schema().unwrap();

        assert_eq!(schema.elements.len(), 1);
        let element = &schema.elements[0];
        assert_eq!(
            *element,
            Element {
                name: "Revenue".to_string(),
                id: Some("Revenue".to_string()),
                type_name: Some(QName {
                    prefix: Some("xbrli".to_string()),
                    local_name: "monetaryItemType".to_string(),
                }),
                substitution_group: Some(QName {
                    prefix: Some("xbrli".to_string()),
                    local_name: "item".to_string(),
                }),
                is_nillable: true,
                is_abstract: false,
                period_type: Some(PeriodType::Duration),
                balance: None,
            }
        );
    }

    #[test]
    fn test_parse_simple_type_restriction() {
        let xml = r#"<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema"
                                targetNamespace="http://example.com"
                                xmlns="http://example.com">
                                <xs:simpleType name="myStringType">
                                    <xs:restriction base="xs:string">
                                        <xs:maxLength value="100" />
                                    </xs:restriction>
                                </xs:simpleType>
                            </xs:schema>"#;
        let mut parser = SchemaParser::new(xml.as_bytes(), PathBuf::from("test.xsd"));
        let schema = parser.parse_schema().unwrap();

        assert_eq!(schema.simple_types.len(), 1);
        let simple_type = &schema.simple_types[0];
        assert_eq!(
            *simple_type,
            SimpleType {
                name: Some("myStringType".to_string()),
                base: Some(QName {
                    prefix: Some("xs".to_string()),
                    local_name: "string".to_string()
                }),
                enumerations: vec![],
            }
        );
    }

    #[test]
    fn test_parse_simple_type_enumeration() {
        let xml = r#"<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema"
                                targetNamespace="http://example.com"
                                xmlns="http://example.com">
                                <xs:simpleType name="StatusType">
                                    <xs:restriction base="xs:string">
                                        <xs:enumeration value="Open" />
                                        <xs:enumeration value="Closed" />
                                    </xs:restriction>
                                </xs:simpleType>
                            </xs:schema>"#;
        let mut parser = SchemaParser::new(xml.as_bytes(), PathBuf::from("test.xsd"));
        let schema = parser.parse_schema().unwrap();

        assert_eq!(schema.simple_types.len(), 1);
        let simple_type = &schema.simple_types[0];
        assert_eq!(
            *simple_type,
            SimpleType {
                name: Some("StatusType".to_string()),
                base: Some(QName {
                    prefix: Some("xs".to_string()),
                    local_name: "string".to_string()
                }),
                enumerations: vec!["Open".to_string(), "Closed".to_string()],
            }
        );
    }

    #[test]
    fn test_parse_schema() {
        let xml = r#"<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema"
                                targetNamespace="http://example.com"
                                xmlns="http://example.com"
                                xmlns:xbrli="http://www.xbrl.org/2003/instance">

                                <xs:import namespace="http://www.xbrl.org/2003/instance"
                                    schemaLocation="xbrl-instance.xsd" />

                                <xs:simpleType name="MyStringType">
                                    <xs:restriction base="xs:string" />
                                </xs:simpleType>

                                <xs:element name="Revenue"
                                    type="xbrli:monetaryItemType"
                                    substitutionGroup="xbrli:item"
                                    xbrli:periodType="duration" />
                            </xs:schema>"#;
        let mut parser = SchemaParser::new(xml.as_bytes(), PathBuf::from("test.xsd"));
        let schema = parser.parse_schema().unwrap();

        assert_eq!(
            schema,
            RawSchema {
                file_path: PathBuf::from("test.xsd"),
                target_namespace: Some("http://example.com".to_string()),
                namespaces: {
                    let mut map = HashMap::new();
                    map.insert("xmlns".to_string(), "http://example.com".to_string());
                    map.insert(
                        "xmlns:xbrli".to_string(),
                        "http://www.xbrl.org/2003/instance".to_string(),
                    );
                    map
                },
                imports: vec![SchemaImport {
                    namespace: "http://www.xbrl.org/2003/instance".to_string(),
                    schema_location: Some("xbrl-instance.xsd".to_string()),
                }],
                includes: vec![],
                linkbase_refs: vec![],
                elements: vec![Element {
                    name: "Revenue".to_string(),
                    id: None,
                    type_name: Some(QName {
                        prefix: Some("xbrli".to_string()),
                        local_name: "monetaryItemType".to_string(),
                    }),
                    substitution_group: Some(QName {
                        prefix: Some("xbrli".to_string()),
                        local_name: "item".to_string(),
                    }),
                    is_nillable: false,
                    is_abstract: false,
                    period_type: Some(PeriodType::Duration),
                    balance: None,
                }],
                simple_types: vec![SimpleType {
                    name: Some("MyStringType".to_string()),
                    base: Some(QName {
                        prefix: Some("xs".to_string()),
                        local_name: "string".to_string()
                    }),
                    enumerations: vec![],
                }],
                complex_types: vec![],
            }
        );
    }
}
