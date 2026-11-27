//! Inspect a document and its elements.
//!
//! # Entry points
//!
//! | Type | What it is |
//! |---|---|
//! | [`Document`] | One interchange unit: its root packages and every element under them. |
//! | [`SacmElement`] | A view of one element, which borrows the document. |
//! | [`ElementId`] | An element's identity: its `xmi:id`. |
//! | [`ElementKind`] | An element's concrete SACM class. |
//! | [`AbstractClass`], [`MetamodelPackage`] | The SACM 2.3 metamodel: which abstract classes a kind specializes, and which package defines it. |
//! | [`LangString`], [`MultiLangString`], [`AssertionDeclaration`] | Values, which you construct directly. |
//! | [`Flag`], [`ReferenceEnd`] | The name of a flag or a reference end, for the edits that set them. |
//! | [`Revision`] | A document's current state within the process, from [`Document::revision`]. |
//!
//! # Rules
//!
//! - **Only a `Document` holds elements.** Read one with [`Document::get`]; change it
//!   with [`edit::apply`](crate::edit::apply).
//! - **A reference is the [`ElementId`] it names,** resolved with [`Document::get`].
//! - **Values are not elements.** They have no id and no kind of their own. The `xmi:id`
//!   of a `name`, `content`, `key` or `location` is kept with its owner
//!   ([`SacmElement::name_id`], [`SacmElement::content_id`]).
//! - **A name may be missing.** In SACM every
//!   [`ModelElement`](AbstractClass::ModelElement) has a `name`; [`SacmElement::name`]
//!   returns `None` when the document holds the element without one.
//! - **A `Revision` is for previews.** [`edit::apply_preview`](crate::edit::apply_preview)
//!   compares it.
//!
//! # What an element holds
//!
//! | Which elements | What | Read with |
//! |---|---|---|
//! | every element (§8.2) | `gid` | [`SacmElement::gid`] |
//! | every element (§8.2) | `isCitation`, `isAbstract` | [`SacmElement::is_citation`], [`SacmElement::is_abstract`] |
//! | every element (§8.2) | `citedElement`, `abstractForm` | [`SacmElement::cited_element`], [`SacmElement::abstract_form`] |
//! | every `ModelElement` | descriptions, implementation constraints, notes and tagged values: the `UtilityElement`s | [`SacmElement::descriptions`] and the like |
//! | a `UtilityElement` | its text | [`content`](SacmElement::content) |
//! | every Assertion: a Claim or an asserted relationship | `assertionDeclaration`, meta-claims | [`SacmElement::assertion_declaration`], [`SacmElement::meta_claims`] |
//! | an asserted relationship, such as an `AssertedInference` | its sources and target, an optional `ArgumentReasoning`, and whether it counters its declared purpose | [`sources`](SacmElement::sources), [`target`](SacmElement::target), [`reasoning`](SacmElement::reasoning), [`is_counter`](SacmElement::is_counter) |
//! | an `ArgumentReasoning` | the package that details it | [`structure`](SacmElement::structure) |
//!
//! [`SacmElement`] lists the rest.
//!
//! # Examples
//!
//! ```
//! use sacm::model::{Document, ElementId, ElementKind, LangString};
//!
//! // Built with the unstable test-support feature (not shown on docs.rs; see the crate's Features): edit::apply does not create a missing name.
//! let mut doc = Document::new();
//! let arguments = ElementId::from("arguments");
//! let name = |content: &str| Some(LangString::new(content).with_lang("en"));
//! doc.insert(None, ElementKind::ArgumentPackage, arguments.clone(), name("Arguments")).unwrap();
//! doc.insert(Some(&arguments), ElementKind::Claim, ElementId::from("claim_top"), name("Top claim")).unwrap();
//! // A claim whose name is missing.
//! doc.insert(Some(&arguments), ElementKind::Claim, ElementId::from("claim_sub"), None).unwrap();
//!
//! let package = doc.get("arguments").expect("the document has it");
//! println!("{} is an {}", package.id(), package.kind());
//! for child in package.children() {
//!     match child.name() {
//!         Some(name) => println!("  {} {} {:?}", child.kind(), child.id(), name.content()),
//!         None => println!("  {} {} (name missing)", child.kind(), child.id()),
//!     }
//! }
//!
//! let claim = doc.get("claim_sub").expect("the document has it");
//! assert_eq!(claim.name(), None);
//! let parent = claim.parent().expect("a claim here has a parent");
//! assert_eq!(parent.id().as_str(), "arguments");
//! ```
//!
//! The value types:
//!
//! ```
//! use sacm::model::{AssertionDeclaration, LangString, MultiLangString};
//!
//! let mut name = LangString::new("C1").with_lang("en");
//! assert_eq!(name.lang(), Some("en"));
//!
//! // Absent and empty are different values.
//! assert_ne!(LangString::new(""), LangString::default());
//! name.set_lang(None);
//! assert_eq!(name.lang(), None);
//!
//! // A MultiLangString keeps its values in order, duplicates included.
//! let text: MultiLangString = [name.clone(), name].into_iter().collect();
//! assert_eq!(text.len(), 2);
//!
//! let declaration: AssertionDeclaration = "needsSupport".parse()?;
//! assert_eq!(declaration, AssertionDeclaration::NeedsSupport);
//! assert_eq!(AssertionDeclaration::default(), AssertionDeclaration::Asserted);
//!
//! // Parsing is exact and case-sensitive.
//! let error = "Asserted".parse::<AssertionDeclaration>().unwrap_err();
//! assert_eq!(error.input(), "Asserted");
//! # Ok::<(), sacm::model::ParseAssertionDeclarationError>(())
//! ```

mod argumentation;
mod containment;
mod document;
mod href;
mod id;
mod kind;
mod lang_string;
mod preserved;
mod reference;
mod revision;
mod subclass;

pub use argumentation::{AssertionDeclaration, ParseAssertionDeclarationError};
pub(crate) use containment::{DeclaredType, Role, admits, role_named};
pub(crate) use document::{
    CapturedElement, Compared, IdGenerator, Snapshot, StringAttribute, Text, TextRole,
};
pub use document::{Document, Referrer, SacmElement};
#[cfg(feature = "test-support")]
pub use document::{InsertError, SetAssertionDeclarationError};
pub(crate) use href::is_xml_char;
pub use href::{Href, HrefError, Proxy};
pub use id::ElementId;
pub(crate) use id::is_ncname;
pub use kind::{AbstractClass, ElementKind, MetamodelPackage};
pub use lang_string::{LangString, MultiLangString};
pub(crate) use preserved::{
    Attr, EMF_XMI_NS, Event, Fragment, Item, Placed, Preserved, QName, XMI_NS, XSI_NS,
};
pub use reference::{Flag, ReferenceEnd};
pub(crate) use reference::{OWNERS, flag_named, reference_end_named};
pub use revision::Revision;
pub use subclass::Subclass;
