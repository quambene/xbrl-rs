use crate::{
    error::{LinkbaseType, Result, XbrlError},
    taxonomy::split_qname,
};
use quick_xml::{
    Reader,
    events::{Event, attributes::Attributes},
};
use rust_decimal::Decimal;
use std::{collections::HashMap, io, str::FromStr};

/// A dimensional relationship from a definition linkbase.
#[derive(Debug, Clone, PartialEq)]
pub struct DefinitionArc {
    /// Source concept element ID.
    pub from: String,
    /// Target concept element ID.
    pub to: String,
    /// Display/processing order.
    pub order: Option<Decimal>,
    /// The arc role URI (e.g., `http://xbrl.org/int/dim/arcrole/domain-member`).
    pub arcrole: String,
}

enum DefinitionTag {
    DefinitionLink,
    Loc,
    DefinitionArc,
    Unknown,
}

impl DefinitionTag {
    fn from_name(name: &[u8]) -> Result<Self> {
        Ok(match split_qname(name)?.local {
            "definitionLink" => Self::DefinitionLink,
            "loc" => Self::Loc,
            "definitionArc" => Self::DefinitionArc,
            _ => Self::Unknown,
        })
    }
}

/// Parse a definition linkbase XML file.
///
/// Returns a map from role URI (the `xlink:role` on `<definitionLink>`)
/// to a list of [`DefinitionArc`]s.
pub fn parse_definition_linkbase(
    reader: &mut Reader<impl io::BufRead>,
) -> Result<HashMap<String, Vec<DefinitionArc>>> {
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
                let tag = DefinitionTag::from_name(e.name().as_ref())?;

                if matches!(tag, DefinitionTag::DefinitionLink) {
                    current_role = extract_role(e.attributes())?;
                    locators.clear();
                    arcs.clear();
                }
            }
            Ok(Event::Empty(ref e)) => {
                let tag = DefinitionTag::from_name(e.name().as_ref())?;

                match tag {
                    DefinitionTag::Loc => {
                        parse_loc(e.attributes(), &mut locators)?;
                    }
                    DefinitionTag::DefinitionArc => {
                        if let Some(arc) = parse_arc(e.attributes())? {
                            arcs.push(arc);
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) => {
                let tag = DefinitionTag::from_name(e.name().as_ref())?;

                if matches!(tag, DefinitionTag::DefinitionLink) {
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
    order: Option<Decimal>,
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

fn extract_role(attrs: Attributes) -> Result<String> {
    for attr in attrs.flatten() {
        if split_qname(attr.key.as_ref())?.local == "role"
            && let Ok(val) = attr.unescape_value()
        {
            return Ok(val.to_string());
        }
    }
    Ok(String::new())
}

fn parse_loc(attrs: Attributes, locators: &mut HashMap<String, String>) -> Result<()> {
    let mut href = None;
    let mut label = None;

    for attr in attrs.flatten() {
        let local = split_qname(attr.key.as_ref())?.local;
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

    Ok(())
}

fn parse_arc(attrs: Attributes) -> Result<Option<RawDefArc>> {
    let mut from = None;
    let mut to = None;
    let mut order = None;
    let mut arcrole = String::new();

    for attr in attrs.flatten() {
        let local = split_qname(attr.key.as_ref())?.local;
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
            "arcrole" => {
                if let Ok(val) = attr.unescape_value() {
                    arcrole = val.to_string();
                }
            }
            _ => {}
        }
    }

    Ok(match (from, to) {
        (Some(from), Some(to)) => Some(RawDefArc {
            from,
            to,
            order,
            arcrole,
        }),
        _ => None,
    })
}
