//! The SACM document model: [`Document`], element identity, element kinds and
//! the value types elements hold.
//!
//! A [`Document`] owns one interchange unit: its root packages and every
//! element under them. Each element has an [`ElementId`] (its `xmi:id`) and an
//! [`ElementKind`] (its concrete SACM class), and is read through a
//! [`SacmElement`] view that borrows the document.
//!
//! [`ElementKind`], [`AbstractClass`] and [`MetamodelPackage`] describe the
//! SACM 2.3 metamodel itself: which class a kind is, which package defines it,
//! and which abstract classes it specializes.
//!
//! [`LangString`] and [`AssertionDeclaration`] are values, not elements: they
//! have no id and no kind, and you can build them freely.
//!
//! # Examples
//!
//! ```
//! use sacm::model::{Document, ElementId, ElementKind};
//!
//! // Built with the unstable test-support feature until the XMI reader exists.
//! let mut doc = Document::new();
//! let arguments = ElementId::from("arguments");
//! doc.insert(None, ElementKind::ArgumentPackage, arguments.clone()).unwrap();
//! doc.insert(Some(&arguments), ElementKind::Claim, ElementId::from("claim_top")).unwrap();
//! doc.insert(Some(&arguments), ElementKind::Claim, ElementId::from("claim_sub")).unwrap();
//!
//! let package = doc.get("arguments").expect("the document has it");
//! println!("{} is an {}", package.id(), package.kind());
//! for child in package.children() {
//!     println!("  {} {}", child.kind(), child.id());
//! }
//!
//! let claim = doc.get("claim_sub").expect("the document has it");
//! let parent = claim.parent().expect("a claim here has a parent");
//! assert_eq!(parent.id().as_str(), "arguments");
//! ```
//!
//! The value types:
//!
//! ```
//! use sacm::model::{AssertionDeclaration, LangString};
//!
//! let mut name = LangString::new("The system is acceptably safe").with_lang("en");
//! assert_eq!(name.lang(), Some("en"));
//!
//! // Absent and empty are different values.
//! assert_ne!(LangString::new(""), LangString::default());
//! name.set_lang(None);
//! assert_eq!(name.lang(), None);
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
mod document;
mod id;
mod kind;
mod lang_string;

pub use argumentation::{AssertionDeclaration, ParseAssertionDeclarationError};
#[cfg(feature = "test-support")]
pub use document::InsertError;
pub use document::{Document, SacmElement};
pub use id::ElementId;
pub use kind::{AbstractClass, ElementKind, MetamodelPackage};
pub use lang_string::LangString;
