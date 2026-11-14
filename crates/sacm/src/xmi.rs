//! Reading and writing SACM 2.3 XMI documents.
//!
//! [`read`] takes the bytes of an XMI document and returns a [`ReadOutput`] or
//! a [`ReadError`]. [`write`](fn@write) takes a [`Document`] and returns its XMI text or
//! a [`WriteError`]. Neither does I/O: the caller reads and writes the file.
//!
//! In this release `read` reads `AssuranceCasePackage` and `ArgumentPackage`
//! roots, bare or inside an `xmi:XMI` element, and what they hold of eleven
//! kinds: `AssuranceCasePackage` and `ArgumentPackage` elements in an
//! `AssuranceCasePackage`, and `ArgumentPackage`, `Claim`,
//! `ArgumentReasoning`, `ArtifactReference`, `ArgumentGroup`,
//! `AssertedInference`, `AssertedEvidence`, `AssertedContext`,
//! `AssertedArtifactSupport` and `AssertedArtifactContext` elements in an
//! `ArgumentPackage`, each with its id (generated when it has no `xmi:id`),
//! `gid`, `isCitation`, `isAbstract`, `citedElement`, `abstractForm` and
//! name, an Assertion's `assertionDeclaration` and `metaClaim`, an asserted
//! relationship's `isCounter`, `reasoning`, `source` and `target`, an
//! `ArgumentReasoning`'s `structure`, an `ArtifactReference`'s
//! `referencedArtifactElement`, an `ArgumentGroup`'s members
//! (`argumentElement`), and the descriptions, implementation constraints,
//! notes and tagged values of each, with their `content` and `key` (SACM 2.3
//! §2.2, §2.4, §8.2-§8.12, §9.2, §11.2, §11.4, §11.9-§11.18). A reference is held as the id it names, and one that names no
//! element is kept (see [References](fn@read#references)). It refuses input that is unsafe
//! ([`ReadError::Doctype`], SACM-SEC-001), not well-formed
//! ([`ReadError::Malformed`], SACM-XML-001), not rooted in a SACM package
//! ([`ReadError::InvalidRoot`] and [`ReadError::NoPackage`], SACM-XMI-001),
//! with an `xmi:id` that repeats ([`ReadError::DuplicateId`], SACM-ID-001)
//! or is empty ([`ReadError::InvalidId`], SACM-ID-002), with a value given
//! twice ([`ReadError::RepeatedValue`], SACM-MULT-001), or with a value that
//! is not a literal ([`ReadError::InvalidLiteral`], SACM-ENUM-001). Content
//! it cannot represent yet is refused as [`ReadError::Unsupported`], never
//! dropped.
//!
//! `write` writes every element of a document, with its `gid`, its flags and
//! references, its name, its `content` and `key` and, on an Assertion, its
//! `assertionDeclaration`, in
//! document order, in one deterministic form. Its refusals are a document
//! with no root ([`WriteError::NoRoot`]), in strict mode a document that
//! validation reports an Error for ([`WriteError::Invalid`]), and an id,
//! gid, name or text that XML 1.0 cannot represent
//! ([`WriteError::UnrepresentableCharacter`]). A document returned by this
//! release's `read` always writes in tolerant mode, and what `write` writes
//! for a document built with [`edit::apply`](crate::edit::apply) reads back
//! with the same ids, kinds, names, contents, keys, declarations, parents and
//! order. A document with a reference that names no element is refused by a
//! strict `write` (SACM-REF-001) and written as held by a tolerant one.
//!
//! Both take a [`Mode`] in their options (ADR 0022). Strict aims at SACM 2.3
//! conformance (see [`Mode`] for what it checks today); tolerant also accepts recognised variations, as Warnings, and
//! writes a document as it is held. [`ReadOptions`] default to tolerant and
//! [`WriteOptions`] to strict.
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

use std::fmt;

use crate::diagnostic::{Code, Diagnostic, SourceLocation};
use crate::model::{Document, ElementId, ReferenceEnd};

mod input;
mod namespaces;
mod output;
mod roots;

