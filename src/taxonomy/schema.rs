use crate::error::{Result, XbrlError};
use quick_xml::{
    Reader,
    events::{Event, attributes::Attributes},
};
use std::{
    collections::HashMap,
    io,
    path::{Path, PathBuf},
};

/// An `xs:element` definition from a taxonomy schema.
#[derive(Debug, Clone)]
pub struct ElementDefinition {
    /// The element's local name (e.g., "bs.ass.fixAss").
    pub name: String,
    /// The element's id attribute (e.g., "de-gaap-ci_bs.ass.fixAss").
    pub id: Option<String>,
    /// The XSD type (e.g., "xbrli:monetaryItemType").
    pub type_name: Option<String>,
    /// Substitution group (e.g., "xbrli:item", "xbrli:tuple").
    pub substitution_group: Option<String>,
    /// Whether this element is nillable.
    pub nillable: bool,
    /// Whether this element is abstract.
    pub is_abstract: bool,
    /// The XBRL period type ("instant" or "duration").
    pub period_type: Option<String>,
    /// The XBRL balance type ("debit" or "credit").
    pub balance: Option<String>,
}

/// A `link:roleType` definition from a taxonomy schema.
#[derive(Debug, Clone)]
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
#[derive(Debug, Clone)]
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
    pub cycles_allowed: Option<String>,
}

/// A `link:linkbaseRef` found in a schema's `xs:annotation/xs:appinfo`.
#[derive(Debug, Clone)]
pub struct LinkbaseRef {
    /// The xlink:href value (relative path to the linkbase file).
    pub href: String,
    /// The xlink:role (e.g., <http://www.xbrl.org/2003/role/labelLinkbaseRef>).
    pub role: Option<String>,
    /// The xlink:arcrole (typically <http://www.w3.org/1999/xlink/properties/linkbase>).
    pub arcrole: Option<String>,
    /// The xlink:title.
    pub title: Option<String>,
}

/// An `xs:import` reference in a schema.
#[derive(Debug, Clone)]
pub struct SchemaImport {
    pub namespace: String,
    pub schema_location: Option<String>,
}

/// An `xs:include` reference in a schema.
#[derive(Debug, Clone)]
pub struct SchemaInclude {
    pub schema_location: String,
}

/// A parsed taxonomy schema (.xsd) file.
#[derive(Debug)]
pub struct TaxonomySchema {
    /// Absolute file path of this schema.
    pub file_path: PathBuf,
    /// The targetNamespace of this schema.
    pub target_namespace: Option<String>,
    /// Namespace declarations (prefix -> URI).
    pub namespaces: HashMap<String, String>,
    /// `xs:import` references.
    pub imports: Vec<SchemaImport>,
    /// `xs:include` references.
    pub includes: Vec<SchemaInclude>,
    /// `link:linkbaseRef` entries.
    pub linkbase_refs: Vec<LinkbaseRef>,
    /// Locations referenced by `xsi:schemaLocation` and
    /// `xsi:noNamespaceSchemaLocation` attributes.
    pub schema_location_refs: Vec<String>,
    /// `link:roleType` definitions.
    pub role_types: Vec<RoleType>,
    /// `link:arcroleType` definitions.
    pub arcrole_types: Vec<ArcroleType>,
    /// `xs:element` definitions.
    pub elements: Vec<ElementDefinition>,
}

impl TaxonomySchema {
    /// Parse a taxonomy schema from an XML reader.
    pub fn from_xml<R: io::BufRead>(path: &Path, reader: &mut Reader<R>) -> Result<Self> {
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
        };

        let mut buf = Vec::new();
        let mut inside_appinfo = false;

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) => {
                    collect_schema_location_refs(e.attributes(), &mut schema.schema_location_refs);

                    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    let local = local_name(&name);

                    match local {
                        "schema" => {
                            extract_schema_attrs(e.attributes(), &mut schema);
                        }
                        "appinfo" => {
                            inside_appinfo = true;
                        }
                        "roleType" if inside_appinfo => {
                            schema
                                .role_types
                                .push(parse_role_type(reader, e.attributes())?);
                        }
                        "arcroleType" if inside_appinfo => {
                            schema
                                .arcrole_types
                                .push(parse_arcrole_type(reader, e.attributes())?);
                        }
                        "element" => {
                            if let Some(elem) = parse_element_def(e.attributes()) {
                                schema.elements.push(elem);
                            }
                            skip_to_end(reader, &name)?;
                        }
                        _ => {}
                    }
                }
                Ok(Event::Empty(ref e)) => {
                    collect_schema_location_refs(e.attributes(), &mut schema.schema_location_refs);

                    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    let local = local_name(&name);

                    match local {
                        "linkbaseRef" if inside_appinfo => {
                            schema
                                .linkbase_refs
                                .push(parse_linkbase_ref(e.attributes()));
                        }
                        "import" => {
                            if let Some(imp) = parse_import(e.attributes()) {
                                schema.imports.push(imp);
                            }
                        }
                        "include" => {
                            if let Some(inc) = parse_include(e.attributes()) {
                                schema.includes.push(inc);
                            }
                        }
                        "element" => {
                            if let Some(elem) = parse_element_def(e.attributes()) {
                                schema.elements.push(elem);
                            }
                        }
                        _ => {}
                    }
                }
                Ok(Event::End(ref e)) => {
                    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    if local_name(&name) == "appinfo" {
                        inside_appinfo = false;
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

        Ok(schema)
    }
}

/// Extract the local name from a possibly prefixed XML name.
fn local_name(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}

