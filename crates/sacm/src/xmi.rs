//! Read and write SACM 2.3 XMI documents.
//!
//! # Entry points
//!
//! | Function | What it does |
//! |---|---|
//! | [`read`] | Reads the bytes of an XMI document into a [`Document`], with diagnostics of what it found. |
//! | [`write`](fn@write) | Writes a [`Document`] as XMI text. |
//!
//! # Rules
//!
//! - **No I/O.** The caller reads and writes the file.
//! - **What is read and written.** The four package kinds, as roots (bare or inside an
//!   `xmi:XMI` element) or nested, with their elements, interfaces and bindings.
//!   [`read`] lists every role and feature.
//! - **No model content is dropped.** Content that is not SACM 2.3 where it stands, such
//!   as a vendor attribute or child element, is kept as written, with SACM-XMI-003; see
//!   [Preserved content](fn@read#preserved-content). Comments and processing
//!   instructions are not kept.
//! - **A reference is the id it names.** One that names no element is kept; see
//!   [References](fn@read#references).
//! - **String values are held as written,** the date-typed ones included (D-DATE). A flag
//!   or an `assertionDeclaration` is read as its value: `isCitation="1"` is written as
//!   `isCitation="true"`.
//! - **An id is generated** for an element with no `xmi:id`.
//! - **Deterministic output.** `write` writes every element and the preserved content
//!   `read` kept, in document order, in one form.
//! - **Round trip.** A document `read` returned always writes in tolerant mode. What
//!   `write` writes for a document built with [`edit::apply`](crate::edit::apply) reads
//!   back with the same ids, kinds, names, contents, keys, declarations, parents and order.
//!
//! # Modes
//!
//! Both take a [`Mode`] in their options, strict by default (ADR 0022, ADR 0035).
//!
//! - **Strict read.** Refuses what it cannot read conformantly.
//! - **Strict write.** Refuses what is not schema-valid, apart from XMI metadata it
//!   writes as read.
//! - **[`Mode::Tolerant`].** Opens a well-formed SACM file that a strict read refuses
//!   for a defect it can keep. The table lists what a tolerant read still refuses.
//!
//! | | Strict | Tolerant |
//! |---|---|---|
//! | `read` refuses | a DOCTYPE, malformed XML, no SACM 2.3 package root (a root in another SACM namespace included), a repeated or empty `xmi:id`, a value given twice, a value that is no literal, an untyped child under an abstract role, a C++ name or text shape, what the reader cannot represent ([`ReadError::Unsupported`]), and input over its [`Limits`] | a DOCTYPE, malformed XML, no SACM package root (a root outside SACM, a SACM 2.4 root, an `XMI` element in neither XMI 2.5.1's namespace nor `http://www.omg.org/XMI`, or another element of an XMI namespace as the document element), a repeated `xmi:id`, an encoding it cannot decode, input over its [`Limits`], and the parser's capacity |
//! | `read` keeps as preserved content | content that does not conform where it stands, with SACM-XMI-003 as an Error | the same, with SACM-XMI-003 as a Warning; a root in an older SACM revision's or a SACM-like namespace is read as SACM 2.3, with SACM-XMI-008 or SACM-XMI-002; and every other strict refusal, at its construct, with one Warning at the strict refusal's location: SACM-ENUM-001, SACM-MULT-001 (the first occurrence is read), SACM-XMI-005, SACM-ID-002 (an element gets a generated id), and SACM-XMI-003 for content on or in `xmi:XMI` |
//! | `write` refuses | no root; preserved content ([`WriteError::PreservedContent`]); output that would not be schema-valid: an `xmi:id` that is no `NCName` or a reference to no `xmi:id` it writes ([`WriteError::Invalid`]); a character XML 1.0 cannot represent | no root; a character XML 1.0 cannot represent |
//!
//! - **A strict write checks schema validity only.** SACM 2.3 clause 2 asks for XMI
//!   "that conform with the SACM XML Schema", whose schemas ignore multiplicities
//!   (XMI 2.5.1 §7.8.2).
//! - **It never validates.** It does not run
//!   [`validation::validate`](crate::validation::validate), so a draft, such as a
//!   package with no name, saves strictly. An application calls `validate` for model
//!   checks.
//! - **A tolerant read never reports an Error.** An application shows its Warnings, and
//!   saving in tolerant mode keeps what it preserved.
//!
//! ```
//! use sacm::diagnostic::{Code, Severity};
//! use sacm::edit::{self, SetAssertionDeclaration};
//! use sacm::model::AssertionDeclaration;
//! use sacm::{validation, xmi::{self, Mode, ReadOptions, WriteOptions}};
//!
//! let input = br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
//!     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="arguments">
//!   <argumentElement xmi:type="sacm:Claim" xmi:id="c1" assertionDeclaration="Assumed"/>
//! </sacm:ArgumentPackage>"#;
//! let output = xmi::read(input, &ReadOptions::default().with_mode(Mode::Tolerant))?; // tolerant: it opens
//! let found = &output.diagnostics()[0];
//! assert_eq!((found.code(), found.severity()), (Code::ENUM_INVALID_LITERAL, Severity::Warning));
//! let mut document = output.into_document();
//! edit::apply(&mut document, SetAssertionDeclaration::new("c1", AssertionDeclaration::Assumed))?;
//! assert!(!document.has_preserved_content());                       // the edit repaired it
//! let text = xmi::write(&document, &WriteOptions::default())?;      // a nameless draft saves strictly
//! let findings = validation::validate(&document);                   // model checks: the app's call
//! # assert!(text.contains("assertionDeclaration=\"assumed\""));
//! # assert!(sacm::diagnostic::has_errors(&findings));
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # Examples
//!
//! Reading a document, writing it, and reading the output back:
//!
//! ```
//! use sacm::model::ElementKind;
//! use sacm::xmi::{self, ReadOptions, WriteOptions};
//!
//! let input = br#"<?xml version="1.0" encoding="UTF-8"?>
//! <xmi:XMI xmlns:xmi="http://www.omg.org/spec/XMI/20131001"
//!          xmlns:sacm="http://www.omg.org/spec/SACM/20220301">
//!   <sacm:AssuranceCasePackage xmi:id="case">
//!     <name lang="en" content="Safety case"/>
//!     <argumentPackage xmi:id="arguments">
//!       <name lang="en" content="Arguments"/>
//!       <argumentElement xmi:type="sacm:Claim" xmi:id="c1">
//!         <name lang="en" content="The system is safe"/>
//!       </argumentElement>
//!     </argumentPackage>
//!   </sacm:AssuranceCasePackage>
//! </xmi:XMI>"#;
//! let document = xmi::read(input, &ReadOptions::default())?.into_document();
//! let c1 = document.get("c1").expect("the Claim was read");
//! assert_eq!(c1.kind(), ElementKind::Claim);
//! assert_eq!(c1.parent().map(|parent| parent.id().as_str()), Some("arguments"));
//!
//! let text = xmi::write(&document, &WriteOptions::default())?;
//! let back = xmi::read(text.as_bytes(), &ReadOptions::default())?.into_document();
//! let ids: Vec<_> = back.elements().map(|element| element.id().as_str()).collect();
//! assert_eq!(ids, ["case", "arguments", "c1"]);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! Reading and writing files is the caller's I/O. This function shows the
//! shape; the example defines it but does not call it:
//!
//! ```
//! use std::path::Path;
//!
//! use sacm::xmi::{self, ReadOptions, WriteOptions};
//!
//! /// Reads the XMI file at `from` and writes it again to `to`.
//! fn rewrite(from: &Path, to: &Path) -> Result<(), Box<dyn std::error::Error>> {
//!     let document = xmi::read(&std::fs::read(from)?, &ReadOptions::default())?.into_document();
//!     let text = xmi::write(&document, &WriteOptions::default())?;
//!     std::fs::write(to, text)?;
//!     Ok(())
//! }
//! # let _ = rewrite;
//! ```

use std::collections::BTreeMap;
use std::fmt;

use crate::diagnostic::template;
use crate::diagnostic::{Argument, Code, Details, Diagnostic, SourceLocation};
use crate::model::{Document, ElementId, ElementKind, ReferenceEnd};

mod input;
mod limits;
mod namespaces;
mod output;
mod preserve;
mod refusal;
mod roots;

pub use limits::{Limit, Limits};
// The rule the edits share with the writer (LIB-EDIT-025).
pub(crate) use output::{first_unrepresentable, name_texts, string_attribute, value_texts};

