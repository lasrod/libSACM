//! Diagnostics: findings about the content of a SACM document.
//!
//! A [`Diagnostic`] reports something about the content, such as a reference
//! that does not resolve or an element kept from a non-SACM dialect. It is not
//! an error: an error means an operation could not proceed and comes back as
//! the `Err` of a `Result`, while diagnostics come back alongside a result
//! that succeeded.
//!
//! Each diagnostic carries a stable [`Code`] from the
//! [diagnostics catalog][catalog], a [`Severity`], the matrix row and clause
//! it enforces, the ids of the affected elements, an optional
//! [`SourceLocation`], a message, and the
//! [template and arguments](Diagnostic::template) the message is made from.
//!
//! [`validation::validate`](crate::validation::validate) and
//! [`xmi::read`](crate::xmi::read) return diagnostics, and a strict
//! [`xmi::write`](crate::xmi::write) refuses a document with the validation
//! Errors in [`WriteError::Invalid`](crate::xmi::WriteError::Invalid).
//! Every function that returns diagnostics returns them sorted by
//! [`Diagnostic`]'s order. [`has_errors`] says whether a list holds a
//! diagnostic of severity [`Severity::Error`].
//!
//! Diagnostics are read, not built: only this crate creates them. The
//! `test-support` feature makes the constructors and `with_*` builders
//! public so that tests can build diagnostics. They are not stable API, even
//! though docs.rs shows them. With the `serde` feature a diagnostic also
//! serializes, and reads back in the stable form of ADR 0030. A
//! deserialized `Diagnostic`, `Mutation` or `EditError` is a record:
//! [`has_errors`] reads a deserialized diagnostic's severity as given, and
//! nothing else in the API takes one as input.
//!
//! # Examples
//!
//! ```
//! use sacm::diagnostic::{self, Diagnostic};
//!
//! /// Prints each diagnostic on its own line and says whether any is an error.
//! fn report(diagnostics: &[Diagnostic]) -> bool {
//!     for diagnostic in diagnostics {
//!         eprintln!("{diagnostic}");
//!     }
//!     diagnostic::has_errors(diagnostics)
//! }
//!
//! assert!(!report(&[]));
//! ```
//!
//! [catalog]: https://github.com/lasrod/libSACM/blob/main/docs/sacm/diagnostics.md

use std::borrow::Cow;
use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::num::NonZeroU32;

use crate::model::ElementId;

pub(crate) mod template;

use template::Template;

/// How serious a diagnostic is: `Info < Warning < Error`.
///
/// ```
/// use sacm::diagnostic::Severity;
///
/// assert!(Severity::Warning < Severity::Error);
/// assert_eq!(Severity::Info.to_string(), "info");
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "lowercase")
)]
#[non_exhaustive]
pub enum Severity {
    /// Information: nothing is wrong.
    Info,
    /// Something is questionable but the content is usable.
    Warning,
    /// The content does not conform.
    Error,
}

impl Severity {
    /// The severity as lowercase text: `info`, `warning` or `error`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }
}

impl fmt::Display for Severity {
    /// Writes [`as_str`](Self::as_str).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(self.as_str())
    }
}

