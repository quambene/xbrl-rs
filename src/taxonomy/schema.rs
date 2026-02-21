use crate::error::{Result, XbrlError};
use quick_xml::{
    Reader,
    events::{Event, attributes::Attributes},
};
use std::{
    collections::{HashMap, HashSet},
    fs, io,
    io::BufReader,
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
    /// Named simple/complex type derivations: type name -> base QName.
    pub type_bases: HashMap<String, String>,
    /// Named types with declared decimals/precision attributes (fixed/default) on restrictions.
    pub type_declared_accuracy: HashMap<String, (Option<String>, Option<String>)>,
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
            type_bases: HashMap::new(),
            type_declared_accuracy: HashMap::new(),
        };

        let mut buf = Vec::new();
        let mut inside_appinfo = false;
        let mut has_schema_root = false;

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) => {
                    collect_schema_location_refs(e.attributes(), &mut schema.schema_location_refs);

                    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    let local = local_name(&name);

                    match local {
                        "schema" => {
                            has_schema_root = true;
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
                        "complexType" | "simpleType" => {
                            if let Some((type_name, base, declared_decimals, declared_precision)) =
                                parse_named_type_base(reader, e.attributes(), &name)?
                            {
                                schema.type_bases.insert(type_name.clone(), base);
                                schema
                                    .type_declared_accuracy
                                    .insert(type_name, (declared_decimals, declared_precision));
                            }
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

        if !has_schema_root {
            return Err(XbrlError::InvalidSchemaDocument {
                path: path.to_path_buf(),
                reason: "missing <schema> root element".to_string(),
            });
        }

        schema.validate()?;

        Ok(schema)
    }

    /// Validate schema-level XBRL constraints used by conformance tests.
    pub fn validate(&self) -> Result<()> {
        if self
            .target_namespace
            .as_deref()
            .is_some_and(|ns| ns.trim().is_empty())
        {
            return invalid_schema(
                &self.file_path,
                "empty targetNamespace is not allowed".to_string(),
            );
        }

        for element in &self.elements {
            let substitution = element
                .substitution_group
                .as_deref()
                .map(local_name)
                .unwrap_or("");

            if substitution == "item"
                && element
                    .period_type
                    .as_deref()
                    .is_none_or(|period| period.trim().is_empty())
            {
                return invalid_schema(
                    &self.file_path,
                    format!("item '{}' is missing xbrli:periodType", element.name),
                );
            }

            if substitution == "tuple" && element.period_type.is_some() {
                return invalid_schema(
                    &self.file_path,
                    format!("tuple '{}' must not declare xbrli:periodType", element.name),
                );
            }

            if element.balance.is_some() {
                if substitution == "tuple" {
                    return invalid_schema(
                        &self.file_path,
                        format!("tuple '{}' must not declare xbrli:balance", element.name),
                    );
                }

                let is_monetary = element
                    .type_name
                    .as_deref()
                    .is_some_and(|type_name| self.is_monetary_type(type_name));

                if !is_monetary {
                    return invalid_schema(
                        &self.file_path,
                        format!(
                            "element '{}' has xbrli:balance but is not monetaryItemType-derived",
                            element.name
                        ),
                    );
                }
            }

            if substitution == "item"
                && element
                    .type_name
                    .as_deref()
                    .is_some_and(|type_name| self.is_known_complex_item_type(type_name))
            {
                return invalid_schema(
                    &self.file_path,
                    format!(
                        "item '{}' has unsupported complex content type",
                        element.name
                    ),
                );
            }
        }

        for role_type in &self.role_types {
            if !role_type.id.is_empty() && !is_ncname(&role_type.id) {
                return invalid_schema(
                    &self.file_path,
                    format!("roleType id '{}' is not an NCName", role_type.id),
                );
            }

            if role_type.role_uri.trim().is_empty() {
                return invalid_schema(&self.file_path, "roleType roleURI is required".to_string());
            }
        }

        for arcrole_type in &self.arcrole_types {
            if !arcrole_type.id.is_empty() && !is_ncname(&arcrole_type.id) {
                return invalid_schema(
                    &self.file_path,
                    format!("arcroleType id '{}' is not an NCName", arcrole_type.id),
                );
            }

            if arcrole_type.arcrole_uri.trim().is_empty() {
                return invalid_schema(
                    &self.file_path,
                    "arcroleType arcroleURI is required".to_string(),
                );
            }

            if !is_absolute_uri(&arcrole_type.arcrole_uri) {
                return invalid_schema(
                    &self.file_path,
                    format!(
                        "arcroleType arcroleURI '{}' is not an absolute URI",
                        arcrole_type.arcrole_uri
                    ),
                );
            }

            if arcrole_type
                .cycles_allowed
                .as_deref()
                .is_some_and(|value| value != "any" && value != "undirected" && value != "none")
            {
                return invalid_schema(
                    &self.file_path,
                    format!(
                        "arcroleType cyclesAllowed '{}' is invalid",
                        arcrole_type.cycles_allowed.as_deref().unwrap_or_default()
                    ),
                );
            }
        }

        if self.linkbase_refs.iter().any(|linkbase_ref| {
            linkbase_ref
                .role
                .as_deref()
                .is_some_and(|role| role.ends_with("/role/labelLinkbaseRef"))
        }) {
            for role_type in &self.role_types {
                if role_type
                    .used_on
                    .iter()
                    .any(|value| local_name(value) == "label")
                {
                    return invalid_schema(
                        &self.file_path,
                        "roleType usedOn label is not valid for standard label linkbase usage"
                            .to_string(),
                    );
                }
            }
        }

        self.validate_xml_structure()
    }

    fn is_monetary_type(&self, type_name: &str) -> bool {
        if local_name(type_name) == "monetaryItemType" {
            return true;
        }

        let mut current = type_name.to_string();
        let mut visited: HashSet<String> = HashSet::new();

        while visited.insert(current.clone()) {
            let Some(base) = self.type_bases.get(&current) else {
                return false;
            };

            if local_name(base) == "monetaryItemType" {
                return true;
            }

            current = base.clone();
        }

        false
    }

    fn is_known_complex_item_type(&self, type_name: &str) -> bool {
        let local = local_name(type_name);
        !local.ends_with("ItemType")
    }

    fn validate_xml_structure(&self) -> Result<()> {
        let xml_file = fs::File::open(&self.file_path).map_err(|source| XbrlError::FileRead {
            path: self.file_path.clone(),
            context: "schema".to_string(),
            source,
        })?;

        let mut reader = Reader::from_reader(BufReader::new(xml_file));
        reader.config_mut().trim_text_start = true;
        reader.config_mut().trim_text_end = true;

        let mut buf = Vec::new();
        let mut inside_appinfo = false;
        let mut inside_linkbase_depth = 0u32;
        let mut tuple_element_depth: Option<u32> = None;
        let mut xml_depth = 0u32;

        let mut linkbase_role_refs: HashSet<String> = HashSet::new();
        let mut linkbase_arcrole_refs: HashSet<String> = HashSet::new();
        let mut named_complex_type_stack: Vec<(Option<String>, u32)> = Vec::new();
        let mut complex_types_with_local_elements: HashSet<String> = HashSet::new();

        let mut namespace_stack: Vec<HashMap<String, String>> = vec![HashMap::new()];
        let mut element_name_stack: Vec<String> = Vec::new();
        let mut role_type_used_on: Option<HashSet<String>> = None;
        let mut arcrole_type_used_on: Option<HashSet<String>> = None;

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) => {
                    xml_depth += 1;

                    let mut scope = namespace_stack.last().cloned().unwrap_or_default();
                    merge_namespace_declarations(e.attributes(), &mut scope);
                    namespace_stack.push(scope);

                    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    let local = local_name(&name).to_string();
                    element_name_stack.push(local.clone());

                    if local == "redefine" {
                        return invalid_schema(
                            &self.file_path,
                            "xsd:redefine is not allowed in taxonomy schemas".to_string(),
                        );
                    }

                    if local == "appinfo" {
                        inside_appinfo = true;
                    }

                    if inside_appinfo && local == "roleType" {
                        role_type_used_on = Some(HashSet::new());
                    }
                    if inside_appinfo && local == "arcroleType" {
                        arcrole_type_used_on = Some(HashSet::new());
                    }

                    if local == "complexType" {
                        named_complex_type_stack
                            .push((attr_by_local_name(e.attributes(), "name"), xml_depth));
                    }

                    if inside_appinfo && local == "linkbase" {
                        inside_linkbase_depth = 1;
                        linkbase_role_refs.clear();
                        linkbase_arcrole_refs.clear();
                    } else if inside_linkbase_depth > 0 {
                        inside_linkbase_depth += 1;
                        if !is_allowed_embedded_linkbase_element(&local) {
                            return invalid_schema(
                                &self.file_path,
                                format!("embedded linkbase contains invalid element '{}'", local),
                            );
                        }

                        if local == "roleRef"
                            && let Some(uri) = attr_by_local_name(e.attributes(), "roleURI")
                            && !linkbase_role_refs.insert(uri.clone())
                        {
                            return invalid_schema(
                                &self.file_path,
                                format!("duplicate roleRef '{}' in embedded linkbase", uri),
                            );
                        }

                        if local == "arcroleRef"
                            && let Some(uri) = attr_by_local_name(e.attributes(), "arcroleURI")
                            && !linkbase_arcrole_refs.insert(uri.clone())
                        {
                            return invalid_schema(
                                &self.file_path,
                                format!("duplicate arcroleRef '{}' in embedded linkbase", uri),
                            );
                        }
                    }

                    if local == "element"
                        && attr_by_local_name(e.attributes(), "substitutionGroup").is_some_and(
                            |substitution_group| local_name(&substitution_group) == "tuple",
                        )
                    {
                        tuple_element_depth = Some(xml_depth);
                    }

                    if local == "element"
                        && let Some((Some(type_name), type_depth)) = named_complex_type_stack
                            .iter()
                            .rev()
                            .find(|(_, depth)| *depth < xml_depth)
                        && attr_by_local_name(e.attributes(), "name").is_some()
                        && xml_depth > *type_depth
                    {
                        complex_types_with_local_elements.insert(type_name.clone());
                    }

                    if let Some(tuple_depth) = tuple_element_depth
                        && xml_depth > tuple_depth
                    {
                        if (local == "complexType" || local == "complexContent")
                            && attr_by_local_name(e.attributes(), "mixed")
                                .is_some_and(|mixed| mixed.eq_ignore_ascii_case("true"))
                        {
                            return invalid_schema(
                                &self.file_path,
                                "tuple declarations must not use mixed content".to_string(),
                            );
                        }

                        if local == "element"
                            && attr_by_local_name(e.attributes(), "name").is_some()
                            && attr_by_local_name(e.attributes(), "ref").is_none()
                        {
                            return invalid_schema(
                                &self.file_path,
                                "tuple content must reference global elements".to_string(),
                            );
                        }

                        if local == "attribute"
                            && attr_by_local_name(e.attributes(), "ref").is_some_and(|reference| {
                                reference.starts_with("xbrli:") || reference.starts_with("xlink:")
                            })
                        {
                            return invalid_schema(
                                &self.file_path,
                                "tuple declarations must not declare XBRL/XLink attribute refs"
                                    .to_string(),
                            );
                        }
                    }

                    if attr_by_local_name(e.attributes(), "integerAttribute")
                        .is_some_and(|value| value.parse::<i64>().is_err())
                    {
                        return invalid_schema(
                            &self.file_path,
                            "integerAttribute value is not a valid integer".to_string(),
                        );
                    }

                    if local == "usedOn"
                        && let Ok(Event::Text(text)) = reader.read_event_into(&mut Vec::new())
                    {
                        let text = String::from_utf8_lossy(text.as_ref()).trim().to_string();
                        let normalized = normalize_qname(&text, namespace_stack.last());

                        if let Some(seen) = role_type_used_on.as_mut()
                            && !seen.insert(normalized.clone())
                        {
                            return invalid_schema(
                                &self.file_path,
                                "roleType contains s-equal usedOn elements".to_string(),
                            );
                        }

                        if let Some(seen) = arcrole_type_used_on.as_mut()
                            && !seen.insert(normalized)
                        {
                            return invalid_schema(
                                &self.file_path,
                                "arcroleType contains s-equal usedOn elements".to_string(),
                            );
                        }
                    }
                }
                Ok(Event::Empty(ref e)) => {
                    let mut scope = namespace_stack.last().cloned().unwrap_or_default();
                    merge_namespace_declarations(e.attributes(), &mut scope);

                    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    let local = local_name(&name).to_string();

                    if local == "redefine" {
                        return invalid_schema(
                            &self.file_path,
                            "xsd:redefine is not allowed in taxonomy schemas".to_string(),
                        );
                    }

                    if inside_linkbase_depth > 0 {
                        if !is_allowed_embedded_linkbase_element(&local) {
                            return invalid_schema(
                                &self.file_path,
                                format!("embedded linkbase contains invalid element '{}'", local),
                            );
                        }

                        if local == "roleRef"
                            && let Some(uri) = attr_by_local_name(e.attributes(), "roleURI")
                            && !linkbase_role_refs.insert(uri.clone())
                        {
                            return invalid_schema(
                                &self.file_path,
                                format!("duplicate roleRef '{}' in embedded linkbase", uri),
                            );
                        }

                        if local == "arcroleRef"
                            && let Some(uri) = attr_by_local_name(e.attributes(), "arcroleURI")
                            && !linkbase_arcrole_refs.insert(uri.clone())
                        {
                            return invalid_schema(
                                &self.file_path,
                                format!("duplicate arcroleRef '{}' in embedded linkbase", uri),
                            );
                        }
                    }

                    if let Some(tuple_depth) = tuple_element_depth
                        && xml_depth > tuple_depth
                    {
                        if local == "element"
                            && attr_by_local_name(e.attributes(), "name").is_some()
                            && attr_by_local_name(e.attributes(), "ref").is_none()
                        {
                            return invalid_schema(
                                &self.file_path,
                                "tuple content must reference global elements".to_string(),
                            );
                        }

                        if local == "attribute"
                            && attr_by_local_name(e.attributes(), "ref").is_some_and(|reference| {
                                reference.starts_with("xbrli:") || reference.starts_with("xlink:")
                            })
                        {
                            return invalid_schema(
                                &self.file_path,
                                "tuple declarations must not declare XBRL/XLink attribute refs"
                                    .to_string(),
                            );
                        }
                    }

                    if local == "element"
                        && let Some((Some(type_name), _type_depth)) =
                            named_complex_type_stack.iter().rev().next()
                        && attr_by_local_name(e.attributes(), "name").is_some()
                    {
                        complex_types_with_local_elements.insert(type_name.clone());
                    }

                    if attr_by_local_name(e.attributes(), "integerAttribute")
                        .is_some_and(|value| value.parse::<i64>().is_err())
                    {
                        return invalid_schema(
                            &self.file_path,
                            "integerAttribute value is not a valid integer".to_string(),
                        );
                    }

                    if local == "integerElement"
                        && attr_by_local_name(e.attributes(), "value")
                            .is_some_and(|value| value.parse::<i64>().is_err())
                    {
                        return invalid_schema(
                            &self.file_path,
                            "integerElement value is not a valid integer".to_string(),
                        );
                    }
                }
                Ok(Event::Text(text)) => {
                    if inside_appinfo
                        && element_name_stack
                            .last()
                            .is_some_and(|name| name == "integerElement")
                    {
                        let value = String::from_utf8_lossy(text.as_ref()).trim().to_string();
                        if value.parse::<i64>().is_err() {
                            return invalid_schema(
                                &self.file_path,
                                "integerElement value is not a valid integer".to_string(),
                            );
                        }
                    }
                }
                Ok(Event::End(ref e)) => {
                    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    let local = local_name(&name);

                    if local == "appinfo" {
                        inside_appinfo = false;
                    }

                    if local == "roleType" {
                        role_type_used_on = None;
                    }

                    if local == "arcroleType" {
                        arcrole_type_used_on = None;
                    }

                    if local == "complexType" {
                        named_complex_type_stack.pop();
                    }

                    if inside_linkbase_depth > 0 {
                        inside_linkbase_depth -= 1;
                        if inside_linkbase_depth == 0 {
                            linkbase_role_refs.clear();
                            linkbase_arcrole_refs.clear();
                        }
                    }

                    if tuple_element_depth.is_some_and(|tuple_depth| tuple_depth == xml_depth) {
                        tuple_element_depth = None;
                    }

                    xml_depth = xml_depth.saturating_sub(1);
                    namespace_stack.pop();
                    element_name_stack.pop();
                }
                Ok(Event::Eof) => break,
                Err(err) => {
                    return Err(XbrlError::XmlParse {
                        position: reader.buffer_position(),
                        element: Some(format!("schema {}", self.file_path.display())),
                        source: err,
                    });
                }
                _ => {}
            }
            buf.clear();
        }

        for element in &self.elements {
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
                return invalid_schema(
                    &self.file_path,
                    format!(
                        "item '{}' uses complex type '{}' with local element content",
                        element.name,
                        element.type_name.as_deref().unwrap_or_default()
                    ),
                );
            }
        }

        Ok(())
    }
}

