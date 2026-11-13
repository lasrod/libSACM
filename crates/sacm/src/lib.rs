//! Read, edit, validate and write assurance cases in the
//! [OMG Structured Assurance Case Metamodel (SACM) 2.3][sacm].
//!
//! An assurance case is a structured argument, backed by evidence, that a
//! system is acceptably safe, secure or otherwise fit for its purpose. SACM is
//! the OMG standard for representing such cases so that tools can exchange
//! them. This crate is an independent, tool-neutral implementation of SACM 2.3.
//! It aims to provide:
//!
//! - a typed model of the whole metamodel (packages, claims, asserted
//!   relationships, artifacts, terminology, interfaces and bindings), partly
//!   built (see [Status](#status)),
//! - XMI import and deterministic export, with a strict SACM 2.3 mode and an
//!   explicit tolerant mode for third-party and legacy files; built so far:
//!   [`xmi::read`], which reads `AssuranceCasePackage` and `ArgumentPackage`
//!   roots, the packages and Claims in them, their names and each Claim's
//!   `assertionDeclaration`, and refuses everything else, and
//!   [`xmi::write`], which writes everything a document holds,
//! - validation against the specification's constraints, reported as
//!   machine-readable diagnostics with stable codes; built so far: the
//!   diagnostic type and [`validation::validate`], which reports a
//!   `ModelElement` with no name,
//! - SACM-native editing, each operation checked in full before anything
//!   changes, with previews for destructive operations; built so far:
//!   [`edit::apply`] with the operations that create an
//!   `AssuranceCasePackage`, an `ArgumentPackage` and a `Claim`.
//!
//! # Status
//!
//! This crate is under active development and its API is not yet stable.
//! What is implemented, and how it is tested, is tracked requirement by
//! requirement in the [functionality matrix][matrix]. So far the crate
//! provides the document model, the metamodel's element kinds,
//! element names ([`SacmElement::name`](model::SacmElement::name)), each
//! Assertion's `assertionDeclaration` (whether the Assertion is asserted,
//! assumed, axiomatic, defeated, still needs support, or as cited; SACM 2.3
//! §11.8), read with [`SacmElement::assertion_declaration`][declaration],
//! and the first value types ([`LangString`](model::LangString),
//! [`MultiLangString`](model::MultiLangString) and
//! [`AssertionDeclaration`](model::AssertionDeclaration)), in [`model`],
//! and the [`Diagnostic`](diagnostic::Diagnostic) type that findings are
//! reported in, in [`diagnostic`]. [`validation::validate`] checks a
//! document and reports each `ModelElement` with no name as
//! [`Code::MULTIPLICITY_VIOLATION`](diagnostic::Code::MULTIPLICITY_VIOLATION).
//! [`sacm::xmi::read`](xmi::read) reads `AssuranceCasePackage` and
//! `ArgumentPackage` roots, bare or in an `xmi:XMI` element, and the packages
//! and Claims in them, with their names and each Claim's
//! `assertionDeclaration`. It refuses unsafe and malformed input and roots
//! that are not SACM packages with a diagnostic code, and content it cannot
//! represent yet as `Unsupported`. [`sacm::xmi::write`](xmi::write) writes a
//! document's roots and every element under them, with their names and
//! declarations, deterministically. [`edit::apply`] creates
//! AssuranceCasePackage, ArgumentPackage and Claim elements, with a name and
//! a given or generated id, or refuses with an [`EditError`](edit::EditError)
//! and changes nothing. Reading the other element kinds and attributes (such
//! as a `description` or a `gid`), validation rules other than the missing
//! name, and the other edit operations are not built yet.
//!
//! # Example
//!
//! ```
//! use sacm::model::AssertionDeclaration;
//! use sacm::xmi::{self, ReadOptions, WriteOptions};
//! use sacm::{diagnostic, validation};
//!
//! let input = br#"<?xml version="1.0" encoding="UTF-8"?>
//! <xmi:XMI xmlns:xmi="http://www.omg.org/spec/XMI/20131001"
//!          xmlns:sacm="http://www.omg.org/spec/SACM/20220301">
//!   <sacm:AssuranceCasePackage xmi:id="case"/>
//!   <sacm:ArgumentPackage xmi:id="arguments">
//!     <name lang="en" content="Arguments"/>
//!     <argumentElement xmi:type="sacm:Claim" xmi:id="c1" assertionDeclaration="needsSupport">
//!       <name lang="en" content="The system is safe"/>
//!     </argumentElement>
//!   </sacm:ArgumentPackage>
//! </xmi:XMI>"#;
//! let output = xmi::read(input, &ReadOptions::default())?;
//! let document = output.document();
//! for element in document.elements() {
//!     let name = element.name().and_then(|n| n.content());
//!     println!("{} {} {name:?}", element.kind(), element.id());
//! }
//! let c1 = document.get("c1").and_then(|c1| c1.assertion_declaration());
//! assert_eq!(c1, Some(AssertionDeclaration::NeedsSupport));
//!
//! // Check it against SACM's rules: the case package has no name.
//! let mut findings = output.diagnostics().to_vec();
//! findings.extend(validation::validate(document));
//! findings.sort();
//! for finding in &findings {
//!     println!("{finding}");
//! }
//! assert!(diagnostic::has_errors(&findings));
//!
//! // Write it back as XMI, in the one deterministic form.
//! let text = xmi::write(document, &WriteOptions::default())?;
//! assert!(text.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<xmi:XMI "));
//! // Saving is ordinary I/O: std::fs::write("case.sacm.xmi", &text)?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! Look up an element with [`model::Document::get`], walk every element with
//! [`model::Document::elements`], or move one level at a time with
//! [`children`](model::SacmElement::children) and
//! [`parent`](model::SacmElement::parent). Read an element's name with
//! [`name`](model::SacmElement::name), and an Assertion's declaration with
//! [`assertion_declaration`](model::SacmElement::assertion_declaration).
//! Check a document against SACM's rules with [`validation::validate`]; merge
//! its findings with the reader's and test them with
//! [`diagnostic::has_errors`].
//! Write a document back with [`xmi::write`], which returns the
//! XMI text for you to save.
//! Build a document from scratch, or extend one you read, with
//! [`edit::apply`] and the create operations such as [`edit::CreateClaim`];
//! the [`edit`] module example builds, validates and writes a complete
//! document, which [`xmi::read`] reads back.
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
//! [declaration]: model::SacmElement::assertion_declaration
//! [matrix]: https://github.com/lasrod/libSACM/blob/main/docs/matrix/functionality-matrix.md
#![cfg_attr(docsrs, feature(doc_cfg))]

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