/// A diagnostic code from the [diagnostics catalog][catalog], such as
/// `SACM-REF-001`.
///
/// Codes are stable API: a code's meaning never changes. Codes compare and
/// order by their text. Each code arrives with the matrix row that emits it;
/// the codes defined so far are the associated constants below.
///
/// ```
/// use sacm::diagnostic::Code;
///
/// assert_eq!(Code::XML_MALFORMED.to_string(), "SACM-XML-001");
/// ```
///
/// [catalog]: https://github.com/lasrod/libSACM/blob/main/docs/sacm/diagnostics.md
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Code(&'static str);

/// Defines the catalog codes, each written as an associated constant of
/// [`Code`], and puts every one of them, in order, in the crate-private
/// `Code::CATALOG`, so that the constants and the codes serde reads back
/// cannot drift apart (ADR 0030).
macro_rules! catalog {
    ($(
        $(#[$doc:meta])*
        pub const $name:ident: Code = Code::$constructor:ident($code:literal);
    )+) => {
        impl Code {
            $(
                $(#[$doc])*
                pub const $name: Code = Code::$constructor($code);
            )+

            /// Every code of the catalog, in the order of the constants:
            /// the codes a [`Code`] deserializes from (ADR 0030 R5).
            #[cfg_attr(
                not(any(feature = "serde", test)),
                expect(dead_code, reason = "only serde reads the table back")
            )]
            pub(crate) const CATALOG: &[Code] = &[$(Code::$name),+];
        }
    };
}

catalog! {
    /// SACM-XML-001: malformed XML. The code of
    /// [`ReadError::Malformed`](crate::xmi::ReadError::Malformed)
    /// (LIB-SEC-002).
    pub const XML_MALFORMED: Code = Code::from_static("SACM-XML-001");

    /// SACM-SEC-001: the input has a DOCTYPE, which may declare entities, so
    /// it is refused. The code of
    /// [`ReadError::Doctype`](crate::xmi::ReadError::Doctype) (LIB-SEC-001).
    /// Named after the C++ library's `kXmlDoctypeRejected`.
    pub const XML_DOCTYPE_REJECTED: Code = Code::from_static("SACM-SEC-001");

    /// SACM-XML-002: the input is in an encoding this version does not read:
    /// one other than UTF-8, ASCII and UTF-16 with a byte order mark, by its
    /// mark, its first bytes or its declaration. Both modes refuse it as
    /// [`ReadError::Unsupported`](crate::xmi::ReadError::Unsupported), with
    /// the label `(a limit of this version)` (LIB-API-009, LIB-API-017).
    /// Fix: convert the file to UTF-8.
    pub const XML_ENCODING_NOT_READ: Code = Code::from_static("SACM-XML-002");

    /// SACM-XML-003: an element's id, name or other text holds a character
    /// that XML 1.0 cannot represent, such as U+0001, so the element cannot
    /// be written. The code of
    /// [`WriteError::UnrepresentableCharacter`](crate::xmi::WriteError::UnrepresentableCharacter)
    /// (SACM23-XMI-011, LIB-API-017). Fix: remove the character with an
    /// edit.
    pub const XML_UNREPRESENTABLE_CHARACTER: Code = Code::from_static("SACM-XML-003");

    /// SACM-SEC-002: the input has more nodes, attributes or namespaces than
    /// the parser can hold, or an element the document could not hold, which
    /// checked input never gives. Both modes refuse it as
    /// [`ReadError::Unsupported`](crate::xmi::ReadError::Unsupported), with
    /// the label `(a limit of this version)` (LIB-API-011, LIB-API-017).
    pub const PARSER_LIMIT: Code = Code::from_static("SACM-SEC-002");

    /// SACM-XMI-001: an interchange root (the document element, or each
    /// element child outside the XMI namespace of an `xmi:XMI` element in the
    /// XMI 2.5.1 namespace `http://www.omg.org/spec/XMI/20131001`) is not
    /// named as a SACM package kind, or the XMI element holds no root. The
    /// code of
    /// [`ReadError::InvalidRoot`](crate::xmi::ReadError::InvalidRoot) and
    /// [`ReadError::NoPackage`](crate::xmi::ReadError::NoPackage)
    /// (SACM23-XMI-001), and of
    /// [`WriteError::NoRoot`](crate::xmi::WriteError::NoRoot), a document
    /// with no package to write (LIB-API-017). Named after the C++ library's
    /// `kXmiInvalidRoot`.
    pub const XMI_INVALID_ROOT: Code = Code::from_static("SACM-XMI-001");

    /// SACM-XMI-002: a root is in a namespace that is not the pinned SACM 2.3
    /// namespace `http://www.omg.org/spec/SACM/20220301`, or in none. In this
    /// release both modes refuse it as
    /// [`ReadError::Unsupported`](crate::xmi::ReadError::Unsupported), with
    /// this code; the Warning of a tolerant read is reserved for
    /// LIB-COMPAT-004, which is to read it (D-NS-PIN, LIB-API-017).
    pub const XMI_NAMESPACE_NOT_PINNED: Code = Code::from_static("SACM-XMI-002");

    /// SACM-XMI-004: an element has no `xmi:id`, which XMI 2.5.1 §7.6.1
    /// makes optional. [`xmi::read`](crate::xmi::read) reads the element
    /// with a generated id, `<snake_case kind>_<n>` as an edit generates one,
    /// that no `xmi:id` in the input holds, and reports one Info per element
    /// in both modes, affecting the generated id (SACM23-XMI-020, ADR 0024).
    /// The generated id is an ordinary id: [`xmi::write`](crate::xmi::write)
    /// writes it, so the saved file carries it. Nothing needs fixing.
    pub const XMI_MISSING_ID: Code = Code::from_static("SACM-XMI-004");

    /// SACM-XMI-005: a child under a containment role whose declared type is
    /// abstract carries no `xmi:type` (or `xsi:type`), so its class is not
    /// known (XMI 2.5.1 §7.6.3). Its element name is the role, so no class
    /// can be inferred, and none is guessed. The code of
    /// [`ReadError::MissingType`](crate::xmi::ReadError::MissingType) in
    /// strict mode (LIB-COMPAT-010, ADR 0022). A tolerant read keeps the
    /// child whole as preserved content of its parent, its `xmi:id`s in the
    /// id space, with one Warning at its start tag affecting the parent
    /// (LIB-COMPAT-001, ADR 0035). Fix: give the child an `xmi:type` naming
    /// its class, such as `xmi:type="sacm:Claim"`. An untyped role element
    /// with `xlink:href` or `xmi:href` is rightly this code, because only an
    /// unqualified `href` is a reference form (XMI.xsd `LinkAttribs`).
    pub const XMI_MISSING_TYPE: Code = Code::from_static("SACM-XMI-005");

    /// SACM-XMI-003: content that neither the SACM XML Schema nor the XMI
    /// 2.5.1 production rules allow where it stands, such as a vendor
    /// attribute or child element, an attribute the element's class lacks,
    /// a child whose type the role does not admit, or text in element-only
    /// content, and an attribute or text on the `xmi:XMI` element.
    /// [`xmi::read`](crate::xmi::read) keeps it exactly as written, in both
    /// modes, as preserved content of the element it sits in, or of the
    /// document for content on `xmi:XMI`, and reports one per construct,
    /// affecting that element: a Warning in tolerant mode and an Error in
    /// strict mode (LIB-COMPAT-003, ADR 0033). A preserved attribute whose
    /// prefix `sacm` or `xmi` is bound to another namespace, and an attribute
    /// or text on the `xmi:XMI` element, are refused by a strict read as
    /// [`ReadError::Unsupported`](crate::xmi::ReadError::Unsupported) with
    /// this code (LIB-API-017); a tolerant read keeps the first under the
    /// first prefix `nsN` unbound at its element, which the message names,
    /// and the second as the document's preserved content (ADR 0035).
    /// A tolerant [`xmi::write`](crate::xmi::write) writes it back; a strict
    /// one refuses it (SACM-XMI-006). Named after the C++ library's
    /// `kXmiUnknownElement`; it covers attributes and text too. Fix: remove
    /// the content, or keep it in an `xmi:Extension` once SACM23-XMI-019
    /// reads one.
    pub const XMI_UNKNOWN_CONTENT: Code = Code::from_static("SACM-XMI-003");

    /// SACM-XMI-006: a strict [`xmi::write`](crate::xmi::write) refuses a
    /// document that holds preserved content (SACM-XMI-003, SACM-XMI-007,
    /// SACM-XMI-014), because it cannot show that content it did not read
    /// is schema-valid. The code of
    /// [`WriteError::PreservedContent`](crate::xmi::WriteError::PreservedContent)
    /// (SACM23-XMI-008, ADR 0033, ADR 0035). Fix: write it in tolerant mode,
    /// which writes the content back, or delete the elements that hold it.
    /// Content the document holds outside its elements can be written only
    /// in tolerant mode in this release.
    pub const XMI_STRICT_SAVE_REFUSED: Code = Code::from_static("SACM-XMI-006");

    /// SACM-XMI-007: a reference to an element of another document, an
    /// `href` with a document part such as `other.xmi#c9`, or `XLink`
    /// attributes, which this release does not read yet. A tolerant
    /// [`xmi::read`](crate::xmi::read) keeps the reference element, or the
    /// role element in reference form, whole as preserved content, with one
    /// Warning; the reference end stays empty. A strict read refuses it as
    /// [`ReadError::Unsupported`](crate::xmi::ReadError::Unsupported) with
    /// this code (LIB-API-011, LIB-API-017, ADR 0035; SACM23-XMI-027 reads
    /// it). It is never
    /// SACM-XMI-003: such a reference conforms.
    pub const XMI_EXTERNAL_HREF: Code = Code::from_static("SACM-XMI-007");

    /// SACM-XMI-010: an `xsi:type` attribute is used. XMI 2.5.1 §9.5.2 rule
    /// 2g allows it only if `org.omg.xmi.useSchemaExtensions` is true, and
    /// SACM 2.3 sets no such tag. [`xmi::read`](crate::xmi::read) reports one
    /// per attribute, an Error in strict mode and a Warning in tolerant mode,
    /// and reads the type as if it were `xmi:type` (LIB-COMPAT-001). Fix it by
    /// writing `xmi:type`, as [`xmi::write`](crate::xmi::write) does.
    pub const XMI_XSI_TYPE: Code = Code::from_static("SACM-XMI-010");

    /// SACM-XMI-011: an `xmi:version` attribute on the `xmi:XMI` element or
    /// a root is present and ignored, because the XMI namespace identifies
    /// the version (XMI 2.5.1 §7.5.3). [`xmi::read`](crate::xmi::read)
    /// reports one per attribute, as Info in both modes (LIB-COMPAT-001). An
    /// `xmi:version` on any other element is an attribute XMI.xsd does not
    /// define, kept as preserved content with SACM-XMI-003
    /// ([`XMI_UNKNOWN_CONTENT`](Self::XMI_UNKNOWN_CONTENT)). Nothing
    /// is lost and nothing needs fixing. To silence it, remove the attribute,
    /// or save the document again with [`xmi::write`](crate::xmi::write),
    /// which does not write it.
    pub const XMI_VERSION_IGNORED: Code = Code::from_static("SACM-XMI-011");

    /// SACM-XMI-012: a name or a text is written in a shape of the
    /// assurance-forge C++ library that maps one to one onto its standard
    /// form, such as a `name="…"` attribute or text in a `name`. A tolerant
    /// [`xmi::read`](crate::xmi::read) reads it as its standard form and
    /// reports one Warning per occurrence; a strict read refuses it as
    /// [`ReadError::LegacyShape`](crate::xmi::ReadError::LegacyShape)
    /// (LIB-COMPAT-008, ADR 0022 amendment 1). Both saves write the standard
    /// form. Fix: read the file in tolerant mode and save it again.
    pub const XMI_LEGACY_SHAPE: Code = Code::from_static("SACM-XMI-012");

    /// SACM-XMI-013: a name spelled where formal/23-05-08 and ptc/22-03-13
    /// disagree and the crate follows the other (D-TEXT-MODEL): the type
    /// `ArtifactAssertedRelationship` under a role that admits an
    /// `ArtifactAssetRelationship`, an Event's `occurece`, and
    /// `argumentationElement` as an ArgumentPackage's role or an
    /// `ArgumentGroup`'s end.
    ///
    /// A tolerant [`xmi::read`](crate::xmi::read) reads it as the resolved
    /// name, with one Warning per construct read, at its start tag or
    /// attribute name, affecting the element the spelling belongs to; a value
    /// given under both names is read from the resolved name, and the spelled
    /// one is a repeat (SACM-MULT-001). A strict read keeps the construct as
    /// preserved content of the element it sits in, with one Error, so a
    /// strict write refuses the document (SACM-XMI-006).
    /// [`xmi::write`](crate::xmi::write) writes the resolved names. Fix: read
    /// the file in tolerant mode and save it again (SACM23-XMI-009, ADR 0034
    /// amendment 2).
    pub const XMI_SPELLING_VARIANT: Code = Code::from_static("SACM-XMI-013");

    /// SACM-XMI-014: conforming content this release does not read yet,
    /// which a tolerant [`xmi::read`](crate::xmi::read) keeps as preserved
    /// content with one Warning: an element of a kind not read yet (such as
    /// a `TerminologyPackage` or an `ArtifactPackageInterface`), a package's
    /// `interface` end, or XMI metadata (such as
    /// `xmi:uuid`, `xsi:schemaLocation` or `xmi:Documentation`). It is not a
    /// defect: the message ends naming the row that reads it. A strict read refuses it as
    /// [`ReadError::Unsupported`](crate::xmi::ReadError::Unsupported) with
    /// this code, and a strict write refuses a document that holds it
    /// (SACM-XMI-006) (LIB-API-011, LIB-API-017, ADR 0035).
    pub const XMI_UNREAD_CONTENT: Code = Code::from_static("SACM-XMI-014");

    /// SACM-XMI-015: an element named `XMI` is not in the XMI 2.5.1
    /// namespace `http://www.omg.org/spec/XMI/20131001`, so it is not read as
    /// the XMI element. Both modes refuse it as
    /// [`ReadError::Unsupported`](crate::xmi::ReadError::Unsupported); a
    /// tolerant read is to read it under LIB-COMPAT-002 (LIB-API-017).
    pub const XMI_WRAPPER_NAMESPACE: Code = Code::from_static("SACM-XMI-015");

    /// SACM-XMI-016: a strict [`xmi::write`](crate::xmi::write) refuses a
    /// document whose output would not be schema-valid: an `xmi:id` that is
    /// no `NCName`, or a reference to no `xmi:id` the output writes. The code
    /// of [`WriteError::Invalid`](crate::xmi::WriteError::Invalid), whose
    /// diagnostics carry the codes of the schema errors (LIB-COMPAT-007,
    /// LIB-API-017). Fix: correct the ids and references, or write in
    /// tolerant mode.
    pub const XMI_STRICT_SAVE_INVALID: Code = Code::from_static("SACM-XMI-016");

    /// SACM-MULT-001: a property has more or fewer values than its
    /// multiplicity allows. [`validation::validate`](crate::validation::validate)
    /// reports it, as an Error, for a `ModelElement` with no name (§8.6
    /// `name:LangString[1]`, SACM23-VAL-018); fix it by giving the element a
    /// name. It reports it as a Warning for a `ModelElement` with more than
    /// one description (§8.6 `description:Description[0..1]`,
    /// SACM23-VAL-011): every description is kept, and a strict save is not
    /// refused; fix it by merging them into one. It reports it as an Error
    /// for an asserted relationship with no `source` (§11.13
    /// `source:ArgumentAsset[1..*]`) or no `target` (`target:ArgumentAsset[1]`),
    /// one per missing end (SACM23-VAL-005); fix it by giving the missing
    /// end. It is also the code of
    /// [`EditError::TooFewReferences`](crate::edit::EditError::TooFewReferences)
    /// and [`EditError::TooManyReferences`](crate::edit::EditError::TooManyReferences),
    /// an edit that would give a reference end fewer or more ids than its
    /// bounds allow, and of
    /// [`ReadError::RepeatedValue`](crate::xmi::ReadError::RepeatedValue)
    /// in strict mode; a tolerant read reads the first occurrence and keeps
    /// each later one as preserved content, with one Warning at it
    /// (LIB-COMPAT-001, ADR 0035).
    ///
    /// ```
    /// use sacm::diagnostic::Code;
    ///
    /// assert_eq!(Code::MULTIPLICITY_VIOLATION.as_str(), "SACM-MULT-001");
    /// ```
    pub const MULTIPLICITY_VIOLATION: Code = Code::from_static("SACM-MULT-001");

    /// SACM-ENUM-001: a value is not one of its type's literals. The code of
    /// [`ReadError::InvalidLiteral`](crate::xmi::ReadError::InvalidLiteral),
    /// refused in strict mode: an `assertionDeclaration` that is not exactly
    /// one of the six `AssertionDeclaration` literals (SACM23-VAL-006), and
    /// an `isCitation`, `isAbstract` or `isCounter` (SACM23-ARG-007) that is
    /// not one of the `xsd:boolean` literals `true`, `false`, `1` and `0`,
    /// with white space around them collapsed (SACM23-BASE-007,
    /// SACM23-BASE-008). A tolerant read keeps the attribute or value
    /// element as preserved content, with one Warning, and holds the
    /// default, `asserted` or false (SACM23-VAL-006, ADR 0035). Fix: write
    /// one of
    /// `asserted`, `needsSupport`, `assumed`, `axiomatic`, `defeated` and
    /// `asCited`, or one of `true` and `false`, spelled and cased exactly.
    pub const ENUM_INVALID_LITERAL: Code = Code::from_static("SACM-ENUM-001");

    /// SACM-CMD-001: an operation's target is not in the document. The code
    /// of [`EditError::ParentNotFound`](crate::edit::EditError::ParentNotFound)
    /// (LIB-EDIT-001), of
    /// [`EditError::ReferenceNotFound`](crate::edit::EditError::ReferenceNotFound),
    /// a referenced id that names no element (SACM23-ARG-005, -008, -010,
    /// LIB-EDIT-009), of
    /// [`EditError::ElementNotFound`](crate::edit::EditError::ElementNotFound),
    /// an element to edit that is not in the document (LIB-EDIT-008 to
    /// LIB-EDIT-012), of
    /// [`EditError::ReferenceNotHeld`](crate::edit::EditError::ReferenceNotHeld),
    /// an id to remove that the end does not hold (LIB-EDIT-009), and of
    /// [`EditError::ChildNotFound`](crate::edit::EditError::ChildNotFound),
    /// an id to reorder that is no child or root (LIB-EDIT-010). Create the
    /// element first, or correct its id. Ids are compared exactly, case
    /// included.
    pub const CMD_TARGET_NOT_FOUND: Code = Code::from_static("SACM-CMD-001");

    /// SACM-CMD-002: a delete would leave a reference dangling, and its
    /// reference policy is `RejectIfReferenced`. The code of
    /// [`EditError::Referenced`](crate::edit::EditError::Referenced)
    /// (LIB-EDIT-005). Choose `DeleteReferencingRelationships` or
    /// `ScrubReferences`, or remove the reference first. Named after the C++
    /// library's code.
    ///
    /// ```
    /// use sacm::diagnostic::Code;
    ///
    /// assert_eq!(Code::CMD_DELETE_REFERENCED.as_str(), "SACM-CMD-002");
    /// ```
    pub const CMD_DELETE_REFERENCED: Code = Code::from_static("SACM-CMD-002");

    /// SACM-CMD-003: a preview is applied to a document that changed after
    /// the preview was taken, or to another document. The code of
    /// [`EditError::PreviewExpired`](crate::edit::EditError::PreviewExpired)
    /// (LIB-EDIT-007). Preview the operation again. Named after the C++
    /// library's code.
    pub const CMD_PREVIEW_EXPIRED: Code = Code::from_static("SACM-CMD-003");

    /// SACM-CMD-004: a create operation's caller-given id is already in the
    /// document. The code of
    /// [`EditError::DuplicateId`](crate::edit::EditError::DuplicateId)
    /// (LIB-EDIT-001). Ids are unique across the whole document, not per
    /// parent. Choose another id, or leave out `with_id` to have one
    /// generated.
    pub const CMD_DUPLICATE_ID: Code = Code::from_static("SACM-CMD-004");

    /// SACM-CMD-005: the parent cannot contain an element of the requested
    /// kind. The code of
    /// [`EditError::InvalidParent`](crate::edit::EditError::InvalidParent)
    /// (LIB-EDIT-001). Choose a parent whose class can hold the new kind. For
    /// example, a Claim goes in an ArgumentPackage, and an ArgumentPackage in
    /// an AssuranceCasePackage or another ArgumentPackage.
    pub const CMD_INVALID_PARENT: Code = Code::from_static("SACM-CMD-005");

    /// SACM-CMD-006: a delete names an element that contains elements other
    /// than `UtilityElement`s, and its package policy is `RejectIfNonEmpty`.
    /// The code of [`EditError::NotEmpty`](crate::edit::EditError::NotEmpty)
    /// (LIB-EDIT-006). Choose `DeleteRecursively`, or delete the contents
    /// first. Named after the C++ library's code.
    pub const CMD_PACKAGE_NOT_EMPTY: Code = Code::from_static("SACM-CMD-006");

    /// SACM-CMD-007: a delete would leave a reference dangling across a
    /// package boundary, and its cross-package policy is
    /// `RejectIfExternalReferencesExist`. The code of
    /// [`EditError::CrossPackageReference`](crate::edit::EditError::CrossPackageReference)
    /// (LIB-EDIT-006). Choose `ApplyReferencePolicy` with a reference policy
    /// that handles the reference, or remove it first. Named after the C++
    /// library's code.
    pub const CMD_EXTERNAL_REFERENCES: Code = Code::from_static("SACM-CMD-007");

    /// SACM-CMD-008: the operation does not apply to the element's kind. The
    /// element exists, but its class has no such feature: a `name` on a
    /// `UtilityElement` (§8.6 gives names to `ModelElement`s), `isCounter` on
    /// a kind that is no `AssertedRelationship` (§11.13), an
    /// `assertionDeclaration` on one that is no Assertion (§11.10), a
    /// `content` on one that is no `UtilityElement` (§8.7), a `key` on one
    /// that is no `TaggedValue` (§8.12), or a reference end the kind does not
    /// own, such as a `source` on a Claim. The code of
    /// [`EditError::FeatureNotApplicable`](crate::edit::EditError::FeatureNotApplicable)
    /// (LIB-EDIT-008, LIB-EDIT-009, LIB-EDIT-012). Edit an element whose kind
    /// has the feature, or correct the id.
    ///
    /// ```
    /// use sacm::diagnostic::Code;
    ///
    /// assert_eq!(Code::CMD_FEATURE_NOT_APPLICABLE.as_str(), "SACM-CMD-008");
    /// ```
    pub const CMD_FEATURE_NOT_APPLICABLE: Code = Code::from_static("SACM-CMD-008");

    /// SACM-CMD-009: a reorder's list does not give one order: it names no
    /// element, or names one more than once. The code of
    /// [`EditError::InvalidOrder`](crate::edit::EditError::InvalidOrder)
    /// (LIB-EDIT-010). Name each child, or each root, to move exactly once,
    /// in the new order; the children not named keep their positions.
    ///
    /// ```
    /// use sacm::diagnostic::Code;
    ///
    /// assert_eq!(Code::CMD_INVALID_ORDER.as_str(), "SACM-CMD-009");
    /// ```
    pub const CMD_INVALID_ORDER: Code = Code::from_static("SACM-CMD-009");

    /// SACM-ID-001: two or more `xmi:id`s in the input have the same value,
    /// on elements or on `name`s; XMI 2.5.1 §7.6.1 requires them to be unique
    /// within the file. Values are compared after white space around an
    /// `NCName` is trimmed. The code of
    /// [`ReadError::DuplicateId`](crate::xmi::ReadError::DuplicateId)
    /// (SACM23-VAL-001), which gives the location of every element carrying
    /// the value. Fix: give each element and each `name` its own id, or leave
    /// a `name`'s out.
    ///
    /// ```
    /// use sacm::diagnostic::Code;
    ///
    /// assert_eq!(Code::ID_DUPLICATE.as_str(), "SACM-ID-001");
    /// ```
    pub const ID_DUPLICATE: Code = Code::from_static("SACM-ID-001");

    /// SACM-ID-002: an id is not an XML `NCName` (XMI 2.5.1 §7.6.1: `xmi:id`
    /// is an `xsd:ID`). An `NCName` starts with a letter or `_` and continues
    /// with letters, digits, `-`, `.` or `_`; it has no spaces and no `:`.
    ///
    /// It is the code of
    /// [`EditError::InvalidId`](crate::edit::EditError::InvalidId)
    /// (LIB-EDIT-001): give an `NCName`, or leave out `with_id` to have an id
    /// generated. It is the code of
    /// [`EditError::InvalidReference`](crate::edit::EditError::InvalidReference),
    /// a referenced id, given to a create, `SetReferences` or `AddReference`
    /// and not already held, that holds XML white space, so it cannot be
    /// written as one id. It is the code of
    /// [`ReadError::InvalidId`](crate::xmi::ReadError::InvalidId) for an
    /// empty `xmi:id` on an element or a `name` in strict mode; a tolerant
    /// read gives such an element a generated id, and a `name` none, with
    /// one Warning (ADR 0035).
    /// [`validation::validate`](crate::validation::validate) reports it as an
    /// Error for every other id that is not an `NCName`, a name's `xmi:id`
    /// included (SACM23-VAL-020), and a strict
    /// [`xmi::write`](crate::xmi::write) refuses such an id (schema check,
    /// not `validate`). An id with white space around an `NCName` is first
    /// read trimmed (SACM-ID-004).
    pub const ID_INVALID: Code = Code::from_static("SACM-ID-002");

    /// SACM-ID-003: two or more elements hold the same `gid`, which §8.2
    /// requires to be "unique within the scope of the model instance".
    /// [`validation::validate`](crate::validation::validate) reports one
    /// Error per repeated value, affecting every holder in document order
    /// (SACM23-VAL-002). An absent or empty (`""`) `gid` is not compared;
    /// every other value is compared exactly, letter case and white space
    /// included (D-GID-EMPTY). A `gid` is not an `xmi:id` (SACM-ID-001). Fix:
    /// give each element its own `gid`, or remove the copies.
    pub const GID_DUPLICATE: Code = Code::from_static("SACM-ID-003");

    /// SACM-ID-004: an `xmi:id` has XML white space (space, TAB, LF or CR)
    /// around an `NCName`, so it is read trimmed: an `xmi:id` is an `xsd:ID`,
    /// whose white space is collapsed (XMI 2.5.1 §7.6.1; XML Schema 1.0 Part 2
    /// §4.3.6). [`xmi::read`](crate::xmi::read) reports one Info per attribute in both
    /// modes, quoting the value as read, and affecting the trimmed id, or,
    /// for the id of a `name`, the element that owns the name
    /// (SACM23-XMI-025, ADR 0024). [`xmi::write`](crate::xmi::write) writes
    /// the trimmed id. Nothing needs fixing.
    pub const ID_TRIMMED: Code = Code::from_static("SACM-ID-004");

    /// SACM-PKG-001: a package holds content its clause does not allow.
    /// [`validation::validate`](crate::validation::validate) reports it as an
    /// Error for an `ArgumentPackage` that nests a plain `ArgumentPackage`
    /// and also holds another argumentation element (§11.4: "it is only
    /// allowed to contain `ArgumentPackages`"), one per such element, affecting
    /// the package and the element (SACM23-VAL-012). A contained
    /// `ArgumentPackageInterface` or `ArgumentPackageBinding` neither
    /// triggers nor is reported, and `UtilityElement`s are not counted
    /// (D-PKG-HOMOG). Fix: move the other elements into a nested package.
    pub const PACKAGE_CONTENT_INVALID: Code = Code::from_static("SACM-PKG-001");

    /// SACM-REF-001: a reference names no element of the document: an id
    /// absent from it, or the `xmi:id` of a `name`, `content`, `key`, `location` or
    /// `value`, which identifies no element.
    /// [`validation::validate`](crate::validation::validate) reports one
    /// Error per distinct missing id of each reference end of each element,
    /// with the end's clause (8.2 for `citedElement` and `abstractForm`,
    /// 11.2 for an `ArgumentGroup`'s `argumentElement`, 11.9 for
    /// `referencedArtifactElement`, 11.10 for `metaClaim`, 11.12 for
    /// `structure`, 11.13 for `reasoning`, `source` and `target`, 12.3 for an
    /// `ArtifactGroup`'s `artifactElement`); an id a multi-valued end holds twice is one
    /// finding. Each affects the referring element, then the missing id
    /// (SACM23-VAL-003). [`xmi::read`](crate::xmi::read) keeps such a
    /// reference in both modes, and a tolerant [`xmi::write`](crate::xmi::write)
    /// writes it back, so nothing is lost; a strict write refuses a
    /// reference that names no `xmi:id` it writes (schema check, not
    /// `validate`; ADR 0026, ADR 0035). Fix: point the reference at an element of the
    /// document, add the element it names, or remove the reference, in the
    /// file or with [`SetReferences`](crate::edit::SetReferences) or
    /// [`RemoveReference`](crate::edit::RemoveReference) (LIB-EDIT-009),
    /// which refuse a new dangling id but keep one the end already holds.
    pub const REF_DANGLING: Code = Code::from_static("SACM-REF-001");

    /// SACM-REF-002: a reference names an element of a kind its end does not
    /// admit, such as a `metaClaim` (`Claim[0..*]`, §11.10) naming an
    /// `ArgumentReasoning`. [`validation::validate`](crate::validation::validate)
    /// reports one Error per distinct such id of each end of each element,
    /// with the end's clause, affecting the referring element, then the
    /// target (SACM23-VAL-004); and one per distinct source of an
    /// `AssertedEvidence` (§11.15), or source or target of an
    /// `AssertedArtifactSupport` (§11.17) or `AssertedArtifactContext`
    /// (§11.18), that is an `ArgumentAsset` but not an `ArtifactReference`
    /// (SACM23-VAL-016). [`xmi::read`](crate::xmi::read) keeps such a
    /// reference in both modes and both modes of
    /// [`xmi::write`](crate::xmi::write) write it (ADR 0035 D4). A create, and
    /// [`SetReferences`](crate::edit::SetReferences) or
    /// [`AddReference`](crate::edit::AddReference) for an id the end does
    /// not already hold, refuses an id its end's declared type does not
    /// admit as
    /// [`EditError::WrongReferenceType`](crate::edit::EditError::WrongReferenceType)
    /// (ADR 0028, ADR 0029). Fix: point the reference at an element of the
    /// end's type, in the file or with an edit.
    ///
    /// One code covers both rules: the declared-type check (SACM23-VAL-004),
    /// which edits also refuse, and the `ArtifactReference` rules of §11.15,
    /// §11.17 and §11.18 (SACM23-VAL-016), which edits accept.
    /// [`Diagnostic::requirement_id`] tells them apart.
    pub const REF_WRONG_TYPE: Code = Code::from_static("SACM-REF-002");

    /// SACM-REF-003: a reference names the `xmi:id` of an element the
    /// document holds only as preserved content (SACM-XMI-003), so its type
    /// cannot be checked. [`validation::validate`](crate::validation::validate)
    /// reports one Warning per distinct such id of each end of each element,
    /// with the end's clause, affecting the referring element, then the id
    /// (LIB-COMPAT-003, ADR 0033). It is not SACM-REF-001: the target is in
    /// the file, merely not read. Fix: point the reference at an element of
    /// the document, or make the target SACM 2.3.
    pub const REF_PRESERVED_TARGET: Code = Code::from_static("SACM-REF-003");

    /// SACM-CITE-001: an element has a `citedElement` but its `isCitation`
    /// is not true, against the §8.2 constraint
    /// `self.citedElement <> null implies self.isCitation = true`.
    /// [`validation::validate`](crate::validation::validate) reports one
    /// Error per such element, affecting it (SACM23-VAL-007). §8.2 states
    /// only that direction, so `isCitation` true with no `citedElement` is
    /// no finding (ADR 0026). Fix: set `isCitation="true"`, or remove the
    /// `citedElement`, in the file or with [`SetFlag`](crate::edit::SetFlag)
    /// or [`SetReferences::clear`](crate::edit::SetReferences::clear); an
    /// edit refuses neither alone (ADR 0029).
    pub const CITATION_INVALID: Code = Code::from_static("SACM-CITE-001");

    /// SACM-CITE-002: an Assertion declared `asCited` is not a citation of
    /// an Assertion. §11.8 derives an `asCited` declaration from "the cited
    /// Assertion", so [`validation::validate`](crate::validation::validate)
    /// reports one Warning when its `isCitation` is not true, and one when
    /// its `citedElement` is absent or names an element that is not of an
    /// Assertion kind (SACM23-VAL-019). A `citedElement` that names no
    /// element gives only SACM-REF-001. A strict save is not refused. Fix:
    /// make the element a citation of a Claim or an asserted relationship,
    /// or give it another declaration.
    pub const AS_CITED_INVALID: Code = Code::from_static("SACM-CITE-002");

    /// SACM-ABS-001: an `abstractForm` breaks a §8.2 rule, or an
    /// implementation constraint is on a concrete element (§8.6).
    /// [`validation::validate`](crate::validation::validate) reports one
    /// Error for each `ModelElement` with one or more
    /// `implementationConstraint` children whose `isAbstract` is not true,
    /// affecting it (SACM23-VAL-021); fix it by making the element abstract
    /// or removing the constraints. It reports, for an
    /// element with an `abstractForm` (SACM23-VAL-008): an Error when its own
    /// `isAbstract` is true (§8.2: "+isAbstract of the `SACMElement` is
    /// false"), affecting the element; and, when the target is an element of
    /// the document, a Warning when the target is not abstract, and another
    /// when it is of another kind (both "should"), each affecting the
    /// element, then the target. A target that names no element gives only
    /// SACM-REF-001. Fix: make the element concrete, or point the
    /// `abstractForm` at an abstract element of the same kind, in the file
    /// or with [`SetFlag`](crate::edit::SetFlag) and
    /// [`SetReferences`](crate::edit::SetReferences); an edit refuses
    /// neither (ADR 0029).
    ///
    /// It also reports, for each `Expression` whose `isAbstract` is not true,
    /// each distinct id of its `element` end that names an abstract `Term` or
    /// `Expression` of the document, as one Error with clause 10.10,
    /// affecting the Expression, then the element, because §10.10 gives the
    /// OCL `self.isAbstract = false implies
    /// self.element->forall(expr|expr.isAbstract = false)` (SACM23-VAL-017).
    /// An id that names no element, or an element of another kind, is only
    /// SACM-REF-001 or SACM-REF-002. Reading, editing and both saves never
    /// refuse it. Fix: make the Expression abstract, or its elements
    /// concrete, with [`SetFlag`](crate::edit::SetFlag).
    pub const ABSTRACT_FORM_INVALID: Code = Code::from_static("SACM-ABS-001");

    /// SACM-EXPR-001: an `ExpressionLangString` has an `expression` and a
    /// content that is not empty, against §8.4 "If expression is not empty,
    /// then +content should be empty."
    /// [`validate`](crate::validation::validate) reports one Warning per such
    /// name or value, with clause 8.4, affecting the element that holds it
    /// (SACM23-VAL-009); an absent content and `""` are empty. Reading,
    /// editing and saving never refuse it. Fix: remove the content, or make
    /// the text plain with
    /// [`LangString::without_expression`](crate::model::LangString::without_expression).
    pub const EXPRESSION_WITH_CONTENT: Code = Code::from_static("SACM-EXPR-001");
}

impl Code {
    /// The code with the given text.
    ///
    /// **Not part of the stable API**: public only under the `test-support`
    /// feature.
    #[cfg(feature = "test-support")]
    #[cfg_attr(docsrs, doc(cfg(feature = "test-support")))]
    #[must_use]
    pub const fn new(code: &'static str) -> Self {
        Self::from_static(code)
    }

    /// The code with the given text. The crate-internal form of `new`.
    pub(crate) const fn from_static(code: &'static str) -> Self {
        Self(code)
    }

    /// The code's text, such as `SACM-REF-001`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl fmt::Display for Code {
    /// Writes [`as_str`](Self::as_str).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(self.0)
    }
}

