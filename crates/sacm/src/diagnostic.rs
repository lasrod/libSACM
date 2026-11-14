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
//! [`SourceLocation`] and a message.
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
//! though docs.rs shows them.
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

use std::cmp::Ordering;
use std::fmt;
use std::num::NonZeroU32;

use crate::model::ElementId;

/// How serious a diagnostic is: `Info < Warning < Error`.
///
/// ```
/// use sacm::diagnostic::Severity;
///
/// assert!(Severity::Warning < Severity::Error);
/// assert_eq!(Severity::Info.to_string(), "info");
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
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

impl Code {
    /// SACM-XML-001: malformed XML. The code of
    /// [`ReadError::Malformed`](crate::xmi::ReadError::Malformed)
    /// (LIB-SEC-002).
    pub const XML_MALFORMED: Code = Code::from_static("SACM-XML-001");

    /// SACM-SEC-001: the input has a DOCTYPE, which may declare entities, so
    /// it is refused. The code of
    /// [`ReadError::Doctype`](crate::xmi::ReadError::Doctype) (LIB-SEC-001).
    /// Named after the C++ library's `kXmlDoctypeRejected`.
    pub const XML_DOCTYPE_REJECTED: Code = Code::from_static("SACM-SEC-001");

    /// SACM-XMI-001: an interchange root (the document element, or each
    /// element child outside the XMI namespace of an `xmi:XMI` element in the
    /// XMI 2.5.1 namespace `http://www.omg.org/spec/XMI/20131001`) is not
    /// named as a SACM package kind, or the XMI element holds no root. The
    /// code of
    /// [`ReadError::InvalidRoot`](crate::xmi::ReadError::InvalidRoot) and
    /// [`ReadError::NoPackage`](crate::xmi::ReadError::NoPackage)
    /// (SACM23-XMI-001). Named after the C++ library's `kXmiInvalidRoot`.
    pub const XMI_INVALID_ROOT: Code = Code::from_static("SACM-XMI-001");

    /// SACM-XMI-004: an element has no `xmi:id`, which XMI 2.5.1 §7.6.1
    /// makes optional. [`xmi::read`](crate::xmi::read) reads the element
    /// with a generated id, `<snake_case kind>_<n>` as an edit generates one,
    /// that no `xmi:id` in the input holds, and reports one Info per element
    /// in both modes, affecting the generated id (SACM23-XMI-020, ADR 0024).
    /// The generated id is an ordinary id: [`xmi::write`](crate::xmi::write)
    /// writes it, so the saved file carries it. Nothing needs fixing.
    pub const XMI_MISSING_ID: Code = Code::from_static("SACM-XMI-004");

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
    /// `xmi:version` on any other element is refused as
    /// [`ReadError::Unsupported`](crate::xmi::ReadError::Unsupported). Nothing
    /// is lost and nothing needs fixing. To silence it, remove the attribute,
    /// or save the document again with [`xmi::write`](crate::xmi::write),
    /// which does not write it.
    pub const XMI_VERSION_IGNORED: Code = Code::from_static("SACM-XMI-011");

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
    /// and [`ReadError::RepeatedValue`](crate::xmi::ReadError::RepeatedValue).
    ///
    /// ```
    /// use sacm::diagnostic::Code;
    ///
    /// assert_eq!(Code::MULTIPLICITY_VIOLATION.as_str(), "SACM-MULT-001");
    /// ```
    pub const MULTIPLICITY_VIOLATION: Code = Code::from_static("SACM-MULT-001");

    /// SACM-ENUM-001: a value is not one of its type's literals. The code of
    /// [`ReadError::InvalidLiteral`](crate::xmi::ReadError::InvalidLiteral),
    /// refused in both modes: an `assertionDeclaration` that is not exactly
    /// one of the six `AssertionDeclaration` literals (SACM23-VAL-006), and
    /// an `isCitation`, `isAbstract` or `isCounter` (SACM23-ARG-007) that is
    /// not one of the `xsd:boolean` literals `true`, `false`, `1` and `0`,
    /// with white space around them collapsed (SACM23-BASE-007,
    /// SACM23-BASE-008). Fix: write one of
    /// `asserted`, `needsSupport`, `assumed`, `axiomatic`, `defeated` and
    /// `asCited`, or one of `true` and `false`, spelled and cased exactly.
    pub const ENUM_INVALID_LITERAL: Code = Code::from_static("SACM-ENUM-001");

