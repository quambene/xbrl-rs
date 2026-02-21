use crate::error::{Result, XbrlError};
use quick_xml::{Reader, events::Event};
use std::io;

/// Local names of the five standard XBRL extended-link elements.
const STANDARD_LINK_NAMES: &[&[u8]] = &[
    b"labelLink",
    b"referenceLink",
    b"presentationLink",
    b"calculationLink",
    b"definitionLink",
];

/// A locator extracted from a linkbase `<loc>` element.
#[derive(Debug, Clone)]
pub struct LinkbaseLocator {
    /// The raw `xlink:href` attribute value.
    pub href: String,
    /// Whether this locator lives inside a standard XBRL extended-link element
    /// (`labelLink`, `referenceLink`, `presentationLink`, `calculationLink`,
    /// `definitionLink`).  Standard links require locators to point to concept
    /// elements; custom link types do not impose this constraint.
    pub in_standard_link: bool,
}

impl LinkbaseLocator {
    /// Parse every `<loc>` element from a linkbase document.
    ///
    /// Tracks whether each locator lives inside a standard XBRL extended-link
    /// element so that [`LinkbaseLocator::validate`] can apply the appropriate
    /// rules.
    pub fn parse<R: io::BufRead>(reader: &mut Reader<R>) -> Result<Vec<Self>> {
        reader.config_mut().trim_text(true);

        let mut locators = Vec::new();
        let mut in_standard_link = false;
        let mut buf = Vec::new();

        loop {
            buf.clear();
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) | Ok(Event::Empty(ref e)) => {
                    let local = e.name().local_name();
                    if STANDARD_LINK_NAMES.iter().any(|s| local.as_ref() == *s) {
                        in_standard_link = true;
                    }
                    if local.as_ref() == b"loc" {
                        let mut href = String::new();
                        for attr in e.attributes().flatten() {
                            if attr.key.local_name().as_ref() == b"href" {
                                href = String::from_utf8_lossy(&attr.value).to_string();
                            }
                        }
                        locators.push(LinkbaseLocator {
                            href,
                            in_standard_link,
                        });
                    }
                }
                Ok(Event::End(ref e)) => {
                    let local = e.name().local_name();
                    if STANDARD_LINK_NAMES.iter().any(|s| local.as_ref() == *s) {
                        in_standard_link = false;
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => return Err(XbrlError::Xml(e)),
                _ => {}
            }
        }

        Ok(locators)
    }

    /// Validate this locator's href for illegal XBRL 2.1 pointer syntax (§3.5.4).
    ///
    /// Returns `Err(InvalidHref)` if any of the following apply:
    ///
    /// - The href is empty and this locator is inside a standard XBRL link
    ///   (locators in standard links must point to an `xs:element` concept
    ///   declaration).
    /// - The fragment identifier uses the `xpointer()` scheme, which is illegal
    ///   in XBRL 2.1.
    /// - The fragment contains `xmlns()`, which is illegal per §3.5.4 even when
    ///   combined with a legal `element()` scheme.
    pub fn validate(&self) -> Result<()> {
        if self.href.is_empty() && self.in_standard_link {
            return Err(XbrlError::InvalidHref {
                href: self.href.clone(),
                reason: "empty href in standard XBRL link locator \
                         (must point to a concept element)"
                    .to_string(),
            });
        }

        let fragment = self.href.split_once('#').map(|(_, f)| f).unwrap_or("");

        if fragment.starts_with("xpointer(") {
            return Err(XbrlError::InvalidHref {
                href: self.href.clone(),
                reason: "xpointer() scheme is illegal in XBRL 2.1 (§3.5.4)".to_string(),
            });
        }

        if fragment.contains("xmlns(") {
            return Err(XbrlError::InvalidHref {
                href: self.href.clone(),
                reason: "xmlns() scheme is illegal in XBRL 2.1 XPointer expressions (§3.5.4)"
                    .to_string(),
            });
        }

        Ok(())
    }
}
