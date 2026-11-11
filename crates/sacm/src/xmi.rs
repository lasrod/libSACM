//! Reading SACM 2.3 XMI documents (in this release, every input is refused).
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
//! use sacm::xmi::{self, ReadError, ReadOptions};
//!
//! // A real SACM document is refused in this release: nothing in it is read yet.
//! let input = br#"<?xml version="1.0" encoding="UTF-8"?>
//! <sacm:AssuranceCasePackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"/>"#;
//! let error = xmi::read(input, &ReadOptions::default()).unwrap_err();
//! assert!(matches!(error, ReadError::Unsupported { .. }));
//! assert_eq!(error.code(), None);
//! ```
//!
//! Reading a file is the caller's I/O. This function shows the shape; the
//! example defines it but does not call it:
//!
//! ```
//! use std::path::Path;
//!
//! use sacm::xmi::{self, ReadOptions};
//!
//! /// Reads the file at `path` and prints why it was refused, if it was.
//! fn report(path: &Path) -> std::io::Result<()> {
//!     match xmi::read(&std::fs::read(path)?, &ReadOptions::default()) {
//!         Ok(_output) => eprintln!("{}: read", path.display()),
//!         Err(error) => eprintln!("{}: {error}", path.display()),
//!     }
//!     Ok(())
//! }
//! # let _ = report;
//! ```

use std::fmt;

use crate::diagnostic::{Code, SourceLocation};

mod input;

/// Reads an XMI document from its bytes; in this release it refuses every
/// input.
///
/// `read` refuses rather than drops: it never returns a document that is
/// missing content from its input (ADR 0015). In this release every
/// well-formed document is refused as [`ReadError::Unsupported`], at its root
/// element. A later release (row SACM23-XMI-001) will read the first root
/// packages.
///
/// The input is UTF-8 or ASCII. A DOCTYPE is refused before anything in it is
/// read, so no entity is declared, resolved or expanded.
///
/// # Errors
///
/// The checks run in this order, and the first failure is returned: the
/// encoding, then UTF-8, then the XML (a DOCTYPE or a well-formedness error,
/// whichever comes first), then the content.
///
/// - [`ReadError::Doctype`] (SACM-SEC-001): the input has a DOCTYPE, with or
///   without entity declarations.
/// - [`ReadError::Malformed`] (SACM-XML-001): the input is not well-formed
///   XML, is truncated, is not valid UTF-8, or declares ASCII but holds a
///   non-ASCII character.
/// - [`ReadError::Unsupported`] (no code): the input is well-formed but this
///   version cannot represent it, such as UTF-16, UTF-32 or UCS-4 input, a
///   declared encoding other than UTF-8 or ASCII, more nodes, attributes or
///   namespaces than the parser can hold, or, in this release, any
///   well-formed document.
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
///     "the input has a DOCTYPE, which is refused at line 2, column 1"
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
/// [`code`](Self::code) gives its catalog code. Lines end as XML 1.0 §2.11
/// ends them (a line feed, a carriage return and line feed, or a lone
/// carriage return), and a leading UTF-8 byte order mark takes no column.
///
/// # Display
///
/// `Display` writes one line that starts in lower case, without trailing
/// punctuation. The location follows as ` at line L, column C` when it is
/// known, and the reason after `: `. Names from the input are quoted with
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
    /// The input is not well-formed XML, is truncated, is not valid UTF-8, or
    /// declares ASCII but holds a non-ASCII character (SACM-XML-001).
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

    /// Where in the input the refusal is, if known. Lines end at a line
    /// feed, a carriage return and line feed, or a lone carriage return; a
    /// leading UTF-8 byte order mark takes no column.
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
            Self::Doctype { .. } => ("the input has a DOCTYPE, which is refused", None),
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
