use super::parser::{ComplexType, Element, RawSchema, SimpleType};
use crate::{Balance, ExpandedName, PeriodType, xml::QName};
use std::collections::{HashMap, HashSet};

/// Standard XBRL base types (from xbrli) and common custom types (e.g.,
/// shares, percent).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XbrlType {
    Boolean,
    String,
    Integer,
    Decimal,
    Monetary,
    Date,
    DateTime,
    QName,
    /// For xbrli:pureItemType
    Pure,
    Float,
    Double,
    Shares,
    Fraction,
    Percent,
    PerShare,
    /// A custom simple type defined in the taxonomy
    Simple(String),
    /// A custom complex type (usually tuples)
    Complex(String),
}

impl XbrlType {
    pub fn is_monetary(&self) -> bool {
        matches!(self, XbrlType::Monetary)
    }

    pub fn is_shares(&self) -> bool {
        matches!(self, XbrlType::Shares)
    }

    pub fn is_numeric(&self) -> bool {
        matches!(
            self,
            XbrlType::Monetary
                | XbrlType::Decimal
                | XbrlType::Integer
                | XbrlType::Float
                | XbrlType::Double
                | XbrlType::Shares
                | XbrlType::Fraction
                | XbrlType::Percent
                | XbrlType::PerShare
        )
    }
}

/// The base substitution group of a concept, resolved to either `Item` or
/// `Tuple`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BaseSubstitutionGroup {
    Item,
    Tuple,
}

/// The resolved substitution group of a concept, including both the resolved
/// base group and the original QName.
#[derive(Debug, PartialEq, Eq)]
pub struct SubstitutionGroup {
    /// The fully resolved base group (item, tuple, dimension, etc.)
    pub base: BaseSubstitutionGroup,
    /// The original QName of the substitution group
    pub original: QName,
}

impl SubstitutionGroup {
    pub fn is_item(&self) -> bool {
        self.base == BaseSubstitutionGroup::Item
    }

    pub fn is_tuple(&self) -> bool {
        self.base == BaseSubstitutionGroup::Tuple
    }
}

/// Maximum occurrences of a child element in a tuple's content model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MaxOccurs {
    /// A finite upper bound (e.g., `maxOccurs="1"`).
    Bounded(u32),
    /// No upper bound (`maxOccurs="unbounded"`).
    Unbounded,
}

/// A child element reference declared inside a tuple's `xs:complexType`.
#[derive(Debug, Clone, PartialEq)]
pub struct TupleChild {
    /// The qualified name of the referenced element (e.g., `"my:street"`).
    pub name: QName,
    /// Minimum occurrences from the `minOccurs` attribute; defaults to `1` per
    /// the XSD spec.
    pub min_occurs: u32,
    /// Maximum occurrences from the `maxOccurs` attribute; defaults to
    /// `MaxOccurs::Bounded(1)` per the XSD spec.
    pub max_occurs: MaxOccurs,
}

/// An XBRL concept defined in the taxonomy schema.
///
/// A `Concept` represents a reportable item or tuple as defined by the
/// taxonomy's XML Schema (XSD), after schema resolution has been performed.
///
/// This is a *semantic model* abstraction, not a direct representation of an
/// `xs:element`. All relevant XBRL-specific properties (such as substitution
/// group, period type, and balance) are resolved during taxonomy loading.
///
/// # What This Represents
///
/// - An element in the taxonomy belonging to either `xbrli:item` or
///   `xbrli:tuple` substitution groups.
/// - A reportable concept that may appear in instance documents.
/// - The structural and semantic metadata required for validation, instance
///   building, and linkbase processing.
#[derive(Debug)]
pub struct Concept {
    /// The element's id attribute (e.g., "de-gaap-ci_bs.ass.fixAss").
    pub id: Option<String>,
    /// The element's qualified name.
    pub name: ExpandedName,
    /// The resolved XSD type.
    pub data_type: XbrlType,
    /// Substitution group (e.g., "xbrli:item", "xbrli:tuple").
    pub substitution_group: SubstitutionGroup,
    /// The XBRL period type ("instant" or "duration").
    pub period_type: Option<PeriodType>,
    /// The XBRL balance type ("debit" or "credit").
    pub balance: Option<Balance>,
    /// Whether this element is nillable.
    pub nillable: bool,
    /// Whether this element is abstract.
    pub is_abstract: bool,
    /// For tuple elements: the child elements declared via `xs:element[@ref]`
    /// inside the tuple's inline `xs:complexType`. Empty for non-tuple
    /// elements.
    pub tuple_children: Vec<TupleChild>,
}

