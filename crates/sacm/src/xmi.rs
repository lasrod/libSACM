//! Reading SACM 2.3 XMI documents.
//!
//! [`read`] takes the bytes of an XMI document and returns a [`ReadOutput`] or
//! a [`ReadError`]. It does no I/O: the caller reads the file (ADR 0015).
//! Writing comes later.
//!
//! In this release `read` reads no SACM content yet. It refuses input that is
//! unsafe ([`ReadError::Doctype`], SACM-SEC-001) or not well-formed
//! ([`ReadError::Malformed`], SACM-XML-001), and refuses every well-formed
//! document as [`ReadError::Unsupported`], because it would otherwise drop
//! the content.
//!
//! # Examples
//!
//! ```
//! use std::path::Path;
//!
//! use sacm::xmi::{self, ReadError, ReadOptions};
//!
//! /// Reads the file at `path` and prints why it was refused, if it was.
//! fn report(path: &Path) -> std::io::Result<()> {
//!     match xmi::read(&std::fs::read(path)?, &ReadOptions::default()) {
//!         Ok(_output) => eprintln!("{}: read", path.display()),
//!         Err(error) => eprintln!("{}: {error}", path.display()),
//!     }
//!     Ok(())
//! }
//!
//! let error = xmi::read(b"<r></s>", &ReadOptions::default()).unwrap_err();
//! assert!(matches!(error, ReadError::Malformed { .. }));
//! # let _ = report;
//! ```

use std::fmt;

use crate::diagnostic::{Code, SourceLocation};

mod input;

/// Reads an XMI document from its bytes.
///
/// `read` refuses rather than drops: it never returns a document that is
/// missing content from its input (ADR 0015). In this release every
/// well-formed document is refused as [`ReadError::Unsupported`], at its root
/// element; SACM23-XMI-001 will read the first root packages.
///
/// The input is UTF-8 or ASCII. A DOCTYPE is refused before anything in it is
/// read, so no entity is declared, resolved or expanded.
///
/// # Errors
///
/// - [`ReadError::Doctype`] (SACM-SEC-001): the input has a DOCTYPE, with or
///   without entity declarations.
/// - [`ReadError::Malformed`] (SACM-XML-001): the input is not well-formed
///   XML, is truncated, or is not valid UTF-8.
/// - [`ReadError::Unsupported`] (no code): the input is well-formed but this
///   version cannot represent it, such as UTF-16 input, a declared encoding
///   other than UTF-8 or ASCII, or, in this release, any document.
///
/// # Examples
///
/// ```
/// use sacm::diagnostic::Code;
/// use sacm::xmi::{self, ReadError, ReadOptions};
///
/// // An XML external entity: refused at the DOCTYPE, never resolved.
/// let xxe = b"<?xml version=\"1.0\"?>\n\
///     <!DOCTYPE r [ <!ENTITY xxe SYSTEM \"file:///etc/passwd\"> ]>\n\
///     <r>&xxe;</r>";
/// let error = xmi::read(xxe, &ReadOptions::default()).unwrap_err();
/// assert!(matches!(error, ReadError::Doctype { .. }));
/// assert_eq!(error.code(), Some(Code::XML_DOCTYPE_REJECTED));
/// assert_eq!(
///     error.to_string(),
///     "refused a DOCTYPE or ENTITY declaration at line 2, column 1"
/// );
///
/// // Truncated input.
/// let error = xmi::read(b"<r><c/>", &ReadOptions::default()).unwrap_err();
/// assert_eq!(error.code(), Some(Code::XML_MALFORMED));
/// ```
pub fn read(input: &[u8], options: &ReadOptions) -> Result<ReadOutput, ReadError> {
    let ReadOptions {} = options;
    let text = input::decode(input)?;
    let document = input::parse(text)?;
    Err(input::unsupported_root(text, &document))
}

/// Options for [`read`].
///
/// Empty for now: the load mode (LIB-COMPAT-001) and the resource limits
/// (LIB-SEC-003) come later. Use [`ReadOptions::default`].
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct ReadOptions {}

/// What [`read`] returns for a document it reads.
///
/// It will hold the complete document and its diagnostics (SACM23-XMI-001).
/// It is never returned in this release.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct ReadOutput {}

/// Why [`read`] refused its input.
///
/// Each refusal has a [`SourceLocation`] where one is known, and
/// [`code`](Self::code) gives its catalog code.
///
/// # Display
///
/// `Display` writes one lowercase line without trailing punctuation. The
/// location follows as ` at line L, column C` when it is known, and the reason
/// after `: `. Names from the input are quoted with `{:?}`, so line breaks in
/// them never break the line. For example:
///
/// ```text
/// refused a DOCTYPE or ENTITY declaration at line 2, column 1
/// the input is not well-formed XML at line 2, column 4: the end tag "s" does not match the start tag "r"
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
    /// The input is not well-formed XML, is truncated, or is not valid UTF-8
    /// (SACM-XML-001).
    #[non_exhaustive]
    Malformed {
        /// Where the input stops being well-formed.
        location: Option<SourceLocation>,
        /// What is wrong, without a position.
        reason: String,
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
}

impl ReadError {
    /// The catalog code of the refusal: SACM-SEC-001 for
    /// [`Doctype`](Self::Doctype), SACM-XML-001 for
    /// [`Malformed`](Self::Malformed), and `None` for
    /// [`Unsupported`](Self::Unsupported).
    #[must_use]
    pub fn code(&self) -> Option<Code> {
        match self {
            Self::Doctype { .. } => Some(Code::XML_DOCTYPE_REJECTED),
            Self::Malformed { .. } => Some(Code::XML_MALFORMED),
            Self::Unsupported { .. } => None,
        }
    }

    /// Where in the input the refusal is, if known.
    #[must_use]
    pub fn location(&self) -> Option<SourceLocation> {
        match self {
            Self::Doctype { location }
            | Self::Malformed { location, .. }
            | Self::Unsupported { location, .. } => *location,
        }
    }
}

impl fmt::Display for ReadError {
    /// Writes the refusal on one line, as described on [`ReadError`].
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (lead, reason) = match self {
            Self::Doctype { .. } => ("refused a DOCTYPE or ENTITY declaration", None),
            Self::Malformed { reason, .. } => ("the input is not well-formed XML", Some(reason)),
            Self::Unsupported { reason, .. } => {
                ("the reader cannot represent the input", Some(reason))
            }
        };
        f.write_str(lead)?;
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
