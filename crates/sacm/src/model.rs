//! The SACM document model: [`Document`], element identity, element kinds,
//! element names, assertion declarations and the first value types.
//!
//! A [`Document`] owns one interchange unit: its root packages and every
//! element under them. Each element has an [`ElementId`] (its `xmi:id`) and an
//! [`ElementKind`] (its concrete SACM class), and is read through a
//! [`SacmElement`] view that borrows the document. In SACM every
//! [`ModelElement`](AbstractClass::ModelElement) has a `name`;
//! [`SacmElement::name`] returns it, or `None` when the document holds the
//! element without one. Every Assertion (a Claim or an asserted relationship)
//! has an `assertionDeclaration`, which [`SacmElement::assertion_declaration`]
//! returns.
//!
//! [`ElementKind`], [`AbstractClass`] and [`MetamodelPackage`] describe the
//! SACM 2.3 metamodel itself: which class a kind is, which package defines it,
//! and which abstract classes it specializes.
//!
//! Every element may have a `gid` ([`SacmElement::gid`]), and every
//! `ModelElement` may hold descriptions, implementation constraints, notes
//! and tagged values ([`SacmElement::descriptions`] and the like), the
//! `UtilityElement`s whose text is their [`content`](SacmElement::content).
//! Every element also has the §8.2 flags [`SacmElement::is_citation`] and
//! [`SacmElement::is_abstract`], and may reference another element through
//! [`SacmElement::cited_element`] and [`SacmElement::abstract_form`]. A
//! reference is the [`ElementId`] it names, resolved with [`Document::get`].
//!
//! [`LangString`], [`MultiLangString`] and [`AssertionDeclaration`] are
//! values, not elements: they have no id and no kind of their own (the
//! `xmi:id` of a `name`, `content` or `key` is kept with its owner:
//! [`SacmElement::name_id`], [`SacmElement::content_id`]), and you construct
//! them directly, unlike elements, which only a [`Document`] holds.
//!
//! # Examples
//!
//! ```
//! use sacm::model::{Document, ElementId, ElementKind, LangString};
//!
//! // Built with the unstable test-support feature: edit::apply does not create a missing name.
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
mod id;
mod kind;
mod lang_string;
mod reference;

pub use argumentation::{AssertionDeclaration, ParseAssertionDeclarationError};
pub(crate) use containment::{DeclaredType, admits, role_named};
#[cfg(not(feature = "test-support"))]
pub(crate) use document::InsertError;
pub(crate) use document::{Compared, IdGenerator, Text, TextRole};
pub use document::{Document, SacmElement};
#[cfg(feature = "test-support")]
pub use document::{InsertError, SetAssertionDeclarationError};
pub use id::ElementId;
pub(crate) use id::is_ncname;
pub use kind::{AbstractClass, ElementKind, MetamodelPackage};
pub use lang_string::{LangString, MultiLangString};
pub(crate) use reference::{Flag, ReferenceEnd, reference_end_named};
