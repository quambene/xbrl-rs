use crate::{
    Balance, NamespacePrefix, NamespaceUri, PeriodType, XbrlError,
    xml::{self, QName},
};
use quick_xml::{
    Reader,
    events::{BytesStart, Event, attributes::Attributes},
};
use std::{
    collections::HashMap,
    fmt,
    fs::File,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    str::FromStr,
};

/// Represents the `elementFormDefault` and `attributeFormDefault` values from
/// an XBRL schema's root `xs:schema` element.
pub enum FormDefault {
    Qualified,
    Unqualified,
}

/// The kind of derivation in a `simpleContent` extension or restriction.
#[derive(Debug, PartialEq, Eq)]
pub enum DerivationKind {
    Extension,
    Restriction,
}

/// The compositor type for a complex type's content model (sequence or choice).
#[derive(Debug, PartialEq, Eq)]
pub enum Compositor {
    Sequence,
    Choice,
}

/// The allowed cycle direction for an arcrole (`cyclesAllowed` attribute).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CyclesAllowed {
    /// Any cycles are allowed.
    Any,
    /// Only undirected cycles are allowed.
    Undirected,
    /// No cycles are allowed.
    None,
}

impl FromStr for CyclesAllowed {
    type Err = XbrlError;

    fn from_str(str: &str) -> Result<Self, XbrlError> {
        match str {
            "any" => Ok(Self::Any),
            "undirected" => Ok(Self::Undirected),
            "none" => Ok(Self::None),
            _ => Err(XbrlError::ParseError {
                expected: "CyclesAllowed",
                value: str.to_owned(),
            }),
        }
    }
}

impl fmt::Display for CyclesAllowed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Any => f.write_str("any"),
            Self::Undirected => f.write_str("undirected"),
            Self::None => f.write_str("none"),
        }
    }
}

/// Represents an `xs:import` in the schema. Used when a schema needs to import
/// types from another namespace. The `namespace` field is required, but the
/// `schema_location` is optional in XBRL taxonomies.
#[derive(Debug, PartialEq, Eq)]
pub struct SchemaImport {
    /// Namespace being imported.
    pub namespace: String,
    /// Location of the imported schema file (from schemaLocation).
    pub schema_location: Option<String>,
}

/// Represents an `xs:include` in the schema. Used when a schema needs to include
/// types from another schema in the same namespace.
#[derive(Debug, PartialEq, Eq)]
pub struct SchemaInclude {
    /// Location of the included schema file.
    pub schema_location: String,
}

/// Represents a `link:linkbaseRef` in the schema's `xs:annotation/xs:appinfo`.
#[derive(Debug, PartialEq, Eq)]
pub struct LinkbaseRef {
    /// Href to the linkbase file.
    ///
    /// The xlink:href value (relative path to the linkbase file).
    pub href: String,
    /// Role type of the linkbase.
    ///
    /// The xlink:role (e.g., <http://www.xbrl.org/2003/role/labelLinkbaseRef>).
    pub role: Option<String>,
    /// The xlink:arcrole (typically <http://www.w3.org/1999/xlink/properties/linkbase>).
    pub arcrole: Option<String>,
    /// Type of the linkbase (extended/simple).
    pub link_type: Option<String>,
}

/// A `link:roleType` definition from a taxonomy schema.
#[derive(Debug, PartialEq, Eq)]
pub struct RoleType {
    /// The id attribute (e.g., "role_balanceSheet").
    pub id: String,
    /// The roleURI attribute.
    pub role_uri: String,
    /// The human-readable definition (child `link:definition` text).
    pub definition: Option<String>,
    /// Which link types this role is used on (child `link:usedOn` texts).
    pub used_on: Vec<String>,
}

/// A `link:arcroleType` definition from a taxonomy schema.
#[derive(Debug, PartialEq, Eq)]
pub struct ArcroleType {
    /// The id attribute.
    pub id: String,
    /// The arcroleURI attribute.
    pub arcrole_uri: String,
    /// The human-readable definition.
    pub definition: Option<String>,
    /// Which link types this arcrole is used on.
    pub used_on: Vec<String>,
    /// The cycles-allowed attribute.
    pub cycles_allowed: Option<CyclesAllowed>,
}

/// A child element of a tuple (`xs:element[@ref]` inside an inline
/// `xs:complexType`).
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct RawTupleChild {
    /// The QName of the referenced element.
    pub name: QName,
    /// The minimum number of occurrences of this child element (from
    /// `minOccurs`).
    pub min_occurs: u32,
    /// The maximum number of occurrences of this child element (from
    /// `maxOccurs`).
    pub max_occurs: Option<u32>,
}

/// Represents an attribute use in a `simpleContent` extension or restriction.
#[derive(Debug, PartialEq, Eq)]
pub struct AttributeUse {
    /// The QName of the referenced attribute.
    pub ref_name: String,
    /// Whether this attribute is required (use="required").
    pub required: bool,
}

/// Represents a simple type definition (`xs:simpleType`) in the schema.
#[derive(Debug, PartialEq, Eq)]
pub struct SimpleType {
    /// The name of the simple type.
    pub name: Option<String>,
    /// The base type of the simple type in a restriction (`xs:restriction
    /// base="..."`).
    pub base: Option<QName>,
    /// The enumerations of the simple type. Only relevant for simple types that
    /// are restrictions of an enumeration.
    pub enumerations: Vec<String>,
}

/// Represents a complex type definition (`xs:complexType`) in the schema.
#[derive(Debug, PartialEq, Eq)]
pub struct ComplexType {
    /// The name of the complex type.
    pub name: Option<String>,
    /// The base type of the complex type (from `xs:extension` or
    /// `xs:restriction`).
    pub base: Option<QName>,
    /// The kind of derivation (extension or restriction) if this complex type
    /// is derived from another type via `xs:simpleContent`.
    pub derivation: Option<DerivationKind>,
    /// The compositor type for the complex type's content model (sequence or
    /// choice).
    pub compositor: Option<Compositor>,
    /// Attributes declared via `xs:attribute[@ref]` inside an
    /// `xs:simpleContent` of a tuple element.
    pub attributes: Vec<AttributeUse>,
    /// Child elements declared via `xs:element[@ref]` inside an inline
    /// `xs:complexType` of a tuple element.
    pub children: Vec<RawTupleChild>,
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
    /// The complex type of a tuple element.
    pub complex_type: Option<ComplexType>,
}

