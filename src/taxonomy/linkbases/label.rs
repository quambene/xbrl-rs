use crate::{
    error::{LinkbaseType, Result, XbrlError},
    taxonomy::split_qname,
};
use quick_xml::{Reader, events::Event};
use std::{collections::HashMap, io};

/// A human-readable label for a taxonomy concept.
#[derive(Debug, Clone, PartialEq)]
pub struct Label {
    /// The label role URI (e.g., `http://www.xbrl.org/2003/role/label`).
    pub role: String,
    /// The language code (e.g., "de", "en").
    pub lang: String,
    /// The label text.
    pub text: String,
}

/// Intermediate representation of a `<label>` resource element during parsing.
struct LabelResource {
    role: String,
    lang: String,
    text: String,
}

/// Intermediate representation of a `<labelArc>` element during parsing.
struct LabelArc {
    from: String,
    to: String,
}

enum LabelTag {
    Loc,
    LabelArc,
    Label,
    Unknown,
}

impl LabelTag {
    fn from_name(name: &[u8]) -> Result<Self> {
        Ok(match split_qname(name)?.local {
            "loc" => Self::Loc,
            "labelArc" => Self::LabelArc,
            "label" => Self::Label,
            _ => Self::Unknown,
        })
    }
}

/// Parse a label linkbase XML file and return concept labels.
///
/// Returns a map from concept element ID (the `xlink:href` fragment from `<loc>`)
/// to a list of [`Label`]s.
///
/// The parser follows the XBRL linkbase chain:
/// 1. `<loc>` elements map a locator label to a concept element ID
/// 2. `<labelArc>` elements connect locator labels to label resource labels
/// 3. `<label>` resource elements contain the actual text, role, and language
pub fn parse_label_linkbase(
    reader: &mut Reader<impl io::BufRead>,
) -> Result<HashMap<String, Vec<Label>>> {
    reader.config_mut().trim_text_start = true;
    reader.config_mut().trim_text_end = true;

    // Phase 1: Collect all locators, arcs, and label resources
    let mut locators: HashMap<String, String> = HashMap::new(); // loc_label -> concept_id
    let mut arcs: Vec<LabelArc> = Vec::new();
    let mut resources: HashMap<String, LabelResource> = HashMap::new(); // resource_label -> LabelResource

    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref e)) => {
                let tag = LabelTag::from_name(e.name().as_ref())?;

                match tag {
                    LabelTag::Loc => {
                        parse_loc(e.attributes(), &mut locators)?;
                    }
                    LabelTag::LabelArc => {
                        if let Some(arc) = parse_label_arc(e.attributes())? {
                            arcs.push(arc);
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Start(ref e)) => {
                let tag = LabelTag::from_name(e.name().as_ref())?;

                match tag {
                    LabelTag::Loc => {
                        parse_loc(e.attributes(), &mut locators)?;
                    }
                    LabelTag::LabelArc => {
                        if let Some(arc) = parse_label_arc(e.attributes())? {
                            arcs.push(arc);
                        }
                    }
                    LabelTag::Label => {
                        parse_label_resource(reader, e.attributes(), &mut resources)?;
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => {
                return Err(XbrlError::LinkbaseParse {
                    linkbase_type: LinkbaseType::Label,
                    file_path: None,
                    source: err,
                });
            }
            _ => {}
        }
        buf.clear();
    }

    // Phase 2: Resolve the chain: loc -> arc -> label resource
    let mut labels: HashMap<String, Vec<Label>> = HashMap::new();

    for arc in &arcs {
        let Some(concept_id) = locators.get(&arc.from) else {
            continue;
        };
        let Some(resource) = resources.get(&arc.to) else {
            continue;
        };
        labels.entry(concept_id.clone()).or_default().push(Label {
            role: resource.role.clone(),
            lang: resource.lang.clone(),
            text: resource.text.clone(),
        });
    }

    Ok(labels)
}

/// Parse a `<loc>` element's attributes into the locators map.
fn parse_loc(
    attrs: quick_xml::events::attributes::Attributes,
    locators: &mut HashMap<String, String>,
) -> Result<()> {
    let mut href = None;
    let mut label = None;

    for attr in attrs.flatten() {
        let local = split_qname(attr.key.as_ref())?.local;
        match local {
            "href" => {
                if let Ok(val) = attr.unescape_value() {
                    // Extract the fragment (concept element ID) after '#'
                    if let Some(fragment) = val.split('#').nth(1) {
                        href = Some(fragment.to_string());
                    }
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

    Ok(())
}

/// Parse a `<labelArc>` element's attributes.
fn parse_label_arc(attrs: quick_xml::events::attributes::Attributes) -> Result<Option<LabelArc>> {
    let mut from = None;
    let mut to = None;

    for attr in attrs.flatten() {
        let local = split_qname(attr.key.as_ref())?.local;
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

    Ok(match (from, to) {
        (Some(from), Some(to)) => Some(LabelArc { from, to }),
        _ => None,
    })
}

/// Parse a `<label>` resource element: extract attributes and read the text content.
fn parse_label_resource(
    reader: &mut Reader<impl io::BufRead>,
    attrs: quick_xml::events::attributes::Attributes,
    resources: &mut HashMap<String, LabelResource>,
) -> Result<()> {
    let mut label_key = None;
    let mut role = String::new();
    let mut lang = String::new();

    for attr in attrs.flatten() {
        let local = split_qname(attr.key.as_ref())?.local;
        match local {
            "label" => {
                label_key = attr.unescape_value().ok().map(|v| v.to_string());
            }
            "role" => {
                if let Ok(val) = attr.unescape_value() {
                    role = val.to_string();
                }
            }
            "lang" => {
                if let Ok(val) = attr.unescape_value() {
                    lang = val.to_string();
                }
            }
            _ => {}
        }
    }

    // Read the text content until the closing </label> tag
    let mut text = String::new();
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Text(ref t)) => {
                if let Ok(decoded) = str::from_utf8(t.as_ref()) {
                    text.push_str(decoded);
                }
            }
            Ok(Event::End(_)) => break,
            Ok(Event::Eof) => break,
            _ => {}
        }
        buf.clear();
    }

    if let Some(key) = label_key {
        resources.insert(key, LabelResource { role, lang, text });
    }

    Ok(())
}
