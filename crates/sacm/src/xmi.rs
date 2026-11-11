//! Reading SACM 2.3 XMI documents.
//!
//! [`read`] takes the bytes of an XMI document and returns a [`ReadOutput`] or
//! a [`ReadError`]. It does no I/O: the caller reads the file (ADR 0015).
//! Writing comes later.
//!
//! In this release `read` reads the interchange roots: `AssuranceCasePackage`
//! and `ArgumentPackage` elements, bare or inside an `xmi:XMI` element, with
//! nothing in them yet (SACM23-XMI-001). It refuses input that is unsafe
//! ([`ReadError::Doctype`], SACM-SEC-001), not well-formed
//! ([`ReadError::Malformed`], SACM-XML-001) or not rooted in a SACM package
//! ([`ReadError::InvalidRoot`] and [`ReadError::NoPackage`], SACM-XMI-001).
//! Content it cannot represent yet is refused as [`ReadError::Unsupported`],
//! never dropped.
//!
//! # Examples
//!
//! ```
//! use sacm::model::ElementKind;
//! use sacm::xmi::{self, ReadOptions};
//!
//! let input = br#"<?xml version="1.0" encoding="UTF-8"?>
//! <xmi:XMI xmlns:xmi="http://www.omg.org/spec/XMI/20131001"
//!          xmlns:sacm="http://www.omg.org/spec/SACM/20220301">
//!   <sacm:AssuranceCasePackage xmi:id="case"/>
//!   <sacm:ArgumentPackage xmi:id="arguments"/>
//! </xmi:XMI>"#;
//! let document = xmi::read(input, &ReadOptions::default())?.into_document();
//! let roots: Vec<_> = document.roots().map(|root| (root.id().as_str(), root.kind())).collect();
//! assert_eq!(
//!     roots,
//!     [("case", ElementKind::AssuranceCasePackage), ("arguments", ElementKind::ArgumentPackage)]
//! );
//! # Ok::<(), xmi::ReadError>(())
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
//! /// Reads the file at `path` and prints how many roots it has, or why it
//! /// was refused.
//! fn report(path: &Path) -> std::io::Result<()> {
//!     match xmi::read(&std::fs::read(path)?, &ReadOptions::default()) {
//!         Ok(output) => {
//!             let roots = output.document().roots().count();
//!             eprintln!("{}: {roots} roots", path.display());
//!         }
//!         Err(error) => eprintln!("{}: {error}", path.display()),
//!     }
//!     Ok(())
//! }
//! # let _ = report;
//! ```

use std::fmt;

use crate::diagnostic::{Code, Diagnostic, SourceLocation};
use crate::model::Document;

mod input;
mod roots;