/// Represents a raw parsed XBRL schema. Contains only the syntax-level data; no
/// resolved `Concept`s yet.
#[derive(Debug, PartialEq, Eq)]
pub struct RawSchema {
    /// The targetNamespace of the schema.
    pub target_namespace: Option<String>,
    /// Namespace declarations (prefix -> URI).
    pub namespaces: HashMap<NamespacePrefix, NamespaceUri>,
    /// Parsed `xs:import` references.
    pub imports: Vec<SchemaImport>,
    /// Parsed `xs:include` references.
    pub includes: Vec<SchemaInclude>,
    /// Parsed `link:linkbaseRef` entries.
    pub linkbase_refs: Vec<LinkbaseRef>,
    /// Parsed `link:roleType` definitions.
    pub role_types: Vec<RoleType>,
    /// Parsed `link:arcroleType` definitions.
    pub arcrole_types: Vec<ArcroleType>,
    /// Parsed elements (`xs:element`) in this schema.
    pub elements: Vec<Element>,
    /// Parsed simple type definitions (`xs:simpleType`) in this schema.
    pub simple_types: Vec<SimpleType>,
    /// Parsed complex type definitions (`xs:complexType`) in this schema.
    pub complex_types: Vec<ComplexType>,
}

/// The parser for XBRL schema documents.
pub struct SchemaParser<R> {
    /// Path of the currently parsed schema if read from a file. Used for error
    /// reporting.
    path: Option<PathBuf>,
    /// The XML reader for the schema document.
    reader: Reader<R>,
}

impl SchemaParser<BufReader<File>> {
    pub fn from_file(path: &Path) -> Result<Self, XbrlError> {
        let file = File::open(&path).map_err(|err| XbrlError::FileOpen {
            path: path.to_path_buf(),
            context: "opening file".to_string(),
            source: err,
        })?;
        let mut reader = Reader::from_reader(BufReader::new(file));

        reader.config_mut().trim_text_start = true;
        reader.config_mut().trim_text_end = true;

        Ok(Self {
            path: Some(path.to_path_buf()),
            reader,
        })
    }
}

impl<R: BufRead> SchemaParser<R> {
    /// Creates a new `SchemaParser` with the given XML reader and file path.
    pub fn new(reader: Reader<R>) -> Self {
        Self { path: None, reader }
    }

    pub fn from_reader(reader: R) -> Self {
        let mut reader = Reader::from_reader(reader);

        reader.config_mut().trim_text_start = true;
        reader.config_mut().trim_text_end = true;

        Self { path: None, reader }
    }

