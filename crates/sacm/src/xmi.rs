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
/// - **Nothing is dropped.** What `read` does not read is kept as
///   [preserved content](#preserved-content) or, in strict mode, refused.
/// - **Modes.** [`ReadOptions`] picks strict, the default, or tolerant
///   ([Modes](self#modes)); [Errors](#errors) lists what each refuses.
/// - **Prefixes do not matter.** Names match by namespace URI and local name. The
///   SACM namespace is `http://www.omg.org/spec/SACM/20220301`.
/// - **A type is a `QName`.** An `xmi:type` or `xsi:type` value is trimmed and
///   resolved against the namespaces in scope.
/// - **Encodings.** UTF-8, ASCII, a declared ISO-8859-1, and UTF-16 with a byte
///   order mark, which reads as the same characters in UTF-8.
/// - **No entity is expanded.** A DOCTYPE is refused before anything in it is read.
/// - **Nothing is repaired.** The model rules are
///   [`validation::validate`](crate::validation::validate)'s.
///
/// # Roots and children
///
/// - **A root** is the document element, or each element child outside the XMI
///   namespace of an `xmi:XMI` element in the XMI 2.5.1 namespace
///   `http://www.omg.org/spec/XMI/20131001`.
/// - **Its kind** is its element name: one of the twelve package kinds, the four
///   packages (formal/23-05-08 §2.2, §2.4, §2.5), their interfaces and bindings.
/// - **It becomes** a parentless element, in document order.
/// - **A child's element name is its role,** in no namespace. Children are read in
///   document order, each with its id (see [Ids](#ids)).
///
/// | Parent | Role element | Reads | Needs `xmi:type` |
/// |---|---|---|---|
/// | AssuranceCasePackage (§9.2) | `assuranceCasePackage`, `argumentPackage`, `artifactPackage`, `terminologyPackage` | the package the role names | no |
/// | TerminologyPackage (§10.4) | `terminologyElement` | a TerminologyPackage, a `Category`, a `Term`, an `Expression` or a `TerminologyGroup` | yes: the role's type, `TerminologyElement`, is abstract (§10.2) |
/// | ArgumentPackage (§11.4) | `argumentElement` | an ArgumentPackage, a Claim, an `ArgumentReasoning`, an `ArtifactReference`, an `ArgumentGroup`, an `AssertedInference`, an `AssertedEvidence`, an `AssertedContext`, an `AssertedArtifactSupport` or an `AssertedArtifactContext` | yes: the role's type, `ArgumentationElement`, is abstract (§11.3) |
/// | ArtifactPackage (§12.2) | `artifactElement` | an AssuranceCasePackage, an ArgumentPackage, an ArtifactPackage, any of the nine argumentation kinds above, a `Property`, a `Technique`, a `Participant`, an `ArtifactGroup`, an `Artifact`, an `Event`, a `Resource`, an `Activity`, an `ArtifactAssetRelationship`, or any of the five Terminology kinds above | yes: the role's type, `ArtifactElement`, is abstract (§8.10) |
/// | every `ArtifactAsset` (§12.6) | `property` | a `Property` | no |
/// | each of the above (§8.6) | `description`, `implementationConstraint`, `note`, `taggedValue` | the `UtilityElement` the role names | no |
///
/// - **Interfaces and bindings.** A role that reads a package also reads its
///   interface and its binding, which need an `xmi:type` there. They hold their
///   package's roles ([`ElementKind::may_contain`]).
/// - **The class** is the one `xmi:type` or `xsi:type` names (both, if present, must
///   name the same class); without one, the role's declared type.
/// - **No type under an abstract role.** The element name is the role, so no class
///   can be inferred (XMI 2.5.1 §7.6.3), and none is guessed. Strict:
///   [`ReadError::MissingType`]. Tolerant: kept whole in its parent, with SACM-XMI-005.
/// - **Role spellings.** §11.4's `argumentationElement` and §10.4's
///   `TerminologyElement` are [Spellings](#spellings).
/// - **Order.** Several of a role are kept. The four `UtilityElement`s are children
///   in the same document order as the rest;
///   [`SacmElement::descriptions`](crate::model::SacmElement::descriptions) and the
///   like filter it.
///
/// # Names and texts
///
/// - **A name** is the element's `name` child: its `lang` and `content`. A field
///   whose attribute is not written is absent; an element without a `name` has none.
/// - **Values are exact,** never trimmed. A literal TAB, LF or CR in an attribute is
///   already a space (XML 1.0 §3.3.3); write `&#9;`, `&#10;` or `&#13;` to keep one.
/// - **A `UtilityElement`** has no name and holds no element: only its `content`
///   (§8.7) and, on a `TaggedValue`, its `key` (§8.12), in either order.
/// - **A `content` or `key`** is one role element holding a `MultiLangString`
///   (ADR 0025): one `<value>` per [`LangString`](crate::model::LangString), in
///   order and with duplicates, each read as a `name` is.
/// - **Absent or empty.** No `content` is `None`. `<content/>`, or one holding only
///   white space, comments and processing instructions, is an empty `MultiLangString`.
/// - **A `content` or `key` carries** only `xmi:id` and a type naming
///   `sacm:MultiLangString`, besides the C++ shorthand (see
///   [The C++ name and text shapes](#the-c-name-and-text-shapes)).
/// - **A `name` or `value`** is in no namespace and carries only `xmi:id`, `lang`,
///   `content`, a type naming `sacm:LangString` or `sacm:ExpressionLangString` and,
///   on an `ExpressionLangString` (§8.4), its `expression`: one IDREF.
///
/// # Values
///
/// Each is in no namespace, an attribute or a [value element](#value-elements),
/// read exactly as XML gives it.
///
/// | Value | On | Read as |
/// |---|---|---|
/// | `gid` (§8.2) | every element | a String: absent, `""` and any other value are kept apart. It is no id: no id space, no syntax check |
/// | `isCitation`, `isAbstract` (§8.2 `[0..1]=false`) | every element | an `xsd:boolean`; absent is false |
/// | `isCounter` (§11.13 `[1] = false`) | an asserted relationship | an `xsd:boolean`; absent is false |
/// | `assertionDeclaration` (§11.10) | an Assertion: a Claim or an asserted relationship | one of the six literals; absent is `asserted` |
/// | `value` (§10.9 `String[1]`) | a Term or an Expression | a String; one read without it is kept, and `validate` reports it |
/// | `externalReference` (§10.11 `String[0..1]`) | a Term | a String |
/// | `version`, `date`, `startTime`, `endTime` | an Artifact, an Event, an Activity, as [`write`](fn@write) lists | a String (D-DATE) |
///
/// - **An `xsd:boolean`.** White space is collapsed; `true` and `1` read as true,
///   `false` and `0` as false. `TRUE`, `yes` and the empty value are no literals.
/// - **An `assertionDeclaration`** is `asserted`, `needsSupport`, `assumed`,
///   `axiomatic`, `defeated` or `asCited`
///   ([`AssertionDeclaration`](crate::model::AssertionDeclaration)), matched exactly:
///   case-sensitive, nothing trimmed.
/// - **A value that is no literal** is never read as the default, which would
///   change what the case declares. Strict: [`ReadError::InvalidLiteral`], holding
///   the value as read. Fix: correct the spelling.
/// - **Tolerant** keeps it as preserved content, with SACM-ENUM-001, and the
///   property holds its default until an edit sets it:
///   [`SetAssertionDeclaration`](crate::edit::SetAssertionDeclaration) replaces the
///   kept attribute. It counts as the property's first occurrence.
/// - **A feature the kind lacks** is preserved: `isCounter` on a Claim, or
///   `assertionDeclaration` on an `ArtifactReference`, which is no Assertion.
/// - **Nothing is added.** A flag and its reference are independent, and a
///   relationship without a `source` or a `target` is kept as read;
///   [`validation::validate`](crate::validation::validate) reports both
///   (SACM23-VAL-005).
///
/// # Value elements
///
/// Each value above, and the `lang` and `content` of a `name` or `value`, may be
/// written as an element of its name (XMI 2.5.1 §7.8.4), anywhere among its siblings.
///
/// - **Same document.** Both forms read alike, and [`write()`] writes the
///   attribute form.
/// - **Its value** is its character content, joined in document order. CDATA
///   sections and references give their characters, comments and processing
///   instructions are skipped, and an empty element is the empty string.
/// - **White space is kept exactly.** Only a line end (CRLF or a lone CR) reads
///   as LF (XML 1.0 §2.11).
/// - **It carries** only namespace declarations and `xsi:nil`, and holds no element.
/// - **Given more than once,** as an attribute and an element or as two elements,
///   even with equal values. Strict: [`ReadError::RepeatedValue`]. Tolerant: reads
///   the first and keeps each later one, with SACM-MULT-001 at it.
/// - **So is** a second `name`, `content`, `key` or `location`.
///
/// `xsi:nil` (XMI 2.5.1 §9.4.1), in `http://www.w3.org/2001/XMLSchema-instance` by
/// any prefix, is an `xsd:boolean` on a value element (SACM23-XMI-026):
///
/// - **True** reads as if the property were not written: an absent `gid`, `lang`
///   or `content`, a false flag, an `asserted` declaration. Both modes, no
///   diagnostic. [`write()`] writes the property as omitted.
/// - **False** reads the content as usual.
/// - **Any other value,** such as `TRUE`, `yes` or `""`, is a value that is no
///   literal, of the property `xsi:nil`.
/// - **A nil element is empty.** One holding a character, even white space, or an
///   element does not conform, and is preserved.
/// - **It still counts** as an occurrence for `RepeatedValue`.
/// - **Elsewhere** `xsi:nil`, and an unqualified `nil`, is an attribute the class
///   lacks, and is preserved.
///
/// # What an element may carry
///
/// | Element | Carries | Holds |
/// |---|---|---|
/// | a root | namespace declarations, `xmi:id`, `gid`, `isCitation`, `isAbstract`, `citedElement`, `abstractForm`, `xmi:version`, a type naming its own class | at most one `name`, at most one value or reference element of each of those five, the children above |
/// | a child | as a root without `xmi:version`, its type attributes, the features of its kind | as a root, with a value or reference element for each feature of its kind |
/// | a `UtilityElement` | as a child | besides those five, only its `content` and `key` |
/// | a `name` or a `value` | see [Names and texts](#names-and-texts) | the value elements `lang` and `content`, an `ExpressionLangString`'s one `expression` reference element |
///
/// - **Also allowed:** white space, comments and processing instructions in each,
///   and [XMI metadata](#xmi-metadata) on and in each.
/// - **Anything else** does not conform, and is kept as
///   [preserved content](#preserved-content), except the
///   [C++ shapes](#the-c-name-and-text-shapes).
/// - **`xmi:version`** on a root or on `xmi:XMI` is accepted with any value and
///   ignored: the XMI namespace identifies the version (XMI 2.5.1 §7.5.3).
///   [`write()`] does not write it back. It is reported as SACM-XMI-011.
///
/// # Spellings
///
/// formal/23-05-08 and ptc/22-03-13 spell four names differently, and the crate
/// follows one of them (D-TEXT-MODEL; SACM23-XMI-009, SACM23-XMI-028; ADR 0034
/// amendments 2 and 3).
///
/// | Spelling | Where | Resolved name |
/// |---|---|---|
/// | the type `ArtifactAssertedRelationship` | `xmi:type` or `xsi:type`, under a role that admits an `ArtifactAssetRelationship` (§12.14) | that class |
/// | `occurece` | an Event's attribute or value element (§12.9) | `date` |
/// | `argumentationElement` | an ArgumentPackage's role (§11.4) or an `ArgumentGroup`'s end (§11.2) | `argumentElement` |
/// | `TerminologyElement` | a TerminologyPackage's role (§10.4, spec defect D9) | `terminologyElement` |
///
/// - **Tolerant** reads each as the resolved name, under every rule of that name,
///   with one SACM-XMI-013 Warning ([`Code::XMI_SPELLING_VARIANT`]) at its start
///   tag or attribute name, affecting the element the spelling belongs to.
/// - **Both names given.** The value is read from the resolved name; the spelled
///   one is a repeat (SACM-MULT-001), kept as preserved content.
/// - **Strict** never maps a spelling. It keeps the construct as preserved content
///   of the element it sits in, with one SACM-XMI-013 Error, so a strict
///   [`write`](fn@write) refuses the document.
/// - **Elsewhere** a spelling is ordinary content (SACM-XMI-003). So is
///   `TerminologyElement` as a `TerminologyGroup`'s end, which §10.3 spells
///   `terminologyElement`.
/// - **Not read:** the C++ library's `occurence`, in a SACM 2.3 root.
/// - **[`write`](fn@write)** writes the resolved names.
///
/// # Preserved content
///
/// Content that neither the SACM XML Schema nor the XMI 2.5.1 production rules
/// allow where it stands, and that no tolerance reads, is kept exactly as written,
/// in both modes, by the element it sits in: its holder (LIB-COMPAT-003, ADR 0033).
///
/// What is preserved:
///
/// - **An attribute the class lacks:** a vendor attribute, `isCounter` on a Claim,
///   an `id` in no namespace, `xmi:version` on a child, an XMI attribute XMI.xsd
///   does not define, or a reference attribute that holds no id (`citedElement=""`).
/// - **A child** in another namespace or under a role the class lacks, and any
///   child of a `UtilityElement`.
/// - **A child whose type** names no concrete SACM class, is not admitted by its
///   role, or disagrees with its other type attribute.
/// - **A `name`, `content`, `key`, value element or reference element** that does
///   not conform inside: kept whole, so the property is not given.
/// - **Text** that is not all white space in element-only content.
/// - **A root's type attribute** naming another class. The root is read as the
///   kind its element name gives, and no edit makes it a child
///   ([`EditError::KeptRootType`](crate::edit::EditError::KeptRootType)).
///
/// How it is reported and held:
///
/// - **One SACM-XMI-003 each** ([`Code::XMI_UNKNOWN_CONTENT`]), affecting the
///   holder: a Warning in tolerant mode, an Error in strict mode. It is at the
///   attribute's name, the element's start tag, or the text's first character
///   that is not literal white space.
/// - **Nothing inside is read or reported,** but each `xmi:id` in it is checked. A
///   repeated one is [`ReadError::DuplicateId`]. An empty one is
///   [`ReadError::InvalidId`] in strict mode; tolerant keeps it as written,
///   outside the id space, with SACM-ID-002.
/// - **No occurrence.** A preserved part never makes a
///   [`ReadError::RepeatedValue`], unless a tolerant read kept it for a value
///   that is no literal.
/// - **Asking.**
///   [`SacmElement::has_preserved_content`](crate::model::SacmElement::has_preserved_content)
///   tells a holder.
/// - **Place.** A child keeps its index among the holder's children, and an
///   attribute its place after the typed ones, in input order.
/// - **Exact.** Namespace URIs, prefixes, local names, attribute values and the
///   text inside an element are kept as written.
/// - **Trimmed or dropped.** Text directly in a holder is trimmed of XML white
///   space. Comments and processing instructions are dropped, as everywhere.
/// - **A kept type attribute** is a `QName` (XMI 2.5.1 §9.5.2 rule 2g), so the
///   holder also keeps the binding of its prefix. A value prefix `sacm` or `xmi`
///   bound to another namespace, or no prefix under a default namespace, becomes
///   the holder's `nsN` for that namespace (LIB-COMPAT-019).
/// - **Ids.** A preserved id stays in the document's id space, though
///   [`Document::get`] does not find it. A reference to one is SACM-REF-003 in
///   [`validation::validate`](crate::validation::validate).
/// - **Writing.** A tolerant [`write()`] writes it back; a strict one refuses it.
///
/// Defects with no element to hold them (ADR 0035). A strict read refuses each as
/// [`ReadError::Unsupported`]; a tolerant read keeps each, with SACM-XMI-003:
///
/// | Input | A tolerant read keeps it |
/// |---|---|
/// | an attribute or text on `xmi:XMI`, or an XMI-namespace element XMI 2.5.1 does not define under it, such as `xmi:foo` | as the document's own preserved content ([`Document::has_preserved_content`]), affecting no element, written back on and in `xmi:XMI`, in place among the roots |
/// | a preserved attribute whose prefix `sacm` or `xmi` is bound to another namespace than SACM 2.3 or XMI 2.5.1, which the written element binds them to | under the first prefix `nsN` not bound at its element, which the message names |
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
/// XMI metadata is read in both modes with no diagnostic, held exactly as written,
/// never as preserved content, and written back by both saves where it stood
/// (SACM23-XMI-019, ADR 0042).
///
/// | Where | What |
/// |---|---|
/// | on `xmi:XMI` | `xmi:id`, `xmi:label`, `xmi:uuid`, `href`, `xmi:idref`, `xmi:type`, `xsi:schemaLocation`, `xsi:noNamespaceSchemaLocation` |
/// | under `xmi:XMI`, among the roots | `documentation`, `Documentation`, `difference`, `Difference`, `Add`, `Replace`, `Delete`, `extension` and `Extension`, each in the XMI namespace and kept whole |
/// | on a root, a child, a `name`, a `content`, `key` or `location`, or a `value` | `xmi:uuid`, `xmi:label`, `xsi:schemaLocation`, `xsi:noNamespaceSchemaLocation` |
/// | in any of these | `xmi:extension` and `xmi:Extension` elements, kept whole, their content not checked |
///
/// - **An element's own values:**
///   [`SacmElement::uuid`](crate::model::SacmElement::uuid) and
///   [`SacmElement::label`](crate::model::SacmElement::label).
/// - **Ids.** Metadata ids share the document's id space, as preserved ids do.
///   Nothing inside a Difference's `target` is an id (XMI.xsd:
///   `processContents="skip"`).
/// - **Edits.** A copy drops the `xmi:uuid` of each element and part it copies
///   (XMI 2.5.1 §7.6.1: elements with one `xmi:uuid` "are all deemed to reference
///   the same MOF element"). A delete removes the metadata with the element; a
///   move keeps it.
/// - **Differences** are kept, never applied.
/// - **Dropped:** comments and processing instructions inside an extension, as
///   everywhere in the reader.
/// - **Not metadata,** so SACM-XMI-003 and preserved: any other XMI-namespace
///   element where metadata may stand, an `href` or an `xmi:idref` on a definition
///   (production 2a), and an identity attribute or a type on a reference element
///   (production 2c).
/// - **An XMI-namespace document element** other than `xmi:XMI` is
///   [`ReadError::InvalidRoot`].
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
/// A role element of a composite that carries `xmi:idref="id"` or `href="#id"`, at
/// most one type attribute and nothing else, is a proxy of an element of the same
/// document (XMI 2.5.1 §7.10.1; SACM23-XMI-030).
///
/// - **Resolved** once every root is read, in both modes, with no diagnostic: a
///   proxy that names a root of a kind its role and its type admit moves that root
///   into its place.
/// - **The root** becomes a child of the proxy's container and stops being a
///   root. Chains of proxies are followed, and a write writes a moved root as a
///   child.
/// - **Kept instead,** whole, as preserved content of its container, is a proxy
///   that fails one of these checks, in this order: it names no root; one of the
///   wrong kind; one that keeps an `href` or an `xmi:idref` of its own; one
///   another proxy names too; or it is part of a cycle.
/// - **A kept proxy** gives one SACM-XMI-003 at its `<`: an Error in strict mode,
///   a Warning in tolerant mode. So does a proxy with anything else on or in it.
/// - **Its diagnostics,** its `xsi:type`'s SACM-XMI-010 and its spelling's
///   SACM-XMI-013 included, affect its container, never the id it names.
///
/// # Cross-file proxies
///
/// A role element of a composite in reference form whose `href` names an element
/// of another document, such as `<argumentPackage href="arguments.sacm.xmi#args"/>`,
/// or an `XPointer`, is a proxy of a child held in that document (XMI 2.5.1
/// §7.10.1, §7.10.2; SACM23-XMI-029, ADR 0040 amendment 1).
///
/// - **Held,** in both modes with no diagnostic and in document order, as a
///   [`Proxy`](crate::model::Proxy) of its container
///   ([`SacmElement::proxies`](crate::model::SacmElement::proxies)): its role and
///   its [`Href`](crate::model::Href).
/// - **Never resolved.** No file is opened.
/// - **An `XPointer` is never evaluated.** `href="#xpointer(id('args'))"` is held
///   as a proxy even when it names a root of this document, which stays a root.
/// - **Its type,** given by `xmi:type` (or `xsi:type`, with its SACM-XMI-010) or
///   before the URI reference (rule 2n), is held as the href's type. The role
///   must admit it, and the two must agree if both are given.
/// - **Written** as `<role href="…" />`, after the container's children.
/// - **Neither a definition nor a proxy,** so kept whole with SACM-XMI-003: a role
///   element with a link attribute and anything else, such as an `xmi:id`, an
///   `xmi:uuid`, another attribute or content, or with both `href` and `xmi:idref`.
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
/// Depth is counted as resolved (SACM-SEC-003):
///
/// - **A root that stays** keeps its input depth. A moved root takes its proxy's
///   depth, and every element of its tree, its parts included, shifts with it.
/// - **Too deep.** An element nested deeper than [`Limits::max_depth`] as resolved
///   refuses the read in both modes, as [`ReadError::LimitExceeded`] for
///   [`Limit::Depth`], at the first such element.
/// - **So** a chain of proxies between flat roots cannot nest a document deeper
///   than its limit, and its output reads back under the same limits.
///
/// # The C++ name and text shapes
///
/// The assurance-forge C++ library writes some names and texts in shapes that XMI
/// 2.5.1 does not define. Each maps one to one onto the standard form
/// (LIB-COMPAT-008, ADR 0022 amendment 1).
///
/// | Shape | Read as |
/// |---|---|
/// | a `name="…"` attribute on a root or child that is no `UtilityElement` | its name, with the value as `content` and no `lang` |
/// | text in a `name` or a `value` | its `content` |
/// | a `name` holding one `value` and nothing else, or one `content` carrying a `lang` attribute | the name's `lang` and `content` |
/// | on a `content` or `key` with no `value`, a `content` attribute or text, with an optional `lang` attribute | a `MultiLangString` of one value, keeping the role element's `xmi:id` |
/// | `content="…"`, or text, with an optional `lang="…"`, on a `UtilityElement` | its `content`, of one value |
/// | `key="…"` and `value="…"` on a `TaggedValue`, with an optional `lang="…"` for `value="…"` | its `key`, of one value with no `lang`, and its `content`, of one value |
///
/// - **Tolerant** reads each as its standard form, with one SACM-XMI-012 Warning
///   per occurrence ([`Code::XMI_LEGACY_SHAPE`]).
/// - **Strict** never applies a tolerance. Once the rest of the input reads
///   without a refusal, it refuses the first shape as [`ReadError::LegacyShape`].
/// - **[`write()`]** writes the standard form, in both modes.
/// - **Values are exact.** Absent is not `""`, and nothing is trimmed. Text is all
///   of the element's character content, joined; CDATA sections and references
///   give their characters.
/// - **No shape:** text that is all white space, and a `lang` attribute alone.
/// - **A repeat.** A field given both in a shape and in the standard form, or
///   twice, such as `content="a"` and text in one `name`, is
///   [`ReadError::RepeatedValue`] in strict mode. Tolerant keeps it as any repeat.
/// - **Preserved whole:** shapes that would invent or merge elements, such as a
///   `name` holding two values, a `value` beside other fields in a `name`, or text
///   beside the `value`s of a `content` (LIB-COMPAT-012 is to read them).
/// - **An `xmi:id` on the `value` of a `name`** does not conform either. Both modes
///   keep the whole `name` as preserved content, with SACM-XMI-003 at its start
///   tag (SACM23-XMI-019).
///
/// # Ids
///
/// An element's id is the value of its `xmi:id`, an `xsd:ID` (XMI 2.5.1 §7.6.1).
/// The ids of the elements, and of the names, `content`s, `key`s and `value`s, of
/// a file share one id space (ADR 0024, ADR 0025).
///
/// - **Only `xmi:id` is one.** An `id` in no namespace or another, such as `id` or
///   `o:id`, is an attribute the class lacks, and is preserved.
/// - **Trimmed.** XML white space (space, TAB, LF, CR) around an `NCName`, as in
///   `" c1 "`, is dropped: an `xsd:ID`'s white space is collapsed (XMI 2.5.1
///   §7.6.1; XML Schema 1.0 Part 2 §4.3.6). Any other id is read as written.
/// - **Generated.** An element without an `xmi:id`, a root or a child, gets
///   `<snake_case kind>_<n>`, as [`edit::apply`](crate::edit::apply) generates one
///   (`claim_1`).
/// - **Which `n`.** The smallest whose id is the value of no `xmi:id` in the input
///   outside a Difference's `target`, is named by no reference in it, and was not
///   generated earlier in the read. So a reference that names no element never
///   resolves to a generated one.
/// - **A generated id is ordinary.** [`write()`] writes it, and an edit continues
///   above it, also skipping every referenced id. The same bytes always give the
///   same ids.
/// - **A `gid` is not an id.** It is in no id space, never trimmed and never
///   generated, and may be the same string as any `xmi:id`.
///
/// The `xmi:id` of a part is kept with the element that owns it, and [`write()`]
/// writes it back. It identifies no element, and a part without one gets none.
///
/// | Part | Its id | Its values' ids, by position |
/// |---|---|---|
/// | `name` | [`name_id`](crate::model::SacmElement::name_id) | |
/// | `content` | [`content_id`](crate::model::SacmElement::content_id) | [`content_value_ids`](crate::model::SacmElement::content_value_ids) |
/// | `key` | [`key_id`](crate::model::SacmElement::key_id) | [`key_value_ids`](crate::model::SacmElement::key_value_ids) |
/// | `location` | [`location_id`](crate::model::SacmElement::location_id) | [`location_value_ids`](crate::model::SacmElement::location_value_ids) |
///
/// # References
///
/// A reference to another element is held as the id it names, unresolved
/// ([ADR 0026]).
///
/// - **The ends** are those of [`ReferenceEnd`], which says who holds each and how
///   many ids it takes. `citedElement` and `abstractForm` (§8.2) are on every
///   element.
/// - **Reading one.**
///   [`SacmElement::references`](crate::model::SacmElement::references) gives an
///   end's ids, and [`Document::get`] resolves one.
/// - **A multi-valued end** holds any number of ids, in document order, repeats
///   included ([ADR 0027], D-ORDER): `metaClaim="m1 m1"` holds `m1` twice. A
///   single-valued end holds at most one.
/// - **Resolution** is by id across the whole input, whatever the order: to a
///   later element, an element of another root, or the element itself.
/// - **The target's type** is not checked on read;
///   [`validation::validate`](crate::validation::validate) reports a wrong one
///   (SACM-REF-002).
///
/// The two forms read to the same document, and [`write()`] writes the attribute
/// form:
///
/// - **The attribute form.** An attribute in no namespace named after the end
///   holds the ids separated by XML white space (`xsd:IDREFS`, XMI 2.5.1 §9.5.2
///   rule 2j, SACM23-XMI-005).
/// - **Its tokens** are held exactly as written: `" c2 "` is `c2`, and `#c2` is the
///   id `#c2`, not `c2`. A value with no token, such as `""`, does not conform:
///   it is preserved, never read as no reference.
/// - **The element form.** An element in no namespace named after the end,
///   anywhere among its siblings, with exactly one link attribute: `xmi:idref="id"`
///   or an unqualified `href="#id"` (XMI 2.5.1 §9.5.2 production 2c, rules 2l and
///   2n, SACM23-XMI-015).
/// - **Its value** is read without the XML white space around it, and names one
///   id. The element holds only white space, comments and processing
///   instructions. On a multi-valued end, each element appends one id, in
///   document order (rule 2b).
/// - **A group's member end** shares its name with a package's role:
///   `argumentElement`, `artifactElement`, `terminologyElement`. The parent's kind
///   decides: under a group, such an element is a reference element, never a
///   child.
///
/// Given too often, a reference is [`ReadError::RepeatedValue`] in strict mode:
///
/// | Given as | Locations |
/// |---|---|
/// | two or more tokens in the attribute of a single-valued end | one per token, at the token |
/// | an attribute and an element for the same end (XMI 2.5.1 rule 2h) | every occurrence |
/// | two elements on a single-valued end | every occurrence |
///
/// Preserved, never dropped, each at its start tag or attribute name:
///
/// - **A reference element that does not conform:** with both link attributes or
///   neither, with any other attribute such as an unqualified `idref` or
///   `xmi:href`, or holding a child element or text.
/// - **An `xmi:idref` that names no single id:** empty or, once trimmed, still
///   holding XML white space (space, TAB, LF or CR), such as `a b` or `a&#9;b`. A
///   tolerant [`write()`] would give it back as two ids.
/// - **More than its link** (production 2c, rule 4e; SACM23-XMI-029): an identity
///   attribute (`xmi:id`, `xmi:uuid`, `xmi:label`), a type or an `XLink`
///   attribute. The whole element is kept with SACM-XMI-003, in both modes.
/// - **Both links.** An `href` beside an `xmi:idref` (§7.6.2: "any one of which
///   may be used at one time").
/// - **A group member that is no reference,** such as
///   `<argumentElement xmi:type="sacm:Claim" xmi:id="c9"/>` or one holding only a
///   `name`: the group preserves it whole.
/// - **In a namespace:** an end or a reference element such as
///   `sacm:citedElement`.
/// - **Where there is no end:** a reference on a `name`, `content`, `key`,
///   `location` or `value`, and an end on a kind that lacks it, such as `source`
///   on a Claim or `metaClaim` on a Note.
///
/// Cross-document references are read (SACM23-XMI-027, SACM23-XMI-029, ADR 0040):
///
/// - **`document#fragment`.** An `href` that, trimmed, has one `#`, a fragment
///   without `#` and no XML white space, such as `other.xmi#c2`, names an
///   element of another document.
/// - **An `XPointer`** is a fragment that holds `(` or `)`, such as
///   `#xpointer(id('c2'))`. Its document part may be empty.
/// - **A type may stand before the URI reference** (XMI 2.5.1 rule 2n): a `QName`
///   that names a concrete SACM class, then XML white space, as in
///   `sacm:Claim other.xmi#c2`. A type before `#id` is read as the id.
/// - **Held, never resolved,** in the end's own list,
///   [`SacmElement::hrefs`](crate::model::SacmElement::hrefs), as
///   [`Href`](crate::model::Href) describes. It counts as a value of the end.
/// - **An `ExpressionLangString`'s `expression` element** holds one the same way
///   ([`LangString::expression_href`](crate::model::LangString::expression_href)).
/// - **An `href` that names no element** keeps the whole reference element as
///   preserved content, with SACM-XMI-003: no `#` (`c2`), `#` alone, empty, an
///   empty fragment, a `#` in the fragment, white space in the URI reference, or
///   a type that names no concrete SACM class.
///
/// Dangling references are kept:
///
/// - **What one is.** A reference whose id names no element: an id absent from
///   the input, or the `xmi:id` of a `name`, `content`, `key`, `location` or
///   `value`.
/// - **Read** in both modes without a diagnostic, held as written, and written
///   back by a tolerant [`write()`].
/// - **Reported** by [`validation::validate`](crate::validation::validate) as
///   SACM-REF-001, an Error (SACM23-VAL-003).
/// - **A strict [`write()`]** refuses one that names no `xmi:id` it writes: an
///   IDREF that names no ID is not schema-valid.
///
/// [ADR 0026]: https://github.com/lasrod/libSACM/blob/main/docs/adr/0026-references-between-elements.md
/// [ADR 0027]: https://github.com/lasrod/libSACM/blob/main/docs/adr/0027-asserted-relationships.md
///
/// # Diagnostics
///
/// A document that is read comes with diagnostics, in [`ReadOutput::diagnostics`],
/// sorted by [`Diagnostic`]'s order (LIB-COMPAT-001, LIB-COMPAT-003,
/// LIB-COMPAT-008; ADR 0022, ADR 0024, ADR 0033).
///
/// - **Once each:** per attribute, at the first character of its name, or per
///   element, at the `<` of its start tag.
/// - **Except** SACM-XMI-012, once per C++ shape, and SACM-XMI-003, once per
///   preserved construct. Either may be located at text.
/// - **None refuses the input,** and a tolerant read reports no Error.
/// - **Later releases.** A minor release may add a read diagnostic, so a strict
///   read may then report an Error where it did not; the release notes list each.
///
/// Both modes report these:
///
/// | Code | Clause | For each | Affects | Severity |
/// |---|---|---|---|---|
/// | SACM-XMI-003 ([`Code::XMI_UNKNOWN_CONTENT`]) | 7.3.3 | construct kept as [preserved content](#preserved-content) | its holder | Error in strict, Warning in tolerant |
/// | SACM-XMI-004 ([`Code::XMI_MISSING_ID`]) | 7.6.1 | element read without an `xmi:id` (SACM23-XMI-020); it names the kind and the id | the generated id | Info |
/// | SACM-ID-004 ([`Code::ID_TRIMMED`]) | 7.6.1 | `xmi:id` read without the white space around it (SACM23-XMI-025); it quotes the value as read | the trimmed id, or the element that owns the `name`, `content`, `key`, `location` or `value` | Info |
/// | SACM-XMI-010 ([`Code::XMI_XSI_TYPE`]) | 9.5.2 | `xsi:type` on a root, a child, a `name`, a `content`, a `key`, a `location` or a `value` | the element, or the element that owns the part | Error in strict, Warning in tolerant |
/// | SACM-XMI-011 ([`Code::XMI_VERSION_IGNORED`]) | 7.5.3 | `xmi:version` on `xmi:XMI` or on a root | the root; no element for `xmi:XMI` | Info |
/// | SACM-XMI-013 ([`Code::XMI_SPELLING_VARIANT`]) | 12.14, 12.9, 11.4, 11.2 or 10.4: the text that names the construct | [spelling](#spellings) | tolerant: the element the spelling belongs to; strict: the element that keeps it | Error in strict, Warning in tolerant |
///
/// - **Why `xsi:type` is reported.** XMI 2.5.1 §9.5.2 rule 2g allows it only if
///   `org.omg.xmi.useSchemaExtensions` is true, and SACM sets no such tag. The
///   type is read as an `xmi:type` would be. An element with both type attributes
///   gets one SACM-XMI-010.
/// - **`xmi:version` elsewhere** is an attribute XMI.xsd does not define, and is
///   preserved.
///
/// Only a tolerant read reports these, each a Warning (ADR 0035 D1; see
/// [Modes](self#modes)). A strict read refuses each construct instead:
///
/// | Code | For | What is kept |
/// |---|---|---|
/// | SACM-XMI-012 ([`Code::XMI_LEGACY_SHAPE`]), clause 7.8.6 | each [C++ shape](#the-c-name-and-text-shapes) (LIB-COMPAT-008) | nothing: it is read as its standard form |
/// | SACM-ENUM-001 ([`Code::ENUM_INVALID_LITERAL`]) | a value that is no literal | the value as written; the property holds its default |
/// | SACM-MULT-001 ([`Code::MULTIPLICITY_VIOLATION`]) | each occurrence of a property after its first | the occurrence as written |
/// | SACM-XMI-005 ([`Code::XMI_MISSING_TYPE`]) | an untyped child under an abstract role | the child, whole, in its parent |
/// | SACM-ID-002 ([`Code::ID_INVALID`]) | an empty `xmi:id` | see below |
/// | SACM-XMI-003 | an attribute, text or an XMI element XMI 2.5.1 does not define, on or in `xmi:XMI`; an attribute whose prefix is rebound | the first by the document; the second under `nsN` |
///
/// - **Where.** At the strict refusal's location, affecting the element that
///   holds the construct, or no element for content outside every element.
/// - **SACM-XMI-012** is at the shape's first attribute, else its first text
///   character that is not literal white space, else the `<` of a `value` in a
///   `name`. It affects the element whose name or text it gives.
/// - **Two ids in the attribute of a single-valued end.** SACM-MULT-001 is at the
///   second; the whole attribute is kept and the end is empty.
/// - **An empty `xmi:id`.** An element is read with a generated id, and no
///   SACM-XMI-004; a `name`, `content`, `key` or `value` with none. One in
///   preserved content is kept as written, outside the id space.
/// - **Also tolerant only:** SACM-XMI-008 and SACM-XMI-002 (see
///   [Other SACM namespaces](#other-sacm-namespaces)), and SACM-XMI-015 and
///   SACM-XMI-017 (see [The EMF dialect](#the-emf-dialect)).
/// - **Otherwise the same document.** Apart from these constructs, both modes
///   give the same document, preserved content included.
///
/// # Limits
///
/// Both modes read under the [`Limits`] of [`ReadOptions::with_limits`]
/// (LIB-SEC-003, LIB-SEC-007). A breach is [`ReadError::LimitExceeded`]
/// (SACM-SEC-003). So the memory a read takes is bounded by the limits, not by
/// the input's shape.
///
/// | Limit | Default | Checked |
/// |---|---|---|
/// | input size | 32 MiB | first, on the bytes |
/// | nesting depth | 256 | by one linear scan before parsing |
/// | namespace bindings in scope at an element | 256 | by the scan |
/// | attributes on an element | 256 | by the scan |
/// | XML nodes | 1,000,000 | by the parser |
///
/// - **The scan also counts** the bindings in scope at the elements that declare
///   one, against the node limit. The first namespace declaration of each prefix
///   is not counted.
/// - **One budget for what a read keeps** besides the parsed tree, against the
///   size limit (ADR 0041 amendment 1). The read is refused at the element,
///   attribute or diagnostic where the budget passes the limit.
/// - **Kept namespace text** (`SACM-SEC-003/kept-namespaces`): each namespace URI
///   of a kept name, and `prefix + URI + 10` for each binding preserved content
///   keeps, counted as each element or attribute is kept.
/// - **Kept diagnostic text** (`SACM-SEC-003/kept-text`): the text of each
///   diagnostic the read emits, the bytes of its text arguments and of its
///   affected ids, counted where it is emitted.
/// - **Rendered messages are not counted.** One repeats its text arguments,
///   escaped, up to six times their size, and a few templates render an argument
///   twice. So the diagnostics take at most about seven times what the budget
///   counts.
///
/// The peak memory of a read at the defaults, the input included, was measured
/// (slice 0072: a release build, rustc 1.100, Windows 11, an AMD Ryzen 9 8945HS
/// with 31 GiB; the peak working set of the process).
///
/// | Input | Peak | Time |
/// |---|---|---|
/// | a flat ArgumentPackage of 341,176 Claims, just under 32 MiB | 811 MiB | 1.9 s |
/// | 999,998 preserved `<x/>`, the most the node limit lets through: 4 MB, whose diagnostics keep 18 MB of text | 1.25 GiB | 1.7 to 2.3 s |
/// | a root with 255 bindings whose 3,905 Claims declare one more each, up to the namespace-copy limit: 281 KB | 15 MiB | 0.24 s |
///
/// - **The second** is mostly the SACM-XMI-003 Warning each preserved element gets.
/// - **Less memory than that?** An application that reads untrusted files sets
///   lower limits.
///
/// # Other SACM namespaces
///
/// No SACM revision sets a namespace for its documents (spec defect D7), so files
/// use many (LIB-COMPAT-004; ADR 0041 D4, D5). [`write()`] writes SACM 2.3 in the
/// pinned namespace.
///
/// | A root in | Strict | Tolerant |
/// |---|---|---|
/// | the pinned namespace `http://www.omg.org/spec/SACM/20220301` | SACM 2.3 | SACM 2.3 |
/// | an older SACM revision's namespace it recognises | [`ReadError::Unsupported`] | read as SACM 2.3, with one SACM-XMI-008 Warning |
/// | any other namespace whose URI contains `sacm` in any letter case, such as the undated `http://www.omg.org/spec/SACM/2.3` | [`ReadError::Unsupported`] | read as SACM 2.3, with one SACM-XMI-002 Warning |
/// | the SACM 2.4 namespace (`…/20260504` or `…/2.4`) | refused with SACM-XMI-008, until LIB-COMPAT-018 decides how it is read | the same |
/// | no namespace, or one without `sacm` | [`ReadError::InvalidRoot`] | the same |
///
/// - **The older revisions' namespaces:** `http://www.omg.org/spec/SACM/20170843`,
///   `/20190235` and `/20210218` (2.0, 2.1, 2.2); `http://www.omg.org/spec/SACM/2.0`,
///   `/2.1` and `/2.2`; `http://omg.sacm/2.0`, `/2.1` and `/2.2`; and
///   `http://SACM_ACEditor/sacm/2.1`.
/// - **With a package.** Each is recognised alone or followed by `/` and a package
///   name of ASCII letters.
/// - **`occurence`.** In such a root, an Event's `occurence`, the 2.0 to 2.2
///   spelling, is read as its date, with one SACM-XMI-008 Warning more.
/// - **Class names.** In a root, the namespaces of its family name SACM classes:
///   the base and, for a recognised revision or the undated 2.3, the base with a
///   package. A type in any other namespace is foreign (SACM-XMI-003).
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
/// Most SACM tools are built on EMF, and a tolerant read reads its dialect
/// (LIB-COMPAT-002; ADR 0044 D1, D2). Both saves write XMI 2.5.1 and ids. EMF:
///
/// - **binds `xmi` to `http://www.omg.org/XMI`,** no XMI version's namespace (XMI
///   2.5.1 §7.5.3);
/// - **names same-document targets by fragment paths,** such as
///   `//@argumentPackage.0/@argumentElement.3` or `/1`, instead of `xmi:id`s
///   (§7.10.2).
///
/// EMF's XMI namespace, in a tolerant read:
///
/// - **Read as XMI 2.5.1's:** the `XMI` element, and the attributes `id`, `type`,
///   `idref`, `version`, `uuid` and `label`, with one SACM-XMI-015 Warning per
///   document, at the first name in that namespace.
/// - **Any other name in it** is kept as written (SACM-XMI-003).
/// - **Both namespaces.** An element carrying the same XMI attribute in both reads
///   XMI 2.5.1's and keeps EMF's as written (LIB-COMPAT-019).
/// - **A kept type.** An element that keeps a `type` in EMF's namespace or an
///   `xsi:type` (only a root keeps one), and none in XMI 2.5.1's, is written with
///   its own `xmi:type`.
///
/// Fragment paths, in a tolerant read:
///
/// - **Where.** A token of a reference end's attribute, the fragment of a
///   same-document `href`, and an `ExpressionLangString`'s `expression`, when it
///   begins with `/` and equals no `xmi:id`.
/// - **Read as** the id of the element it names: an element read as an element,
///   the root of a resolved same-file proxy, or a preserved element with an
///   `xmi:id`.
/// - **Reported** by one SACM-XMI-017 Warning per document, which counts them.
/// - **A path that names nothing** is kept as written, and
///   [`validation::validate`](crate::validation::validate) reports it
///   (SACM-REF-001).
/// - **Never rewritten:** paths inside preserved content.
///
/// What is refused:
///
/// - **Strict** refuses any name in `http://www.omg.org/XMI` as
///   [`ReadError::Unsupported`] (SACM-XMI-015), and resolves no path.
/// - **Except** inside the content of an `xmi:Extension` or of a Difference's
///   `target`, which XMI.xsd leaves open: both modes keep it as written.
/// - **Both modes** refuse an `XMI` element in any other namespace, or in none.
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
/// 1. **Size.** The input's size (`LimitExceeded`).
/// 2. **Byte order mark and first bytes.** UTF-16 with a mark is decoded; UTF-16
///    without one, UTF-32 and UCS-4 are `Unsupported`.
/// 3. **XML declaration.** The grammar of its `<?xml`, `version` and `encoding`
///    (`Malformed`).
/// 4. **Declared encoding,** as the first table below. Then, for an accepted one,
///    the rest of the declaration: `standalone` and `?>` (`Malformed`).
/// 5. **ASCII.** Under an ASCII declaration, a non-ASCII character (`Malformed`).
/// 6. **Encoding** (`Malformed`). Invalid UTF-8, at the first invalid byte. In
///    UTF-16, an unpaired surrogate, at the surrogate, or an odd byte count, after
///    the last whole character. ISO-8859-1 has none.
/// 7. **The scan before parsing** (`LimitExceeded`), at each start tag in document
///    order: the depth, the bindings and the attributes, as [Limits](#limits)
///    counts them. The scan stops at a DOCTYPE.
/// 8. **The parse.** A DOCTYPE (`Doctype`), the node limit (`LimitExceeded`), or a
///    well-formedness error (`Malformed`), whichever comes first.
/// 9. **Late well-formedness errors** (`Malformed`), the earliest first: those the
///    XML parser accepts and this release refuses, listed below.
/// 10. **Root names and namespaces,** in the order listed below.
/// 11. **EMF names,** in strict mode (`Unsupported`): the first element or
///     attribute name in `http://www.omg.org/XMI`, in document order, before any
///     id is read (see [The EMF dialect](#the-emf-dialect)).
/// 12. **Content,** in document order: the `xmi:XMI` element's attributes, each
///     root with its content, and the content beside it, by the rules below.
/// 13. **C++ shapes.** In strict mode, the first C++ shape in the input
///     (`LegacyShape`).
///
/// Step 4, the declared encoding:
///
/// | Byte order mark | Declared | Result |
/// |---|---|---|
/// | no UTF-16 mark | `ISO-8859-1`, in any letter case | decoded as ISO-8859-1, and the declaration is checked again; `Malformed` after a UTF-8 mark |
/// | no UTF-16 mark | another, neither UTF-8 nor ASCII | `Unsupported` |
/// | a UTF-16 mark | another than UTF-16, UTF-16LE or UTF-16BE | `Malformed` |
///
/// Step 9, the late well-formedness errors:
///
/// - a character reference to a surrogate or above U+10FFFF;
/// - a declaration of the namespace prefix `xmlns` (`xmlns:xmlns`);
/// - a namespace declaration repeated in one start tag (`xmlns`, or `xmlns:xml`,
///   twice);
/// - a namespace prefix undeclared with `xmlns:p=""`;
/// - an element or attribute name with an empty prefix (`<:c>`, `</:c>`,
///   ` :a="1"`);
/// - a processing instruction whose target (the XML Name after `<?`) is `xml` in
///   any letter case, contains a colon, or is followed by neither white space
///   nor `?>`.
///
/// Within the W3C XML Conformance Test Suite (20130923) this list is closed: every
/// not-well-formed XML 1.0 and Namespaces 1.0 case in it is refused. Outside it,
/// other leniencies of the parser may remain: a sweep (LIB-SEC-005) checks only
/// that these checks agree with the parser's tree.
///
/// Step 10, the roots' names and namespaces:
///
/// 1. an `XMI` element in neither XMI 2.5.1's namespace nor
///    `http://www.omg.org/XMI` is `Unsupported`; so, in strict mode, is one in
///    `http://www.omg.org/XMI`;
/// 2. any other element of either namespace as the document element is
///    `InvalidRoot`;
/// 3. an `xmi:XMI` element with no root is `NoPackage`;
/// 4. the first root not named as a package kind, or in no namespace or one that
///    does not name SACM, is `InvalidRoot`;
/// 5. the first root in the SACM 2.4 namespace, and in strict mode in an older
///    SACM revision's or a SACM-like one, is `Unsupported` (see
///    [Other SACM namespaces](#other-sacm-namespaces)).
///
/// Step 12, the content:
///
/// - **Within an element.** Its start tag (its role, type, kind and id) is
///   checked before its other attributes, and those before its content.
/// - **Ids.** An empty id is `InvalidId`. An id whose value an earlier element,
///   name, `content`, `key`, `value` or preserved element carries is
///   `DuplicateId`, so a repeat is refused before anything after its id.
/// - **Literals.** A value that is not a literal is `InvalidLiteral` where it is
///   met.
/// - **Repeats.** A `name`, `content`, `key`, value or reference element is
///   checked whole when it is met, then, unless it is preserved, against the
///   occurrences before it.
/// - **So** the second occurrence of a value is `RepeatedValue` unless it is
///   itself refused or preserved first. A later occurrence is listed in its
///   [`locations`](ReadError::locations) even if it does not conform itself.
/// - **Nowhere to be kept.** Content on or beside the roots that no element can
///   hold is `Unsupported`.
/// - **The budget.** Kept namespace text and diagnostic text count against the
///   size limit as [Limits](#limits) says (`LimitExceeded`).
///
/// The refusals. A tolerant read gives only those marked both; it keeps every
/// other construct, with a Warning (see [Diagnostics](#diagnostics)):
///
/// | Refusal | Code | Modes | When |
/// |---|---|---|---|
/// | [`ReadError::Doctype`] | SACM-SEC-001 | both | the input has a DOCTYPE, with or without entity declarations |
/// | [`ReadError::Malformed`] | SACM-XML-001 | both | the input is not well-formed XML, is truncated, is not valid UTF-8 or UTF-16, declares ASCII but holds a non-ASCII character, has a UTF-16 byte order mark but declares another encoding, or has a UTF-8 byte order mark but declares ISO-8859-1 |
/// | [`ReadError::InvalidRoot`] | SACM-XMI-001 | both | a root's local name is not one of the twelve SACM package kinds; the root is in no namespace or one whose URI does not contain `sacm`; or the document element is in an XMI namespace (XMI 2.5.1's or `http://www.omg.org/XMI`) but is not `xmi:XMI` |
/// | [`ReadError::NoPackage`] | SACM-XMI-001 | both | the `xmi:XMI` element holds no root: no element child outside the XMI namespace |
/// | [`ReadError::DuplicateId`] | SACM-ID-001 | both | two or more elements (names, `content`s, `key`s, `value`s and preserved elements included) carry the same `xmi:id` (XMI 2.5.1 §7.6.1), compared by `xsd:ID` value (see [Ids](#ids)). It names every element that carries the first id to repeat |
/// | [`ReadError::LimitExceeded`] | SACM-SEC-003 | both | the input passes one of the [`Limits`] (see [Limits](#limits)) |
/// | [`ReadError::InvalidId`] | SACM-ID-002 | strict | the `xmi:id` of an element, of a `name`, `content`, `key`, `location` or `value`, or of an element of preserved content, is empty |
/// | [`ReadError::RepeatedValue`] | SACM-MULT-001 | strict | a property is given more often than XMI allows, even with equal values; it gives every occurrence |
/// | [`ReadError::InvalidLiteral`] | SACM-ENUM-001 | strict | an `assertionDeclaration` is not exactly one of the six literals; an `isCitation`, `isAbstract` or `isCounter`, or a value element's `xsi:nil`, is not `true`, `false`, `1` or `0`, white space around them aside. It holds the value as read |
/// | [`ReadError::MissingType`] | SACM-XMI-005 | strict | a child under a role whose declared type is abstract has no `xmi:type` or `xsi:type`, and no `xmi:idref` or `href`, which would make it a proxy |
/// | [`ReadError::LegacyShape`] | SACM-XMI-012 | strict | a C++ name or text shape (see [The C++ name and text shapes](#the-c-name-and-text-shapes)) |
/// | [`ReadError::Unsupported`] | by input, below | by input, below | well-formed input a strict read does not read and does not keep as preserved content (LIB-API-011) |
///
/// - **An id that is not empty** is read by its `xsd:ID` value. One that is still
///   no XML `NCName`, such as `1c` or `a b`, is reported by
///   [`validation::validate`](crate::validation::validate), and refused by a
///   strict [`write()`].
/// - **`RepeatedValue`, single-valued:** an attribute and an element, which XMI
///   2.5.1 §9.5.2 rule 2h forbids, two or more elements, or two or more ids in
///   the attribute of a single-valued end.
/// - **`RepeatedValue`, multi-valued:** an end such as `metaClaim` or `source`
///   given both as an attribute and as elements.
/// - **Its properties** are listed on the variant. Preserved content is never an
///   occurrence.
///
/// [`ReadError::Unsupported`], by input (LIB-API-017). Its reason ends with the
/// label, and its code is the one a tolerant read gives the same input:
///
/// | Input | Label | Code | A tolerant read |
/// |---|---|---|---|
/// | UTF-16 without a byte order mark, UTF-32 or UCS-4 input; a declared encoding other than UTF-8, ASCII or ISO-8859-1 in input without a UTF-16 mark | `(a limit of this version)` | SACM-XML-002 | refuses it |
/// | more than 65,535 distinct namespace bindings or 2³² attributes | `(a limit of this version)` | SACM-SEC-002 | refuses it |
/// | a root in the SACM 2.4 namespace (LIB-COMPAT-018) | `(a limit of this version)` | SACM-XMI-008 | refuses it |
/// | an `XMI` element in neither XMI 2.5.1's namespace nor EMF's `http://www.omg.org/XMI`, such as XMI 2.4.1's, or in none | `(a limit of this version)` | SACM-XMI-015 | refuses it |
/// | a root in an older SACM revision's namespace or a SACM-like one (LIB-COMPAT-004) | `(strict mode never reads it; ROW reads it in tolerant mode)` | SACM-XMI-008 or SACM-XMI-002 | reads it |
/// | any name in EMF's XMI namespace (LIB-COMPAT-002) | `(strict mode never reads it; ROW reads it in tolerant mode)` | SACM-XMI-015 | reads it |
/// | a defect with nowhere to be kept: an attribute, text, or an XMI-namespace element XMI 2.5.1 does not define there, on or in `xmi:XMI`; an attribute that would be preserved but whose prefix `sacm` or `xmi` is bound to another namespace (see [Preserved content](#preserved-content)) | `(a defect in the input)` | SACM-XMI-003 | keeps it |
///
/// - **Not a defect.** Input a tolerant read reads need not be defective: a
///   document in another SACM namespace is not (D-NS-PIN).
/// - **No conforming input is refused as not read yet.** XMI metadata and every
///   link form are read (SACM23-XMI-019, SACM23-XMI-029).
/// - **The spellings are no refusal.** A strict read keeps each as preserved
///   content, with a SACM-XMI-013 Error (see [Spellings](#spellings)).
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
/// | | [`Strict`](Self::Strict) | [`Tolerant`](Self::Tolerant) |
/// |---|---|---|
/// | A read | reports what it reads as a variation or keeps as preserved content as an Error, and refuses the rest | opens a well-formed SACM file, keeping what a strict read refuses, with a Warning and never an Error |
/// | A write | writes only schema-valid output | writes the document as it is held, preserved content included |
///
/// A strict read:
///
/// - **Errors:** SACM-XMI-010 for an `xsi:type`, SACM-XMI-003 for preserved
///   content, and SACM-XMI-013 for a name spelled as in the other of the text and
///   the model (SACM23-XMI-009).
/// - **Refuses** the C++ name and text shapes ([`ReadError::LegacyShape`]), a value
///   that is no literal, a value given twice, an untyped child under an abstract
///   role, an empty `xmi:id`, and what the reader cannot represent.
/// - **Does not validate.** To check a file, test [`ReadOutput::has_errors`] on the
///   read, and [`diagnostic::has_errors`](crate::diagnostic::has_errors) on the
///   findings of [`validation::validate`](crate::validation::validate).
///
/// A tolerant read:
///
/// - **Opens** a well-formed file whose roots are all SACM packages, in SACM 2.3,
///   an older revision's or a SACM-like namespace (LIB-COMPAT-004).
/// - **Still refuses** a DOCTYPE, a SACM 2.4 root, an `XMI` element in a namespace
///   other than XMI 2.5.1's or EMF's `http://www.omg.org/XMI`, a repeated
///   `xmi:id`, an encoding it cannot decode, and input over the parser's capacity
///   or its [`Limits`].
/// - **Reads the dialect EMF writes:** its XMI namespace `http://www.omg.org/XMI`
///   and its fragment paths (LIB-COMPAT-002).
/// - **Reads declared subclasses:** elements typed with classes the application
///   declares subclasses of SACM 2.3 classes ([`ReadOptions::with_subclass`],
///   LIB-COMPAT-017).
///
/// A write:
///
/// - **Strict** refuses a document that holds preserved content
///   ([`WriteError::PreservedContent`]), then one with an `xmi:id` that is no
///   `NCName` or a reference to no `xmi:id` it writes ([`WriteError::Invalid`]).
/// - **Never validates.** A draft with an open model Error, such as a package
///   with no name, writes.
/// - **Same bytes.** The two modes write the same bytes for a document both write.
///
/// `Mode` has no default of its own: [`ReadOptions`] and [`WriteOptions`] both
/// default to `Strict`. Later releases may add variants, so match it with a
/// fallback arm.
///
/// # Examples
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
    /// Aims at SACM 2.3 conformance: a read refuses what it does not read, and a
    /// write refuses output that would not be schema-valid.
    Strict,
    /// Opens a well-formed SACM file: a read keeps what a strict read refuses,
    /// with a Warning, and a write writes the document as it is held.
    Tolerant,
}

