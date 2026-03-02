use crate::{
    error::{Result, XbrlError},
    instance::Decimals,
    taxonomy::{
        schema::{
            ArcroleType, Concept, CyclesAllowed, DeclaredAccuracy, LinkbaseRef, MaxOccurs,
            RoleType, SchemaImport, SchemaInclude, TaxonomySchema, TupleChildRef,
        },
        split_qname,
    },
};
use quick_xml::{
    Reader,
    events::{Event, attributes::Attributes},
};
use std::{
    collections::{HashMap, HashSet},
    io,
    path::Path,
};

/// Parsed metadata for a named `xs:simpleType`/`xs:complexType` base relation.
struct NamedTypeBase {
    /// The declared type name (`@name`) on the type definition.
    type_name: String,
    /// The resolved `@base` QName from restriction/extension.
    base: Option<String>,
    /// Declared `decimals` value from type-level attribute constraints.
    declared_decimals: Option<Decimals>,
    /// Declared `precision` value from type-level attribute constraints.
    declared_precision: Option<Decimals>,
    /// Whether the named type declares local element content.
    has_local_element_content: bool,
}

enum SchemaTag {
    Schema,
    Annotation,
    Appinfo,
    RoleType,
    ArcroleType,
    Element,
    ComplexType,
    SimpleType,
    Redefine,
    IntegerElement,
    Linkbase,
    RoleRef,
    ArcroleRef,
    PresentationLink,
    CalculationLink,
    DefinitionLink,
    LabelLink,
    ReferenceLink,
    FootnoteLink,
    Loc,
    Label,
    Reference,
    Footnote,
    PresentationArc,
    CalculationArc,
    DefinitionArc,
    LabelArc,
    ReferenceArc,
    FootnoteArc,
    LinkbaseRef,
    Import,
    Include,
    Unknown(String),
}

impl SchemaTag {
    fn from_name(name: &[u8]) -> Result<Self> {
        let qname = split_qname(name)?;
        let _namespace = qname.namespace;
        Ok(Self::from_local_name(qname.local_name))
    }

    fn from_local_name(local: &str) -> Self {
        match local {
            "schema" => Self::Schema,
            "annotation" => Self::Annotation,
            "appinfo" => Self::Appinfo,
            "roleType" => Self::RoleType,
            "arcroleType" => Self::ArcroleType,
            "element" => Self::Element,
            "complexType" => Self::ComplexType,
            "simpleType" => Self::SimpleType,
            "redefine" => Self::Redefine,
            "integerElement" => Self::IntegerElement,
            "linkbase" => Self::Linkbase,
            "roleRef" => Self::RoleRef,
            "arcroleRef" => Self::ArcroleRef,
            "presentationLink" => Self::PresentationLink,
            "calculationLink" => Self::CalculationLink,
            "definitionLink" => Self::DefinitionLink,
            "labelLink" => Self::LabelLink,
            "referenceLink" => Self::ReferenceLink,
            "footnoteLink" => Self::FootnoteLink,
            "loc" => Self::Loc,
            "label" => Self::Label,
            "reference" => Self::Reference,
            "footnote" => Self::Footnote,
            "presentationArc" => Self::PresentationArc,
            "calculationArc" => Self::CalculationArc,
            "definitionArc" => Self::DefinitionArc,
            "labelArc" => Self::LabelArc,
            "referenceArc" => Self::ReferenceArc,
            "footnoteArc" => Self::FootnoteArc,
            "linkbaseRef" => Self::LinkbaseRef,
            "import" => Self::Import,
            "include" => Self::Include,
            _ => Self::Unknown(local.to_string()),
        }
    }

    fn local_name(&self) -> &str {
        match self {
            Self::Schema => "schema",
            Self::Annotation => "annotation",
            Self::Appinfo => "appinfo",
            Self::RoleType => "roleType",
            Self::ArcroleType => "arcroleType",
            Self::Element => "element",
            Self::ComplexType => "complexType",
            Self::SimpleType => "simpleType",
            Self::Redefine => "redefine",
            Self::IntegerElement => "integerElement",
            Self::Linkbase => "linkbase",
            Self::RoleRef => "roleRef",
            Self::ArcroleRef => "arcroleRef",
            Self::PresentationLink => "presentationLink",
            Self::CalculationLink => "calculationLink",
            Self::DefinitionLink => "definitionLink",
            Self::LabelLink => "labelLink",
            Self::ReferenceLink => "referenceLink",
            Self::FootnoteLink => "footnoteLink",
            Self::Loc => "loc",
            Self::Label => "label",
            Self::Reference => "reference",
            Self::Footnote => "footnote",
            Self::PresentationArc => "presentationArc",
            Self::CalculationArc => "calculationArc",
            Self::DefinitionArc => "definitionArc",
            Self::LabelArc => "labelArc",
            Self::ReferenceArc => "referenceArc",
            Self::FootnoteArc => "footnoteArc",
            Self::LinkbaseRef => "linkbaseRef",
            Self::Import => "import",
            Self::Include => "include",
            Self::Unknown(local) => local,
        }
    }
}