    /// Parses an XBRL schema document from the reader. Path is used for error
    /// reporting.
    pub fn parse_schema(&mut self) -> Result<RawSchema, XbrlError> {
        let mut schema = RawSchema {
            target_namespace: None,
            namespaces: HashMap::new(),
            imports: vec![],
            includes: vec![],
            linkbase_refs: vec![],
            role_types: vec![],
            arcrole_types: vec![],
            elements: vec![],
            simple_types: vec![],
            complex_types: vec![],
        };

        let mut has_schema_root = false;
        let mut buf = Vec::new();

        loop {
            match self.reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref event)) => {
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
                        b"linkbaseRef" => {
                            let linkbase_ref = self.parse_linkbase_ref(attributes)?;
                            schema.linkbase_refs.push(linkbase_ref);
                        }
                        b"roleType" => {
                            let role_type = self.parse_role_type(attributes)?;
                            schema.role_types.push(role_type);
                        }
                        b"arcroleType" => {
                            let arcrole_type = self.parse_arcrole_type(attributes)?;
                            schema.arcrole_types.push(arcrole_type);
                        }
                        b"element" => {
                            let element = self.parse_element(event, true)?;
                            schema.elements.push(element);
                        }
                        b"simpleType" => {
                            let simple_type = self.parse_simple_type(attributes)?;
                            schema.simple_types.push(simple_type);
                        }
                        b"complexType" => {
                            let complex_type = self.parse_complex_type(event)?;
                            schema.complex_types.push(complex_type);
                        }
                        b"annotation" => self.parse_annotation(&mut schema)?,
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
                Ok(Event::Empty(ref event)) => {
                    let event_name = event.name();
                    let local_name = event_name.local_name();
                    let attributes = event.attributes();

                    match local_name.as_ref() {
                        b"import" => self.parse_import(&mut schema, attributes)?,
                        b"include" => self.parse_include(&mut schema, attributes)?,
                        b"linkbaseRef" => {
                            let linkbase_ref = self.parse_linkbase_ref(attributes)?;
                            schema.linkbase_refs.push(linkbase_ref);
                        }
                        b"element" => {
                            let element = self.parse_element(event, false)?;
                            schema.elements.push(element);
                        }
                        b"complexType" => {
                            let complex_type = self.parse_complex_type(event)?;
                            schema.complex_types.push(complex_type);
                        }
                        b"annotation" => self.parse_annotation(&mut schema)?,
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
                Ok(Event::End(_)) => {}
                Ok(Event::Text(_)) => {
                    // TODO: parse `xs:annotation` and `xs:documentation`
                }
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

        if !has_schema_root {
            return Err(XbrlError::InvalidSchemaDocument {
                path: self.path.clone(),
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
        for attribute in attributes {
            let attribute = attribute.map_err(|err| XbrlError::XmlParse {
                path: self.path.clone(),
                position: self.reader.buffer_position(),
                element: Some("schema".to_string()),
                source: err.into(),
            })?;
            let local_name = attribute.key.local_name();
            let value = attribute.decode_and_unescape_value(self.reader.decoder())?;

            match local_name.as_ref() {
                b"targetNamespace" => {
                    schema.target_namespace = Some(value.to_string());
                }
                b"xmlns" => {
                    schema.namespaces.insert(
                        NamespacePrefix::from(str::from_utf8(local_name.as_ref())?),
                        NamespaceUri::from(value.to_string()),
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

        for attribute in attributes {
            let attribute = attribute.map_err(|err| XbrlError::XmlParse {
                path: self.path.clone(),
                position: self.reader.buffer_position(),
                element: Some("import".to_string()),
                source: err.into(),
            })?;
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

        for attribute in attributes {
            let attribute = attribute.map_err(|err| XbrlError::XmlParse {
                path: self.path.clone(),
                position: self.reader.buffer_position(),
                element: Some("include".to_string()),
                source: err.into(),
            })?;
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

    /// Parse a `link:linkbaseRef` element.
    fn parse_linkbase_ref(&mut self, attributes: Attributes) -> Result<LinkbaseRef, XbrlError> {
        let mut href = String::new();
        let mut role = None;
        let mut arcrole = None;
        let mut link_type = None;

        for attribute in attributes {
            let attribute = attribute.map_err(|err| XbrlError::XmlParse {
                path: self.path.clone(),
                position: self.reader.buffer_position(),
                element: Some("include".to_string()),
                source: err.into(),
            })?;
            let local_name = attribute.key.local_name();
            let value = attribute.decode_and_unescape_value(self.reader.decoder())?;

            match local_name.as_ref() {
                b"href" => {
                    href = value.to_string();
                }
                b"role" => {
                    role = Some(value.to_string());
                }
                b"arcrole" => {
                    arcrole = Some(value.to_string());
                }
                b"type" => {
                    link_type = Some(value.to_string());
                }
                _ => {}
            }
        }

        Ok(LinkbaseRef {
            href,
            role,
            arcrole,
            link_type,
        })
    }

    /// Parses a `link:roleType` element.
    fn parse_role_type(&mut self, attributes: Attributes) -> Result<RoleType, XbrlError> {
        let mut id = String::new();
        let mut role_uri = String::new();

        for attribute in attributes {
            let attribute = attribute.map_err(|err| XbrlError::XmlParse {
                path: self.path.clone(),
                position: self.reader.buffer_position(),
                element: Some("roleType".to_string()),
                source: err.into(),
            })?;
            let local_name = attribute.key.local_name();
            let value = attribute.decode_and_unescape_value(self.reader.decoder())?;
            match local_name.as_ref() {
                b"id" => id = value.into_owned(),
                b"roleURI" => role_uri = value.into_owned(),
                _ => {}
            }
        }

        let mut definition = None;
        let mut used_on = Vec::new();
        let mut buf = Vec::new();

        loop {
            match self.reader.read_event_into(&mut buf)? {
                Event::Start(ref event) => match event.local_name().as_ref() {
                    b"definition" => {
                        if let Event::Text(text) = self.reader.read_event_into(&mut buf)? {
                            definition = Some(
                                text.xml_content()
                                    .map_err(quick_xml::Error::from)?
                                    .into_owned(),
                            );
                        }
                    }
                    b"usedOn" => {
                        if let Event::Text(text) = self.reader.read_event_into(&mut buf)? {
                            used_on.push(
                                text.xml_content()
                                    .map_err(quick_xml::Error::from)?
                                    .into_owned(),
                            );
                        }
                    }
                    _ => self.skip_element()?,
                },
                Event::End(ref event) if event.local_name().as_ref() == b"roleType" => break,
                _ => {}
            }
            buf.clear();
        }

        Ok(RoleType {
            id,
            role_uri,
            definition,
            used_on,
        })
    }

    /// Parses a `link:arcroleType` element.
    fn parse_arcrole_type(&mut self, attributes: Attributes) -> Result<ArcroleType, XbrlError> {
        let mut id = String::new();
        let mut arcrole_uri = String::new();
        let mut cycles_allowed = None;

        for attribute in attributes {
            let attribute = attribute.map_err(|err| XbrlError::XmlParse {
                path: self.path.clone(),
                position: self.reader.buffer_position(),
                element: Some("arcroleType".to_string()),
                source: err.into(),
            })?;
            let local_name = attribute.key.local_name();
            let value = attribute.decode_and_unescape_value(self.reader.decoder())?;
            match local_name.as_ref() {
                b"id" => id = value.into_owned(),
                b"arcroleURI" => arcrole_uri = value.into_owned(),
                b"cyclesAllowed" => cycles_allowed = Some(value.parse::<CyclesAllowed>()?),
                _ => {}
            }
        }

        let mut definition = None;
        let mut used_on = Vec::new();
        let mut buf = Vec::new();

        loop {
            match self.reader.read_event_into(&mut buf)? {
                Event::Start(ref event) => match event.local_name().as_ref() {
                    b"definition" => {
                        if let Event::Text(text) = self.reader.read_event_into(&mut buf)? {
                            definition = Some(
                                text.xml_content()
                                    .map_err(quick_xml::Error::from)?
                                    .into_owned(),
                            );
                        }
                    }
                    b"usedOn" => {
                        if let Event::Text(text) = self.reader.read_event_into(&mut buf)? {
                            used_on.push(
                                text.xml_content()
                                    .map_err(quick_xml::Error::from)?
                                    .into_owned(),
                            );
                        }
                    }
                    _ => self.skip_element()?,
                },
                Event::End(ref event) if event.local_name().as_ref() == b"arcroleType" => break,
                _ => {}
            }
            buf.clear();
        }

        Ok(ArcroleType {
            id,
            arcrole_uri,
            definition,
            used_on,
            cycles_allowed,
        })
    }

    /// Parses an `xs:annotation` element, including its child `xs:appinfo`.
    fn parse_annotation(&mut self, schema: &mut RawSchema) -> Result<(), XbrlError> {
        let mut buf = Vec::new();

        loop {
            match self.reader.read_event_into(&mut buf)? {
                Event::Start(event) => match event.local_name().as_ref() {
                    b"appinfo" => self.parse_appinfo(schema)?,
                    _ => self.skip_element()?,
                },
                Event::End(event) if event.local_name().as_ref() == b"annotation" => break,
                _ => {}
            }
        }

        Ok(())
    }

    /// Parses an `xs:appinfo` element, including its child elements like
    /// `link:roleType`, `link:arcroleType`, and `link:linkbaseRef`.
    fn parse_appinfo(&mut self, schema: &mut RawSchema) -> Result<(), XbrlError> {
        let mut buf = Vec::new();

        loop {
            match self.reader.read_event_into(&mut buf)? {
                Event::Start(event) | Event::Empty(event) => match event.local_name().as_ref() {
                    b"linkbaseRef" => {
                        let linkbase_ref = self.parse_linkbase_ref(event.attributes())?;
                        schema.linkbase_refs.push(linkbase_ref);
                    }
                    b"roleType" => {
                        let role_type = self.parse_role_type(event.attributes())?;
                        schema.role_types.push(role_type);
                    }
                    b"arcroleType" => {
                        let arcrole_type = self.parse_arcrole_type(event.attributes())?;
                        schema.arcrole_types.push(arcrole_type);
                    }
                    _ => {}
                },
                Event::End(event) if event.local_name().as_ref() == b"appinfo" => break,
                _ => {}
            }
        }

        Ok(())
    }

    /// Skips the current element, including all nested child elements.
    /// The reader must be positioned at the `Start` or `Empty` event of the element.
    fn skip_element(&mut self) -> Result<(), XbrlError> {
        let mut buf = Vec::new();
        let mut depth = 0;

        loop {
            match self.reader.read_event_into(&mut buf)? {
                Event::Start(_) => {
                    // entering a nested element
                    depth += 1;
                }
                Event::End(_) => {
                    if depth == 0 {
                        // matched the original element's end
                        break;
                    } else {
                        depth -= 1;
                    }
                }
                Event::Empty(_) => {
                    // empty element counts as start+end, so no depth change needed
                }
                Event::Eof => {
                    return Err(XbrlError::ParseError {
                        expected: "end tag while skipping element",
                        value: "".to_string(),
                    });
                }
                _ => {}
            }

            buf.clear();
        }

        Ok(())
    }

    /// Parses an `xs:element` element, which can be either an item or a tuple
    /// depending on the `substitutionGroup` attribute. If `has_children` is
    /// true, also looks for an inline `xs:complexType` child.
    fn parse_element(
        &mut self,
        start: &BytesStart,
        has_children: bool,
    ) -> Result<Element, XbrlError> {
        let mut element = self.parse_item_element(start)?;

        if has_children {
            self.parse_element_children(start, &mut element)?;
        }

        Ok(element)
    }

    /// Parses an `xs:element` element with `substitutionGroup="xbrli:item"`.
    fn parse_item_element(&mut self, start: &BytesStart) -> Result<Element, XbrlError> {
        let mut name = None;
        let mut id = None;
        let mut type_name = None;
        let mut substitution_group = None;
        let mut is_abstract = false;
        let mut is_nillable = false;
        let mut period_type = None;
        let mut balance = None;

        for attribute in start.attributes() {
            let attribute = attribute.map_err(|err| XbrlError::XmlParse {
                path: self.path.clone(),
                position: self.reader.buffer_position(),
                element: Some("element".to_string()),
                source: err.into(),
            })?;
            let qname = attribute.key;
            let local_name = qname.local_name();
            let value = attribute.decode_and_unescape_value(self.reader.decoder())?;

            match local_name.as_ref() {
                b"name" => name = Some(value.to_string()),
                b"id" => id = Some(value.to_string()),
                b"type" => type_name = Some(xml::parse_qname(&value)),
                b"substitutionGroup" => substitution_group = Some(xml::parse_qname(&value)),
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
            complex_type: None,
        };

        Ok(element)
    }

    /// Parses child elements of an `xs:element`, looking for an inline
    /// `xs:complexType`. Complex types are allowed in both tuple and item
    /// elements.
    fn parse_element_children(
        &mut self,
        start: &BytesStart,
        element: &mut Element,
    ) -> Result<(), XbrlError> {
        let mut buf = Vec::new();

        loop {
            match self.reader.read_event_into(&mut buf)? {
                Event::Start(ref event) => {
                    if event.local_name().as_ref() == b"complexType" {
                        element.complex_type = Some(self.parse_complex_type(event)?);
                    }
                }
                Event::End(ref event) if event.name().as_ref() == start.name().as_ref() => {
                    break;
                }
                Event::Eof => break,
                _ => {}
            }
            buf.clear();
        }

        Ok(())
    }

    /// Parses an `xs:simpleType` element.
    fn parse_simple_type(&mut self, attributes: Attributes) -> Result<SimpleType, XbrlError> {
        let mut name = None;

        for attribute in attributes {
            let attribute = attribute.map_err(|err| XbrlError::XmlParse {
                path: self.path.clone(),
                position: self.reader.buffer_position(),
                element: Some("simpleType".to_string()),
                source: err.into(),
            })?;
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
                    let local_name = event.local_name();

                    match local_name.as_ref() {
                        b"restriction" => {
                            for attribute in event.attributes() {
                                let attribute = attribute.map_err(|err| XbrlError::XmlParse {
                                    path: self.path.clone(),
                                    position: self.reader.buffer_position(),
                                    element: Some("restriction".to_string()),
                                    source: err.into(),
                                })?;

                                if attribute.key.as_ref() == b"base" {
                                    let value = attribute
                                        .decode_and_unescape_value(self.reader.decoder())?;
                                    base = Some(xml::parse_qname(&value));
                                }
                            }
                        }
                        b"enumeration" => {
                            for attribute in event.attributes() {
                                let attribute = attribute.map_err(|err| XbrlError::XmlParse {
                                    path: self.path.clone(),
                                    position: self.reader.buffer_position(),
                                    element: Some("enumeration".to_string()),
                                    source: err.into(),
                                })?;

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
                    let local_name = event.local_name();

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
    fn parse_complex_type(&mut self, start: &BytesStart) -> Result<ComplexType, XbrlError> {
        let mut buf = Vec::new();
        let mut complex_type = ComplexType {
            name: None,
            base: None,
            derivation: None,
            compositor: None,
            attributes: Vec::new(),
            children: Vec::new(),
        };

        for attribute in start.attributes() {
            let attribute = attribute.map_err(|err| XbrlError::XmlParse {
                path: self.path.clone(),
                position: self.reader.buffer_position(),
                element: Some("complexType".to_string()),
                source: err.into(),
            })?;

            if attribute.key.local_name().as_ref() == b"name" {
                let value = attribute.decode_and_unescape_value(self.reader.decoder())?;
                complex_type.name = Some(value.to_string());
            }
        }

        loop {
            match self.reader.read_event_into(&mut buf)? {
                Event::Start(ref event) | Event::Empty(ref event) => {
                    match event.local_name().as_ref() {
                        b"simpleContent" => {
                            self.parse_simple_content(&mut complex_type)?;
                        }
                        b"complexContent" => {
                            self.parse_complex_content(&mut complex_type)?;
                        }
                        b"sequence" => {
                            let tuple_children = self.parse_sequence()?;
                            complex_type.children.extend(tuple_children);
                            complex_type.compositor = Some(Compositor::Sequence);
                        }
                        b"choice" => {
                            let tuple_children = self.parse_sequence()?;
                            complex_type.children.extend(tuple_children);
                            complex_type.compositor = Some(Compositor::Choice);
                        }
                        b"attribute" => {
                            let attribute = self.parse_attribute(event)?;
                            complex_type.attributes.push(attribute);
                        }
                        b"restriction" => {
                            return Err(XbrlError::InvalidSchemaDocument {
                                path: self.path.clone(),
                                reason: "restriction is only allowed inside simpleContent or complexContent"
                                    .to_string(),
                            });
                        }
                        b"extension" => {
                            return Err(XbrlError::InvalidSchemaDocument {
                                path: self.path.clone(),
                                reason: "extension is only allowed inside simpleContent or complexContent"
                                    .to_string(),
                            });
                        }

                        _ => {
                            // ignore unknown tags inside complexType
                        }
                    }
                }
                Event::End(ref event) if event.name().as_ref() == start.name().as_ref() => {
                    break;
                }
                Event::Eof => break,
                _ => {}
            }

            buf.clear();
        }

        Ok(complex_type)
    }

    /// Parses an `xs:sequence` element.
    fn parse_sequence(&mut self) -> Result<Vec<RawTupleChild>, XbrlError> {
        let mut buf = Vec::new();
        let mut children = Vec::new();

        loop {
            match self.reader.read_event_into(&mut buf)? {
                Event::Start(ref event) | Event::Empty(ref event)
                    if event.local_name().as_ref() == b"element" =>
                {
                    let mut ref_name = None;
                    let mut min_occurs = 1;
                    let mut max_occurs = Some(1);

                    for attribute in event.attributes() {
                        let attribute = attribute.map_err(|err| XbrlError::XmlParse {
                            path: self.path.clone(),
                            position: self.reader.buffer_position(),
                            element: Some("sequence".to_string()),
                            source: err.into(),
                        })?;
                        let local_name = attribute.key.local_name();
                        let value = attribute.decode_and_unescape_value(self.reader.decoder())?;

                        match local_name.as_ref() {
                            b"ref" => ref_name = Some(xml::parse_qname(&value)),
                            b"minOccurs" => min_occurs = xml::parse_u32(&value)?,
                            b"maxOccurs" => {
                                max_occurs = if value == "unbounded" {
                                    None
                                } else {
                                    Some(xml::parse_u32(&value)?)
                                }
                            }
                            _ => {}
                        }
                    }

                    if let Some(name) = ref_name {
                        children.push(RawTupleChild {
                            name,
                            min_occurs,
                            max_occurs,
                        });
                    }
                }

                Event::End(ref event)
                    if matches!(event.local_name().as_ref(), b"sequence" | b"choice") =>
                {
                    break;
                }
                Event::Eof => break,
                _ => {}
            }

            buf.clear();
        }

        Ok(children)
    }

    /// Parses an `xs:simpleContent` element.
    fn parse_simple_content(&mut self, complex_type: &mut ComplexType) -> Result<(), XbrlError> {
        let mut buf = Vec::new();

        loop {
            match self.reader.read_event_into(&mut buf)? {
                Event::Start(ref event) => match event.local_name().as_ref() {
                    b"extension" => {
                        self.parse_derivation(event, complex_type)?;
                        complex_type.derivation = Some(DerivationKind::Extension);
                    }
                    b"restriction" => {
                        self.parse_derivation(event, complex_type)?;
                        complex_type.derivation = Some(DerivationKind::Restriction);
                    }
                    _ => {}
                },

                Event::End(ref event) if event.local_name().as_ref() == b"simpleContent" => {
                    break;
                }

                Event::Eof => break,
                _ => {}
            }

            buf.clear();
        }

        Ok(())
    }

    /// Parses an `xs:complexContent` element.
    fn parse_complex_content(&mut self, complex_type: &mut ComplexType) -> Result<(), XbrlError> {
        let mut buf = Vec::new();

        loop {
            match self.reader.read_event_into(&mut buf)? {
                Event::Start(ref event) => match event.local_name().as_ref() {
                    b"extension" => {
                        self.parse_derivation(event, complex_type)?;
                        complex_type.derivation = Some(DerivationKind::Extension);
                    }
                    b"restriction" => {
                        self.parse_derivation(event, complex_type)?;
                        complex_type.derivation = Some(DerivationKind::Restriction);
                    }
                    _ => {}
                },

                Event::End(ref event) if event.local_name().as_ref() == b"complexContent" => {
                    break;
                }

                Event::Eof => break,
                _ => {}
            }

            buf.clear();
        }

        Ok(())
    }

    /// Parses an `xs:extension` or `xs:restriction` element inside
    /// `xs:simpleContent` or `xs:complexContent`.
    fn parse_derivation(
        &mut self,
        start: &BytesStart,
        complex_type: &mut ComplexType,
    ) -> Result<(), XbrlError> {
        let mut buf = Vec::new();

        // Parse base attribute
        for attribute in start.attributes() {
            let attribute = attribute.map_err(|err| XbrlError::XmlParse {
                path: self.path.clone(),
                position: self.reader.buffer_position(),
                element: Some("extension or restriction".to_string()),
                source: err.into(),
            })?;

            if attribute.key.local_name().as_ref() == b"base" {
                let value = attribute.decode_and_unescape_value(self.reader.decoder())?;
                complex_type.base = Some(xml::parse_qname(&value));
            }
        }

        loop {
            match self.reader.read_event_into(&mut buf)? {
                Event::Start(ref event) | Event::Empty(ref event) => {
                    match event.local_name().as_ref() {
                        b"attribute" => {
                            let attribute = self.parse_attribute(event)?;
                            complex_type.attributes.push(attribute);
                        }
                        b"sequence" => {
                            let children = self.parse_sequence()?;
                            complex_type.children.extend(children);
                            complex_type.compositor = Some(Compositor::Sequence);
                        }
                        b"choice" => {
                            let children = self.parse_sequence()?;
                            complex_type.children.extend(children);
                            complex_type.compositor = Some(Compositor::Choice);
                        }
                        _ => {}
                    }
                }

                Event::End(ref event) if event.name().as_ref() == start.name().as_ref() => {
                    break;
                }

                Event::Eof => break,
                _ => {}
            }

            buf.clear();
        }

        Ok(())
    }

    /// Parses an `xs:attribute` element inside `xs:simpleContent`.
    fn parse_attribute(&self, start: &BytesStart) -> Result<AttributeUse, XbrlError> {
        let mut ref_name = String::new();
        let mut required = false;

        for attribute in start.attributes() {
            let attribute = attribute.map_err(|err| XbrlError::XmlParse {
                path: self.path.clone(),
                position: self.reader.buffer_position(),
                element: Some("extension or restriction attribute".to_string()),
                source: err.into(),
            })?;

            match attribute.key.local_name().as_ref() {
                b"ref" => {
                    let value = attribute.decode_and_unescape_value(self.reader.decoder())?;
                    ref_name = value.to_string();
                }
                b"use" => {
                    let value = attribute.decode_and_unescape_value(self.reader.decoder())?;

                    if value.as_ref() == "required" {
                        required = true;
                    }
                }
                _ => {}
            }
        }

        Ok(AttributeUse { ref_name, required })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_matches::assert_matches;

    #[test]
    fn test_parse_schema_root_invalid() {
        let xml = r#"<root/>"#;
        let mut parser = SchemaParser::from_reader(xml.as_bytes());
        let result = parser.parse_schema();

        assert_matches!(result, Err(XbrlError::InvalidSchemaDocument { reason, .. }) if reason == "root is not allowed in taxonomy schemas");
    }

    #[test]
    fn test_parse_schema_root_missing() {
        let xml = r#"<xsd:import
                            xmlns:xsd="http://www.w3.org/2001/XMLSchema"
                            namespace="http://www.xbrl.org/2003/instance"
                            schemaLocation="http://www.xbrl.org/2003/xbrl-instance-2003-12-31.xsd" />"#;
        let mut parser = SchemaParser::from_reader(xml.as_bytes());
        let result = parser.parse_schema();

        assert_matches!(result, Err(XbrlError::InvalidSchemaDocument { reason, .. }) if reason == "missing <schema> root element");
    }

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
        let mut parser = SchemaParser::from_reader(xml.as_bytes());
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
        let mut parser = SchemaParser::from_reader(xml.as_bytes());
        let schema = parser.parse_schema().unwrap();

        let includes = &schema.includes;
        assert!(includes.len() == 1);
        let include = &includes[0];
        assert_eq!(include.schema_location, "test.xsd");
    }

    #[test]
    fn test_parse_linkbase_ref() {
        let xml = r#"<xs:schema
                            xmlns:xs="http://www.w3.org/2001/XMLSchema"
                            xmlns:link="http://www.xbrl.org/2003/linkbase"
                            xmlns:xlink="http://www.w3.org/1999/xlink">
                            <xs:annotation>
                                <xs:appinfo>
                                    <link:linkbaseRef
                                        xlink:type="simple"
                                        xlink:href="de-gaap-ci_pre.xml"
                                        xlink:role="http://www.w3.org/1999/xlink/properties/linkbase"
                                        xlink:arcrole="http://www.w3.org/1999/xlink/properties/linkbase" />
                                </xs:appinfo>
                            </xs:annotation>
                        </xs:schema>"#;
        let mut parser = SchemaParser::from_reader(xml.as_bytes());
        let schema = parser.parse_schema().unwrap();

        assert_eq!(schema.linkbase_refs.len(), 1);
        let linkbase_ref = &schema.linkbase_refs[0];
        assert_eq!(linkbase_ref.href, "de-gaap-ci_pre.xml");
        assert_eq!(
            linkbase_ref.role.as_deref(),
            Some("http://www.w3.org/1999/xlink/properties/linkbase")
        );
        assert_eq!(
            linkbase_ref.arcrole.as_deref(),
            Some("http://www.w3.org/1999/xlink/properties/linkbase")
        );
    }

    #[test]
    fn test_parse_role_type() {
        let xml = r#"<xs:schema
                            xmlns:xs="http://www.w3.org/2001/XMLSchema"
                            xmlns:link="http://www.xbrl.org/2003/linkbase"
                            xmlns:xlink="http://www.w3.org/1999/xlink">
                            <xs:annotation>
                                <xs:appinfo>
                                    <link:roleType
                                        roleURI="http://www.xbrl.de/taxonomies/de-gaap-ci/role/balanceSheet"
                                        id="balanceSheet">
                                        <link:definition>Balance Sheet</link:definition>
                                        <link:usedOn>link:presentationLink</link:usedOn>
                                    </link:roleType>
                                </xs:appinfo>
                            </xs:annotation>
                        </xs:schema>"#;
        let mut parser = SchemaParser::from_reader(xml.as_bytes());
        let schema = parser.parse_schema().unwrap();

        assert_eq!(schema.role_types.len(), 1);
        let role_type = &schema.role_types[0];
        assert_eq!(
            role_type.role_uri,
            "http://www.xbrl.de/taxonomies/de-gaap-ci/role/balanceSheet"
        );
        assert_eq!(role_type.id, "balanceSheet");
    }

    #[test]
    fn test_parse_arcrole_type() {
        let xml = r#"<xs:schema
                            xmlns:xs="http://www.w3.org/2001/XMLSchema"
                            xmlns:link="http://www.xbrl.org/2003/linkbase"
                            xmlns:xlink="http://www.w3.org/1999/xlink">
                            <xs:annotation>
                                <xs:appinfo>
                                    <link:arcroleType
                                        arcroleURI="http://www.xbrl.de/taxonomies/de-gaap-ci/arcrole/parent-child"
                                        cyclesAllowed="undirected"
                                        id="parentChild">
                                        <link:definition>Parent-child relationship</link:definition>
                                        <link:usedOn>link:definitionArc</link:usedOn>
                                    </link:arcroleType>
                                </xs:appinfo>
                            </xs:annotation>
                        </xs:schema>"#;
        let mut parser = SchemaParser::from_reader(xml.as_bytes());
        let schema = parser.parse_schema().unwrap();

        assert_eq!(schema.arcrole_types.len(), 1);
        let arcrole_type = &schema.arcrole_types[0];
        assert_eq!(
            arcrole_type.arcrole_uri,
            "http://www.xbrl.de/taxonomies/de-gaap-ci/arcrole/parent-child"
        );
        assert_eq!(arcrole_type.cycles_allowed, Some(CyclesAllowed::Undirected));
        assert_eq!(arcrole_type.id, "parentChild");
    }

    #[test]
    fn test_parse_item_element() {
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
        let mut parser = SchemaParser::from_reader(xml.as_bytes());
        let schema = parser.parse_schema().unwrap();

        assert_eq!(schema.elements.len(), 1);
        let element = &schema.elements[0];
        assert_eq!(
            *element,
            Element {
                name: "Revenue".to_string(),
                id: Some("Revenue".to_string()),
                type_name: Some(QName {
                    prefix: Some(NamespacePrefix::from("xbrli")),
                    local_name: "monetaryItemType".to_string(),
                }),
                substitution_group: Some(QName {
                    prefix: Some(NamespacePrefix::from("xbrli")),
                    local_name: "item".to_string(),
                }),
                is_nillable: true,
                is_abstract: false,
                period_type: Some(PeriodType::Duration),
                balance: None,
                complex_type: None,
            }
        );
    }

    // Test parsing a complexType inside a tuple element.
    #[test]
    fn test_parse_tuple_element_with_sequence() {
        let xml = r#"<xsd:schema xmlns:xs="http://www.w3.org/2001/XMLSchema"
                            targetNamespace="http://example.com"
                            xmlns="http://example.com"
                            xmlns:xbrli="http://www.xbrl.org/2003/instance">
                            <xs:element name="address" substitutionGroup="xbrli:tuple">
                                <xs:complexType>
                                    <xs:sequence>
                                        <xs:element ref="my:city" />
                                        <xs:element ref="my:country" minOccurs="0" />
                                    </xs:sequence>
                                </xs:complexType>
                            </xs:element>
                        </xsd:schema>"#;
        let mut parser = SchemaParser::from_reader(xml.as_bytes());
        let mut schema = parser.parse_schema().unwrap();

        assert_eq!(schema.elements.len(), 1);
        let element = schema.elements.remove(0);
        assert_eq!(
            element,
            Element {
                name: "address".to_string(),
                id: None,
                type_name: None,
                substitution_group: Some(QName {
                    prefix: Some(NamespacePrefix::from("xbrli")),
                    local_name: "tuple".to_string(),
                }),
                is_nillable: false,
                is_abstract: false,
                period_type: None,
                balance: None,
                complex_type: Some(ComplexType {
                    name: None,
                    base: None,
                    derivation: None,
                    compositor: Some(Compositor::Sequence),
                    attributes: vec![],
                    children: vec![
                        RawTupleChild {
                            name: QName {
                                prefix: Some(NamespacePrefix::from("my")),
                                local_name: "city".to_string(),
                            },
                            min_occurs: 1,
                            max_occurs: Some(1),
                        },
                        RawTupleChild {
                            name: QName {
                                prefix: Some(NamespacePrefix::from("my")),
                                local_name: "country".to_string(),
                            },
                            min_occurs: 0,
                            max_occurs: Some(1),
                        },
                    ],
                }),
            }
        );
    }

    #[test]
    fn test_parse_tuple_element_min_max() {
        let xml = r#"<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema"
                            targetNamespace="http://example.com"
                            xmlns="http://example.com"
                            xmlns:xbrli="http://www.xbrl.org/2003/instance">
                            <xs:element name="address" substitutionGroup="xbrli:tuple">
                                <xs:complexType>
                                    <xs:sequence>
                                        <xs:element ref="my:itemA" minOccurs="2" maxOccurs="2" />
                                        <xs:element ref="my:itemB" minOccurs="0" maxOccurs="unbounded" />
                                    </xs:sequence>
                                </xs:complexType>
                            </xs:element>
                        </xs:schema>"#;
        let mut parser = SchemaParser::from_reader(xml.as_bytes());
        let mut schema = parser.parse_schema().unwrap();

        assert_eq!(schema.elements.len(), 1);
        let element = schema.elements.remove(0);
        assert_eq!(
            element,
            Element {
                name: "address".to_string(),
                id: None,
                type_name: None,
                substitution_group: Some(QName {
                    prefix: Some(NamespacePrefix::from("xbrli")),
                    local_name: "tuple".to_string(),
                }),
                is_nillable: false,
                is_abstract: false,
                period_type: None,
                balance: None,
                complex_type: Some(ComplexType {
                    name: None,
                    base: None,
                    derivation: None,
                    compositor: Some(Compositor::Sequence),
                    attributes: vec![],
                    children: vec![
                        RawTupleChild {
                            name: QName {
                                prefix: Some(NamespacePrefix::from("my")),
                                local_name: "itemA".to_string(),
                            },
                            min_occurs: 2,
                            max_occurs: Some(2),
                        },
                        RawTupleChild {
                            name: QName {
                                prefix: Some(NamespacePrefix::from("my")),
                                local_name: "itemB".to_string(),
                            },
                            min_occurs: 0,
                            max_occurs: None,
                        },
                    ],
                }),
            }
        );
    }

    #[test]
    fn test_parse_tuple_element_with_choice() {
        let xml = r#"<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema"
                            targetNamespace="http://example.com"
                            xmlns="http://example.com"
                            xmlns:xbrli="http://www.xbrl.org/2003/instance">
                            <xs:element name="MyTuple" substitutionGroup="xbrli:tuple"
                                xmlns:xs="http://www.w3.org/2001/XMLSchema">
                                <xs:complexType>
                                    <xs:choice>
                                        <xs:element ref="my:optA" />
                                        <xs:element ref="my:optB" />
                                    </xs:choice>
                                </xs:complexType>
                            </xs:element>
                        </xs:schema>"#;
        let mut parser = SchemaParser::from_reader(xml.as_bytes());
        let mut schema = parser.parse_schema().unwrap();

        assert_eq!(schema.elements.len(), 1);
        let element = schema.elements.remove(0);
        assert_eq!(
            element,
            Element {
                name: "MyTuple".to_string(),
                id: None,
                type_name: None,
                substitution_group: Some(QName {
                    prefix: Some(NamespacePrefix::from("xbrli")),
                    local_name: "tuple".to_string(),
                }),
                is_nillable: false,
                is_abstract: false,
                period_type: None,
                balance: None,
                complex_type: Some(ComplexType {
                    name: None,
                    base: None,
                    derivation: None,
                    compositor: Some(Compositor::Choice),
                    attributes: vec![],
                    children: vec![
                        RawTupleChild {
                            name: QName {
                                prefix: Some(NamespacePrefix::from("my")),
                                local_name: "optA".to_string(),
                            },
                            min_occurs: 1,
                            max_occurs: Some(1),
                        },
                        RawTupleChild {
                            name: QName {
                                prefix: Some(NamespacePrefix::from("my")),
                                local_name: "optB".to_string(),
                            },
                            min_occurs: 1,
                            max_occurs: Some(1),
                        },
                    ],
                }),
            }
        );
    }

    // XSD constraint maxLength doesn't have a specific meaning in XBRL, and
    // won't be parsed.
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
        let mut parser = SchemaParser::from_reader(xml.as_bytes());
        let schema = parser.parse_schema().unwrap();

        assert_eq!(schema.simple_types.len(), 1);
        let simple_type = &schema.simple_types[0];
        assert_eq!(
            *simple_type,
            SimpleType {
                name: Some("myStringType".to_string()),
                base: Some(QName {
                    prefix: Some(NamespacePrefix::from("xs")),
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
        let mut parser = SchemaParser::from_reader(xml.as_bytes());
        let schema = parser.parse_schema().unwrap();

        assert_eq!(schema.simple_types.len(), 1);
        let simple_type = &schema.simple_types[0];
        assert_eq!(
            *simple_type,
            SimpleType {
                name: Some("StatusType".to_string()),
                base: Some(QName {
                    prefix: Some(NamespacePrefix::from("xs")),
                    local_name: "string".to_string()
                }),
                enumerations: vec!["Open".to_string(), "Closed".to_string()],
            }
        );
    }

    #[test]
    fn test_parse_complex_type_empty() {
        let xml = r#"<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema"
                            targetNamespace="http://example.com"
                            xmlns="http://example.com"
                            xmlns:xbrli="http://www.xbrl.org/2003/instance">
                            <xs:complexType name="emptyType"
                                xmlns:xs="http://www.w3.org/2001/XMLSchema">
                        </xs:complexType>
                        </xs:schema>"#;
        let mut parser = SchemaParser::from_reader(xml.as_bytes());
        let schema = parser.parse_schema().unwrap();

        assert_eq!(schema.complex_types.len(), 1);
        let complex_type = &schema.complex_types[0];
        assert_eq!(
            complex_type,
            &ComplexType {
                name: Some("emptyType".to_string()),
                base: None,
                derivation: None,
                compositor: None,
                attributes: vec![],
                children: vec![],
            }
        );
    }

    #[test]
    fn test_parse_complex_type_invalid_restriction() {
        let xml = r#"<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema"
                            targetNamespace="http://example.com"
                            xmlns="http://example.com"
                            xmlns:xbrli="http://www.xbrl.org/2003/instance">
                            <xs:complexType xmlns:xs="http://www.w3.org/2001/XMLSchema">
                                <xs:restriction base="xbrli:decimalItemType" />
                            </xs:complexType>
                        </xs:schema>"#;
        let mut parser = SchemaParser::from_reader(xml.as_bytes());
        let res = parser.parse_schema();

        assert_matches!(res, Err(XbrlError::InvalidSchemaDocument { .. }));
    }

    #[test]
    fn test_parse_complex_type_invalid_extension() {
        let xml = r#"<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema"
                            targetNamespace="http://example.com"
                            xmlns="http://example.com"
                            xmlns:xbrli="http://www.xbrl.org/2003/instance">
                            <xs:complexType xmlns:xs="http://www.w3.org/2001/XMLSchema">
                                <xs:extension base="xbrli:decimalItemType" />
                            </xs:complexType>
                        </xs:schema>"#;
        let mut parser = SchemaParser::from_reader(xml.as_bytes());
        let res = parser.parse_schema();

        assert_matches!(res, Err(XbrlError::InvalidSchemaDocument { .. }));
    }

    #[test]
    fn test_parse_complex_type_with_simple_content_and_extension() {
        let xml = r#"<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema"
                            targetNamespace="http://example.com"
                            xmlns="http://example.com"
                            xmlns:xbrli="http://www.xbrl.org/2003/instance">
                            <xs:complexType name="monetaryItemType">
                                <xs:simpleContent>
                                    <xs:extension base="xbrli:decimalItemType">
                                        <xs:attribute ref="xbrli:unitRef" use="required" />
                                        <xs:attribute ref="xbrli:decimals" />
                                    </xs:extension>
                                </xs:simpleContent>
                            </xs:complexType>
                        </xs:schema>"#;
        let mut parser = SchemaParser::from_reader(xml.as_bytes());
        let schema = parser.parse_schema().unwrap();

        assert_eq!(schema.complex_types.len(), 1);
        let complex_type = &schema.complex_types[0];
        assert_eq!(
            complex_type,
            &ComplexType {
                name: Some("monetaryItemType".to_string()),
                base: Some(QName {
                    prefix: Some(NamespacePrefix::from("xbrli")),
                    local_name: "decimalItemType".to_string(),
                }),
                derivation: Some(DerivationKind::Extension),
                compositor: None,
                attributes: vec![
                    AttributeUse {
                        ref_name: "xbrli:unitRef".to_string(),
                        required: true,
                    },
                    AttributeUse {
                        ref_name: "xbrli:decimals".to_string(),
                        required: false,
                    },
                ],
                children: vec![],
            }
        );
    }

    #[test]
    fn test_parse_complex_type_with_simple_content_and_restriction() {
        let xml = r#"<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema"
                            targetNamespace="http://example.com"
                            xmlns="http://example.com"
                            xmlns:xbrli="http://www.xbrl.org/2003/instance">
                            <xs:complexType name="restrictedDecimal">
                                <xs:simpleContent>
                                    <xs:restriction base="xbrli:decimalItemType">
                                        <xs:attribute ref="xbrli:unitRef" use="required" />
                                    </xs:restriction>
                                </xs:simpleContent>
                            </xs:complexType>
                        </xs:schema>"#;
        let mut parser = SchemaParser::from_reader(xml.as_bytes());
        let schema = parser.parse_schema().unwrap();

        assert_eq!(schema.complex_types.len(), 1);
        let complex_type = &schema.complex_types[0];
        assert_eq!(
            complex_type,
            &ComplexType {
                name: Some("restrictedDecimal".to_string()),
                base: Some(QName {
                    prefix: Some(NamespacePrefix::from("xbrli")),
                    local_name: "decimalItemType".to_string(),
                }),
                derivation: Some(DerivationKind::Restriction),
                compositor: None,
                attributes: vec![AttributeUse {
                    ref_name: "xbrli:unitRef".to_string(),
                    required: true,
                },],
                children: vec![],
            }
        );
    }

    #[test]
    fn test_parse_complex_type_with_complex_content_and_extension() {
        let xml = r#"<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema"
                            targetNamespace="http://example.com"
                            xmlns="http://example.com">
                            <!-- Base complex type -->
                            <xs:complexType name="baseAccountType">
                                <xs:sequence>
                                    <xs:element ref="xs:name" />
                                </xs:sequence>
                            </xs:complexType>
                            <!-- Derived type extending the base -->
                            <xs:complexType name="extendedAccountType">
                                <xs:complexContent>
                                    <xs:extension base="baseAccountType">
                                        <xs:sequence>
                                            <xs:element ref="xs:balance" />
                                        </xs:sequence>
                                        <xs:attribute ref="currency" />
                                    </xs:extension>
                                </xs:complexContent>
                            </xs:complexType>
                        </xs:schema>"#;
        let mut parser = SchemaParser::from_reader(xml.as_bytes());
        let schema = parser.parse_schema().unwrap();

        assert_eq!(schema.complex_types.len(), 2);
        let base_type = &schema.complex_types[0];
        assert_eq!(
            base_type,
            &ComplexType {
                name: Some("baseAccountType".to_string()),
                base: None,
                derivation: None,
                compositor: Some(Compositor::Sequence),
                attributes: vec![],
                children: vec![RawTupleChild {
                    name: QName {
                        prefix: Some(NamespacePrefix::from("xs")),
                        local_name: "name".to_string(),
                    },
                    min_occurs: 1,
                    max_occurs: Some(1),
                }],
            }
        );
        let extended_type = &schema.complex_types[1];
        assert_eq!(
            extended_type,
            &ComplexType {
                name: Some("extendedAccountType".to_string()),
                base: Some(QName {
                    prefix: None,
                    local_name: "baseAccountType".to_string(),
                }),
                derivation: Some(DerivationKind::Extension),
                compositor: Some(Compositor::Sequence),
                attributes: vec![AttributeUse {
                    ref_name: "currency".to_string(),
                    required: false,
                }],
                children: vec![RawTupleChild {
                    name: QName {
                        prefix: Some(NamespacePrefix::from("xs")),
                        local_name: "balance".to_string(),
                    },
                    min_occurs: 1,
                    max_occurs: Some(1),
                }],
            }
        );
    }

    #[test]
    fn test_parse_complex_type_with_complex_content_and_restriction() {
        let xml = r#"<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema"
                            targetNamespace="http://example.com"
                            xmlns="http://example.com">
                            <!-- Base complex type -->
                            <xs:complexType name="baseAccountType">
                                <xs:sequence>
                                    <xs:element ref="xs:name" />
                                    <xs:element ref="xs:balance" />
                                </xs:sequence>
                                <xs:attribute ref="currency" />
                            </xs:complexType>
                            <!-- Restricted type -->
                            <xs:complexType name="restrictedAccountType">
                                <xs:complexContent>
                                    <xs:restriction base="baseAccountType">
                                        <xs:sequence>
                                            <xs:element ref="xs:name" />
                                        </xs:sequence>
                                        <xs:attribute ref="currency" use="required" />
                                    </xs:restriction>
                                </xs:complexContent>
                            </xs:complexType>
                        </xs:schema>"#;
        let mut parser = SchemaParser::from_reader(xml.as_bytes());
        let schema = parser.parse_schema().unwrap();

        let base_type = &schema.complex_types[0];
        assert_eq!(
            base_type,
            &ComplexType {
                name: Some("baseAccountType".to_string()),
                base: None,
                derivation: None,
                compositor: Some(Compositor::Sequence),
                attributes: vec![AttributeUse {
                    ref_name: "currency".to_string(),
                    required: false,
                }],
                children: vec![
                    RawTupleChild {
                        name: QName {
                            prefix: Some(NamespacePrefix::from("xs")),
                            local_name: "name".to_string(),
                        },
                        min_occurs: 1,
                        max_occurs: Some(1),
                    },
                    RawTupleChild {
                        name: QName {
                            prefix: Some(NamespacePrefix::from("xs")),
                            local_name: "balance".to_string(),
                        },
                        min_occurs: 1,
                        max_occurs: Some(1),
                    },
                ],
            }
        );
        let restricted_type = &schema.complex_types[1];
        assert_eq!(
            restricted_type,
            &ComplexType {
                name: Some("restrictedAccountType".to_string()),
                base: Some(QName {
                    prefix: None,
                    local_name: "baseAccountType".to_string(),
                }),
                derivation: Some(DerivationKind::Restriction),
                compositor: Some(Compositor::Sequence),
                attributes: vec![AttributeUse {
                    ref_name: "currency".to_string(),
                    required: true,
                }],
                children: vec![RawTupleChild {
                    name: QName {
                        prefix: Some(NamespacePrefix::from("xs")),
                        local_name: "name".to_string(),
                    },
                    min_occurs: 1,
                    max_occurs: Some(1),
                }],
            }
        );
    }
}
