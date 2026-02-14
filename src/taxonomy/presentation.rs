use anyhow::Result;
use quick_xml::{
    Reader,
    events::{Event, attributes::Attributes},
};
use std::collections::HashMap;

/// A parent-child relationship from a presentation linkbase.
#[derive(Debug, Clone, PartialEq)]
pub struct PresentationArc {
    /// Parent concept element ID.
    pub from: String,
    /// Child concept element ID.
    pub to: String,
    /// Display order among siblings.
    pub order: Option<f64>,
}

/// Parse a presentation linkbase XML file.
///
/// Returns a map from role URI (the `xlink:role` on `<presentationLink>`)
/// to a list of [`PresentationArc`]s.
pub fn parse_presentation_linkbase(
    xml_content: &str,
) -> Result<HashMap<String, Vec<PresentationArc>>> {
    let mut reader = Reader::from_str(xml_content);
    reader.config_mut().trim_text_start = true;
    reader.config_mut().trim_text_end = true;

    let mut result: HashMap<String, Vec<PresentationArc>> = HashMap::new();
    let mut current_role = String::new();
    let mut locators: HashMap<String, String> = HashMap::new();
    let mut arcs: Vec<RawArc> = Vec::new();

    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let local = local_name(&name);

                if local == "presentationLink" {
                    // Start a new link group — reset per-link state
                    current_role = extract_role(e.attributes());
                    locators.clear();
                    arcs.clear();
                }
            }
            Ok(Event::Empty(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let local = local_name(&name);

                match local {
                    "loc" => {
                        parse_loc(e.attributes(), &mut locators);
                    }
                    "presentationArc" => {
                        if let Some(arc) = parse_arc(e.attributes()) {
                            arcs.push(arc);
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let local = local_name(&name);

                if local == "presentationLink" {
                    // Resolve and flush arcs for this link
                    let resolved = resolve_arcs(&locators, &arcs);
                    result
                        .entry(current_role.clone())
                        .or_default()
                        .extend(resolved);
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "Error parsing presentation linkbase: {}",
                    e
                ));
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
    order: Option<f64>,
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

fn local_name(name: &str) -> &str {
    name.rsplit(':').next().unwrap_or(name)
}

fn extract_role(attrs: Attributes) -> String {
    for attr in attrs.flatten() {
        let key = String::from_utf8_lossy(attr.key.as_ref());
        if local_name(&key) == "role" {
            if let Ok(val) = attr.unescape_value() {
                return val.to_string();
            }
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
                if let Ok(val) = attr.unescape_value() {
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
                order = attr
                    .unescape_value()
                    .ok()
                    .and_then(|v| v.parse::<f64>().ok());
            }
            _ => {}
        }
    }

    match (from, to) {
        (Some(from), Some(to)) => Some(RawArc { from, to, order }),
        _ => None,
    }
}