/// Reads an XMI document from its bytes.
///
/// In this release `read` reads `AssuranceCasePackage`, `ArgumentPackage`,
/// `ArtifactPackage` and `TerminologyPackage` roots (formal/23-05-08 §2.2,
/// §2.4, §2.5): the document element, or each element
/// child outside the XMI namespace of an `xmi:XMI` element in the XMI 2.5.1
/// namespace `http://www.omg.org/spec/XMI/20131001`. Each root becomes
/// a parentless element of its kind, in document order, whose id is its
/// `xmi:id` (see [Ids](#ids)), and whose name is its `name` child: `lang` and `content` are the
/// attribute values exactly as XML 1.0 gives them, never trimmed (a literal
/// TAB, LF or CR in a value is already a space by XML 1.0 §3.3.3; write
/// `&#9;`, `&#10;` or `&#13;` to keep one). A field whose attribute is not
/// written is absent. A root without a `name` has no name.
///
/// Under each root, `read` reads these children, in document order, each
/// with its id (see [Ids](#ids)) and its name. A child's element name is its role, in no
/// namespace:
///
/// | Parent | Role element | Reads | Needs `xmi:type` |
/// |---|---|---|---|
/// | AssuranceCasePackage (§9.2) | `assuranceCasePackage` | an AssuranceCasePackage | no |
/// | AssuranceCasePackage (§9.2) | `argumentPackage` | an ArgumentPackage | no |
/// | AssuranceCasePackage (§9.2) | `artifactPackage` | an ArtifactPackage | no |
/// | AssuranceCasePackage (§9.2) | `terminologyPackage` | a TerminologyPackage | no |
/// | TerminologyPackage (§10.4) | `terminologyElement` | a TerminologyPackage, a `Category`, a `Term`, an `Expression` or a `TerminologyGroup` | yes: the role's type, `TerminologyElement`, is abstract (§10.2) |
/// | ArgumentPackage (§11.4) | `argumentElement` | an ArgumentPackage, a Claim, an `ArgumentReasoning`, an `ArtifactReference`, an `ArgumentGroup`, an `AssertedInference`, an `AssertedEvidence`, an `AssertedContext`, an `AssertedArtifactSupport` or an `AssertedArtifactContext` | yes: the role's type, `ArgumentationElement`, is abstract (§11.3) |
/// | ArtifactPackage (§12.2) | `artifactElement` | an AssuranceCasePackage, an ArgumentPackage, an ArtifactPackage, any of the nine argumentation kinds above, a `Property`, a `Technique`, a `Participant`, an `ArtifactGroup`, an `Artifact`, an `Event`, a `Resource`, an `Activity`, an `ArtifactAssetRelationship`, or any of the five Terminology kinds above | yes: the role's type, `ArtifactElement`, is abstract (§8.10) |
/// | `Property`, `Technique`, `Participant`, `Artifact`, `Event`, `Resource` or `Activity` (§12.6) | `property` | a `Property` | no |
/// | each of the above (§8.6) | `description` | a `Description` | no |
/// | each of the above (§8.6) | `implementationConstraint` | an `ImplementationConstraint` | no |
/// | each of the above (§8.6) | `note` | a `Note` | no |
/// | each of the above (§8.6) | `taggedValue` | a `TaggedValue` | no |
///
/// The class is the one the child's `xmi:type` or `xsi:type` names (both, if
/// present, must name the same class); without one, it is the role's declared
/// type. Under a role whose declared type is abstract, such as
/// `argumentElement`, a child without one is [`ReadError::MissingType`]
/// (SACM-XMI-005) in strict mode: its element name is the role, so no class
/// can be inferred (XMI 2.5.1 §7.6.3), and none is guessed. A tolerant read
/// keeps it whole as preserved content of its parent, with SACM-XMI-005 as a
/// Warning (see [Modes](self#modes)). §11.4's text spells the
/// ArgumentPackage role `argumentationElement`; a tolerant read reads that
/// spelling as `argumentElement`, with one SACM-XMI-013 Warning, and a strict
/// read keeps the child whole as preserved content of the package, with
/// SACM-XMI-013 as an Error (see [Spellings](#spellings)). §10.4's text
/// spells the TerminologyPackage role `TerminologyElement`, read the same
/// way (SACM23-XMI-028).
///
/// The four `UtilityElement`s are children in the same document order as the
/// rest ([`SacmElement::descriptions`](crate::model::SacmElement::descriptions)
/// and the like filter it), and several of a role are kept. A `UtilityElement`
/// has no name and holds no element: its `content` (§8.7) and, on a
/// `TaggedValue`, its `key` (§8.12), in either order, are each one role element
/// holding a `MultiLangString` (ADR 0025): `<content>` with one `<value>` per
/// [`LangString`](crate::model::LangString), in order and with duplicates.
/// Each `value` is read as a `name` is, below. No `content` is `None`;
/// `<content/>`, or one holding only white space, comments and processing
/// instructions, is an empty `MultiLangString`. A `content` or `key` may carry
/// only `xmi:id` and an `xmi:type` or `xsi:type` naming
/// `sacm:MultiLangString`, besides the C++ shorthand (see
/// [The C++ name and text shapes](#the-c-name-and-text-shapes)).
///
/// Every element may carry a `gid` (§8.2), in no namespace, as an attribute or
/// a value element (below). It is a String, read exactly as XML gives it: an
/// absent gid, `""` and any other value are kept apart, and nothing is
/// trimmed. It is not an id: it is in no id space and has no syntax check.
///
/// Every element may also carry the flags `isCitation` and `isAbstract`
/// (§8.2 `[0..1]=false`), in no namespace, as an attribute or a value
/// element. Each is an `xsd:boolean`: its white space is collapsed, `true`
/// and `1` read as true, and `false` and `0` as false. Without one, the flag
/// is false, so `false` and absent read alike. Any other value, such as
/// `TRUE`, `yes` or the empty value, is [`ReadError::InvalidLiteral`]
/// (SACM-ENUM-001), holding the value as read, in strict mode: it is never
/// read as false. A tolerant read keeps it as preserved content, with
/// SACM-ENUM-001 as a Warning, and the flag holds false. And it may carry the references `citedElement` and
/// `abstractForm` (§8.2 `[0..1]`), described under
/// [References](#references). The flag and its reference are independent,
/// and nothing repairs a file on read; the §8.2 rules between them are
/// [`validation::validate`](crate::validation::validate)'s.
///
/// The argumentation kinds and the `ArtifactGroup` have more (ADR 0027,
/// ADR 0028, ADR 0034). Every Assertion
/// (a Claim or one of the five asserted relationships) may carry `metaClaim`
/// (§11.10 `[0..*]`). An asserted relationship (§11.13) may carry the flag
/// `isCounter` (`[1] = false`), read as `isCitation` is, and the references
/// `reasoning` (`[0..1]`), `source` (`[1..*]`) and `target` (`[1]`). An
/// `ArgumentReasoning` may carry `structure` (§11.12 `[0..1]`), an
/// `ArtifactReference` `referencedArtifactElement` (§11.9 `[0..*]`), an
/// `ArgumentGroup` its members, `argumentElement` (§11.2 `[0..*]`), and an
/// `ArtifactGroup` its members, `artifactElement` (§12.3 `[0..*]`). Every
/// `TerminologyAsset` (a Category, a Term or an Expression) may carry
/// `category` (§10.8, §10.9 `[0..*]`), an Expression `element` (§10.10
/// `[0..*]`), a Term `origin` (§10.11 `[0..1]`), and a `TerminologyGroup`
/// its members, `terminologyElement` (§10.3 `[0..*]`). A Term or an
/// Expression may carry `value` (§10.9 `String[1]`) and a Term
/// `externalReference` (§10.11 `String[0..1]`), in no namespace, as an
/// attribute or a value element, each held exactly as written; one read
/// without a `value` is kept, and
/// [`validation::validate`](crate::validation::validate) reports it. An
/// `ArtifactReference` is no Assertion, so it has no `assertionDeclaration`
/// or `metaClaim`. A relationship read without a `source` or a `target` is
/// kept as read, and nothing is added;
/// [`validation::validate`](crate::validation::validate) reports it
/// (SACM23-VAL-005). On a kind that lacks one of these, such as `isCounter`
/// on a Claim or `metaClaim` on an `ArgumentReasoning`, it is preserved (see
/// [Preserved content](#preserved-content)).
///
/// A group's member end is a reference, though it shares its name with a
/// package's composite role: `argumentElement` with an ArgumentPackage's,
/// `artifactElement` with an ArtifactPackage's, `terminologyElement` with a
/// TerminologyPackage's. The parent's kind decides:
/// under a group, such an element is a reference element, never read as a
/// child. One with `xmi:type`, `xsi:type` or `xmi:id`, such as
/// `<argumentElement xmi:type="sacm:Claim" xmi:id="c9"/>`, is
/// [`ReadError::Unsupported`] at the first of these attributes: they are
/// conforming XMI on a reference element that this release does not read
/// yet (SACM23-XMI-019; see [References](#references)). One with none of
/// them and no link, holding only a `name`, is a reference element that
/// does not conform, and the group preserves it whole.
///
/// An Assertion's `assertionDeclaration` attribute (§11.10), in no namespace, is
/// read as its declaration. Its value must be one of the six
/// [`AssertionDeclaration`](crate::model::AssertionDeclaration) literals:
/// `asserted`, `needsSupport`, `assumed`, `axiomatic`, `defeated` or
/// `asCited`. Matching is exact: case-sensitive, with no white space trimmed.
/// A literal tab or line break in the value is already a space (XML 1.0
/// §3.3.3), so it does not match either. Without the attribute, or with
/// `"asserted"`, the Assertion is `asserted`. Any other value is
/// [`ReadError::InvalidLiteral`] (SACM-ENUM-001) at the attribute's name,
/// holding the value as read, in strict mode. It is never read as `asserted`
/// silently, because that would change what the case declares: a tolerant
/// read keeps the attribute as preserved content, with SACM-ENUM-001 as a
/// Warning, and the Assertion holds `asserted` until an edit sets it
/// ([`SetAssertionDeclaration`](crate::edit::SetAssertionDeclaration)
/// replaces the kept attribute). A value that is no literal counts as the
/// property's first occurrence. Fix: correct the spelling.
///
/// Each of these values may also be written as a value element (XMI 2.5.1
/// §7.8.4): `<lang>` and `<content>` in the `name` or `value`, `<gid>`,
/// `<isCitation>` and `<isAbstract>` in any element,
/// `<assertionDeclaration>` in an Assertion, and `<isCounter>` in an asserted
/// relationship, in no namespace,
/// anywhere among their siblings. Both forms read to the same document, and [`write()`]
/// writes the attribute form. The value is the element's character content,
/// joined in document order: CDATA sections and references give their
/// characters, comments and processing instructions are skipped, and an empty
/// element is the empty string. Element text keeps its white space exactly:
/// nothing is trimmed, and only a line end (CRLF or a lone CR) reads as LF
/// (XML 1.0 §2.11). An `assertionDeclaration` element must hold one literal
/// exactly, as the attribute must; any other value is
/// [`ReadError::InvalidLiteral`] at its start tag, holding the value. A value
/// given more than once, as an attribute and an element or as two elements,
/// is [`ReadError::RepeatedValue`] (SACM-MULT-001) in strict mode, even when
/// the values are equal, and so is a second `name`, `content`, `key` or `location`. A
/// tolerant read reads the first occurrence and keeps each later one as
/// preserved content, with SACM-MULT-001 as a Warning at it.
///
/// A value element may carry `xsi:nil` (XMI 2.5.1 §9.4.1), in the XML Schema
/// instance namespace `http://www.w3.org/2001/XMLSchema-instance`, by any
/// prefix. It is an `xsd:boolean`: `true` or `1`, white space around them
/// collapsed, reads exactly as if the property were not written (an absent
/// `gid`, `lang` or `content`, a false flag, an `asserted` declaration), in
/// both modes and with no diagnostic, and `false` or `0` reads the content as
/// usual. Any other value, such as `TRUE`, `yes` or `""`, is
/// [`ReadError::InvalidLiteral`] (SACM-ENUM-001) with the property
/// `xsi:nil`. A nil element holds no characters, not even white space, and
/// no element: either does not conform, and is preserved. A nil element
/// still counts as an occurrence for `RepeatedValue`. `xsi:nil` on any other
/// element, and an unqualified `nil`, is an attribute its class lacks, and
/// is preserved.
/// [`write()`] writes the property as omitted (SACM23-XMI-026).
///
/// Elements and
/// attributes are matched by namespace URI and local name, and an `xmi:type`
/// or `xsi:type` value is resolved as a `QName`, without its leading and
/// trailing white space, against the namespaces in scope, so the choice of
/// prefix does not matter. The SACM namespace is
/// `http://www.omg.org/spec/SACM/20220301`.
///
/// `read` never returns a document that is missing content from its input.
/// A root may carry namespace declarations, `xmi:id`, `gid`, `isCitation`,
/// `isAbstract`, `citedElement`, `abstractForm`, `xmi:version` and an
/// `xmi:type` or `xsi:type` naming its own class, and hold at most one
/// `name`, at most one value or reference element of each of `gid`,
/// `isCitation`, `isAbstract`, `citedElement` and `abstractForm`, white
/// space, comments, processing instructions and the children above. A
/// child may carry namespace declarations, `xmi:id`, those five, its type
/// attributes and the features of its kind above (`assertionDeclaration`
/// and `metaClaim` on an Assertion, `isCounter`, `reasoning`, `source` and
/// `target` on an asserted relationship, `structure` on an
/// `ArgumentReasoning`, `referencedArtifactElement` on an
/// `ArtifactReference`, `argumentElement` on an `ArgumentGroup`,
/// `artifactElement` on an `ArtifactGroup`), and hold
/// the same content as a root, with a value or reference element for each
/// of its kind's features, except that a `UtilityElement` holds, besides
/// those five, only its `content` and `key`.
/// An `xmi:version` on a root or on the `xmi:XMI` wrapper is accepted with
/// any value and ignored: the XMI namespace identifies the version
/// (XMI 2.5.1 §7.5.3), so [`write()`] does not write it back. It is reported
/// as SACM-XMI-011 (see [Diagnostics](#diagnostics)).
/// The `name`, and each `value`, must be in no namespace, carry only
/// `xmi:id`, `lang`, `content`, an `xmi:type` or `xsi:type` naming
/// `sacm:LangString` or `sacm:ExpressionLangString` and, on an
/// `ExpressionLangString` (§8.4), its `expression` as one IDREF, and hold
/// only the value elements `lang` and `content`, an `ExpressionLangString`'s
/// one `expression` reference element, white space, comments and processing instructions. A value element may
/// carry only namespace declarations and `xsi:nil`, and may hold no element.
/// Each part may also carry and hold XMI metadata (below). What else an
/// element carries or holds does not conform, and is kept as preserved
/// content (below), except the C++ shapes below.
///
/// # Spellings
///
/// Where formal/23-05-08 and ptc/22-03-13 spell a name differently and the
/// crate follows the other (D-TEXT-MODEL), a tolerant read reads four
/// spellings as the resolved name, each with one SACM-XMI-013 Warning
/// ([`Code::XMI_SPELLING_VARIANT`]) at its start tag or attribute name,
/// affecting the element the spelling belongs to (SACM23-XMI-009, ADR 0034
/// amendment 2; SACM23-XMI-028, amendment 3):
/// - the type `ArtifactAssertedRelationship`, in `xmi:type` or `xsi:type`,
///   under a role that admits an `ArtifactAssetRelationship` (§12.14), read
///   as that class;
/// - an Event's `occurece`, as an attribute or a value element, read as its
///   `date` (§12.9);
/// - `argumentationElement` as an ArgumentPackage's role (§11.4) or an
///   `ArgumentGroup`'s end (§11.2), read as `argumentElement`;
/// - `TerminologyElement` as a TerminologyPackage's role (§10.4, spec defect
///   D9), read as `terminologyElement`. As a `TerminologyGroup`'s end it is
///   no spelling: §10.3 spells that end `terminologyElement`.
///
/// Each is read under every rule of the resolved name, except that a value
/// given under both names is read from the resolved name, and the spelled
/// one is a repeat (SACM-MULT-001), kept as preserved content. A strict read
/// never maps a spelling: it keeps the construct as preserved content of the
/// element it sits in, with one SACM-XMI-013 Error, so a strict
/// [`write`](fn@write) refuses the document. Elsewhere a spelling is ordinary
/// content (SACM-XMI-003), and the C++ library's `occurence` is not read.
/// [`write`](fn@write) writes the resolved names.
///
/// # Preserved content
///
/// Content that neither the SACM XML Schema nor the XMI 2.5.1 production
/// rules allow where it stands, and that no tolerance reads, is kept
/// exactly as written, in both modes, as preserved content of the element
/// it sits in, its holder (LIB-COMPAT-003, ADR 0033). That is:
///
/// - an attribute the element's class lacks, such as a vendor attribute,
///   `isCounter` on a Claim, an `id` in no namespace, `xmi:version` on a
///   child, an XMI attribute XMI.xsd does not define, or a reference
///   attribute that holds no id, such as `citedElement=""`;
/// - a child element in another namespace, or under a role the class
///   lacks, and any child of a `UtilityElement`;
/// - a child whose type names no concrete SACM class, is not admitted by
///   its role, or disagrees with its other type attribute;
/// - a `name`, `content`, `key`, value element or reference element that
///   does not conform inside, kept whole, so the property is not given;
/// - text that is not all white space in element-only content;
/// - a root's type attribute naming another class (the root is read as the
///   kind its element name gives). An edit never makes such a root a child
///   ([`EditError::KeptRootType`](crate::edit::EditError::KeptRootType)).
///
/// Each construct gives one SACM-XMI-003 ([`Code::XMI_UNKNOWN_CONTENT`]),
/// affecting the holder, at the attribute's name, the element's start tag
/// or the text's first character that is not literal white space: a
/// Warning in tolerant mode and an Error in strict mode. Nothing inside a
/// preserved element is read or reported, but each `xmi:id` in it is
/// checked: a repeated one is [`ReadError::DuplicateId`], and an empty one
/// [`ReadError::InvalidId`] in strict mode; a tolerant read keeps an empty
/// one as written, outside the id space, with SACM-ID-002 as a Warning. A
/// preserved part is no occurrence of a property, so it never makes a
/// [`ReadError::RepeatedValue`], unless a tolerant read kept it for a value
/// that is no literal.
///
/// [`SacmElement::has_preserved_content`](crate::model::SacmElement::has_preserved_content)
/// tells a holder. The content keeps its place: a child its index among the
/// holder's children, and an attribute its place after the typed ones, in
/// input order. Namespace URIs, prefixes, local names, attribute values and
/// the text inside an element are kept exactly, with one exception. A kept
/// type attribute's value is a `QName` (XMI 2.5.1 §9.5.2 rule 2g), so the
/// holder also keeps the binding of the prefix it names, as XMI metadata
/// does for its type. A value prefix `sacm` or `xmi` bound to another
/// namespace, or no prefix under a default namespace, is replaced by the
/// holder's `nsN` for that namespace (LIB-COMPAT-019). Text in a holder is
/// trimmed of XML white space, and comments and processing instructions
/// are dropped, as everywhere in the reader. A preserved id stays in the
/// document's id space, though [`Document::get`] does not find it, and a
/// reference to one is SACM-REF-003 in
/// [`validation::validate`](crate::validation::validate). A tolerant
/// [`write()`] writes the content back; a strict one refuses it.
///
/// Two inputs that do not conform a strict read refuses as
/// [`ReadError::Unsupported`]: an attribute or text on the `xmi:XMI`
/// element, which is no element of the document, and an attribute that
/// would be preserved but whose prefix `sacm` or `xmi` is bound to another
/// namespace than SACM 2.3 or XMI 2.5.1, which the written element binds
/// them to. A tolerant read keeps the first as the document's own preserved
/// content, outside every element
/// ([`Document::has_preserved_content`]), with SACM-XMI-003 affecting no
/// element, written back on and in `xmi:XMI` in place among the roots; and
/// the second under the first prefix `nsN` not bound at its element, which
/// the SACM-XMI-003 message names (ADR 0035). An XMI-namespace element
/// under `xmi:XMI` that XMI 2.5.1 does not define there, such as
/// `xmi:foo`, is a defect of the same kind: refused by a strict read, and
/// kept by a tolerant one as the document's own content with SACM-XMI-003.
///
/// ```
/// use sacm::diagnostic::{Code, Severity};
/// use sacm::xmi::{self, Mode, ReadOptions};
///
/// let input = br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmlns:v="urn:vendor" xmi:id="p">
///   <name content="P"/>
///   <v:layout x="10" y="20"/>
/// </sacm:ArgumentPackage>"#;
/// let output = xmi::read(input, &ReadOptions::default().with_mode(Mode::Tolerant))?;
/// let found = &output.diagnostics()[0];
/// assert_eq!((found.code(), found.severity()), (Code::XMI_UNKNOWN_CONTENT, Severity::Warning));
/// assert_eq!(found.location().map(|at| (at.line(), at.column())), Some((4, 3)));
/// assert!(output.document().get("p").is_some_and(|p| p.has_preserved_content()));
/// # Ok::<(), xmi::ReadError>(())
/// ```
///
/// # XMI metadata
///
/// XMI metadata is read in both modes with no diagnostic, held exactly as
/// written, never as preserved content, and written back by both saves
/// where it stood (SACM23-XMI-019, ADR 0042):
///
/// - on `xmi:XMI`: `xmi:id`, `xmi:label`, `xmi:uuid`, `href`, `xmi:idref`,
///   `xmi:type`, `xsi:schemaLocation` and `xsi:noNamespaceSchemaLocation`;
/// - under `xmi:XMI`, among the roots: `documentation`, `Documentation`,
///   `difference`, `Difference`, `Add`, `Replace`, `Delete`, `extension` and
///   `Extension`, each in the XMI namespace and kept whole;
/// - on a root, a child, a `name`, a `content`, `key` or `location`, or a
///   `value`: `xmi:uuid`, `xmi:label`, `xsi:schemaLocation` and
///   `xsi:noNamespaceSchemaLocation`;
/// - in any of these: `xmi:extension` and `xmi:Extension` elements, kept
///   whole, their content not checked. Comments and processing instructions
///   inside one are dropped, as everywhere in the reader.
///
/// [`SacmElement::uuid`](crate::model::SacmElement::uuid) and
/// [`SacmElement::label`](crate::model::SacmElement::label) give an
/// element's own values. Metadata ids share the document's id space, as
/// preserved ids do; nothing inside a Difference's `target` is an id
/// (XMI.xsd: `processContents="skip"`). A copy drops the `xmi:uuid` of each
/// element and part
/// it copies (XMI 2.5.1 §7.6.1: elements with one `xmi:uuid` "are all
/// deemed to reference the same MOF element"), a delete removes the metadata with the
/// element, and a move keeps it. Differences are kept, never applied.
///
/// Any other XMI-namespace element where metadata may stand, an `href` or
/// an `xmi:idref` on a definition (production 2a), and an identity
/// attribute or a type on a reference element (production 2c) do not
/// conform: SACM-XMI-003, kept as preserved content. An XMI-namespace
/// document element other than `xmi:XMI` is [`ReadError::InvalidRoot`].
///
/// ```
/// use sacm::xmi::{self, Mode, ReadOptions, WriteOptions};
///
/// let input = br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="p" xmi:uuid="urn:uuid:p">
///   <name content="P"/>
///   <xmi:Extension extender="tool"><layout x="1"/></xmi:Extension>
/// </sacm:ArgumentPackage>"#;
/// let output = xmi::read(input, &ReadOptions::default().with_mode(Mode::Strict))?;
/// assert!(output.diagnostics().is_empty());
/// let document = output.into_document();
/// assert_eq!(document.get("p").and_then(|p| p.uuid()), Some("urn:uuid:p"));
/// // A strict save writes the metadata back.
/// let text = xmi::write(&document, &WriteOptions::default())?;
/// assert!(text.contains(r#"<xmi:Extension extender="tool"><layout x="1" /></xmi:Extension>"#));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Same-file proxies
///
/// A role element of a composite that carries `xmi:idref="id"` or
/// `href="#id"`, at most one type attribute and nothing else, is a proxy of
/// an element of the same document (XMI 2.5.1 §7.10.1; SACM23-XMI-030).
/// Once every root is read, a proxy that names a root of a kind its role
/// and its type admit moves that root into its place, in both modes, with
/// no diagnostic: the root becomes a child of the proxy's container and
/// stops being a root, and chains of proxies are followed. A proxy that
/// names no root, names one of the wrong kind, names a root that keeps an
/// `href` or an `xmi:idref` of its own (which is not moved), names a root
/// another proxy names too, or is part of a cycle, in that order of checks,
/// is kept whole as preserved content of its container instead, with one
/// SACM-XMI-003 at its `<`: an Error in strict mode, a Warning in tolerant
/// mode. A proxy with anything else on or in it is kept with SACM-XMI-003
/// too. A write writes a moved root as a child. A diagnostic about a proxy
/// that is kept, its `xsi:type`'s SACM-XMI-010 or its spelling's
/// SACM-XMI-013, affects its container, never the id it names.
///
/// # Cross-file proxies
///
/// A role element of a composite in reference form whose `href` names an
/// element of another document, such as
/// `<argumentPackage href="arguments.sacm.xmi#args"/>`, or an `XPointer`, is
/// a proxy of a child held in that document (XMI 2.5.1 §7.10.1, §7.10.2;
/// SACM23-XMI-029, ADR 0040 amendment 1). It is read in both modes with no
/// diagnostic and held, in document order, as a
/// [`Proxy`](crate::model::Proxy) of its container
/// ([`SacmElement::proxies`](crate::model::SacmElement::proxies)): its role
/// and its [`Href`](crate::model::Href). It is never resolved, and no file
/// is opened. An `XPointer` is never evaluated, so
/// `href="#xpointer(id('args'))"` is held as a proxy even when it names a
/// root of this document, which then stays a root. Its type, given by
/// `xmi:type` (or `xsi:type`, with its SACM-XMI-010) or before the URI
/// reference (rule 2n), is held as the href's type; the role must admit it, and the two must agree if both are
/// given. A write writes each proxy as `<role href="…" />`, after the
/// container's children. A role element with a link attribute and anything
/// else, such as an `xmi:id`, an `xmi:uuid`, another attribute or content,
/// or with both `href` and `xmi:idref`, is neither a definition nor a
/// proxy: it is kept whole with SACM-XMI-003.
///
/// ```
/// use sacm::xmi::{self, ReadOptions, WriteOptions};
///
/// let input = br#"<sacm:AssuranceCasePackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="case">
///   <name content="Case"/>
///   <argumentPackage xmi:type="sacm:ArgumentPackageInterface" href="braking.sacm.xmi#api"/>
/// </sacm:AssuranceCasePackage>"#;
/// let output = xmi::read(input, &ReadOptions::default())?;
/// assert!(output.diagnostics().is_empty());
/// let document = output.into_document();
/// let case = document.get("case").ok_or("not read")?;
/// assert_eq!(case.proxies()[0].href().document(), "braking.sacm.xmi");
/// // A strict save writes the proxy back, its type before the URI reference.
/// let text = xmi::write(&document, &WriteOptions::default())?;
/// assert!(text.contains(
///     r#"<argumentPackage href="sacm:ArgumentPackageInterface braking.sacm.xmi#api" />"#
/// ));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// Depth is counted as resolved: a root that stays keeps its input depth, a
/// moved root takes its proxy's depth, and every element of its tree, its
/// parts included, shifts with it. An element nested deeper than
/// [`Limits::max_depth`] as resolved refuses the read in both modes, as
/// [`ReadError::LimitExceeded`] for [`Limit::Depth`] (SACM-SEC-003), at the
/// first such element. So a chain of proxies between flat roots cannot nest
/// a document deeper than its limit, and its output reads back under the
/// same limits.
///
/// # The C++ name and text shapes
///
/// The assurance-forge C++ library writes some names and texts in shapes
/// that XMI 2.5.1 does not define, each of which maps one to one onto the
/// standard form (LIB-COMPAT-008, ADR 0022 amendment 1):
///
/// - a `name="…"` attribute on a root or child that is no `UtilityElement`:
///   its name, with the value as `content` and no `lang`;
/// - text in a `name` or a `value`: its `content`;
/// - a `name` holding one `value`, and nothing else, or one `content`
///   carrying a `lang` attribute: the name's `lang` and `content`;
/// - on a `content` or `key` with no `value`, a `content` attribute or text,
///   with an optional `lang` attribute: a `MultiLangString` of one value,
///   keeping the role element's `xmi:id`;
/// - `content="…"`, or text, with an optional `lang="…"`, on a
///   `UtilityElement`: its `content`, of one value;
/// - `key="…"` and `value="…"` on a `TaggedValue`: its `key`, of one value
///   with no `lang`, and its `content`, of one value; an optional `lang="…"`
///   applies to `value="…"`, as to `content="…"` above.
///
/// A tolerant read reads each as its standard form and reports one
/// SACM-XMI-012 Warning per occurrence ([`Code::XMI_LEGACY_SHAPE`]). Values
/// are kept exactly: absent is not `""`, nothing is trimmed, and text is all
/// of the element's character content, joined, with CDATA sections and
/// references giving their characters; text that is all white space is no
/// shape, and neither is a `lang` attribute alone. A field given both in a
/// shape and in the standard form, or twice, is [`ReadError::RepeatedValue`]
/// in strict mode, and kept by a tolerant read as any repeat is:
/// `content="a"` and text in one `name`, say. A strict read
/// never applies a tolerance: once the rest of the input reads without a
/// refusal, it refuses the first shape as [`ReadError::LegacyShape`]. Both
/// modes of [`write()`] write the standard form. Shapes that would invent or
/// merge elements do not conform, and are preserved whole, such as a `name`
/// holding two values, a `value` beside other fields in a `name`, or text
/// beside the `value`s of a `content` (LIB-COMPAT-012 is to read them). An
/// `xmi:id` on the `value` of a `name` does not conform either: both modes
/// keep the whole `name` as preserved content, with SACM-XMI-003 at its
/// start tag (SACM23-XMI-019).
///
/// The input is UTF-8, ASCII, or UTF-16 with a byte order mark, which reads
/// exactly as the same characters in UTF-8. A DOCTYPE is refused before
/// anything in it is read, so no entity is declared, resolved or expanded.
///
/// # Ids
///
/// An element's id is the value of its `xmi:id`, an `xsd:ID` (XMI 2.5.1
/// §7.6.1), and only `xmi:id` in the XMI namespace is one: an `id` attribute
/// in no namespace or another namespace, such as `id` or `o:id`, is an
/// attribute the class lacks, and is preserved. The ids of the elements, and
/// of the names,
/// `content`s, `key`s and `value`s, of a file share one id space (ADR 0024,
/// ADR 0025):
///
/// - An `xmi:id` with XML white space (space, TAB, LF, CR) around an
///   `NCName`, such as `" c1 "`, is read as the trimmed value, `c1`, since an
///   `xsd:ID`'s white space is collapsed (XMI 2.5.1 §7.6.1; XML Schema 1.0
///   Part 2 §4.3.6). Any other id is read as written.
/// - An element without an `xmi:id`, a root or a child, gets a generated id,
///   `<snake_case kind>_<n>` as [`edit::apply`](crate::edit::apply)
///   generates one (`claim_1`), with the smallest `n` whose id is the value
///   of no `xmi:id` in the input outside a Difference's `target`, is named
///   by no reference in it (see
///   [References](#references)), and was not generated earlier in the read,
///   so a reference that names no element never resolves to a generated
///   one. It is an ordinary id: [`write()`] writes it, and an edit continues
///   above it, also skipping every referenced id. The same bytes always give
///   the same ids.
/// - An `xmi:id` on a `name` is kept with the element that owns the name, as
///   [`SacmElement::name_id`](crate::model::SacmElement::name_id), and
///   [`write()`] writes it back. A `name` without one gets none.
/// - So is an `xmi:id` on a `content`, `key` or `location`
///   ([`SacmElement::content_id`](crate::model::SacmElement::content_id),
///   [`SacmElement::key_id`](crate::model::SacmElement::key_id),
///   [`SacmElement::location_id`](crate::model::SacmElement::location_id)), and on each
///   `value` in one, by position
///   ([`SacmElement::content_value_ids`](crate::model::SacmElement::content_value_ids),
///   [`SacmElement::key_value_ids`](crate::model::SacmElement::key_value_ids),
///   [`SacmElement::location_value_ids`](crate::model::SacmElement::location_value_ids)).
///   They identify no element.
/// - A `gid` is not an id: it is in no id space, never trimmed and never
///   generated, and a `gid` may be the same string as any `xmi:id`.
///
/// # References
///
/// A reference to another element is held as the id it names, unresolved
/// ([ADR 0026]): [`SacmElement::cited_element`](crate::model::SacmElement::cited_element),
/// [`abstract_form`](crate::model::SacmElement::abstract_form),
/// [`meta_claims`](crate::model::SacmElement::meta_claims),
/// [`sources`](crate::model::SacmElement::sources),
/// [`target`](crate::model::SacmElement::target),
/// [`reasoning`](crate::model::SacmElement::reasoning),
/// [`structure`](crate::model::SacmElement::structure),
/// [`referenced_artifact_elements`](crate::model::SacmElement::referenced_artifact_elements)
/// and [`group_members`](crate::model::SacmElement::group_members) give it,
/// and [`Document::get`] resolves it. In this release the reference ends are
/// `citedElement` and `abstractForm` (§8.2), on every element, `metaClaim`
/// (§11.10) on every Assertion, `reasoning`, `source` and `target` (§11.13)
/// on every asserted relationship, `structure` (§11.12) on an
/// `ArgumentReasoning`, `referencedArtifactElement` (§11.9) on an
/// `ArtifactReference`, `argumentElement` (§11.2) on an `ArgumentGroup` and
/// `artifactElement` (§12.3) on an `ArtifactGroup`. Five are multi-valued:
/// `metaClaim`, `source`, `referencedArtifactElement`, an `ArgumentGroup`'s
/// `argumentElement` and an `ArtifactGroup`'s `artifactElement` hold any number of ids, in document
/// order, repeats included ([ADR 0027], D-ORDER); `metaClaim="m1 m1"` holds
/// `m1` twice. The others hold at most one. A reference resolves by id
/// across the whole input, whatever the order: to a later element, an
/// element of another root, or the element itself. Whether the target is of
/// the end's type is not checked on read;
/// [`validation::validate`](crate::validation::validate) reports one that is
/// not (SACM-REF-002).
///
/// - **The attribute form.** An attribute in no namespace named after the end
///   holds the target ids separated by XML white space (`xsd:IDREFS`, XMI
///   2.5.1 §9.5.2 rule 2j, SACM23-XMI-005). Each token is held exactly as written:
///   `" c2 "` is `c2`, and `#c2` is the id `#c2`, not `c2`. A value with no
///   token, such as `""`, does not conform: it is preserved, never read as
///   no reference. A multi-valued end holds every token, and two or more tokens
///   on a single-valued end are [`ReadError::RepeatedValue`], with one
///   location per token, each at the attribute's name.
/// - **The element form.** An element in no namespace named after the end,
///   with exactly one link attribute, `xmi:idref="id"` (in the XMI
///   namespace, by any prefix) or an unqualified `href="#id"` (XMI 2.5.1
///   §9.5.2 production 2c, rules 2l and 2n, SACM23-XMI-015), anywhere among its
///   siblings. The value is read without the XML white space around it; it
///   names one id, so a value that still holds XML white space inside it is
///   refused (below). The element holds only white space, comments and
///   processing instructions. It reads to the same document as the attribute form,
///   and [`write()`] writes the attribute form. On a multi-valued end, each
///   element appends one id, in document order (XMI 2.5.1 rule 2b). An
///   attribute and an element for the same end, or two elements on a
///   single-valued end, are [`ReadError::RepeatedValue`] (XMI 2.5.1 rule
///   2h), with every occurrence.
/// - **Preserved, never dropped,** at its start tag, is a reference element
///   that does not conform: with both link attributes or neither, with any
///   other attribute such as an unqualified `idref` or `xmi:href`, with an
///   `xmi:idref` that is empty or, once trimmed, still holds XML white space
///   (space, TAB, LF or CR), such as `a b` or `a&#9;b`, since a tolerant
///   [`write()`] would give it back as two ids, or holding a child element
///   or text. So is an end or a reference element in a namespace, such as
///   `sacm:citedElement`, a reference on a `name`, `content`, `key`, `location` or
///   `value`, which have none, and an end on a kind that lacks it, such as
///   `source` on a Claim or `metaClaim` on a Note (see
///   [Preserved content](#preserved-content)).
/// - **Cross-document references are read** (SACM23-XMI-027,
///   SACM23-XMI-029, ADR 0040): an `href` that, trimmed, is
///   `document#fragment`, such as `other.xmi#c2`, with one `#`, a fragment
///   without `#`, and no XML white space, names an element of another
///   document. A fragment that holds `(` or `)` is an `XPointer`, such as
///   `#xpointer(id('c2'))`, whose document part may be empty. A type may
///   stand before the URI reference (XMI 2.5.1 rule 2n): a `QName` that
///   names a concrete SACM class, then XML white space, as in
///   `sacm:Claim other.xmi#c2`; a type before `#id` is read as the id. Each
///   is held in the end's own list,
///   [`SacmElement::hrefs`](crate::model::SacmElement::hrefs), as
///   [`Href`](crate::model::Href) describes, never resolved, and counts as
///   a value of the end; an `ExpressionLangString`'s `expression` element
///   holds one the same way
///   ([`LangString::expression_href`](crate::model::LangString::expression_href)).
///   An `href` that names no element (no `#`, such as `c2`, `#`, empty, an
///   empty fragment, a `#` in the fragment, white space in the URI
///   reference, or a type that names no concrete SACM class) keeps the whole
///   reference element as preserved content, with SACM-XMI-003.
/// - **A reference element carries only its link** (production 2c, rule
///   4e; SACM23-XMI-029): an identity attribute (`xmi:id`, `xmi:uuid`,
///   `xmi:label`), a type, an `XLink` attribute or any other attribute, and
///   an `href` beside an `xmi:idref` (§7.6.2: "any one of which may be used
///   at one time"), do not conform: the whole element is kept with
///   SACM-XMI-003, in both modes.
/// - **Dangling references are kept.** A reference whose id names no
///   element, an id absent from the input or the `xmi:id` of a `name`,
///   `content`, `key`, `location` or `value`, is read in both modes without a
///   diagnostic, held as written, and written back by a tolerant
///   [`write()`]. [`validation::validate`](crate::validation::validate)
///   reports it as SACM-REF-001, an Error (SACM23-VAL-003), and a strict
///   [`write()`] refuses one that names no `xmi:id` it writes, since an
///   IDREF that names no ID is not schema-valid.
///
/// [ADR 0026]: https://github.com/lasrod/libSACM/blob/main/docs/adr/0026-references-between-elements.md
/// [ADR 0027]: https://github.com/lasrod/libSACM/blob/main/docs/adr/0027-asserted-relationships.md
///
/// # Diagnostics
///
/// A document that is read comes with diagnostics, in
/// [`ReadOutput::diagnostics`], sorted by [`Diagnostic`]'s order. Each is
/// reported once per attribute, located at the first character of the
/// attribute's name, or once per element, located at the `<` of its start
/// tag, except SACM-XMI-012, which is once per C++ shape and may be located
/// at text, and SACM-XMI-003, which is once per preserved construct and may
/// be located at text (LIB-COMPAT-001, LIB-COMPAT-003, LIB-COMPAT-008,
/// ADR 0022, ADR 0024, ADR 0033):
///
/// - SACM-XMI-003 ([`Code::XMI_UNKNOWN_CONTENT`]), clause 7.3.3, for each
///   construct kept as preserved content (see
///   [Preserved content](#preserved-content)). It affects the element that
///   holds it, and is an Error in [`Mode::Strict`] and a Warning in
///   [`Mode::Tolerant`].
/// - SACM-XMI-004 ([`Code::XMI_MISSING_ID`]), clause 7.6.1, for each element
///   read without an `xmi:id` (SACM23-XMI-020). It affects the generated id,
///   names the kind and the id, and is Info in both modes.
/// - SACM-ID-004 ([`Code::ID_TRIMMED`]), clause 7.6.1, for each `xmi:id` read
///   without the white space around it (SACM23-XMI-025). It affects the
///   trimmed id, or, for the id of a `name`, `content`, `key`, `location` or `value`, the
///   element that owns it, quotes the value as read, and is Info in both
///   modes.
/// - SACM-XMI-010 ([`Code::XMI_XSI_TYPE`]), clause 9.5.2, for each `xsi:type`
///   on a root, a child, a `name`, a `content`, a `key`, a `location` or a `value`. It
///   affects the element, or, for the others, the element that owns them. It
///   is an Error in [`Mode::Strict`] and
///   a Warning in [`Mode::Tolerant`]: XMI 2.5.1 §9.5.2 rule 2g allows
///   `xsi:type` only if `org.omg.xmi.useSchemaExtensions` is true, and SACM
///   sets no such tag. The type is read as an `xmi:type` would be. An element
///   with both type attributes gets one.
/// - SACM-XMI-011 ([`Code::XMI_VERSION_IGNORED`]), clause 7.5.3, for each
///   `xmi:version` on the `xmi:XMI` element, which affects no element, or on
///   a root, which it affects. It is Info in both modes. An `xmi:version` on
///   any other element is an attribute XMI.xsd does not define, and is
///   preserved.
/// - SACM-XMI-012 ([`Code::XMI_LEGACY_SHAPE`]), clause 7.8.6, a Warning for
///   each C++ shape a tolerant read reads (LIB-COMPAT-008, see
///   [The C++ name and text shapes](#the-c-name-and-text-shapes)), at the
///   shape's first attribute, else its first text character that is not
///   literal white space, else the `<` of a `value` in a `name`, affecting
///   the element whose name or text it gives.
/// - SACM-XMI-013 ([`Code::XMI_SPELLING_VARIANT`]), with the clause of the
///   text that names the construct (12.14, 12.9, 11.4 or 11.2), for each
///   spelling of SACM23-XMI-009 (see [Spellings](#spellings)): a Warning in
///   [`Mode::Tolerant`], which reads it as the resolved name, affecting the
///   element the spelling belongs to, and an Error in [`Mode::Strict`],
///   which keeps the construct as preserved content of the element it sits
///   in, affecting that element.
///
/// A tolerant read also reports, as a Warning, each construct it keeps where
/// a strict read refuses it (ADR 0035 D1, see [Modes](self#modes)), at the
/// strict refusal's location, affecting the element that holds it, or no
/// element for content outside every element:
///
/// - SACM-ENUM-001 ([`Code::ENUM_INVALID_LITERAL`]) for a value that is no
///   literal, kept as written; the property holds its default.
/// - SACM-MULT-001 ([`Code::MULTIPLICITY_VIOLATION`]) for each occurrence of
///   a property after its first, kept as written; for two ids in the
///   attribute of a single-valued end, at the second, with the whole
///   attribute kept and the end empty.
/// - SACM-XMI-005 ([`Code::XMI_MISSING_TYPE`]) for an untyped child under
///   an abstract role, kept whole in its parent.
/// - SACM-ID-002 ([`Code::ID_INVALID`]) for an empty `xmi:id`: an element is
///   read with a generated id (and no SACM-XMI-004), a `name`, `content`,
///   `key` or `value` with none, and one in preserved content is kept as
///   written, outside the id space.
/// - SACM-XMI-003 for an attribute, text or an XMI element XMI 2.5.1 does
///   not define on or in `xmi:XMI`, held by the document, and for an
///   attribute whose prefix is rebound, kept under `nsN`.
///
/// None of these diagnostics refuses the input, and a tolerant read reports
/// no Error. A strict read refuses each of these constructs instead, and the
/// C++ shapes as [`ReadError::LegacyShape`]; otherwise both modes give the
/// same document, preserved content included. A later minor release may add
/// a read diagnostic, so a strict read may then report an Error where it did
/// not before; the release notes list each one.
///
/// # Limits
///
/// Both modes read under the [`Limits`] of [`ReadOptions::with_limits`]
/// (LIB-SEC-003, LIB-SEC-007): by default 32 MiB of input, a nesting depth
/// of 256, 1,000,000 XML nodes, 256 namespace bindings in scope at an
/// element and 256 attributes on an element. A breach is
/// [`ReadError::LimitExceeded`] (SACM-SEC-003). The size is checked first, on the bytes; the depth, the
/// bindings in scope, the bindings in scope at the elements that declare one
/// (which count against the node limit) and the attributes of each element,
/// the first namespace declaration of each prefix not counted, by one linear
/// scan before parsing; and the nodes by the parser.
/// Everything else a read keeps besides the parsed tree counts against the
/// size limit, in one budget (ADR 0041 amendment 1): the namespace text that
/// preserved content keeps, each namespace URI of a kept name and
/// `prefix + URI + 10` for each binding it keeps, counted as each element or
/// attribute is kept (`SACM-SEC-003/kept-namespaces`); and the text of each
/// diagnostic the read emits, the bytes of its text arguments and of its
/// affected ids, counted where it is emitted (`SACM-SEC-003/kept-text`). A
/// diagnostic's rendered message repeats its text arguments (escaped, up to
/// six times their size) and is not counted on its own, so the diagnostics
/// take at most about seven times what the budget counts. A few templates
/// render an argument twice. The read is refused
/// at the element, attribute or diagnostic where the budget passes the limit.
/// So the memory a read takes is bounded by the limits, not by the input's
/// shape.
///
/// The peak memory of a read at the defaults was measured (slice 0072: a
/// release build, rustc 1.100, Windows 11 on an AMD Ryzen 9 8945HS with 31
/// GiB; the peak working set of the process, the input included). A flat
/// ArgumentPackage of 341,176 Claims just under 32 MiB peaks at 811 MiB, in
/// 1.9 s; 999,998 preserved `<x/>`, the most the node limit lets through (4
/// MB, whose diagnostics keep 18 MB of text, within the size limit), at 1.25
/// GiB, in 1.7 to 2.3 s, measured again with the one budget, most of it the
/// SACM-XMI-003 Warning each preserved element gets; and a root with 255
/// bindings whose 3,905 Claims declare one more each, up to the
/// namespace-copy limit (281 KB), at 15 MiB, in 0.24 s.
/// An application that reads untrusted files with less memory than that sets
/// lower limits.
///
/// # Other SACM namespaces
///
/// No SACM revision sets a namespace for its documents (spec defect D7), so
/// files use many (LIB-COMPAT-004; ADR 0041 D4, D5). A root in the pinned
/// namespace `http://www.omg.org/spec/SACM/20220301` is SACM 2.3. A tolerant
/// read also reads as SACM 2.3:
///
/// - a root in the namespace of an older SACM revision it recognises, with
///   one SACM-XMI-008 Warning: `http://www.omg.org/spec/SACM/20170843`,
///   `/20190235` and `/20210218` (2.0, 2.1, 2.2), `http://www.omg.org/spec/SACM/2.0`,
///   `/2.1` and `/2.2`, `http://omg.sacm/2.0`, `/2.1` and `/2.2`, and
///   `http://SACM_ACEditor/sacm/2.1`, each alone or followed by `/` and a
///   package name of ASCII letters. An Event's `occurence`, the 2.0 to 2.2
///   spelling, is read as its date, with one SACM-XMI-008 Warning more;
/// - a root in any other namespace whose URI contains `sacm` in any letter
///   case, such as the undated `http://www.omg.org/spec/SACM/2.3`, with one
///   SACM-XMI-002 Warning.
///
/// A strict read refuses both as [`ReadError::Unsupported`]. Both modes
/// refuse a root in the SACM 2.4 namespace (`…/20260504` or `…/2.4`) with
/// SACM-XMI-008, until LIB-COMPAT-018 decides how it is read, and a root in
/// no namespace or in one without `sacm` as [`ReadError::InvalidRoot`]. In a
/// root, the namespaces of its family name SACM classes: the base and, for a
/// recognised revision or the undated 2.3, the base with a package; a type
/// in any other namespace is foreign (SACM-XMI-003). [`write()`] writes SACM
/// 2.3 in the pinned namespace.
///
/// ```
/// use sacm::diagnostic::Code;
/// use sacm::xmi::{self, Mode, ReadError, ReadOptions, WriteOptions};
///
/// let older = br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/2.2/Argumentation"
///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="p"/>"#;
/// let output = xmi::read(older, &ReadOptions::default().with_mode(Mode::Tolerant))?;
/// assert_eq!(output.diagnostics()[0].code(), Code::XMI_OTHER_REVISION);
/// let text = xmi::write(output.document(), &WriteOptions::default())
///     .expect("an empty package writes");
/// assert!(text.contains("xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\""));
///
/// let strict = ReadOptions::default().with_mode(Mode::Strict);
/// let error = xmi::read(older, &strict).unwrap_err();
/// assert!(matches!(error, ReadError::Unsupported { .. }));
/// assert_eq!(error.code(), Code::XMI_OTHER_REVISION);
/// # Ok::<(), ReadError>(())
/// ```
///
/// # The EMF dialect
///
/// Most SACM tools are built on EMF, which binds `xmi` to
/// `http://www.omg.org/XMI`, no XMI version's namespace (XMI 2.5.1 §7.5.3),
/// and names same-document targets by fragment paths such as
/// `//@argumentPackage.0/@argumentElement.3` or `/1` instead of `xmi:id`s
/// (§7.10.2). A tolerant read reads that dialect (LIB-COMPAT-002, ADR 0044
/// D1, D2):
///
/// - the `XMI` element, and the attributes `id`, `type`, `idref`, `version`,
///   `uuid` and `label` in `http://www.omg.org/XMI`, as XMI 2.5.1's, with one
///   SACM-XMI-015 Warning per document at the first name in that namespace.
///   Any other name in it is kept as written (SACM-XMI-003), and an element
///   carrying the same XMI attribute in both namespaces reads XMI 2.5.1's
///   and keeps EMF's as written (LIB-COMPAT-019). An element that keeps a
///   `type` in EMF's namespace or an `xsi:type` (only a root keeps one), and
///   none in XMI 2.5.1's, is written with its own `xmi:type`;
/// - a token of a reference end's attribute, the fragment of a
///   same-document `href` and an `ExpressionLangString`'s `expression` that
///   begin with `/` and equal no `xmi:id`, as the id of the element they
///   name: an element read as an element, the root of a resolved same-file
///   proxy, or a preserved element with an `xmi:id`. One SACM-XMI-017
///   Warning per document counts them; one that names nothing is kept as
///   written, and [`validation::validate`](crate::validation::validate)
///   reports it (SACM-REF-001). Paths inside preserved content are never
///   rewritten.
///
/// Both saves write XMI 2.5.1 and ids. A strict read refuses any name in
/// `http://www.omg.org/XMI` outside the content of an `xmi:Extension` or of
/// a Difference's `target`, which XMI.xsd leaves open and both modes keep as
/// written, as [`ReadError::Unsupported`] (SACM-XMI-015), and resolves no
/// path. Both modes refuse an `XMI` element in any other
/// namespace, or in none.
///
/// ```
/// use sacm::diagnostic::Code;
/// use sacm::xmi::{self, Mode, ReadError, ReadOptions, WriteOptions};
///
/// let emf = br#"<sacm:ArgumentPackage xmi:version="2.0" xmlns:xmi="http://www.omg.org/XMI"
///     xmlns:sacm="http://www.omg.org/spec/SACM/20220301" xmi:id="p">
///   <name content="Arguments"/>
///   <argumentElement xmi:type="sacm:Claim" xmi:id="c1"><name content="Safe"/></argumentElement>
///   <argumentElement xmi:type="sacm:Claim" xmi:id="c2"><name content="Tested"/></argumentElement>
///   <argumentElement xmi:type="sacm:AssertedInference" xmi:id="i1"
///       source="//@argumentElement.1" target="//@argumentElement.0"><name content="By test"/></argumentElement>
/// </sacm:ArgumentPackage>"#;
/// let output = xmi::read(emf, &ReadOptions::default().with_mode(Mode::Tolerant))?;
/// let inference = output.document().get("i1").ok_or("not read")?;
/// assert_eq!((inference.sources()[0].as_str(), inference.targets()[0].as_str()), ("c2", "c1"));
/// let codes: Vec<Code> = output.diagnostics().iter().map(|d| d.code()).collect();
/// assert!(codes.contains(&Code::XMI_WRAPPER_NAMESPACE) && codes.contains(&Code::XMI_FRAGMENT_PATH));
/// let text = xmi::write(output.document(), &WriteOptions::default())?;
/// assert!(text.contains(r#"source="c2" target="c1""#) && !text.contains("http://www.omg.org/XMI\""));
///
/// let strict = ReadOptions::default().with_mode(Mode::Strict);
/// let error = xmi::read(emf, &strict).unwrap_err();
/// assert!(matches!(error, ReadError::Unsupported { .. }));
/// assert_eq!(error.code(), Code::XMI_WRAPPER_NAMESPACE);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Errors
///
/// The checks run in this order, and the first failure is returned:
///
/// 1. the input's size (`LimitExceeded`);
/// 2. the byte order mark and the first bytes: UTF-16 with a mark is
///    decoded; UTF-16 without one, UTF-32 and UCS-4 are `Unsupported`;
/// 3. the grammar of the XML declaration's `<?xml`, `version` and `encoding`
///    (`Malformed`);
/// 4. the declared encoding: without a UTF-16 mark, `ISO-8859-1` in any
///    letter case decodes the input as ISO-8859-1 and the declaration is
///    checked again (`Malformed` after a UTF-8 mark), and another one is
///    `Unsupported` unless UTF-8 or ASCII; after a UTF-16 mark, `Malformed`
///    unless UTF-16, UTF-16LE or UTF-16BE. Then, for an accepted one, the
///    rest of the declaration: `standalone` and `?>` (`Malformed`);
/// 5. under an ASCII declaration, a non-ASCII character (`Malformed`);
/// 6. the encoding (`Malformed`): invalid UTF-8 at the first invalid byte;
///    in UTF-16, an unpaired surrogate at the surrogate, or an odd byte count
///    after the last whole character; ISO-8859-1 has none;
/// 7. the scan before parsing, at each start tag in document order: the
///    depth, the bindings in scope, the bindings in scope at the elements
///    that declare one, and the attributes of the element, the first
///    namespace declaration of each prefix not counted (`LimitExceeded`). The
///    scan stops at a DOCTYPE;
/// 8. the parse: a DOCTYPE (`Doctype`), the node limit (`LimitExceeded`), or
///    a well-formedness error (`Malformed`), whichever comes first;
/// 9. the well-formedness errors this release refuses after the XML parser
///    has accepted the input, the earliest first (`Malformed`): a character
///    reference to a surrogate or above U+10FFFF, a declaration of the
///    namespace prefix `xmlns` (`xmlns:xmlns`), a namespace declaration
///    repeated in one start tag (`xmlns`, or `xmlns:xml`, twice), a namespace
///    prefix undeclared with `xmlns:p=""`, an element or attribute name with an
///    empty prefix (`<:c>`, `</:c>`, ` :a="1"`), and a processing instruction
///    whose target (the XML Name after `<?`) is `xml` in any letter case,
///    contains a colon, or is followed by neither white space nor `?>`.
///    Within the W3C XML Conformance Test Suite (20130923), this list is
///    closed: every not-well-formed XML 1.0 and Namespaces 1.0 case in it is
///    refused. Outside the suite, other leniencies of the parser may remain:
///    a sweep (LIB-SEC-005) checks only that these checks agree with the
///    parser's tree;
/// 10. the roots, in two passes. First the names and namespaces: an `XMI`
///     element in neither XMI 2.5.1's namespace nor `http://www.omg.org/XMI`
///     is `Unsupported`, and so, in strict mode, is one in
///     `http://www.omg.org/XMI`; any other element of either namespace as the
///     document element is `InvalidRoot`; an `xmi:XMI` element
///     with no root is `NoPackage`; otherwise the first root not named as a
///     package kind, or in no namespace or one that does not name SACM, is
///     `InvalidRoot`; then the first root in the SACM 2.4 namespace, and in
///     strict mode in an older SACM revision's or a SACM-like one, is
///     `Unsupported` (see [Other SACM namespaces](#other-sacm-namespaces)).
///     Then, in strict mode, the first element or attribute name in
///     `http://www.omg.org/XMI` outside the content of an `xmi:Extension` or
///     of a Difference's `target`, in document order, is `Unsupported`,
///     before any id is read (see [The EMF dialect](#the-emf-dialect)).
///     Then, in document order, the `xmi:XMI` element's
///     attributes, each root with its content, and the content beside it
///     (`Unsupported`). An element's start tag (its role, type, kind and id)
///     is checked before its other attributes, and those before its content.
///     An empty id is `InvalidId`, and an id whose value an earlier element,
///     name, `content`, `key`, `value` or preserved element carries is
///     `DuplicateId`, so a repeat is refused before anything after its id. A
///     value that is not a literal is `InvalidLiteral` where it is met. A
///     `name`, `content`, `key`, value or reference element is checked whole
///     when it is met, then, unless it is preserved, against the occurrences
///     before it, so the second occurrence of a value is `RepeatedValue`
///     unless it is itself refused or preserved first. A later occurrence is
///     listed in `RepeatedValue`'s [`locations`](ReadError::locations) even
///     if it does not conform itself, because each occurrence is checked
///     against those before it. The namespace text preserved content keeps
///     counts against the size limit as it is kept, and the text of each
///     diagnostic where it is emitted, in one budget (`LimitExceeded`);
/// 11. in strict mode, the first C++ shape in the input (`LegacyShape`).
///
/// A tolerant read refuses only `Doctype`, `Malformed`, `InvalidRoot`,
/// `NoPackage`, `DuplicateId`, `LimitExceeded`, and `Unsupported` for an
/// encoding, the parser's capacity, a SACM 2.4 root or an `XMI` element in
/// neither XMI 2.5.1's namespace nor `http://www.omg.org/XMI`; it keeps every other construct listed
/// below, with a Warning (see [Diagnostics](#diagnostics)), so those
/// refusals are a strict read's only. The refusals:
///
/// - [`ReadError::Doctype`] (SACM-SEC-001): the input has a DOCTYPE, with or
///   without entity declarations.
/// - [`ReadError::Malformed`] (SACM-XML-001): the input is not well-formed
///   XML, is truncated, is not valid UTF-8 or UTF-16, declares ASCII but
///   holds a non-ASCII character, has a UTF-16 byte order mark but declares
///   another encoding, or has a UTF-8 byte order mark but declares
///   ISO-8859-1.
/// - [`ReadError::InvalidRoot`] (SACM-XMI-001): a root's local name is not
///   one of the twelve SACM package kinds, or the root is in no namespace or
///   one whose URI does not contain `sacm`, or the document element is in
///   an XMI namespace (XMI 2.5.1's or `http://www.omg.org/XMI`) but is not
///   `xmi:XMI`.
/// - [`ReadError::NoPackage`] (SACM-XMI-001): the `xmi:XMI` element holds no
///   root: no element child outside the XMI namespace.
/// - [`ReadError::DuplicateId`] (SACM-ID-001): two or more elements (names,
///   `content`s, `key`s, `value`s and preserved elements included) carry the
///   same `xmi:id` (XMI 2.5.1 §7.6.1), compared by `xsd:ID` value (see
///   [Ids](#ids)). It names every element that carries the first id to
///   repeat.
/// - [`ReadError::InvalidId`] (SACM-ID-002): the `xmi:id` of an element, of
///   a `name`, `content`, `key`, `location` or `value`, or of an element of preserved
///   content, is empty. Any other id is read by its `xsd:ID` value; one that
///   is still not an XML `NCName`, such as `1c` or `a b`, is reported by
///   [`validation::validate`](crate::validation::validate), and refused by a
///   strict [`write()`].
/// - [`ReadError::RepeatedValue`] (SACM-MULT-001): an element's `gid`,
///   `isCitation`, `isAbstract`, `citedElement`, `abstractForm` or `name`, a
///   `UtilityElement`'s `content`, a `TaggedValue`'s `key`, the `lang` or
///   `content` of a name or `value`, an Assertion's `assertionDeclaration`,
///   an asserted relationship's `isCounter`, `reasoning` or `target`, or an
///   `ArgumentReasoning`'s `structure`, is given more than once: as an
///   attribute and an element, which XMI 2.5.1 §9.5.2 rule 2h forbids, or as
///   two or more elements, though the property holds one value. This holds
///   even with equal values. It gives every occurrence. Two or more ids in
///   the attribute of a single-valued end are too, with one location per id,
///   at the attribute's name. A multi-valued end (`metaClaim`, `source`,
///   `referencedArtifactElement`, an `ArgumentGroup`'s `argumentElement`, an
///   `ArtifactGroup`'s `artifactElement`) given both as
///   an attribute and as elements is too, with every occurrence. Preserved
///   content is never an occurrence.
/// - [`ReadError::InvalidLiteral`] (SACM-ENUM-001): an Assertion's
///   `assertionDeclaration` is not exactly one of the six literals, or an
///   `isCitation`, `isAbstract` or `isCounter` is not one of `true`,
///   `false`, `1` and `0`, white space around them aside, as an attribute or
///   as an element, or the `xsi:nil` of a value element is not either. It
///   holds the value as read.
/// - [`ReadError::MissingType`] (SACM-XMI-005): a child under a role whose
///   declared type is abstract has no `xmi:type` or `xsi:type`, and no
///   `xmi:idref` or `href` (the reference form of a composite: a proxy, of
///   a root of the same document or of an element of another document).
/// - [`ReadError::LimitExceeded`] (SACM-SEC-003), in both modes: the input
///   passes one of the [`Limits`] (see [Limits](#limits)).
/// - [`ReadError::LegacyShape`] (SACM-XMI-012), in strict mode only: a C++
///   name or text shape (see
///   [The C++ name and text shapes](#the-c-name-and-text-shapes)).
/// - [`ReadError::Unsupported`] (the code a tolerant read keeps the input
///   with: SACM-XMI-003; the code a tolerant
///   read reads it with: SACM-XMI-002, SACM-XMI-008 or SACM-XMI-015; or
///   SACM-XMI-008, SACM-XMI-015, SACM-XML-002 or SACM-SEC-002 for what both
///   modes refuse; LIB-API-017): well-formed input a strict read
///   does not read, and that it does not keep as preserved content
///   (LIB-API-011). Its reason ends with one label that says why:
///   `(a limit of this version)` for a limit, `(strict mode never reads it;
///   ROW reads it in tolerant mode)` for input a tolerant read reads (which
///   need not be defective: a document in another SACM namespace is not,
///   D-NS-PIN), and `(a defect in the input)` for input that does not
///   conform and that a strict read refuses. A tolerant read refuses only
///   the limits, a SACM 2.4 root and an `XMI` element in an XMI namespace
///   it does not read, reads a root in another SACM namespace and the XMI
///   names EMF writes, and keeps the rest (SACM-XMI-003). No
///   conforming input is refused as not read yet: XMI metadata and every
///   link form are read (SACM23-XMI-019, SACM23-XMI-029). In this release
///   they are:
///   - limits: UTF-16 without a byte order mark, UTF-32 or UCS-4 input, a
///     declared encoding other than UTF-8, ASCII or ISO-8859-1 in input
///     without a UTF-16 mark, more than 65,535 distinct namespace bindings or
///     2³² attributes, and a root in the SACM 2.4 namespace (LIB-COMPAT-018);
///   - an `XMI` element in neither XMI 2.5.1's namespace nor EMF's
///     `http://www.omg.org/XMI`, such as XMI 2.4.1's, or in none;
///   - in strict mode, input a tolerant read reads: a root in an older SACM
///     revision's namespace or a SACM-like one (LIB-COMPAT-004), and any
///     name in EMF's XMI namespace (LIB-COMPAT-002).
///     The spellings of SACM23-XMI-009 are no refusal: a strict read keeps
///     each as preserved content with a SACM-XMI-013 Error (see
///     [Spellings](crate::xmi::read#spellings));
///   - defects with nowhere to be kept: any other attribute, text, or an
///     XMI-namespace element XMI 2.5.1 does not define there, on or in the
///     `xmi:XMI` element, and an attribute that would be preserved but whose
///     prefix `sacm` or `xmi` is bound to another namespace (see
///     [Preserved content](#preserved-content)).
///
/// # Examples
///
/// ```
/// use sacm::diagnostic::{Code, Severity};
/// use sacm::model::{AssertionDeclaration, ElementKind};
/// use sacm::xmi::{self, Mode, ReadError, ReadOptions};
///
/// let options = ReadOptions::default().with_mode(Mode::Tolerant);
/// let input = br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="arguments"/>"#;
/// let output = xmi::read(input, &options)?;
/// let root = output.document().get("arguments").expect("the root was read");
/// assert_eq!(root.kind(), ElementKind::ArgumentPackage);
/// assert!(output.diagnostics().is_empty());
///
/// // A root's name is read.
/// let named = br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="arguments">
///   <name lang="en" content="Arguments"/>
/// </sacm:ArgumentPackage>"#;
/// let document = xmi::read(named, &options)?.into_document();
/// let name = document.get("arguments").and_then(|root| root.name());
/// assert_eq!(name.and_then(|name| name.content()), Some("Arguments"));
///
/// // A Claim in a package is read as its child.
/// let claim = br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="arguments">
///   <argumentElement xmi:type="sacm:Claim" xmi:id="c1"/>
/// </sacm:ArgumentPackage>"#;
/// let document = xmi::read(claim, &options)?.into_document();
/// let c1 = document.get("c1").expect("the Claim was read");
/// assert_eq!(c1.kind(), ElementKind::Claim);
/// assert_eq!(c1.parent().map(|parent| parent.id().as_str()), Some("arguments"));
///
/// // A Claim's `assertionDeclaration` is read.
/// let declared = br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="arguments">
///   <argumentElement xmi:type="sacm:Claim" xmi:id="c1" assertionDeclaration="assumed"/>
/// </sacm:ArgumentPackage>"#;
/// let document = xmi::read(declared, &options)?.into_document();
/// let c1 = document.get("c1").expect("the Claim was read");
/// assert_eq!(c1.assertion_declaration(), Some(AssertionDeclaration::Assumed));
///
/// // A gid and a description, with its content, are read.
/// let described = br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="arguments" gid="urn:x:1">
///   <description xmi:id="d1">
///     <content><value lang="en" content="Why the brakes are safe"/></content>
///   </description>
/// </sacm:ArgumentPackage>"#;
/// let document = xmi::read(described, &options)?.into_document();
/// let arguments = document.get("arguments").expect("the root was read");
/// assert_eq!(arguments.gid(), Some("urn:x:1"));
/// let d1 = arguments.descriptions().next().expect("the description was read");
/// // A description's content() is a MultiLangString; the text is the content() of one of its LangString values.
/// let content = d1.content().expect("it has a content");
/// assert_eq!(content.values()[0].content(), Some("Why the brakes are safe"));
///
/// // An `xsi:type` is read as `xmi:type` and reported as SACM-XMI-010 at the
/// // attribute's name: a Warning in tolerant mode, an Error in strict mode.
/// let typed = br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001"
///     xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmi:id="arguments">
///   <argumentElement xsi:type="sacm:Claim" xmi:id="c1"/>
/// </sacm:ArgumentPackage>"#;
/// let output = xmi::read(typed, &options)?;
/// assert_eq!(output.document().get("c1").map(|c1| c1.kind()), Some(ElementKind::Claim));
/// let found = &output.diagnostics()[0];
/// assert_eq!(found.code(), Code::XMI_XSI_TYPE);
/// assert_eq!(found.severity(), Severity::Warning);
/// assert_eq!(found.affected()[0].as_str(), "c1");
/// assert_eq!(found.location().map(|at| (at.line(), at.column())), Some((4, 20)));
/// let strict = xmi::read(typed, &ReadOptions::default().with_mode(Mode::Strict))?;
/// assert_eq!(strict.diagnostics()[0].severity(), Severity::Error);
///
/// // A value that is not a literal (here the wrong letter case) is refused by
/// // a strict read at the attribute's name, line 3, column 54, and never read
/// // as `asserted`. A tolerant read keeps it as preserved content, with a
/// // Warning there.
/// let misspelt = br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="arguments">
///   <argumentElement xmi:type="sacm:Claim" xmi:id="c1" assertionDeclaration="Assumed"/>
/// </sacm:ArgumentPackage>"#;
/// let strict_options = ReadOptions::default().with_mode(Mode::Strict);
/// let error = xmi::read(misspelt, &strict_options).unwrap_err();
/// assert!(matches!(&error, ReadError::InvalidLiteral { value, .. } if value == "Assumed"));
/// assert_eq!(error.code(), Code::ENUM_INVALID_LITERAL);
/// assert_eq!(error.location().map(|at| (at.line(), at.column())), Some((3, 54)));
/// let output = xmi::read(misspelt, &options)?;
/// let found = &output.diagnostics()[0];
/// assert_eq!((found.code(), found.severity()), (Code::ENUM_INVALID_LITERAL, Severity::Warning));
/// assert_eq!(found.location(), error.location());
/// assert!(output.document().get("c1").is_some_and(|c1| c1.has_preserved_content()));
///
/// // An element without an `xmi:id` is read with a generated one, reported
/// // as SACM-XMI-004 Info at its start tag.
/// let idless = br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="arguments">
///   <argumentElement xmi:type="sacm:Claim"/>
/// </sacm:ArgumentPackage>"#;
/// let output = xmi::read(idless, &options)?;
/// assert_eq!(output.document().get("claim_1").map(|c| c.kind()), Some(ElementKind::Claim));
/// let found = &output.diagnostics()[0];
/// assert_eq!((found.code(), found.severity()), (Code::XMI_MISSING_ID, Severity::Info));
/// assert_eq!(found.affected()[0].as_str(), "claim_1");
///
/// // References, in the attribute form and the element form. One names no
/// // element: it is kept, and validation reports it as SACM-REF-001.
/// let cited = br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="arguments">
///   <name content="Arguments"/>
///   <argumentElement xmi:type="sacm:Claim" xmi:id="c1" isCitation="true" citedElement="c2">
///     <name content="C1"/>
///   </argumentElement>
///   <argumentElement xmi:type="sacm:Claim" xmi:id="c2" isCitation="1">
///     <name content="C2"/>
///     <citedElement xmi:idref="c9"/>
///   </argumentElement>
/// </sacm:ArgumentPackage>"#;
/// let document = xmi::read(cited, &options)?.into_document();
/// let c1 = document.get("c1").expect("the Claim was read");
/// assert!(c1.is_citation());
/// assert_eq!(c1.cited_element().map(|id| id.as_str()), Some("c2"));
/// let c2 = document.get("c2").expect("the Claim was read");
/// assert_eq!(c2.cited_element().map(|id| id.as_str()), Some("c9"));
/// assert!(document.get("c9").is_none());
/// let found = sacm::validation::validate(&document);
/// assert_eq!(found.len(), 1);
/// assert_eq!((found[0].code(), found[0].severity()), (Code::REF_DANGLING, Severity::Error));
///
/// // A root that is not a SACM package.
/// let error = xmi::read(b"<r/>", &options).unwrap_err();
/// assert!(matches!(error, ReadError::InvalidRoot { .. }));
/// assert_eq!(error.code(), Code::XMI_INVALID_ROOT);
/// assert_eq!(
///     error.to_string(),
///     "the root element \"r\" is not a SACM package at line 1, column 1"
/// );
///
/// // An XML external entity: refused at the DOCTYPE, never resolved.
/// let xxe = b"<?xml version=\"1.0\"?>\n\
///     <!DOCTYPE r [ <!ENTITY xxe SYSTEM \"file:///etc/passwd\"> ]>\n\
///     <r>&xxe;</r>";
/// let error = xmi::read(xxe, &options).unwrap_err();
/// assert_eq!(error.code(), Code::XML_DOCTYPE_REJECTED);
///
/// // UTF-16 with a byte order mark (U+FEFF) reads as the same text in UTF-8.
/// let text = "\u{FEFF}<sacm:ArgumentPackage xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"
///     xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" xmi:id=\"arguments\"/>";
/// let utf16: Vec<u8> = text.encode_utf16().flat_map(u16::to_le_bytes).collect();
/// assert!(xmi::read(&utf16, &options)?.document().get("arguments").is_some());
/// # Ok::<(), ReadError>(())
/// ```
pub fn read(input: &[u8], options: &ReadOptions) -> Result<ReadOutput, ReadError> {
    let mode = options.mode;
    let limits = options.limits;
    limits::check_size(input, &limits)?;
    let text = input::decode(input)?;
    limits::scan(&text, &limits)?;
    let tree = input::parse(&text, &limits)?;
    let (document, mut diagnostics) = roots::read(&text, &tree, options)?;
    diagnostics.sort();
    Ok(ReadOutput {
        document,
        diagnostics,
        mode,
    })
}