impl Concept {
    /// Returns `true` if this element is an XBRL tuple (`substitutionGroup="xbrli:tuple"`).
    pub fn is_tuple(&self) -> bool {
        self.substitution_group.is_tuple()
    }

    /// Returns `true` if this element is a concrete (non-abstract) item fact.
    /// Such elements are the only ones that should appear as facts in an instance document.
    pub fn is_concrete_item(&self) -> bool {
        !self.is_abstract && self.period_type.is_some()
    }

    pub fn is_abstract(&self) -> bool {
        self.is_abstract
    }
}

/// Resolve all elements in a `RawSchema` into fully resolved `Concept`s.
///
/// Elements without a `substitutionGroup` attribute are skipped — they are
/// plain XSD elements, not XBRL concepts.
pub fn resolve_concepts(raw: &RawSchema) -> Vec<Concept> {
    let elements_by_name: HashMap<&str, &Element> =
        raw.elements.iter().map(|e| (e.name.as_str(), e)).collect();

    let simple_types_by_name: HashMap<&str, &SimpleType> = raw
        .simple_types
        .iter()
        .filter_map(|simple_type| simple_type.name.as_deref().map(|name| (name, simple_type)))
        .collect();

    let complex_types_by_name: HashMap<&str, &ComplexType> = raw
        .complex_types
        .iter()
        .filter_map(|complex_type| {
            complex_type
                .name
                .as_deref()
                .map(|name| (name, complex_type))
        })
        .collect();

    let target_namespace = raw.target_namespace.as_deref().unwrap_or("");

    raw.elements
        .iter()
        .filter(|element| element.substitution_group.is_some())
        .map(|element| {
            let sub_group = resolve_substitution_group(element, &elements_by_name);

            let data_type = match &element.type_name {
                Some(type_qname) => {
                    resolve_type(type_qname, &simple_types_by_name, &complex_types_by_name)
                }
                None => XbrlType::Complex(element.name.clone()),
            };

            Concept {
                id: element.id.clone(),
                name: ExpandedName::new(target_namespace.to_owned(), element.name.clone()),
                data_type,
                substitution_group: sub_group,
                period_type: element.period_type.clone(),
                balance: element.balance.clone(),
                nillable: element.is_nillable,
                is_abstract: element.is_abstract,
                tuple_children: element
                    .complex_type
                    .as_ref()
                    .map(|complex_type| complex_type.children.clone())
                    .unwrap_or_default()
                    .into_iter()
                    .map(|child| TupleChild {
                        name: child.name,
                        min_occurs: child.min_occurs,
                        max_occurs: match child.max_occurs {
                            Some(n) => MaxOccurs::Bounded(n),
                            None => MaxOccurs::Unbounded,
                        },
                    })
                    .collect(),
            }
        })
        .collect()
}

/// Resolve the substitution group chain for an element to determine the base
/// group (`Item` or `Tuple`).
fn resolve_substitution_group(
    element: &Element,
    elements_by_name: &HashMap<&str, &Element>,
) -> SubstitutionGroup {
    // The element is guaranteed to have a substitution group at this point.
    let substitution_group = element.substitution_group.as_ref().unwrap();
    let original = convert_qname(substitution_group);

    if let Some(base) = match_head_group(&substitution_group.local_name) {
        return SubstitutionGroup { base, original };
    }

    // Walk the chain following substitution group references.
    let mut current_name = substitution_group.local_name.as_str();
    let mut seen = HashSet::new();

    while seen.insert(current_name) {
        let Some(parent) = elements_by_name.get(current_name) else {
            break;
        };

        let Some(parent_substitution_group) = &parent.substitution_group else {
            break;
        };

        if let Some(base) = match_head_group(&parent_substitution_group.local_name) {
            return SubstitutionGroup { base, original };
        }

        current_name = parent_substitution_group.local_name.as_str();
    }

    // Could not resolve — default to Item.
    SubstitutionGroup {
        base: BaseSubstitutionGroup::Item,
        original,
    }
}