impl Mode {
    /// The mode's name in lower case, `"strict"` or `"tolerant"`: what a
    /// command line or a settings file spells it, and what it displays as.
    ///
    /// # Examples
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

/// How [`read`] reads a document.
///
/// | Option | Default | Set with |
/// |---|---|---|
/// | the [`Mode`] | strict | [`with_mode`](Self::with_mode) |
/// | the [`Limits`] (LIB-SEC-003) | the default limits | [`with_limits`](Self::with_limits) |
/// | the classes outside SACM 2.3 declared subclasses of SACM 2.3 classes (LIB-COMPAT-017) | none | [`with_subclass`](Self::with_subclass) |
///
/// Options compare and hash by content: the order in which declarations are
/// made does not matter.
///
/// # Examples
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
    ///
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
    /// # Examples
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

    /// These options, declaring the class `name` of the namespace `namespace` a
    /// subclass of the SACM 2.3 class `kind` (LIB-COMPAT-017, ADR 0044 D3).
    ///
    /// - **Tolerant only.** A strict read ignores every declaration; ask for a
    ///   tolerant read with [`with_mode`](Self::with_mode).
    /// - **What it reads.** A child element typed with that class (`xmi:type` or
    ///   `xsi:type`, matched by namespace URI and local name), under a role that
    ///   admits `kind`, as an element of `kind`.
    /// - **The class is kept** as preserved content of the element, with a
    ///   SACM-XMI-003 Warning.
    ///   [`SacmElement::subclass`](crate::model::SacmElement::subclass) gives it.
    /// - **Writing.** A tolerant write writes it back as the element's `xmi:type`;
    ///   a strict write refuses the element.
    /// - **A package root** keeps a type naming a declared subclass of its own
    ///   class the same way. Its class still comes from its element name.
    /// - **Never applies** to a proxy, to an element that carries a type in both
    ///   XMI namespaces, or to a type in the root's SACM namespaces or in SACM
    ///   2.3's. An empty `namespace` matches no type.
    /// - **Declared twice.** A later declaration of the same class replaces this
    ///   one.
    /// - **Why a tolerance.** SACM 2.3's own extension mechanism is the
    ///   `TaggedValue` (§8.12). A file typed with foreign classes is not SACM 2.3
    ///   XMI (XMI 2.5.1 §7.8.6, Annex B.2).
    ///
    /// # Examples
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