/// How strictly [`read`] and [`write`](fn@write) treat a document (D-STRICT,
/// ADR 0022, ADR 0035).
///
/// # Modes
///
/// - [`Strict`](Self::Strict) aims at SACM 2.3 conformance. A strict read
///   reports each recognised variation it reads as an Error (in this release,
///   `xsi:type`, SACM-XMI-010), and each construct it keeps as preserved
///   content as an Error (SACM-XMI-003, or SACM-XMI-013 for a name spelled
///   as in the other of the text and the model, SACM23-XMI-009), and refuses
///   everything else it does not read: the C++ name and text shapes ([`ReadError::LegacyShape`],
///   SACM-XMI-012), a value that is no literal, a value given twice, an
///   untyped child under an abstract role, an empty `xmi:id`, and what the
///   reader cannot represent. Otherwise it returns the document, and does not
///   validate it. To check a file, test [`ReadOutput::has_errors`] on the
///   read, and [`diagnostic::has_errors`](crate::diagnostic::has_errors) on
///   the findings of [`validation::validate`](crate::validation::validate).
///
///   A strict write writes only schema-valid output: it refuses a document
///   that holds preserved content ([`WriteError::PreservedContent`],
///   SACM-XMI-006), then one with an `xmi:id` that is no `NCName` or a
///   reference to no `xmi:id` it writes ([`WriteError::Invalid`]). It never
///   validates: a draft with an open model Error, such as a package with no
///   name, writes.
/// - [`Tolerant`](Self::Tolerant) opens a well-formed file whose roots are all
///   SACM packages, in SACM 2.3, an older revision's or a SACM-like namespace
///   (LIB-COMPAT-004), unless it has a DOCTYPE, a SACM 2.4 root, an `XMI`
///   element in a namespace other than XMI 2.5.1's or EMF's
///   `http://www.omg.org/XMI`, a repeated `xmi:id` or an encoding it cannot
///   decode, or passes the parser's capacity or its [`Limits`]. It also accepts
///   recognised variations, the dialect EMF writes among them (its XMI
///   namespace `http://www.omg.org/XMI` and its fragment paths,
///   LIB-COMPAT-002), reads elements typed with classes the application
///   declares subclasses of SACM 2.3 classes ([`ReadOptions::with_subclass`],
///   LIB-COMPAT-017), and keeps every construct a strict read refuses but a
///   malformed or unsafe input, a repeated `xmi:id`, an encoding, a SACM 2.4
///   root, an `XMI` element in another XMI namespace and a limit, as preserved
///   content; it reports each as a Warning, and never an Error. A tolerant
///   write writes the document as it is held, preserved content included.
///
/// `Mode` has no default of its own; [`ReadOptions`] and [`WriteOptions`]
/// both default to `Strict`. The two modes of a write write the same
/// bytes for a document both write. Later releases may add variants, so
/// match it with a fallback arm.
///
/// ```
/// use sacm::xmi::{self, Mode, ReadOptions, WriteOptions};
///
/// assert_eq!(ReadOptions::default().mode(), Mode::Strict);
/// assert_eq!(WriteOptions::default().mode(), Mode::Strict);
/// let tolerant = ReadOptions::default().with_mode(Mode::Tolerant);
/// assert_eq!(tolerant.mode(), Mode::Tolerant);
///
/// // A package with no name, a model Error, writes in both modes.
/// let draft = br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="draft"/>"#;
/// let document = xmi::read(draft, &ReadOptions::default())?.into_document();
/// let tolerant_write = WriteOptions::default().with_mode(Mode::Tolerant);
/// assert_eq!(xmi::write(&document, &WriteOptions::default())?, xmi::write(&document, &tolerant_write)?);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Mode {
    /// Aims at SACM 2.3 conformance: a read reports each recognised variation
    /// and each preserved construct as an Error and refuses what it does not
    /// read, and a write refuses a document that holds preserved content or
    /// whose output would not be schema-valid.
    Strict,
    /// Opens a well-formed SACM file unless it is one of the inputs the
    /// [`Mode`] docs list as refused (a DOCTYPE, a repeated `xmi:id`, an
    /// encoding, a SACM 2.4 root, an `XMI` element in a namespace other than
    /// XMI 2.5.1's or EMF's, a limit): a read keeps what a strict read refuses,
    /// with a Warning, reads a root in an older SACM revision's or a SACM-like
    /// namespace as SACM 2.3, the dialect EMF writes as XMI 2.5.1, and elements
    /// typed with classes the application declares subclasses of SACM 2.3
    /// classes as those classes, and a write writes the document as it is held.
    Tolerant,
}