/// Check if a local name matches one of the XBRL head substitution groups.
fn match_head_group(local_name: &str) -> Option<BaseSubstitutionGroup> {
    match local_name {
        "item" => Some(BaseSubstitutionGroup::Item),
        "tuple" => Some(BaseSubstitutionGroup::Tuple),
        _ => None,
    }
}

/// Resolve a type QName to an `XbrlType` by checking well-known types first,
/// then walking the type inheritance chain through simple/complex type
/// definitions.
fn resolve_type(
    type_qname: &QName,
    simple_types: &HashMap<&str, &SimpleType>,
    complex_types: &HashMap<&str, &ComplexType>,
) -> XbrlType {
    if let Some(known) = match_known_type(&type_qname.local_name) {
        return known;
    }

    // Walk simple type chain.
    if let Some(resolved) = walk_simple_type_chain(&type_qname.local_name, simple_types) {
        return resolved;
    }

    // Walk complex type chain.
    if let Some(resolved) = walk_complex_type_chain(&type_qname.local_name, complex_types) {
        return resolved;
    }

    // Heuristic fallback: match on substrings of the local name.
    heuristic_type(&type_qname.local_name)
}

/// Match a local name against well-known XBRL and XSD types.
fn match_known_type(local_name: &str) -> Option<XbrlType> {
    match local_name {
        // XBRL item types
        "monetaryItemType" => Some(XbrlType::Monetary),
        "stringItemType"
        | "normalizedStringItemType"
        | "tokenItemType"
        | "languageItemType"
        | "NCNameItemType"
        | "anyURIItemType"
        | "textBlockItemType"
        | "escapedItemType" => Some(XbrlType::String),
        "decimalItemType" => Some(XbrlType::Decimal),
        "integerItemType"
        | "nonNegativeIntegerItemType"
        | "positiveIntegerItemType"
        | "nonPositiveIntegerItemType"
        | "negativeIntegerItemType" => Some(XbrlType::Integer),
        "booleanItemType" => Some(XbrlType::Boolean),
        "dateItemType" => Some(XbrlType::Date),
        "dateTimeItemType" | "dateUnionItemType" => Some(XbrlType::DateTime),
        "pureItemType" => Some(XbrlType::Pure),
        "QNameItemType" => Some(XbrlType::QName),
        "floatItemType" => Some(XbrlType::Float),
        "doubleItemType" => Some(XbrlType::Double),
        "sharesItemType" => Some(XbrlType::Shares),
        "fractionItemType" => Some(XbrlType::Fraction),

        // XSD built-in types
        "string" | "normalizedString" | "token" | "language" | "Name" | "NCName" | "anyURI"
        | "NMTOKEN" => Some(XbrlType::String),
        "decimal" => Some(XbrlType::Decimal),
        "integer" | "nonNegativeInteger" | "positiveInteger" | "nonPositiveInteger"
        | "negativeInteger" | "int" | "long" | "short" | "byte" | "unsignedInt"
        | "unsignedLong" | "unsignedShort" | "unsignedByte" => Some(XbrlType::Integer),
        "boolean" => Some(XbrlType::Boolean),
        "date" | "gYear" | "gYearMonth" | "gMonth" | "gMonthDay" | "gDay" => Some(XbrlType::Date),
        "dateTime" | "time" | "duration" => Some(XbrlType::DateTime),
        "float" => Some(XbrlType::Float),
        "double" => Some(XbrlType::Double),
        "QName" => Some(XbrlType::QName),
        "anyType" | "anySimpleType" => Some(XbrlType::String),

        _ => None,
    }
}