/// Reads an XMI document from its bytes.
///
/// In this release `read` reads `AssuranceCasePackage` and `ArgumentPackage`
/// roots (formal/23-05-08 §2.2, §2.4): the document element, or each element
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
/// | ArgumentPackage (§11.4) | `argumentElement` | an ArgumentPackage, a Claim, an `ArgumentReasoning`, an `ArtifactReference`, an `ArgumentGroup`, an `AssertedInference`, an `AssertedEvidence`, an `AssertedContext`, an `AssertedArtifactSupport` or an `AssertedArtifactContext` | yes: the role's type, `ArgumentationElement`, is abstract (§11.3) |
/// | each of the above (§8.6) | `description` | a `Description` | no |
/// | each of the above (§8.6) | `implementationConstraint` | an `ImplementationConstraint` | no |
/// | each of the above (§8.6) | `note` | a `Note` | no |
/// | each of the above (§8.6) | `taggedValue` | a `TaggedValue` | no |
///
/// The class is the one the child's `xmi:type` or `xsi:type` names (both, if
/// present, must name the same class); without one, it is the role's declared
/// type. §11.4's text spells the ArgumentPackage role `argumentationElement`;
/// this release refuses that spelling as [`ReadError::Unsupported`].
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
/// `sacm:MultiLangString`.
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
/// (SACM-ENUM-001), holding the value as read, in both modes: it is never
/// read as false. And it may carry the references `citedElement` and
/// `abstractForm` (§8.2 `[0..1]`), described under
/// [References](#references). The flag and its reference are independent,
/// and nothing repairs a file on read; the §8.2 rules between them are
/// [`validation::validate`](crate::validation::validate)'s.
///
/// The argumentation kinds have more (ADR 0027, ADR 0028). Every Assertion
/// (a Claim or one of the five asserted relationships) may carry `metaClaim`
/// (§11.10 `[0..*]`). An asserted relationship (§11.13) may carry the flag
/// `isCounter` (`[1] = false`), read as `isCitation` is, and the references
/// `reasoning` (`[0..1]`), `source` (`[1..*]`) and `target` (`[1]`). An
/// `ArgumentReasoning` may carry `structure` (§11.12 `[0..1]`), an
/// `ArtifactReference` `referencedArtifactElement` (§11.9 `[0..*]`), and an
/// `ArgumentGroup` its members, `argumentElement` (§11.2 `[0..*]`). An
/// `ArtifactReference` is no Assertion, so it has no `assertionDeclaration`
/// or `metaClaim`. A group's `argumentElement` is a reference, though it
/// shares its name with an ArgumentPackage's composite role: under a group,
/// an `argumentElement` element is a reference element, so one that holds a
/// nested element, such as `<argumentElement xmi:type="sacm:Claim" .../>`,
/// is [`ReadError::Unsupported`] as any malformed reference element is, and
/// never read as a child. A
/// relationship read without a `source` or a `target` is kept as read, and
/// nothing is added; [`validation::validate`](crate::validation::validate)
/// reports it (SACM23-VAL-005). On a kind that lacks one of these, such as
/// `isCounter` on a Claim or `metaClaim` on an `ArgumentReasoning`, it is
/// [`ReadError::Unsupported`].
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
/// holding the value as read, in both modes. It is never read as `asserted`,
/// because that would silently change what the case declares. Fix: correct
/// the spelling.
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
/// is [`ReadError::RepeatedValue`] (SACM-MULT-001), even when the values are
/// equal, and so is a second `name`, `content` or `key`.
///
/// Elements and
/// attributes are matched by namespace URI and local name, and an `xmi:type`
/// or `xsi:type` value is resolved as a `QName`, without its leading and
/// trailing white space, against the namespaces in scope, so the choice of
/// prefix does not matter. The SACM namespace is
/// `http://www.omg.org/spec/SACM/20220301`.
///
/// `read` refuses rather than drops: it never returns a document that is
/// missing content from its input. A root may carry only namespace
/// declarations, `xmi:id`, `gid`, `isCitation`, `isAbstract`, `citedElement`,
/// `abstractForm`, `xmi:version` and an `xmi:type` or `xsi:type` naming its
/// own class, and hold at most one `name`, at most one value or reference
/// element of each of `gid`, `isCitation`, `isAbstract`, `citedElement` and
/// `abstractForm`, and otherwise only white space, comments, processing
/// instructions and the children above. A child may carry only namespace
/// declarations, `xmi:id`, those five, its type attributes and the features
/// of its kind above (`assertionDeclaration` and `metaClaim` on an
/// Assertion, `isCounter`, `reasoning`, `source` and `target` on an asserted
/// relationship, `structure` on an `ArgumentReasoning`,
/// `referencedArtifactElement` on an `ArtifactReference`, `argumentElement`
/// on an `ArgumentGroup`), and hold the same
/// content as a root, with a value or reference element for each of its
/// kind's features, except that a `UtilityElement` holds, besides those
/// five, only its `content` and `key`.
/// An `xmi:version` on a root or on the `xmi:XMI` wrapper is accepted with
/// any value and ignored: the XMI namespace identifies the version
/// (XMI 2.5.1 §7.5.3), so [`write()`] does not write it back. It is reported
/// as SACM-XMI-011 (see [Diagnostics](#diagnostics)).
/// The `name`, and each `value`, must be in no namespace, carry only
/// `xmi:id`, `lang`, `content` and an `xmi:type` or `xsi:type` naming
/// `sacm:LangString`, and hold only the value elements `lang` and `content`,
/// white space, comments and processing instructions. A value element may
/// carry only namespace declarations and may hold no element.
/// Anything else is refused as [`ReadError::Unsupported`] until a later
/// release reads it. That includes the single-language shorthand some tools
/// write, which LIB-COMPAT-008 is to read: `lang` or `content` attributes or
/// text on a `content` or `key`, `content="…"` or text on a `UtilityElement`,
/// and `key="…"` or `value="…"` on a `TaggedValue`.
///
/// The input is UTF-8, ASCII, or UTF-16 with a byte order mark, which reads
/// exactly as the same characters in UTF-8. A DOCTYPE is refused before
/// anything in it is read, so no entity is declared, resolved or expanded.
///
/// # Ids
///
/// An element's id is the value of its `xmi:id`, an `xsd:ID` (XMI 2.5.1
/// §7.6.1), and only `xmi:id` in the XMI namespace is one: an `id` attribute
/// in no namespace or another namespace, such as `id` or `o:id`, is refused
/// as [`ReadError::Unsupported`]. The ids of the elements, and of the names,
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
///   of no `xmi:id` in the input, is named by no reference in it (see
///   [References](#references)), and was not generated earlier in the read,
///   so a reference that names no element never resolves to a generated
///   one. It is an ordinary id: [`write()`] writes it, and an edit continues
///   above it, also skipping every referenced id. The same bytes always give
///   the same ids.
/// - An `xmi:id` on a `name` is kept with the element that owns the name, as
///   [`SacmElement::name_id`](crate::model::SacmElement::name_id), and
///   [`write()`] writes it back. A `name` without one gets none.
/// - So is an `xmi:id` on a `content` or `key`
///   ([`SacmElement::content_id`](crate::model::SacmElement::content_id),
///   [`SacmElement::key_id`](crate::model::SacmElement::key_id)), and on each
///   `value` in one, by position
///   ([`SacmElement::content_value_ids`](crate::model::SacmElement::content_value_ids),
///   [`SacmElement::key_value_ids`](crate::model::SacmElement::key_value_ids)).
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
/// `ArtifactReference` and `argumentElement` (§11.2) on an `ArgumentGroup`.
/// Four are multi-valued: `metaClaim`, `source`, `referencedArtifactElement`
/// and a group's `argumentElement` hold any number of ids, in document
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
///   token, such as `""`, is [`ReadError::Unsupported`], never read as no
///   reference. A multi-valued end holds every token, and two or more tokens
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
/// - **Refused, never dropped,** as [`ReadError::Unsupported`] at the
///   construct: a reference element with both link attributes (at the
///   second) or neither (at its start tag); any other attribute on it, such
///   as `xmi:id`, `xmi:type`, an unqualified `idref` or `xmi:href`; an
///   `xmi:idref` that is empty or, once trimmed, still holds XML white space
///   (space, TAB, LF or CR), such as `a b` or `a&#9;b`, at the attribute,
///   since a tolerant [`write()`] would give it back as two ids; an `href`
///   that is not `#id`: with a document part, such as `other.xmi#c2` or `c2`
///   (cross-document links are SACM23-XMI-027's), a pointer such as
///   `#xpointer(id('c2'))`, a fragment with white space inside it, `#` or
///   empty; a child element
///   or text in it; and an end or a reference element in a namespace, such as
///   `sacm:citedElement`. A reference on a `name`, `content`, `key` or
///   `value` is refused as `Unsupported`: those have no references. So is
///   an end on a kind that lacks it, such as `source` on a Claim or
///   `metaClaim` on a Note.
/// - **Dangling references are kept.** A reference whose id names no
///   element, an id absent from the input or the `xmi:id` of a `name`,
///   `content`, `key` or `value`, is read in both modes without a
///   diagnostic, held as written, and written back by a tolerant
///   [`write()`]. [`validation::validate`](crate::validation::validate)
///   reports it as SACM-REF-001, an Error (SACM23-VAL-003), so a strict
///   [`write()`] refuses it.
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
/// tag (LIB-COMPAT-001, ADR 0022, ADR 0024):
///
/// - SACM-XMI-004 ([`Code::XMI_MISSING_ID`]), clause 7.6.1, for each element
///   read without an `xmi:id` (SACM23-XMI-020). It affects the generated id,
///   names the kind and the id, and is Info in both modes.
/// - SACM-ID-004 ([`Code::ID_TRIMMED`]), clause 7.6.1, for each `xmi:id` read
///   without the white space around it (SACM23-XMI-025). It affects the
///   trimmed id, or, for the id of a `name`, `content`, `key` or `value`, the
///   element that owns it, quotes the value as read, and is Info in both
///   modes.
/// - SACM-XMI-010 ([`Code::XMI_XSI_TYPE`]), clause 9.5.2, for each `xsi:type`
///   on a root, a child, a `name`, a `content`, a `key` or a `value`. It
///   affects the element, or, for the others, the element that owns them. It
///   is an Error in [`Mode::Strict`] and
///   a Warning in [`Mode::Tolerant`]: XMI 2.5.1 §9.5.2 rule 2g allows
///   `xsi:type` only if `org.omg.xmi.useSchemaExtensions` is true, and SACM
///   sets no such tag. The type is read as an `xmi:type` would be. An element
///   with both type attributes gets one.
/// - SACM-XMI-011 ([`Code::XMI_VERSION_IGNORED`]), clause 7.5.3, for each
///   `xmi:version` on the `xmi:XMI` element, which affects no element, or on
///   a root, which it affects. It is Info in both modes. An `xmi:version` on
///   any other element is refused as [`ReadError::Unsupported`].
///
/// None refuses the input: in this release both
/// modes read and refuse the same inputs, and give the same document. A
/// later minor release may add a read diagnostic, so a strict read may then
/// report an Error where it did not before; the release notes list each one.
///
/// # Errors
///
/// The checks run in this order, and the first failure is returned:
///
/// 1. the byte order mark and the first bytes: UTF-16 with a mark is
///    decoded; UTF-16 without one, UTF-32 and UCS-4 are `Unsupported`;
/// 2. the grammar of the XML declaration's `<?xml`, `version` and `encoding`
///    (`Malformed`);
/// 3. the declared encoding: without a UTF-16 mark, `Unsupported` unless
///    UTF-8 or ASCII; after one, `Malformed` unless UTF-16, UTF-16LE or
///    UTF-16BE. Then, for an accepted one, the rest of the declaration:
///    `standalone` and `?>` (`Malformed`);
/// 4. under an ASCII declaration, a non-ASCII character (`Malformed`);
/// 5. the encoding (`Malformed`): invalid UTF-8 at the first invalid byte;
///    in UTF-16, an unpaired surrogate at the surrogate, or an odd byte count
///    after the last whole character;
/// 6. the parse: a DOCTYPE (`Doctype`) or a well-formedness error
///    (`Malformed`), whichever comes first;
/// 7. the well-formedness errors this release refuses after the XML parser
///    has accepted the input, the earliest first (`Malformed`): a character
///    reference to a surrogate or above U+10FFFF, a declaration of the
///    namespace prefix `xmlns` (`xmlns:xmlns`), a namespace prefix
///    undeclared with `xmlns:p=""`, an element or attribute name with an
///    empty prefix (`<:c>`, `</:c>`, ` :a="1"`), and a processing instruction
///    whose target (the XML Name after `<?`) is `xml` in any letter case,
///    contains a colon, or is followed by neither white space nor `?>`.
///    Within the W3C XML Conformance Test Suite (20130923), this list is
///    closed: every not-well-formed XML 1.0 and Namespaces 1.0 case in it is
///    refused. Outside the suite, other leniencies of the parser may remain:
///    a sweep (LIB-SEC-005) checks only that these checks agree with the
///    parser's tree;
/// 8. the roots, in two passes. First the names: an `XMI` element outside the
///    XMI 2.5.1 namespace, or any other XMI-namespace element, as the document
///    element is `Unsupported`; an `xmi:XMI` element with no root is
///    `NoPackage`; otherwise the first root not named as a package kind is
///    `InvalidRoot`. Then, in document order, the `xmi:XMI` element's
///    attributes, each root with its content, and the content beside it
///    (`Unsupported`). An element's start tag (its role, type, kind and id)
///    is checked before its other attributes, and those before its content.
///    An empty id is `InvalidId`, and an id whose value an earlier element,
///    name, `content`, `key` or `value` carries is `DuplicateId`, so a repeat
///    is refused before anything after its id. A value that is not a literal
///    is `InvalidLiteral` where it is met. A `name`, `content`, `key` or
///    value element is checked whole when it is met, then against the
///    occurrences before it,
///    so the second occurrence of a value is `RepeatedValue` unless it is
///    itself refused first. A later occurrence
///    is listed in `RepeatedValue`'s
///    [`locations`](ReadError::locations) even if it is itself malformed,
///    because each occurrence is checked against those before it.
///
/// The refusals:
///
/// - [`ReadError::Doctype`] (SACM-SEC-001): the input has a DOCTYPE, with or
///   without entity declarations.
/// - [`ReadError::Malformed`] (SACM-XML-001): the input is not well-formed
///   XML, is truncated, is not valid UTF-8 or UTF-16, declares ASCII but
///   holds a non-ASCII character, or has a UTF-16 byte order mark but
///   declares another encoding.
/// - [`ReadError::InvalidRoot`] (SACM-XMI-001): a root's local name is not
///   one of the twelve SACM package kinds.
/// - [`ReadError::NoPackage`] (SACM-XMI-001): the `xmi:XMI` element holds no
///   root: no element child outside the XMI namespace.
/// - [`ReadError::DuplicateId`] (SACM-ID-001): two or more elements (names,
///   `content`s, `key`s and `value`s included) carry the same `xmi:id`
///   (XMI 2.5.1 §7.6.1), compared by `xsd:ID` value (see [Ids](#ids)). It
///   names every element that carries the first id to repeat.
/// - [`ReadError::InvalidId`] (SACM-ID-002): the `xmi:id` of an element, or
///   of a `name`, `content`, `key` or `value`, is empty. Any other id is read
///   by its `xsd:ID` value; one that is still not an XML `NCName`, such as
///   `1c` or `a b`, is reported by
///   [`validation::validate`](crate::validation::validate).
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
///   `referencedArtifactElement`, a group's `argumentElement`) given both as
///   an attribute and as elements is too, with every occurrence.
/// - [`ReadError::InvalidLiteral`] (SACM-ENUM-001): an Assertion's
///   `assertionDeclaration` is not exactly one of the six literals, or an
///   `isCitation`, `isAbstract` or `isCounter` is not one of `true`,
///   `false`, `1` and `0`, white space around them aside, as an attribute or
///   as an element. It holds the value as read.
/// - [`ReadError::Unsupported`] (no code): the input is well-formed but this
///   version cannot represent it. Examples are UTF-16 without a byte order
///   mark, UTF-32 or UCS-4 input, a declared encoding other than UTF-8 or
///   ASCII in input without a UTF-16 mark, more nodes, attributes or
///   namespaces than the parser can hold, the other ten package kinds as
///   roots, a package name outside the SACM namespace, an `id` attribute
///   that is not `xmi:id` (such as `id` or `o:id`), an `XMI` element outside
///   the XMI 2.5.1 namespace (such as an older XMI revision's), an
///   XMI-namespace element as the document element (such as
///   `xmi:Documentation`) or beside the roots (such as `xmi:documentation`),
///   an attribute of `xmi:XMI` other than `xmi:version`, any other attribute
///   or content of a `name` or `value` (such as `xml:lang`, `xmi:uuid`, `gid`
///   or a `value` element), a `name` or `value` in a namespace (such as
///   `sacm:name`) or with a type naming another class (such as
///   `sacm:ExpressionLangString`), any other attribute or content of a
///   `content` or `key` (such as `gid`, `xml:lang`, the single-language
///   shorthand or a `name` element), a value element that carries an
///   attribute (such as `xmi:id` or `xsi:nil`), holds an element, or is in a
///   namespace (such as `sacm:content`, `sacm:gid` or `sacm:isCitation`), a
///   reference attribute that holds no id, or a malformed reference element
///   (see [References](#references)), and any other attribute or content of a
///   root or child. For a child, that includes a role its parent's class
///   lacks (any child of a `UtilityElement`), a role element in a namespace,
///   a type that names no concrete SACM class or that the role does not
///   admit, two types that disagree, no type under a role of an abstract
///   type, any other kind (such as an `ArgumentPackageInterface` or an
///   `ArgumentPackageBinding`), text, attributes such as `xmi:uuid` or
///   `sacm:isCitation`; a `name` on a `UtilityElement`; an
///   `assertionDeclaration`, as an attribute or an element, that is on a
///   package or an `ArgumentReasoning` (even `"asserted"`) or in a namespace;
///   and a flag or reference end on a kind that lacks it, such as
///   `isCounter` on a Claim (see [References](#references)).
///
/// # Examples
///
/// ```
/// use sacm::diagnostic::{Code, Severity};
/// use sacm::model::{AssertionDeclaration, ElementKind};
/// use sacm::xmi::{self, Mode, ReadError, ReadOptions};
///
/// let options = ReadOptions::default();
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
/// // A value that is not a literal (here the wrong letter case) is refused at
/// // the attribute's name, line 3, column 54, and never read as `asserted`.
/// let misspelt = br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="arguments">
///   <argumentElement xmi:type="sacm:Claim" xmi:id="c1" assertionDeclaration="Assumed"/>
/// </sacm:ArgumentPackage>"#;
/// let error = xmi::read(misspelt, &options).unwrap_err();
/// assert!(matches!(&error, ReadError::InvalidLiteral { value, .. } if value == "Assumed"));
/// assert_eq!(error.code(), Some(Code::ENUM_INVALID_LITERAL));
/// assert_eq!(error.location().map(|at| (at.line(), at.column())), Some((3, 54)));
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
/// assert_eq!(error.code(), Some(Code::XMI_INVALID_ROOT));
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
/// assert_eq!(error.code(), Some(Code::XML_DOCTYPE_REJECTED));
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
    let text = input::decode(input)?;
    let tree = input::parse(&text)?;
    let (document, mut diagnostics) = roots::read(&text, &tree, mode)?;
    diagnostics.sort();
    Ok(ReadOutput {
        document,
        diagnostics,
        mode,
    })
}