/// A code serializes as its text, such as `"SACM-REF-001"` (ADR 0030 R5).
#[cfg(feature = "serde")]
impl serde::Serialize for Code {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.0)
    }
}

/// A code deserializes from the text of a code of the catalog, exactly; any
/// other text, including a code the catalog reserves but no constant names
/// yet, is refused (ADR 0030 R5).
#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Code {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = <String as serde::Deserialize>::deserialize(deserializer)?;
        Code::CATALOG
            .iter()
            .copied()
            .find(|code| code.0 == text)
            .ok_or_else(|| {
                serde::de::Error::invalid_value(
                    serde::de::Unexpected::Str(&text),
                    &"a code of the diagnostics catalog, such as \"SACM-REF-001\"",
                )
            })
    }
}

/// The value of one message argument: text, or a count.
///
/// Values order Text before Count, then by value. Later releases may add
/// kinds, so match it with a fallback arm.
///
/// ```
/// use sacm::diagnostic::ArgumentValue;
///
/// let value = ArgumentValue::Text("Claim".to_owned());
/// assert_eq!((value.as_text(), value.as_count()), (Some("Claim"), None));
/// assert_eq!(ArgumentValue::Count(2).to_string(), "2");
/// ```
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum ArgumentValue {
    /// Text, unquoted: a class name, an id, a name or value as written, a row
    /// id.
    Text(String),
    /// A count or a number, such as how often a value is given, or a code
    /// point.
    Count(u64),
}