/// Parse a taxonomy schema from an XML reader.
pub(crate) fn read_schema<R: io::BufRead>(
    path: &Path,
    reader: &mut Reader<R>,
) -> Result<TaxonomySchema> {
    reader.config_mut().trim_text_start = true;
    reader.config_mut().trim_text_end = true;

    let mut schema = TaxonomySchema {
        file_path: path.to_path_buf(),
        target_namespace: None,
        namespaces: HashMap::new(),
        imports: Vec::new(),
        includes: Vec::new(),
        linkbase_refs: Vec::new(),
        schema_location_refs: Vec::new(),
        role_types: Vec::new(),
        arcrole_types: Vec::new(),
        elements: Vec::new(),
        type_bases: HashMap::new(),
        type_declared_accuracy: HashMap::new(),
    };

    let mut buf = Vec::new();
    let mut inside_appinfo = false;
    let mut annotation_base: Option<String> = None;
    let mut appinfo_base: Option<String> = None;
    let mut has_schema_root = false;
    let mut inside_linkbase_depth = 0u32;
    let mut linkbase_role_refs: HashSet<String> = HashSet::new();
    let mut linkbase_arcrole_refs: HashSet<String> = HashSet::new();
    let mut complex_types_with_local_elements: HashSet<String> = HashSet::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                collect_schema_location_refs(e.attributes(), &mut schema.schema_location_refs)?;

                let tag = SchemaTag::from_name(e.name().as_ref())?;

                if matches!(tag, SchemaTag::Redefine) {
                    return Err(XbrlError::InvalidSchemaDocument {
                        path: path.to_path_buf(),
                        reason: "xsd:redefine is not allowed in taxonomy schemas".to_string(),
                    });
                }

                match &tag {
                    SchemaTag::Schema => {
                        has_schema_root = true;
                        extract_schema_attrs(e.attributes(), &mut schema);
                    }
                    SchemaTag::Annotation => {
                        let local_base = attr_xml_base(e.attributes());
                        annotation_base = local_base;
                    }
                    SchemaTag::Appinfo => {
                        inside_appinfo = true;
                        let local_base = attr_xml_base(e.attributes());
                        appinfo_base = resolve_inherited_xml_base(
                            annotation_base.as_deref(),
                            local_base.as_deref(),
                        );
                    }
                    SchemaTag::LinkbaseRef if inside_appinfo => {
                        schema
                            .linkbase_refs
                            .push(parse_linkbase_ref(e.attributes(), appinfo_base.as_deref())?);
                    }
                    SchemaTag::RoleType if inside_appinfo => {
                        schema.role_types.push(parse_role_type(
                            reader,
                            e.attributes(),
                            path,
                            &schema.namespaces,
                        )?);
                    }
                    SchemaTag::ArcroleType if inside_appinfo => {
                        schema.arcrole_types.push(parse_arcrole_type(
                            reader,
                            e.attributes(),
                            path,
                            &schema.namespaces,
                        )?);
                    }
                    SchemaTag::Element => {
                        let elem = parse_element_def(e.attributes())?;
                        let tuple_decl = elem
                            .as_ref()
                            .and_then(|element| element.substitution_group.as_deref())
                            .is_some_and(|substitution_group| {
                                local_name(substitution_group) == "tuple"
                            });

                        if let Some(element) = elem {
                            schema.elements.push(element);
                        }
                        let element_name = e.name();
                        let tag_name = str::from_utf8(element_name.as_ref()).unwrap_or("");
                        let children =
                            skip_to_end_with_tuple_checks(reader, tag_name, tuple_decl, path)?;
                        if tuple_decl && let Some(elem) = schema.elements.last_mut() {
                            elem.tuple_children = children;
                        }
                    }
                    SchemaTag::ComplexType | SchemaTag::SimpleType => {
                        let element_name = e.name();
                        let tag_name = str::from_utf8(element_name.as_ref()).unwrap_or("");
                        if let Some(NamedTypeBase {
                            type_name,
                            base,
                            declared_decimals,
                            declared_precision,
                            has_local_element_content,
                        }) = parse_named_type_base(reader, e.attributes(), tag_name, path)?
                        {
                            if has_local_element_content {
                                complex_types_with_local_elements.insert(type_name.clone());
                            }
                            if let Some(base) = base {
                                schema.type_bases.insert(type_name.clone(), base);
                            }
                            schema.type_declared_accuracy.insert(
                                type_name,
                                DeclaredAccuracy {
                                    decimals: declared_decimals,
                                    precision: declared_precision,
                                },
                            );
                        }
                    }
                    _ => {}
                }

                if attr_by_local_name(e.attributes(), "integerAttribute")?
                    .is_some_and(|value| value.parse::<i64>().is_err())
                {
                    return Err(XbrlError::InvalidSchemaDocument {
                        path: path.to_path_buf(),
                        reason: "integerAttribute value is not a valid integer".to_string(),
                    });
                }

                if matches!(tag, SchemaTag::IntegerElement) {
                    if let Some(value) = attr_by_local_name(e.attributes(), "value")?
                        && value.parse::<i64>().is_err()
                    {
                        return Err(XbrlError::InvalidSchemaDocument {
                            path: path.to_path_buf(),
                            reason: "integerElement value is not a valid integer".to_string(),
                        });
                    }

                    let mut text_buf = Vec::new();
                    if let Ok(Event::Text(text)) = reader.read_event_into(&mut text_buf)
                        && let Ok(value) = str::from_utf8(text.as_ref()).map(str::trim)
                        && !value.is_empty()
                        && value.parse::<i64>().is_err()
                    {
                        return Err(XbrlError::InvalidSchemaDocument {
                            path: path.to_path_buf(),
                            reason: "integerElement value is not a valid integer".to_string(),
                        });
                    }
                }

                if inside_appinfo && matches!(tag, SchemaTag::Linkbase) {
                    inside_linkbase_depth = 1;
                    linkbase_role_refs.clear();
                    linkbase_arcrole_refs.clear();
                } else if inside_linkbase_depth > 0 {
                    inside_linkbase_depth += 1;
                    if !is_allowed_embedded_linkbase_element(&tag) {
                        return Err(XbrlError::InvalidSchemaDocument {
                            path: path.to_path_buf(),
                            reason: format!(
                                "embedded linkbase contains invalid element '{}'",
                                tag.local_name()
                            ),
                        });
                    }

                    if matches!(tag, SchemaTag::RoleRef)
                        && let Some(uri) = attr_by_local_name(e.attributes(), "roleURI")?
                        && !linkbase_role_refs.insert(uri.clone())
                    {
                        return Err(XbrlError::InvalidSchemaDocument {
                            path: path.to_path_buf(),
                            reason: format!("duplicate roleRef '{}' in embedded linkbase", uri),
                        });
                    }

                    if matches!(tag, SchemaTag::ArcroleRef)
                        && let Some(uri) = attr_by_local_name(e.attributes(), "arcroleURI")?
                        && !linkbase_arcrole_refs.insert(uri.clone())
                    {
                        return Err(XbrlError::InvalidSchemaDocument {
                            path: path.to_path_buf(),
                            reason: format!("duplicate arcroleRef '{}' in embedded linkbase", uri),
                        });
                    }
                }
            }
            Ok(Event::Empty(ref e)) => {
                collect_schema_location_refs(e.attributes(), &mut schema.schema_location_refs)?;

                let tag = SchemaTag::from_name(e.name().as_ref())?;

                if matches!(tag, SchemaTag::Redefine) {
                    return Err(XbrlError::InvalidSchemaDocument {
                        path: path.to_path_buf(),
                        reason: "xsd:redefine is not allowed in taxonomy schemas".to_string(),
                    });
                }

                match &tag {
                    SchemaTag::LinkbaseRef if inside_appinfo => {
                        schema
                            .linkbase_refs
                            .push(parse_linkbase_ref(e.attributes(), appinfo_base.as_deref())?);
                    }
                    SchemaTag::Import => {
                        if let Some(imp) = parse_import(e.attributes())? {
                            schema.imports.push(imp);
                        }
                    }
                    SchemaTag::Include => {
                        if let Some(inc) = parse_include(e.attributes())? {
                            schema.includes.push(inc);
                        }
                    }
                    SchemaTag::Element => {
                        if let Some(elem) = parse_element_def(e.attributes())? {
                            schema.elements.push(elem);
                        }
                    }
                    _ => {}
                }

                if attr_by_local_name(e.attributes(), "integerAttribute")?
                    .is_some_and(|value| value.parse::<i64>().is_err())
                {
                    return Err(XbrlError::InvalidSchemaDocument {
                        path: path.to_path_buf(),
                        reason: "integerAttribute value is not a valid integer".to_string(),
                    });
                }

                if matches!(tag, SchemaTag::IntegerElement)
                    && attr_by_local_name(e.attributes(), "value")?
                        .is_some_and(|value| value.parse::<i64>().is_err())
                {
                    return Err(XbrlError::InvalidSchemaDocument {
                        path: path.to_path_buf(),
                        reason: "integerElement value is not a valid integer".to_string(),
                    });
                }

                if inside_linkbase_depth > 0 {
                    if !is_allowed_embedded_linkbase_element(&tag) {
                        return Err(XbrlError::InvalidSchemaDocument {
                            path: path.to_path_buf(),
                            reason: format!(
                                "embedded linkbase contains invalid element '{}'",
                                tag.local_name()
                            ),
                        });
                    }

                    if matches!(tag, SchemaTag::RoleRef)
                        && let Some(uri) = attr_by_local_name(e.attributes(), "roleURI")?
                        && !linkbase_role_refs.insert(uri.clone())
                    {
                        return Err(XbrlError::InvalidSchemaDocument {
                            path: path.to_path_buf(),
                            reason: format!("duplicate roleRef '{}' in embedded linkbase", uri),
                        });
                    }

                    if matches!(tag, SchemaTag::ArcroleRef)
                        && let Some(uri) = attr_by_local_name(e.attributes(), "arcroleURI")?
                        && !linkbase_arcrole_refs.insert(uri.clone())
                    {
                        return Err(XbrlError::InvalidSchemaDocument {
                            path: path.to_path_buf(),
                            reason: format!("duplicate arcroleRef '{}' in embedded linkbase", uri),
                        });
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let tag = SchemaTag::from_name(e.name().as_ref())?;
                if matches!(tag, SchemaTag::Appinfo) {
                    inside_appinfo = false;
                    appinfo_base = None;
                }
                if matches!(tag, SchemaTag::Annotation) {
                    annotation_base = None;
                }

                if inside_linkbase_depth > 0 {
                    inside_linkbase_depth -= 1;
                    if inside_linkbase_depth == 0 {
                        linkbase_role_refs.clear();
                        linkbase_arcrole_refs.clear();
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => {
                return Err(XbrlError::XmlParse {
                    position: reader.buffer_position(),
                    element: Some(format!("schema {}", path.display())),
                    source: err,
                });
            }
            _ => {}
        }
        buf.clear();
    }

    if !has_schema_root {
        return Err(XbrlError::InvalidSchemaDocument {
            path: path.to_path_buf(),
            reason: "missing <schema> root element".to_string(),
        });
    }

    for element in &schema.elements {
        let substitution = element
            .substitution_group
            .as_deref()
            .map(local_name)
            .unwrap_or("");
        if substitution == "item"
            && element.type_name.as_deref().is_some_and(|type_name| {
                complex_types_with_local_elements.contains(local_name(type_name))
            })
        {
            return Err(XbrlError::InvalidSchemaDocument {
                path: path.to_path_buf(),
                reason: format!(
                    "item '{}' uses complex type '{}' with local element content",
                    element.name,
                    element.type_name.as_deref().unwrap_or_default()
                ),
            });
        }
    }

    Ok(schema)
}

fn skip_to_end_with_tuple_checks<R: io::BufRead>(
    reader: &mut Reader<R>,
    tag_name: &str,
    tuple_decl: bool,
    path: &Path,
) -> Result<Vec<TupleChildRef>> {
    let mut buf = Vec::new();
    let mut depth = 1u32;
    let mut children: Vec<TupleChildRef> = Vec::new();
    // Stack that tracks, for each open Start element, whether it is an `xs:choice`.
    // Used to suppress `min_occurs` for element refs inside a choice group, because
    // a choice requires only one of its alternatives — not all of them.
    let mut choice_stack: Vec<bool> = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                depth += 1;

                if tuple_decl {
                    let element_name = e.name();
                    let local = split_qname(element_name.as_ref())?.local_name;

                    choice_stack.push(local == "choice");

                    if (local == "complexType" || local == "complexContent")
                        && attr_by_local_name(e.attributes(), "mixed")?
                            .is_some_and(|mixed| mixed.eq_ignore_ascii_case("true"))
                    {
                        return Err(XbrlError::InvalidSchemaDocument {
                            path: path.to_path_buf(),
                            reason: "tuple declarations must not use mixed content".to_string(),
                        });
                    }

                    if local == "element"
                        && attr_by_local_name(e.attributes(), "name")?.is_some()
                        && attr_by_local_name(e.attributes(), "ref")?.is_none()
                    {
                        return Err(XbrlError::InvalidSchemaDocument {
                            path: path.to_path_buf(),
                            reason: "tuple content must reference global elements".to_string(),
                        });
                    }

                    if local == "attribute"
                        && attr_by_local_name(e.attributes(), "ref")?.is_some_and(|reference| {
                            reference.starts_with("xbrli:") || reference.starts_with("xlink:")
                        })
                    {
                        return Err(XbrlError::InvalidSchemaDocument {
                            path: path.to_path_buf(),
                            reason: "tuple declarations must not declare XBRL/XLink attribute refs"
                                .to_string(),
                        });
                    }
                }
            }
            Ok(Event::Empty(e)) => {
                if tuple_decl {
                    let element_name = e.name();
                    let local = split_qname(element_name.as_ref())?.local_name;

                    if local == "element"
                        && attr_by_local_name(e.attributes(), "name")?.is_some()
                        && attr_by_local_name(e.attributes(), "ref")?.is_none()
                    {
                        return Err(XbrlError::InvalidSchemaDocument {
                            path: path.to_path_buf(),
                            reason: "tuple content must reference global elements".to_string(),
                        });
                    }

                    if local == "element"
                        && let Some(qname) = attr_by_local_name(e.attributes(), "ref")?
                    {
                        // Inside xs:choice only one alternative is required; using min_occurs=0
                        // for individual elements avoids false "missing required child" errors.
                        let in_choice = choice_stack.iter().any(|&c| c);
                        let min_occurs = if in_choice {
                            0
                        } else {
                            attr_by_local_name(e.attributes(), "minOccurs")?
                                .and_then(|v| v.parse::<u32>().ok())
                                .unwrap_or(1)
                        };
                        let max_occurs = attr_by_local_name(e.attributes(), "maxOccurs")?
                            .map(|v| {
                                if v == "unbounded" {
                                    MaxOccurs::Unbounded
                                } else {
                                    v.parse::<u32>()
                                        .map(MaxOccurs::Bounded)
                                        .unwrap_or(MaxOccurs::Bounded(1))
                                }
                            })
                            .unwrap_or(MaxOccurs::Bounded(1));
                        children.push(TupleChildRef {
                            qname,
                            min_occurs,
                            max_occurs,
                        });
                    }

                    if local == "attribute"
                        && attr_by_local_name(e.attributes(), "ref")?.is_some_and(|reference| {
                            reference.starts_with("xbrli:") || reference.starts_with("xlink:")
                        })
                    {
                        return Err(XbrlError::InvalidSchemaDocument {
                            path: path.to_path_buf(),
                            reason: "tuple declarations must not declare XBRL/XLink attribute refs"
                                .to_string(),
                        });
                    }
                }
            }
            Ok(Event::End(_)) => {
                if tuple_decl {
                    choice_stack.pop();
                }
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => {
                return Err(XbrlError::XmlParse {
                    position: reader.buffer_position(),
                    element: Some(tag_name.to_string()),
                    source: err,
                });
            }
            _ => {}
        }
        buf.clear();
    }

    Ok(children)
}

