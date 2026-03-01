use crate::{
    ConceptId,
    error::{LinkbaseType, Result, XbrlError},
    taxonomy::split_qname,
};
use indexmap::IndexMap;
use quick_xml::{
    Reader,
    events::{Event, attributes::Attributes},
};
use rust_decimal::Decimal;
use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
    io,
    str::FromStr,
};

/// A presentation hierarchy defined by a presentation linkbase, used for
/// displaying facts in a human-friendly way.
#[derive(Debug, Clone, Default)]
pub struct PresentationNetwork {
    /// Root concept IDs: appear as `from` but never as `to`, in display order.
    pub(crate) roots: Vec<ConceptId>,
    /// Children of each concept, already sorted by presentation `order`.
    pub(crate) children: HashMap<ConceptId, Vec<ConceptId>>,
}

impl PresentationNetwork {
    /// Returns root concept IDs (concepts that are never a child of another concept).
    pub fn roots(&self) -> &[ConceptId] {
        &self.roots
    }

    /// Returns children of `concept_id`, already sorted by presentation order.
    pub fn children_of(&self, concept_id: &str) -> &[ConceptId] {
        self.children
            .get(concept_id)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Returns `true` if this network has no concepts.
    pub fn is_empty(&self) -> bool {
        self.roots.is_empty() && self.children.is_empty()
    }
}

/// Build a [`PresentationNetwork`] from a flat list of presentation arcs.
///
/// Computes roots and groups children by parent (sorted by `order`).
/// Called once per role after all linkbases are parsed.
pub(crate) fn build_network(arcs: Vec<PresentationArc>) -> Result<PresentationNetwork> {
    if arcs.is_empty() {
        return Ok(PresentationNetwork::default());
    }

    // Collect all distinct node IDs and group children by parent.
    let mut children_raw: HashMap<&str, Vec<(Option<Decimal>, &str)>> = HashMap::new();
    let mut all_nodes: HashSet<&str> = HashSet::new();
    for arc in &arcs {
        all_nodes.insert(arc.from.as_str());
        all_nodes.insert(arc.to.as_str());
        children_raw
            .entry(arc.from.as_str())
            .or_default()
            .push((arc.order, arc.to.as_str()));
    }

    // Sort children by order.
    for kids in children_raw.values_mut() {
        kids.sort_by(|(a, _), (b, _)| match (a, b) {
            (Some(x), Some(y)) => x.cmp(y),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        });
    }

    // Roots: `from` IDs that never appear as `to`, in first-occurrence order.
    let to_set: HashSet<&str> = arcs.iter().map(|a| a.to.as_str()).collect();
    let mut seen: HashSet<&str> = HashSet::new();
    let mut roots_raw: Vec<&str> = Vec::new();
    for arc in &arcs {
        let from = arc.from.as_str();
        if !to_set.contains(from) && seen.insert(from) {
            roots_raw.push(from);
        }
    }

    // Sort roots by their minimum outgoing arc order.
    roots_raw.sort_by(|a, b| {
        let min_order = |id: &&str| {
            children_raw
                .get(*id)
                .and_then(|kids| kids.iter().filter_map(|(ord, _)| *ord).min())
        };
        match (min_order(a), min_order(b)) {
            (Some(x), Some(y)) => x.cmp(&y),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        }
    });

    // Convert to owned ConceptId structures.
    let roots: Vec<ConceptId> = roots_raw.into_iter().map(ConceptId::from).collect();
    let children: HashMap<ConceptId, Vec<ConceptId>> = children_raw
        .into_iter()
        .map(|(parent, kids)| {
            (
                ConceptId::from(parent),
                kids.into_iter()
                    .map(|(_, child)| ConceptId::from(child))
                    .collect(),
            )
        })
        .collect();

    Ok(PresentationNetwork { roots, children })
}

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
    fn from_name(name: &[u8]) -> Result<Self> {
        Ok(match split_qname(name)?.local_name {
            "presentationLink" => Self::PresentationLink,
            "loc" => Self::Loc,
            "presentationArc" => Self::PresentationArc,
            _ => Self::Unknown,
        })
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
                let tag = PresentationTag::from_name(e.name().as_ref())?;

                if matches!(tag, PresentationTag::PresentationLink) {
                    // Start a new link group — reset per-link state
                    current_role = extract_role(e.attributes())?;
                    locators.clear();
                    arcs.clear();
                }
            }
            Ok(Event::Empty(ref e)) => {
                let tag = PresentationTag::from_name(e.name().as_ref())?;

                match tag {
                    PresentationTag::Loc => {
                        parse_loc(e.attributes(), &mut locators)?;
                    }
                    PresentationTag::PresentationArc => {
                        if let Some(arc) = parse_arc(e.attributes())? {
                            arcs.push(arc);
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) => {
                let tag = PresentationTag::from_name(e.name().as_ref())?;

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

fn extract_role(attrs: Attributes) -> Result<String> {
    for attr in attrs.flatten() {
        if split_qname(attr.key.as_ref())?.local_name == "role"
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
        let local = split_qname(attr.key.as_ref())?.local_name;
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

fn parse_arc(attrs: Attributes) -> Result<Option<RawArc>> {
    let mut from = None;
    let mut to = None;
    let mut order = None;

    for attr in attrs.flatten() {
        let local = split_qname(attr.key.as_ref())?.local_name;
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

    Ok(match (from, to) {
        (Some(from), Some(to)) => Some(RawArc { from, to, order }),
        _ => None,
    })
}