impl ArgumentValue {
    /// The text, if this is `Text`.
    #[must_use]
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            Self::Count(_) => None,
        }
    }

    /// The count, if this is `Count`.
    #[must_use]
    pub fn as_count(&self) -> Option<u64> {
        match self {
            Self::Count(count) => Some(*count),
            Self::Text(_) => None,
        }
    }
}

impl fmt::Display for ArgumentValue {
    /// Writes Text unchanged and a Count in decimal.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text(text) => f.write_str(text),
            Self::Count(count) => write!(f, "{count}"),
        }
    }
}

/// One named value that a message is made from, such as `class` = `Claim`.
///
/// The arguments of a diagnostic, or of a refusal, are in
/// [`Diagnostic::arguments`],
/// [`ReadError::arguments`](crate::xmi::ReadError::arguments) and
/// [`WriteError::arguments`](crate::xmi::WriteError::arguments), in the
/// order they first appear in its English sentence; the catalog lists each
/// template's argument names and kinds (ADR 0030 amendment 3).
///
/// ```
/// use sacm::diagnostic::ArgumentValue;
/// use sacm::xmi::{self, ReadOptions};
///
/// let error = xmi::read(b"<r></s>", &ReadOptions::default()).unwrap_err();
/// let first = &error.arguments()[0];
/// assert_eq!((first.name(), first.value()), ("end_tag", &ArgumentValue::Text("s".into())));
/// assert_eq!(first.to_string(), r#"end_tag="s""#);
/// ```
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Argument {
    name: &'static str,
    value: ArgumentValue,
}

