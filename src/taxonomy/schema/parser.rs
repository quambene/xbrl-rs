use crate::{
    Balance, NamespacePrefix, NamespaceUri, PeriodType, RoleUri, XbrlError,
    xml::{self, ArcroleUri, QName},
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
#[derive(Debug, PartialEq, Eq, Default)]
pub enum FormDefault {
    /// Elements or attributes must be qualified with the target namespace to be
    /// valid.
    Qualified,
    /// Elements or attributes can be unqualified (no namespace) to be valid.
    /// This is the default in XBRL taxonomies.
    #[default]
    Unqualified,
}

/// The kind of derivation in a `simpleContent`.
#[derive(Debug, PartialEq, Eq)]
pub enum DerivationKind {
    /// Derivation by extension (from `xs:extension`).
    Extension,
    /// Derivation by restriction (from `xs:restriction`).
    Restriction,
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
pub struct RawRoleType {
    /// The id attribute (e.g., "role_balanceSheet").
    pub id: String,
    /// The roleURI attribute.
    pub role_uri: RoleUri,
    /// The human-readable definition (child `link:definition` text).
    pub definition: Option<String>,
    /// Which link types this role is used on (child `link:usedOn` texts).
    pub used_on: Vec<QName>,
}

/// A `link:arcroleType` definition from a taxonomy schema.
#[derive(Debug, PartialEq, Eq)]
pub struct RawArcroleType {
    /// The id attribute.
    pub id: String,
    /// The arcroleURI attribute.
    pub arcrole_uri: ArcroleUri,
    /// The human-readable definition.
    pub definition: Option<String>,
    /// Which link types this arcrole is used on.
    pub used_on: Vec<QName>,
    /// The cycles-allowed attribute.
    pub cycles_allowed: Option<CyclesAllowed>,
}

/// Represents an attribute use in a `simpleContent` extension or restriction.
#[derive(Debug, PartialEq, Eq)]
pub struct AttributeUse {
    /// The QName of the referenced attribute.
    pub ref_name: String,
    /// Whether this attribute is required (use="required").
    pub required: bool,
}

/// Represents an `xs:anyAttribute` in a complex type's content model.
#[derive(Debug, PartialEq, Eq)]
pub struct AnyAttribute {
    /// The `namespace` attribute of the `xs:anyAttribute`, which determines
    /// which attributes are allowed.
    pub namespace: AnyAttributeNamespace,
}

/// Represents the `namespace` attribute of an `xs:anyAttribute`.
#[derive(Debug, PartialEq, Eq)]
pub enum AnyAttributeNamespace {
    /// All attributes from any namespace are allowed.
    Any,
    /// Only attributes from namespaces other than the target namespace are allowed.
    Other,
    /// Only attributes from the target namespace are allowed.
    TargetNamespace,
    /// Only attributes from specific namespaces are allowed (from a
    /// whitespace-separated list in the `namespace` attribute).
    List(Vec<String>),
}

/// Represents the occurence constraints for a
/// particle in a complex type's content model.
#[derive(Debug, PartialEq, Eq)]
pub struct Occurrence {
    /// The minimum number of occurrences (from `minOccurs`).
    pub min: u32,
    /// The maximum number of occurrences (from `maxOccurs`). None means
    /// unbounded.
    pub max: Option<u32>,
}

/// Represents the derivation method (extension or restriction) for a
/// `complexContent` in a complex type.
#[derive(Debug, PartialEq, Eq)]
pub enum Derivation {
    /// Derivation by extension (from `xs:extension`).
    Extension(QName),
    /// Derivation by restriction (from `xs:restriction`).
    Restriction(QName),
}

/// Represents an element declaration in the schema, which can be either a
/// global element or an inline element declaration inside a compositor.
#[derive(Debug, PartialEq, Eq)]
pub struct ElementDecl {
    /// The name of the element.
    pub name: String,
    /// The type of the element, if specified.
    pub type_name: Option<QName>,
    /// The inline complex type of the element, if specified.
    pub inline_type: Option<Box<ComplexType>>,
}

/// Represents an element particle in a complex type's content model.
#[derive(Debug, PartialEq, Eq)]
pub enum ElementParticle {
    /// A reference to a globally defined element (from `xs:element[@ref]`).
    Ref(QName),
    /// An inline element declaration (from `xs:element` inside a compositor).
    /// This is used in complex content of tuple elements in older XBRL
    /// taxonomies that don't use `complexContent` for tuples.
    Decl(ElementDecl),
}

/// Represents a group definition in a complex type's content model.
#[derive(Debug, PartialEq, Eq)]
pub struct GroupDef {
    /// The name of the group (from `xs:group[@name]`). This is optional because
    /// XBRL allows anonymous groups defined via `xs:group` inside compositors.
    pub name: Option<QName>,
    /// The particle that this group wraps (sequence, choice, or another group).
    pub particle: Box<Particle>,
}

/// Represents a group particle in a complex type's content model.
#[derive(Debug, PartialEq, Eq)]
pub enum GroupParticle {
    /// A reference to a globally defined group (from `xs:group[@ref]`).
    Ref(QName),
    /// An inline group definition (from `xs:group` inside a compositor).
    Def(GroupDef),
}

/// Represents a particle in a complex type's content model.
///
/// A particle is a building block of a complex type's content model, which can
/// be an element, a sequence, a choice, or a group.
#[derive(Debug, PartialEq, Eq)]
pub enum Particle {
    /// An element, which can be either a reference to a globally defined
    /// element or an inline element declaration.
    Element {
        element: ElementParticle,
        occurs: Occurrence,
    },
    /// A sequence compositor, which contains a list of child particles.
    Sequence {
        children: Vec<Particle>,
        occurs: Occurrence,
    },
    /// A choice compositor, which contains a list of child particles.
    Choice {
        children: Vec<Particle>,
        occurs: Occurrence,
    },
    /// A group reference or definition, which can be either a reference to a
    /// globally defined group or an inline group definition.
    Group {
        group: GroupParticle,
        occurs: Occurrence,
    },
}

/// Represents a `simpleContent` in a complex type, which is used for defining
/// tuple types in XBRL.
#[derive(Debug, PartialEq, Eq)]
pub struct SimpleContent {
    /// The base type of the simple content (from `xs:extension` or
    /// `xs:restriction`).
    pub base: QName,
    /// The kind of derivation (extension or restriction) for this simple
    /// content.
    pub derivation: DerivationKind,
}
/// Represents a `complexContent` in a complex type, which is used for defining
/// tuple types with child elements in XBRL.
#[derive(Debug, PartialEq, Eq)]
pub struct ComplexContent {
    /// The kind of derivation (extension or restriction) for this complex
    /// content.
    pub derivation: Option<Derivation>,
    /// The content model particle, if any. `None` means the particle is
    /// inherited from the base type (extension/restriction with no inline
    /// particle).
    pub particle: Option<Particle>,
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
    ///
    /// Usually it's sufficient to support xs:enumeration facets.
    pub enumerations: Vec<String>,
}

/// Represents a complex type definition (`xs:complexType`) in the schema, which
/// can be used for both item and tuple elements in XBRL.
#[derive(Debug, PartialEq, Eq)]
pub enum ComplexTypeContent {
    /// A `simpleContent` is used for tuple types that only have attributes and
    /// no child elements.
    SimpleContent(SimpleContent),
    /// A `complexContent` is used for tuple types that have child elements, and
    /// can also have attributes.
    ///
    /// In XML Schema, this may be defined either via `<complexContent>` or
    /// implicitly by directly containing a particle (`sequence`, `choice`,
    /// `all`). Both forms are treated uniformly.
    ComplexContent(ComplexContent),
    /// An empty complex type has no explicit content. This is used for tuple
    /// types that have no attributes and no child elements.
    Empty,
}

/// Represents a complex type definition (`xs:complexType`) in the schema.
#[derive(Debug, PartialEq, Eq)]
pub struct ComplexType {
    /// The name of the complex type.
    pub name: Option<String>,
    /// Whether this complex type is mixed (from `mixed="true"` in the
    /// `xs:complexType`).
    pub mixed: bool,
    /// Attributes defined on this type via `xs:attribute` elements.
    pub attributes: Vec<AttributeUse>,
    /// The `xs:anyAttribute` of this complex content, if present.
    pub any_attribute: Option<AnyAttribute>,
    /// The content of the complex type.
    pub content: Option<ComplexTypeContent>,
}

/// A parsed XML element from the schema (xs:element).
#[derive(Debug, PartialEq, Eq)]
pub struct Element {
    /// The element's local name (e.g., "bs.ass.fixAss").
    pub name: String,
    /// The element's id attribute (e.g. "de-gaap-ci_bs.ass.fixAss"). Optional
    /// in XBRL.
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
    /// The `elementFormDefault` of the schema.
    pub element_form_default: FormDefault,
    /// The `attributeFormDefault` of the schema.
    pub attribute_form_default: FormDefault,
    /// Parsed `xs:import` references.
    pub imports: Vec<SchemaImport>,
    /// Parsed `xs:include` references.
    pub includes: Vec<SchemaInclude>,
    /// Parsed `link:linkbaseRef` entries.
    pub linkbase_refs: Vec<LinkbaseRef>,
    /// Parsed `link:roleType` definitions.
    pub role_types: Vec<RawRoleType>,
    /// Parsed `link:arcroleType` definitions.
    pub arcrole_types: Vec<RawArcroleType>,
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
        let file = File::open(path).map_err(|err| XbrlError::FileOpen {
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
            element_form_default: FormDefault::Unqualified,
            attribute_form_default: FormDefault::Unqualified,
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
                        b"group" => {
                            // xs:group definitions at the schema level are
                            // skipped; group refs inside compositors are parsed.
                            self.skip_until(b"group")?;
                        }
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

            match attribute.key.prefix() {
                Some(prefix) if prefix.as_ref() == b"xmlns" => {
                    schema.namespaces.insert(
                        NamespacePrefix::from(str::from_utf8(local_name.as_ref())?),
                        NamespaceUri::from(value.to_string()),
                    );
                }
                _ => {}
            }

            match local_name.as_ref() {
                b"targetNamespace" => {
                    schema.target_namespace = Some(value.to_string());
                }
                b"elementFormDefault" => {
                    schema.element_form_default = match value.as_ref() {
                        "qualified" => FormDefault::Qualified,
                        "unqualified" => FormDefault::Unqualified,
                        _ => {
                            return Err(XbrlError::ParseError {
                                expected: "elementFormDefault value",
                                value: value.to_string(),
                            });
                        }
                    };
                }
                b"attributeFormDefault" => {
                    schema.attribute_form_default = match value.as_ref() {
                        "qualified" => FormDefault::Qualified,
                        "unqualified" => FormDefault::Unqualified,
                        _ => {
                            return Err(XbrlError::ParseError {
                                expected: "attributeFormDefault value",
                                value: value.to_string(),
                            });
                        }
                    };
                }
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
    fn parse_role_type(&mut self, attributes: Attributes) -> Result<RawRoleType, XbrlError> {
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
                            used_on.push(xml::parse_qname(
                                &text.xml_content().map_err(quick_xml::Error::from)?,
                            ));
                        }
                    }
                    _ => self.skip_element()?,
                },
                Event::End(ref event) if event.local_name().as_ref() == b"roleType" => break,
                _ => {}
            }
            buf.clear();
        }

        Ok(RawRoleType {
            id,
            role_uri: RoleUri::from(role_uri),
            definition,
            used_on,
        })
    }

    /// Parses a `link:arcroleType` element.
    fn parse_arcrole_type(&mut self, attributes: Attributes) -> Result<RawArcroleType, XbrlError> {
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
                            used_on.push(xml::parse_qname(
                                &text.xml_content().map_err(quick_xml::Error::from)?,
                            ));
                        }
                    }
                    _ => self.skip_element()?,
                },
                Event::End(ref event) if event.local_name().as_ref() == b"arcroleType" => break,
                _ => {}
            }
            buf.clear();
        }

        Ok(RawArcroleType {
            id,
            arcrole_uri: ArcroleUri::from(arcrole_uri),
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

    /// Skips all events until (and including) the closing tag with `end_tag`
    /// local name.
    fn skip_until(&mut self, end_tag: &[u8]) -> Result<(), XbrlError> {
        let mut buf = Vec::new();
        let mut depth = 1usize;

        loop {
            match self.reader.read_event_into(&mut buf)? {
                Event::Start(_) => depth += 1,
                Event::End(ref event) => {
                    depth -= 1;

                    if depth == 0 {
                        debug_assert_eq!(event.local_name().as_ref(), end_tag);
                        break;
                    }
                }
                Event::Eof => break,
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
            mixed: false,
            attributes: Vec::new(),
            any_attribute: None,
            content: None,
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

            if attribute.key.local_name().as_ref() == b"mixed" {
                let value = attribute.decode_and_unescape_value(self.reader.decoder())?;
                complex_type.mixed = value == "true";
            }
        }

        // Element appearing directly in the complexType body (not inside
        // simpleContent or complexContent).
        let mut direct_particle: Option<Particle> = None;

        loop {
            let (event, has_body) = match self.reader.read_event_into(&mut buf)? {
                Event::Start(event) => (event, true),
                Event::Empty(event) => (event, false),
                Event::End(ref event) if event.name() == start.name() => break,
                Event::Eof => break,
                _ => {
                    buf.clear();
                    continue;
                }
            };

            match event.local_name().as_ref() {
                b"sequence" | b"choice" if has_body => {
                    direct_particle = Some(self.parse_particle(&event)?);
                }
                b"sequence" => {
                    let occurs = self.parse_occurs(&event)?;
                    direct_particle = Some(Particle::Sequence {
                        children: vec![],
                        occurs,
                    });
                }
                b"choice" => {
                    let occurs = self.parse_occurs(&event)?;
                    direct_particle = Some(Particle::Choice {
                        children: vec![],
                        occurs,
                    });
                }
                b"simpleContent" => {
                    if direct_particle.is_some() {
                        return Err(XbrlError::InvalidSchemaDocument {
                            path: self.path.clone(),
                            reason: "simpleContent cannot appear alongside a direct particle"
                                .to_string(),
                        });
                    }

                    self.parse_simple_content(&mut complex_type)?;
                    // simpleContent must be the only content of a complexType,
                    // so we break the loop after parsing it.
                    break;
                }
                b"complexContent" => {
                    if direct_particle.is_some() {
                        return Err(XbrlError::InvalidSchemaDocument {
                            path: self.path.clone(),
                            reason: "complexContent cannot appear alongside a direct particle"
                                .to_string(),
                        });
                    }

                    self.parse_complex_content(&mut complex_type, &event)?;
                    // complexContent must be the only content of a complexType, so we
                    break;
                }

                b"attribute" => {
                    complex_type.attributes.push(self.parse_attribute(&event)?);
                }
                b"anyAttribute" => {
                    let any_attribute = Some(self.parse_any_attribute(&event)?);

                    if complex_type.any_attribute.is_some() {
                        return Err(XbrlError::InvalidSchemaDocument {
                            path: self.path.clone(),
                            reason: "only one anyAttribute is allowed per complexType".to_string(),
                        });
                    }

                    complex_type.any_attribute = any_attribute;
                }
                b"restriction" => {
                    return Err(XbrlError::InvalidSchemaDocument {
                        path: self.path.clone(),
                        reason:
                            "restriction is only allowed inside simpleContent or complexContent"
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
                _ => {}
            }

            buf.clear();
        }

        // Build content from direct-body elements if simpleContent/complexContent
        // didn't already set it.
        if complex_type.content.is_none() {
            complex_type.content = direct_particle.map(|particle| {
                ComplexTypeContent::ComplexContent(ComplexContent {
                    derivation: None,
                    particle: Some(particle),
                })
            });
        }

        Ok(complex_type)
    }

    /// Parses `minOccurs`/`maxOccurs` attributes from a start tag.
    fn parse_occurs(&self, start: &BytesStart) -> Result<Occurrence, XbrlError> {
        let mut min = 1u32;
        let mut max = Some(1u32);

        for attribute in start.attributes() {
            let attribute = attribute.map_err(|err| XbrlError::XmlParse {
                path: self.path.clone(),
                position: self.reader.buffer_position(),
                element: None,
                source: err.into(),
            })?;
            let value = attribute.decode_and_unescape_value(self.reader.decoder())?;
            match attribute.key.local_name().as_ref() {
                b"minOccurs" => min = xml::parse_u32(&value)?,
                b"maxOccurs" => {
                    max = if value == "unbounded" {
                        None
                    } else {
                        Some(xml::parse_u32(&value)?)
                    };
                }
                _ => {}
            }
        }

        Ok(Occurrence { min, max })
    }

    /// Parses an `xs:sequence` or `xs:choice` element into a recursive
    /// [`Particle`] tree. `start` is the opening tag (used to determine the
    /// compositor kind and to recognise the matching closing tag).
    fn parse_particle(&mut self, start: &BytesStart) -> Result<Particle, XbrlError> {
        let occurs = self.parse_occurs(start)?;
        let mut children = Vec::new();
        let mut buf = Vec::new();

        loop {
            match self.reader.read_event_into(&mut buf)? {
                Event::Start(ref event) => match event.local_name().as_ref() {
                    b"element" => {
                        children.push(self.parse_element_particle(event, false)?);
                    }
                    b"sequence" | b"choice" => {
                        children.push(self.parse_particle(event)?);
                    }
                    b"group" => {
                        children.push(self.parse_group_particle(event)?);
                    }
                    _ => {}
                },
                Event::Empty(ref event) => match event.local_name().as_ref() {
                    b"element" => {
                        children.push(self.parse_element_particle(event, true)?);
                    }
                    b"sequence" => {
                        let nested_occurs = self.parse_occurs(event)?;
                        children.push(Particle::Sequence {
                            children: vec![],
                            occurs: nested_occurs,
                        });
                    }
                    b"choice" => {
                        let nested_occurs = self.parse_occurs(event)?;
                        children.push(Particle::Choice {
                            children: vec![],
                            occurs: nested_occurs,
                        });
                    }
                    b"group" => {
                        children.push(self.parse_group_particle(event)?);
                    }
                    _ => {}
                },
                Event::End(ref event)
                    if event.local_name().as_ref() == start.local_name().as_ref() =>
                {
                    break;
                }
                Event::Eof => break,
                _ => {}
            }

            buf.clear();
        }

        match start.local_name().as_ref() {
            b"sequence" => Ok(Particle::Sequence { children, occurs }),
            b"choice" => Ok(Particle::Choice { children, occurs }),
            _ => {
                return Err(XbrlError::InvalidSchemaDocument {
                    path: self.path.clone(),
                    reason: format!(
                        "unexpected compositor tag: {}",
                        String::from_utf8_lossy(start.local_name().as_ref())
                    ),
                });
            }
        }
    }

    /// Parses an `xs:element` inside a compositor (sequence or choice).
    /// `is_empty` indicates whether the event was an `Empty` (self-closing)
    /// tag.
    fn parse_element_particle(
        &mut self,
        start: &BytesStart,
        is_empty: bool,
    ) -> Result<Particle, XbrlError> {
        let occurs = self.parse_occurs(start)?;
        let mut ref_name: Option<QName> = None;
        let mut decl_name: Option<String> = None;
        let mut type_name: Option<QName> = None;

        for attribute in start.attributes() {
            let attribute = attribute.map_err(|err| XbrlError::XmlParse {
                path: self.path.clone(),
                position: self.reader.buffer_position(),
                element: Some("element".to_string()),
                source: err.into(),
            })?;
            let value = attribute.decode_and_unescape_value(self.reader.decoder())?;
            match attribute.key.local_name().as_ref() {
                b"ref" => ref_name = Some(xml::parse_qname(&value)),
                b"name" => decl_name = Some(value.to_string()),
                b"type" => type_name = Some(xml::parse_qname(&value)),
                _ => {}
            }
        }

        if let Some(qname) = ref_name {
            // xs:element[@ref] carries only ref/minOccurs/maxOccurs; any child
            // content (e.g. xs:annotation) is irrelevant for XBRL parsing.
            if !is_empty {
                self.skip_until(b"element")?;
            }
            return Ok(Particle::Element {
                element: ElementParticle::Ref(qname),
                occurs,
            });
        }

        // Inline element declaration.
        let name = decl_name.unwrap_or_default();
        let mut inline_type: Option<Box<ComplexType>> = None;

        if !is_empty {
            // Read child events to find an optional inline xs:complexType.
            let mut buf = Vec::new();
            loop {
                match self.reader.read_event_into(&mut buf)? {
                    Event::Start(ref event) if event.local_name().as_ref() == b"complexType" => {
                        inline_type = Some(Box::new(self.parse_complex_type(event)?));
                    }
                    Event::End(ref event) if event.local_name().as_ref() == b"element" => {
                        break;
                    }
                    Event::Eof => break,
                    _ => {}
                }
                buf.clear();
            }
        }

        Ok(Particle::Element {
            element: ElementParticle::Decl(ElementDecl {
                name,
                type_name,
                inline_type,
            }),
            occurs,
        })
    }

    /// Parses an `xs:group` reference inside a compositor.
    fn parse_group_particle(&mut self, start: &BytesStart) -> Result<Particle, XbrlError> {
        let occurs = self.parse_occurs(start)?;
        let mut ref_name: Option<QName> = None;

        for attribute in start.attributes() {
            let attribute = attribute.map_err(|err| XbrlError::XmlParse {
                path: self.path.clone(),
                position: self.reader.buffer_position(),
                element: Some("group".to_string()),
                source: err.into(),
            })?;
            let value = attribute.decode_and_unescape_value(self.reader.decoder())?;
            if attribute.key.local_name().as_ref() == b"ref" {
                ref_name = Some(xml::parse_qname(&value));
            }
        }

        Ok(Particle::Group {
            group: GroupParticle::Ref(ref_name.ok_or_else(|| {
                XbrlError::InvalidSchemaDocument {
                    path: self.path.clone(),
                    reason: "xs:group inside a compositor requires a ref attribute".to_string(),
                }
            })?),
            occurs,
        })
    }

    /// Parses an `xs:simpleContent` element.
    fn parse_simple_content(&mut self, complex_type: &mut ComplexType) -> Result<(), XbrlError> {
        let mut buf = Vec::new();

        loop {
            match self.reader.read_event_into(&mut buf)? {
                Event::Start(ref event)
                    if matches!(event.local_name().as_ref(), b"extension" | b"restriction") =>
                {
                    let derivation = if event.local_name().as_ref() == b"extension" {
                        DerivationKind::Extension
                    } else {
                        DerivationKind::Restriction
                    };

                    let mut base: Option<QName> = None;
                    for attribute in event.attributes() {
                        let attribute = attribute.map_err(|err| XbrlError::XmlParse {
                            path: self.path.clone(),
                            position: self.reader.buffer_position(),
                            element: Some("simpleContent extension/restriction".to_string()),
                            source: err.into(),
                        })?;

                        if attribute.key.local_name().as_ref() == b"base" {
                            let value =
                                attribute.decode_and_unescape_value(self.reader.decoder())?;
                            base = Some(xml::parse_qname(&value));
                        }
                    }

                    if let Some(base) = base {
                        complex_type.content =
                            Some(ComplexTypeContent::SimpleContent(SimpleContent {
                                base,
                                derivation,
                            }));
                    }

                    complex_type.attributes =
                        self.parse_attributes_until(event.local_name().as_ref())?;
                }

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

    /// Reads `<xs:attribute>` elements until the closing tag `end_tag`,
    /// returning a vector of `AttributeUse`.
    fn parse_attributes_until(&mut self, end_tag: &[u8]) -> Result<Vec<AttributeUse>, XbrlError> {
        let mut buf = Vec::new();
        let mut attributes = Vec::new();

        loop {
            match self.reader.read_event_into(&mut buf)? {
                Event::Start(ref event) | Event::Empty(ref event)
                    if event.local_name().as_ref() == b"attribute" =>
                {
                    attributes.push(self.parse_attribute(event)?);
                }
                Event::End(ref event) if event.local_name().as_ref() == end_tag => {
                    break;
                }
                Event::Eof => {
                    return Err(XbrlError::ParseError {
                        expected: "end tag while parsing attributes",
                        value: "EOF reached".to_string(),
                    });
                }
                _ => {}
            }
            buf.clear();
        }

        Ok(attributes)
    }

    /// Parses an `xs:complexContent` element.
    /// `start` is the `<xs:complexContent>` tag, used to read its `mixed`
    /// attribute and to recognise the matching closing tag.
    fn parse_complex_content(
        &mut self,
        complex_type: &mut ComplexType,
        start: &BytesStart,
    ) -> Result<(), XbrlError> {
        for attribute in start.attributes() {
            let attribute = attribute.map_err(|err| XbrlError::XmlParse {
                path: self.path.clone(),
                position: self.reader.buffer_position(),
                element: Some("complexContent".to_string()),
                source: err.into(),
            })?;
            if attribute.key.local_name().as_ref() == b"mixed" {
                let value = attribute.decode_and_unescape_value(self.reader.decoder())?;
                complex_type.mixed = value == "true";
            }
        }

        let mut buf = Vec::new();

        loop {
            match self.reader.read_event_into(&mut buf)? {
                Event::Start(ref event)
                    if matches!(event.local_name().as_ref(), b"extension" | b"restriction") =>
                {
                    let derivation_kind = match event.local_name().as_ref() {
                        b"extension" => DerivationKind::Extension,
                        b"restriction" => DerivationKind::Restriction,
                        _ => unreachable!(),
                    };
                    let mut base: Option<QName> = None;

                    for attribute in event.attributes() {
                        let attribute = attribute.map_err(|err| XbrlError::XmlParse {
                            path: self.path.clone(),
                            position: self.reader.buffer_position(),
                            element: Some("complexContent extension/restriction".to_string()),
                            source: err.into(),
                        })?;

                        if attribute.key.local_name().as_ref() == b"base" {
                            let value =
                                attribute.decode_and_unescape_value(self.reader.decoder())?;
                            base = Some(xml::parse_qname(&value));
                        }
                    }

                    let tag_name = event.local_name();
                    let tag = tag_name.as_ref();
                    let (particle, attributes, any_attribute) =
                        self.parse_complex_derivation(tag)?;

                    complex_type.attributes.extend(attributes);
                    complex_type.any_attribute = any_attribute;
                    complex_type.content =
                        Some(ComplexTypeContent::ComplexContent(ComplexContent {
                            derivation: base.map(|base| match derivation_kind {
                                DerivationKind::Extension => Derivation::Extension(base),
                                DerivationKind::Restriction => Derivation::Restriction(base),
                            }),
                            particle,
                        }));
                }

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

    /// Parses the body of an `xs:extension` or `xs:restriction` inside
    /// `xs:complexContent`, collecting the optional particle, attributes, and
    /// `anyAttribute`. Returns when the matching closing tag is consumed.
    fn parse_complex_derivation(
        &mut self,
        end_tag: &[u8],
    ) -> Result<(Option<Particle>, Vec<AttributeUse>, Option<AnyAttribute>), XbrlError> {
        let mut buf = Vec::new();
        let mut particle: Option<Particle> = None;
        let mut attributes = Vec::new();
        let mut any_attribute: Option<AnyAttribute> = None;

        loop {
            match self.reader.read_event_into(&mut buf)? {
                Event::Start(ref event) => match event.local_name().as_ref() {
                    b"sequence" | b"choice" => {
                        particle = Some(self.parse_particle(event)?);
                    }
                    b"attribute" => {
                        attributes.push(self.parse_attribute(event)?);
                    }
                    _ => {}
                },
                Event::Empty(ref event) => match event.local_name().as_ref() {
                    b"sequence" => {
                        let occurs = self.parse_occurs(event)?;
                        particle = Some(Particle::Sequence {
                            children: vec![],
                            occurs,
                        });
                    }
                    b"choice" => {
                        let occurs = self.parse_occurs(event)?;
                        particle = Some(Particle::Choice {
                            children: vec![],
                            occurs,
                        });
                    }
                    b"attribute" => {
                        attributes.push(self.parse_attribute(event)?);
                    }
                    b"anyAttribute" => {
                        any_attribute = Some(self.parse_any_attribute(event)?);
                    }
                    _ => {}
                },
                Event::End(ref event) if event.local_name().as_ref() == end_tag => {
                    break;
                }
                Event::Eof => break,
                _ => {}
            }

            buf.clear();
        }

        Ok((particle, attributes, any_attribute))
    }

    /// Parses an `xs:anyAttribute` element.
    fn parse_any_attribute(&self, start: &BytesStart) -> Result<AnyAttribute, XbrlError> {
        let mut namespace = AnyAttributeNamespace::Any;

        for attribute in start.attributes() {
            let attribute = attribute.map_err(|err| XbrlError::XmlParse {
                path: self.path.clone(),
                position: self.reader.buffer_position(),
                element: Some("anyAttribute".to_string()),
                source: err.into(),
            })?;

            if attribute.key.local_name().as_ref() == b"namespace" {
                let value = attribute.decode_and_unescape_value(self.reader.decoder())?;
                namespace = match value.as_ref() {
                    "##any" => AnyAttributeNamespace::Any,
                    "##other" => AnyAttributeNamespace::Other,
                    "##targetNamespace" => AnyAttributeNamespace::TargetNamespace,
                    other => {
                        let namespaces = other
                            .split_whitespace()
                            .map(|s| s.to_string())
                            .collect::<Vec<_>>();
                        if namespaces.len() == 1 {
                            AnyAttributeNamespace::List(namespaces)
                        } else {
                            AnyAttributeNamespace::List(namespaces)
                        }
                    }
                };
            }
        }

        Ok(AnyAttribute { namespace })
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
    fn test_parse_schema_root() {
        let xml = r#"<xsd:schema
                                xmlns:xsd="http://www.w3.org/2001/XMLSchema"
                                xmlns:xbrli="http://www.xbrl.org/2003/instance"
                                targetNamespace="http://example.com/taxonomy"
                                elementFormDefault="qualified">
                            </xsd:schema>"#;
        let mut parser = SchemaParser::from_reader(xml.as_bytes());
        let schema = parser.parse_schema().unwrap();

        assert_matches!(schema.element_form_default, FormDefault::Qualified);
        assert_matches!(schema.attribute_form_default, FormDefault::Unqualified);
        assert_eq!(
            schema.target_namespace,
            Some("http://example.com/taxonomy".to_string())
        );
        assert_eq!(
            schema.namespaces,
            HashMap::from_iter([
                ("xsd".into(), "http://www.w3.org/2001/XMLSchema".into()),
                ("xbrli".into(), "http://www.xbrl.org/2003/instance".into()),
            ])
        );
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
            role_type.role_uri.as_str(),
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
            arcrole_type.arcrole_uri.as_str(),
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
                    mixed: false,
                    attributes: vec![],
                    any_attribute: None,
                    content: Some(ComplexTypeContent::ComplexContent(ComplexContent {
                        derivation: None,
                        particle: Some(Particle::Sequence {
                            children: vec![
                                Particle::Element {
                                    element: ElementParticle::Ref(QName {
                                        prefix: Some(NamespacePrefix::from("my")),
                                        local_name: "city".to_string(),
                                    }),
                                    occurs: Occurrence {
                                        min: 1,
                                        max: Some(1)
                                    },
                                },
                                Particle::Element {
                                    element: ElementParticle::Ref(QName {
                                        prefix: Some(NamespacePrefix::from("my")),
                                        local_name: "country".to_string(),
                                    }),
                                    occurs: Occurrence {
                                        min: 0,
                                        max: Some(1)
                                    },
                                },
                            ],
                            occurs: Occurrence {
                                min: 1,
                                max: Some(1)
                            },
                        }),
                    })),
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
                    mixed: false,
                    attributes: vec![],
                    any_attribute: None,
                    content: Some(ComplexTypeContent::ComplexContent(ComplexContent {
                        derivation: None,
                        particle: Some(Particle::Sequence {
                            children: vec![
                                Particle::Element {
                                    element: ElementParticle::Ref(QName {
                                        prefix: Some(NamespacePrefix::from("my")),
                                        local_name: "itemA".to_string(),
                                    }),
                                    occurs: Occurrence {
                                        min: 2,
                                        max: Some(2)
                                    },
                                },
                                Particle::Element {
                                    element: ElementParticle::Ref(QName {
                                        prefix: Some(NamespacePrefix::from("my")),
                                        local_name: "itemB".to_string(),
                                    }),
                                    occurs: Occurrence { min: 0, max: None },
                                },
                            ],
                            occurs: Occurrence {
                                min: 1,
                                max: Some(1)
                            },
                        }),
                    })),
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
                    mixed: false,
                    attributes: vec![],
                    any_attribute: None,
                    content: Some(ComplexTypeContent::ComplexContent(ComplexContent {
                        derivation: None,
                        particle: Some(Particle::Choice {
                            children: vec![
                                Particle::Element {
                                    element: ElementParticle::Ref(QName {
                                        prefix: Some(NamespacePrefix::from("my")),
                                        local_name: "optA".to_string(),
                                    }),
                                    occurs: Occurrence {
                                        min: 1,
                                        max: Some(1)
                                    },
                                },
                                Particle::Element {
                                    element: ElementParticle::Ref(QName {
                                        prefix: Some(NamespacePrefix::from("my")),
                                        local_name: "optB".to_string(),
                                    }),
                                    occurs: Occurrence {
                                        min: 1,
                                        max: Some(1)
                                    },
                                },
                            ],
                            occurs: Occurrence {
                                min: 1,
                                max: Some(1)
                            },
                        }),
                    })),
                }),
            }
        );
    }

    #[test]
    fn test_parse_element_particle_with_inline_complex_type() {
        let xml = r#"<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema"
                                targetNamespace="http://example.com"
                                xmlns:my="http://example.com"
                                xmlns:xbrli="http://www.xbrl.org/2003/instance">
                                <xs:element name="AddressTuple" substitutionGroup="xbrli:tuple">
                                    <xs:complexType>
                                        <xs:sequence>
                                            <xs:element name="street">
                                                <xs:complexType>
                                                    <xs:sequence>
                                                        <xs:element ref="my:line1" />
                                                    </xs:sequence>
                                                </xs:complexType>
                                            </xs:element>
                                        </xs:sequence>
                                    </xs:complexType>
                                </xs:element>
                            </xs:schema>"#;
        let mut parser = SchemaParser::from_reader(xml.as_bytes());
        let schema = parser.parse_schema().unwrap();

        let element = &schema.elements[0];
        assert_eq!(
            element,
            &Element {
                name: "AddressTuple".to_string(),
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
                    mixed: false,
                    attributes: vec![],
                    any_attribute: None,
                    content: Some(ComplexTypeContent::ComplexContent(ComplexContent {
                        derivation: None,
                        particle: Some(Particle::Sequence {
                            children: vec![Particle::Element {
                                element: ElementParticle::Decl(ElementDecl {
                                    name: "street".to_string(),
                                    type_name: None,
                                    inline_type: Some(Box::new(ComplexType {
                                        name: None,
                                        mixed: false,
                                        attributes: vec![],
                                        any_attribute: None,
                                        content: Some(ComplexTypeContent::ComplexContent(
                                            ComplexContent {
                                                derivation: None,
                                                particle: Some(Particle::Sequence {
                                                    children: vec![Particle::Element {
                                                        element: ElementParticle::Ref(QName {
                                                            prefix: Some(NamespacePrefix::from(
                                                                "my"
                                                            )),
                                                            local_name: "line1".to_string(),
                                                        }),
                                                        occurs: Occurrence {
                                                            min: 1,
                                                            max: Some(1)
                                                        },
                                                    }],
                                                    occurs: Occurrence {
                                                        min: 1,
                                                        max: Some(1)
                                                    },
                                                }),
                                            }
                                        )),
                                    })),
                                }),
                                occurs: Occurrence {
                                    min: 1,
                                    max: Some(1)
                                },
                            }],
                            occurs: Occurrence {
                                min: 1,
                                max: Some(1)
                            },
                        }),
                    })),
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
                mixed: false,
                attributes: vec![],
                any_attribute: None,
                content: None,
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
                mixed: false,
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
                any_attribute: None,
                content: Some(ComplexTypeContent::SimpleContent(SimpleContent {
                    base: QName {
                        prefix: Some(NamespacePrefix::from("xbrli")),
                        local_name: "decimalItemType".to_string(),
                    },
                    derivation: DerivationKind::Extension,
                })),
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
                mixed: false,
                attributes: vec![AttributeUse {
                    ref_name: "xbrli:unitRef".to_string(),
                    required: true,
                }],
                any_attribute: None,
                content: Some(ComplexTypeContent::SimpleContent(SimpleContent {
                    base: QName {
                        prefix: Some(NamespacePrefix::from("xbrli")),
                        local_name: "decimalItemType".to_string(),
                    },
                    derivation: DerivationKind::Restriction,
                })),
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
                mixed: false,
                attributes: vec![],
                any_attribute: None,
                content: Some(ComplexTypeContent::ComplexContent(ComplexContent {
                    derivation: None,
                    particle: Some(Particle::Sequence {
                        children: vec![Particle::Element {
                            element: ElementParticle::Ref(QName {
                                prefix: Some(NamespacePrefix::from("xs")),
                                local_name: "name".to_string(),
                            }),
                            occurs: Occurrence {
                                min: 1,
                                max: Some(1)
                            },
                        }],
                        occurs: Occurrence {
                            min: 1,
                            max: Some(1)
                        },
                    }),
                })),
            }
        );
        let complex_type = &schema.complex_types[1];
        assert_eq!(
            complex_type,
            &ComplexType {
                name: Some("extendedAccountType".to_string()),
                mixed: false,
                attributes: vec![AttributeUse {
                    ref_name: "currency".to_string(),
                    required: false,
                }],
                any_attribute: None,
                content: Some(ComplexTypeContent::ComplexContent(ComplexContent {
                    derivation: Some(Derivation::Extension(QName {
                        prefix: None,
                        local_name: "baseAccountType".to_string(),
                    })),
                    particle: Some(Particle::Sequence {
                        children: vec![Particle::Element {
                            element: ElementParticle::Ref(QName {
                                prefix: Some(NamespacePrefix::from("xs")),
                                local_name: "balance".to_string(),
                            }),
                            occurs: Occurrence {
                                min: 1,
                                max: Some(1)
                            },
                        }],
                        occurs: Occurrence {
                            min: 1,
                            max: Some(1)
                        },
                    }),
                })),
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

        let complex_type = &schema.complex_types[0];
        assert_eq!(
            complex_type,
            &ComplexType {
                name: Some("baseAccountType".to_string()),
                mixed: false,
                attributes: vec![AttributeUse {
                    ref_name: "currency".to_string(),
                    required: false,
                }],
                any_attribute: None,
                content: Some(ComplexTypeContent::ComplexContent(ComplexContent {
                    derivation: None,
                    particle: Some(Particle::Sequence {
                        children: vec![
                            Particle::Element {
                                element: ElementParticle::Ref(QName {
                                    prefix: Some(NamespacePrefix::from("xs")),
                                    local_name: "name".to_string(),
                                }),
                                occurs: Occurrence {
                                    min: 1,
                                    max: Some(1)
                                },
                            },
                            Particle::Element {
                                element: ElementParticle::Ref(QName {
                                    prefix: Some(NamespacePrefix::from("xs")),
                                    local_name: "balance".to_string(),
                                }),
                                occurs: Occurrence {
                                    min: 1,
                                    max: Some(1)
                                },
                            },
                        ],
                        occurs: Occurrence {
                            min: 1,
                            max: Some(1)
                        },
                    }),
                })),
            }
        );
        let restricted_type = &schema.complex_types[1];
        assert_eq!(
            restricted_type,
            &ComplexType {
                name: Some("restrictedAccountType".to_string()),
                mixed: false,
                attributes: vec![AttributeUse {
                    ref_name: "currency".to_string(),
                    required: true,
                }],
                any_attribute: None,
                content: Some(ComplexTypeContent::ComplexContent(ComplexContent {
                    derivation: Some(Derivation::Restriction(QName {
                        prefix: None,
                        local_name: "baseAccountType".to_string(),
                    })),
                    particle: Some(Particle::Sequence {
                        children: vec![Particle::Element {
                            element: ElementParticle::Ref(QName {
                                prefix: Some(NamespacePrefix::from("xs")),
                                local_name: "name".to_string(),
                            }),
                            occurs: Occurrence {
                                min: 1,
                                max: Some(1)
                            },
                        }],
                        occurs: Occurrence {
                            min: 1,
                            max: Some(1)
                        },
                    }),
                })),
            }
        );
    }

    #[test]
    fn test_parse_complex_type_with_mixed_type() {
        let xml = r#"<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema"
                                targetNamespace="http://example.com"
                                xmlns="http://example.com">
                                <xs:complexType name="MixedType" mixed="true">
                                    <xs:sequence>
                                        <xs:element name="child" type="xs:string" />
                                    </xs:sequence>
                                </xs:complexType>
                            </xs:schema>"#;
        let mut parser = SchemaParser::from_reader(xml.as_bytes());
        let schema = parser.parse_schema().unwrap();

        let complex_type = &schema.complex_types[0];
        assert_eq!(
            complex_type,
            &ComplexType {
                name: Some("MixedType".to_string()),
                mixed: true,
                attributes: vec![],
                any_attribute: None,
                content: Some(ComplexTypeContent::ComplexContent(ComplexContent {
                    derivation: None,
                    particle: Some(Particle::Sequence {
                        children: vec![Particle::Element {
                            element: ElementParticle::Decl(ElementDecl {
                                name: "child".to_string(),
                                type_name: Some(QName {
                                    prefix: Some(NamespacePrefix::from("xs")),
                                    local_name: "string".to_string(),
                                }),
                                inline_type: None,
                            }),
                            occurs: Occurrence {
                                min: 1,
                                max: Some(1)
                            },
                        }],
                        occurs: Occurrence {
                            min: 1,
                            max: Some(1)
                        },
                    }),
                })),
            }
        );
    }

    #[test]
    fn test_parse_complex_type_with_any_attribute() {
        let xml = r#"<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema"
                                targetNamespace="http://example.com"
                                xmlns="http://example.com">
                                <!-- ##any -->
                                <xs:complexType name="AnyAttrType">
                                    <xs:sequence />
                                    <xs:anyAttribute />
                                </xs:complexType>
                                <!-- specific namespace -->
                                <xs:complexType name="SpecificAttrType">
                                    <xs:sequence />
                                    <xs:anyAttribute namespace="http://example.com/other" />
                                </xs:complexType>
                            </xs:schema>"#;
        let mut parser = SchemaParser::from_reader(xml.as_bytes());
        let schema = parser.parse_schema().unwrap();

        let complex_type = &schema.complex_types[0];
        assert_eq!(
            complex_type,
            &ComplexType {
                name: Some("AnyAttrType".to_string()),
                mixed: false,
                attributes: vec![],
                any_attribute: Some(AnyAttribute {
                    namespace: AnyAttributeNamespace::Any,
                }),
                content: Some(ComplexTypeContent::ComplexContent(ComplexContent {
                    derivation: None,
                    particle: Some(Particle::Sequence {
                        children: vec![],
                        occurs: Occurrence {
                            min: 1,
                            max: Some(1)
                        },
                    }),
                })),
            }
        );
    }

    #[test]
    fn test_parse_complex_type_with_group_references() {
        let xml = r#"<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema"
                                targetNamespace="http://example.com"
                                xmlns="http://example.com">
                                <xs:group name="CommonGroup">
                                    <xs:sequence>
                                        <xs:element name="a" type="xs:string" />
                                        <xs:element name="b" type="xs:string" />
                                    </xs:sequence>
                                </xs:group>
                                <xs:complexType name="UsesGroup">
                                    <xs:sequence>
                                        <xs:group ref="CommonGroup" />
                                    </xs:sequence>
                                </xs:complexType>
                            </xs:schema>"#;
        let mut parser = SchemaParser::from_reader(xml.as_bytes());
        let schema = parser.parse_schema().unwrap();

        let complex_type = &schema.complex_types[0];
        assert_eq!(
            complex_type,
            &ComplexType {
                name: Some("UsesGroup".to_string()),
                mixed: false,
                attributes: vec![],
                any_attribute: None,
                content: Some(ComplexTypeContent::ComplexContent(ComplexContent {
                    derivation: None,
                    particle: Some(Particle::Sequence {
                        children: vec![Particle::Group {
                            group: GroupParticle::Ref(QName {
                                prefix: None,
                                local_name: "CommonGroup".to_string()
                            }),
                            occurs: Occurrence {
                                min: 1,
                                max: Some(1)
                            }
                        }],
                        occurs: Occurrence {
                            min: 1,
                            max: Some(1)
                        },
                    }),
                })),
            }
        );
    }

    #[test]
    fn test_parse_complex_type_with_anonymous_child_element() {
        let xml = r#"<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema"
                                targetNamespace="http://example.com"
                                xmlns="http://example.com">
                                <xs:complexType name="AnonymousChildrenType">
                                    <xs:sequence>
                                        <xs:element name="inlineChild" type="xs:string" />
                                    </xs:sequence>
                                </xs:complexType>
                            </xs:schema>"#;
        let mut parser = SchemaParser::from_reader(xml.as_bytes());
        let schema = parser.parse_schema().unwrap();

        let complex_type = &schema.complex_types[0];
        assert_eq!(
            complex_type,
            &ComplexType {
                name: Some("AnonymousChildrenType".to_string()),
                mixed: false,
                attributes: vec![],
                any_attribute: None,
                content: Some(ComplexTypeContent::ComplexContent(ComplexContent {
                    derivation: None,
                    particle: Some(Particle::Sequence {
                        children: vec![Particle::Element {
                            element: ElementParticle::Decl(ElementDecl {
                                name: "inlineChild".to_string(),
                                type_name: Some(QName {
                                    prefix: Some(NamespacePrefix::from("xs")),
                                    local_name: "string".to_string(),
                                }),
                                inline_type: None,
                            }),
                            occurs: Occurrence {
                                min: 1,
                                max: Some(1)
                            },
                        }],
                        occurs: Occurrence {
                            min: 1,
                            max: Some(1)
                        },
                    }),
                })),
            }
        );
    }

    #[test]
    fn test_parse_complex_type_with_referenced_children() {
        let xml = r#"<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema"
                                targetNamespace="http://example.com"
                                xmlns="http://example.com">
                                <xs:element name="Child" type="xs:string" />
                                <xs:complexType name="RefChildrenType">
                                    <xs:sequence>
                                        <xs:element ref="Child" />
                                    </xs:sequence>
                                </xs:complexType>
                            </xs:schema>"#;
        let mut parser = SchemaParser::from_reader(xml.as_bytes());
        let schema = parser.parse_schema().unwrap();

        let complex_type = &schema.complex_types[0];
        assert_eq!(
            complex_type,
            &ComplexType {
                name: Some("RefChildrenType".to_string()),
                mixed: false,
                attributes: vec![],
                any_attribute: None,
                content: Some(ComplexTypeContent::ComplexContent(ComplexContent {
                    derivation: None,
                    particle: Some(Particle::Sequence {
                        children: vec![Particle::Element {
                            element: ElementParticle::Ref(QName {
                                prefix: None,
                                local_name: "Child".to_string(),
                            }),
                            occurs: Occurrence {
                                min: 1,
                                max: Some(1)
                            },
                        }],
                        occurs: Occurrence {
                            min: 1,
                            max: Some(1)
                        },
                    }),
                })),
            }
        );
    }

    #[test]
    fn test_parse_complex_type_with_direct_sequence_and_attribute() {
        let xml = r#"<xs:schema xmlns:xs="http://www.w3.org/2001/XMLSchema"
                                targetNamespace="http://example.com"
                                xmlns="http://example.com">
                                <xs:complexType name="SequenceWithAttr">
                                    <xs:sequence>
                                        <xs:element ref="child" />
                                    </xs:sequence>
                                    <xs:attribute ref="lang" />
                                </xs:complexType>
                            </xs:schema>"#;
        let mut parser = SchemaParser::from_reader(xml.as_bytes());
        let schema = parser.parse_schema().unwrap();

        let complex_type = &schema.complex_types[0];
        assert_eq!(
            complex_type,
            &ComplexType {
                name: Some("SequenceWithAttr".to_string()),
                mixed: false,
                attributes: vec![AttributeUse {
                    ref_name: "lang".to_string(),
                    required: false,
                }],
                any_attribute: None,
                content: Some(ComplexTypeContent::ComplexContent(ComplexContent {
                    derivation: None,
                    particle: Some(Particle::Sequence {
                        children: vec![Particle::Element {
                            element: ElementParticle::Ref(QName {
                                prefix: None,
                                local_name: "child".to_string(),
                            }),
                            occurs: Occurrence {
                                min: 1,
                                max: Some(1)
                            },
                        }],
                        occurs: Occurrence {
                            min: 1,
                            max: Some(1)
                        },
                    }),
                })),
            }
        );
    }
}
