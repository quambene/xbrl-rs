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

/// A summation-item relationship from a calculation linkbase.
#[derive(Debug, Clone, PartialEq)]
pub struct CalculationArc {
    /// Parent (summation) concept element ID.
    pub from: String,
    /// Child (contributing item) concept element ID.
    pub to: String,
    /// Display order among siblings.
    pub order: Option<Decimal>,
    /// Weight factor (typically 1.0 or -1.0).
    pub weight: Decimal,
}

enum CalculationTag {
    CalculationLink,
    Loc,
    CalculationArc,
    Unknown,
}

impl CalculationTag {
    fn from_name(name: &[u8]) -> Self {
        match split_qname(name).local_name {
            "calculationLink" => Self::CalculationLink,
            "loc" => Self::Loc,
            "calculationArc" => Self::CalculationArc,
            _ => Self::Unknown,
        }
    }
}

/// Parse a calculation linkbase XML file.
///
/// Returns a map from role URI (the `xlink:role` on `<calculationLink>`)
/// to a list of [`CalculationArc`]s.
pub fn parse_calculation_linkbase(
    reader: &mut Reader<impl io::BufRead>,
) -> Result<HashMap<String, Vec<CalculationArc>>> {
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
                let tag = CalculationTag::from_name(e.name().as_ref());

                if matches!(tag, CalculationTag::CalculationLink) {
                    current_role = extract_role(e.attributes());
                    locators.clear();
                    arcs.clear();
                }
            }
            Ok(Event::Empty(ref e)) => {
                let tag = CalculationTag::from_name(e.name().as_ref());

                match tag {
                    CalculationTag::Loc => {
                        parse_loc(e.attributes(), &mut locators);
                    }
                    CalculationTag::CalculationArc => {
                        if let Some(arc) = parse_arc(e.attributes()) {
                            arcs.push(arc);
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) => {
                let tag = CalculationTag::from_name(e.name().as_ref());

                if matches!(tag, CalculationTag::CalculationLink) {
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
                    linkbase_type: LinkbaseType::Calculation,
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

struct RawCalcArc {
    from: String,
    to: String,
    order: Option<Decimal>,
    weight: Decimal,
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
                    href = Some(percent_decode(fragment));
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

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let (Some(h1), Some(h2)) = (hex_value(bytes[i + 1]), hex_value(bytes[i + 2]))
        {
            out.push((h1 << 4) | h2);
            i += 3;
            continue;
        }

        out.push(bytes[i]);
        i += 1;
    }

    String::from_utf8_lossy(&out).into_owned()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn parse_arc(attrs: Attributes) -> Option<RawCalcArc> {
    let mut from = None;
    let mut to = None;
    let mut order = None;
    let mut weight = Decimal::ONE;

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
            "weight" => {
                if let Ok(val) = attr.unescape_value() {
                    weight = Decimal::from_str(&val)
                        .ok()
                        .or_else(|| Decimal::from_scientific(&val).ok())
                        .unwrap_or(Decimal::ONE);
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