fn invalid_schema<T>(path: &Path, reason: String) -> Result<T> {
    Err(XbrlError::InvalidSchemaDocument {
        path: path.to_path_buf(),
        reason,
    })
}

fn is_ncname(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };

    if !(first == '_' || first.is_ascii_alphabetic()) {
        return false;
    }

    chars.all(|ch| ch == '_' || ch == '-' || ch == '.' || ch.is_ascii_alphanumeric())
}

fn is_absolute_uri(value: &str) -> bool {
    if value.is_empty() || value.starts_with('#') {
        return false;
    }

    let Some((scheme, _rest)) = value.split_once(':') else {
        return false;
    };

    !scheme.is_empty()
        && scheme.chars().enumerate().all(|(index, ch)| {
            ch.is_ascii_alphabetic()
                || (index > 0 && matches!(ch, '+' | '-' | '.'))
                || (index > 0 && ch.is_ascii_digit())
        })
}

fn merge_namespace_declarations(attrs: Attributes, namespaces: &mut HashMap<String, String>) {
    for attr in attrs.flatten() {
        let key = String::from_utf8_lossy(attr.key.as_ref());
        if key == "xmlns" {
            if let Ok(value) = attr.unescape_value() {
                namespaces.insert(String::new(), value.to_string());
            }
        } else if let Some(prefix) = key.strip_prefix("xmlns:")
            && let Ok(value) = attr.unescape_value()
        {
            namespaces.insert(prefix.to_string(), value.to_string());
        }
    }
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

fn is_allowed_embedded_linkbase_element(local: &str) -> bool {
    matches!(
        local,
        "linkbase"
            | "roleRef"
            | "arcroleRef"
            | "presentationLink"
            | "calculationLink"
            | "definitionLink"
            | "labelLink"
            | "referenceLink"
            | "footnoteLink"
            | "loc"
            | "label"
            | "reference"
            | "footnote"
            | "presentationArc"
            | "calculationArc"
            | "definitionArc"
            | "labelArc"
            | "referenceArc"
            | "footnoteArc"
    )
}

fn attr_by_local_name(attrs: Attributes, expected_local: &str) -> Option<String> {
    for attr in attrs.flatten() {
        let key = String::from_utf8_lossy(attr.key.as_ref());
        if local_name(&key) == expected_local {
            return attr.unescape_value().ok().map(|value| value.to_string());
        }
    }

    None
}

fn parse_named_type_base<R: io::BufRead>(
    reader: &mut Reader<R>,
    attrs: Attributes,
    type_tag_name: &str,
) -> Result<Option<(String, String, Option<String>, Option<String>)>> {
    let mut type_name = None;
    for attr in attrs.flatten() {
        let key = String::from_utf8_lossy(attr.key.as_ref());
        if local_name(&key) == "name" {
            type_name = attr.unescape_value().ok().map(|v| v.to_string());
            break;
        }
    }

    let Some(type_name) = type_name else {
        skip_to_end(reader, type_tag_name)?;
        return Ok(None);
    };

    let mut base: Option<String> = None;
    let mut declared_decimals: Option<String> = None;
    let mut declared_precision: Option<String> = None;
    let mut depth = 1u32;
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                depth += 1;
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let local = local_name(&name);
                if local == "restriction" || local == "extension" {
                    for attr in e.attributes().flatten() {
                        let key = String::from_utf8_lossy(attr.key.as_ref());
                        if local_name(&key) == "base" {
                            base = attr.unescape_value().ok().map(|v| v.to_string());
                            break;
                        }
                    }
                } else if local == "attribute" {
                    let mut attr_name: Option<String> = None;
                    let mut fixed_value: Option<String> = None;
                    let mut default_value: Option<String> = None;
                    for attr in e.attributes().flatten() {
                        let key = String::from_utf8_lossy(attr.key.as_ref());
                        match local_name(&key) {
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
                            Some("decimals") => declared_decimals = Some(value),
                            Some("precision") => declared_precision = Some(value),
                            _ => {}
                        }
                    }
                }
            }
            Ok(Event::Empty(e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let local = local_name(&name);
                if local == "restriction" || local == "extension" {
                    for attr in e.attributes().flatten() {
                        let key = String::from_utf8_lossy(attr.key.as_ref());
                        if local_name(&key) == "base" {
                            base = attr.unescape_value().ok().map(|v| v.to_string());
                            break;
                        }
                    }
                } else if local == "attribute" {
                    let mut attr_name: Option<String> = None;
                    let mut fixed_value: Option<String> = None;
                    let mut default_value: Option<String> = None;
                    for attr in e.attributes().flatten() {
                        let key = String::from_utf8_lossy(attr.key.as_ref());
                        match local_name(&key) {
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
                            Some("decimals") => declared_decimals = Some(value),
                            Some("precision") => declared_precision = Some(value),
                            _ => {}
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
                    element: Some(type_tag_name.to_string()),
                    source: err,
                });
            }
            _ => {}
        }
        buf.clear();
    }

    Ok(base.map(|b| (type_name, b, declared_decimals, declared_precision)))
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