/// Walk the simple type inheritance chain to find a known base type.
fn walk_simple_type_chain(
    type_name: &str,
    simple_types: &HashMap<&str, &SimpleType>,
) -> Option<XbrlType> {
    let mut current = type_name;
    let mut seen = HashSet::new();

    while seen.insert(current) {
        let st = simple_types.get(current)?;
        let base_qname = st.base.as_ref()?;

        if let Some(known) = match_known_type(&base_qname.local_name) {
            return Some(known);
        }

        current = base_qname.local_name.as_str();
    }

    None
}

/// Walk the complex type inheritance chain to find a known base type.
fn walk_complex_type_chain(
    type_name: &str,
    complex_types: &HashMap<&str, &ComplexType>,
) -> Option<XbrlType> {
    let mut current = type_name;
    let mut seen = HashSet::new();

    while seen.insert(current) {
        let ct = complex_types.get(current)?;
        let base_qname = ct.base.as_ref()?;

        if let Some(known) = match_known_type(&base_qname.local_name) {
            return Some(known);
        }

        current = base_qname.local_name.as_str();
    }

    None
}

/// Heuristic fallback: match on substrings of the type's local name.
fn heuristic_type(local_name: &str) -> XbrlType {
    let lower = local_name.to_ascii_lowercase();

    if lower.contains("monetary") {
        XbrlType::Monetary
    } else if lower.contains("pershare") {
        XbrlType::PerShare
    } else if lower.contains("percent") {
        XbrlType::Percent
    } else if lower.contains("shares") {
        XbrlType::Shares
    } else if lower.contains("fraction") {
        XbrlType::Fraction
    } else if lower.contains("pure") {
        XbrlType::Pure
    } else if lower.contains("decimal") || lower.contains("float") || lower.contains("double") {
        XbrlType::Decimal
    } else if lower.contains("integer") {
        XbrlType::Integer
    } else if lower.contains("boolean") {
        XbrlType::Boolean
    } else if lower.contains("datetime") {
        XbrlType::DateTime
    } else if lower.contains("date") {
        XbrlType::Date
    } else if lower.contains("qname") {
        XbrlType::QName
    } else {
        XbrlType::Simple(local_name.to_owned())
    }
}