fn normalize_qname(value: &str, namespaces: Option<&HashMap<String, String>>) -> String {
    let value = value.trim();
    if value.is_empty() {
        return String::new();
    }

    let Some(namespaces) = namespaces else {
        return value.to_string();
    };

    if let Some((prefix, local)) = value.split_once(':') {
        if let Some(uri) = namespaces.get(prefix) {
            return format!("{{{uri}}}{local}");
        }

        return value.to_string();
    }

    if let Some(uri) = namespaces.get("") {
        return format!("{{{uri}}}{value}");
    }

    value.to_string()
}

fn collect_namespace_declarations(attrs: Attributes, namespaces: &mut HashMap<String, String>) {
    for attr in attrs.flatten() {
        let Ok(key) = str::from_utf8(attr.key.as_ref()) else {
            continue;
        };
        if let Some(prefix) = key.strip_prefix("xmlns:") {
            if let Ok(value) = attr.unescape_value() {
                namespaces.insert(prefix.to_string(), value.to_string());
            }
        } else if key == "xmlns"
            && let Ok(value) = attr.unescape_value()
        {
            namespaces.insert("".to_string(), value.to_string());
        }
    }
}

fn is_allowed_embedded_linkbase_element(tag: &SchemaTag) -> bool {
    matches!(
        tag,
        SchemaTag::Linkbase
            | SchemaTag::RoleRef
            | SchemaTag::ArcroleRef
            | SchemaTag::PresentationLink
            | SchemaTag::CalculationLink
            | SchemaTag::DefinitionLink
            | SchemaTag::LabelLink
            | SchemaTag::ReferenceLink
            | SchemaTag::FootnoteLink
            | SchemaTag::Loc
            | SchemaTag::Label
            | SchemaTag::Reference
            | SchemaTag::Footnote
            | SchemaTag::PresentationArc
            | SchemaTag::CalculationArc
            | SchemaTag::DefinitionArc
            | SchemaTag::LabelArc
            | SchemaTag::ReferenceArc
            | SchemaTag::FootnoteArc
    )
}