impl Mode {
    /// The mode's name in lower case, `"strict"` or `"tolerant"`: what a
    /// command line or a settings file spells it, and what it displays as.
    ///
    /// ```
    /// use sacm::xmi::Mode;
    ///
    /// assert_eq!(Mode::Strict.as_str(), "strict");
    /// assert_eq!(Mode::Tolerant.to_string(), "tolerant");
    /// ```
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Strict => "strict",
            Self::Tolerant => "tolerant",
        }
    }
}

impl fmt::Display for Mode {
    /// Writes [`Mode::as_str`].
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(self.as_str())
    }
}

/// How [`read`] reads a document: in which [`Mode`], strict by default,
/// under which [`Limits`], the defaults by default (LIB-SEC-003), and which
/// classes outside SACM 2.3 the application declares subclasses of SACM 2.3
/// classes, none by default ([`with_subclass`](Self::with_subclass),
/// LIB-COMPAT-017).
///
/// Options compare and hash by content: the order in which declarations are
/// made does not matter.
///
/// ```
/// use sacm::xmi::{self, Mode, ReadOptions};
///
/// let input = br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="arguments"/>"#;
/// let output = xmi::read(input, &ReadOptions::default())?;
/// assert_eq!(output.mode(), Mode::Strict);
/// # Ok::<(), xmi::ReadError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct ReadOptions {
    /// The mode to read in.
    mode: Mode,
    /// The limits to read under.
    limits: Limits,
    /// The declared subclasses: by namespace URI, then by class name, the
    /// SACM 2.3 class each is a subclass of (LIB-COMPAT-017, ADR 0044 D3).
    /// Ordered maps, so `==` and `Hash` do not depend on call order.
    subclasses: BTreeMap<String, BTreeMap<String, ElementKind>>,
}