/// Extract targetNamespace and xmlns:* declarations from the xs:schema element.
fn extract_schema_attrs(attrs: Attributes, schema: &mut TaxonomySchema) {
    for attr in attrs.flatten() {
        let key = String::from_utf8_lossy(attr.key.as_ref());
        if *key == *"targetNamespace" {
            schema.target_namespace = attr.unescape_value().ok().map(|v| v.to_string());
        } else if let Some(prefix) = key.strip_prefix("xmlns:") {
            let uri = String::from_utf8_lossy(&attr.value).to_string();
            schema.namespaces.insert(prefix.to_string(), uri);
        }
    }
}

/// Parse a `link:linkbaseRef` element.
fn parse_linkbase_ref(attrs: Attributes) -> LinkbaseRef {
    let mut href = String::new();
    let mut role = None;
    let mut arcrole = None;
    let mut title = None;

    for attr in attrs.flatten() {
        let key = String::from_utf8_lossy(attr.key.as_ref());
        let local = local_name(&key);
        match local {
            "href" => {
                href = attr
                    .unescape_value()
                    .map(|v| v.to_string())
                    .unwrap_or_default();
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

    LinkbaseRef {
        href,
        role,
        arcrole,
        title,
    }
}

/// Parse a `link:roleType` element and its children.
fn parse_role_type<R: io::BufRead>(reader: &mut Reader<R>, attrs: Attributes) -> Result<RoleType> {
    let mut id = String::new();
    let mut role_uri = String::new();

    for attr in attrs.flatten() {
        let key = String::from_utf8_lossy(attr.key.as_ref());
        match key.as_ref() {
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
    let mut buf = Vec::new();
    let mut depth = 1;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                depth += 1;
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let local = local_name(&name);

                if local == "definition" || local == "usedOn" {
                    let mut text_buf = Vec::new();
                    if let Ok(Event::Text(t)) = reader.read_event_into(&mut text_buf) {
                        let text = String::from_utf8_lossy(t.as_ref()).to_string();
                        if local == "definition" {
                            definition = Some(text);
                        } else {
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
) -> Result<ArcroleType> {
    let mut id = String::new();
    let mut arcrole_uri = String::new();
    let mut cycles_allowed = None;

    for attr in attrs.flatten() {
        let key = String::from_utf8_lossy(attr.key.as_ref());
        match key.as_ref() {
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
                cycles_allowed = attr.unescape_value().ok().map(|v| v.to_string());
            }
            _ => {}
        }
    }

    let mut definition = None;
    let mut used_on = Vec::new();
    let mut buf = Vec::new();
    let mut depth = 1;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                depth += 1;
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let local = local_name(&name);

                if local == "definition" || local == "usedOn" {
                    let mut text_buf = Vec::new();
                    if let Ok(Event::Text(t)) = reader.read_event_into(&mut text_buf) {
                        let text = String::from_utf8_lossy(t.as_ref()).to_string();
                        if local == "definition" {
                            definition = Some(text);
                        } else {
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
fn parse_import(attrs: Attributes) -> Option<SchemaImport> {
    let mut namespace = None;
    let mut schema_location = None;

    for attr in attrs.flatten() {
        let key = String::from_utf8_lossy(attr.key.as_ref());
        match key.as_ref() {
            "namespace" => {
                namespace = attr.unescape_value().ok().map(|v| v.to_string());
            }
            "schemaLocation" => {
                schema_location = attr.unescape_value().ok().map(|v| v.to_string());
            }
            _ => {}
        }
    }

    namespace.map(|ns| SchemaImport {
        namespace: ns,
        schema_location,
    })
}

/// Parse an `xs:include` element.
fn parse_include(attrs: Attributes) -> Option<SchemaInclude> {
    for attr in attrs.flatten() {
        let key = String::from_utf8_lossy(attr.key.as_ref());
        if *key == *"schemaLocation"
            && let Ok(val) = attr.unescape_value()
        {
            return Some(SchemaInclude {
                schema_location: val.to_string(),
            });
        }
    }
    None
}

/// Parse an `xs:element` definition's attributes.
fn parse_element_def(attrs: Attributes) -> Option<ElementDefinition> {
    let mut name = None;
    let mut id = None;
    let mut type_name = None;
    let mut substitution_group = None;
    let mut nillable = false;
    let mut is_abstract = false;
    let mut period_type = None;
    let mut balance = None;

    for attr in attrs.flatten() {
        let key = String::from_utf8_lossy(attr.key.as_ref());
        let attr_local = local_name(&key);

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
                period_type = attr.unescape_value().ok().map(|v| v.to_string());
            }
            "balance" => {
                balance = attr.unescape_value().ok().map(|v| v.to_string());
            }
            _ => {}
        }
    }

    name.map(|n| ElementDefinition {
        name: n,
        id,
        type_name,
        substitution_group,
        nillable,
        is_abstract,
        period_type,
        balance,
    })
}

fn collect_schema_location_refs(attrs: Attributes, out: &mut Vec<String>) {
    for attr in attrs.flatten() {
        let key = String::from_utf8_lossy(attr.key.as_ref());
        let attr_local = local_name(&key);

        if attr_local != "schemaLocation" && attr_local != "noNamespaceSchemaLocation" {
            continue;
        }

        let value = String::from_utf8_lossy(attr.value.as_ref());
        for location in parse_schema_location_value(&value) {
            let trimmed = location.trim();
            if !trimmed.is_empty() && !out.iter().any(|existing| existing == trimmed) {
                out.push(trimmed.to_string());
            }
        }
    }
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
