//! Read, edit, validate and write assurance cases in the
//! [OMG Structured Assurance Case Metamodel (SACM) 2.3][sacm].
//!
//! An assurance case is a structured argument, backed by evidence, that a
//! system is acceptably safe, secure or otherwise fit for its purpose. SACM is
//! the OMG standard for exchanging such cases between tools, as XMI files.
//!
//! This crate is an independent, tool-neutral implementation of SACM 2.3. It
//! claims the four model compliance points of clause 2; the
//! [conformance statement][compliance] says what that covers. The crate is
//! pre-1.0, and its API is not stable yet.
//!
//! # Entry points
//!
//! A [`Document`](model::Document) is the centre: read it, work on it, write it.
//!
//! | Module | What it is for | Start at |
//! |---|---|---|
//! | [`xmi`] | Read and write XMI files | [`xmi::read`], [`xmi::write`] |
//! | [`model`] | Inspect a document and its elements | [`Document::get`](model::Document::get), [`SacmElement`](model::SacmElement) |
//! | [`validation`] | Check a document against the SACM 2.3 rules | [`validation::validate`] |
//! | [`edit`] | Change a document | [`edit::apply`] |
//! | [`compare`] | Find what differs between two documents | [`compare::semantic_compare`], [`compare::changes`] |
//! | [`diagnostic`] | Read what `read` and `validate` report | [`Diagnostic`](diagnostic::Diagnostic), [`diagnostic::has_errors`] |
//!
//! All four package kinds are read, written, created, edited and validated, as
//! roots or nested: `AssuranceCasePackage`, `ArgumentPackage`,
//! `ArtifactPackage` and `TerminologyPackage`, with their elements, interfaces
//! and bindings.
//!
//! # Example
//!
//! Read a document, inspect it, edit it, validate it and write it:
//!
//! ```
//! use sacm::edit::{self, CreateClaim};
//! use sacm::model::ElementKind;
//! use sacm::xmi::{self, ReadOptions, WriteOptions};
//! use sacm::{diagnostic, validation};
//!
//! // Read. The input is the bytes of an XMI file, such as
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
//! let created = edit::apply(&mut document, CreateClaim::new("arguments", "The brake is tested"))?;
//! assert_eq!(created.created_ids(), ["claim_1"]);
//!
//! // Validate: no finding is an Error.
//! let findings = validation::validate(&document);
//! assert!(!diagnostic::has_errors(&findings));
//!
//! // Write, in strict mode by default. Saving the text is ordinary I/O, such
//! // as `std::fs::write("case.sacm.xmi", &text)?`.
//! let text = xmi::write(&document, &WriteOptions::default())?;
//!
//! // The text reads back as the same document.
//! let back = xmi::read(text.as_bytes(), &ReadOptions::default())?.into_document();
//! assert_eq!(back, document);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # Key rules
//!
//! ## Strict and tolerant modes
//!
//! Reading and writing each take a [`Mode`](xmi::Mode), strict by default.
//! Neither mode drops model content silently. Comments and processing instructions are
//! not kept.
//!
//! | | Strict (the default) | Tolerant |
//! |---|---|---|
//! | `read` | Refuses, or reports as an Error, what is not SACM 2.3. | Opens files from other tools and keeps most of what strict refuses, with a Warning; [Modes](xmi#modes) lists what it still refuses. |
//! | `write` | Refuses output that would not be schema-valid. | Writes back what a tolerant read kept. |
//!
//! - **Nothing dropped.** Content that is not SACM 2.3 where it stands is kept as
//!   written, as [preserved content](xmi::read#preserved-content), with a diagnostic.
//! - **A strict write never validates.** It refuses only output that would not be
//!   schema-valid, and a document that holds preserved content. A draft with a model
//!   Error, such as an element with no name, saves strictly.
//! - **Model checks are `validate`'s.** Neither `read` nor `write` runs
//!   [`validation::validate`]; call it yourself.
//! - **To check that a file conforms,** read it in strict mode, run `validate`, and
//!   test both results: [`ReadOutput::has_errors`](xmi::ReadOutput::has_errors) and
//!   [`diagnostic::has_errors`]. A tolerant read alone is no conformance check.
//! - **To keep what a tolerant read preserved,** write in tolerant mode:
//!   `WriteOptions::default().with_mode(Mode::Tolerant)`.
//! - **The details.** [Modes](xmi#modes) lists exactly what each mode refuses and keeps.
//!
//! ## Editing
//!
//! Every change is an operation given to [`edit::apply`]. The [`edit`] module
//! lists them all.
//!
//! - **Atomic.** A refused operation changes nothing.
//! - **No new dangling reference.** No edit makes a reference, or XMI metadata, name an
//!   id the document lacks; one read from a file stays until fixed.
//! - **Savable.** No edit stores text XML 1.0 cannot represent. A document read and
//!   edited writes in tolerant mode while it has a root, and in strict mode too if a
//!   strict write accepted it before, but for what a copy brings
//!   ([round trip](xmi::write#round-trip)).
//! - **Ask first.** [`edit::check`] says whether an operation would be accepted;
//!   [`edit::preview`] says what it would do.
//! - **Batches.** [`edit::batch`] and [`edit::apply_all`] apply several operations as one change.
//! - **Deletes.** [`edit::DeleteElement`] cascades only where its
//!   [policies](edit#deleting-an-element) allow; each refuses by default.
//! - **Undo.** The crate keeps no history; an application keeps
//!   [whole-document copies](edit#undo-and-redo).
//!
//! ## References and diagnostics
//!
//! - **A reference is an id.** An element holds the id it names;
//!   [`Document::get`](model::Document::get) resolves it, and
//!   [`Document::referrers`](model::Document::referrers) lists what names an id.
//! - **Other files are never opened.** A reference into another file is held as a
//!   [`model::Href`], and a child held in another file as a [`model::Proxy`]. Both are
//!   written back.
//! - **Codes are stable.** Each [`Diagnostic`](diagnostic::Diagnostic) has a code, such
//!   as SACM-REF-001, and a severity. Match on the code, not on the message text.
//!
//! # Features
//!
//! Both are off by default.
//!
//! - **`serde`.** `Serialize` and `Deserialize` for edits, diagnostics and refusals; see
//!   [below](#the-serde-feature).
//! - **`test-support`.** Not stable API: it may change or disappear in any release.
//!   Enable it only under `[dev-dependencies]`. It makes these public, for tests:
//!   - `Document::insert` and `Document::set_assertion_declaration`, to build documents;
//!   - `edit::apply_unchecked_text`, to build one a write refuses;
//!   - the constructors of diagnostics, to build diagnostics.
//!
//! ## The `serde` feature
//!
//! An edit can cross a process boundary, travel between users or be kept as an
//! audit log.
//!
//! ```toml
//! sacm = { version = "0.4", features = ["serde"] }
//! ```
//!
//! - **What serializes.** [`edit::Operation`] and each operation struct,
//!   [`edit::Mutation`], [`edit::EditError`], [`diagnostic::Diagnostic`],
//!   [`xmi::ReadError`], [`xmi::WriteError`], [`compare::Changes`] and the values
//!   they carry.
//! - **Not a `Document`.** XMI is its only serialization.
//! - **A deserialized operation proves nothing.** [`edit::check`] and [`edit::apply`]
//!   check it as they check one built in Rust.
//! - **The JSON shape is stable API.** [ADR 0030][adr-0030] documents it. It follows the
//!   crate's semver; a diagnostic's `message` text does not.
//! - **Strict input.** Unknown keys, names and codes are refused, so JSON from a newer
//!   release may be refused.
//! - **Self-describing formats only,** such as JSON. The `"type"` tags do not work with
//!   bincode or postcard.
//!
//! The example uses `serde_json`; add it yourself.
//!
//! ```
//! # #[cfg(feature = "serde")]
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! use sacm::edit::{self, CreateArgumentPackage, CreateClaim, Mutation, Operation};
//! use sacm::model::Document;
//!
//! let mut document = Document::new();
//! let arguments = CreateArgumentPackage::root("Arguments").with_id("arguments");
//! edit::apply(&mut document, arguments)?;
//!
//! // An operation received as JSON, from a user interface, say.
//! let json = r#"{"type":"CreateClaim","parent":"arguments","id":null,"name":{"lang":"en","content":"The system is safe"}}"#;
//! let operation: Operation = serde_json::from_str(json)?;
//! let mutation = edit::apply(&mut document, operation)?;
//!
//! // What it did, as a record for the audit log.
//! let record = serde_json::to_string(&mutation)?;
//! assert!(record.starts_with(r#"{"operation":{"type":"CreateClaim","parent":"arguments","#));
//! assert!(record.ends_with(r#""created_ids":["claim_1"],"changed_ids":[],"deleted_ids":[]}"#));
//! let read: Mutation = serde_json::from_str(&record)?;
//! assert_eq!(read, mutation);
//!
//! // A refusal carries its catalog code after its type.
//! let error = edit::check(&document, &CreateClaim::new("nowhere", "C1").into()).unwrap_err();
//! assert_eq!(
//!     serde_json::to_string(&error)?,
//!     r#"{"type":"ParentNotFound","code":"SACM-CMD-001","parent":"nowhere"}"#
//! );
//! # Ok(())
//! # }
//! # #[cfg(not(feature = "serde"))]
//! # fn main() {}
//! ```
//!
//! # More
//!
//! - **How to use it.** The [user guide][guide] walks through reading, inspecting,
//!   validating, editing and saving, step by step.
//! - **What SACM is.** The [SACM primer][primer] explains the concepts, for readers who
//!   have not read the specification.
//! - **What is built.** The [functionality matrix][matrix] is the full inventory: every
//!   requirement, its status and the tests behind it.
//! - **What is claimed.** The [conformance statement][compliance] covers the four
//!   compliance points and where they stop.
//! - **Every diagnostic code.** The [diagnostics catalog][diagnostics].
//! - **How the standard is read.** The [settled decisions][decisions] where SACM 2.3 is
//!   silent or unclear.
//!
//! [sacm]: https://www.omg.org/spec/SACM/2.3
//! [guide]: https://github.com/lasrod/libSACM/blob/main/crates/sacm/GUIDE.md
//! [primer]: https://github.com/lasrod/libSACM/blob/main/crates/sacm/PRIMER.md
//! [matrix]: https://github.com/lasrod/libSACM/blob/main/docs/matrix/functionality-matrix.md
//! [compliance]: https://github.com/lasrod/libSACM/blob/main/docs/sacm/compliance-points.md
//! [diagnostics]: https://github.com/lasrod/libSACM/blob/main/docs/sacm/diagnostics.md
//! [decisions]: https://github.com/lasrod/libSACM/blob/main/docs/sacm/decisions.md
//! [adr-0030]: https://github.com/lasrod/libSACM/blob/main/docs/adr/0030-serde-for-edit-and-diagnostic-values.md
#![cfg_attr(docsrs, feature(doc_cfg))]
// The platform APIs that compile for wasm32-unknown-unknown but fail at run
// time are listed in clippy.toml; the library never calls them (LIB-QA-017).
#![cfg_attr(not(test), deny(clippy::disallowed_methods, clippy::disallowed_types))]

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

/// The user guide's examples, compiled and run as doctests.
#[cfg(doctest)]
#[doc = include_str!("../GUIDE.md")]
struct GuideDoctests;

/// The SACM primer's examples, if it has any, compiled and run as doctests.
#[cfg(doctest)]
#[doc = include_str!("../PRIMER.md")]
struct PrimerDoctests;

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