impl Argument {
    /// The argument's name, stable within its template.
    #[must_use]
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// The argument's value.
    #[must_use]
    pub fn value(&self) -> &ArgumentValue {
        &self.value
    }

    /// The Text argument `name`.
    pub(crate) fn text(name: &'static str, value: impl Into<String>) -> Self {
        Self {
            name,
            value: ArgumentValue::Text(value.into()),
        }
    }

    /// The Count argument `name`.
    pub(crate) fn count(name: &'static str, value: u64) -> Self {
        Self {
            name,
            value: ArgumentValue::Count(value),
        }
    }

    /// The Count argument `name` of a length, saturating at `u64::MAX`.
    pub(crate) fn count_of(name: &'static str, value: usize) -> Self {
        Self::count(name, u64::try_from(value).unwrap_or(u64::MAX))
    }
}

impl fmt::Display for Argument {
    /// Writes `name="text"`, the text Debug-quoted, or `name=count`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.value {
            ArgumentValue::Text(text) => write!(f, "{}={text:?}", self.name),
            ArgumentValue::Count(count) => write!(f, "{}={count}", self.name),
        }
    }
}

/// A message as data: its template and its arguments. Only the crate builds
/// one; read it through [`template`](Self::template) and
/// [`arguments`](Self::arguments), or through the accessors of the refusal
/// that holds it.
///
/// ```
/// use sacm::xmi::{self, ReadError, ReadOptions};
///
/// let error = xmi::read(b"<r></s>", &ReadOptions::default()).unwrap_err();
/// if let ReadError::Malformed { details, .. } = &error {
///     assert_eq!(details.template(), "SACM-XML-001/end-tag-mismatch");
///     assert_eq!(details.arguments(), error.arguments());
/// }
/// ```
#[derive(Clone)]
pub struct Details {
    template: &'static Template,
    arguments: Vec<Argument>,
}

