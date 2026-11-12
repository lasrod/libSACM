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
//! [`validation::validate`](crate::validation::validate) returns
//! diagnostics, and the XMI reader and editing will as their rows are built.
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

    /// SACM-MULT-001: a property has more or fewer values than its
    /// multiplicity allows. [`validation::validate`](crate::validation::validate)
    /// reports it, as an Error, for a `ModelElement` with no name (§8.6
    /// `name:LangString[1]`, SACM23-VAL-018); fix it by giving the element a
    /// name.
    ///
    /// ```
    /// use sacm::diagnostic::Code;
    ///
    /// assert_eq!(Code::MULTIPLICITY_VIOLATION.as_str(), "SACM-MULT-001");
    /// ```
    pub const MULTIPLICITY_VIOLATION: Code = Code::from_static("SACM-MULT-001");

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
/// error SACM-REF-001 [SACM23-VAL-003] at line 12, column 34: the target "claim_9" does not resolve
/// error SACM-CITE-001 [SACM23-VAL-007, clause 8.2]: "claim_1" has isCitation true but no citedElement
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
    #[cfg_attr(
        not(feature = "test-support"),
        expect(dead_code, reason = "the rows that emit diagnostics will build them")
    )]
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