/// How strictly [`read`] and [`write`](fn@write) treat a document (D-STRICT,
/// ADR 0022).
///
/// - [`Strict`](Self::Strict) aims at SACM 2.3 conformance. A strict read
///   reports each recognised variation it reads as an Error (in this release,
///   only `xsi:type`, SACM-XMI-010). It still returns the document and does
///   not validate it. To check a file, test
///   [`diagnostic::has_errors`](crate::diagnostic::has_errors) on the read
///   diagnostics together with the result of
///   [`validation::validate`](crate::validation::validate).
///
///   A strict write validates the document with
///   [`validation::validate`](crate::validation::validate) and refuses it if
///   validation reports an Error. Validation does not check everything SACM
///   requires yet, so a strict write is not yet a guarantee of conformance.
/// - [`Tolerant`](Self::Tolerant) also accepts recognised variations, and
///   reports them as Warnings. A tolerant write writes the document as it is
///   held, without validating it.
///
/// `Mode` has no default of its own: [`ReadOptions`] default to `Tolerant`
/// and [`WriteOptions`] to `Strict`. On a read, in this release, both modes
/// accept and refuse the same inputs, and the mode decides only the severity
/// of SACM-XMI-010 (see [`read`]). On a write, it decides whether the
/// document is validated first. Later releases may add variants, so match it
/// with a fallback arm.
///
/// ```
/// use sacm::xmi::{Mode, ReadOptions, WriteOptions};
///
/// assert_eq!(ReadOptions::default().mode(), Mode::Tolerant);
/// assert_eq!(WriteOptions::default().mode(), Mode::Strict);
/// let strict = ReadOptions::default().with_mode(Mode::Strict);
/// assert_eq!(strict.mode(), Mode::Strict);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Mode {
    /// Aims at SACM 2.3 conformance: a read reports each recognised variation
    /// as an Error, and a write refuses a document that validation reports an
    /// Error for.
    Strict,
    /// Recognised variations are read and reported as Warnings, and a write
    /// writes the document as it is held.
    Tolerant,
}