impl Default for ReadOptions {
    /// Reads in [`Mode::Strict`], under the default [`Limits`], with no
    /// declared subclass.
    fn default() -> Self {
        Self {
            mode: Mode::Strict,
            limits: Limits::default(),
            subclasses: BTreeMap::new(),
        }
    }
}

impl ReadOptions {
    /// These options, reading in `mode`.
    #[must_use]
    pub fn with_mode(mut self, mode: Mode) -> Self {
        self.mode = mode;
        self
    }

    /// The mode to read in.
    #[must_use]
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// These options, reading under `limits`.
    ///
    /// ```
    /// use sacm::xmi::{Limits, ReadOptions};
    ///
    /// let limits = Limits::default().with_max_input_bytes(16 << 20).with_max_depth(64);
    /// let options = ReadOptions::default().with_limits(limits);
    /// assert_eq!(options.limits().max_depth(), 64);
    /// ```
    #[must_use]
    pub fn with_limits(mut self, limits: Limits) -> Self {
        self.limits = limits;
        self
    }

    /// The limits to read under.
    #[must_use]
    pub fn limits(&self) -> Limits {
        self.limits
    }

    /// These options, declaring the class `name` of the namespace
    /// `namespace` a subclass of the SACM 2.3 class `kind` (LIB-COMPAT-017,
    /// ADR 0044 D3).
    ///
    /// A tolerant read then reads a child element typed with that class
    /// (`xmi:type` or `xsi:type`, matched by namespace URI and local name),
    /// under a role that admits `kind`, as an element of `kind`. It keeps the
    /// class as preserved content of the element, which
    /// [`SacmElement::subclass`](crate::model::SacmElement::subclass) gives,
    /// with a SACM-XMI-003 Warning: a tolerant write writes it back as the
    /// element's `xmi:type`, and a strict write refuses the element. A
    /// package root keeps a type naming a declared subclass of its own class
    /// the same way; its class still comes from its element name. A
    /// declaration never applies to a proxy, nor to an element that carries a
    /// type in both XMI namespaces, nor to a type in the root's SACM
    /// namespaces or in SACM 2.3's, and an empty `namespace` matches no type.
    /// A strict read ignores every declaration. Ask for a tolerant read with
    /// [`with_mode`](Self::with_mode). A later declaration of the same class
    /// replaces this one.
    ///
    /// SACM 2.3's own extension mechanism is the `TaggedValue` (§8.12); a
    /// file typed with foreign classes is not SACM 2.3 XMI (XMI 2.5.1
    /// §7.8.6, Annex B.2), so reading one is a tolerance the application asks
    /// for.
    ///
    /// ```
    /// use sacm::model::ElementKind;
    /// use sacm::xmi::{self, Mode, ReadOptions};
    ///
    /// let input = br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
    ///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmlns:p="urn:example:profile"
    ///     xmi:id="arguments">
    ///   <argumentElement xmi:type="p:Proposition" xmi:id="c1"/>
    /// </sacm:ArgumentPackage>"#;
    /// let options = ReadOptions::default()
    ///     .with_mode(Mode::Tolerant)
    ///     .with_subclass("urn:example:profile", "Proposition", ElementKind::Claim);
    /// let output = xmi::read(input, &options)?;
    /// let claim = output.document().get("c1").expect("c1 is read");
    /// assert_eq!(claim.kind(), ElementKind::Claim);
    /// assert_eq!(claim.subclass().map(|class| class.name()), Some("Proposition"));
    /// # Ok::<(), xmi::ReadError>(())
    /// ```
    #[must_use]
    pub fn with_subclass(
        mut self,
        namespace: impl Into<String>,
        name: impl Into<String>,
        kind: ElementKind,
    ) -> Self {
        self.subclasses
            .entry(namespace.into())
            .or_default()
            .insert(name.into(), kind);
        self
    }

    /// The SACM 2.3 class that the class `name` of the namespace `namespace`
    /// is declared a subclass of, if it is
    /// ([`with_subclass`](Self::with_subclass)).
    ///
    /// ```
    /// use sacm::model::ElementKind;
    /// use sacm::xmi::ReadOptions;
    ///
    /// let options = ReadOptions::default()
    ///     .with_subclass("urn:example:profile", "Step", ElementKind::AssertedInference);
    /// assert_eq!(
    ///     options.subclass("urn:example:profile", "Step"),
    ///     Some(ElementKind::AssertedInference)
    /// );
    /// assert_eq!(options.subclass("urn:example:profile", "Proposition"), None);
    /// ```
    #[must_use]
    pub fn subclass(&self, namespace: &str, name: &str) -> Option<ElementKind> {
        self.subclasses.get(namespace)?.get(name).copied()
    }
}

/// What [`read`] returns for a document it reads: the document, the
/// diagnostics found while reading it, and the mode it was read in.
///
/// The document holds all of the input's content: `read` keeps what it does
/// not read as preserved content, of an element or of the document, or, in
/// strict mode, refuses it, rather than drop it. The diagnostics are sorted
/// by [`Diagnostic`]'s order, located ones first. In this release they are
/// SACM-XMI-003 for a construct that does not conform, SACM-XMI-004 for an
/// element without an `xmi:id`, SACM-ID-004 for an `xmi:id` read trimmed,
/// SACM-XMI-010 for an `xsi:type`, SACM-XMI-011 for an `xmi:version` and
/// SACM-XMI-013 for a name formal/23-05-08 and ptc/22-03-13 spell
/// differently (an Error in strict mode, a Warning in tolerant mode); and,
/// in tolerant mode only, SACM-XMI-012 for a C++ shape and, for what a
/// strict read refuses, SACM-ENUM-001 for a value that is no literal,
/// SACM-MULT-001 for a repeat, SACM-XMI-005 for an untyped child,
/// SACM-ID-002 for an empty `xmi:id`, and
/// SACM-XMI-008 and SACM-XMI-002 for a root in an older SACM revision's or a
/// SACM-like namespace (LIB-COMPAT-004), and SACM-XMI-015 and SACM-XMI-017
/// for the XMI namespace and the fragment paths EMF writes (LIB-COMPAT-002),
/// each a Warning (see [`read`]).
///
/// # Examples
///
/// ```
/// use sacm::xmi::{self, ReadOptions};
///
/// let input = br#"<sacm:AssuranceCasePackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="case"/>"#;
/// let output = xmi::read(input, &ReadOptions::default())?;
/// assert!(output.diagnostics().is_empty());
/// let document = output.into_document();
/// assert_eq!(document.element_count(), 1);
/// # Ok::<(), xmi::ReadError>(())
/// ```
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct ReadOutput {
    /// The document read.
    document: Document,
    /// The diagnostics, sorted by [`Diagnostic`]'s order.
    diagnostics: Vec<Diagnostic>,
    /// The mode the document was read in.
    mode: Mode,
}

impl ReadOutput {
    /// The document read.
    #[must_use]
    pub fn document(&self) -> &Document {
        &self.document
    }

    /// The diagnostics found while reading, sorted by [`Diagnostic`]'s order.
    /// Empty for an input with no `xsi:type`, no `xmi:version`, no C++
    /// shape, no content that is not SACM 2.3 and nothing a strict read
    /// refuses, in which every element has an `xmi:id` and no `xmi:id`, of an
    /// element or a `name`, has white space around it.
    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Whether any of the read's [`diagnostics`](Self::diagnostics) is an
    /// Error: the same as
    /// [`diagnostic::has_errors`](crate::diagnostic::has_errors)`(self.diagnostics())`.
    ///
    /// Only a strict read reports Errors: in this release, SACM-XMI-010 for an
    /// `xsi:type` and SACM-XMI-003 for content that does not conform where it
    /// stands. A tolerant read reports what a strict read refuses as Warnings,
    /// so for it this is always `false`. A read without Errors is no
    /// conformance check on its own: run
    /// [`validation::validate`](crate::validation::validate) on the document
    /// and test its findings with
    /// [`diagnostic::has_errors`](crate::diagnostic::has_errors).
    ///
    /// # Examples
    ///
    /// ```
    /// use sacm::xmi::{self, Mode, ReadOptions};
    ///
    /// // A Claim typed with xsi:type rather than xmi:type.
    /// let input = br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
    ///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001"
    ///     xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmi:id="arguments">
    ///   <argumentElement xsi:type="sacm:Claim" xmi:id="c1"/>
    /// </sacm:ArgumentPackage>"#;
    ///
    /// let strict = xmi::read(input, &ReadOptions::default().with_mode(Mode::Strict))?;
    /// assert!(strict.has_errors()); // SACM-XMI-010, an Error
    ///
    /// let tolerant = xmi::read(input, &ReadOptions::default().with_mode(Mode::Tolerant))?;
    /// assert!(!tolerant.has_errors()); // the same finding, as a Warning
    /// assert_eq!(tolerant.diagnostics().len(), 1);
    /// # Ok::<(), xmi::ReadError>(())
    /// ```
    #[must_use]
    pub fn has_errors(&self) -> bool {
        crate::diagnostic::has_errors(&self.diagnostics)
    }

    /// The mode the document was read in: the mode of the [`ReadOptions`]
    /// given to [`read`].
    #[must_use]
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// The document read, without the diagnostics.
    #[must_use]
    pub fn into_document(self) -> Document {
        self.document
    }

    /// The document read and its diagnostics, to keep both without a clone.
    ///
    /// ```
    /// use sacm::xmi::{self, ReadOptions};
    ///
    /// let input = br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
    ///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id=" arguments "/>"#;
    /// let (document, diagnostics) = xmi::read(input, &ReadOptions::default())?.into_parts();
    /// assert!(document.contains("arguments"));
    /// assert_eq!(diagnostics[0].code().as_str(), "SACM-ID-004"); // read trimmed
    /// # Ok::<(), xmi::ReadError>(())
    /// ```
    #[must_use]
    pub fn into_parts(self) -> (Document, Vec<Diagnostic>) {
        (self.document, self.diagnostics)
    }
}

// A document read on one thread can be used on another.
const _: () = {
    const fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<ReadOutput>();
};

