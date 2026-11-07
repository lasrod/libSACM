//! Read, edit, validate and write assurance cases in the
//! [OMG Structured Assurance Case Metamodel (SACM) 2.3][sacm].
//!
//! An assurance case is a structured argument, backed by evidence, that a
//! system is acceptably safe, secure or otherwise fit for its purpose. SACM is
//! the OMG standard for representing such cases so that tools can exchange
//! them. This crate is an independent, tool-neutral implementation of SACM 2.3:
//!
//! - a typed model of the whole metamodel (packages, claims, asserted
//!   relationships, artifacts, terminology, interfaces and bindings),
//! - XMI import and deterministic export, with a strict SACM 2.3 mode and an
//!   explicit tolerant mode for third-party and legacy files,
//! - validation against the specification's constraints, reported as
//!   machine-readable diagnostics with stable codes,
//! - SACM-native editing with previews for destructive operations.
//!
//! # Status
//!
//! This crate is under active development and its API is not yet stable.
//! What is implemented, and how it is tested, is tracked requirement by
//! requirement in the [functionality matrix][matrix].
//!
//! ```
//! assert_eq!(sacm::SACM_VERSION, "2.3");
//! println!("sacm {} implements SACM {}", sacm::VERSION, sacm::SACM_VERSION);
//! ```
//!
//! [sacm]: https://www.omg.org/spec/SACM/2.3
//! [matrix]: https://github.com/lasrod/libSACM/blob/main/docs/matrix/functionality-matrix.md
#![cfg_attr(docsrs, feature(doc_cfg))]

/// The version of this crate, as published.
///
/// ```
/// assert!(!sacm::VERSION.is_empty());
/// ```
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The revision of the OMG SACM specification this crate implements.
///
/// Strict-mode export always writes this revision, whatever revision a
/// document was read from.
///
/// ```
/// assert_eq!(sacm::SACM_VERSION, "2.3");
/// ```
pub const SACM_VERSION: &str = "2.3";