    /// The SACM 2.3 class that [`with_subclass`](Self::with_subclass) declared
    /// the class `name` of the namespace `namespace` a subclass of, if any.
    ///
    /// # Examples
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
/// - **The document holds all of the input's content.** `read` keeps what it
///   does not read as preserved content, of an element or of the document, or,
///   in strict mode, refuses it. It drops nothing.
/// - **The diagnostics** are sorted by [`Diagnostic`]'s order, located ones
///   first. [`read`'s Diagnostics](fn@read#diagnostics) lists each code.
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
    ///
    /// Empty for an input in which all of these hold:
    ///
    /// - no `xsi:type`, no `xmi:version` and no C++ shape;
    /// - no content that is not SACM 2.3, and nothing a strict read refuses;
    /// - every element has an `xmi:id`, and no `xmi:id`, of an element or a
    ///   `name`, has white space around it.
    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Whether any of the read's [`diagnostics`](Self::diagnostics) is an Error.
    ///
    /// - **The same as**
    ///   [`diagnostic::has_errors`](crate::diagnostic::has_errors)`(self.diagnostics())`.
    /// - **Only a strict read reports Errors:** SACM-XMI-010 for an `xsi:type`,
    ///   SACM-XMI-003 for content that does not conform where it stands, and
    ///   SACM-XMI-013 for a spelling.
    /// - **Always `false` for a tolerant read,** which reports what a strict read
    ///   refuses as Warnings.
    /// - **No conformance check on its own.** Run
    ///   [`validation::validate`](crate::validation::validate) on the document
    ///   and test its findings with
    ///   [`diagnostic::has_errors`](crate::diagnostic::has_errors).
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
    /// # Examples
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
/// - **A defect in the input** has its own variant and catalog code.
/// - **[`Unsupported`](Self::Unsupported)** is well-formed input this version
///   does not read. Its reason says why with a label, and its code is the one a
///   tolerant read gives the same input (LIB-API-017).
/// - **Which mode refuses what:** the table under [`read`'s Errors](fn@read#errors).
///   A tolerant read keeps what only a strict read refuses, with a Warning of
///   the variant's code (ADR 0035 D1), and reads a C++ shape.
/// - **Wording it yourself.** Every refusal has a catalog [`code`](Self::code), a
///   message [`template`](Self::template) and its
///   [`arguments`](Self::arguments), so an application can word it in any
///   language (ADR 0030 amendment 3). The catalog lists each template.
///
/// Locations:
///
/// - **Every refusal has a [`SourceLocation`]** except the input-size and node
///   limits and the parser's capacity. [`locations`](Self::locations) gives every
///   location a refusal names.
/// - **Lines** end as XML 1.0 §2.11 ends them: a line feed, a carriage return
///   and line feed, or a lone carriage return.
/// - **Columns** count characters, in UTF-16 input too. A leading byte order
///   mark, UTF-8 or UTF-16, takes no column.
///
/// With the `serde` feature:
///
/// - **It serializes** as `{"type", "code", "template", "arguments", …its fields}`.
/// - **It reads back as a record.** Its `Display` renders its template and
///   arguments, and its other fields are kept as given, unchecked against them.
/// - **So** what the fields promise, such as `reason` ending with its label or
///   `locations` holding at least two, holds for a refusal of [`read`], not for
///   a record.
///
/// # Display
///
/// `Display` writes one line that starts in lower case, without trailing
/// punctuation.
///
/// - **The location** follows as ` at line L, column C` when it is known, and the
///   reason after `: `.
/// - **Several locations.** A [`DuplicateId`](Self::DuplicateId) lists every one,
///   separated by `; `, and a [`RepeatedValue`](Self::RepeatedValue) its first
///   two, so the line stays short however many there are.
/// - **Names from the input** are quoted with `{:?}`, so line breaks in them never
///   break the line.
///
/// # Examples
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
    /// The input is not well-formed XML, or its encoding is broken (SACM-XML-001).
    ///
    /// - **Not well-formed,** or truncated.
    /// - **Not valid** UTF-8 or UTF-16.
    /// - **Declared ASCII,** but it holds a non-ASCII character.
    /// - **A UTF-16 byte order mark,** but it declares another encoding.
    /// - **A UTF-8 byte order mark,** but it declares ISO-8859-1.
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
    /// A root is no SACM package (SACM-XMI-001; ADR 0041 D4). The location is the
    /// `<` of its start tag.
    ///
    /// - **A root** is the document element, or each element child outside the
    ///   XMI namespace of an `xmi:XMI` element.
    /// - **Its name** is not one of the twelve SACM package kinds; or
    /// - **its namespace** is none, or one whose URI does not contain `sacm` in
    ///   any letter case; or
    /// - **the document element** is in an XMI namespace but is not `xmi:XMI`.
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
    /// `target` (SACM-ID-001, SACM23-VAL-001).
    ///
    /// - **Which elements.** Two elements, two names, or an element and a name
    ///   (ADR 0024).
    /// - **Why refused.** XMI 2.5.1 §7.6.1 requires the value to be unique across
    ///   all elements within the file, and a [`Document`] holds each id once. No
    ///   element is dropped or renamed instead.
    /// - **Comparison** is by `xsd:ID` value. XML white space around an `NCName`
    ///   is trimmed first, so `" c1 "` and `c1` repeat. Otherwise ids are exact
    ///   strings, so `c1` and `C1` differ.
    /// - **A generated id** never repeats one.
    /// - **Which id.** The first that repeats in document order.
    ///   [`location`](Self::location) is that first repeat, and
    ///   [`locations`](Self::locations) every element carrying the id.
    ///
    /// # Examples
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
        /// The `<` of the start tag of every element, a `name` included, whose
        /// `xmi:id` has the value, in document order: at least two.
        locations: Vec<SourceLocation>,
        /// The template and arguments of the refusal's message: see
        /// [`template`](Self::template) and [`arguments`](Self::arguments).
        details: Details,
    },
    /// In strict mode, the `xmi:id` of an element or of a `name` is empty
    /// (SACM-ID-002, SACM23-VAL-020). The location is the `<` of its start tag.
    ///
    /// - **Why refused.** An empty id is no XML `NCName`, and a [`Document`]
    ///   cannot hold it.
    /// - **Tolerant** gives such an element a generated id, and such a `name`
    ///   none, with SACM-ID-002 as a Warning.
    /// - **Not this refusal.** An `NCName` with XML white space around it, such
    ///   as `" c1 "`, is read trimmed (SACM23-XMI-025). Other ids that are no
    ///   `NCName`s are read as written, and
    ///   [`validation::validate`](crate::validation::validate) reports them.
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
    /// Well-formed input this version does not read, and that a strict read does
    /// not keep as preserved content (LIB-API-011, ADR 0033).
    ///
    /// The reason ends with exactly one label. The [`code`](ReadError::code) is
    /// the one a tolerant read gives the same input (LIB-API-017; ADR 0035 D1, D2):
    ///
    /// | Label | Input | Code | A tolerant read |
    /// |---|---|---|---|
    /// | `(a limit of this version)` | an encoding | SACM-XML-002 | refuses it |
    /// | `(a limit of this version)` | over the parser's capacity | SACM-SEC-002 | refuses it |
    /// | `(a limit of this version)` | a SACM 2.4 root | SACM-XMI-008 | refuses it |
    /// | `(a limit of this version)` | an `XMI` element in neither XMI 2.5.1's namespace nor `http://www.omg.org/XMI` | SACM-XMI-015 | refuses it |
    /// | `(strict mode never reads it; ROW reads it in tolerant mode)` | a root in an older SACM revision's or a SACM-like namespace (LIB-COMPAT-004) | SACM-XMI-008 or SACM-XMI-002 | reads it, with a Warning of that code |
    /// | `(strict mode never reads it; ROW reads it in tolerant mode)` | a name in EMF's XMI namespace `http://www.omg.org/XMI` (LIB-COMPAT-002) | SACM-XMI-015 | reads it, with a Warning of that code |
    /// | `(a defect in the input)` | input that does not conform and has no element to hold it, such as an attribute on `xmi:XMI` | SACM-XMI-003 | keeps it, with a Warning of that code |
    ///
    /// - **ROW** is the matrix row a tolerant read reads the input under. Such
    ///   input need not be defective: a document in another SACM namespace is not
    ///   (D-NS-PIN).
    /// - **`(a limit of this version; ROW reads it)`** marked conforming input a
    ///   row was to read. No input gives it in this release.
    /// - **Otherwise never refused.** Content that does not conform where it
    ///   stands is preserved by [`read`].
    ///
    /// # Examples
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
    /// In strict mode, a property is given more often than XMI allows
    /// (SACM-MULT-001), even when the values are equal.
    ///
    /// | Property | Refused when given | Fix |
    /// |---|---|---|
    /// | single-valued | as an attribute and an element, as two or more elements, or as two or more ids in a reference attribute | give the value once |
    /// | a multi-valued reference end | both as an attribute and as elements | give every id in the attribute, or each in its own element |
    ///
    /// - **Why.** XMI 2.5.1 §9.5.2 rule 2h serializes a property "either as an XML
    ///   attribute or as XML elements but not both", and a single-valued property
    ///   holds one value. No value is dropped instead.
    /// - **Tolerant** reads the first and keeps each later occurrence as preserved
    ///   content, with SACM-MULT-001 as a Warning at it.
    /// - **The multi-valued ends** are those [`ReferenceEnd`] says take several
    ///   ids, such as `metaClaim` and `source` (ADR 0027, ADR 0028, ADR 0034), and
    ///   an `ArtifactAssetRelationship`'s `source` and `target` (§12.14,
    ///   SACM23-ART-010).
    /// - **By kind.** `Display` calls a property single-valued only when the end
    ///   of that name on an element of the variant's `kind` is: an
    ///   `AssertedRelationship`'s `target` is, an `ArtifactAssetRelationship`'s is
    ///   not.
    /// - **A `name`** (`name:LangString[1]`, formal/23-05-08 §8.6) is single-valued
    ///   too: two `name` elements in no namespace, or the C++ `name` attribute and
    ///   a `name` element (LIB-COMPAT-008), are refused (SACM23-XMI-021).
    /// - **So are** a `UtilityElement`'s `content` (§8.7 `[0..1]`) and a
    ///   `TaggedValue`'s `key` (§8.12 `[1]`), which are never merged.
    /// - **Locations.** [`location`](Self::location) is the second occurrence, and
    ///   [`locations`](Self::locations) every occurrence.
    /// - **`Display`** names the count and the first two: `at {first}; {second}`
    ///   for two occurrences, and `first at {first}; {second}` for more.
    ///
    /// # Examples
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
        /// The element that holds the property, by its `xmi:id` or its generated
        /// id.
        ///
        /// For `name.lang`, `content.value.lang` and the like, it is the element
        /// the name or the value belongs to.
        element: ElementId,
        /// The kind of `element`.
        kind: ElementKind,
        /// The property, by its name.
        ///
        /// | Group | Names |
        /// |---|---|
        /// | values | `gid`, `isCitation`, `isAbstract`, `isCounter`, `assertionDeclaration`, `version`, `date`, `startTime`, `endTime`, `value`, `externalReference` |
        /// | reference ends | `citedElement`, `abstractForm`, `metaClaim`, `reasoning`, `source`, `target` (an asserted relationship's, or an `ArtifactAssetRelationship`'s), `structure`, `referencedArtifactElement`, `category`, `element`, `origin` |
        /// | group ends | `argumentElement` (an `ArgumentGroup`'s), `artifactElement` (an `ArtifactGroup`'s), `terminologyElement` (a `TerminologyGroup`'s) |
        /// | names and texts | `name`, `name.lang`, `name.content`, `content`, `content.value.lang`, `content.value.content`, `key`, `key.value.lang`, `key.value.content`, `location`, `location.value.lang`, `location.value.content` |
        property: String,
        /// Every occurrence, in document order: at least two.
        ///
        /// - **An attribute** is at the first character of its name, an element
        ///   at its `<`.
        /// - **Two or more ids in one reference attribute** give one location per
        ///   id, at the id's first character as written in the value, past any
        ///   white space or character reference before it.
        locations: Vec<SourceLocation>,
        /// The template and arguments of the refusal's message: see
        /// [`template`](Self::template) and [`arguments`](Self::arguments).
        details: Details,
    },
    /// In strict mode, a value is not one of its type's literals (SACM-ENUM-001).
    ///
    /// | Value | Literals | Fix |
    /// |---|---|---|
    /// | an `assertionDeclaration` (formal/23-05-08 §11.8; SACM23-VAL-006) | exactly one of the six [`AssertionDeclaration`](crate::model::AssertionDeclaration) literals: `asserted`, `needsSupport`, `assumed`, `axiomatic`, `defeated`, `asCited` | write the literal exactly, letter case included, with no white space around it |
    /// | an `isCitation` or `isAbstract` (§8.2), or an `isCounter` (§11.13) (SACM23-BASE-007, SACM23-BASE-008, SACM23-ARG-007) | an `xsd:boolean`: `true`, `false`, `1` or `0`, XML white space around it allowed | write `true` or `false` |
    /// | the `xsi:nil` of a value element (SACM23-XMI-026) | an `xsd:boolean`, as the flags are | write `true` or `false` |
    ///
    /// - **Never read as the default.** Reading a declaration as `asserted` would
    ///   silently change what the case declares (ADR 0021 amendment 2). `TRUE`,
    ///   `yes` and the empty value are never read as false (ADR 0026).
    /// - **Tolerant** keeps the attribute or value element as preserved content,
    ///   with SACM-ENUM-001 as a Warning, and holds the default (ADR 0035).
    /// - **`Display`** gives the literals the property accepts as its reason.
    ///
    /// # Examples
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
        /// The value as read.
        ///
        /// - **An attribute's:** after XML attribute-value normalisation (XML 1.0
        ///   §3.3.3).
        /// - **A value element's:** its exact character content.
        value: String,
        /// The first character of the attribute's name, or the `<` of the
        /// value element's start tag.
        location: Option<SourceLocation>,
        /// The template and arguments of the refusal's message: see
        /// [`template`](Self::template) and [`arguments`](Self::arguments).
        details: Details,
    },
    /// A strict read met a name or a text written in a shape of the
    /// assurance-forge C++ library (SACM-XMI-012, LIB-COMPAT-008).
    ///
    /// - **The shapes,** such as a `name="…"` attribute or text in a `name`:
    ///   [The C++ name and text shapes](fn@read#the-c-name-and-text-shapes).
    /// - **Strict** never applies a tolerance (ADR 0022 amendment 1). It refuses
    ///   the first shape, once the rest of the input has read without a refusal.
    /// - **Tolerant** reads each shape as its standard form, with a Warning.
    /// - **Fix:** read the file in tolerant mode and write it again, which writes
    ///   the standard form.
    ///
    /// # Examples
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
        /// Where the shape is, by the first of these that applies.
        ///
        /// 1. Its first attribute's name.
        /// 2. Its first text character that is not literal white space: a CDATA
        ///    section or reference at its start.
        /// 3. For a `value` in a `name`, the `<` of the `value`.
        location: Option<SourceLocation>,
        /// The template and arguments of the refusal's message: see
        /// [`template`](Self::template) and [`arguments`](Self::arguments).
        details: Details,
    },
    /// In strict mode, a child under a containment role whose declared type is
    /// abstract carries no `xmi:type` or `xsi:type` (SACM-XMI-005, LIB-COMPAT-010).
    ///
    /// - **Such as** an `<argumentElement>` with none in an `ArgumentPackage`.
    /// - **Why.** Its element name is its role, so its class cannot be inferred
    ///   (XMI 2.5.1 §7.6.3), and none is guessed.
    /// - **Tolerant** keeps the child whole as preserved content of its parent,
    ///   with SACM-XMI-005 as a Warning (ADR 0035).
    /// - **Fix:** give it an `xmi:type` naming its class, such as
    ///   `xmi:type="sacm:Claim"`.
    /// - **Not this refusal:** a child with an `xmi:idref` or an unqualified
    ///   `href`. It is the reference form of a composite, a proxy, which
    ///   [`read`] reads.
    /// - **Rightly this refusal:** a child with only an `xlink:href` or an
    ///   `xmi:href`. Only an unqualified `href` is a reference form (XMI.xsd
    ///   `LinkAttribs`).
    ///
    /// # Examples
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
    /// In either mode, the input passes one of the [`Limits`] of
    /// [`ReadOptions::with_limits`] (SACM-SEC-003, LIB-SEC-003, LIB-SEC-007).
    ///
    /// Fix: if the file is trusted, read it with higher limits.
    ///
    /// # Examples
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
        /// Where the limit is passed; `None` for the input's size and the node
        /// count.
        ///
        /// - **Depth, bindings in scope or namespace copies:** the `<` of the
        ///   element.
        /// - **Kept namespace text:** the `<` of the preserved element, or the
        ///   name of the preserved attribute.
        /// - **Kept diagnostic text:** where the diagnostic is located.
        location: Option<SourceLocation>,
        /// The template and arguments of the refusal's message.
        details: Details,
    },
}