/// How [`read`] reads a document: in which [`Mode`], tolerant by default.
///
/// The resource limits (LIB-SEC-003) come later.
///
/// ```
/// use sacm::xmi::{self, Mode, ReadOptions};
///
/// let input = br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="arguments"/>"#;
/// let output = xmi::read(input, &ReadOptions::default().with_mode(Mode::Strict))?;
/// assert_eq!(output.mode(), Mode::Strict);
/// # Ok::<(), xmi::ReadError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct ReadOptions {
    /// The mode to read in.
    mode: Mode,
}

impl Default for ReadOptions {
    /// Reads in [`Mode::Tolerant`].
    fn default() -> Self {
        Self {
            mode: Mode::Tolerant,
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
}

/// What [`read`] returns for a document it reads: the document, the
/// diagnostics found while reading it, and the mode it was read in.
///
/// The document holds all of the input's content: `read` refuses what it
/// cannot represent rather than drop it. The diagnostics are sorted by
/// [`Diagnostic`]'s order, located ones first. In this release they are
/// SACM-XMI-004 for an element without an `xmi:id`, SACM-ID-004 for an
/// `xmi:id` read trimmed, SACM-XMI-010 for an `xsi:type` and SACM-XMI-011 for
/// an `xmi:version` (see [`read`]).
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
    /// Empty for an input with no `xsi:type` and no `xmi:version`, in which
    /// every element has an `xmi:id` and no `xmi:id`, of an element or a
    /// `name`, has white space around it.
    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
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
/// [`RepeatedValue`](Self::RepeatedValue) and
/// [`InvalidLiteral`](Self::InvalidLiteral). [`Unsupported`](Self::Unsupported),
/// with no code, is input this version cannot read yet.
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
/// known, and the reason after `: `; a [`DuplicateId`](Self::DuplicateId) or
/// [`RepeatedValue`](Self::RepeatedValue) lists every location, separated by
/// `; `. Names from the input are quoted with
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
    /// UTF-16, declares ASCII but holds a non-ASCII character, or has a
    /// UTF-16 byte order mark but declares another encoding (SACM-XML-001).
    #[non_exhaustive]
    Malformed {
        /// Where the input stops being well-formed.
        location: Option<SourceLocation>,
        /// What is wrong, without a position.
        reason: String,
    },
    /// An interchange root (the document element, or each element child
    /// outside the XMI namespace of an `xmi:XMI` element in the XMI 2.5.1
    /// namespace `http://www.omg.org/spec/XMI/20131001`) is not named as one
    /// of the twelve SACM package kinds (SACM-XMI-001). The location is the
    /// `<` of its start tag.
    #[non_exhaustive]
    InvalidRoot {
        /// Where the root's start tag starts.
        location: Option<SourceLocation>,
        /// The root's name as written: `prefix:Local`, or `Local` without a
        /// prefix.
        element_name: String,
    },
    /// The `xmi:XMI` element holds no root: no element child outside the XMI
    /// namespace (SACM-XMI-001). The location is the `<` of its start tag.
    #[non_exhaustive]
    NoPackage {
        /// Where the `xmi:XMI` start tag starts.
        location: Option<SourceLocation>,
    },
    /// Two or more elements carry the same `xmi:id` (SACM-ID-001,
    /// SACM23-VAL-001): two elements, two names, or an element and a name
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
    /// assert_eq!(error.code(), Some(Code::ID_DUPLICATE));
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
    },
    /// The `xmi:id` of an element or of a `name` is empty, which is not an
    /// XML `NCName` and which a [`Document`] cannot hold (SACM-ID-002,
    /// SACM23-VAL-020). The location is the `<` of the element's start tag.
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
    },
    /// The input is well-formed, but this version cannot represent it. It
    /// has no catalog code.
    #[non_exhaustive]
    Unsupported {
        /// Where the content that cannot be represented starts.
        location: Option<SourceLocation>,
        /// What cannot be represented, without a position.
        reason: String,
    },
    /// A property is given more than XMI allows (SACM-MULT-001): a
    /// single-valued one more than once, as an attribute and an element or as
    /// two or more elements, or a multi-valued reference end both as an
    /// attribute and as elements.
    ///
    /// XMI 2.5.1 §9.5.2 rule 2h serializes a property "either as an XML
    /// attribute or as XML elements but not both", and a single-valued
    /// property holds one value, so the input is refused even when the values
    /// are equal, rather than read with one of them dropped. Fix: give the
    /// value once.
    ///
    /// A multi-valued reference end, `metaClaim` or `source` (ADR 0027),
    /// takes any number of ids, in one attribute or in one element each,
    /// but not both: an attribute and an element for one is refused too.
    /// Fix: give every id in the attribute, or each in its own element.
    ///
    /// A `ModelElement`'s `name` (`name:LangString[1]`, formal/23-05-08 §8.6)
    /// is such a property too: two or more `name` elements in no namespace
    /// are refused, even when they are equal (SACM23-XMI-021). So are a
    /// `UtilityElement`'s `content` (§8.7 `[0..1]`) and a `TaggedValue`'s `key`
    /// (§8.12 `[1]`), which are never merged.
    ///
    /// [`location`](Self::location) is the second occurrence, and
    /// [`locations`](Self::locations) every occurrence.
    ///
    /// ```
    /// use sacm::diagnostic::Code;
    /// use sacm::xmi::{self, ReadError, ReadOptions};
    ///
    /// let input = br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
    ///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="arguments">
    ///   <name content="Arguments">
    ///     <content>Arguments</content>
    ///   </name>
    /// </sacm:ArgumentPackage>"#;
    /// let error = xmi::read(input, &ReadOptions::default()).unwrap_err();
    /// assert!(matches!(
    ///     &error,
    ///     ReadError::RepeatedValue { property, .. } if property == "name.content"
    /// ));
    /// assert_eq!(error.code(), Some(Code::MULTIPLICITY_VIOLATION));
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
    /// let error = xmi::read(input, &ReadOptions::default()).unwrap_err();
    /// assert_eq!(
    ///     error.to_string(),
    ///     "the property \"source\" of the element \"i1\" is given both as an attribute \
    ///      and as elements, which XMI 2.5.1 rule 2h forbids, at line 3, column 66; \
    ///      line 4, column 5"
    /// );
    /// ```
    #[non_exhaustive]
    RepeatedValue {
        /// The element that holds the property; for `name.lang`,
        /// `content.value.lang` and the like, the element the name or the
        /// value belongs to. An element without an `xmi:id` is named by its
        /// generated id.
        element: ElementId,
        /// The property: `gid`, `isCitation`, `isAbstract`, `isCounter`,
        /// `citedElement`, `abstractForm`, `metaClaim`, `reasoning`,
        /// `source`, `target`, `structure`, `name`, `name.lang`,
        /// `name.content`, `content`, `content.value.lang`,
        /// `content.value.content`, `key`, `key.value.lang`,
        /// `key.value.content` or `assertionDeclaration`.
        property: String,
        /// Every occurrence, in document order: an attribute at the first
        /// character of its name, an element at its `<`. At least two. Two
        /// or more ids in one reference attribute give one location per id,
        /// each at the attribute's name.
        locations: Vec<SourceLocation>,
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
    /// It is refused in both modes. `Display` gives the literals the
    /// property accepts as its reason.
    ///
    /// ```
    /// use sacm::diagnostic::Code;
    /// use sacm::xmi::{self, ReadError, ReadOptions};
    ///
    /// let input = br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
    ///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="arguments">
    ///   <argumentElement xmi:type="sacm:Claim" xmi:id="c1" assertionDeclaration="Assumed"/>
    /// </sacm:ArgumentPackage>"#;
    /// let error = xmi::read(input, &ReadOptions::default()).unwrap_err();
    /// assert!(matches!(&error, ReadError::InvalidLiteral { value, .. } if value == "Assumed"));
    /// assert_eq!(error.code(), Some(Code::ENUM_INVALID_LITERAL));
    /// assert_eq!(
    ///     error.to_string(),
    ///     "the value \"Assumed\" of the property \"assertionDeclaration\" of the element \"c1\" \
    ///      is not a literal at line 3, column 54: the literals are asserted, needsSupport, \
    ///      assumed, axiomatic, defeated and asCited"
    /// );
    ///
    /// let flag = br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
    ///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="arguments" isAbstract="yes"/>"#;
    /// let error = xmi::read(flag, &ReadOptions::default()).unwrap_err();
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
        /// `isCitation`, `isAbstract` or `isCounter`.
        property: String,
        /// The value as read: for an attribute, after XML attribute-value
        /// normalisation (XML 1.0 §3.3.3); for a value element, its exact
        /// character content.
        value: String,
        /// The first character of the attribute's name, or the `<` of the
        /// value element's start tag.
        location: Option<SourceLocation>,
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
    /// [`InvalidLiteral`](Self::InvalidLiteral), and `None` for
    /// [`Unsupported`](Self::Unsupported).
    #[must_use]
    pub fn code(&self) -> Option<Code> {
        match self {
            Self::Doctype { .. } => Some(Code::XML_DOCTYPE_REJECTED),
            Self::Malformed { .. } => Some(Code::XML_MALFORMED),
            Self::InvalidRoot { .. } | Self::NoPackage { .. } => Some(Code::XMI_INVALID_ROOT),
            Self::DuplicateId { .. } => Some(Code::ID_DUPLICATE),
            Self::InvalidId { .. } => Some(Code::ID_INVALID),
            Self::RepeatedValue { .. } => Some(Code::MULTIPLICITY_VIOLATION),
            Self::InvalidLiteral { .. } => Some(Code::ENUM_INVALID_LITERAL),
            Self::Unsupported { .. } => None,
        }
    }

    /// Where in the input the refusal is: an element at the `<` of its start
    /// tag, an attribute at the first character of its name, text at its
    /// first character that is not literal white space (a CDATA section or
    /// reference at its start). For [`DuplicateId`](Self::DuplicateId), the
    /// first repeat; for [`RepeatedValue`](Self::RepeatedValue), the second
    /// occurrence. `None` only when the input exceeds the parser's
    /// capacity.
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
/// `isAbstract` and `isCounter`, is an `xsd:boolean`.
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
    /// Writes the refusal on one line, as described on [`ReadError`].
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let reason = match self {
            Self::Doctype { .. } => {
                f.write_str("the input has a DOCTYPE, which is refused")?;
                None
            }
            Self::Malformed { reason, .. } => {
                f.write_str("the input is not well-formed XML")?;
                Some(reason.as_str())
            }
            Self::InvalidRoot { element_name, .. } => {
                write!(f, "the root element {element_name:?} is not a SACM package")?;
                None
            }
            Self::NoPackage { .. } => {
                f.write_str("the XMI element holds no SACM package")?;
                None
            }
            // Every location, not only the first repeat.
            Self::DuplicateId { id, locations } => {
                write!(
                    f,
                    "the xmi:id {:?} is carried by {} elements",
                    id.as_str(),
                    locations.len()
                )?;
                return write_locations(f, locations);
            }
            Self::RepeatedValue {
                element,
                property,
                locations,
            } => {
                let is_many = ReferenceEnd::ALL
                    .iter()
                    .any(|end| end.is_many() && end.name() == property);
                if is_many {
                    write!(
                        f,
                        "the property {property:?} of the element {:?} is given both as an \
                         attribute and as elements, which XMI 2.5.1 rule 2h forbids",
                        element.as_str()
                    )?;
                } else {
                    write!(
                        f,
                        "the single-valued property {property:?} of the element {:?} is given {} times",
                        element.as_str(),
                        locations.len()
                    )?;
                }
                return write_locations(f, locations);
            }
            Self::InvalidId { id, .. } => {
                write!(f, "the xmi:id {id:?} is not an NCName")?;
                None
            }
            Self::InvalidLiteral {
                element,
                property,
                value,
                ..
            } => {
                write!(
                    f,
                    "the value {value:?} of the property {property:?} of the element {:?} is not a literal",
                    element.as_str()
                )?;
                Some(literals_reason(property))
            }
            Self::Unsupported { reason, .. } => {
                f.write_str("the reader cannot represent the input")?;
                Some(reason.as_str())
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
/// (XMI 2.5.1 §7.5.3). The document element alone declares the SACM 2.3
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
/// from §11.4's text (`argumentationElement`). A child carries
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
/// 6. on an `ArgumentGroup` (§11.2): `argumentElement`.
///
/// So an `AssertedInference` with everything set starts
/// `xmi:type xmi:id gid isCitation isAbstract citedElement abstractForm
/// assertionDeclaration metaClaim isCounter reasoning source target`. A flag
/// is written only when true, as `"true"` (false, the default, is not
/// written). A reference end is written only when it holds an id, as the ids
/// it holds, in order, repeats included, separated by one space and escaped,
/// held exactly, whether or not they name an element. A reference read in
/// the element form is written in this attribute form. The
/// `assertionDeclaration` is written only when it is not `asserted`, the
/// default (XMI 2.5.1 §9.4.1), as
/// [`AssertionDeclaration::as_str`](crate::model::AssertionDeclaration::as_str)
/// spells it. A
/// named element's first child is its name, `<name lang="…" content="…" />`,
/// with each field written only if present, so an empty field is `=""` and an
/// absent one is no attribute; an element with no name has no `name` child.
/// A name read with an `xmi:id`
/// ([`SacmElement::name_id`](crate::model::SacmElement::name_id)) carries it
/// first: `<name xmi:id="…" lang="…" content="…" />`. An id is written
/// as it is, escaped, and so are a name's fields. An id that is not an XML
/// `NCName`, such as `1 2`, is a validation Error (SACM-ID-002), so a strict
/// write refuses it; a tolerant write writes it as it is, and the output
/// then does not validate against an XMI schema. In attribute values `&`, `<`, `>`, `"`,
/// TAB, LF and CR are escaped; everything else is written as it is.
///
/// Inside a `UtilityElement`, its `content` element is written first, then,
/// on a `TaggedValue`, its `key`, superclass first (XMI 2.5.1 Annex B.5.2). Each is its role
/// element, with its `xmi:id` first if it has one, holding one
/// `<value … />` per value, in order, written as a name is:
/// `<content>`, `<value lang="en" content="…" />`, `</content>`. An empty
/// one is `<content />`, and an absent one is not written. No type attribute
/// is written on a `UtilityElement`, a `content`, a `key` or a `value`.
///
/// # Modes
///
/// In [`Mode::Strict`], the default, `write` writes only a document that
/// [`validation::validate`](crate::validation::validate) reports no Error
/// for, and refuses any other as [`WriteError::Invalid`]; Warnings and Infos
/// never refuse. In [`Mode::Tolerant`] it does not validate, and writes the
/// document as it is held, such as a draft whose elements have no name yet.
/// The two modes write the same bytes for a document both write.
///
/// A later minor release may add a validation Error rule. A strict write may
/// then refuse a document that it wrote before; this is not a breaking
/// change, and the release notes list each such rule (ADR 0022).
///
/// # Errors
///
/// The document is checked before anything is written, and the first failure
/// is returned:
///
/// 1. a document with no root is [`WriteError::NoRoot`];
/// 2. in strict mode only, validation: if `validate` reports an Error, the
///    refusal is [`WriteError::Invalid`], which holds every Error in
///    `validate`'s order;
/// 3. then each element, in [`Document::elements`] order, with all of an
///    element's checks run before the next element's: first its id, where a
///    character outside XML 1.0's `Char` is
///    [`WriteError::UnrepresentableCharacter`], naming the attribute and its
///    first such character; then its gid, its name's `lang`, its name's
///    `content`, and each value of its `content` and then of its `key`,
///    `lang` before `content`, the same way.
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
/// // A package with no name is refused in strict mode, the default, and
/// // written as it is held in tolerant mode.
/// let draft = br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="draft"/>"#;
/// let document = xmi::read(draft, &ReadOptions::default())?.into_document();
/// let error = xmi::write(&document, &WriteOptions::default()).unwrap_err();
/// assert!(matches!(error, WriteError::Invalid { .. }));
/// let text = xmi::write(&document, &WriteOptions::default().with_mode(Mode::Tolerant))?;
/// assert!(text.ends_with("xmi:id=\"draft\" />\n"));
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
/// // Write a draft as it is held, without validating it.
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
/// strict mode, a document that validation reports an Error for. None of the
/// refusals has a catalog code: [`code`](Self::code) is `None`.
/// [`element`](Self::element) names the element refused, where there is one,
/// and [`diagnostics`](Self::diagnostics) gives the validation Errors of
/// [`Invalid`](Self::Invalid).
///
/// # Display
///
/// `Display` writes one line that starts in lower case, without trailing
/// punctuation. Ids are quoted with `{:?}`, so line breaks in them never break
/// the line. [`Invalid`](Self::Invalid) gives the number of Errors and the
/// code and first affected id of the first one, introduced by `the first is`
/// when there are several. For example:
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
    /// The document has no root, so there is no document element to write.
    /// Add a package as a root. This variant has no fields and will not gain
    /// any.
    NoRoot,
    /// An element's id or gid, its name's `lang` or `content`, or the `lang`
    /// or `content` of a value of its `content` or `key`, holds a character
    /// that XML 1.0 cannot represent: one outside its `Char` production, such
    /// as U+0000, another C0 control other than TAB, LF and CR, U+FFFE or
    /// U+FFFF.
    ///
    /// ```
    /// use sacm::edit::{self, CreateArgumentPackage};
    /// use sacm::model::{Document, LangString};
    /// use sacm::xmi::{self, WriteAttribute, WriteError, WriteOptions};
    ///
    /// // An edit stores a name exactly, so it can hold a character XML cannot.
    /// let mut doc = Document::new();
    /// let name = LangString::new("bad\u{1}");
    /// edit::apply(&mut doc, CreateArgumentPackage::root(name).with_id("arguments")).unwrap();
    /// let error = xmi::write(&doc, &WriteOptions::default()).unwrap_err();
    /// assert!(matches!(
    ///     error,
    ///     WriteError::UnrepresentableCharacter {
    ///         attribute: WriteAttribute::NameContent,
    ///         character: '\u{1}',
    ///         ..
    ///     }
    /// ));
    /// assert_eq!(
    ///     error.to_string(),
    ///     "the element \"arguments\" cannot be written: its name's content holds U+0001, \
    ///      which XML 1.0 cannot represent"
    /// );
    /// ```
    #[non_exhaustive]
    UnrepresentableCharacter {
        /// The element whose id, gid, name, `content` or `key` holds the
        /// character.
        element: ElementId,
        /// The first such character in the attribute.
        character: char,
        /// The attribute that holds it.
        attribute: WriteAttribute,
    },
    /// In strict mode, [`validation::validate`](crate::validation::validate)
    /// reports an Error for the document (ADR 0022). `diagnostics` holds every
    /// Error, in `validate`'s order; each carries its own code and the ids of
    /// the elements it affects. Fix the findings, or write the document as it
    /// is held with `WriteOptions::default().with_mode(Mode::Tolerant)`.
    ///
    /// ```
    /// use sacm::diagnostic::Code;
    /// use sacm::xmi::{self, Mode, ReadOptions, WriteError, WriteOptions};
    ///
    /// // An ArgumentPackage with no name, which SACM requires (§8.6).
    /// let input = br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
    ///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="draft"/>"#;
    /// let document = xmi::read(input, &ReadOptions::default())?.into_document();
    /// let error = xmi::write(&document, &WriteOptions::default()).unwrap_err();
    /// assert!(matches!(error, WriteError::Invalid { .. }));
    /// assert_eq!(error.diagnostics()[0].code(), Code::MULTIPLICITY_VIOLATION);
    /// assert_eq!(
    ///     error.to_string(),
    ///     "the document cannot be written in strict mode: validation reports 1 error, \
    ///      SACM-MULT-001 on \"draft\""
    /// );
    ///
    /// // Tolerant mode writes it as it is held.
    /// let tolerant = WriteOptions::default().with_mode(Mode::Tolerant);
    /// let text = xmi::write(&document, &tolerant)?;
    /// assert!(text.contains("xmi:id=\"draft\" />"));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[non_exhaustive]
    Invalid {
        /// Every validation Error, in `validate`'s order. Never empty.
        diagnostics: Vec<Diagnostic>,
    },
}

impl WriteError {
    /// The element refused, if the refusal names one: the element of
    /// [`UnrepresentableCharacter`](Self::UnrepresentableCharacter). `None`
    /// for [`NoRoot`](Self::NoRoot), and for [`Invalid`](Self::Invalid),
    /// whose [`diagnostics`](Self::diagnostics) name the elements they affect.
    #[must_use]
    pub fn element(&self) -> Option<&ElementId> {
        match self {
            Self::NoRoot | Self::Invalid { .. } => None,
            Self::UnrepresentableCharacter { element, .. } => Some(element),
        }
    }

    /// The catalog code of the refusal: `None` for every variant in this
    /// release. The diagnostics of [`Invalid`](Self::Invalid) carry their own
    /// codes.
    #[must_use]
    pub fn code(&self) -> Option<Code> {
        match self {
            Self::NoRoot | Self::UnrepresentableCharacter { .. } | Self::Invalid { .. } => None,
        }
    }

    /// The validation Errors of [`Invalid`](Self::Invalid), in `validate`'s
    /// order; empty for every other variant.
    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        match self {
            Self::Invalid { diagnostics } => diagnostics,
            Self::NoRoot | Self::UnrepresentableCharacter { .. } => &[],
        }
    }
}

impl fmt::Display for WriteError {
    /// Writes the refusal on one line, as described on [`WriteError`].
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoRoot => f.write_str("the document has no root to write"),
            Self::UnrepresentableCharacter {
                element,
                character,
                attribute,
            } => write!(
                f,
                "the element {:?} cannot be written: its {attribute} holds U+{:04X}, which XML 1.0 cannot represent",
                element.as_str(),
                u32::from(*character)
            ),
            Self::Invalid { diagnostics } => {
                let count = diagnostics.len();
                let errors = if count == 1 { "error" } else { "errors" };
                write!(
                    f,
                    "the document cannot be written in strict mode: validation reports {count} {errors}"
                )?;
                if let Some(first) = diagnostics.first() {
                    let lead = if count == 1 { "" } else { "the first is " };
                    write!(f, ", {lead}{}", first.code())?;
                    if let Some(id) = first.affected().first() {
                        write!(f, " on {:?}", id.as_str())?;
                    }
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for WriteError {}

/// The attribute of an element that a [`WriteError`] names.
///
/// Later releases add variants as they write more attributes, so match it
/// with a fallback arm.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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
}

impl fmt::Display for WriteAttribute {
    /// Writes `xmi:id`, `gid`, `name's lang`, `name's content`, `content
    /// value's lang` and so on.
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
    use crate::diagnostic::SourceLocation;
    use crate::model::ElementId;

    /// A multi-valued end given as an attribute and as elements is not
    /// called single-valued (ADR 0027); every other property is.
    #[test]
    fn a_repeated_value_names_a_multi_valued_end_by_rule_2h() {
        let repeated = |property: &str| ReadError::RepeatedValue {
            element: ElementId::new("i1"),
            property: property.to_owned(),
            locations: [SourceLocation::at(1, 2), SourceLocation::at(3, 4)]
                .into_iter()
                .flatten()
                .collect(),
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