/// Reads an XMI document from its bytes.
///
/// In this release `read` reads `AssuranceCasePackage` and `ArgumentPackage`
/// roots (formal/23-05-08 §2.2, §2.4): the document element, or each element
/// child outside the XMI namespace of an `xmi:XMI` element in the XMI 2.5.1
/// namespace `http://www.omg.org/spec/XMI/20131001`. Each root becomes
/// a parentless element of its kind, in document order, whose id is its
/// `xmi:id` and whose name is missing. Elements and attributes are matched by
/// namespace and local name, never by prefix. The SACM namespace is
/// `http://www.omg.org/spec/SACM/20220301`.
///
/// `read` refuses rather than drops: it never returns a document that is
/// missing content from its input (ADR 0015). A root may carry only namespace
/// declarations, `xmi:id`, `xmi:version` and an `xmi:type` or `xsi:type`
/// naming its own class, and hold only white space, comments and processing
/// instructions. Anything else is refused as [`ReadError::Unsupported`] until
/// a later release reads it.
///
/// The input is UTF-8 or ASCII. A DOCTYPE is refused before anything in it is
/// read, so no entity is declared, resolved or expanded.
///
/// # Errors
///
/// The checks run in this order, and the first failure is returned:
///
/// 1. the byte order mark and the first bytes: UTF-16, UTF-32 and UCS-4 are
///    `Unsupported`;
/// 2. the grammar of the XML declaration's `<?xml`, `version` and `encoding`
///    (`Malformed`);
/// 3. the declared encoding (`Unsupported` unless UTF-8 or ASCII), then, for
///    a supported one, the rest of the declaration: `standalone` and `?>`
///    (`Malformed`);
/// 4. under an ASCII declaration, a non-ASCII character (`Malformed`);
/// 5. UTF-8 (`Malformed` at the first invalid byte);
/// 6. the parse: a DOCTYPE (`Doctype`) or a well-formedness error
///    (`Malformed`), whichever comes first;
/// 7. the well-formedness errors the parser lets through, the earliest first
///    (`Malformed`): a character reference to a surrogate or above U+10FFFF, a
///    namespace prefix undeclared with `xmlns:p=""`, and a processing
///    instruction whose target is `xml` in any letter case;
/// 8. the roots, in two passes. First the names: an `XMI` element outside the
///    XMI 2.5.1 namespace, or any other XMI-namespace element, as the document
///    element is `Unsupported`; an `xmi:XMI` element with no root is
///    `NoPackage`; otherwise the first root not named as a package kind is
///    `InvalidRoot`. Then, in document order, the `xmi:XMI` element's
///    attributes, each root, and the content beside it (`Unsupported`).
///
/// The refusals:
///
/// - [`ReadError::Doctype`] (SACM-SEC-001): the input has a DOCTYPE, with or
///   without entity declarations.
/// - [`ReadError::Malformed`] (SACM-XML-001): the input is not well-formed
///   XML, is truncated, is not valid UTF-8, or declares ASCII but holds a
///   non-ASCII character.
/// - [`ReadError::InvalidRoot`] (SACM-XMI-001): a root's local name is not
///   one of the twelve SACM package kinds.
/// - [`ReadError::NoPackage`] (SACM-XMI-001): the `xmi:XMI` element holds no
///   root: no element child outside the XMI namespace.
/// - [`ReadError::Unsupported`] (no code): the input is well-formed but this
///   version cannot represent it. Examples are UTF-16, UTF-32 or UCS-4 input,
///   a declared encoding other than UTF-8 or ASCII, more nodes, attributes or
///   namespaces than the parser can hold, the other ten package kinds as
///   roots, a package name outside the SACM namespace, a root without an
///   `xmi:id` or with the id of an earlier root, an `XMI` element outside the
///   XMI 2.5.1 namespace (such as an older XMI revision's), an XMI-namespace
///   element such as `xmi:Documentation` as the document element or beside
///   the roots, an attribute of `xmi:XMI` other than `xmi:version`, and any
///   other attribute or content of a root. So in this release a package with
///   a name, a description or any element in it is refused.
///
/// # Examples
///
/// ```
/// use sacm::diagnostic::Code;
/// use sacm::model::ElementKind;
/// use sacm::xmi::{self, ReadError, ReadOptions};
///
/// let options = ReadOptions::default();
/// let input = br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="arguments"/>"#;
/// let output = xmi::read(input, &options)?;
/// let root = output.document().get("arguments").expect("the root was read");
/// assert_eq!(root.kind(), ElementKind::ArgumentPackage);
/// assert!(output.diagnostics().is_empty());
///
/// // Content inside a package is not read yet: refused, never dropped.
/// let named = br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="arguments">
///   <name content="Arguments"/>
/// </sacm:ArgumentPackage>"#;
/// let error = xmi::read(named, &options).unwrap_err();
/// assert!(matches!(error, ReadError::Unsupported { .. }));
/// assert_eq!(error.code(), None);
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
/// # Ok::<(), ReadError>(())
/// ```
pub fn read(input: &[u8], options: &ReadOptions) -> Result<ReadOutput, ReadError> {
    let ReadOptions {} = options;
    let text = input::decode(input)?;
    let tree = input::parse(text)?;
    let document = roots::read(text, &tree)?;
    Ok(ReadOutput {
        document,
        diagnostics: Vec::new(),
    })
}

/// Options for [`read`].
///
/// Empty for now: the load mode (LIB-COMPAT-001) and the resource limits
/// (LIB-SEC-003) come later. Use [`ReadOptions::default`].
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct ReadOptions {}

/// What [`read`] returns for a document it reads: the document, and the
/// diagnostics found while reading it.
///
/// The document holds all of the input's content: `read` refuses what it
/// cannot represent rather than drop it. The diagnostics are sorted by
/// [`Diagnostic`]'s order, located ones first. This release reports none.
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
}

impl ReadOutput {
    /// The document read.
    #[must_use]
    pub fn document(&self) -> &Document {
        &self.document
    }

    /// The diagnostics found while reading, sorted by [`Diagnostic`]'s order.
    /// Empty in this release.
    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
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
    /// [`Malformed`](Self::Malformed), SACM-XMI-001 for
    /// [`InvalidRoot`](Self::InvalidRoot) and [`NoPackage`](Self::NoPackage),
    /// and `None` for [`Unsupported`](Self::Unsupported).
    #[must_use]
    pub fn code(&self) -> Option<Code> {
        match self {
            Self::Doctype { .. } => Some(Code::XML_DOCTYPE_REJECTED),
            Self::Malformed { .. } => Some(Code::XML_MALFORMED),
            Self::InvalidRoot { .. } | Self::NoPackage { .. } => Some(Code::XMI_INVALID_ROOT),
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
            | Self::InvalidRoot { location, .. }
            | Self::NoPackage { location }
            | Self::Unsupported { location, .. } => *location,
        }
    }
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
                Some(reason)
            }
            Self::InvalidRoot { element_name, .. } => {
                write!(f, "the root element {element_name:?} is not a SACM package")?;
                None
            }
            Self::NoPackage { .. } => {
                f.write_str("the XMI element holds no SACM package")?;
                None
            }
            Self::Unsupported { reason, .. } => {
                f.write_str("the reader cannot represent the input")?;
                Some(reason)
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