impl Details {
    /// The details of a message of `template`, made from `arguments`.
    pub(crate) fn new(template: &'static Template, arguments: Vec<Argument>) -> Self {
        Self {
            template,
            arguments,
        }
    }

    /// The template id, `<CODE>/<form>`, such as
    /// `SACM-XMI-014/unread-kind`: a stable name for the sentence form
    /// (ADR 0030 amendment 3).
    #[must_use]
    pub fn template(&self) -> &'static str {
        self.template.id
    }

    /// The arguments, in the order they first appear in the sentence.
    #[must_use]
    pub fn arguments(&self) -> &[Argument] {
        &self.arguments
    }

    /// The catalog code of the template.
    pub(crate) fn code(&self) -> Code {
        self.template.code
    }

    /// The text argument `name`, if the details have one.
    pub(crate) fn text(&self, name: &str) -> Option<&str> {
        self.arguments
            .iter()
            .find(|argument| argument.name == name)
            .and_then(|argument| argument.value.as_text())
    }

    /// The details with `argument` added last.
    #[must_use]
    pub(crate) fn with(mut self, argument: Argument) -> Self {
        self.arguments.push(argument);
        self
    }

    /// The variable part of the message, rendered from the template and the
    /// arguments.
    pub(crate) fn render(&self) -> String {
        template::render(self.template, &self.arguments)
    }
}

impl PartialEq for Details {
    fn eq(&self, other: &Self) -> bool {
        self.template.id == other.template.id && self.arguments == other.arguments
    }
}

impl Eq for Details {}

impl Hash for Details {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.template.id.hash(state);
        self.arguments.hash(state);
    }
}

impl fmt::Debug for Details {
    /// Writes `Details { template: "…", arguments: [...] }`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Details")
            .field("template", &self.template.id)
            .field("arguments", &self.arguments)
            .finish()
    }
}

/// The arguments of a message as serialized: a JSON object whose keys are
/// the argument names in the template's order, each Text a string and each
/// Count a non-negative integer (ADR 0030 amendment 3, R8).
#[cfg(feature = "serde")]
pub(crate) struct ArgumentsRecord(Vec<(String, ArgumentValue)>);

#[cfg(feature = "serde")]
impl ArgumentsRecord {
    /// The record of `arguments`.
    pub(crate) fn of(arguments: &[Argument]) -> Self {
        Self(
            arguments
                .iter()
                .map(|argument| (argument.name.to_owned(), argument.value.clone()))
                .collect(),
        )
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for ArgumentsRecord {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;

        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (name, value) in &self.0 {
            match value {
                ArgumentValue::Text(text) => map.serialize_entry(name, text)?,
                ArgumentValue::Count(count) => map.serialize_entry(name, count)?,
            }
        }
        map.end()
    }
}

/// One argument value as read: a string is Text, a non-negative integer a
/// Count, and anything else is refused.
#[cfg(feature = "serde")]
struct ArgumentValueRecord(ArgumentValue);

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for ArgumentValueRecord {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;

        impl serde::de::Visitor<'_> for Visitor {
            type Value = ArgumentValueRecord;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a string or a non-negative integer")
            }

            fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Self::Value, E> {
                Ok(ArgumentValueRecord(ArgumentValue::Text(value.to_owned())))
            }

            fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<Self::Value, E> {
                Ok(ArgumentValueRecord(ArgumentValue::Count(value)))
            }

            fn visit_i64<E: serde::de::Error>(self, value: i64) -> Result<Self::Value, E> {
                u64::try_from(value)
                    .map(|count| ArgumentValueRecord(ArgumentValue::Count(count)))
                    .map_err(|_| E::invalid_value(serde::de::Unexpected::Signed(value), &self))
            }
        }

        deserializer.deserialize_any(Visitor)
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for ArgumentsRecord {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;

        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = ArgumentsRecord;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("an object of message arguments")
            }

            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<Self::Value, A::Error> {
                let mut arguments = Vec::new();
                while let Some((name, ArgumentValueRecord(value))) = map.next_entry()? {
                    arguments.push((name, value));
                }
                Ok(ArgumentsRecord(arguments))
            }
        }

        deserializer.deserialize_map(Visitor)
    }
}

/// The details a record of a refusal reads back to: `template` must be a
/// template of `carrier`, `code` its code, and `arguments` exactly its
/// arguments, by name and kind, in its order (ADR 0030 amendment 3, R11).
/// Checked in that order: the code against the carrier's codes first.
///
/// # Errors
///
/// The reason, if any check fails.
#[cfg(feature = "serde")]
pub(crate) fn details_from_record(
    carrier: template::Carrier,
    code: Code,
    template_id: &str,
    arguments: ArgumentsRecord,
) -> Result<Details, String> {
    let mut of_carrier = template::TEMPLATES
        .iter()
        .copied()
        .filter(|template| template.carrier == carrier);
    if !of_carrier.clone().any(|template| template.code == code) {
        return Err(format!(
            "the code {code} is not one of this refusal's codes"
        ));
    }
    let Some(template) = of_carrier.find(|template| template.id == template_id) else {
        return Err(match template::find(template_id) {
            Some(found) if found.carrier == template::Carrier::Diagnostic => {
                format!("the template {template_id:?} is a diagnostic's, not a refusal's")
            }
            Some(_) => format!("the template {template_id:?} is another refusal's"),
            None => format!("the template {template_id:?} is unknown"),
        });
    };
    if template.code != code {
        return Err(format!(
            "the code {code} is not the code of the template {template_id:?}"
        ));
    }
    Ok(Details::new(
        template,
        arguments_from_record(template, arguments)?,
    ))
}

/// The arguments `arguments` of a record of `template`: exactly its
/// arguments, by name and kind, in its order (ADR 0030 amendment 3, R11).
///
/// # Errors
///
/// The reason, if they are not.
#[cfg(feature = "serde")]
fn arguments_from_record(
    template: &'static Template,
    arguments: ArgumentsRecord,
) -> Result<Vec<Argument>, String> {
    let template_id = template.id;
    let ArgumentsRecord(given) = arguments;
    if given.len() != template.arguments.len() {
        return Err(format!(
            "the template {template_id:?} takes {} arguments, not {}",
            template.arguments.len(),
            given.len()
        ));
    }
    let mut read = Vec::with_capacity(given.len());
    for ((name, value), &(declared, kind)) in given.into_iter().zip(template.arguments) {
        if name != declared {
            return Err(format!(
                "the argument {name:?} is not {declared:?}, the next argument of {template_id:?}"
            ));
        }
        let fits = matches!(
            (&value, kind),
            (ArgumentValue::Text(_), template::Kind::Text)
                | (ArgumentValue::Count(_), template::Kind::Count)
        );
        if !fits {
            return Err(format!("the argument {name:?} is not of the kind {kind:?}"));
        }
        read.push(Argument {
            name: declared,
            value,
        });
    }
    Ok(read)
}

/// The details a record of a diagnostic of `code` reads back to, from its
/// `template` and `arguments` (ADR 0030 amendment 3, R9 and R11), checked in
/// this order: both or neither are given; with neither, or with the code's
/// text as template and no arguments, there are none; otherwise the template
/// is one of the crate's, a diagnostic's, of `code`, and the arguments are
/// exactly its arguments.
///
/// # Errors
///
/// The reason, if any check fails.
#[cfg(feature = "serde")]
fn diagnostic_details_from_record(
    code: Code,
    template: Option<String>,
    arguments: Option<ArgumentsRecord>,
) -> Result<Option<Details>, String> {
    let (template_id, arguments) = match (template, arguments) {
        (None, None) => return Ok(None),
        (Some(template_id), Some(arguments)) => (template_id, arguments),
        (Some(_), None) | (None, Some(_)) => {
            return Err(
                "a diagnostic gives template and arguments together, or neither".to_owned(),
            );
        }
    };
    if template_id == code.as_str() {
        return match arguments.0.len() {
            0 => Ok(None),
            n => Err(format!(
                "the template {template_id:?} takes 0 arguments, not {n}"
            )),
        };
    }
    let Some(template) = template::find(&template_id) else {
        return Err(format!("the template {template_id:?} is unknown"));
    };
    if template.carrier != template::Carrier::Diagnostic {
        return Err(format!(
            "the template {template_id:?} is a refusal's, not a diagnostic's"
        ));
    }
    if template.code != code {
        return Err(format!(
            "the code {code} is not the code of the template {template_id:?}"
        ));
    }
    Ok(Some(Details::new(
        template,
        arguments_from_record(template, arguments)?,
    )))
}