/// Why [`read`] refused its input.
///
/// The refusals of a defect in the input have their own variant and catalog
/// code: [`Doctype`](Self::Doctype), [`Malformed`](Self::Malformed),
/// [`InvalidRoot`](Self::InvalidRoot), [`NoPackage`](Self::NoPackage),
/// [`DuplicateId`](Self::DuplicateId), [`InvalidId`](Self::InvalidId),
/// [`RepeatedValue`](Self::RepeatedValue),
/// [`InvalidLiteral`](Self::InvalidLiteral),
/// [`LegacyShape`](Self::LegacyShape) and
/// [`MissingType`](Self::MissingType). [`Unsupported`](Self::Unsupported)
/// is well-formed input this version does not read; its reason says why
/// with a label (see [`read`]'s refusals), and its code is the one a
/// tolerant read gives the same input (LIB-API-017).
///
/// Every refusal has a catalog [`code`](Self::code), a message
/// [`template`](Self::template) and the [`arguments`](Self::arguments) its
/// message is made from, so that an application can word it in any language
/// (ADR 0030 amendment 3). The catalog lists each template's id, arguments
/// and English. With the `serde` feature a refusal serializes as
/// `{"type", "code", "template", "arguments", …its fields}` and reads back
/// as a record: its `Display` renders its template and arguments, and its
/// other fields are kept as given, unchecked against them, so what the
/// fields promise (such as `reason` ending with its label, or `locations`
/// holding at least two) holds for a refusal of [`read`], not for a record.
///
/// The first five, `LimitExceeded`, and `Unsupported` for an encoding, the
/// parser's capacity, a SACM 2.4 root or an `XMI` element in neither XMI
/// 2.5.1's namespace nor `http://www.omg.org/XMI`,
/// are refused in both modes. The rest
/// are refused in strict mode only: a tolerant read keeps the construct as
/// preserved content with a Warning of the variant's code, or, for
/// `Unsupported`, of SACM-XMI-003 (ADR 0035
/// D1; see [Modes](self#modes)). [`LegacyShape`](Self::LegacyShape) is
/// strict only too: a tolerant read reads the shape.
///
/// Every refusal has a [`SourceLocation`] except the parser's capacity-limit
/// refusals, and [`code`](Self::code) gives its catalog code;
/// [`locations`](Self::locations) gives every location a refusal names. Lines end as XML 1.0 §2.11
/// ends them (a line feed, a carriage return and line feed, or a lone
/// carriage return), and a leading byte order mark, UTF-8 or UTF-16, takes
/// no column. Columns count characters, in UTF-16 input too.
///
/// # Display
///
/// `Display` writes one line that starts in lower case, without trailing
/// punctuation. The location follows as ` at line L, column C` when it is
/// known, and the reason after `: `; a [`DuplicateId`](Self::DuplicateId)
/// lists every location, separated by `; `, and a
/// [`RepeatedValue`](Self::RepeatedValue) its first two, so the line stays
/// short however many there are. Names from the input are quoted with
/// `{:?}`, so line breaks in them never break the line. For example:
///
/// ```
/// use sacm::xmi::{self, ReadOptions};
///
/// let error = xmi::read(b"<r></s>", &ReadOptions::default()).unwrap_err();
/// assert_eq!(
///     error.to_string(),
///     "the input is not well-formed XML at line 1, column 4: the end tag \"s\" does not match the start tag \"r\""
/// );
/// ```
///
/// # Templates and arguments
///
/// ```
/// use sacm::diagnostic::{ArgumentValue, Code};
/// use sacm::xmi::{self, ReadOptions};
///
/// let error = xmi::read(b"<r></s>", &ReadOptions::default()).unwrap_err();
/// assert_eq!((error.code(), error.template()), (Code::XML_MALFORMED, "SACM-XML-001/end-tag-mismatch"));
/// assert_eq!(error.arguments()[0].value(), &ArgumentValue::Text("s".into()));
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ReadError {
    /// The input has a DOCTYPE, which may declare entities (SACM-SEC-001).
    /// The location is the `<` of `<!DOCTYPE`.
    #[non_exhaustive]
    Doctype {
        /// Where the DOCTYPE starts.
        location: Option<SourceLocation>,
    },
    /// The input is not well-formed XML, is truncated, is not valid UTF-8 or
    /// UTF-16, declares ASCII but holds a non-ASCII character, has a UTF-16
    /// byte order mark but declares another encoding, or has a UTF-8 byte
    /// order mark but declares ISO-8859-1 (SACM-XML-001).
    #[non_exhaustive]
    Malformed {
        /// Where the input stops being well-formed.
        location: Option<SourceLocation>,
        /// What is wrong, without a position.
        reason: String,
        /// The template and arguments of the refusal's message: see
        /// [`template`](Self::template) and [`arguments`](Self::arguments).
        details: Details,
    },
    /// An interchange root (the document element, or each element child
    /// outside the XMI namespace of an `xmi:XMI` element in the XMI 2.5.1
    /// namespace `http://www.omg.org/spec/XMI/20131001`) is not named as one
    /// of the twelve SACM package kinds, or is in no namespace or in one whose
    /// URI does not contain `sacm` in any letter case (SACM-XMI-001; ADR 0041
    /// D4). The location is the `<` of its start tag.
    #[non_exhaustive]
    InvalidRoot {
        /// Where the root's start tag starts.
        location: Option<SourceLocation>,
        /// The root's name as written: `prefix:Local`, or `Local` without a
        /// prefix.
        element_name: String,
        /// The template and arguments of the refusal's message: see
        /// [`template`](Self::template) and [`arguments`](Self::arguments).
        details: Details,
    },
    /// The `xmi:XMI` element holds no root: no element child outside the XMI
    /// namespace (SACM-XMI-001). The location is the `<` of its start tag.
    #[non_exhaustive]
    NoPackage {
        /// Where the `xmi:XMI` start tag starts.
        location: Option<SourceLocation>,
    },
    /// Two or more elements carry the same `xmi:id`, outside a Difference's
    /// `target` (SACM-ID-001, SACM23-VAL-001): two elements, two names, or an
    /// element and a name
    /// (ADR 0024). XMI 2.5.1 §7.6.1 requires the value to be unique across
    /// all elements within the file, and a [`Document`] holds each id once,
    /// so the input is refused rather than read with an element dropped or
    /// renamed. Ids compare by their `xsd:ID` value: XML white space around
    /// an `NCName` is trimmed first, so `" c1 "` and `c1` repeat, and
    /// otherwise as exact strings, so `c1` and `C1` differ. A generated id
    /// never repeats one.
    ///
    /// The refusal concerns the first id that repeats in document order.
    /// [`location`](Self::location) is that first repeat, and
    /// [`locations`](Self::locations) every element carrying the id.
    ///
    /// ```
    /// use sacm::diagnostic::Code;
    /// use sacm::xmi::{self, ReadError, ReadOptions};
    ///
    /// let input = br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
    ///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="arguments">
    ///   <argumentElement xmi:type="sacm:Claim" xmi:id="c1"/>
    ///   <argumentElement xmi:type="sacm:Claim" xmi:id="c1"/>
    /// </sacm:ArgumentPackage>"#;
    /// let error = xmi::read(input, &ReadOptions::default()).unwrap_err();
    /// assert!(matches!(&error, ReadError::DuplicateId { id, .. } if id.as_str() == "c1"));
    /// assert_eq!(error.code(), Code::ID_DUPLICATE);
    /// assert_eq!(error.location().map(|at| (at.line(), at.column())), Some((4, 3)));
    /// assert_eq!(error.locations().len(), 2);
    /// assert_eq!(
    ///     error.to_string(),
    ///     "the xmi:id \"c1\" is carried by 2 elements, at line 3, column 3; line 4, column 3"
    /// );
    /// ```
    #[non_exhaustive]
    DuplicateId {
        /// The id that repeats.
        id: ElementId,
        /// The `<` of the start tag of every element in the input, a `name`
        /// included, whose `xmi:id` has the value, in document order: at
        /// least two.
        locations: Vec<SourceLocation>,
        /// The template and arguments of the refusal's message: see
        /// [`template`](Self::template) and [`arguments`](Self::arguments).
        details: Details,
    },
    /// In strict mode, the `xmi:id` of an element or of a `name` is empty,
    /// which is not an XML `NCName` and which a [`Document`] cannot hold
    /// (SACM-ID-002, SACM23-VAL-020). The location is the `<` of the
    /// element's start tag. A tolerant read gives such an element a
    /// generated id, and such a `name` none, with SACM-ID-002 as a Warning.
    /// An `NCName` with XML white space around it, such as `" c1 "`, is read
    /// trimmed (SACM23-XMI-025). Other ids that are not `NCName`s are read as
    /// written, and [`validation::validate`](crate::validation::validate)
    /// reports them.
    #[non_exhaustive]
    InvalidId {
        /// The id as written: in this release, always empty.
        id: String,
        /// Where the element's start tag starts.
        location: Option<SourceLocation>,
        /// The template and arguments of the refusal's message: see
        /// [`template`](Self::template) and [`arguments`](Self::arguments).
        details: Details,
    },
    /// Well-formed input this version does not read, which a strict read
    /// refuses rather than keep as preserved content (LIB-API-011, ADR
    /// 0033). A tolerant read refuses only an
    /// encoding, the parser's capacity, a SACM 2.4 root and an `XMI` element
    /// in neither XMI 2.5.1's namespace nor `http://www.omg.org/XMI`, reads a
    /// root in an older SACM revision's or a SACM-like namespace
    /// (LIB-COMPAT-004) and the XMI names EMF writes in
    /// `http://www.omg.org/XMI` (LIB-COMPAT-002), and keeps the rest with
    /// SACM-XMI-003 (ADR 0035 D1, D2).
    ///
    /// Its [`code`](ReadError::code) is the code a tolerant read gives the
    /// same input (LIB-API-017): SACM-XMI-003
    /// for a defect, SACM-XMI-008 and SACM-XMI-002 for a root in an older
    /// SACM revision's or a SACM-like namespace, which a tolerant read reads
    /// with a Warning of that code, and SACM-XMI-015 for a name in EMF's
    /// XMI namespace, which a tolerant read reads with that code; and, for
    /// what both modes refuse, SACM-XMI-008 for a SACM 2.4 root,
    /// SACM-XMI-015 for an `XMI` element in another namespace, SACM-XML-002
    /// for an encoding and SACM-SEC-002 for a parser limit.
    ///
    /// The reason ends with exactly one label: `(a limit of this version;
    /// ROW reads it)` for conforming input the matrix row ROW is to read,
    /// `(a limit of this version)` for a limit such as an encoding, `(strict
    /// mode never reads it; ROW reads it in tolerant mode)` for input strict
    /// mode never reads and that a tolerant read reads under ROW, which need
    /// not be defective (such as a document in another SACM namespace,
    /// D-NS-PIN), and `(a defect in the input)` for input that does not
    /// conform and that a strict read refuses, such as an attribute on the
    /// `xmi:XMI` element. Content that does not conform where it stands is
    /// otherwise never refused: [`read`] preserves it.
    ///
    /// ```
    /// use sacm::diagnostic::Code;
    /// use sacm::xmi::{self, Mode, ReadError, ReadOptions};
    ///
    /// let input = br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/2.4"
    ///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="a"/>"#;
    /// let strict = ReadOptions::default().with_mode(Mode::Strict);
    /// let error = xmi::read(input, &strict).unwrap_err();
    /// assert!(matches!(
    ///     &error,
    ///     ReadError::Unsupported { reason, .. } if reason.ends_with("(a limit of this version)")
    /// ));
    /// assert_eq!(error.code(), Code::XMI_OTHER_REVISION);
    /// assert_eq!(error.template(), "SACM-XMI-008/sacm-2-4");
    ///
    /// // A tolerant read refuses a SACM 2.4 root too (LIB-COMPAT-018).
    /// let tolerant = xmi::read(input, &ReadOptions::default());
    /// assert!(matches!(tolerant, Err(ReadError::Unsupported { .. })));
    /// # Ok::<(), ReadError>(())
    /// ```
    #[non_exhaustive]
    Unsupported {
        /// Where the content that is not read starts.
        location: Option<SourceLocation>,
        /// What is not read, and why, ending with its label, without a
        /// position.
        reason: String,
        /// The template and arguments of the refusal's message: see
        /// [`template`](Self::template) and [`arguments`](Self::arguments).
        details: Details,
    },
    /// A property is given more than XMI allows (SACM-MULT-001): a
    /// single-valued one more than once, as an attribute and an element or as
    /// two or more elements, or a multi-valued reference end both as an
    /// attribute and as elements.
    ///
    /// XMI 2.5.1 §9.5.2 rule 2h serializes a property "either as an XML
    /// attribute or as XML elements but not both", and a single-valued
    /// property holds one value, so a strict read refuses the input even when
    /// the values are equal, rather than read it with one of them dropped. A
    /// tolerant read reads the first and keeps each later occurrence as
    /// preserved content, with SACM-MULT-001 as a Warning at it. Fix: give
    /// the value once.
    ///
    /// A multi-valued reference end, `metaClaim`, `source`,
    /// `referencedArtifactElement`, an `ArgumentGroup`'s `argumentElement` or
    /// an `ArtifactGroup`'s `artifactElement` (ADR 0027, ADR 0028, ADR 0034),
    /// takes any number of ids, in one attribute or in one element each,
    /// but not both: an attribute and an element for one is refused too. So
    /// is an `ArtifactAssetRelationship`'s `source` or `target` (§12.14,
    /// SACM23-ART-010): `Display` calls a property single-valued only when
    /// the end of that name that an element of the variant's `kind` has is,
    /// so an `AssertedRelationship`'s `target` is single-valued and an
    /// `ArtifactAssetRelationship`'s is not.
    /// Fix: give every id in the attribute, or each in its own element.
    ///
    /// A `ModelElement`'s `name` (`name:LangString[1]`, formal/23-05-08 §8.6)
    /// is such a property too: two or more `name` elements in no namespace,
    /// or the C++ `name` attribute and a `name` element (LIB-COMPAT-008),
    /// are refused, even when they are equal (SACM23-XMI-021). So are a
    /// `UtilityElement`'s `content` (§8.7 `[0..1]`) and a `TaggedValue`'s `key`
    /// (§8.12 `[1]`), which are never merged.
    ///
    /// [`location`](Self::location) is the second occurrence, and
    /// [`locations`](Self::locations) every occurrence. `Display` names the
    /// count and the first two: `at {first}; {second}` for two occurrences,
    /// and `first at {first}; {second}` for more.
    ///
    /// ```
    /// use sacm::diagnostic::Code;
    /// use sacm::xmi::{self, Mode, ReadError, ReadOptions};
    ///
    /// let strict = ReadOptions::default().with_mode(Mode::Strict);
    ///
    /// let input = br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
    ///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="arguments">
    ///   <name content="Arguments">
    ///     <content>Arguments</content>
    ///   </name>
    /// </sacm:ArgumentPackage>"#;
    /// let error = xmi::read(input, &strict).unwrap_err();
    /// assert!(matches!(
    ///     &error,
    ///     ReadError::RepeatedValue { property, kind: sacm::model::ElementKind::ArgumentPackage, .. }
    ///         if property == "name.content"
    /// ));
    /// assert_eq!(error.code(), Code::MULTIPLICITY_VIOLATION);
    /// assert_eq!(error.location().map(|at| (at.line(), at.column())), Some((4, 5)));
    /// assert_eq!(
    ///     error.to_string(),
    ///     "the single-valued property \"name.content\" of the element \"arguments\" \
    ///      is given 2 times, at line 3, column 9; line 4, column 5"
    /// );
    ///
    /// // Sources in the attribute and in an element.
    /// let input = br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
    ///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="arguments">
    ///   <argumentElement xmi:type="sacm:AssertedInference" xmi:id="i1" source="c2 c3">
    ///     <source xmi:idref="c4"/>
    ///   </argumentElement>
    /// </sacm:ArgumentPackage>"#;
    /// let error = xmi::read(input, &strict).unwrap_err();
    /// assert_eq!(
    ///     error.to_string(),
    ///     "the property \"source\" of the element \"i1\" is given both as an attribute \
    ///      and as elements, which XMI 2.5.1 rule 2h forbids, at line 3, column 66; \
    ///      line 4, column 5"
    /// );
    ///
    /// // Three ids on the single-valued `target`: each at its own column.
    /// let input = br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
    ///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="arguments">
    ///   <argumentElement xmi:type="sacm:AssertedInference" xmi:id="i1" source="c1" target="c2 c3 c4"/>
    /// </sacm:ArgumentPackage>"#;
    /// let error = xmi::read(input, &strict).unwrap_err();
    /// assert_eq!(error.locations().len(), 3);
    /// assert_eq!(
    ///     error.to_string(),
    ///     "the single-valued property \"target\" of the element \"i1\" is given 3 times, \
    ///      first at line 3, column 86; line 3, column 89"
    /// );
    /// ```
    #[non_exhaustive]
    RepeatedValue {
        /// The element that holds the property; for `name.lang`,
        /// `content.value.lang` and the like, the element the name or the
        /// value belongs to. An element without an `xmi:id` is named by its
        /// generated id.
        element: ElementId,
        /// The kind of `element`.
        kind: ElementKind,
        /// The property: `gid`, `isCitation`, `isAbstract`, `isCounter`,
        /// `citedElement`, `abstractForm`, `metaClaim`, `reasoning`,
        /// `source`, `target` (an asserted relationship's, or an
        /// `ArtifactAssetRelationship`'s), `structure`,
        /// `referencedArtifactElement`, `argumentElement` (an
        /// `ArgumentGroup`'s end), `artifactElement` (an `ArtifactGroup`'s
        /// end), `category`, `element`, `origin`, `terminologyElement` (a
        /// `TerminologyGroup`'s end),
        /// `name`, `name.lang`, `name.content`, `content`,
        /// `content.value.lang`, `content.value.content`, `key`,
        /// `key.value.lang`, `key.value.content`, `assertionDeclaration`,
        /// `version`, `date`, `startTime`, `endTime`, `value`,
        /// `externalReference`, `location`, `location.value.lang` or
        /// `location.value.content`.
        property: String,
        /// Every occurrence, in document order: an attribute at the first
        /// character of its name, an element at its `<`. At least two. Two
        /// or more ids in one reference attribute give one location per id,
        /// each at the id's first character as written in the value (past
        /// any white space or character reference before it).
        locations: Vec<SourceLocation>,
        /// The template and arguments of the refusal's message: see
        /// [`template`](Self::template) and [`arguments`](Self::arguments).
        details: Details,
    },
    /// A value is not one of its type's literals (SACM-ENUM-001). In this
    /// release, either:
    ///
    /// - an `assertionDeclaration` that is not exactly one of the six
    ///   [`AssertionDeclaration`](crate::model::AssertionDeclaration)
    ///   literals, `asserted`, `needsSupport`, `assumed`, `axiomatic`,
    ///   `defeated` and `asCited` (formal/23-05-08 §11.8; SACM23-VAL-006).
    ///   It is never read as `asserted`, because that would silently change
    ///   what the case declares (ADR 0021 amendment 2). Fix: write the
    ///   literal exactly, letter case included, with no white space around
    ///   it.
    /// - an `isCitation` or `isAbstract` (§8.2), or an `isCounter` (§11.13), that is not an
    ///   `xsd:boolean`: `true`, `false`, `1` or `0`, with XML white space
    ///   around it allowed (SACM23-BASE-007, SACM23-BASE-008, SACM23-ARG-007). `TRUE`, `yes`
    ///   and the empty value are refused, never read as false (ADR 0026).
    ///   Fix: write `true` or `false`.
    ///
    /// - the `xsi:nil` of a value element (SACM23-XMI-026), which is an
    ///   `xsd:boolean` as the flags are. Fix: write `true` or `false`.
    ///
    /// It is refused in strict mode; a tolerant read keeps the attribute or
    /// value element as preserved content, with SACM-ENUM-001 as a Warning,
    /// and holds the default (ADR 0035). `Display` gives the literals the
    /// property accepts as its reason.
    ///
    /// ```
    /// use sacm::diagnostic::Code;
    /// use sacm::xmi::{self, Mode, ReadError, ReadOptions};
    ///
    /// let strict = ReadOptions::default().with_mode(Mode::Strict);
    ///
    /// let input = br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
    ///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="arguments">
    ///   <argumentElement xmi:type="sacm:Claim" xmi:id="c1" assertionDeclaration="Assumed"/>
    /// </sacm:ArgumentPackage>"#;
    /// let error = xmi::read(input, &strict).unwrap_err();
    /// assert!(matches!(&error, ReadError::InvalidLiteral { value, .. } if value == "Assumed"));
    /// assert_eq!(error.code(), Code::ENUM_INVALID_LITERAL);
    /// assert_eq!(
    ///     error.to_string(),
    ///     "the value \"Assumed\" of the property \"assertionDeclaration\" of the element \"c1\" \
    ///      is not a literal at line 3, column 54: the literals are asserted, needsSupport, \
    ///      assumed, axiomatic, defeated and asCited"
    /// );
    ///
    /// let flag = br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
    ///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="arguments" isAbstract="yes"/>"#;
    /// let error = xmi::read(flag, &strict).unwrap_err();
    /// assert_eq!(
    ///     error.to_string(),
    ///     "the value \"yes\" of the property \"isAbstract\" of the element \"arguments\" \
    ///      is not a literal at line 2, column 73: the literals are true, false, 1 and 0"
    /// );
    /// ```
    #[non_exhaustive]
    InvalidLiteral {
        /// The element that holds the property: its `xmi:id`, or the id
        /// generated for it when it has none.
        element: ElementId,
        /// The property: in this release, `assertionDeclaration`,
        /// `isCitation`, `isAbstract`, `isCounter` or `xsi:nil`.
        property: String,
        /// The value as read: for an attribute, after XML attribute-value
        /// normalisation (XML 1.0 §3.3.3); for a value element, its exact
        /// character content.
        value: String,
        /// The first character of the attribute's name, or the `<` of the
        /// value element's start tag.
        location: Option<SourceLocation>,
        /// The template and arguments of the refusal's message: see
        /// [`template`](Self::template) and [`arguments`](Self::arguments).
        details: Details,
    },
    /// A strict read met a name or a text written in a shape of the
    /// assurance-forge C++ library (SACM-XMI-012, LIB-COMPAT-008), such as
    /// a `name="…"` attribute or text in a `name`. A tolerant read reads
    /// each such shape as its standard form, with a Warning; a strict read
    /// never applies a tolerance (ADR 0022 amendment 1), so it refuses the
    /// first one, once the rest of the input has read without a refusal.
    /// See [`read`] for the shapes. Fix: read the file in tolerant mode and
    /// write it again, which writes the standard form.
    ///
    /// ```
    /// use sacm::diagnostic::Code;
    /// use sacm::xmi::{self, Mode, ReadError, ReadOptions};
    ///
    /// let input = br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
    ///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="arguments" name="Arguments"/>"#;
    /// let strict = ReadOptions::default().with_mode(Mode::Strict);
    /// let error = xmi::read(input, &strict).unwrap_err();
    /// assert!(matches!(&error, ReadError::LegacyShape { element, .. } if element.as_str() == "arguments"));
    /// assert_eq!(error.code(), Code::XMI_LEGACY_SHAPE);
    /// assert_eq!(error.location().map(|at| (at.line(), at.column())), Some((2, 73)));
    ///
    /// // A tolerant read reads it as `<name content="Arguments"/>`, with a Warning.
    /// let output = xmi::read(input, &ReadOptions::default().with_mode(Mode::Tolerant))?;
    /// let name = output.document().get("arguments").and_then(|root| root.name());
    /// assert_eq!(name.and_then(|name| name.content()), Some("Arguments"));
    /// assert_eq!(output.diagnostics()[0].code(), Code::XMI_LEGACY_SHAPE);
    /// # Ok::<(), ReadError>(())
    /// ```
    #[non_exhaustive]
    LegacyShape {
        /// The element whose name or text the shape gives: its `xmi:id`,
        /// or the id generated for it.
        element: ElementId,
        /// What is written, without a position, such as `the name of the
        /// ArgumentPackage "arguments" is given as the attribute "name"`.
        shape: String,
        /// Where the shape is: its first attribute's name, else its first
        /// text character that is not literal white space (a CDATA section
        /// or reference at its start), else, for a `value` in a `name`, the
        /// `<` of the `value`.
        location: Option<SourceLocation>,
        /// The template and arguments of the refusal's message: see
        /// [`template`](Self::template) and [`arguments`](Self::arguments).
        details: Details,
    },
    /// A child under a containment role whose declared type is abstract
    /// carries no `xmi:type` or `xsi:type` (SACM-XMI-005, LIB-COMPAT-010),
    /// such as an `<argumentElement>` with none in an `ArgumentPackage`. Its
    /// element name is its role, so its class cannot be inferred (XMI 2.5.1
    /// §7.6.3), and none is guessed. Refused in strict mode; a tolerant read
    /// keeps the child whole as preserved content of its parent, with
    /// SACM-XMI-005 as a Warning (ADR 0035). Fix: give it an
    /// `xmi:type` naming its class, such as `xmi:type="sacm:Claim"`. A
    /// child with an `xmi:idref` or an unqualified `href` is not this
    /// refusal: it is the reference form of a composite, which this release
    /// refuses as [`Unsupported`](Self::Unsupported). A child with only an
    /// `xlink:href` or an `xmi:href` is rightly this refusal, because only an
    /// unqualified `href` is a reference form (XMI.xsd `LinkAttribs`).
    ///
    /// ```
    /// use sacm::diagnostic::Code;
    /// use sacm::xmi::{self, Mode, ReadError, ReadOptions};
    ///
    /// let input = br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
    ///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="arguments">
    ///   <argumentElement xmi:id="c1"/>
    /// </sacm:ArgumentPackage>"#;
    /// let error = xmi::read(input, &ReadOptions::default().with_mode(Mode::Strict)).unwrap_err();
    /// assert!(matches!(&error, ReadError::MissingType { role, .. } if role == "argumentElement"));
    /// assert_eq!(error.code(), Code::XMI_MISSING_TYPE);
    /// assert_eq!(
    ///     error.to_string(),
    ///     "the element \"argumentElement\" has no xmi:type, and its role has the abstract \
    ///      type ArgumentationElement at line 3, column 3: give it an xmi:type that names \
    ///      its class"
    /// );
    /// ```
    #[non_exhaustive]
    MissingType {
        /// The role: the element's local name.
        role: String,
        /// The role's declared type, an abstract class, such as
        /// `ArgumentationElement`.
        declared_type: String,
        /// The `<` of the element's start tag.
        location: Option<SourceLocation>,
        /// The template and arguments of the refusal's message: see
        /// [`template`](Self::template) and [`arguments`](Self::arguments).
        details: Details,
    },
    /// The input passes one of the [`Limits`] of
    /// [`ReadOptions::with_limits`] (SACM-SEC-003, LIB-SEC-003, LIB-SEC-007),
    /// in either mode. Fix: if the file is trusted, read it with higher limits.
    ///
    /// ```
    /// use sacm::diagnostic::Code;
    /// use sacm::xmi::{self, Limit, Limits, ReadError, ReadOptions};
    ///
    /// let bytes = br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
    ///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="p"/>"#;
    /// let limits = Limits::default().with_max_input_bytes(16 << 20).with_max_depth(64);
    /// let options = ReadOptions::default().with_limits(limits);
    /// match xmi::read(bytes, &options) {
    ///     Ok(output) => assert_eq!(output.document().element_count(), 1),
    ///     Err(ReadError::LimitExceeded { limit: Limit::InputBytes, max, .. }) => {
    ///         panic!("refused: larger than {max} bytes")
    ///     }
    ///     Err(error) => panic!("{} {error}", error.code()),
    /// }
    ///
    /// let small = ReadOptions::default().with_limits(Limits::default().with_max_input_bytes(8));
    /// let error = xmi::read(bytes, &small).unwrap_err();
    /// assert_eq!(error.code(), Code::LIMIT_EXCEEDED);
    /// assert_eq!(error.template(), "SACM-SEC-003/input-bytes");
    /// assert!(error.location().is_none());
    /// ```
    #[non_exhaustive]
    LimitExceeded {
        /// Which limit.
        limit: Limit,
        /// The limit's value, as set.
        max: u64,
        /// The `<` of the element whose depth, bindings in scope or namespace
        /// copies pass the limit, or of the preserved element or the name of
        /// the preserved attribute whose kept namespace text passes it, or
        /// where the diagnostic whose text passes it is located; `None` for
        /// the input's size and the node count.
        location: Option<SourceLocation>,
        /// The template and arguments of the refusal's message.
        details: Details,
    },
}

impl ReadError {
    /// The catalog code of the refusal: SACM-SEC-001 for
    /// [`Doctype`](Self::Doctype), SACM-XML-001 for
    /// [`Malformed`](Self::Malformed), SACM-XMI-001 for
    /// [`InvalidRoot`](Self::InvalidRoot) and [`NoPackage`](Self::NoPackage),
    /// SACM-ID-001 for [`DuplicateId`](Self::DuplicateId), SACM-ID-002 for
    /// [`InvalidId`](Self::InvalidId), SACM-MULT-001 for
    /// [`RepeatedValue`](Self::RepeatedValue), SACM-ENUM-001 for
    /// [`InvalidLiteral`](Self::InvalidLiteral), SACM-XMI-012 for
    /// [`LegacyShape`](Self::LegacyShape), SACM-XMI-005 for
    /// [`MissingType`](Self::MissingType), SACM-SEC-003 for
    /// [`LimitExceeded`](Self::LimitExceeded), and for
    /// [`Unsupported`](Self::Unsupported) the code a tolerant read gives the
    /// same input (see the variant; LIB-API-017).
    ///
    /// ```
    /// use sacm::diagnostic::Code;
    /// use sacm::xmi::{self, ReadOptions};
    ///
    /// let latin_2 = b"<?xml version=\"1.0\" encoding=\"ISO-8859-2\"?><r/>";
    /// let error = xmi::read(latin_2, &ReadOptions::default()).unwrap_err();
    /// assert_eq!(error.code(), Code::XML_ENCODING_NOT_READ);
    /// ```
    #[must_use]
    pub fn code(&self) -> Code {
        match self.details() {
            Some(details) => details.code(),
            None => match self {
                Self::Doctype { .. } => Code::XML_DOCTYPE_REJECTED,
                _ => Code::XMI_INVALID_ROOT,
            },
        }
    }

