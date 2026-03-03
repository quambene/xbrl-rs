use crate::XbrlError;
use quick_xml::{Reader, events::Event};
use std::{
    collections::HashMap,
    io::BufRead,
    path::{Path, PathBuf},
};

/// Represents the different XML tags that can appear in an XBRL schema
/// document.
enum SchemaTag {
    /// The root <xs:schema> element.
    Schema,
    /// An <xs:import> definition.
    Import,
    /// An <xs:include> definition.
    Include,
    /// An <xs:element> definition.
    Element,
    /// An <xs:simpleType> definition.
    SimpleType,
    /// An <xs:complexType> definition.
    ComplexType,
    /// An `xs:restriction` element (used in complex type definitions).
    Restriction,
    /// An `xs:extension` element (used in complex type definitions).
    Extension,
    /// An `xs:sequence` element (used in complex type definitions).
    Sequence,
    /// An `xs:choice` element (used in complex type definitions).
    Choice,
    /// An `xs:attribute` element (used in type definitions).
    Attribute,
    /// An `xs:annotation` element (can contain `xs:appinfo`).
    Annotation,
    /// An `xs:appinfo` element.
    Appinfo,
    /// An `xs:redefine` element (not allowed in XBRL taxonomies).
    Redefine,
}

impl SchemaTag {
    fn from_name(bytes: &[u8]) -> Result<Self, XbrlError> {
        match bytes {
            b"xs:schema" => Ok(Self::Schema),
            b"xs:import" => Ok(Self::Import),
            b"xs:include" => Ok(Self::Include),
            b"xs:element" => Ok(Self::Element),
            b"xs:simpleType" => Ok(Self::SimpleType),
            b"xs:complexType" => Ok(Self::ComplexType),
            b"xs:restriction" => Ok(Self::Restriction),
            b"xs:extension" => Ok(Self::Extension),
            b"xs:sequence" => Ok(Self::Sequence),
            b"xs:choice" => Ok(Self::Choice),
            b"xs:attribute" => Ok(Self::Attribute),
            b"xs:annotation" => Ok(Self::Annotation),
            b"xs:appinfo" => Ok(Self::Appinfo),
            b"xs:redefine" => Ok(Self::Redefine),
            _ => Err(XbrlError::ParseError {
                expected: "SchemaTag",
                value: String::from_utf8_lossy(bytes).to_string(),
            }),
        }
    }

    fn from_local_name(bytes: &[u8]) -> Result<Self, XbrlError> {
        match bytes {
            b"schema" => Ok(Self::Schema),
            b"import" => Ok(Self::Import),
            b"include" => Ok(Self::Include),
            b"element" => Ok(Self::Element),
            b"simpleType" => Ok(Self::SimpleType),
            b"complexType" => Ok(Self::ComplexType),
            b"restriction" => Ok(Self::Restriction),
            b"extension" => Ok(Self::Extension),
            b"sequence" => Ok(Self::Sequence),
            b"choice" => Ok(Self::Choice),
            b"attribute" => Ok(Self::Attribute),
            b"annotation" => Ok(Self::Annotation),
            b"appinfo" => Ok(Self::Appinfo),
            b"redefine" => Ok(Self::Redefine),
            _ => Err(XbrlError::ParseError {
                expected: "SchemaTag",
                value: String::from_utf8_lossy(bytes).to_string(),
            }),
        }
    }

    fn local_name(&self) -> &'static str {
        match self {
            Self::Schema => "schema",
            Self::Import => "import",
            Self::Include => "include",
            Self::Element => "element",
            Self::SimpleType => "simpleType",
            Self::ComplexType => "complexType",
            Self::Restriction => "restriction",
            Self::Extension => "extension",
            Self::Sequence => "sequence",
            Self::Choice => "choice",
            Self::Attribute => "attribute",
            Self::Annotation => "annotation",
            Self::Appinfo => "appinfo",
            Self::Redefine => "redefine",
        }
    }
}

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

/// Represents a raw parsed XBRL schema. Contains only the syntax-level data; no
/// resolved `Concept`s yet.
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
    /// Parsed simple/complex type definitions.
    pub types: Vec<TypeDefinition>,
}

