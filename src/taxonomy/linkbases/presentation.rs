use crate::{
    error::{LinkbaseType, Result, XbrlError},
    taxonomy::{linkbases::local_name, split_qname},
};
use indexmap::IndexMap;
use quick_xml::{
    Reader,
    events::{Event, attributes::Attributes},
};
use rust_decimal::Decimal;
use std::{collections::HashMap, io, str::FromStr};

/// A parent-child relationship from a presentation linkbase.
#[derive(Debug, Clone, PartialEq)]
pub struct PresentationArc {
    /// Parent concept element ID.
    pub from: String,
    /// Child concept element ID.
    pub to: String,
    /// Display order among siblings.
    pub order: Option<Decimal>,
}

enum PresentationTag {
    PresentationLink,
    Loc,
    PresentationArc,
    Unknown,
}

impl PresentationTag {
    fn from_name(name: &[u8]) -> Self {
        match split_qname(name).local_name {
            "presentationLink" => Self::PresentationLink,
            "loc" => Self::Loc,
            "presentationArc" => Self::PresentationArc,
            _ => Self::Unknown,
        }
    }
}

/// Parse a presentation linkbase XML file.
///
/// Returns a map from role URI (the `xlink:role` on `<presentationLink>`)
/// to a list of [`PresentationArc`]s.
pub fn parse_presentation_linkbase(
    reader: &mut Reader<impl io::BufRead>,
) -> Result<IndexMap<String, Vec<PresentationArc>>> {
    reader.config_mut().trim_text_start = true;
    reader.config_mut().trim_text_end = true;

    let mut result: IndexMap<String, Vec<PresentationArc>> = IndexMap::new();
    let mut current_role = String::new();
    let mut locators: HashMap<String, String> = HashMap::new();
    let mut arcs: Vec<RawArc> = Vec::new();

    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let tag = PresentationTag::from_name(e.name().as_ref());

                if matches!(tag, PresentationTag::PresentationLink) {
                    // Start a new link group — reset per-link state
                    current_role = extract_role(e.attributes());
                    locators.clear();
                    arcs.clear();
                }
            }
            Ok(Event::Empty(ref e)) => {
                let tag = PresentationTag::from_name(e.name().as_ref());

                match tag {
                    PresentationTag::Loc => {
                        parse_loc(e.attributes(), &mut locators);
                    }
                    PresentationTag::PresentationArc => {
                        if let Some(arc) = parse_arc(e.attributes()) {
                            arcs.push(arc);
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) => {
                let tag = PresentationTag::from_name(e.name().as_ref());

                if matches!(tag, PresentationTag::PresentationLink) {
                    // Resolve and flush arcs for this link
                    let resolved = resolve_arcs(&locators, &arcs);
                    result
                        .entry(current_role.clone())
                        .or_default()
                        .extend(resolved);
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => {
                return Err(XbrlError::LinkbaseParse {
                    linkbase_type: LinkbaseType::Presentation,
                    file_path: None,
                    source: err,
                });
            }
            _ => {}
        }
        buf.clear();
    }

    Ok(result)
}

struct RawArc {
    from: String,
    to: String,
    order: Option<Decimal>,
}

fn resolve_arcs(locators: &HashMap<String, String>, arcs: &[RawArc]) -> Vec<PresentationArc> {
    arcs.iter()
        .filter_map(|arc| {
            let from = locators.get(&arc.from)?;
            let to = locators.get(&arc.to)?;
            Some(PresentationArc {
                from: from.clone(),
                to: to.clone(),
                order: arc.order,
            })
        })
        .collect()
}

fn extract_role(attrs: Attributes) -> String {
    for attr in attrs.flatten() {
        let key = String::from_utf8_lossy(attr.key.as_ref());
        if local_name(&key) == "role"
            && let Ok(val) = attr.unescape_value()
        {
            return val.to_string();
        }
    }
    String::new()
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

fn parse_arc(attrs: Attributes) -> Option<RawArc> {
    let mut from = None;
    let mut to = None;
    let mut order = None;

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
            "order" => {
                order = attr.unescape_value().ok().and_then(|v| {
                    Decimal::from_str(&v)
                        .ok()
                        .or_else(|| Decimal::from_scientific(&v).ok())
                });
            }
            _ => {}
        }
    }

    match (from, to) {
        (Some(from), Some(to)) => Some(RawArc { from, to, order }),
        _ => None,
    }
}
