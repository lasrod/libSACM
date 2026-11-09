//! The SACM document model: [`Document`], element identity and element kinds.
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
//! # Examples
//!
//! ```
//! use sacm::model::{AbstractClass, Document, ElementKind};
//!
//! let doc = Document::new();
//! assert_eq!(doc.element_count(), 0);
//!
//! assert!(ElementKind::Claim.is_a(AbstractClass::Assertion));
//! ```

mod document;
mod id;
mod kind;

#[cfg(feature = "test-support")]
pub use document::InsertError;
pub use document::{Document, SacmElement};
pub use id::ElementId;
pub use kind::{AbstractClass, ElementKind, MetamodelPackage};