/// Whether any of `diagnostics` has [`Severity::Error`].
///
/// A document with an Error diagnostic does not conform to SACM; Warning and
/// Info diagnostics do not count.
///
/// ```
/// use sacm::diagnostic;
/// use sacm::validation;
/// use sacm::xmi::{self, ReadOptions};
///
/// // An ArgumentPackage with no name.
/// let input = br#"<sacm:ArgumentPackage
///     xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001"
///     xmi:id="arguments"/>"#;
/// let document = xmi::read(input, &ReadOptions::default())?.into_document();
/// assert!(diagnostic::has_errors(&validation::validate(&document)));
/// assert!(!diagnostic::has_errors(&[]));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn has_errors(diagnostics: &[Diagnostic]) -> bool {
    diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == Severity::Error)
}

/// A position in a source file: a 1-based line and column.
///
/// The column counts Unicode scalar values (`char`s), not bytes. Locations
/// order by line, then column.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
pub struct SourceLocation {
    line: NonZeroU32,
    column: NonZeroU32,
}

impl SourceLocation {
    /// The location at `line` and `column`, or `None` if either is 0.
    ///
    /// **Not part of the stable API**: public only under the `test-support`
    /// feature.
    #[cfg(feature = "test-support")]
    #[cfg_attr(docsrs, doc(cfg(feature = "test-support")))]
    #[must_use]
    pub fn new(line: u32, column: u32) -> Option<Self> {
        Self::at(line, column)
    }

    /// The location at `line` and `column`, or `None` if either is 0. The
    /// crate-internal form of `new`.
    pub(crate) fn at(line: u32, column: u32) -> Option<Self> {
        Some(Self {
            line: NonZeroU32::new(line)?,
            column: NonZeroU32::new(column)?,
        })
    }

    /// The 1-based line.
    #[must_use]
    pub const fn line(self) -> u32 {
        self.line.get()
    }

    /// The 1-based column, in Unicode scalar values.
    #[must_use]
    pub const fn column(self) -> u32 {
        self.column.get()
    }
}

impl fmt::Display for SourceLocation {
    /// Writes `line L, column C`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}, column {}", self.line, self.column)
    }
}

/// A finding about the content of a document.
///
/// # Display
///
/// `Display` writes one line: the severity, the code, and the requirement id
/// in square brackets. Inside the brackets, `, clause ` and the clause follow
/// the requirement id if there is a clause. Then come ` at ` and the location
/// if there is one, and last `: ` and the message. For example:
///
/// ```text
/// warning SACM-XMI-010 [LIB-COMPAT-001, clause 9.5.2] at line 4, column 20: the Claim "c1" is typed with xsi:type, …
/// error SACM-REF-001 [SACM23-VAL-003, clause 8.2]: Claim "claim_1" has the citedElement "claim_9", which names no element of the document …
/// error SACM-CITE-001 [SACM23-VAL-007, clause 8.2]: Claim "claim_1" has the citedElement "claim_2" but isCitation is not true …
/// error SACM-MULT-001 [SACM23-VAL-018, clause 8.6]: Claim "c1" has no name (name:LangString[1])
/// ```
///
/// The affected ids are not shown. Ids in messages are quoted with `{:?}`, as
/// in `InsertError`'s messages. A message may contain line breaks; `Display`
/// escapes them so the output stays on one line: line feed and carriage
/// return as `\n` and `\r`, and U+0085, U+2028 and U+2029 as `\u{85}`,
/// `\u{2028}` and `\u{2029}`. Every other character is written unchanged.
/// [`message`](Self::message) returns the text unchanged.
///
/// # Order
///
/// Diagnostics compare by these keys, in turn:
/// 1. location: located before unlocated, then by line and column;
/// 2. code;
/// 3. affected ids, id by id; a shorter list that is a prefix of a longer one
///    sorts first;
/// 4. requirement id;
/// 5. clause, with no clause first;
/// 6. severity, `Info < Warning < Error`;
/// 7. message;
/// 8. template id;
/// 9. arguments, argument by argument (by name, then value; Text before
///    Count), a shorter list that is a prefix sorting first.
///
/// Text keys (code, requirement id, clause, message, template, and each
/// affected id) compare as `str`: byte-wise lexicographic, so clause `"10"`
/// sorts before `"2"`.
///
/// Two diagnostics compare `Equal` exactly when they are equal.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Diagnostic {
    code: Code,
    severity: Severity,
    // `Cow`, so that a diagnostic read back with serde holds its own text;
    // the crate's own diagnostics borrow static text.
    requirement_id: Cow<'static, str>,
    clause: Option<Cow<'static, str>>,
    affected: Vec<ElementId>,
    location: Option<SourceLocation>,
    message: String,
    // The template and arguments the message is made from; `None` for a
    // diagnostic built with `test-support`'s `new` and for a record that
    // gives neither. Invariant: `Some(details)` has the code `code`. With
    // `None` against `Some`, the derived `Eq` agrees with comparing
    // `(template(), arguments())`, because template ids contain `/` and code
    // texts do not.
    details: Option<Details>,
}

