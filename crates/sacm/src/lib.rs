//! Read, edit, validate and write assurance cases in the
//! [OMG Structured Assurance Case Metamodel (SACM) 2.3][sacm].
//!
//! An assurance case is a structured argument, backed by evidence, that a
//! system is acceptably safe, secure or otherwise fit for its purpose. In
//! SACM, the argument is made of claims, the relationships between them and
//! the artifacts that back them, grouped in packages. SACM is the OMG
//! standard for representing such cases so that tools can exchange them, as
//! XMI files. This crate is an independent, tool-neutral implementation of
//! SACM 2.3. It is under active development, and its API is not stable yet.
//!
//! # What works today
//!
//! - [`xmi::read`] reads `AssuranceCasePackage` and `ArgumentPackage` roots
//!   and the packages and Claims in them, with their names and each Claim's
//!   `assertionDeclaration`. It refuses what it cannot represent yet rather
//!   than drop it.
//! - [`xmi::write`] writes a document deterministically, and by default
//!   refuses one that validation reports an Error for.
//! - [`validation::validate`] reports each element that has no name, and
//!   each element whose id is not an XML `NCName`, as
//!   [`Diagnostic`](diagnostic::Diagnostic)s with stable codes.
//! - [`edit::apply`] creates `AssuranceCasePackage`, `ArgumentPackage` and
//!   `Claim` elements; a refused edit changes nothing.
//! - [`compare::semantic_compare`] lists every difference in SACM content
//!   between two documents, and `==` on a [`Document`](model::Document) is
//!   true when there is none.
//!
//! The [functionality matrix][matrix] tracks every requirement: what is
//! built, what is planned, and the tests behind each.
//!
//! # Example
//!
//! Load a document, inspect it, edit it, validate it and save it:
//!
//! ```
//! use sacm::diagnostic;
//! use sacm::edit::{self, CreateClaim};
//! use sacm::model::{ElementKind, LangString};
//! use sacm::validation;
//! use sacm::xmi::{self, ReadOptions, WriteOptions};
//!
//! // Load. The input is the bytes of an XMI file, such as
//! // `std::fs::read("case.sacm.xmi")?`; here it is inline.
//! let input = br#"<?xml version="1.0" encoding="UTF-8"?>
//! <sacm:AssuranceCasePackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
//!     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="case">
//!   <name lang="en" content="Braking case"/>
//!   <argumentPackage xmi:id="arguments">
//!     <name lang="en" content="Arguments"/>
//!     <argumentElement xmi:type="sacm:Claim" xmi:id="c1">
//!       <name lang="en" content="The brake stops the vehicle"/>
//!     </argumentElement>
//!   </argumentPackage>
//! </sacm:AssuranceCasePackage>"#;
//! let output = xmi::read(input, &ReadOptions::default())?;
//! assert!(output.diagnostics().is_empty());
//! let mut document = output.into_document();
//!
//! // Inspect.
//! let c1 = document.get("c1").expect("the Claim was read");
//! assert_eq!(c1.kind(), ElementKind::Claim);
//! assert_eq!(c1.name().and_then(|name| name.content()), Some("The brake stops the vehicle"));
//! assert_eq!(c1.parent().map(|parent| parent.id().as_str()), Some("arguments"));
//!
//! // Edit: add a Claim to the ArgumentPackage. Its id is generated.
//! let name = LangString::new("The brake is tested").with_lang("en");
//! let created = edit::apply(&mut document, CreateClaim::new("arguments", name))?;
//! assert_eq!(created.created_ids()[0].as_str(), "claim_1");
//!
//! // Validate. This release checks that every element has a name and an
//! // `NCName` id.
//! let findings = validation::validate(&document);
//! assert!(!diagnostic::has_errors(&findings));
//!
//! // Save. The write is strict by default; writing the file is ordinary I/O,
//! // such as std::fs::write("case.sacm.xmi", &text).
//! let text = xmi::write(&document, &WriteOptions::default())?;
//!
//! // The saved text reads back as the same document.
//! let back = xmi::read(text.as_bytes(), &ReadOptions::default())?.into_document();
//! assert_eq!(back, document);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! A strict save refuses a document with a validation Error, as
//! [`WriteError::Invalid`](xmi::WriteError::Invalid); a draft is saved with
//! `WriteOptions::default().with_mode(Mode::Tolerant)` ([`xmi::Mode`]). A
//! later release may add validation rules, so a strict save may then refuse
//! a document it saved before.
//!
//! Which [`xmi::Mode`] to use: read in tolerant mode, the default, to open
//! files from other tools, and in strict mode to have each variation from
//! SACM 2.3 reported as an Error. Save in strict mode, the default, for a
//! file you hand on.
//!
//! Walk a document with [`model::Document::elements`], or one level at a
//! time with [`children`](model::SacmElement::children) and
//! [`parent`](model::SacmElement::parent). To report reading and validation
//! together, merge [`ReadOutput::diagnostics`](xmi::ReadOutput::diagnostics)
//! with the findings of [`validation::validate`] and sort the result. The
//! [`edit`] module builds a document from scratch.
//!
//! # Features
//!
//! - `test-support` (off by default): makes `Document::insert`,
//!   `InsertError`, `Document::set_assertion_declaration` and
//!   `SetAssertionDeclarationError` public in [`model`], so tests can build
//!   documents, and
//!   makes the constructors of `Code`, `SourceLocation` and `Diagnostic`
//!   and the `with_*` builders of `Diagnostic` public in [`diagnostic`], so
//!   tests can build diagnostics. It is **not part of the stable API** and
//!   may change or disappear in any release.
//!   Enable it only under `[dev-dependencies]`.
//!
//! [sacm]: https://www.omg.org/spec/SACM/2.3
//! [matrix]: https://github.com/lasrod/libSACM/blob/main/docs/matrix/functionality-matrix.md
#![cfg_attr(docsrs, feature(doc_cfg))]

pub mod compare;
pub mod diagnostic;
pub mod edit;
pub mod model;
pub mod validation;
pub mod xmi;

/// The crate README's examples, compiled and run as doctests.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

/// The version of this crate, as published.
///
/// ```
/// assert!(!sacm::VERSION.is_empty());
/// ```
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The revision of the OMG SACM specification this crate implements.
///
/// ```
/// assert_eq!(sacm::SACM_VERSION, "2.3");
/// ```
pub const SACM_VERSION: &str = "2.3";