    /// SACM-CMD-001: an operation's target is not in the document. The code
    /// of [`EditError::ParentNotFound`](crate::edit::EditError::ParentNotFound)
    /// (LIB-EDIT-001) and of
    /// [`EditError::ReferenceNotFound`](crate::edit::EditError::ReferenceNotFound),
    /// a source, target, `reasoning` or `structure` that names no element
    /// (SACM23-ARG-005, -008, -010). Create the parent or the referenced
    /// element first, or correct its id. Ids are compared exactly, case
    /// included.
    pub const CMD_TARGET_NOT_FOUND: Code = Code::from_static("SACM-CMD-001");

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
    /// a reference a create operation is given whose id holds XML white
    /// space, so it cannot be written as one id. It is the code of
    /// [`ReadError::InvalidId`](crate::xmi::ReadError::InvalidId) for an
    /// empty `xmi:id` on an element or a `name`.
    /// [`validation::validate`](crate::validation::validate) reports it as an
    /// Error for every other id that is not an `NCName`, a name's `xmi:id`
    /// included (SACM23-VAL-020). An id with white space around an `NCName`
    /// is first read trimmed (SACM-ID-004).
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
    /// absent from it, or the `xmi:id` of a `name`, `content`, `key` or
    /// `value`, which identifies no element.
    /// [`validation::validate`](crate::validation::validate) reports one
    /// Error per distinct missing id of each reference end of each element,
    /// with the end's clause (8.2 for `citedElement` and `abstractForm`,
    /// 11.10 for `metaClaim`, 11.12 for `structure`, 11.13 for `reasoning`,
    /// `source` and `target`); an id a multi-valued end holds twice is one
    /// finding. Each affects the referring element, then the missing id
    /// (SACM23-VAL-003). [`xmi::read`](crate::xmi::read) keeps such a
    /// reference in both modes, and a tolerant [`xmi::write`](crate::xmi::write)
    /// writes it back, so nothing is lost; a strict write refuses the
    /// document (ADR 0026). Fix: point the reference at an element of the
    /// document, add the element it names, or remove the reference, in the
    /// file; edits that set flags and references are LIB-EDIT-008/009, not
    /// built yet.
    pub const REF_DANGLING: Code = Code::from_static("SACM-REF-001");

    /// SACM-CITE-001: an element has a `citedElement` but its `isCitation`
    /// is not true, against the §8.2 constraint
    /// `self.citedElement <> null implies self.isCitation = true`.
    /// [`validation::validate`](crate::validation::validate) reports one
    /// Error per such element, affecting it (SACM23-VAL-007). §8.2 states
    /// only that direction, so `isCitation` true with no `citedElement` is
    /// no finding (ADR 0026). Fix: set `isCitation="true"`, or remove the
    /// `citedElement`, in the file; edits that set flags and references are
    /// LIB-EDIT-008/009, not built yet.
    pub const CITATION_INVALID: Code = Code::from_static("SACM-CITE-001");

    /// SACM-ABS-001: an `abstractForm` breaks a §8.2 rule.
    /// [`validation::validate`](crate::validation::validate) reports, for an
    /// element with an `abstractForm` (SACM23-VAL-008): an Error when its own
    /// `isAbstract` is true (§8.2: "+isAbstract of the `SACMElement` is
    /// false"), affecting the element; and, when the target is an element of
    /// the document, a Warning when the target is not abstract, and another
    /// when it is of another kind (both "should"), each affecting the
    /// element, then the target. A target that names no element gives only
    /// SACM-REF-001. Fix: make the element concrete, or point the
    /// `abstractForm` at an abstract element of the same kind, in the file;
    /// edits that set flags and references are LIB-EDIT-008/009, not built
    /// yet.
    pub const ABSTRACT_FORM_INVALID: Code = Code::from_static("SACM-ABS-001");

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
/// 7. message.
///
/// Text keys (code, requirement id, clause, message, and each affected id)
/// compare as `str`: byte-wise lexicographic, so clause `"10"` sorts before
/// `"2"`.
///
/// Two diagnostics compare `Equal` exactly when they are equal.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Diagnostic {
    code: Code,
    severity: Severity,
    requirement_id: &'static str,
    clause: Option<&'static str>,
    affected: Vec<ElementId>,
    location: Option<SourceLocation>,
    message: String,
}

impl Diagnostic {
    /// A diagnostic with no clause, no affected ids and no location.
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
        Self::create(code, severity, requirement_id, message)
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

    /// The crate-internal form of `new`.
    pub(crate) fn create(
        code: Code,
        severity: Severity,
        requirement_id: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code,
            severity,
            requirement_id,
            clause: None,
            affected: Vec::new(),
            location: None,
            message: message.into(),
        }
    }

    /// The crate-internal form of `with_clause`.
    #[must_use]
    pub(crate) fn set_clause(mut self, clause: &'static str) -> Self {
        self.clause = Some(clause);
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
        self.requirement_id
    }

    /// The clause the diagnostic enforces: a bare clause number, such as
    /// `"8.2"`, in the specification named by the matrix row's source. `None`
    /// if the row cites no clause.
    #[must_use]
    pub fn clause(&self) -> Option<&str> {
        self.clause
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

    /// The human-readable message, exactly as given. See [`Diagnostic`] for
    /// how `Display` writes it.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
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
        if let Some(clause) = self.clause {
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
    }
}

impl PartialOrd for Diagnostic {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