fn attr_by_local_name(attrs: Attributes, expected_local: &str) -> Result<Option<String>> {
    for attr in attrs.flatten() {
        if split_qname(attr.key.as_ref())?.local_name == expected_local {
            return Ok(attr.unescape_value().ok().map(|value| value.to_string()));
        }
    }

    Ok(None)
}

fn parse_named_type_base<R: io::BufRead>(
    reader: &mut Reader<R>,
    attrs: Attributes,
    type_tag_name: &str,
    path: &Path,
) -> Result<Option<NamedTypeBase>> {
    let mut type_name = None;
    for attr in attrs.flatten() {
        if split_qname(attr.key.as_ref())?.local_name == "name" {
            type_name = attr.unescape_value().ok().map(|v| v.to_string());
            break;
        }
    }

    let Some(type_name) = type_name else {
        skip_to_end(reader, type_tag_name)?;
        return Ok(None);
    };

    let mut base: Option<String> = None;
    let mut declared_decimals: Option<Decimals> = None;
    let mut declared_precision: Option<Decimals> = None;
    let mut has_local_element_content = false;
    let mut depth = 1u32;
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                depth += 1;
                let element_name = e.name();
                let local = split_qname(element_name.as_ref())?.local_name;
                if local == "restriction" || local == "extension" {
                    for attr in e.attributes().flatten() {
                        if split_qname(attr.key.as_ref())?.local_name == "base" {
                            base = attr.unescape_value().ok().map(|v| v.to_string());
                            break;
                        }
                    }
                } else if local == "attribute" {
                    let mut attr_name: Option<String> = None;
                    let mut fixed_value: Option<String> = None;
                    let mut default_value: Option<String> = None;
                    for attr in e.attributes().flatten() {
                        match split_qname(attr.key.as_ref())?.local_name {
                            "name" => {
                                attr_name = attr.unescape_value().ok().map(|v| v.to_string());
                            }
                            "fixed" => {
                                fixed_value = attr.unescape_value().ok().map(|v| v.to_string());
                            }
                            "default" => {
                                default_value = attr.unescape_value().ok().map(|v| v.to_string());
                            }
                            _ => {}
                        }
                    }
                    let declared_value = fixed_value.or(default_value);
                    if let Some(value) = declared_value {
                        match attr_name.as_deref() {
                            Some("decimals") => {
                                declared_decimals =
                                    Some(value.parse::<Decimals>().map_err(|e| {
                                        XbrlError::InvalidSchemaDocument {
                                            path: path.to_path_buf(),
                                            reason: e.to_string(),
                                        }
                                    })?);
                            }
                            Some("precision") => {
                                declared_precision =
                                    Some(value.parse::<Decimals>().map_err(|e| {
                                        XbrlError::InvalidSchemaDocument {
                                            path: path.to_path_buf(),
                                            reason: e.to_string(),
                                        }
                                    })?);
                            }
                            _ => {}
                        }
                    }
                } else if local == "element" {
                    let mut has_name = false;
                    for attr in e.attributes().flatten() {
                        if split_qname(attr.key.as_ref())?.local_name == "name" {
                            has_name = true;
                            break;
                        }
                    }
                    if has_name {
                        has_local_element_content = true;
                    }
                }
            }
            Ok(Event::Empty(e)) => {
                let element_name = e.name();
                let local = split_qname(element_name.as_ref())?.local_name;
                if local == "restriction" || local == "extension" {
                    for attr in e.attributes().flatten() {
                        if split_qname(attr.key.as_ref())?.local_name == "base" {
                            base = attr.unescape_value().ok().map(|v| v.to_string());
                            break;
                        }
                    }
                } else if local == "attribute" {
                    let mut attr_name: Option<String> = None;
                    let mut fixed_value: Option<String> = None;
                    let mut default_value: Option<String> = None;
                    for attr in e.attributes().flatten() {
                        match split_qname(attr.key.as_ref())?.local_name {
                            "name" => {
                                attr_name = attr.unescape_value().ok().map(|v| v.to_string());
                            }
                            "fixed" => {
                                fixed_value = attr.unescape_value().ok().map(|v| v.to_string());
                            }
                            "default" => {
                                default_value = attr.unescape_value().ok().map(|v| v.to_string());
                            }
                            _ => {}
                        }
                    }
                    let declared_value = fixed_value.or(default_value);
                    if let Some(value) = declared_value {
                        match attr_name.as_deref() {
                            Some("decimals") => {
                                declared_decimals =
                                    Some(value.parse::<Decimals>().map_err(|e| {
                                        XbrlError::InvalidSchemaDocument {
                                            path: path.to_path_buf(),
                                            reason: e.to_string(),
                                        }
                                    })?);
                            }
                            Some("precision") => {
                                declared_precision =
                                    Some(value.parse::<Decimals>().map_err(|e| {
                                        XbrlError::InvalidSchemaDocument {
                                            path: path.to_path_buf(),
                                            reason: e.to_string(),
                                        }
                                    })?);
                            }
                            _ => {}
                        }
                    }
                } else if local == "element" {
                    let mut has_name = false;
                    for attr in e.attributes().flatten() {
                        if split_qname(attr.key.as_ref())?.local_name == "name" {
                            has_name = true;
                            break;
                        }
                    }
                    if has_name {
                        has_local_element_content = true;
                    }
                }
            }
            Ok(Event::End(_)) => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => {
                return Err(XbrlError::XmlParse {
                    position: reader.buffer_position(),
                    element: Some(type_tag_name.to_string()),
                    source: err,
                });
            }
            _ => {}
        }
        buf.clear();
    }

    Ok(Some(NamedTypeBase {
        type_name,
        base,
        declared_decimals,
        declared_precision,
        has_local_element_content,
    }))
}

