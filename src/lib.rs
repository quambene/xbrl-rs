//! Parser for XBRL documents with support for:
//! - XBRL instance documents
//! - XBRL taxonomy schemas
//! - XBRL taxonomy linkbases (presentation, calculation, definition, label,
//!   reference)

mod error;
mod instance;
mod taxonomy;
pub(crate) mod validation;
mod xml;

pub use error::{LinkbaseType, Result, ValueError, XbrlError};
pub use instance::{
    Context, ContextId, Decimals, DocumentView, EntityIdentifier, Fact, FactAttribute,
    FactAttributeName, FactValue, FootnoteArc, FootnoteLink, FootnoteLocator, FootnoteResource,
    InstanceDocument, InstanceParser, InstanceWriter, ItemFact, Period, SectionView, TreeNode,
    TupleFact, TypedFact, TypedInstanceDocument, TypedItemFact, TypedTupleFact, Unit, UnitId,
    resolve_instance,
};
pub use quick_xml::{Reader as XmlReader, Writer as XmlWriter};
pub use taxonomy::{
    ArcroleType, Balance, BaseSubstitutionGroup, CalculationArc, Concept, ConceptView,
    DeclaredAccuracy, DefinitionArc, ElementDecl, ElementParticle, GroupDef, GroupParticle, Label,
    LinkbaseLocator, LinkbaseParser, Occurrence, Particle, PeriodType, PresentationArc,
    PresentationRelationView, Reference, ReferencePart, RoleType, SchemaParser, SubstitutionGroup,
    TaxonomySchema, TaxonomySectionView, TaxonomySet, TaxonomyTreeNode, TaxonomyView,
    TupleElementView, TupleParticleView, XbrlType,
};
#[cfg(feature = "download")]
pub use taxonomy::{DownloadResult, TaxonomyLoader};
pub use validation::{Severity, ValidationMessage, ValidationResult};
pub use xml::{
    ArcroleUri, ConceptId, ExpandedName, NamespacePrefix, NamespaceUri, QName, RoleUri,
    SchemaRefUrl,
};

pub const ROLE_TERSE: &str = "http://www.xbrl.org/2003/role/terseLabel";
pub const ROLE_LABEL: &str = "http://www.xbrl.org/2003/role/label";