    /// The id of the refusal's message template, `<CODE>/<form>`, such as
    /// `SACM-XMI-003/unread-attribute`: a stable name for its English sentence
    /// form, under which an application keeps its own wording (ADR 0030
    /// amendment 3). [`Doctype`](Self::Doctype) is `SACM-SEC-001/doctype`
    /// and [`NoPackage`](Self::NoPackage) `SACM-XMI-001/no-package`.
    #[must_use]
    pub fn template(&self) -> &'static str {
        match self.details() {
            Some(details) => details.template(),
            None => match self {
                Self::Doctype { .. } => template::SEC_001_DOCTYPE.id,
                _ => template::XMI_001_NO_PACKAGE.id,
            },
        }
    }

    /// The values the refusal's message is made from, in the order they
    /// first appear in it: empty for [`Doctype`](Self::Doctype) and
    /// [`NoPackage`](Self::NoPackage).
    #[must_use]
    pub fn arguments(&self) -> &[Argument] {
        self.details().map_or(&[], Details::arguments)
    }

    /// The details of the refusal's message, if its variant has them.
    fn details(&self) -> Option<&Details> {
        match self {
            Self::Doctype { .. } | Self::NoPackage { .. } => None,
            Self::Malformed { details, .. }
            | Self::InvalidRoot { details, .. }
            | Self::DuplicateId { details, .. }
            | Self::InvalidId { details, .. }
            | Self::Unsupported { details, .. }
            | Self::RepeatedValue { details, .. }
            | Self::InvalidLiteral { details, .. }
            | Self::LegacyShape { details, .. }
            | Self::MissingType { details, .. }
            | Self::LimitExceeded { details, .. } => Some(details),
        }
    }

    /// The `Malformed` refusal at `location` whose reason `details` give.
    pub(crate) fn malformed(location: Option<SourceLocation>, details: Details) -> Self {
        Self::Malformed {
            location,
            reason: details.render(),
            details,
        }
    }

    /// The `InvalidRoot` refusal of the root written `element_name`, with the
    /// message of `template`: the element, then `rest` (SACM-XMI-001
    /// `/not-a-package`, `/not-a-sacm-namespace` or `/no-namespace`).
    pub(crate) fn invalid_root(
        location: Option<SourceLocation>,
        element_name: String,
        template: &'static template::Template,
        rest: Vec<Argument>,
    ) -> Self {
        let mut arguments = vec![Argument::text("element", element_name.as_str())];
        arguments.extend(rest);
        let details = Details::new(template, arguments);
        Self::InvalidRoot {
            location,
            element_name,
            details,
        }
    }

    /// The `DuplicateId` refusal of `id`, carried at `locations`.
    pub(crate) fn duplicate_id(id: ElementId, locations: Vec<SourceLocation>) -> Self {
        let details = Details::new(
            &template::ID_001_DUPLICATE_ID,
            vec![
                Argument::text("id", id.as_str()),
                Argument::count_of("count", locations.len()),
            ],
        );
        Self::DuplicateId {
            id,
            locations,
            details,
        }
    }

    /// The `InvalidId` refusal of the id written `id`.
    pub(crate) fn invalid_id(id: String, location: Option<SourceLocation>) -> Self {
        let details = Details::new(
            &template::ID_002_READ_ID_NOT_AN_NCNAME,
            vec![Argument::text("id", id.as_str())],
        );
        Self::InvalidId {
            id,
            location,
            details,
        }
    }

    /// The `RepeatedValue` refusal of `property` of `element`, of `kind`,
    /// given at `locations`. A property is given in both forms when the end
    /// of that name that an element of `kind` has is multi-valued: `target`
    /// is an asserted relationship's single-valued end and an
    /// `ArtifactAssetRelationship`'s multi-valued one (SACM23-ART-010).
    pub(crate) fn repeated_value(
        element: ElementId,
        kind: ElementKind,
        property: String,
        locations: Vec<SourceLocation>,
    ) -> Self {
        let is_many = ReferenceEnd::ALL
            .iter()
            .any(|end| end.name() == property && end.applies_to(kind) && end.is_many());
        let mut arguments = vec![
            Argument::text("property", property.as_str()),
            Argument::text("element", element.as_str()),
        ];
        let template = if is_many {
            &template::MULT_001_BOTH_FORMS
        } else {
            arguments.push(Argument::count_of("count", locations.len()));
            &template::MULT_001_REPEATED_VALUE
        };
        Self::RepeatedValue {
            element,
            kind,
            property,
            locations,
            details: Details::new(template, arguments),
        }
    }

    /// The `InvalidLiteral` refusal of `value` of `property` of `element`.
    pub(crate) fn invalid_literal(
        element: ElementId,
        property: String,
        value: String,
        location: Option<SourceLocation>,
    ) -> Self {
        let details = Details::new(
            &template::ENUM_001_NOT_A_LITERAL,
            vec![
                Argument::text("value", value.as_str()),
                Argument::text("property", property.as_str()),
                Argument::text("element", element.as_str()),
            ],
        );
        Self::InvalidLiteral {
            element,
            property,
            value,
            location,
            details,
        }
    }

    /// The `LegacyShape` refusal of the shape `shape` gives, of `element`.
    pub(crate) fn legacy_shape(
        element: ElementId,
        shape: Details,
        location: Option<SourceLocation>,
    ) -> Self {
        Self::LegacyShape {
            element,
            shape: shape.render(),
            location,
            details: shape,
        }
    }

    /// The `MissingType` refusal of a child of the role `role`, of the
    /// abstract type `declared_type`.
    pub(crate) fn missing_type(
        role: String,
        declared_type: String,
        location: Option<SourceLocation>,
    ) -> Self {
        let details = Details::new(
            &template::XMI_005_MISSING_TYPE,
            vec![
                Argument::text("role", role.as_str()),
                Argument::text("type", declared_type.as_str()),
            ],
        );
        Self::MissingType {
            role,
            declared_type,
            location,
            details,
        }
    }

    /// Where in the input the refusal is: an element at the `<` of its start
    /// tag, an attribute at the first character of its name, text at its
    /// first character that is not literal white space (a CDATA section or
    /// reference at its start). For [`DuplicateId`](Self::DuplicateId), the
    /// first repeat; for [`RepeatedValue`](Self::RepeatedValue), the second
    /// occurrence. `None` for an input-size or node limit and for the
    /// parser's capacity.
    ///
    /// Lines end at a line feed, a carriage return and line feed, or a lone
    /// carriage return; a leading byte order mark, UTF-8 or UTF-16, takes no
    /// column.
    #[must_use]
    pub fn location(&self) -> Option<SourceLocation> {
        match self {
            Self::Doctype { location }
            | Self::Malformed { location, .. }
            | Self::InvalidRoot { location, .. }
            | Self::NoPackage { location }
            | Self::InvalidId { location, .. }
            | Self::InvalidLiteral { location, .. }
            | Self::LegacyShape { location, .. }
            | Self::MissingType { location, .. }
            | Self::LimitExceeded { location, .. }
            | Self::Unsupported { location, .. } => *location,
            // The first element carries the id, or the first occurrence gives
            // the value; the second repeats it.
            Self::DuplicateId { locations, .. } | Self::RepeatedValue { locations, .. } => {
                locations.get(1).copied()
            }
        }
    }

    /// Every location the refusal names: for
    /// [`DuplicateId`](Self::DuplicateId), the start tag of every element
    /// carrying the id, in document order; for
    /// [`RepeatedValue`](Self::RepeatedValue), every occurrence of the
    /// value, in document order; for every other refusal, its
    /// [`location`](Self::location), or nothing when it has none.
    ///
    /// ```
    /// use sacm::xmi::{self, ReadOptions};
    ///
    /// let error = xmi::read(b"<r></s>", &ReadOptions::default()).unwrap_err();
    /// assert_eq!(
    ///     error.locations().iter().map(|at| (at.line(), at.column())).collect::<Vec<_>>(),
    ///     [(1, 4)]
    /// );
    /// ```
    #[must_use]
    pub fn locations(&self) -> &[SourceLocation] {
        match self {
            Self::Doctype { location }
            | Self::Malformed { location, .. }
            | Self::InvalidRoot { location, .. }
            | Self::NoPackage { location }
            | Self::InvalidId { location, .. }
            | Self::InvalidLiteral { location, .. }
            | Self::LegacyShape { location, .. }
            | Self::MissingType { location, .. }
            | Self::LimitExceeded { location, .. }
            | Self::Unsupported { location, .. } => location.as_slice(),
            Self::DuplicateId { locations, .. } | Self::RepeatedValue { locations, .. } => {
                locations
            }
        }
    }
}

/// The reason of a [`ReadError::InvalidLiteral`] for the property
/// `property`: the literals it accepts. `assertionDeclaration`'s are its six,
/// in §11.8 order; every other property read in this release, `isCitation`,
/// `isAbstract`, `isCounter` and `xsi:nil`, is an `xsd:boolean`.
fn literals_reason(property: &str) -> &'static str {
    match property {
        "assertionDeclaration" => {
            "the literals are asserted, needsSupport, assumed, axiomatic, defeated and asCited"
        }
        _ => "the literals are true, false, 1 and 0",
    }
}

/// Writes `locations` as `, at L1; L2; …`, the tail of the `Display` of a
/// refusal that names several.
fn write_locations(f: &mut fmt::Formatter<'_>, locations: &[SourceLocation]) -> fmt::Result {
    for (index, location) in locations.iter().enumerate() {
        f.write_str(if index == 0 { ", at " } else { "; " })?;
        write!(f, "{location}")?;
    }
    Ok(())
}

impl fmt::Display for ReadError {
    /// Writes the refusal on one line, as described on [`ReadError`]: each
    /// variant's fixed frame around the variable part, which its template
    /// and arguments render.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let reason = match self {
            Self::Doctype { .. } => {
                f.write_str(&template::render(&template::SEC_001_DOCTYPE, &[]))?;
                None
            }
            Self::Malformed { details, .. } => {
                f.write_str("the input is not well-formed XML")?;
                Some(details.render())
            }
            Self::NoPackage { .. } => {
                f.write_str(&template::render(&template::XMI_001_NO_PACKAGE, &[]))?;
                None
            }
            Self::InvalidRoot { details, .. }
            | Self::InvalidId { details, .. }
            | Self::LimitExceeded { details, .. } => {
                f.write_str(&details.render())?;
                None
            }
            // Every location, not only the first repeat.
            Self::DuplicateId {
                locations, details, ..
            } => {
                f.write_str(&details.render())?;
                return write_locations(f, locations);
            }
            // The count and the first two locations, so that the line stays
            // bounded however many occurrences there are (SACM23-XMI-005).
            Self::RepeatedValue {
                locations, details, ..
            } => {
                f.write_str(&details.render())?;
                return match locations.as_slice() {
                    [first, second, _, ..] => write!(f, ", first at {first}; {second}"),
                    _ => write_locations(f, locations),
                };
            }
            Self::InvalidLiteral {
                property, details, ..
            } => {
                f.write_str(&details.render())?;
                Some(literals_reason(property).to_owned())
            }
            Self::LegacyShape { details, .. } => {
                write!(
                    f,
                    "{}, a shape of the assurance-forge C++ library",
                    details.render()
                )?;
                Some(
                    "a strict read refuses it; a tolerant read reads it as its standard form, \
                     which write writes"
                        .to_owned(),
                )
            }
            Self::MissingType { details, .. } => {
                f.write_str(&details.render())?;
                Some("give it an xmi:type that names its class".to_owned())
            }
            Self::Unsupported { details, .. } => {
                f.write_str("the reader cannot represent the input")?;
                Some(refusal::reason(details))
            }
        };
        if let Some(location) = self.location() {
            write!(f, " at {location}")?;
        }
        if let Some(reason) = reason {
            write!(f, ": {reason}")?;
        }
        Ok(())
    }
}

impl std::error::Error for ReadError {}

/// Writes a document as XMI text.
///
/// The output is deterministic: the same document gives the same bytes. It
/// is UTF-8 without a byte order mark, starts with
/// `<?xml version="1.0" encoding="UTF-8"?>`, ends lines with LF, indents two
/// spaces per depth, writes one tag per line and an element with no children
/// as `<x … />`, and ends with an LF. One root is the document element;
/// several are the children of one `xmi:XMI` element, in document order
/// (XMI 2.5.1 §7.5.3), and so is one root, or none, when the document holds
/// preserved content outside its elements (ADR 0035 D3). The document
/// element alone declares the SACM 2.3
/// namespace `http://www.omg.org/spec/SACM/20220301` as `sacm`, the XMI 2.5.1
/// namespace `http://www.omg.org/spec/XMI/20131001` as `xmi`. No element
/// carries `xmi:version`: the XMI namespace identifies the version
/// (XMI 2.5.1 §7.5.3; [ADR 0017] amendment 3), and an `xmi:version` that
/// [`read`] ignored is not written back.
///
/// [ADR 0017]: https://github.com/lasrod/libSACM/blob/main/docs/adr/0017-xmi-writer.md
///
/// A root is written as `sacm:<Kind>`, and every other element under the name
/// of the role that holds it in its parent's class, with no prefix, and with
/// its children in document order. Role names are spelled as in the
/// machine-readable model ptc/22-03-13 (`argumentElement`), which differs
/// from §11.4's text (`argumentationElement`); class names and attributes as
/// in the text where the two disagree (`sacm:ArtifactAssetRelationship`, an
/// Event's `date`; D-TEXT-MODEL), so the spellings
/// [`read`](crate::xmi::read#spellings) reads are written by their resolved
/// names. A child carries
/// `xmi:type="sacm:<Kind>"` exactly when the role's declared type is not its
/// kind: an abstract class, as for `argumentElement`, or a package role
/// holding an Interface or Binding, such as an `ArgumentPackageInterface` in
/// an AssuranceCasePackage's `argumentPackage` role. This is XMI's own type
/// attribute (XMI 2.5.1 §7.6.3, §7.8.8 and rule 2g; SACM sets no
/// `useSchemaExtensions`). `xsi:type` is never written; `read` accepts both,
/// and reports an `xsi:type` as SACM-XMI-010.
///
/// Every element then carries its id as `xmi:id`, and directly after it its
/// `gid`, if it has one: `gid=""` for an empty one, and no attribute for an
/// absent one. Then come its flags, its declaration and its references, class
/// by class, superclass first (XMI 2.5.1 Annex B.5.2; ADR 0026, ADR 0027):
///
/// 1. the `SACMElement` features (§8.2): `isCitation`, `isAbstract`,
///    `citedElement`, `abstractForm`;
/// 2. on an Assertion (§11.10): `assertionDeclaration`, `metaClaim`;
/// 3. on an asserted relationship (§11.13): `isCounter`, `reasoning`,
///    `source`, `target`;
/// 4. on an `ArgumentReasoning` (§11.12): `structure`;
/// 5. on an `ArtifactReference` (§11.9): `referencedArtifactElement`;
/// 6. on an `ArgumentGroup` (§11.2): `argumentElement`;
/// 7. on an `ArtifactGroup` (§12.3): `artifactElement`.
///
/// Then come the string-valued attributes, each only when present, exactly
/// as held and escaped: an Artifact's `version` and `date`, an Event's
/// `date`, and an Activity's `startTime` and `endTime` (ADR 0034 amendment
/// 1).
///
/// So an `AssertedInference` with everything set starts
/// `xmi:type xmi:id gid isCitation isAbstract citedElement abstractForm
/// assertionDeclaration metaClaim isCounter reasoning source target`. A flag
/// is written only when true, as `"true"` (false, the default, is not
/// written). A reference end is written only when it holds an id, as the ids
/// it holds, in order, repeats included, separated by one space and escaped,
/// held exactly, whether or not they name an element. A reference read in
/// the element form is written in this attribute form, except when the
/// element also holds, as preserved content, an attribute of the end's name
/// that held no id (such as `metaClaim=""` beside `<metaClaim>` elements):
/// then each id is written as a `<metaClaim xmi:idref="…" />` element,
/// after the `name`, `content` and `key`, so that no start tag carries an
/// attribute twice and nothing is dropped (ADR 0033). An edit that sets the
/// end replaces such an attribute. The
/// `assertionDeclaration` is written only when it is not `asserted`, the
/// default (XMI 2.5.1 §9.4.1), as
/// [`AssertionDeclaration::as_str`](crate::model::AssertionDeclaration::as_str)
/// spells it. A
/// named element's first child is its name, `<name lang="…" content="…" />`,
/// with each field written only if present, so an empty field is `=""` and an
/// absent one is no attribute; an element with no name has no `name` child.
/// A name read with an `xmi:id`
/// ([`SacmElement::name_id`](crate::model::SacmElement::name_id)) carries it
/// first: `<name xmi:id="…" lang="…" content="…" />`. An
/// `ExpressionLangString` name or value (§8.4) starts with
/// `xmi:type="sacm:ExpressionLangString"`, and its `expression`, if it has
/// one, follows `content`: `<name xmi:type="sacm:ExpressionLangString"
/// xmi:id="…" lang="…" content="…" expression="…" />`. The derived
/// `expression` end is written only there, never on the element. An id is written
/// as it is, escaped, and so are a name's fields. An id that is not an XML
/// `NCName`, such as `1 2`, is not an `xsd:ID`, so a strict write refuses it
/// (SACM-ID-002); a tolerant write writes it as it is, and the output then
/// does not validate against an XMI schema. In attribute values `&`, `<`, `>`, `"`,
/// TAB, LF and CR are escaped; everything else is written as it is.
///
/// Inside a `UtilityElement`, its `content` element is written first, then,
/// on a `TaggedValue`, its `key`, superclass first (XMI 2.5.1 Annex B.5.2).
/// A Resource's `location` is written the same way, directly after its
/// `name`. Each is its role
/// element, with its `xmi:id` first if it has one, holding one
/// `<value … />` per value, in order, written as a name is:
/// `<content>`, `<value lang="en" content="…" />`, `</content>`. An empty
/// one is `<content />`, and an absent one is not written. No type attribute
/// is written on a `UtilityElement`, a `content`, a `key` or a `location`,
/// nor on a `value` that holds a plain `LangString`.
///
/// # Preserved content
///
/// What [`read`] kept as preserved content, because it is not SACM 2.3
/// where it stands (see [Preserved content](fn@read#preserved-content)) or,
/// in tolerant mode, because a strict read refuses it, a tolerant `write`
/// writes back where it was read (LIB-COMPAT-003, ADR 0033, ADR 0035): its attributes after the element's typed attributes, in input
/// order, after the namespace declarations they need; each of its children
/// on its own line, before the typed child it preceded, after the element's
/// `name`, `content` and `key`, and before the end tag when it followed the
/// last. Text in the element is written trimmed, `&`, `<`, `>` and CR
/// escaped. A preserved element is written with its subtree on one line,
/// exactly as held: names with their prefixes, attribute values escaped as
/// above, text exactly, and `<x … />` for one that holds nothing. It
/// declares every namespace binding that was in scope where it was read,
/// other than `xml` and `sacm` and `xmi` bound as the document element binds
/// them, and each element inside declares what it declared. So a written
/// document reads back to an equal one, and writes again to the same bytes.
/// A strict write refuses a document that holds any, as
/// [`WriteError::PreservedContent`].
///
/// XMI metadata (see [XMI metadata](fn@read#xmi-metadata)) is written back
/// the same way by both modes (SACM23-XMI-019, ADR 0042): its attributes
/// after the typed ones and before the preserved ones, after the bindings
/// both need, once per prefix, sorted by prefix; its children before the
/// preserved children placed at the same index. On a part, the metadata
/// attributes follow its typed attributes, and metadata children make it a
/// start and an end tag, each child on its own line, by value in a
/// `content`, `key` or `location`. A document that holds metadata outside
/// every element is wrapped in `xmi:XMI`. A strict write checks it as
/// XMI.xsd types it: an `xmi:id` that is no `NCName` (SACM-ID-002) and an
/// `IDREF` or `IDREFS` token that names no `xmi:id` of the output
/// (SACM-REF-001) are [`WriteError::Invalid`]; what an extension holds is
/// not checked otherwise.
///
/// ```
/// use sacm::xmi::{self, Mode, ReadOptions, WriteOptions};
///
/// let input = br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmlns:v="urn:vendor" xmi:id="p">
///   <v:layout x="10"/>
///   <name content="P"/>
/// </sacm:ArgumentPackage>"#;
/// let document = xmi::read(input, &ReadOptions::default())?.into_document();
/// let text = xmi::write(&document, &WriteOptions::default().with_mode(Mode::Tolerant))?;
/// assert_eq!(
///     text,
///     "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
///      <sacm:ArgumentPackage xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\" \
///      xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" xmi:id=\"p\">\n  \
///      <name content=\"P\" />\n  \
///      <v:layout xmlns:v=\"urn:vendor\" x=\"10\" />\n\
///      </sacm:ArgumentPackage>\n"
/// );
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Modes
///
/// In [`Mode::Strict`], the default, `write` writes only output that is
/// schema-valid (SACM 2.3 clause 2; ADR 0035 D4): it refuses a document that
/// holds preserved content, which it cannot show is schema-valid, as
/// [`WriteError::PreservedContent`], and one with an `xmi:id` that is no
/// `NCName` or a reference to no `xmi:id` it writes as
/// [`WriteError::Invalid`]. It never runs
/// [`validation::validate`](crate::validation::validate): XMI schemas ignore
/// multiplicities (XMI 2.5.1 §7.8.2), so a draft whose elements have no name
/// yet, or a relationship with no target, writes; an application calls
/// `validate` for model checks. In [`Mode::Tolerant`] it refuses neither,
/// and writes the document as it is held. The two modes write the same
/// bytes for a document both write.
///
/// A strict write checks schema validity only, so a new validation rule
/// never changes what it writes. A new strict write refusal is listed in the
/// release notes as a change.
///
/// # Output size
///
/// `write` builds the whole output in memory and has no limit of its own.
/// Each line is indented two spaces per level. So, for a document that
/// [`read`] returned from B bytes under [`Limits`] with depth limit D and size
/// limit M, the output is at most (2·D + 48)·B + 6·M + 8,192 bytes. A document
/// built or changed by edits has no such bound: its output grows with its
/// elements, their text and their depth.
///
/// # Errors
///
/// The document is checked before anything is written, and the first failure
/// is returned:
///
/// 1. the document has no root element, so there is no SACM package to
///    write: [`WriteError::NoRoot`], in both modes (an `xmi:XMI` holding only
///    XMI metadata holds no package);
/// 2. in strict mode only, preserved content: a document that holds any is
///    [`WriteError::PreservedContent`], which lists each element that holds
///    some (SACM-XMI-006), and no element when only the document holds it;
/// 3. in strict mode only, the schema check: one SACM-ID-002 per `xmi:id`,
///    of an element, a `name`, a `content`, a `key`, a `location` or a `value`, that is no
///    `NCName` (XML Schema 1.0 Part 2 §3.3.8), and one SACM-REF-001 per
///    distinct id of a reference end that is no `xmi:id` the output writes
///    (Part 1 §3.3.4; XMI 2.5.1 rule 4i), in [`Diagnostic`] order, make
///    [`WriteError::Invalid`];
/// 4. then each element, in [`Document::elements`] order, with all of an
///    element's checks run before the next element's: first its id, where a
///    character outside XML 1.0's `Char` is
///    [`WriteError::UnrepresentableCharacter`], naming the attribute and its
///    first such character; then its gid, its `version`, `date`,
///    `startTime`, `endTime`, `value` and `externalReference`, its name's
///    `lang`, its name's `content`,
///    and each value of its `content`, then of its `key` and then of its
///    `location`, `lang` before `content`, the same way, naming the value's
///    position too. Preserved content is not
///    checked: [`read`] read it, so it is representable.
///
/// Nothing else a document holds is refused.
///
/// # Examples
///
/// ```
/// use sacm::model::Document;
/// use sacm::xmi::{self, Mode, ReadOptions, WriteError, WriteOptions};
///
/// // A name is written as the element's first child.
/// let input = br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="arguments">
///   <name lang="en" content="Arguments"/>
/// </sacm:ArgumentPackage>"#;
/// let document = xmi::read(input, &ReadOptions::default())?.into_document();
/// let text = xmi::write(&document, &WriteOptions::default())?;
/// assert_eq!(
///     text,
///     "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
///      <sacm:ArgumentPackage xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\" \
///      xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" xmi:id=\"arguments\">\n  \
///      <name lang=\"en\" content=\"Arguments\" />\n\
///      </sacm:ArgumentPackage>\n"
/// );
///
/// // A `needsSupport` Claim carries its declaration after its id; an
/// // `asserted` one carries none.
/// let claims = br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="arguments">
///   <name content="Arguments"/>
///   <argumentElement xmi:type="sacm:Claim" xmi:id="c1" assertionDeclaration="needsSupport">
///     <name content="C1"/>
///   </argumentElement>
///   <argumentElement xmi:type="sacm:Claim" xmi:id="c2" assertionDeclaration="asserted">
///     <name content="C2"/>
///   </argumentElement>
/// </sacm:ArgumentPackage>"#;
/// let document = xmi::read(claims, &ReadOptions::default())?.into_document();
/// let text = xmi::write(&document, &WriteOptions::default())?;
/// assert!(text.contains(
///     "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\" assertionDeclaration=\"needsSupport\">\n"
/// ));
/// assert!(text.contains("<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c2\">\n"));
///
/// // A package with no name, a draft, is schema-valid: it is written in
/// // strict mode, the default, as in tolerant mode.
/// let draft = br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="draft"/>"#;
/// let document = xmi::read(draft, &ReadOptions::default())?.into_document();
/// let text = xmi::write(&document, &WriteOptions::default())?;
/// assert!(text.ends_with("xmi:id=\"draft\" />\n"));
/// let tolerant = xmi::write(&document, &WriteOptions::default().with_mode(Mode::Tolerant))?;
/// assert_eq!(text, tolerant);
///
/// // A document with no root would be an empty `xmi:XMI`, which a SACM
/// // reader refuses, so it is refused.
/// let error = xmi::write(&Document::new(), &WriteOptions::default()).unwrap_err();
/// assert_eq!(error, WriteError::NoRoot);
/// assert_eq!(error.to_string(), "the document has no root to write");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn write(document: &Document, options: &WriteOptions) -> Result<String, WriteError> {
    output::write(document, options.mode)
}