/// Extract the local name from a possibly prefixed XML name.
fn local_name(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}

/// Extract targetNamespace and xmlns:* declarations from the xs:schema element.
fn extract_schema_attrs(attrs: Attributes, schema: &mut TaxonomySchema) {
    for attr in attrs.flatten() {
        let Ok(key) = str::from_utf8(attr.key.as_ref()) else {
            continue;
        };
        if key == "targetNamespace" {
            schema.target_namespace = attr.unescape_value().ok().map(|v| v.to_string());
        } else if let Some(prefix) = key.strip_prefix("xmlns:") {
            if let Ok(uri) = str::from_utf8(attr.value.as_ref()) {
                schema
                    .namespaces
                    .insert(prefix.to_string(), uri.to_string());
            }
        } else if key == "xmlns"
            && let Ok(uri) = str::from_utf8(attr.value.as_ref())
        {
            schema.namespaces.insert("".to_string(), uri.to_string());
        }
    }
}

/// Parse a `link:linkbaseRef` element.
fn parse_linkbase_ref(attrs: Attributes, inherited_base: Option<&str>) -> Result<LinkbaseRef> {
    let mut href = String::new();
    let mut local_xml_base = None;
    let mut role = None;
    let mut arcrole = None;
    let mut title = None;

    for attr in attrs.flatten() {
        let key = str::from_utf8(attr.key.as_ref()).ok();
        let local = split_qname(attr.key.as_ref())?.local_name;
        match local {
            "href" => {
                href = attr
                    .unescape_value()
                    .map(|v| v.to_string())
                    .unwrap_or_default();
            }
            "base" => {
                if key == Some("xml:base") {
                    local_xml_base = attr.unescape_value().ok().map(|v| v.to_string());
                }
            }
            "role" => {
                role = attr.unescape_value().ok().map(|v| v.to_string());
            }
            "arcrole" => {
                arcrole = attr.unescape_value().ok().map(|v| v.to_string());
            }
            "title" => {
                title = attr.unescape_value().ok().map(|v| v.to_string());
            }
            _ => {}
        }
    }

    let effective_base = resolve_inherited_xml_base(inherited_base, local_xml_base.as_deref());
    if let Some(base) = effective_base.as_deref() {
        href = resolve_href_with_xml_base(base, &href);
    }

    Ok(LinkbaseRef {
        href,
        role,
        arcrole,
        title,
    })
}

