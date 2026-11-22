//! Read, edit, validate and write assurance cases in the
//! [OMG Structured Assurance Case Metamodel (SACM) 2.3][sacm].
//!
//! It claims the four model compliance points of SACM 2.3 clause 2; the
//! [conformance statement](https://github.com/lasrod/libSACM/blob/main/docs/sacm/compliance-points.md)
//! says what that covers.
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
//! - [`xmi::read`] reads `AssuranceCasePackage`, `ArgumentPackage`,
//!   `ArtifactPackage` and `TerminologyPackage` roots and the packages and
//!   Claims in them, with their names, `gid`s and each
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
//!   SACM-REF-001, and a strict save refuses it, since the output would not
//!   be schema-valid; see
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
//!   In an `ArtifactPackage` it reads `Technique`, `Participant`, `Property`,
//!   `ArtifactGroup`, `Artifact`, `Event`, `Resource` and `Activity`
//!   elements, with each asset's
//!   [`properties`](model::SacmElement::properties), each group's members,
//!   an Artifact's [`version`](model::SacmElement::version), the
//!   [`date`](model::SacmElement::date) of an Artifact or an Event, an
//!   Activity's [`start_time`](model::SacmElement::start_time) and
//!   [`end_time`](model::SacmElement::end_time), each held exactly as
//!   written, and a Resource's [`location`](model::SacmElement::location).
//!   In a `TerminologyPackage` it reads `Category`, `Term`, `Expression`,
//!   `TerminologyGroup` and nested `TerminologyPackage` elements (see
//!   [Terminology](#terminology)). The interfaces and bindings of every
//!   package are read as roots and as children, with each interface's
//!   [`implements`](model::SacmElement::implements), each binding's
//!   [`participant_packages`](model::SacmElement::participant_packages) and
//!   each package's [`interfaces`](model::SacmElement::interfaces). A
//!   reference element whose `href` names an element of another document,
//!   such as `other.sacm.xmi#c1`, is held as a [`model::Href`] in the end's
//!   [`hrefs`](model::SacmElement::hrefs), written back and counted toward
//!   the end's bounds, but never resolved; its fragment may be an
//!   `XPointer`, and a type may stand before it
//!   ([`Href::kind`](model::Href::kind)). A child held in another document,
//!   such as `<argumentPackage href="arguments.sacm.xmi#args"/>`, is held
//!   as a [`model::Proxy`] of its container
//!   ([`proxies`](model::SacmElement::proxies); see
//!   [Cross-file proxies](xmi::read#cross-file-proxies)).
//!   A `name`, or a value of a `content`, `key` or `location`, may be an
//!   `ExpressionLangString` (§8.4) that links an `ExpressionElement`
//!   ([`LangString::expression`](model::LangString::expression),
//!   [`expressions`](model::SacmElement::expressions)), or one of another
//!   document
//!   ([`LangString::expression_href`](model::LangString::expression_href)).
//!   Content that is not SACM 2.3 where it stands, such as a vendor
//!   attribute or element, is kept as written as preserved content, with
//!   SACM-XMI-003 ([`has_preserved_content`](model::SacmElement::has_preserved_content);
//!   see [Preserved content](xmi::read#preserved-content)). A tolerant read,
//!   the default, opens every well-formed file with SACM 2.3 package roots:
//!   what a strict read refuses, such as a value that is no literal or an
//!   untyped child under an abstract role, it keeps as preserved content
//!   with a Warning, never dropped (see [Modes](xmi#modes)). An element
//!   without an `xmi:id` gets a generated one, such as `claim_1`, which
//!   `write` saves; see [Ids](xmi::read#ids). XMI metadata, such as
//!   `xmi:uuid` ([`uuid`](model::SacmElement::uuid)) or an
//!   `xmi:Extension`, is read in both modes and kept as written (see
//!   [XMI metadata](xmi::read#xmi-metadata)), and a root that a same-file
//!   proxy names moves into the proxy's place (see
//!   [Same-file proxies](xmi::read#same-file-proxies)).
//! - [`xmi::write`] writes a document deterministically, XMI metadata and
//!   preserved content included. A strict save, the default, refuses output that would not be
//!   schema-valid, and one that holds preserved content; it never validates,
//!   so call [`validation::validate`] for model checks.
//! - [`validation::validate`] reports each element that has no name, each
//!   `content`, `key` or `location` text that is empty or gives one language
//!   twice, each tagged value without a key, each id that is not an XML
//!   `NCName`, each `gid` two elements share, each element with more than one
//!   description, each `ArgumentPackage` that nests one and also holds other
//!   argumentation elements, each reference that names no element or an
//!   element of the wrong kind, each element with a `citedElement` whose
//!   `isCitation` is not true, each `abstractForm` that breaks a §8.2 rule,
//!   each asserted relationship without a source or a target, each evidence
//!   or artifact relationship whose ends are not artifact references (§11.15,
//!   §11.17, §11.18), each Term or Expression without a `value`, each
//!   `ExpressionLangString` without an `expression` or with both an
//!   `expression` and a non-empty content, each concrete Expression whose
//!   elements are abstract, each id a reference end repeats, each child of a
//!   package interface or binding that its clause does not allow (§9.3,
//!   §10.5, §10.6, §11.5, §11.6, §12.4, §12.5), each interface with no
//!   `implements` and each binding with fewer than two distinct
//!   participants, each binding named as a participant, and each interface
//!   outside the package it implements, as
//!   [`Diagnostic`](diagnostic::Diagnostic)s with stable codes.
//! - [`edit::apply`] creates `AssuranceCasePackage`, `ArgumentPackage`,
//!   `Claim`, `ArgumentReasoning`, `ArtifactReference` and `ArgumentGroup`
//!   elements and the five asserted relationships
//!   ([`edit::CreateAssertedInference`] and the like), `ArtifactPackage`,
//!   `Property`, `Technique`, `Participant`, `ArtifactGroup`, `Artifact`,
//!   `Event`, `Resource` and `Activity` elements
//!   ([`edit::CreateArtifactPackage`] and the like), `TerminologyPackage`,
//!   `Category`, `Term`, `Expression` and `TerminologyGroup` elements
//!   ([`edit::CreateTerminologyPackage`] and the like), the interfaces and
//!   bindings of every package ([`edit::CreateArgumentPackageInterface`] and
//!   the like), and descriptions, implementation constraints, notes and
//!   tagged values on them ([`edit::CreateNote`], [`edit::CreateTaggedValue`]
//!   and the like); a refused edit changes nothing, and an edit refuses a
//!   reference that names no element or an element its end's declared type
//!   does not admit. The stricter §11.15, §11.17 and §11.18 rules, that
//!   evidence and artifact relationships link `ArtifactReference`s, are left
//!   to [`validation::validate`]. It also changes existing elements of every
//!   kind: their names, `gid`s, flags and `assertionDeclaration`s
//!   ([`edit::SetName`] and the like), the `content` and `key` of
//!   descriptions, implementation constraints, notes and tagged values
//!   ([`edit::SetContent`],
//!   [`edit::SetKey`]), an Artifact's `version`, the `date` of an Artifact
//!   or an Event, a Resource's `location` and an Activity's `startTime` and
//!   `endTime` ([`edit::SetVersion`], [`edit::SetDate`],
//!   [`edit::SetLocation`], [`edit::SetStartTime`], [`edit::SetEndTime`]),
//!   a Term's or an Expression's `value` and a Term's `externalReference`
//!   ([`edit::SetValue`], [`edit::SetExternalReference`]),
//!   every reference end, within its bounds
//!   ([`edit::SetReferences`], [`edit::AddReference`],
//!   [`edit::RemoveReference`]), and the order of an element's children or
//!   of the roots ([`edit::ReorderElements`]). [`edit::MoveElement`] moves
//!   an element, with everything it contains, under another parent or to
//!   the roots, and [`edit::CopyElements`] copies elements of another
//!   document, such as a draft, with fresh ids where theirs clash.
//!   [`edit::DeleteElement`]
//!   deletes an element of any kind under explicit policies, and never
//!   leaves a reference dangling. [`edit::check`] answers whether `apply`
//!   would accept an operation, and [`edit::preview`] what it would do,
//!   with the references to deleted elements a delete removes, without changing the
//!   document; [`edit::apply_preview`] applies a preview unless the document
//!   has changed since ([`Document::revision`](model::Document::revision)).
//!   [`edit::batch`] and [`edit::apply_all`] apply several operations as one
//!   atomic change.
//! - [`Document::referrers`](model::Document::referrers) lists the typed
//!   references that name an id, and
//!   [`SacmElement::flag`](model::SacmElement::flag) and
//!   [`SacmElement::references`](model::SacmElement::references), with
//!   [`Flag::ALL`](model::Flag::ALL) and
//!   [`ReferenceEnd::ALL`](model::ReferenceEnd::ALL), read an element's flags
//!   and ends without naming each one.
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
//! A strict save refuses output that would not be schema-valid, an `xmi:id`
//! that is no `NCName` or a reference to no `xmi:id` it writes, as
//! [`WriteError::Invalid`](xmi::WriteError::Invalid), and a document that
//! holds preserved content, as
//! [`WriteError::PreservedContent`](xmi::WriteError::PreservedContent). It
//! never validates: a draft with an open model Error, such as an element with
//! no name, saves strictly. Call [`validation::validate`] for model checks.
//!
//! Which [`xmi::Mode`] to use: read in tolerant mode, the default, to open
//! files from other tools: it keeps what a strict read refuses, with a
//! Warning. A strict read refuses what it does not read, and reports the
//! variations it recognises as Errors instead of Warnings. Neither mode
//! validates what it reads. To check that a file conforms, read it in strict
//! mode, run [`validation::validate`], and test the read with
//! [`ReadOutput::has_errors`](xmi::ReadOutput::has_errors) and the findings
//! with [`diagnostic::has_errors`]: a tolerant read reports what a strict read
//! refuses as Warnings and keeps it as preserved content
//! ([`Document::has_preserved_content`](model::Document::has_preserved_content)),
//! so its diagnostics alone are no conformance check. Save in strict mode, the default, for a file
//! you hand on; save in tolerant mode to keep what a tolerant read
//! preserved.
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
//! (§11.9), which name the artifacts. An `ArtifactReference` names its
//! artifacts, such as an `Artifact` (§12.7), in
//! `referencedArtifactElement`. Build them with [`edit`], and read them back
//! with [`sources`](model::SacmElement::sources) and
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
//! # Artifacts
//!
//! An `ArtifactPackage` (§12.2) holds what the argument's evidence is about.
//! This release reads, writes and creates its `Technique`s (§12.12),
//! `Participant`s (§12.13), `Property`s (§12.8), `ArtifactGroup`s (§12.3),
//! `Artifact`s (§12.7), `Event`s (§12.9), `Resource`s (§12.10),
//! `Activity`s (§12.11) and `ArtifactAssetRelationship`s (§12.14). An
//! artifact asset's characteristics are its
//! [`properties`](model::SacmElement::properties), and a group's members are
//! references, as an `ArgumentGroup`'s are. An `ArtifactAssetRelationship`
//! links assets through its [`sources`](model::SacmElement::sources) and
//! [`targets`](model::SacmElement::targets), one or more each, which a
//! delete handles as it handles an asserted relationship's. The date-typed
//! `date`, `startTime` and `endTime` are held as strings exactly as written:
//! SACM 2.3 defines no lexical form for them (D-DATE). A tolerant read also
//! reads the machine-readable model's spellings `ArtifactAssertedRelationship`
//! and an Event's `occurece`, and the text's `argumentationElement`, each with
//! a SACM-XMI-013 Warning; a strict read keeps them as preserved content with
//! an Error (see [`xmi::read`](xmi::read#spellings)). This example creates a
//! `Technique` with a `Property`, a `Participant` and an `ArtifactGroup` of the
//! two:
//!
//! ```
//! use sacm::edit::{self, CreateArtifactGroup, CreateArtifactPackage, CreateParticipant, CreateProperty, CreateTechnique};
//! use sacm::model::{Document, LangString};
//! use sacm::xmi::{self, WriteOptions};
//!
//! let mut doc = Document::new();
//! let en = |text: &str| LangString::new(text).with_lang("en");
//! edit::apply(&mut doc, CreateArtifactPackage::root(en("Evidence")).with_id("evidence"))?;
//! edit::apply(&mut doc, CreateTechnique::new("evidence", en("Fault tree analysis")).with_id("fta"))?;
//! edit::apply(&mut doc, CreateProperty::new("fta", en("Qualified tool")))?;
//! edit::apply(&mut doc, CreateParticipant::new("evidence", en("Independent assessor")).with_id("assessor"))?;
//! let group = CreateArtifactGroup::new("evidence", en("Analysis")).with_group_members(["fta", "assessor"]);
//! let created = edit::apply(&mut doc, group)?;
//!
//! let analysis = doc.get(&created.created_ids()[0]).expect("just created");
//! assert_eq!(analysis.group_members().len(), 2);
//! assert_eq!(doc.get("fta").expect("created").properties().count(), 1);
//! let xml = xmi::write(&doc, &WriteOptions::default())?;
//! assert!(xml.contains(r#"artifactElement="fta assessor""#));
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # Terminology
//!
//! A `TerminologyPackage` (§10.4) holds the vocabulary of an assurance case:
//! `Category`s (§10.8), `Term`s (§10.11), `Expression`s (§10.10) and
//! `TerminologyGroup`s (§10.3), and nested `TerminologyPackages`. This release
//! reads, writes and creates them; a TerminologyPackage is read as a root
//! (the Terminology unit of interchange, §2.5), in an AssuranceCasePackage,
//! nested, and in an ArtifactPackage. A Term or an Expression has a
//! [`value`](model::SacmElement::value) (`value:String[1]`, held exactly as
//! written; [`validation::validate`] reports one without it) and the
//! [`categories`](model::SacmElement::categories) it belongs to; a Term has
//! an [`origin`](model::SacmElement::origin) and an
//! [`external_reference`](model::SacmElement::external_reference), and an
//! Expression the
//! [`expression_elements`](model::SacmElement::expression_elements) it is
//! formed from. A `TerminologyGroup`'s members are references, as an
//! `ArgumentGroup`'s are. A tolerant read also reads the text's spelling
//! `TerminologyElement` of a TerminologyPackage's role, with a SACM-XMI-013
//! Warning.
//!
//! ```
//! use sacm::edit::{self, CreateExpression, CreateTerm, CreateTerminologyPackage};
//! use sacm::model::{Document, LangString};
//!
//! let en = |text: &str| LangString::new(text).with_lang("en");
//! let mut doc = Document::new();
//! edit::apply(&mut doc, CreateTerminologyPackage::root(en("Blender terms")).with_id("terms"))?;
//! edit::apply(&mut doc, CreateTerm::new("terms", en("Blade"), "blade").with_id("blade"))?;
//! edit::apply(&mut doc, CreateTerm::new("terms", en("Lid"), "lid").with_id("lid"))?;
//! let guarded = CreateExpression::new("terms", en("Blade guarded"), "$Blade$ is covered by the $Lid$")
//!     .with_expression_elements(["blade", "lid"]);
//! edit::apply(&mut doc, guarded.with_id("guarded"))?;
//! let parts: Vec<&str> = doc.get("guarded").expect("created").expression_elements().iter().map(|id| id.as_str()).collect();
//! assert_eq!(parts, ["blade", "lid"]);
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
//! [`diagnostic::Diagnostic`], the read and write refusals
//! [`xmi::ReadError`] and [`xmi::WriteError`], and the values they carry
//! (ids, kinds, names, flags, reference ends, codes, severities, locations
//! and [`xmi::WriteAttribute`]) implement `Serialize` and `Deserialize`. So
//! an edit can cross a process boundary, travel between users or be kept as
//! an audit log. A refusal is
//! `{"type", "code", "template", "arguments", …its fields}`, every key
//! required, and reads back only with a code and a template of its variant
//! and exactly the template's arguments (ADR 0030 amendment 3). A
//! [`Document`](model::Document) does not: XMI is its only serialization.
//!
//! The JSON shape is documented in [ADR 0030][adr-0030] and follows the
//! crate's semver (see ADR 0030): an operation
//! is tagged `"type"` with its operation name, keys are the snake-case
//! accessor names (for an `EditError`, its variant's field names), every
//! key is written, and on input a key may be left out only where a builder
//! sets a default: an operation's `id`, `structure`, `reasoning`,
//! `is_counter`, `referenced_artifact_elements` and `group_members`; a
//! `LangString`'s `lang` and `content`; and a `Diagnostic`'s `clause`,
//! `affected` and `location`, and its `template` and `arguments` together,
//! as 0.1.0 wrote it. A diagnostic writes `template` and `arguments` after
//! `message`. A refusal's or diagnostic's `arguments` is an object written in
//! the template's order; on input it may give its keys in any order, each name
//! once. A package create's `"parent": null` means a root; leaving out
//! `parent` is refused. Unknown keys, names and codes are refused. A
//! deserialized operation proves nothing: [`edit::check`] and
//! [`edit::apply`] check it as they check one built in Rust.
//!
//! Adding a variant, or a key whose default keeps the old meaning, is not a
//! breaking change; a change to a key, tag or meaning is, and ships only in
//! a release that bumps the minor version (0.x). JSON from a newer release
//! may be refused. A diagnostic's `message` text is not part
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