/// A parsed XML element from the schema (xs:element).
pub struct Element {
    /// The element's local name (e.g., "bs.ass.fixAss").
    pub name: String,
    /// The element's id attribute (optional in XBRL).
    pub id: Option<String>,
    /// The type QName (e.g., "xbrli:monetaryItemType").
    pub type_name: Option<String>,
    /// Substitution group (e.g., "xbrli:item", "xbrli:tuple").
    pub substitution_group: Option<String>,
    /// Whether this element is nillable.
    pub is_nillable: bool,
    /// Whether this element is abstract.
    pub is_abstract: bool,
    /// The XBRL period type ("instant" or "duration").
    pub period_type: Option<String>,
    /// The XBRL balance ("debit" or "credit").
    pub balance: Option<String>,
}

/// A simple/complex type definition from the schema.
pub struct TypeDefinition {
    /// The type's name.
    pub name: String,
    /// The base type name, if any.
    pub base: Option<String>,
    /// Additional restrictions or facets can be stored as key-value pairs.
    pub restrictions: std::collections::HashMap<String, String>,
}

/// Represents an `xs:import` in the schema.
pub struct SchemaImport {
    /// Namespace being imported.
    pub namespace: String,
    /// Location of the imported schema file (from schemaLocation).
    pub schema_location: Option<String>,
}

/// Represents an `xs:include` in the schema.
pub struct SchemaInclude {
    /// Location of the included schema file.
    pub schema_location: String,
}

/// Represents a `link:linkbaseRef` in the schema.
pub struct LinkbaseRef {
    /// Href to the linkbase file.
    pub href: String,
    /// Role type of the linkbase (optional).
    pub role: Option<String>,
    /// Type of the linkbase (extended/simple).
    pub link_type: Option<String>,
}

pub struct SchemaParser<R: BufRead> {
    reader: Reader<R>,
}

impl<R: BufRead> SchemaParser<R> {
    pub fn new(reader: R) -> Self {
        Self {
            reader: Reader::from_reader(reader),
        }
    }

    pub fn parse_schema(&mut self, path: &Path) -> Result<RawSchema, XbrlError> {
        let mut buf = Vec::new();

        loop {
            match self.reader.read_event_into(&mut buf) {
                Ok(quick_xml::events::Event::Start(ref event)) => {
                    let tag = SchemaTag::from_name(event.name().as_ref())?;

                    if matches!(tag, SchemaTag::Redefine) {
                        return Err(XbrlError::InvalidSchemaDocument {
                            path: path.to_path_buf(),
                            reason: "xsd:redefine is not allowed in taxonomy schemas".to_string(),
                        });
                    }

                    match tag {
                        SchemaTag::Schema => todo!(),
                        SchemaTag::Import => todo!(),
                        SchemaTag::Include => todo!(),
                        SchemaTag::Element => todo!(),
                        SchemaTag::SimpleType => todo!(),
                        SchemaTag::ComplexType => todo!(),
                        SchemaTag::Restriction => todo!(),
                        SchemaTag::Extension => todo!(),
                        SchemaTag::Sequence => todo!(),
                        SchemaTag::Choice => todo!(),
                        SchemaTag::Attribute => todo!(),
                        SchemaTag::Annotation => todo!(),
                        SchemaTag::Appinfo => todo!(),
                        SchemaTag::Redefine => todo!(),
                    }
                }
                Ok(Event::End(ref event)) => {
                    todo!()
                }
                Ok(Event::Eof) => break,
                Err(err) => {
                    return Err(XbrlError::XmlParse {
                        position: self.reader.buffer_position(),
                        element: Some(format!("schema {}", path.display())),
                        source: err,
                    });
                }
                _ => {}
            }
        }

        Ok(RawSchema {
            file_path: todo!(),
            namespaces: todo!(),
            target_namespace: todo!(),
            imports: todo!(),
            includes: todo!(),
            linkbase_refs: todo!(),
            elements: todo!(),
            types: todo!(),
        })
    }
}
