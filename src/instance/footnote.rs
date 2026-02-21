/// A single `link:footnoteLink` extended link in an XBRL instance.
#[derive(Debug, Clone, Default)]
pub struct FootnoteLink {
    /// Optional `xlink:role` on the footnote link.
    pub role: Option<String>,
    /// Optional `xml:lang` inherited by contained footnote resources.
    pub xml_lang: Option<String>,
    /// Locator resources (`link:loc` or custom locator-like elements).
    pub locators: Vec<FootnoteLocator>,
    /// Footnote resources (`link:footnote`).
    pub footnotes: Vec<FootnoteResource>,
    /// Arcs connecting locators and footnote resources.
    pub arcs: Vec<FootnoteArc>,
}

/// A locator within a footnote link, usually a `link:loc` element.
#[derive(Debug, Clone)]
pub struct FootnoteLocator {
    /// Local name of the locator element (e.g. `loc` or a custom element).
    pub element_local_name: String,
    /// Optional `xlink:label` used for arc endpoints.
    pub label: Option<String>,
    /// Optional `xlink:href` target, typically a same-document fragment.
    pub href: Option<String>,
}

/// A footnote resource within a footnote link (`link:footnote`).
#[derive(Debug, Clone)]
pub struct FootnoteResource {
    /// Optional `xlink:label` used for arc endpoints.
    pub label: Option<String>,
    /// Optional XML `id` of the footnote resource.
    pub id: Option<String>,
    /// Optional `xlink:role` of the resource.
    pub role: Option<String>,
    /// Optional `xml:lang` for the footnote text content.
    pub xml_lang: Option<String>,
}

/// An arc in a footnote link (for example `link:footnoteArc`).
#[derive(Debug, Clone)]
pub struct FootnoteArc {
    /// Optional `xlink:from` label.
    pub from: Option<String>,
    /// Optional `xlink:to` label.
    pub to: Option<String>,
    /// Optional `xlink:arcrole` of the relationship.
    pub arcrole: Option<String>,
}