fn convert_qname(raw: &QName) -> QName {
    QName {
        prefix: raw.prefix.clone(),
        local_name: raw.local_name.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instance::NamespaceUri;
    use std::{collections::HashMap, path::PathBuf};

    fn empty_schema() -> RawSchema {
        RawSchema {
            file_path: PathBuf::from("test.xsd"),
            target_namespace: Some("http://example.com/taxonomy".to_owned()),
            namespaces: HashMap::new(),
            imports: vec![],
            includes: vec![],
            linkbase_refs: vec![],
            role_types: vec![],
            arcrole_types: vec![],
            elements: vec![],
            simple_types: vec![],
            complex_types: vec![],
        }
    }

    fn item_qname() -> QName {
        QName {
            prefix: Some("xbrli".to_owned()),
            local_name: "item".to_owned(),
        }
    }

    fn tuple_qname() -> QName {
        QName {
            prefix: Some("xbrli".to_owned()),
            local_name: "tuple".to_owned(),
        }
    }

    fn monetary_type() -> QName {
        QName {
            prefix: Some("xbrli".to_owned()),
            local_name: "monetaryItemType".to_owned(),
        }
    }

    fn string_type() -> QName {
        QName {
            prefix: Some("xbrli".to_owned()),
            local_name: "stringItemType".to_owned(),
        }
    }

    #[test]
    fn resolve_direct_item() {
        let mut schema = empty_schema();
        schema.elements.push(Element {
            name: "Revenue".to_owned(),
            id: Some("Revenue".to_owned()),
            type_name: Some(monetary_type()),
            substitution_group: Some(item_qname()),
            is_nillable: true,
            is_abstract: false,
            period_type: Some(PeriodType::Duration),
            balance: Some(Balance::Credit),
            complex_type: None,
        });

        let concepts = resolve_concepts(&schema);

        assert_eq!(concepts.len(), 1);
        let concept = &concepts[0];
        assert_eq!(concept.id, Some("Revenue".to_owned()));
        assert_eq!(
            concept.name,
            ExpandedName::new(
                "http://example.com/taxonomy".to_owned(),
                "Revenue".to_owned()
            )
        );
        assert_eq!(concept.data_type, XbrlType::Monetary);
        assert_eq!(concept.substitution_group.base, BaseSubstitutionGroup::Item);
        assert_eq!(concept.period_type, Some(PeriodType::Duration));
        assert_eq!(concept.balance, Some(Balance::Credit));
        assert!(concept.nillable);
        assert!(!concept.is_abstract);
    }

    #[test]
    fn resolve_direct_tuple() {
        let mut schema = empty_schema();
        schema.elements.push(Element {
            name: "Address".to_owned(),
            id: None,
            type_name: None,
            substitution_group: Some(tuple_qname()),
            is_nillable: false,
            is_abstract: false,
            period_type: None,
            balance: None,
            complex_type: None,
        });

        let concepts = resolve_concepts(&schema);

        assert_eq!(concepts.len(), 1);
        let c = &concepts[0];
        assert_eq!(c.substitution_group.base, BaseSubstitutionGroup::Tuple);
        assert_eq!(c.data_type, XbrlType::Complex("Address".to_owned()));
    }

    #[test]
    fn resolve_substitution_group_chain() {
        let mut schema = empty_schema();

        // Abstract head element in xbrli:item chain
        schema.elements.push(Element {
            name: "abstractItem".to_owned(),
            id: Some("abstractItem".to_owned()),
            type_name: Some(string_type()),
            substitution_group: Some(item_qname()),
            is_nillable: false,
            is_abstract: true,
            period_type: Some(PeriodType::Instant),
            balance: None,
            complex_type: None,
        });

        // Concrete element pointing to abstract head
        schema.elements.push(Element {
            name: "ConcreteItem".to_owned(),
            id: Some("ConcreteItem".to_owned()),
            type_name: Some(string_type()),
            substitution_group: Some(QName {
                prefix: None,
                local_name: "abstractItem".to_owned(),
            }),
            is_nillable: true,
            is_abstract: false,
            period_type: Some(PeriodType::Instant),
            balance: None,
            complex_type: None,
        });

        let concepts = resolve_concepts(&schema);

        assert_eq!(concepts.len(), 2);
        let concrete = &concepts[1];
        assert_eq!(concrete.name.local_name, "ConcreteItem");
        assert_eq!(
            concrete.substitution_group.base,
            BaseSubstitutionGroup::Item
        );
        assert_eq!(
            concrete.substitution_group.original.local_name,
            "abstractItem"
        );
    }

    #[test]
    fn resolve_type_inheritance_chain() {
        let mut schema = empty_schema();

        // Custom type deriving from another custom type
        schema.simple_types.push(SimpleType {
            name: Some("myBaseType".to_owned()),
            base: Some(string_type()),
            enumerations: vec![],
        });

        schema.simple_types.push(SimpleType {
            name: Some("myDerivedType".to_owned()),
            base: Some(QName {
                prefix: None,
                local_name: "myBaseType".to_owned(),
            }),
            enumerations: vec![],
        });

        schema.elements.push(Element {
            name: "CustomElement".to_owned(),
            id: Some("CustomElement".to_owned()),
            type_name: Some(QName {
                prefix: None,
                local_name: "myDerivedType".to_owned(),
            }),
            substitution_group: Some(item_qname()),
            is_nillable: true,
            is_abstract: false,
            period_type: Some(PeriodType::Duration),
            balance: None,
            complex_type: None,
        });

        let concepts = resolve_concepts(&schema);

        assert_eq!(concepts.len(), 1);
        assert_eq!(concepts[0].data_type, XbrlType::String);
    }

    #[test]
    fn skip_elements_without_substitution_group() {
        let mut schema = empty_schema();

        schema.elements.push(Element {
            name: "plainElement".to_owned(),
            id: None,
            type_name: Some(string_type()),
            substitution_group: None,
            is_nillable: false,
            is_abstract: false,
            period_type: None,
            balance: None,
            complex_type: None,
        });

        let concepts = resolve_concepts(&schema);
        assert!(concepts.is_empty());
    }

    #[test]
    fn substitution_group_cycle_defaults_to_item() {
        let mut schema = empty_schema();

        schema.elements.push(Element {
            name: "a".to_owned(),
            id: None,
            type_name: Some(string_type()),
            substitution_group: Some(QName {
                prefix: None,
                local_name: "b".to_owned(),
            }),
            is_nillable: false,
            is_abstract: false,
            period_type: None,
            balance: None,
            complex_type: None,
        });

        schema.elements.push(Element {
            name: "b".to_owned(),
            id: None,
            type_name: Some(string_type()),
            substitution_group: Some(QName {
                prefix: None,
                local_name: "a".to_owned(),
            }),
            is_nillable: false,
            is_abstract: false,
            period_type: None,
            balance: None,
            complex_type: None,
        });

        let concepts = resolve_concepts(&schema);

        assert_eq!(concepts.len(), 2);
        // Both should default to Item when cycle is detected.
        assert_eq!(
            concepts[0].substitution_group.base,
            BaseSubstitutionGroup::Item
        );
        assert_eq!(
            concepts[1].substitution_group.base,
            BaseSubstitutionGroup::Item
        );
    }

    #[test]
    fn unknown_type_uses_heuristic() {
        let mut schema = empty_schema();

        schema.elements.push(Element {
            name: "SharesOutstanding".to_owned(),
            id: None,
            type_name: Some(QName {
                prefix: Some("custom".to_owned()),
                local_name: "sharesType".to_owned(),
            }),
            substitution_group: Some(item_qname()),
            is_nillable: true,
            is_abstract: false,
            period_type: Some(PeriodType::Instant),
            balance: None,
            complex_type: None,
        });

        let concepts = resolve_concepts(&schema);

        assert_eq!(concepts.len(), 1);
        assert_eq!(concepts[0].data_type, XbrlType::Shares);
    }

    #[test]
    fn complex_type_inheritance_chain() {
        let mut schema = empty_schema();

        schema.complex_types.push(ComplexType {
            name: Some("myComplexBase".to_owned()),
            base: Some(monetary_type()),
            derivation: None,
            compositor: None,
            attributes: vec![],
            children: vec![],
        });

        schema.complex_types.push(ComplexType {
            name: Some("myComplexDerived".to_owned()),
            base: Some(QName {
                prefix: None,
                local_name: "myComplexBase".to_owned(),
            }),
            derivation: None,
            compositor: None,
            attributes: vec![],
            children: vec![],
        });

        schema.elements.push(Element {
            name: "MoneyElement".to_owned(),
            id: None,
            type_name: Some(QName {
                prefix: None,
                local_name: "myComplexDerived".to_owned(),
            }),
            substitution_group: Some(item_qname()),
            is_nillable: true,
            is_abstract: false,
            period_type: Some(PeriodType::Instant),
            balance: Some(Balance::Debit),
            complex_type: None,
        });

        let concepts = resolve_concepts(&schema);

        assert_eq!(concepts.len(), 1);
        assert_eq!(concepts[0].data_type, XbrlType::Monetary);
    }

    #[test]
    fn no_target_namespace_uses_empty_string() {
        let mut schema = empty_schema();
        schema.target_namespace = None;

        schema.elements.push(Element {
            name: "Elem".to_owned(),
            id: None,
            type_name: Some(string_type()),
            substitution_group: Some(item_qname()),
            is_nillable: false,
            is_abstract: false,
            period_type: Some(PeriodType::Instant),
            balance: None,
            complex_type: None,
        });

        let concepts = resolve_concepts(&schema);

        assert_eq!(concepts.len(), 1);
        assert_eq!(concepts[0].name.namespace_uri, NamespaceUri::from(""));
    }
}