fn resolve_href_with_xml_base(xml_base: &str, href: &str) -> String {
    if href.is_empty() || href.contains("://") || href.starts_with('/') || href.starts_with('#') {
        return href.to_string();
    }

    let base = xml_base.trim();
    if base.is_empty() {
        return href.to_string();
    }

    let combined = if base.ends_with('/') {
        format!("{base}{href}")
    } else if let Some((parent, _)) = base.rsplit_once('/') {
        if parent.is_empty() {
            href.to_string()
        } else {
            format!("{parent}/{href}")
        }
    } else {
        href.to_string()
    };

    normalize_uri_path(&combined)
}

fn resolve_inherited_xml_base(inherited: Option<&str>, local: Option<&str>) -> Option<String> {
    match (inherited, local) {
        (Some(parent), Some(local_base)) => Some(resolve_href_with_xml_base(parent, local_base)),
        (None, Some(local_base)) => Some(normalize_uri_path(local_base.trim())),
        (Some(parent), None) => Some(parent.to_string()),
        (None, None) => None,
    }
}

fn normalize_uri_path(path: &str) -> String {
    let is_absolute = path.starts_with('/');
    let keep_trailing_slash = path.ends_with('/');

    let mut segments: Vec<&str> = Vec::new();
    for segment in path.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                if !segments.is_empty() {
                    segments.pop();
                }
            }
            _ => segments.push(segment),
        }
    }

    let mut normalized = String::new();
    if is_absolute {
        normalized.push('/');
    }

    normalized.push_str(&segments.join("/"));

    if keep_trailing_slash && !normalized.is_empty() && !normalized.ends_with('/') {
        normalized.push('/');
    }

    normalized
}