/// How [`write`](fn@write) writes a document: in which [`Mode`], strict by
/// default.
///
/// ```
/// use sacm::xmi::{Mode, WriteOptions};
///
/// assert_eq!(WriteOptions::default().mode(), Mode::Strict);
/// // Write a document as it is held, preserved content included.
/// let drafts = WriteOptions::default().with_mode(Mode::Tolerant);
/// assert_eq!(drafts.mode(), Mode::Tolerant);
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct WriteOptions {
    /// The mode to write in.
    mode: Mode,
}

impl Default for WriteOptions {
    /// Writes in [`Mode::Strict`].
    fn default() -> Self {
        Self { mode: Mode::Strict }
    }
}

impl WriteOptions {
    /// These options, writing in `mode`.
    #[must_use]
    pub fn with_mode(mut self, mode: Mode) -> Self {
        self.mode = mode;
        self
    }

    /// The mode to write in.
    #[must_use]
    pub fn mode(&self) -> Mode {
        self.mode
    }
}

/// Why [`write`](fn@write) refused a document.
///
/// The writer writes everything a [`Document`] holds, so it refuses only a
/// document with no root, a character XML 1.0 cannot represent, and, in
/// strict mode, a document that holds preserved content or whose output
/// would not be schema-valid (ADR 0035 D4). Every refusal has a catalog
/// [`code`](Self::code): SACM-XMI-001 for [`NoRoot`](Self::NoRoot),
/// SACM-XML-003 for
/// [`UnrepresentableCharacter`](Self::UnrepresentableCharacter),
/// SACM-XMI-016 for [`Invalid`](Self::Invalid) and SACM-XMI-006 for
/// [`PreservedContent`](Self::PreservedContent) (LIB-API-017), and a
/// message [`template`](Self::template) and its
/// [`arguments`](Self::arguments), as [`ReadError`] has. With the `serde`
/// feature it reads back as a record, as a `ReadError` does: its fields are
/// kept as given, so a record's `Invalid` may hold no diagnostics.
/// [`element`](Self::element) names the element refused, where there is
/// one, and [`diagnostics`](Self::diagnostics) gives the schema errors of
/// [`Invalid`](Self::Invalid).
///
/// # Display
///
/// `Display` writes one line that starts in lower case, without trailing
/// punctuation. Ids are quoted with `{:?}`, so line breaks in them never break
/// the line. [`Invalid`](Self::Invalid) gives the number of schema errors
/// and the code and first affected id of the first one, introduced by `the
/// first is` when there are several, and
/// [`PreservedContent`](Self::PreservedContent) the one element that holds
/// preserved content, or their number and the first, or that the document
/// holds it outside its elements. For example:
///
/// ```
/// use sacm::model::Document;
/// use sacm::xmi::{self, WriteOptions};
///
/// let error = xmi::write(&Document::new(), &WriteOptions::default()).unwrap_err();
/// assert_eq!(error.to_string(), "the document has no root to write");
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum WriteError {
    /// The document has no root element, so there is no SACM package to
    /// write. Add a package as a root. This variant has no fields
    /// and will not gain any.
    NoRoot,
    /// An element's id, gid, `version`, `date`, `startTime`, `endTime`,
    /// `value` or `externalReference`, its name's `lang` or `content`, or the
    /// `lang` or `content` of a value of its `content`, `key` or `location`,
    /// holds a character that XML 1.0 cannot represent: one outside its
    /// `Char` production, such as U+0000, another C0 control other than TAB,
    /// LF and CR, U+FFFE or U+FFFF.
    ///
    /// - **Not reached by edits or reads.** No edit stores such text
    ///   ([`EditError::UnrepresentableCharacter`](crate::edit::EditError::UnrepresentableCharacter)),
    ///   and [`read`] cannot hold it. Only a document built with the
    ///   `test-support` seams reaches this refusal.
    /// - **Position.** For a value of a `content`, `key` or `location`,
    ///   `position` is the value's index and the template is
    ///   `SACM-XML-003/unrepresentable-character-in-value`; otherwise
    ///   `position` is `None` and the template is
    ///   `SACM-XML-003/unrepresentable-character`.
    #[non_exhaustive]
    UnrepresentableCharacter {
        /// The element whose id, gid, `version`, `date`, `startTime`,
        /// `endTime`, `value`, `externalReference`, name, `content`, `key` or
        /// `location` holds the character.
        element: ElementId,
        /// The first such character in the attribute.
        character: char,
        /// The attribute that holds it.
        attribute: WriteAttribute,
        /// The value's index, from 0, among the values of the content, key
        /// or location, for the six *Value* attributes; `None` otherwise.
        position: Option<usize>,
        /// The template and arguments of the refusal's message: see
        /// [`template`](Self::template) and [`arguments`](Self::arguments).
        details: Details,
    },
    /// In strict mode, the output would not be schema-valid (LIB-COMPAT-007,
    /// ADR 0035 D4): an `xmi:id`, of an element, a `name`, a `content`, a
    /// `key` or a `value`, is no `NCName`, which an `xsd:ID` must be
    /// (SACM-ID-002; XML Schema 1.0 Part 2 §3.3.8), or a reference names no
    /// `xmi:id` the output writes, which an `xsd:IDREF` must (SACM-REF-001;
    /// Part 1 §3.3.4, XMI 2.5.1 rule 4i). `diagnostics` holds every such
    /// Error, in [`Diagnostic`] order; each carries its own code and the ids
    /// of the elements it affects. It is no validation: a reference to the
    /// `xmi:id` of a `name` is written, though
    /// [`validation::validate`](crate::validation::validate) reports it. Fix
    /// the ids and references, or write the document as it is held with
    /// `WriteOptions::default().with_mode(Mode::Tolerant)`.
    ///
    /// ```
    /// use sacm::diagnostic::Code;
    /// use sacm::xmi::{self, Mode, ReadOptions, WriteError, WriteOptions};
    ///
    /// // A Claim that cites an id no element of the document has.
    /// let input = br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
    ///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="p">
    ///   <argumentElement xmi:type="sacm:Claim" xmi:id="c1" isCitation="true" citedElement="nope"/>
    /// </sacm:ArgumentPackage>"#;
    /// let document = xmi::read(input, &ReadOptions::default())?.into_document();
    /// let error = xmi::write(&document, &WriteOptions::default()).unwrap_err();
    /// assert!(matches!(error, WriteError::Invalid { .. }));
    /// assert_eq!(error.diagnostics()[0].code(), Code::REF_DANGLING);
    /// assert_eq!(
    ///     error.to_string(),
    ///     "the document cannot be written in strict mode: 1 schema error, \
    ///      SACM-REF-001 on \"c1\""
    /// );
    ///
    /// // Tolerant mode writes it as it is held.
    /// let tolerant = WriteOptions::default().with_mode(Mode::Tolerant);
    /// let text = xmi::write(&document, &tolerant)?;
    /// assert!(text.contains("citedElement=\"nope\""));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[non_exhaustive]
    Invalid {
        /// Every schema error, in [`Diagnostic`] order. Never empty.
        diagnostics: Vec<Diagnostic>,
        /// The template and arguments of the refusal's message: see
        /// [`template`](Self::template) and [`arguments`](Self::arguments).
        details: Details,
    },
    /// In strict mode, the document holds preserved content: what [`read`]
    /// kept, as written, because it does not conform where it stands
    /// (SACM-XMI-006, SACM23-XMI-008, ADR 0033, ADR 0035). The
    /// writer cannot show that content it did not read is schema-valid. It is
    /// refused before the schema check. Write the document in tolerant mode,
    /// which writes the content back, or delete the elements that hold it.
    /// Content the document holds outside its elements can be written only in
    /// tolerant mode in this release.
    ///
    /// ```
    /// use sacm::diagnostic::Code;
    /// use sacm::xmi::{self, Mode, ReadOptions, WriteError, WriteOptions};
    ///
    /// let input = br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
    ///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmlns:v="urn:vendor"
    ///     xmi:id="p" v:colour="red">
    ///   <name content="P"/>
    /// </sacm:ArgumentPackage>"#;
    /// let document = xmi::read(input, &ReadOptions::default())?.into_document();
    /// let error = xmi::write(&document, &WriteOptions::default()).unwrap_err();
    /// assert!(matches!(&error, WriteError::PreservedContent { elements, .. } if elements[0].as_str() == "p"));
    /// assert_eq!(error.code(), Code::XMI_STRICT_SAVE_REFUSED);
    /// assert_eq!(
    ///     error.to_string(),
    ///     "the document cannot be written in strict mode: the element \"p\" holds preserved \
    ///      content"
    /// );
    ///
    /// // Tolerant mode writes it back.
    /// let text = xmi::write(&document, &WriteOptions::default().with_mode(Mode::Tolerant))?;
    /// assert!(text.contains(r#"xmi:id="p" xmlns:v="urn:vendor" v:colour="red">"#));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[non_exhaustive]
    PreservedContent {
        /// Each element that holds preserved content, once, in
        /// [`Document::elements`] order. Empty only when the document alone
        /// holds it, outside its elements
        /// ([`Document::has_preserved_content`]).
        elements: Vec<ElementId>,
        /// The template and arguments of the refusal's message: see
        /// [`template`](Self::template) and [`arguments`](Self::arguments).
        details: Details,
    },
}

impl WriteError {
    /// The element refused, if the refusal names one: the element of
    /// [`UnrepresentableCharacter`](Self::UnrepresentableCharacter). `None`
    /// for [`NoRoot`](Self::NoRoot), for [`Invalid`](Self::Invalid), whose
    /// [`diagnostics`](Self::diagnostics) name the elements they affect, and
    /// for [`PreservedContent`](Self::PreservedContent), which lists its
    /// elements.
    #[must_use]
    pub fn element(&self) -> Option<&ElementId> {
        match self {
            Self::NoRoot | Self::Invalid { .. } | Self::PreservedContent { .. } => None,
            Self::UnrepresentableCharacter { element, .. } => Some(element),
        }
    }

    /// The catalog code of the refusal: SACM-XMI-001
    /// ([`Code::XMI_INVALID_ROOT`]) for [`NoRoot`](Self::NoRoot),
    /// SACM-XML-003 ([`Code::XML_UNREPRESENTABLE_CHARACTER`]) for
    /// [`UnrepresentableCharacter`](Self::UnrepresentableCharacter),
    /// SACM-XMI-016 ([`Code::XMI_STRICT_SAVE_INVALID`]) for
    /// [`Invalid`](Self::Invalid), whose diagnostics carry the codes of the
    /// schema errors, and SACM-XMI-006 ([`Code::XMI_STRICT_SAVE_REFUSED`])
    /// for [`PreservedContent`](Self::PreservedContent) (LIB-API-017).
    #[must_use]
    pub fn code(&self) -> Code {
        self.details().map_or(Code::XMI_INVALID_ROOT, Details::code)
    }

    /// The id of the refusal's message template, `<CODE>/<form>`, such as
    /// `SACM-XMI-006/one-holder` (ADR 0030 amendment 3).
    /// [`NoRoot`](Self::NoRoot) is `SACM-XMI-001/no-root-to-write`.
    #[must_use]
    pub fn template(&self) -> &'static str {
        self.details()
            .map_or(template::XMI_001_NO_ROOT_TO_WRITE.id, Details::template)
    }

    /// The values the refusal's message is made from, in the order they
    /// first appear in it: empty for [`NoRoot`](Self::NoRoot).
    #[must_use]
    pub fn arguments(&self) -> &[Argument] {
        self.details().map_or(&[], Details::arguments)
    }

    /// The details of the refusal's message, if its variant has them.
    fn details(&self) -> Option<&Details> {
        match self {
            Self::NoRoot => None,
            Self::UnrepresentableCharacter { details, .. }
            | Self::Invalid { details, .. }
            | Self::PreservedContent { details, .. } => Some(details),
        }
    }

    /// The `UnrepresentableCharacter` refusal of `character` in `attribute`
    /// of `element`, in the value at `position` for a value's attribute: the
    /// template with the position when there is one, the one without
    /// otherwise.
    pub(crate) fn unrepresentable_character(
        element: ElementId,
        character: char,
        attribute: WriteAttribute,
        position: Option<usize>,
    ) -> Self {
        let mut arguments = vec![
            Argument::text("element", element.as_str()),
            Argument::prose("attribute", attribute.to_string()),
        ];
        let template = match position {
            Some(position) => {
                arguments.push(Argument::count_of("position", position));
                &template::XML_003_UNREPRESENTABLE_CHARACTER_IN_VALUE
            }
            None => &template::XML_003_UNREPRESENTABLE_CHARACTER,
        };
        arguments.push(Argument::count(
            "code_point",
            u64::from(u32::from(character)),
        ));
        Self::UnrepresentableCharacter {
            element,
            character,
            attribute,
            position,
            details: Details::new(template, arguments),
        }
    }

    /// The `Invalid` refusal of the schema errors `diagnostics`.
    pub(crate) fn invalid(diagnostics: Vec<Diagnostic>) -> Self {
        let first = diagnostics.first();
        let first_code = Argument::text(
            "first_code",
            first.map_or("", |first| first.code().as_str()),
        );
        let first_id = Argument::text(
            "first_id",
            // The element the first error affects, or, for one that affects
            // none, the `xmi:id` of the document's metadata it is about.
            first
                .and_then(|first| {
                    first.affected().first().map(ElementId::as_str).or_else(|| {
                        first
                            .arguments()
                            .iter()
                            .find(|argument| argument.name() == "xmi_id")
                            .and_then(|argument| argument.value().as_text())
                    })
                })
                .unwrap_or(""),
        );
        let details = if diagnostics.len() == 1 {
            Details::new(&template::XMI_016_SCHEMA_ERROR, vec![first_code, first_id])
        } else {
            Details::new(
                &template::XMI_016_SCHEMA_ERRORS,
                vec![
                    Argument::count_of("count", diagnostics.len()),
                    first_code,
                    first_id,
                ],
            )
        };
        Self::Invalid {
            diagnostics,
            details,
        }
    }

    /// The `PreservedContent` refusal of the elements `elements` that hold
    /// preserved content, or of the document alone when it is empty.
    pub(crate) fn preserved_content(elements: Vec<ElementId>) -> Self {
        let details = match elements.as_slice() {
            [one] => Details::new(
                &template::XMI_006_ONE_HOLDER,
                vec![Argument::text("id", one.as_str())],
            ),
            [first, ..] => Details::new(
                &template::XMI_006_HOLDERS,
                vec![
                    Argument::count_of("count", elements.len()),
                    Argument::text("id", first.as_str()),
                ],
            ),
            [] => Details::new(&template::XMI_006_OUTSIDE_ELEMENTS, Vec::new()),
        };
        Self::PreservedContent { elements, details }
    }

    /// The schema errors of [`Invalid`](Self::Invalid), in [`Diagnostic`]
    /// order; empty for every other variant.
    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        match self {
            Self::Invalid { diagnostics, .. } => diagnostics,
            Self::NoRoot
            | Self::UnrepresentableCharacter { .. }
            | Self::PreservedContent { .. } => &[],
        }
    }
}

impl fmt::Display for WriteError {
    /// Writes the refusal on one line, as described on [`WriteError`]: the
    /// message its template and arguments render.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.details() {
            Some(details) => f.write_str(&details.render()),
            None => f.write_str(&template::render(&template::XMI_001_NO_ROOT_TO_WRITE, &[])),
        }
    }
}

impl std::error::Error for WriteError {}

/// The attribute of an element that a [`WriteError`] or an
/// [`EditError`](crate::edit::EditError) names.
///
/// Later releases add variants as they write more attributes, so match it
/// with a fallback arm.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum WriteAttribute {
    /// The element's `xmi:id`.
    Id,
    /// The element's name's `lang`.
    NameLang,
    /// The element's name's `content`.
    NameContent,
    /// The element's `gid`.
    Gid,
    /// The `lang` of a value of the element's `content`.
    ContentValueLang,
    /// The `content` of a value of the element's `content`.
    ContentValueContent,
    /// The `lang` of a value of the element's `key`.
    KeyValueLang,
    /// The `content` of a value of the element's `key`.
    KeyValueContent,
    /// The `version` of an `Artifact` (SACM 2.3 §12.7).
    Version,
    /// The `date` of an `Artifact` or an `Event` (SACM 2.3 §12.7, §12.9).
    Date,
    /// The `startTime` of an `Activity` (SACM 2.3 §12.11).
    StartTime,
    /// The `endTime` of an `Activity` (SACM 2.3 §12.11).
    EndTime,
    /// The `value` of a `Term` or an `Expression` (SACM 2.3 §10.9).
    Value,
    /// The `externalReference` of a `Term` (SACM 2.3 §10.11).
    ExternalReference,
    /// The `lang` of a value of a `Resource`'s `location` (SACM 2.3 §12.10).
    LocationValueLang,
    /// The `content` of a value of a `Resource`'s `location` (SACM 2.3
    /// §12.10).
    LocationValueContent,
}

impl fmt::Display for WriteAttribute {
    /// Writes `xmi:id`, `gid`, `name's lang`, `name's content`, `content
    /// value's lang`, `version`, `startTime`, `location value's content`
    /// and so on.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Id => "xmi:id",
            Self::NameLang => "name's lang",
            Self::NameContent => "name's content",
            Self::Gid => "gid",
            Self::ContentValueLang => "content value's lang",
            Self::ContentValueContent => "content value's content",
            Self::KeyValueLang => "key value's lang",
            Self::KeyValueContent => "key value's content",
            Self::Version => "version",
            Self::Date => "date",
            Self::StartTime => "startTime",
            Self::EndTime => "endTime",
            Self::Value => "value",
            Self::ExternalReference => "externalReference",
            Self::LocationValueLang => "location value's lang",
            Self::LocationValueContent => "location value's content",
        })
    }
}

// A refusal on one thread can be reported on another.
const _: () = {
    const fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<WriteError>();
};

#[cfg(test)]
mod tests {
    use super::ReadError;
    use crate::diagnostic::{Argument, Code, Details, SourceLocation, template};
    use crate::model::{ElementId, ElementKind};

    /// The C++ shape no strict read refuses, since the `name-value` shape of
    /// the same `value` comes first, built as the reader builds it
    /// (LIB-API-017, acceptance 4).
    #[test]
    fn the_shape_of_text_in_a_name_value_has_its_template() {
        let arguments = vec![
            Argument::text("class", "ArgumentPackage"),
            Argument::text("id", "p"),
        ];
        let error = ReadError::legacy_shape(
            ElementId::new("p"),
            Details::new(&template::XMI_012_NAME_VALUE_TEXT, arguments.clone()),
            SourceLocation::at(1, 2),
        );
        assert_eq!(
            (error.code(), error.template(), error.arguments()),
            (
                Code::XMI_LEGACY_SHAPE,
                "SACM-XMI-012/name-value-text",
                arguments.as_slice()
            ),
            "LIB-API-017"
        );
        assert_eq!(
            error.to_string(),
            "the value in the name of the ArgumentPackage \"p\" has its content as text, a \
             shape of the assurance-forge C++ library at line 1, column 2: a strict read \
             refuses it; a tolerant read reads it as its standard form, which write writes",
            "LIB-API-017"
        );
    }

    /// A multi-valued end given as an attribute and as elements is not
    /// called single-valued (ADR 0027); every other property is.
    #[test]
    fn a_repeated_value_names_a_multi_valued_end_by_rule_2h() {
        let repeated = |property: &str| {
            ReadError::repeated_value(
                ElementId::new("i1"),
                ElementKind::AssertedInference,
                property.to_owned(),
                [SourceLocation::at(1, 2), SourceLocation::at(3, 4)]
                    .into_iter()
                    .flatten()
                    .collect(),
            )
        };
        for property in ["metaClaim", "source"] {
            assert_eq!(
                repeated(property).to_string(),
                format!(
                    "the property {property:?} of the element \"i1\" is given both as an \
                     attribute and as elements, which XMI 2.5.1 rule 2h forbids, at line 1, \
                     column 2; line 3, column 4"
                )
            );
        }
        for property in ["target", "citedElement", "name"] {
            assert_eq!(
                repeated(property).to_string(),
                format!(
                    "the single-valued property {property:?} of the element \"i1\" is given 2 \
                     times, at line 1, column 2; line 3, column 4"
                )
            );
        }
    }
}