impl ReadError {
    /// The catalog code of the refusal.
    ///
    /// | Variant | Code |
    /// |---|---|
    /// | [`Doctype`](Self::Doctype) | SACM-SEC-001 |
    /// | [`Malformed`](Self::Malformed) | SACM-XML-001 |
    /// | [`InvalidRoot`](Self::InvalidRoot), [`NoPackage`](Self::NoPackage) | SACM-XMI-001 |
    /// | [`DuplicateId`](Self::DuplicateId) | SACM-ID-001 |
    /// | [`InvalidId`](Self::InvalidId) | SACM-ID-002 |
    /// | [`RepeatedValue`](Self::RepeatedValue) | SACM-MULT-001 |
    /// | [`InvalidLiteral`](Self::InvalidLiteral) | SACM-ENUM-001 |
    /// | [`LegacyShape`](Self::LegacyShape) | SACM-XMI-012 |
    /// | [`MissingType`](Self::MissingType) | SACM-XMI-005 |
    /// | [`LimitExceeded`](Self::LimitExceeded) | SACM-SEC-003 |
    /// | [`Unsupported`](Self::Unsupported) | the code a tolerant read gives the same input (see the variant; LIB-API-017) |
    ///
    /// # Examples
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
    /// `SACM-XMI-003/unread-attribute`.
    ///
    /// - **A stable name** for its English sentence form, under which an
    ///   application keeps its own wording (ADR 0030 amendment 3).
    /// - **Without details:** [`Doctype`](Self::Doctype) is `SACM-SEC-001/doctype`
    ///   and [`NoPackage`](Self::NoPackage) `SACM-XMI-001/no-package`.
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