fn attr_xml_base(attrs: Attributes) -> Option<String> {
    for attr in attrs.flatten() {
        if str::from_utf8(attr.key.as_ref()).ok() == Some("xml:base") {
            return attr.unescape_value().ok().map(|v| v.to_string());
        }
    }
    None
}

/// Parse a `link:roleType` element and its children.
fn parse_role_type<R: io::BufRead>(
    reader: &mut Reader<R>,
    attrs: Attributes,
    path: &Path,
    schema_namespaces: &HashMap<String, String>,
) -> Result<RoleType> {
    let mut id = String::new();
    let mut role_uri = String::new();
    let mut role_type_namespaces = schema_namespaces.clone();

    collect_namespace_declarations(attrs.clone(), &mut role_type_namespaces);

    for attr in attrs.flatten() {
        match split_qname(attr.key.as_ref())?.local_name {
            "id" => {
                id = attr
                    .unescape_value()
                    .map(|v| v.to_string())
                    .unwrap_or_default();
            }
            "roleURI" => {
                role_uri = attr
                    .unescape_value()
                    .map(|v| v.to_string())
                    .unwrap_or_default();
            }
            _ => {}
        }
    }

    let mut definition = None;
    let mut used_on = Vec::new();
    let mut normalized_used_on: HashSet<String> = HashSet::new();
    let mut buf = Vec::new();
    let mut depth = 1;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                depth += 1;
                let element_name = e.name();
                let local = split_qname(element_name.as_ref())?.local_name;

                if local == "definition" || local == "usedOn" {
                    let mut used_on_namespaces = role_type_namespaces.clone();
                    collect_namespace_declarations(e.attributes(), &mut used_on_namespaces);
                    let mut text_buf = Vec::new();
                    if let Ok(Event::Text(t)) = reader.read_event_into(&mut text_buf) {
                        let text = str::from_utf8(t.as_ref()).unwrap_or("").to_string();
                        if local == "definition" {
                            definition = Some(text);
                        } else {
                            let normalized = normalize_qname(&text, Some(&used_on_namespaces));
                            if !normalized.is_empty() && !normalized_used_on.insert(normalized) {
                                return Err(XbrlError::InvalidSchemaDocument {
                                    path: path.to_path_buf(),
                                    reason: "roleType contains duplicate s-equal usedOn values"
                                        .to_string(),
                                });
                            }
                            used_on.push(text);
                        }
                    }
                }
            }
            Ok(Event::End(_)) => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => {
                return Err(XbrlError::XmlParse {
                    position: reader.buffer_position(),
                    element: Some("roleType".to_string()),
                    source: err,
                });
            }
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

/// Parse a `link:arcroleType` element and its children.
fn parse_arcrole_type<R: io::BufRead>(
    reader: &mut Reader<R>,
    attrs: Attributes,
    path: &Path,
    schema_namespaces: &HashMap<String, String>,
) -> Result<ArcroleType> {
    let mut id = String::new();
    let mut arcrole_uri = String::new();
    let mut cycles_allowed = None;
    let mut arcrole_type_namespaces = schema_namespaces.clone();

    collect_namespace_declarations(attrs.clone(), &mut arcrole_type_namespaces);

    for attr in attrs.flatten() {
        match split_qname(attr.key.as_ref())?.local_name {
            "id" => {
                id = attr
                    .unescape_value()
                    .map(|v| v.to_string())
                    .unwrap_or_default();
            }
            "arcroleURI" => {
                arcrole_uri = attr
                    .unescape_value()
                    .map(|v| v.to_string())
                    .unwrap_or_default();
            }
            "cyclesAllowed" => {
                let value = attr
                    .unescape_value()
                    .map(|v| v.to_string())
                    .unwrap_or_default();
                cycles_allowed = Some(value.parse::<CyclesAllowed>().map_err(|e| {
                    XbrlError::InvalidSchemaDocument {
                        path: path.to_path_buf(),
                        reason: e.to_string(),
                    }
                })?);
            }
            _ => {}
        }
    }

    let mut definition = None;
    let mut used_on = Vec::new();
    let mut normalized_used_on: HashSet<String> = HashSet::new();
    let mut buf = Vec::new();
    let mut depth = 1;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                depth += 1;
                let element_name = e.name();
                let local = split_qname(element_name.as_ref())?.local_name;

                if local == "definition" || local == "usedOn" {
                    let mut used_on_namespaces = arcrole_type_namespaces.clone();
                    collect_namespace_declarations(e.attributes(), &mut used_on_namespaces);
                    let mut text_buf = Vec::new();
                    if let Ok(Event::Text(t)) = reader.read_event_into(&mut text_buf) {
                        let text = str::from_utf8(t.as_ref()).unwrap_or("").to_string();
                        if local == "definition" {
                            definition = Some(text);
                        } else {
                            let normalized = normalize_qname(&text, Some(&used_on_namespaces));
                            if !normalized.is_empty() && !normalized_used_on.insert(normalized) {
                                return Err(XbrlError::InvalidSchemaDocument {
                                    path: path.to_path_buf(),
                                    reason: "arcroleType contains duplicate s-equal usedOn values"
                                        .to_string(),
                                });
                            }
                            used_on.push(text);
                        }
                    }
                }
            }
            Ok(Event::End(_)) => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => {
                return Err(XbrlError::XmlParse {
                    position: reader.buffer_position(),
                    element: Some("arcroleType".to_string()),
                    source: err,
                });
            }
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

