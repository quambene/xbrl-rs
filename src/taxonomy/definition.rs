use crate::error::{LinkbaseType, Result, XbrlError};
use quick_xml::{
    Reader,
    events::{Event, attributes::Attributes},
};
use std::collections::HashMap;

/// A dimensional relationship from a definition linkbase.
#[derive(Debug, Clone, PartialEq)]
pub struct DefinitionArc {
    /// Source concept element ID.
    pub from: String,
    /// Target concept element ID.
    pub to: String,
    /// Display/processing order.
    pub order: Option<f64>,
    /// The arc role URI (e.g., `http://xbrl.org/int/dim/arcrole/domain-member`).
    pub arcrole: String,
}

/// Parse a definition linkbase XML file.
///
/// Returns a map from role URI (the `xlink:role` on `<definitionLink>`)
/// to a list of [`DefinitionArc`]s.
pub fn parse_definition_linkbase(xml_content: &str) -> Result<HashMap<String, Vec<DefinitionArc>>> {
    let mut reader = Reader::from_str(xml_content);
    reader.config_mut().trim_text_start = true;
    reader.config_mut().trim_text_end = true;

    let mut result: HashMap<String, Vec<DefinitionArc>> = HashMap::new();
    let mut current_role = String::new();
    let mut locators: HashMap<String, String> = HashMap::new();
    let mut arcs: Vec<RawDefArc> = Vec::new();

    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let local = local_name(&name);

                if local == "definitionLink" {
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
                    "definitionArc" => {
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

                if local == "definitionLink" {
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
                    linkbase_type: LinkbaseType::Definition,
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

struct RawDefArc {
    from: String,
    to: String,
    order: Option<f64>,
    arcrole: String,
}

fn resolve_arcs(locators: &HashMap<String, String>, arcs: &[RawDefArc]) -> Vec<DefinitionArc> {
    arcs.iter()
        .filter_map(|arc| {
            let from = locators.get(&arc.from)?;
            let to = locators.get(&arc.to)?;
            Some(DefinitionArc {
                from: from.clone(),
                to: to.clone(),
                order: arc.order,
                arcrole: arc.arcrole.clone(),
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

fn parse_arc(attrs: Attributes) -> Option<RawDefArc> {
    let mut from = None;
    let mut to = None;
    let mut order = None;
    let mut arcrole = String::new();

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
            "arcrole" => {
                if let Ok(val) = attr.unescape_value() {
                    arcrole = val.to_string();
                }
            }
            _ => {}
        }
    }

    match (from, to) {
        (Some(from), Some(to)) => Some(RawDefArc {
            from,
            to,
            order,
            arcrole,
        }),
        _ => None,
    }
}