impl Diagnostic {
    /// A diagnostic with no clause, no affected ids and no location. Its
    /// template is its code's text, and it has no arguments.
    ///
    /// **Not part of the stable API**: public only under the `test-support`
    /// feature.
    #[cfg(feature = "test-support")]
    #[cfg_attr(docsrs, doc(cfg(feature = "test-support")))]
    #[must_use]
    pub fn new(
        code: Code,
        severity: Severity,
        requirement_id: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code,
            severity,
            requirement_id: Cow::Borrowed(requirement_id),
            clause: None,
            affected: Vec::new(),
            location: None,
            message: message.into(),
            details: None,
        }
    }

    /// Sets the clause of the specification the diagnostic enforces.
    ///
    /// **Not part of the stable API**: public only under the `test-support`
    /// feature.
    #[cfg(feature = "test-support")]
    #[cfg_attr(docsrs, doc(cfg(feature = "test-support")))]
    #[must_use]
    pub fn with_clause(self, clause: &'static str) -> Self {
        self.set_clause(clause)
    }

    /// Sets the affected element ids, in the given order.
    ///
    /// **Not part of the stable API**: public only under the `test-support`
    /// feature.
    #[cfg(feature = "test-support")]
    #[cfg_attr(docsrs, doc(cfg(feature = "test-support")))]
    #[must_use]
    pub fn with_affected(self, affected: impl IntoIterator<Item = ElementId>) -> Self {
        self.set_affected(affected)
    }

    /// Sets the source location.
    ///
    /// **Not part of the stable API**: public only under the `test-support`
    /// feature.
    #[cfg(feature = "test-support")]
    #[cfg_attr(docsrs, doc(cfg(feature = "test-support")))]
    #[must_use]
    pub fn with_location(self, location: SourceLocation) -> Self {
        self.set_location(location)
    }

    /// The diagnostic of `template`, made from `arguments`: its code is the
    /// template's, its message the template rendered with them, and it has
    /// no clause, affected ids or location.
    pub(crate) fn emit(
        template: &'static Template,
        severity: Severity,
        requirement_id: &'static str,
        arguments: Vec<Argument>,
    ) -> Self {
        let details = Details::new(template, arguments);
        Self {
            code: details.code(),
            severity,
            requirement_id: Cow::Borrowed(requirement_id),
            clause: None,
            affected: Vec::new(),
            location: None,
            message: details.render(),
            details: Some(details),
        }
    }

    /// The crate-internal form of `with_clause`.
    #[must_use]
    pub(crate) fn set_clause(mut self, clause: &'static str) -> Self {
        self.clause = Some(Cow::Borrowed(clause));
        self
    }

    /// The crate-internal form of `with_affected`.
    #[must_use]
    pub(crate) fn set_affected(mut self, affected: impl IntoIterator<Item = ElementId>) -> Self {
        self.affected = affected.into_iter().collect();
        self
    }

    /// The crate-internal form of `with_location`.
    #[must_use]
    pub(crate) fn set_location(mut self, location: SourceLocation) -> Self {
        self.location = Some(location);
        self
    }

    /// The diagnostic's code.
    #[must_use]
    pub fn code(&self) -> Code {
        self.code
    }

    /// The diagnostic's severity.
    #[must_use]
    pub fn severity(&self) -> Severity {
        self.severity
    }

    /// The id of the [functionality matrix][matrix] row the diagnostic
    /// enforces, such as `SACM23-VAL-003`.
    ///
    /// [matrix]: https://github.com/lasrod/libSACM/blob/main/docs/matrix/functionality-matrix.md
    #[must_use]
    pub fn requirement_id(&self) -> &str {
        &self.requirement_id
    }

    /// The clause the diagnostic enforces: a bare clause number, such as
    /// `"8.2"`, in the specification named by the matrix row's source. `None`
    /// if the row cites no clause.
    #[must_use]
    pub fn clause(&self) -> Option<&str> {
        self.clause.as_deref()
    }

    /// The ids of the affected elements, in the order the crate gave them.
    #[must_use]
    pub fn affected(&self) -> &[ElementId] {
        &self.affected
    }

    /// Where in the source the finding is, if known.
    #[must_use]
    pub fn location(&self) -> Option<SourceLocation> {
        self.location
    }

    /// The English message: its template rendered with its arguments; as
    /// given for a diagnostic built with `test-support` or read with serde.
    /// See [`Diagnostic`] for how `Display` writes it.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// The id of the template the message is made from, `<CODE>/<form>`,
    /// such as `SACM-MULT-001/too-many`: a stable name for its sentence form,
    /// under which an application can keep its own wording (ADR 0030
    /// amendment 3). The catalog lists every template with its arguments and
    /// English. A diagnostic built with `test-support`'s `new`, or read from
    /// JSON that gives no template, as 0.1.0 wrote, has its code's text, such
    /// as `SACM-MULT-001`, and no arguments.
    ///
    /// ```
    /// use sacm::diagnostic::ArgumentValue;
    /// use sacm::{validation, xmi::{self, ReadOptions}};
    ///
    /// let input = br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
    ///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="p"><name content="P"/>
    ///   <argumentElement xmi:type="sacm:Claim" xmi:id="c1" citedElement="c1"><name content="C"/></argumentElement>
    /// </sacm:ArgumentPackage>"#;
    /// let document = xmi::read(input, &ReadOptions::default())?.into_document();
    /// let found = validation::validate(&document);
    /// assert_eq!(found[0].template(), "SACM-CITE-001/not-a-citation");
    /// let words: Vec<String> = found[0].arguments().iter().map(ToString::to_string).collect();
    /// assert_eq!(words, [r#"class="Claim""#, r#"id="c1""#, r#"target="c1""#]);
    /// assert_eq!(found[0].arguments()[1].value(), &ArgumentValue::Text("c1".into()));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn template(&self) -> &'static str {
        self.details
            .as_ref()
            .map_or(self.code.as_str(), Details::template)
    }

    /// The values the message is made from, in the order they first appear
    /// in it, such as `class="Claim"`, `id="CLM_132"`, `count=2`. Empty when
    /// [`template`](Self::template) is the code's text. The affected ids are
    /// in [`affected`](Self::affected) too.
    #[must_use]
    pub fn arguments(&self) -> &[Argument] {
        self.details.as_ref().map_or(&[], Details::arguments)
    }
}

/// A diagnostic serializes as its nine keys, `template` and `arguments` after
/// `message`: `template` always a string, and `arguments` always an object,
/// `{}` when there are none (ADR 0030 amendment 3, R8 and R9).
#[cfg(feature = "serde")]
impl serde::Serialize for Diagnostic {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;

        let mut record = serializer.serialize_struct("Diagnostic", 9)?;
        record.serialize_field("code", &self.code)?;
        record.serialize_field("severity", &self.severity)?;
        record.serialize_field("requirement_id", &self.requirement_id)?;
        record.serialize_field("clause", &self.clause)?;
        record.serialize_field("affected", &self.affected)?;
        record.serialize_field("location", &self.location)?;
        record.serialize_field("message", &self.message)?;
        record.serialize_field("template", self.template())?;
        record.serialize_field("arguments", &ArgumentsRecord::of(self.arguments()))?;
        record.end()
    }
}

/// A diagnostic as read: every key but `code`, `severity`,
/// `requirement_id` and `message` may be left out, and an unknown one is
/// refused. `template` and `arguments` left out are `None`; `null` for
/// either is refused.
#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct DiagnosticRecord {
    code: Code,
    severity: Severity,
    requirement_id: Cow<'static, str>,
    #[serde(default)]
    clause: Option<Cow<'static, str>>,
    #[serde(default)]
    affected: Vec<ElementId>,
    #[serde(default)]
    location: Option<SourceLocation>,
    message: String,
    #[serde(default, deserialize_with = "given")]
    template: Option<String>,
    #[serde(default, deserialize_with = "given")]
    arguments: Option<ArgumentsRecord>,
}

/// A key that, when given, holds a value: `null` is refused.
#[cfg(feature = "serde")]
fn given<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

/// A diagnostic reads back from its record, which keeps its message as
/// given: never rendered again or checked against the template (ADR 0030
/// amendment 3, R9, R11 and Records).
#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Diagnostic {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let DiagnosticRecord {
            code,
            severity,
            requirement_id,
            clause,
            affected,
            location,
            message,
            template,
            arguments,
        } = DiagnosticRecord::deserialize(deserializer)?;
        let details = diagnostic_details_from_record(code, template, arguments)
            .map_err(serde::de::Error::custom)?;
        Ok(Self {
            code,
            severity,
            requirement_id,
            clause,
            affected,
            location,
            message,
            details,
        })
    }
}

impl fmt::Display for Diagnostic {
    /// Writes the diagnostic on one line, as described on [`Diagnostic`].
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {} [{}",
            self.severity, self.code, self.requirement_id
        )?;
        if let Some(clause) = &self.clause {
            write!(f, ", clause {clause}")?;
        }
        f.write_str("]")?;
        if let Some(location) = self.location {
            write!(f, " at {location}")?;
        }
        f.write_str(": ")?;
        write_escaping_line_breaks(f, &self.message)
    }
}

/// Writes `text` with its line breaks escaped, as described on [`Diagnostic`].
fn write_escaping_line_breaks(f: &mut fmt::Formatter<'_>, text: &str) -> fmt::Result {
    let mut rest = text;
    while let Some((index, c)) = rest
        .char_indices()
        .find(|&(_, c)| matches!(c, '\n' | '\r' | '\u{85}' | '\u{2028}' | '\u{2029}'))
    {
        // `index` is a char boundary and `c` starts there, so both splits are
        // in bounds.
        let (before, from_break) = rest.split_at(index);
        f.write_str(before)?;
        if matches!(c, '\n' | '\r') {
            write!(f, "{}", c.escape_default())?;
        } else {
            write!(f, "{}", c.escape_unicode())?;
        }
        rest = from_break.get(c.len_utf8()..).unwrap_or_default();
    }
    f.write_str(rest)
}

impl Ord for Diagnostic {
    /// Compares by the keys listed on [`Diagnostic`].
    fn cmp(&self, other: &Self) -> Ordering {
        // Destructured so that a new field cannot be left out of the key.
        let Self {
            code,
            severity,
            requirement_id,
            clause,
            affected,
            location,
            message,
            details: _,
        } = self;
        location
            .is_none()
            .cmp(&other.location.is_none())
            .then_with(|| location.cmp(&other.location))
            .then_with(|| code.cmp(&other.code))
            .then_with(|| affected.cmp(&other.affected))
            .then_with(|| requirement_id.cmp(&other.requirement_id))
            .then_with(|| clause.cmp(&other.clause))
            .then_with(|| severity.cmp(&other.severity))
            .then_with(|| message.cmp(&other.message))
            // The details through their accessors: a diagnostic without
            // details has its code's text as template and no arguments.
            .then_with(|| self.template().cmp(other.template()))
            .then_with(|| self.arguments().cmp(other.arguments()))
    }
}

impl PartialOrd for Diagnostic {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests {
    use super::Code;

    #[test]
    fn the_catalog_table_holds_every_constant_once_in_order() {
        let texts: Vec<&str> = Code::CATALOG.iter().map(|code| code.as_str()).collect();
        let mut distinct = texts.clone();
        distinct.sort_unstable();
        distinct.dedup();
        assert_eq!(distinct.len(), texts.len(), "a code is in the table twice");
        assert_eq!(texts.first(), Some(&Code::XML_MALFORMED.as_str()));
        assert_eq!(texts.last(), Some(&Code::EXPRESSION_WITH_CONTENT.as_str()));
        assert!(Code::CATALOG.contains(&Code::REF_DANGLING));
    }
}