/// Parse an `xs:import` element.
fn parse_import(attrs: Attributes) -> Result<Option<SchemaImport>> {
    let mut namespace = None;
    let mut schema_location = None;

    for attr in attrs.flatten() {
        match split_qname(attr.key.as_ref())?.local_name {
            "namespace" => {
                namespace = attr.unescape_value().ok().map(|v| v.to_string());
            }
            "schemaLocation" => {
                schema_location = attr.unescape_value().ok().map(|v| v.to_string());
            }
            _ => {}
        }
    }

    Ok(namespace.map(|ns| SchemaImport {
        namespace: ns,
        schema_location,
    }))
}

/// Parse an `xs:include` element.
fn parse_include(attrs: Attributes) -> Result<Option<SchemaInclude>> {
    for attr in attrs.flatten() {
        if split_qname(attr.key.as_ref())?.local_name == "schemaLocation"
            && let Ok(val) = attr.unescape_value()
        {
            return Ok(Some(SchemaInclude {
                schema_location: val.to_string(),
            }));
        }
    }
    Ok(None)
}

/// Parse an `xs:element` definition's attributes.
fn parse_element_def(attrs: Attributes) -> Result<Option<Concept>> {
    let mut name = None;
    let mut id = None;
    let mut type_name = None;
    let mut substitution_group = None;
    let mut nillable = false;
    let mut is_abstract = false;
    let mut period_type = None;
    let mut balance = None;

    for attr in attrs.flatten() {
        let attr_local = split_qname(attr.key.as_ref())?.local_name;

        match attr_local {
            "name" => {
                name = attr.unescape_value().ok().map(|v| v.to_string());
            }
            "id" => {
                id = attr.unescape_value().ok().map(|v| v.to_string());
            }
            "type" => {
                type_name = attr.unescape_value().ok().map(|v| v.to_string());
            }
            "substitutionGroup" => {
                substitution_group = attr.unescape_value().ok().map(|v| v.to_string());
            }
            "nillable" => {
                nillable = attr
                    .unescape_value()
                    .ok()
                    .is_some_and(|v| v.as_ref() == "true");
            }
            "abstract" => {
                is_abstract = attr
                    .unescape_value()
                    .ok()
                    .is_some_and(|v| v.as_ref() == "true");
            }
            "periodType" => {
                period_type = attr.unescape_value().ok().and_then(|v| v.parse().ok());
            }
            "balance" => {
                balance = attr.unescape_value().ok().and_then(|v| v.parse().ok());
            }
            _ => {}
        }
    }

    Ok(name.map(|n| Concept {
        name: n,
        id,
        type_name,
        substitution_group,
        nillable,
        is_abstract,
        period_type,
        balance,
        tuple_children: Vec::new(),
    }))
}

fn collect_schema_location_refs(attrs: Attributes, out: &mut Vec<String>) -> Result<()> {
    for attr in attrs.flatten() {
        let attr_local = split_qname(attr.key.as_ref())?.local_name;

        if attr_local != "schemaLocation" && attr_local != "noNamespaceSchemaLocation" {
            continue;
        }

        if let Ok(value) = str::from_utf8(attr.value.as_ref()) {
            for location in parse_schema_location_value(value) {
                let trimmed = location.trim();
                if !trimmed.is_empty() && !out.iter().any(|existing| existing == trimmed) {
                    out.push(trimmed.to_string());
                }
            }
        }
    }

    Ok(())
}

fn parse_schema_location_value(value: &str) -> Vec<&str> {
    let tokens: Vec<&str> = value.split_whitespace().collect();

    if tokens.is_empty() {
        return Vec::new();
    }

    if tokens.len() == 1 {
        return tokens;
    }

    if tokens.len().is_multiple_of(2) {
        return tokens.into_iter().skip(1).step_by(2).collect();
    }

    tokens
}

/// Skip past the end tag of the current element.
fn skip_to_end<R: io::BufRead>(reader: &mut Reader<R>, tag_name: &str) -> Result<()> {
    let mut buf = Vec::new();
    let mut depth = 1u32;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(_)) => depth += 1,
            Ok(Event::End(_)) => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => {
                return Err(XbrlError::XmlParse {
                    position: reader.buffer_position(),
                    element: Some(tag_name.to_string()),
                    source: err,
                });
            }
            _ => {}
        }
        buf.clear();
    }

    Ok(())
}
