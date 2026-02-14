use anyhow::Result;
use quick_xml::{
    Reader,
    events::{Event, attributes::Attributes},
};
use std::collections::HashMap;

/// A summation-item relationship from a calculation linkbase.
#[derive(Debug, Clone, PartialEq)]
pub struct CalculationArc {
    /// Parent (summation) concept element ID.
    pub from: String,
    /// Child (contributing item) concept element ID.
    pub to: String,
    /// Display order among siblings.
    pub order: Option<f64>,
    /// Weight factor (typically 1.0 or -1.0).
    pub weight: f64,
}

/// Parse a calculation linkbase XML file.
///
/// Returns a map from role URI (the `xlink:role` on `<calculationLink>`)
/// to a list of [`CalculationArc`]s.
pub fn parse_calculation_linkbase(
    xml_content: &str,
) -> Result<HashMap<String, Vec<CalculationArc>>> {
    let mut reader = Reader::from_str(xml_content);
    reader.config_mut().trim_text_start = true;
    reader.config_mut().trim_text_end = true;

    let mut result: HashMap<String, Vec<CalculationArc>> = HashMap::new();
    let mut current_role = String::new();
    let mut locators: HashMap<String, String> = HashMap::new();
    let mut arcs: Vec<RawCalcArc> = Vec::new();

    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let local = local_name(&name);

                if local == "calculationLink" {
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
                    "calculationArc" => {
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

                if local == "calculationLink" {
                    let resolved = resolve_arcs(&locators, &arcs);
                    result
                        .entry(current_role.clone())
                        .or_default()
                        .extend(resolved);
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(anyhow::anyhow!("Error parsing calculation linkbase: {}", e)),
            _ => {}
        }
        buf.clear();
    }

    Ok(result)
}

struct RawCalcArc {
    from: String,
    to: String,
    order: Option<f64>,
    weight: f64,
}

fn resolve_arcs(locators: &HashMap<String, String>, arcs: &[RawCalcArc]) -> Vec<CalculationArc> {
    arcs.iter()
        .filter_map(|arc| {
            let from = locators.get(&arc.from)?;
            let to = locators.get(&arc.to)?;
            Some(CalculationArc {
                from: from.clone(),
                to: to.clone(),
                order: arc.order,
                weight: arc.weight,
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

fn parse_arc(attrs: Attributes) -> Option<RawCalcArc> {
    let mut from = None;
    let mut to = None;
    let mut order = None;
    let mut weight = 1.0;

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
            "weight" => {
                if let Ok(val) = attr.unescape_value() {
                    weight = val.parse::<f64>().unwrap_or(1.0);
                }
            }
            _ => {}
        }
    }

    match (from, to) {
        (Some(from), Some(to)) => Some(RawCalcArc {
            from,
            to,
            order,
            weight,
        }),
        _ => None,
    }
}
