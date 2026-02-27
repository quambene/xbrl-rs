use crate::{
    error::{LinkbaseType, Result, XbrlError},
    taxonomy::split_qname,
};
use quick_xml::{
    Reader,
    events::{Event, attributes::Attributes},
};
use std::{collections::HashMap, io};

/// A regulatory/legal reference for a taxonomy concept.
#[derive(Debug, Clone, PartialEq)]
pub struct Reference {
    /// The reference role URI.
    pub role: String,
    /// The reference parts (e.g., Name="HGB", Paragraph="242").
    pub parts: Vec<ReferencePart>,
}

/// A single key-value part within a reference.
#[derive(Debug, Clone, PartialEq)]
pub struct ReferencePart {
    /// The part name (local element name, e.g., "Name", "Paragraph").
    pub name: String,
    /// The part value (text content, e.g., "HGB", "242").
    pub value: String,
}

enum ReferenceTag {
    Loc,
    Reference,
    ReferenceArc,
    Unknown,
}

impl ReferenceTag {
    fn from_name(name: &[u8]) -> Self {
        match split_qname(name).local_name {
            "loc" => Self::Loc,
            "reference" => Self::Reference,
            "referenceArc" => Self::ReferenceArc,
            _ => Self::Unknown,
        }
    }
}

/// Parse a reference linkbase XML file.
///
/// Returns a map from concept element ID to a list of [`Reference`]s.
/// Follows the same loc → arc → resource pattern as label linkbases.
pub fn parse_reference_linkbase(
    reader: &mut Reader<impl io::BufRead>,
) -> Result<HashMap<String, Vec<Reference>>> {
    reader.config_mut().trim_text_start = true;
    reader.config_mut().trim_text_end = true;

    let mut locators: HashMap<String, String> = HashMap::new();
    let mut arcs: Vec<RawRefArc> = Vec::new();
    let mut resources: HashMap<String, RawReference> = HashMap::new();

    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let tag = ReferenceTag::from_name(e.name().as_ref());

                match tag {
                    ReferenceTag::Loc => {
                        parse_loc(e.attributes(), &mut locators);
                    }
                    ReferenceTag::Reference => {
                        parse_reference_resource(reader, e.attributes(), &mut resources);
                    }
                    _ => {}
                }
            }
            Ok(Event::Empty(ref e)) => {
                let tag = ReferenceTag::from_name(e.name().as_ref());

                match tag {
                    ReferenceTag::Loc => {
                        parse_loc(e.attributes(), &mut locators);
                    }
                    ReferenceTag::ReferenceArc => {
                        if let Some(arc) = parse_arc(e.attributes()) {
                            arcs.push(arc);
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => {
                return Err(XbrlError::LinkbaseParse {
                    linkbase_type: LinkbaseType::Reference,
                    file_path: None,
                    source: err,
                });
            }
            _ => {}
        }
        buf.clear();
    }

    // Resolve: loc → arc → resource
    let mut result: HashMap<String, Vec<Reference>> = HashMap::new();
    for arc in &arcs {
        let Some(concept_id) = locators.get(&arc.from) else {
            continue;
        };
        let Some(resource) = resources.get(&arc.to) else {
            continue;
        };
        result
            .entry(concept_id.clone())
            .or_default()
            .push(Reference {
                role: resource.role.clone(),
                parts: resource.parts.clone(),
            });
    }

    Ok(result)
}

struct RawRefArc {
    from: String,
    to: String,
}

struct RawReference {
    role: String,
    parts: Vec<ReferencePart>,
}

fn local_name(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}

fn parse_loc(attrs: Attributes, locators: &mut HashMap<String, String>) {
    let mut href = None;
    let mut label = None;

    for attr in attrs.flatten() {
        let key = String::from_utf8_lossy(attr.key.as_ref());
        let local = local_name(&key);
        match local {
            "href" => {
                if let Ok(val) = attr.unescape_value()
                    && let Some(fragment) = val.split('#').nth(1)
                {
                    href = Some(fragment.to_string());
                }
            }
            "label" => {
                label = attr.unescape_value().ok().map(|v| v.to_string());
            }
            _ => {}
        }
    }

    if let (Some(label), Some(concept_id)) = (label, href) {
        locators.insert(label, concept_id);
    }
}

fn parse_arc(attrs: Attributes) -> Option<RawRefArc> {
    let mut from = None;
    let mut to = None;

    for attr in attrs.flatten() {
        let key = String::from_utf8_lossy(attr.key.as_ref());
        let local = local_name(&key);
        match local {
            "from" => {
                from = attr.unescape_value().ok().map(|v| v.to_string());
            }
            "to" => {
                to = attr.unescape_value().ok().map(|v| v.to_string());
            }
            _ => {}
        }
    }

    match (from, to) {
        (Some(from), Some(to)) => Some(RawRefArc { from, to }),
        _ => None,
    }
}

/// Parse a `<reference>` resource element: extract attributes and read child parts.
fn parse_reference_resource(
    reader: &mut Reader<impl io::BufRead>,
    attrs: Attributes,
    resources: &mut HashMap<String, RawReference>,
) {
    let mut label_key = None;
    let mut role = String::new();

    for attr in attrs.flatten() {
        let key = String::from_utf8_lossy(attr.key.as_ref());
        let local = local_name(&key);
        match local {
            "label" => {
                label_key = attr.unescape_value().ok().map(|v| v.to_string());
            }
            "role" => {
                if let Ok(val) = attr.unescape_value() {
                    role = val.to_string();
                }
            }
            _ => {}
        }
    }

    // Read child elements (ref:Name, ref:Paragraph, etc.) until closing </reference>
    let mut parts = Vec::new();
    let mut buf = Vec::new();
    let mut current_part_name: Option<String> = None;
    let mut depth = 1u32;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                depth += 1;
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                current_part_name = Some(local_name(&name).to_string());
            }
            Ok(Event::Text(ref t)) => {
                if let Some(ref part_name) = current_part_name {
                    let value = String::from_utf8_lossy(t.as_ref()).to_string();
                    parts.push(ReferencePart {
                        name: part_name.clone(),
                        value,
                    });
                }
            }
            Ok(Event::End(_)) => {
                current_part_name = None;
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            _ => {}
        }
        buf.clear();
    }

    if let Some(key) = label_key {
        resources.insert(key, RawReference { role, parts });
    }
}