    /// The values the refusal's message is made from, in the order they first
    /// appear in it.
    ///
    /// Empty for [`Doctype`](Self::Doctype) and [`NoPackage`](Self::NoPackage).
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

    /// Where in the input the refusal is.
    ///
    /// | Refusal of | Location |
    /// |---|---|
    /// | an element | the `<` of its start tag |
    /// | an attribute | the first character of its name |
    /// | text | its first character that is not literal white space: a CDATA section or reference at its start |
    /// | [`DuplicateId`](Self::DuplicateId) | the first repeat |
    /// | [`RepeatedValue`](Self::RepeatedValue) | the second occurrence |
    /// | an input-size or node limit, the parser's capacity | `None` |
    ///
    /// - **Lines** end at a line feed, a carriage return and line feed, or a lone
    ///   carriage return.
    /// - **A leading byte order mark,** UTF-8 or UTF-16, takes no column.
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

    /// Every location the refusal names, in document order.
    ///
    /// | Refusal | Locations |
    /// |---|---|
    /// | [`DuplicateId`](Self::DuplicateId) | the start tag of every element carrying the id |
    /// | [`RepeatedValue`](Self::RepeatedValue) | every occurrence of the value |
    /// | every other | its [`location`](Self::location), or nothing when it has none |
    ///
    /// # Examples
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
    /// variant's fixed frame around what its template and arguments render.
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
/// - **Deterministic.** The same document gives the same bytes.
/// - **Modes.** Strict, the default, writes only schema-valid output; tolerant
///   writes the document as it is held. See [Modes](#modes).
/// - **What is written:** every element, the XMI metadata and, in tolerant mode,
///   the preserved content [`read`] kept.
///
/// # Round trip
///
/// Take a document from a strict [`read`] that a strict `write` accepts, changed
/// by any edits [`edit::apply`](crate::edit::apply) accepted.
///
/// - **It writes strictly,** with the two exceptions below. No edit stores text
///   XML 1.0 cannot represent, an id that is no `NCName`, or preserved content.
/// - **It reads back equal.** A strict read of the output gives a document `==`
///   to it, under [`Limits`] that admit the output.
/// - **Exception: no root.** Edits that delete every root leave nothing to
///   write ([`WriteError::NoRoot`]).
/// - **Exception: a freed id that is still named.** An edit can free an id that
///   XMI metadata names, by deleting the element, or the `xmi:id` of a name or
///   text that a reference names, by replacing it ([`WriteError::Invalid`]).
/// - **Outside the guarantee:** a document that holds preserved content or a
///   schema error as read, which a tolerant read in particular may give. A
///   strict write refuses it.
///
/// ```
/// use sacm::edit::{self, CreateClaim, SetName};
/// use sacm::xmi::{self, ReadOptions, WriteOptions};
///
/// let input = br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="arguments"/>"#;
/// let strict = ReadOptions::default();
/// let mut document = xmi::read(input, &strict)?.into_document();
/// edit::apply(&mut document, SetName::new("arguments", "Arguments"))?;
/// edit::apply(&mut document, CreateClaim::new("arguments", " The brake\nis tested "))?;
///
/// let text = xmi::write(&document, &WriteOptions::default())?;
/// let back = xmi::read(text.as_bytes(), &strict)?.into_document();
/// assert!(back == document);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # The text
///
/// - **Encoding.** UTF-8 without a byte order mark, starting with
///   `<?xml version="1.0" encoding="UTF-8"?>`.
/// - **Layout.** Lines end with LF, and so does the output. Two spaces of indent
///   per depth, one tag per line, and an element with no children as `<x … />`.
/// - **Roots.** One root is the document element. Several are the children of one
///   `xmi:XMI` element, in document order (XMI 2.5.1 §7.5.3).
/// - **Also in `xmi:XMI`:** one root, or none, when the document holds preserved
///   content or XMI metadata outside its elements (ADR 0035 D3).
/// - **Namespaces.** The document element alone declares SACM 2.3,
///   `http://www.omg.org/spec/SACM/20220301`, as `sacm`, and XMI 2.5.1,
///   `http://www.omg.org/spec/XMI/20131001`, as `xmi`.
/// - **No `xmi:version`.** The XMI namespace identifies the version (XMI 2.5.1
///   §7.5.3; [ADR 0017] amendment 3). One that [`read`] ignored is not written
///   back.
/// - **Escaping.** In attribute values `&`, `<`, `>`, `"`, TAB, LF and CR are
///   escaped; everything else is written as it is.
///
/// [ADR 0017]: https://github.com/lasrod/libSACM/blob/main/docs/adr/0017-xmi-writer.md
///
/// # Elements
///
/// - **Element names.** A root is `sacm:<Kind>`. Every other element has the name
///   of the role that holds it in its parent's class, with no prefix. Children
///   are in document order.
/// - **Role names** are spelled as in the machine-readable model ptc/22-03-13:
///   `argumentElement`, where §11.4's text says `argumentationElement`.
/// - **Class names and attributes** are spelled as in the text where the two
///   disagree: `sacm:ArtifactAssetRelationship`, an Event's `date`
///   (D-TEXT-MODEL). So the [spellings](crate::xmi::read#spellings) `read` reads
///   are written by their resolved names.
/// - **`xmi:type="sacm:<Kind>"`** is on a child exactly when the role's declared
///   type is not its kind: an abstract class, as for `argumentElement`, or a
///   package role holding an Interface or Binding, such as an
///   `ArgumentPackageInterface` in an AssuranceCasePackage's `argumentPackage`.
/// - **Never `xsi:type`.** `xmi:type` is XMI's own type attribute (XMI 2.5.1
///   §7.6.3, §7.8.8 and rule 2g; SACM sets no `useSchemaExtensions`). `read`
///   accepts both, and reports an `xsi:type` as SACM-XMI-010.
///
/// The attributes come in this order, class by class, superclass first (XMI 2.5.1
/// Annex B.5.2; ADR 0026, ADR 0027):
///
/// 1. `xmi:type`, `xmi:id`, and directly after it the `gid`;
/// 2. the `SACMElement` features (§8.2): `isCitation`, `isAbstract`,
///    `citedElement`, `abstractForm`;
/// 3. on an Assertion (§11.10): `assertionDeclaration`, `metaClaim`;
/// 4. on an asserted relationship (§11.13): `isCounter`, `reasoning`, `source`,
///    `target`;
/// 5. on an `ArgumentReasoning` (§11.12): `structure`;
/// 6. on an `ArtifactReference` (§11.9): `referencedArtifactElement`;
/// 7. on an `ArgumentGroup` (§11.2): `argumentElement`;
/// 8. on an `ArtifactGroup` (§12.3): `artifactElement`;
/// 9. the other reference ends, in [`ReferenceEnd::ALL`] order;
/// 10. the string-valued attributes: an Artifact's `version` and `date`, an
///     Event's `date`, an Activity's `startTime` and `endTime`, then `value` and
///     `externalReference` (ADR 0034 amendments 1 and 3).
///
/// So an `AssertedInference` with everything set starts `xmi:type xmi:id gid
/// isCitation isAbstract citedElement abstractForm assertionDeclaration metaClaim
/// isCounter reasoning source target`.
///
/// | Attribute | Written | As |
/// |---|---|---|
/// | `xmi:id` | always | the id as it is, escaped |
/// | `gid` | when the element has one | as held: `gid=""` for an empty one, no attribute for an absent one |
/// | a flag | only when true; false is the default | `"true"` |
/// | `assertionDeclaration` | only when it is not `asserted`, the default (XMI 2.5.1 §9.4.1) | as [`AssertionDeclaration::as_str`](crate::model::AssertionDeclaration::as_str) spells it |
/// | a reference end | only when it holds an id | the ids it holds, in order, repeats included, separated by one space and escaped, whether or not they name an element |
/// | a string-valued attribute | only when present | exactly as held, escaped |
///
/// - **An id that is no XML `NCName`,** such as `1 2`, is no `xsd:ID`. A strict
///   write refuses it (SACM-ID-002). A tolerant write writes it as it is, and the
///   output then does not validate against an XMI schema.
/// - **The attribute form.** A reference read in the element form is written as
///   an attribute, with the two exceptions below.
/// - **A cross-document reference** is written in the element form: only an
///   element can carry an `href`.
/// - **Beside a kept attribute.** When the element also holds, as preserved
///   content, an attribute of the end's name that held no id (`metaClaim=""`
///   beside `<metaClaim>` elements), each id is a `<metaClaim xmi:idref="…" />`.
/// - **Those elements** follow the `name`, `content` and `key`. So no start tag
///   carries an attribute twice and nothing is dropped (ADR 0033). An edit that
///   sets the end replaces such an attribute.
///
/// Names and texts:
///
/// - **A name** is the element's first child, `<name lang="…" content="…" />`.
///   Each field is written only if present: an empty field is `=""`, an absent
///   one no attribute. An element with no name has no `name` child.
/// - **Its `xmi:id`** ([`SacmElement::name_id`](crate::model::SacmElement::name_id))
///   comes first: `<name xmi:id="…" lang="…" content="…" />`.
/// - **An `ExpressionLangString`** name or value (§8.4) starts with
///   `xmi:type="sacm:ExpressionLangString"`, and its `expression`, if it has one,
///   follows `content`. The derived `expression` end is written only there,
///   never on the element.
/// - **In a `UtilityElement`,** its `content` is written first, then, on a
///   `TaggedValue`, its `key`, superclass first (XMI 2.5.1 Annex B.5.2).
/// - **A Resource's `location`** is written the same way, directly after its
///   `name`.
/// - **Each of the three** is its role element, with its `xmi:id` first if it has
///   one, holding one `<value … />` per value, in order, written as a name is:
///   `<content>`, `<value lang="en" content="…" />`, `</content>`.
/// - **Empty or absent.** An empty one is `<content />`; an absent one is not
///   written.
/// - **No type attribute** is written on a `UtilityElement`, a `content`, a `key`
///   or a `location`, nor on a `value` that holds a plain `LangString`.
///
/// # Preserved content
///
/// A tolerant `write` writes back, where it was read, what [`read`] kept as
/// [preserved content](fn@read#preserved-content) (LIB-COMPAT-003, ADR 0033,
/// ADR 0035). A strict write refuses a document that holds any, as
/// [`WriteError::PreservedContent`].
///
/// - **Its attributes** follow the element's typed attributes, in input order,
///   after the namespace declarations they need.
/// - **Its children** are each on their own line, before the typed child they
///   preceded, after the element's `name`, `content` and `key`, and before the
///   end tag when they followed the last.
/// - **Text** in the element is written trimmed, with `&`, `<`, `>` and CR
///   escaped.
/// - **A preserved element** is written with its subtree on one line, exactly as
///   held: names with their prefixes, attribute values escaped as above, text
///   exactly, and `<x … />` for one that holds nothing.
/// - **Its namespaces.** It declares every binding that was in scope where it was
///   read, other than `xml`, and `sacm` and `xmi` bound as the document element
///   binds them. Each element inside declares what it declared.
/// - **So** a written document reads back to an equal one, and writes again to
///   the same bytes.
///
/// [XMI metadata](fn@read#xmi-metadata) is written back the same way, by both
/// modes (SACM23-XMI-019, ADR 0042):
///
/// - **Its attributes** follow the typed ones and come before the preserved
///   ones, after the bindings both need, once per prefix, sorted by prefix.
/// - **Its children** come before the preserved children placed at the same
///   index.
/// - **On a part,** the metadata attributes follow its typed attributes. Metadata
///   children make it a start and an end tag, each child on its own line, by
///   value in a `content`, `key` or `location`.
/// - **Outside every element.** A document that holds such metadata is wrapped
///   in `xmi:XMI`.
/// - **A strict write checks it as XMI.xsd types it.** An `xmi:id` that is no
///   `NCName` (SACM-ID-002) and an `IDREF` or `IDREFS` token that names no
///   `xmi:id` of the output (SACM-REF-001) are [`WriteError::Invalid`].
/// - **Not checked:** anything inside a Difference's `target` (SACM23-XMI-031),
///   and what an extension holds otherwise.
///
/// A tolerant write writes preserved content back, after the `name`:
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
/// - **[`Mode::Strict`],** the default, writes only output that is schema-valid
///   (SACM 2.3 clause 2; ADR 0035 D4).
/// - **It refuses preserved content,** which it cannot show is schema-valid
///   ([`WriteError::PreservedContent`]), and an `xmi:id` that is no `NCName` or a
///   reference to no `xmi:id` it writes ([`WriteError::Invalid`]).
/// - **It never runs** [`validation::validate`](crate::validation::validate). XMI
///   schemas ignore multiplicities (XMI 2.5.1 §7.8.2), so a draft whose elements
///   have no name yet, or a relationship with no target, writes. An application
///   calls `validate` for model checks.
/// - **[`Mode::Tolerant`]** refuses neither, and writes the document as it is
///   held.
/// - **Same bytes.** The two modes write the same bytes for a document both
///   write.
/// - **Stability.** A new validation rule never changes what a strict write
///   writes. A new strict write refusal is listed in the release notes as a
///   change.
///
/// # Output size
///
/// `write` builds the whole output in memory and has no limit of its own.
///
/// - **After a read.** For a document [`read`] returned from B bytes, under
///   [`Limits`] with depth limit D and size limit M, the output is at most
///   (2·D + 48)·B + 6·M + 8,192 bytes. Each line is indented two spaces per level.
/// - **After edits** there is no such bound: the output grows with the
///   document's elements, their text and their depth.
///
/// # Errors
///
/// The document is checked before anything is written, and the first failure is
/// returned. Nothing else a document holds is refused.
///
/// 1. **No root** ([`WriteError::NoRoot`]), in both modes: there is no SACM
///    package to write. An `xmi:XMI` holding only XMI metadata holds no package.
/// 2. **Preserved content** ([`WriteError::PreservedContent`], SACM-XMI-006), in
///    strict mode only. It lists each element that holds some, and no element
///    when only the document holds it.
/// 3. **The schema check** ([`WriteError::Invalid`]), in strict mode only, by the
///    table below, in [`Diagnostic`] order.
/// 4. **Each element's text** ([`WriteError::UnrepresentableCharacter`]), in
///    [`Document::elements`] order, all of an element's checks before the next
///    element's.
///
/// Step 3, the schema errors:
///
/// | Code | One per | Rule |
/// |---|---|---|
/// | SACM-ID-002 | `xmi:id`, of an element, a `name`, a `content`, a `key`, a `location` or a `value`, that is no `NCName` | XML Schema 1.0 Part 2 §3.3.8 |
/// | SACM-REF-001 | distinct id of a reference end that is no `xmi:id` the output writes | Part 1 §3.3.4; XMI 2.5.1 rule 4i |
///
/// Step 4, a character outside XML 1.0's `Char`:
///
/// - **Order within an element:** its id; its gid; its `version`, `date`,
///   `startTime`, `endTime`, `value` and `externalReference`; its name's `lang`,
///   then `content`.
/// - **Then each value** of its `content`, then of its `key`, then of its
///   `location`, `lang` before `content`.
/// - **It names** the attribute and its first such character, and for a value
///   its position too.
/// - **Preserved content is not checked.** [`read`] read it, so it is
///   representable.
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
/// # Examples
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
/// The writer writes everything a [`Document`] holds, so it refuses only these
/// (ADR 0035 D4; LIB-API-017):
///
/// | Variant | Code | Modes | The document |
/// |---|---|---|---|
/// | [`NoRoot`](Self::NoRoot) | SACM-XMI-001 | both | has no root |
/// | [`UnrepresentableCharacter`](Self::UnrepresentableCharacter) | SACM-XML-003 | both | holds a character XML 1.0 cannot represent |
/// | [`PreservedContent`](Self::PreservedContent) | SACM-XMI-006 | strict | holds preserved content |
/// | [`Invalid`](Self::Invalid) | SACM-XMI-016 | strict | would give output that is not schema-valid |
///
/// - **Wording it yourself.** Every refusal has a [`code`](Self::code), a message
///   [`template`](Self::template) and its [`arguments`](Self::arguments), as
///   [`ReadError`] has.
/// - **What is refused.** [`element`](Self::element) names the element, where
///   there is one, and [`diagnostics`](Self::diagnostics) gives the schema errors
///   of [`Invalid`](Self::Invalid).
/// - **With the `serde` feature** it reads back as a record, as a `ReadError`
///   does. Its fields are kept as given, so a record's `Invalid` may hold no
///   diagnostics.
///
/// # Display
///
/// `Display` writes one line that starts in lower case, without trailing
/// punctuation.
///
/// - **Ids** are quoted with `{:?}`, so line breaks in them never break the line.
/// - **[`Invalid`](Self::Invalid)** gives the number of schema errors, and the
///   code and first affected id of the first one, introduced by `the first is`
///   when there are several.
/// - **[`PreservedContent`](Self::PreservedContent)** gives the one element that
///   holds preserved content, or their number and the first, or that the
///   document holds it outside its elements.
///
/// # Examples
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
    /// The document has no root element, so there is no SACM package to write.
    ///
    /// Fix: add a package as a root. This variant has no fields and will not
    /// gain any.
    NoRoot,
    /// A text of an element holds a character that XML 1.0 cannot represent.
    ///
    /// - **Which characters.** One outside XML 1.0's `Char` production, such as
    ///   U+0000, another C0 control other than TAB, LF and CR, U+FFFE or U+FFFF.
    /// - **Which texts.** The element's id, gid, `version`, `date`, `startTime`,
    ///   `endTime`, `value` or `externalReference`; its name's `lang` or
    ///   `content`; the `lang` or `content` of a value of its `content`, `key`
    ///   or `location`.
    /// - **Not reached by edits or reads.** No edit stores such text
    ///   ([`EditError::UnrepresentableCharacter`](crate::edit::EditError::UnrepresentableCharacter)),
    ///   and [`read`] cannot hold it. Only a document built with the
    ///   `test-support` seams reaches this refusal.
    /// - **In a value** of a `content`, `key` or `location`, `position` is the
    ///   value's index and the template is
    ///   `SACM-XML-003/unrepresentable-character-in-value`.
    /// - **Otherwise** `position` is `None` and the template is
    ///   `SACM-XML-003/unrepresentable-character`.
    #[non_exhaustive]
    UnrepresentableCharacter {
        /// The element whose text holds the character.
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
    /// ADR 0035 D4).
    ///
    /// | Schema error | Code | Rule |
    /// |---|---|---|
    /// | an `xmi:id`, of an element, a `name`, a `content`, a `key` or a `value`, is no `NCName`, which an `xsd:ID` must be | SACM-ID-002 | XML Schema 1.0 Part 2 §3.3.8 |
    /// | a reference names no `xmi:id` the output writes, which an `xsd:IDREF` must | SACM-REF-001 | Part 1 §3.3.4; XMI 2.5.1 rule 4i |
    ///
    /// - **`diagnostics`** holds every such Error, in [`Diagnostic`] order. Each
    ///   carries its own code and the ids of the elements it affects.
    /// - **No validation.** A reference to the `xmi:id` of a `name` is written,
    ///   though [`validation::validate`](crate::validation::validate) reports it.
    /// - **Fix** the ids and references, or write the document as it is held with
    ///   `WriteOptions::default().with_mode(Mode::Tolerant)`.
    ///
    /// # Examples
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
    /// In strict mode, the document holds preserved content (SACM-XMI-006,
    /// SACM23-XMI-008, ADR 0033, ADR 0035).
    ///
    /// - **What it is.** What [`read`] kept, as written, because it does not
    ///   conform where it stands.
    /// - **Why refused.** The writer cannot show that content it did not read is
    ///   schema-valid. It is refused before the schema check.
    /// - **Fix:** write the document in tolerant mode, which writes the content
    ///   back, or delete the elements that hold it.
    /// - **Outside its elements.** Content the document itself holds can be
    ///   written only in tolerant mode in this release.
    ///
    /// # Examples
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
        /// [`Document::elements`] order.
        ///
        /// Empty only when the document alone holds it, outside its elements
        /// ([`Document::has_preserved_content`]).
        elements: Vec<ElementId>,
        /// The template and arguments of the refusal's message: see
        /// [`template`](Self::template) and [`arguments`](Self::arguments).
        details: Details,
    },
}

