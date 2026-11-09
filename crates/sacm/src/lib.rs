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
//!   relationships, artifacts, terminology, interfaces and bindings); built so
//!   far: the element kinds and the read-only document,
//! - XMI import and deterministic export, with a strict SACM 2.3 mode and an
//!   explicit tolerant mode for third-party and legacy files (planned),
//! - validation against the specification's constraints, reported as
//!   machine-readable diagnostics with stable codes; built so far: the
//!   diagnostic type,
//! - SACM-native editing with previews for destructive operations (planned).
//!
//! # Status
//!
//! This crate is under active development and its API is not yet stable.
//! What is implemented, and how it is tested, is tracked requirement by
//! requirement in the [functionality matrix][matrix]. So far the crate
//! provides the read-only document model and the metamodel's element kinds,
//! in [`model`], and the [`Diagnostic`](diagnostic::Diagnostic) type that
//! findings are reported in, in [`diagnostic`]. XMI import and export,
//! validation and editing are not built yet, so no diagnostic is emitted yet.
//!
//! Look up an element with [`model::Document::get`], walk every element with
//! [`model::Document::elements`], or move one level at a time with
//! [`children`](model::SacmElement::children) and
//! [`parent`](model::SacmElement::parent).
//!
//! # Features
//!
//! - `test-support` (off by default): makes `Document::insert` and
//!   `InsertError` public in [`model`], so tests can build documents, and
//!   makes the constructors of `Code`, `SourceLocation` and `Diagnostic`
//!   public in [`diagnostic`], so tests can build diagnostics. It is **not
//!   part of the stable API** and may change or disappear in any release.
//!   Enable it only under `[dev-dependencies]`.
//!
//! [sacm]: https://www.omg.org/spec/SACM/2.3
//! [matrix]: https://github.com/lasrod/libSACM/blob/main/docs/matrix/functionality-matrix.md
#![cfg_attr(docsrs, feature(doc_cfg))]

pub mod diagnostic;
pub mod model;

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
