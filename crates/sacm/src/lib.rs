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
//!   and the packages and Claims in them, with their names, `gid`s and each
//!   Assertion's `assertionDeclaration`, and the descriptions, notes,
//!   implementation constraints and tagged values of each
//!   ([`descriptions`](model::SacmElement::descriptions),
//!   [`notes`](model::SacmElement::notes),
//!   [`tagged_values`](model::SacmElement::tagged_values) and the like), with
//!   their [`content`](model::SacmElement::content) and, on a tagged value,
//!   [`key`](model::SacmElement::key). Every element's citation and
//!   abstract-form flags and references are read too
//!   ([`is_citation`](model::SacmElement::is_citation),
//!   [`cited_element`](model::SacmElement::cited_element),
//!   [`is_abstract`](model::SacmElement::is_abstract),
//!   [`abstract_form`](model::SacmElement::abstract_form)), in the attribute
//!   or the element form; a reference is held as the id it names, which
//!   [`Document::get`](model::Document::get) resolves, as in
//!   `c1.cited_element().and_then(|id| document.get(id))`; one that names no
//!   element is kept, and [`validation::validate`] reports it as
//!   SACM-REF-001, so a strict save refuses it; see
//!   [References](xmi::read#references).
//!   In an `ArgumentPackage` it also reads `ArgumentReasoning`,
//!   `ArtifactReference` and `ArgumentGroup` elements and the five asserted
//!   relationships (`AssertedInference`, `AssertedEvidence`,
//!   `AssertedContext`, `AssertedArtifactSupport`, `AssertedArtifactContext`),
//!   with each relationship's [`sources`](model::SacmElement::sources),
//!   [`target`](model::SacmElement::target),
//!   [`reasoning`](model::SacmElement::reasoning) and
//!   [`is_counter`](model::SacmElement::is_counter), each Assertion's
//!   [`meta_claims`](model::SacmElement::meta_claims), each reasoning's
//!   [`structure`](model::SacmElement::structure), each artifact reference's
//!   [`referenced_artifact_elements`](model::SacmElement::referenced_artifact_elements)
//!   and each group's [`group_members`](model::SacmElement::group_members).
//!   It refuses what it cannot represent yet rather than drop it. An element
//!   without an `xmi:id` gets a generated one, such as `claim_1`, which
//!   `write` saves; see [Ids](xmi::read#ids).
//! - [`xmi::write`] writes a document deterministically, and by default
//!   refuses one that validation reports an Error for.
//! - [`validation::validate`] reports each element that has no name, each
//!   `content` or `key` text that is empty or gives one language twice, each
//!   tagged value without a key, each id that is not an XML `NCName`, each
//!   `gid` two elements share, each element with more than one description,
//!   each `ArgumentPackage` that nests one and also holds other
//!   argumentation elements, each reference that names no element or an
//!   element of the wrong kind, each element with a `citedElement` whose
//!   `isCitation` is not true, each `abstractForm` that breaks a §8.2 rule,
//!   each asserted relationship without a source or a target, each evidence
//!   or artifact relationship whose ends are not artifact references (§11.15,
//!   §11.17, §11.18), and each id a reference end repeats, as
//!   [`Diagnostic`](diagnostic::Diagnostic)s with stable codes.
//! - [`edit::apply`] creates `AssuranceCasePackage`, `ArgumentPackage`,
//!   `Claim`, `ArgumentReasoning`, `ArtifactReference` and `ArgumentGroup`
//!   elements and the five asserted relationships
//!   ([`edit::CreateAssertedInference`] and the like), and descriptions,
//!   implementation constraints, notes and tagged values on them
//!   ([`edit::CreateNote`], [`edit::CreateTaggedValue`] and the like); a
//!   refused edit changes nothing, and an edit refuses a reference that
//!   names no element or an element its end's declared type does not admit.
//!   The stricter §11.15, §11.17 and §11.18 rules, that evidence and
//!   artifact relationships link `ArtifactReference`s, are left to
//!   [`validation::validate`].
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
//! // Validate. This release checks names, ids, gids, descriptions, the
//! // content and key of descriptions, implementation constraints, notes and
//! // tagged values, what an ArgumentPackage holds, references, citations,
//! // abstract forms and the ends of asserted relationships.
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
//! files from other tools. A strict read reports the variations it
//! recognises as Errors instead of Warnings (in this release, only
//! `xsi:type`). Neither mode validates what it reads, so to check a file,
//! also run [`validation::validate`] and test both lists with
//! [`diagnostic::has_errors`]. Save in strict mode, the default, for a file
//! you hand on.
//!
//! Walk a document with [`model::Document::elements`], or one level at a
//! time with [`children`](model::SacmElement::children) and
//! [`parent`](model::SacmElement::parent). Follow a reference, such as
//! [`cited_element`](model::SacmElement::cited_element) or an asserted
//! relationship's [`sources`](model::SacmElement::sources) and
//! [`target`](model::SacmElement::target), with [`model::Document::get`]. To report reading and validation
//! together, merge [`ReadOutput::diagnostics`](xmi::ReadOutput::diagnostics)
//! with the findings of [`validation::validate`] and sort the result. The
//! [`edit`] module builds a document from scratch.
//!
//! # Arguments
//!
//! In SACM an argument is Claims linked by asserted relationships (§11.13).
//! An `AssertedInference` (§11.14) asserts that its sources, the premises,
//! infer its target, the conclusion. An `AssertedContext` (§11.16) gives its
//! sources as the context of its target. An `AssertedEvidence` (§11.15)
//! supports its target with evidence: its sources are `ArtifactReference`s
//! (§11.9), which name the artifacts. Artifacts themselves (§12, the
//! `ArtifactPackage`) are not supported yet, so for now an
//! `ArtifactReference` can name only elements of argument and
//! assurance-case packages, or nothing. Build them with [`edit`], and read
//! them back with [`sources`](model::SacmElement::sources) and
//! [`target`](model::SacmElement::target):
//!
//! ```
//! use sacm::edit::{self, CreateArgumentPackage, CreateArtifactReference};
//! use sacm::edit::{CreateAssertedEvidence, CreateAssertedInference, CreateClaim};
//! use sacm::model::{Document, LangString};
//! use sacm::validation;
//!
//! let mut document = Document::new();
//! let en = |text: &str| LangString::new(text).with_lang("en");
//! edit::apply(&mut document, CreateArgumentPackage::root(en("Arguments")).with_id("arguments"))?;
//! edit::apply(&mut document, CreateClaim::new("arguments", en("The brake is safe")).with_id("c1"))?;
//! edit::apply(&mut document, CreateClaim::new("arguments", en("The brake stops the vehicle")).with_id("c2"))?;
//! edit::apply(&mut document, CreateClaim::new("arguments", en("The brake is tested")).with_id("c3"))?;
//! // The premises c2 and c3 infer the conclusion c1.
//! let inference = CreateAssertedInference::new("arguments", en("Function and test show safety"), ["c2", "c3"], "c1");
//! edit::apply(&mut document, inference.with_id("i1"))?;
//!
//! let i1 = document.get("i1").expect("just created");
//! let premises: Vec<_> = i1.sources().iter().filter_map(|id| document.get(id)?.name()?.content()).collect();
//! assert_eq!(premises, ["The brake stops the vehicle", "The brake is tested"]);
//! let conclusion = i1.target().and_then(|id| document.get(id)?.name()?.content());
//! assert_eq!(conclusion, Some("The brake is safe"));
//!
//! // The evidence a1, an ArtifactReference that names nothing yet, supports c3.
//! edit::apply(&mut document, CreateArtifactReference::new("arguments", en("The brake test report")).with_id("a1"))?;
//! let evidence = CreateAssertedEvidence::new("arguments", en("The report shows the test"), ["a1"], "c3");
//! edit::apply(&mut document, evidence.with_id("e1"))?;
//! assert_eq!(document.get("e1").and_then(|e1| e1.target()).map(|id| id.as_str()), Some("c3"));
//! assert!(validation::validate(&document).is_empty());
//! # Ok::<(), sacm::edit::EditError>(())
//! ```
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