impl WriteError {
    /// The element refused, if the refusal names one: the element of
    /// [`UnrepresentableCharacter`](Self::UnrepresentableCharacter).
    ///
    /// `None` for the other variants:
    ///
    /// - **[`NoRoot`](Self::NoRoot)** names none.
    /// - **[`Invalid`](Self::Invalid)** has [`diagnostics`](Self::diagnostics),
    ///   which name the elements they affect.
    /// - **[`PreservedContent`](Self::PreservedContent)** lists its elements.
    #[must_use]
    pub fn element(&self) -> Option<&ElementId> {
        match self {
            Self::NoRoot | Self::Invalid { .. } | Self::PreservedContent { .. } => None,
            Self::UnrepresentableCharacter { element, .. } => Some(element),
        }
    }

    /// The catalog code of the refusal (LIB-API-017).
    ///
    /// | Variant | Code |
    /// |---|---|
    /// | [`NoRoot`](Self::NoRoot) | SACM-XMI-001 ([`Code::XMI_INVALID_ROOT`]) |
    /// | [`UnrepresentableCharacter`](Self::UnrepresentableCharacter) | SACM-XML-003 ([`Code::XML_UNREPRESENTABLE_CHARACTER`]) |
    /// | [`Invalid`](Self::Invalid) | SACM-XMI-016 ([`Code::XMI_STRICT_SAVE_INVALID`]); its diagnostics carry the codes of the schema errors |
    /// | [`PreservedContent`](Self::PreservedContent) | SACM-XMI-006 ([`Code::XMI_STRICT_SAVE_REFUSED`]) |
    #[must_use]
    pub fn code(&self) -> Code {
        self.details().map_or(Code::XMI_INVALID_ROOT, Details::code)
    }

    /// The id of the refusal's message template, `<CODE>/<form>`, such as
    /// `SACM-XMI-006/one-holder` (ADR 0030 amendment 3).
    ///
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
///
/// # Examples
///
/// ```
/// use sacm::xmi::WriteAttribute;
///
/// // It displays as the message names it.
/// assert_eq!(WriteAttribute::Id.to_string(), "xmi:id");
/// assert_eq!(WriteAttribute::NameContent.to_string(), "name's content");
/// assert_eq!(WriteAttribute::LocationValueLang.to_string(), "location value's lang");
/// ```
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
    /// Writes the attribute as a message names it: `xmi:id`, `gid`, `name's
    /// lang`, `content value's lang`, `version`, `startTime` and so on.
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
