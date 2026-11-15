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
//!   [`validation::validate`]. It also changes existing elements of every
//!   kind: their names, `gid`s, flags and `assertionDeclaration`s
//!   ([`edit::SetName`] and the like), the `content` and `key` of
//!   descriptions, implementation constraints, notes and tagged values
//!   ([`edit::SetContent`],
//!   [`edit::SetKey`]), every reference end, within its bounds
//!   ([`edit::SetReferences`], [`edit::AddReference`],
//!   [`edit::RemoveReference`]), and the order of an element's children or
//!   of the roots ([`edit::ReorderElements`]). [`edit::check`] answers
//!   whether `apply` would accept an operation, without changing the
//!   document.
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
//! [`edit`] module builds a document from scratch and changes an existing
//! one; its tables list which operation edits which SACM feature.
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
//! Change an argument after it is built: retarget a relationship and add a
//! source with [`edit::SetReferences`] and [`edit::AddReference`], mark
//! counter-evidence with [`edit::SetFlag`], and reorder a package's elements
//! with [`edit::ReorderElements`]:
//!
//! ```
//! use sacm::edit::{self, AddReference, CreateArgumentPackage, CreateArtifactReference};
//! use sacm::edit::{CreateAssertedEvidence, CreateClaim, ReorderElements, SetFlag, SetReferences};
//! use sacm::model::{Document, Flag, LangString, ReferenceEnd};
//! use sacm::validation;
//!
//! let mut doc = Document::new();
//! let en = |text: &str| LangString::new(text).with_lang("en");
//! edit::apply(&mut doc, CreateArgumentPackage::root(en("Arguments")).with_id("arguments"))?;
//! edit::apply(&mut doc, CreateClaim::new("arguments", en("The brake is safe")).with_id("c1"))?;
//! edit::apply(&mut doc, CreateClaim::new("arguments", en("The brake is tested")).with_id("c2"))?;
//! edit::apply(&mut doc, CreateArtifactReference::new("arguments", en("Bench test report")).with_id("a1"))?;
//! edit::apply(&mut doc, CreateArtifactReference::new("arguments", en("Road test report")).with_id("a2"))?;
//! edit::apply(&mut doc, CreateArtifactReference::new("arguments", en("Field failure report")).with_id("a3"))?;
//!
//! // Link evidence to a claim: a1 supports c1 ...
//! let evidence = CreateAssertedEvidence::new("arguments", en("The tests show it"), ["a1"], "c1");
//! edit::apply(&mut doc, evidence.with_id("e1"))?;
//! // ... retarget it to the claim it supports, c2, and add a second report.
//! edit::apply(&mut doc, SetReferences::new("e1", ReferenceEnd::Target, ["c2"]))?;
//! edit::apply(&mut doc, AddReference::new("e1", ReferenceEnd::Source, "a2"))?;
//! assert_eq!(doc.get("e1").map(|e1| e1.sources().len()), Some(2));
//!
//! // Counter-evidence (§11.13 isCounter): a3 speaks against c1.
//! let against = CreateAssertedEvidence::new("arguments", en("A failure was seen"), ["a3"], "c1");
//! edit::apply(&mut doc, against.with_id("e2"))?;
//! edit::apply(&mut doc, SetFlag::new("e2", Flag::IsCounter, true))?;
//! assert!(doc.get("e2").is_some_and(|e2| e2.is_counter()));
//!
//! // Put c2 before c1; the other children keep their positions.
//! edit::apply(&mut doc, ReorderElements::new("arguments", ["c2", "c1"]))?;
//! let order: Vec<&str> = doc.get("arguments").into_iter().flat_map(|p| p.children()).map(|c| c.id().as_str()).collect();
//! assert_eq!(order, ["c2", "c1", "a1", "a2", "a3", "e1", "e2"]);
//! assert!(validation::validate(&doc).is_empty());
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
//! - `serde` (off by default): see below.
//!
//! ## The `serde` feature
//!
//! Enable it in `Cargo.toml`:
//!
//! ```toml
//! sacm = { version = "0.1", features = ["serde"] }
//! ```
//!
//! The examples use `serde_json`; add it yourself.
//!
//! With `serde`, every operation ([`edit::Operation`] and each operation
//! struct), [`edit::Mutation`], [`edit::EditError`],
//! [`diagnostic::Diagnostic`] and the values they carry (ids, kinds, names,
//! flags, reference ends, codes, severities and locations) implement
//! `Serialize` and `Deserialize`. So an edit can cross a process boundary,
//! travel between users or be kept as an audit log. A
//! [`Document`](model::Document) does not: XMI is its only serialization.
//!
//! The JSON shape is documented in [ADR 0030][adr-0030] and is **stable**,
//! as [`operation_name`](edit::Operation::operation_name) is: an operation
//! is tagged `"type"` with its operation name, keys are the snake-case
//! accessor names (for an `EditError`, its variant's field names), every
//! key is written, and on input a key may be left out only where a builder
//! sets a default: an operation's `id`, `structure`, `reasoning`,
//! `is_counter`, `referenced_artifact_elements` and `group_members`; a
//! `LangString`'s `lang` and `content`; and a `Diagnostic`'s `clause`,
//! `affected` and `location`. A package create's `"parent": null` means a root; leaving out
//! `parent` is refused. Unknown keys, names and codes are refused. A
//! deserialized operation proves nothing: [`edit::check`] and
//! [`edit::apply`] check it as they check one built in Rust.
//!
//! Adding a variant, or a key whose default keeps the old meaning, is not a
//! breaking change, so JSON from an older release still reads. JSON from a
//! newer release may be refused. A diagnostic's `message` text is not part
//! of the promise. Use a self-describing format such as JSON; the `"type"`
//! tags do not work with bincode or postcard.
//!
//! ```
//! # #[cfg(feature = "serde")]
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! use sacm::edit::{self, CreateArgumentPackage, CreateClaim, Mutation, Operation};
//! use sacm::model::{Document, LangString};
//!
//! let mut document = Document::new();
//! let arguments = CreateArgumentPackage::root(LangString::new("Arguments")).with_id("arguments");
//! edit::apply(&mut document, arguments)?;
//!
//! // An operation received as JSON, from a user interface, say.
//! let json = r#"{"type":"CreateClaim","parent":"arguments","id":null,"name":{"lang":"en","content":"The system is safe"}}"#;
//! let operation: Operation = serde_json::from_str(json)?;
//! // Optional: apply runs the same checks; a UI calls check before offering the edit.
//! edit::check(&document, &operation)?;
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
//! let misplaced = Operation::from(CreateClaim::new("nowhere", LangString::new("C1")));
//! let error = edit::check(&document, &misplaced).unwrap_err();
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
//! [adr-0030]: https://github.com/lasrod/libSACM/blob/main/docs/adr/0030-serde-for-edit-and-diagnostic-values.md
//!
//! [sacm]: https://www.omg.org/spec/SACM/2.3
//! [matrix]: https://github.com/lasrod/libSACM/blob/main/docs/matrix/functionality-matrix.md
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
