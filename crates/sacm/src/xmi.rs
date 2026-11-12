//! Reading and writing SACM 2.3 XMI documents.
//!
//! [`read`] takes the bytes of an XMI document and returns a [`ReadOutput`] or
//! a [`ReadError`]. [`write`](fn@write) takes a [`Document`] and returns its XMI text or
//! a [`WriteError`]. Neither does I/O: the caller reads and writes the file.
//!
//! In this release `read` reads the interchange roots: `AssuranceCasePackage`
//! and `ArgumentPackage` elements, bare or inside an `xmi:XMI` element, with
//! their names and nothing else in them yet (SACM23-XMI-001, SACM23-XMI-012).
//! It refuses input that is unsafe ([`ReadError::Doctype`], SACM-SEC-001),
//! not well-formed ([`ReadError::Malformed`], SACM-XML-001) or not rooted in
//! a SACM package ([`ReadError::InvalidRoot`] and [`ReadError::NoPackage`],
//! SACM-XMI-001). Content it cannot represent yet is refused as
//! [`ReadError::Unsupported`], never dropped.
//!
//! `write` writes every element of a document, with its name, in document
//! order, in one deterministic form. It refuses a document with no root
//! ([`WriteError::NoRoot`]) and an id or name that XML 1.0 cannot represent
//! ([`WriteError::UnrepresentableCharacter`]). In this release it also
//! refuses a non-default `assertionDeclaration` ([`WriteError::Unsupported`]),
//! because it is not written yet. A document returned by this release's
//! `read` always writes. Output with elements inside a package cannot be read
//! back by this release's `read` yet.
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
//!   <sacm:AssuranceCasePackage xmi:id="case"/>
//!   <sacm:ArgumentPackage xmi:id="arguments"/>
//! </xmi:XMI>"#;
//! let document = xmi::read(input, &ReadOptions::default())?.into_document();
//! let roots: Vec<_> = document.roots().map(|root| (root.id().as_str(), root.kind())).collect();
//! assert_eq!(
//!     roots,
//!     [("case", ElementKind::AssuranceCasePackage), ("arguments", ElementKind::ArgumentPackage)]
//! );
//!
//! let text = xmi::write(&document, &WriteOptions::default())?;
//! let back = xmi::read(text.as_bytes(), &ReadOptions::default())?.into_document();
//! let ids: Vec<_> = back.roots().map(|root| root.id().as_str()).collect();
//! assert_eq!(ids, ["case", "arguments"]);
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
use crate::model::{Document, ElementId};

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
/// `xmi:id`, and whose name is its `name` child: `lang` and `content` are the
/// attribute values, untrimmed, and a field whose attribute is not written is
/// absent. A root without a `name` has no name. Elements and attributes are
/// matched by namespace URI and local name, and an `xmi:type` or `xsi:type`
/// value is resolved as a `QName`, without its leading and trailing white
/// space, against the namespaces in scope, so the choice of prefix does not
/// matter. The SACM namespace is `http://www.omg.org/spec/SACM/20220301`.
///
/// `read` refuses rather than drops: it never returns a document that is
/// missing content from its input. A root may carry only namespace
/// declarations, `xmi:id`, `xmi:version` and an `xmi:type` or `xsi:type`
/// naming its own class, and hold one `name` and otherwise only white space,
/// comments and processing instructions. The `name` must be in no namespace,
/// carry only `lang`, `content` and a type naming `LangString`, and hold only
/// white space, comments and processing instructions. Anything else is
/// refused as [`ReadError::Unsupported`] until a later release reads it.
///
/// The input is UTF-8, ASCII, or UTF-16 with a byte order mark, which reads
/// exactly as the same characters in UTF-8. A DOCTYPE is refused before
/// anything in it is read, so no entity is declared, resolved or expanded.
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
///    attributes, each root, and the content beside it (`Unsupported`).
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
/// - [`ReadError::Unsupported`] (no code): the input is well-formed but this
///   version cannot represent it. Examples are UTF-16 without a byte order
///   mark, UTF-32 or UCS-4 input, a declared encoding other than UTF-8 or
///   ASCII in input without a UTF-16 mark, more nodes, attributes or
///   namespaces than the parser can hold, the other ten package kinds as
///   roots, a package name outside the SACM namespace, a root without an
///   `xmi:id` or with the id of an earlier root, an `XMI` element outside the
///   XMI 2.5.1 namespace (such as an older XMI revision's), an XMI-namespace
///   element as the document element (such as `xmi:Documentation`) or beside
///   the roots (such as `xmi:documentation`), an attribute of `xmi:XMI` other
///   than `xmi:version`, a second `name`, any other attribute or content of a
///   `name` (such as `xml:lang` or a `content` element), and any other
///   attribute or content of a root. So in this release a package with a
///   description or any element in it is refused.
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
/// // A root's name is read.
/// let named = br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="arguments">
///   <name lang="en" content="Arguments"/>
/// </sacm:ArgumentPackage>"#;
/// let document = xmi::read(named, &options)?.into_document();
/// let name = document.get("arguments").and_then(|root| root.name());
/// assert_eq!(name.and_then(|name| name.content()), Some("Arguments"));
///
/// // Elements inside a package are not read yet: refused, never dropped.
/// let claim = br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="arguments">
///   <argumentElement/>
/// </sacm:ArgumentPackage>"#;
/// let error = xmi::read(claim, &options).unwrap_err();
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
///
/// // UTF-16 with a byte order mark (U+FEFF) reads as the same text in UTF-8.
/// let text = "\u{FEFF}<sacm:ArgumentPackage xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"
///     xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" xmi:id=\"arguments\"/>";
/// let utf16: Vec<u8> = text.encode_utf16().flat_map(u16::to_le_bytes).collect();
/// assert!(xmi::read(&utf16, &options)?.document().get("arguments").is_some());
/// # Ok::<(), ReadError>(())
/// ```
pub fn read(input: &[u8], options: &ReadOptions) -> Result<ReadOutput, ReadError> {
    let ReadOptions {} = options;
    let text = input::decode(input)?;
    let tree = input::parse(&text)?;
    let document = roots::read(&text, &tree)?;
    Ok(ReadOutput {
        document,
        diagnostics: Vec::new(),
    })
}

/// How [`read`] reads a document.
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
/// carriage return), and a leading byte order mark, UTF-8 or UTF-16, takes
/// no column. Columns count characters, in UTF-16 input too.
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
    /// leading byte order mark, UTF-8 or UTF-16, takes no column.
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
/// namespace `http://www.omg.org/spec/XMI/20131001` as `xmi`, and
/// `xmi:version="2.0"` (the value assurance-forge and EMF-based tools write;
/// XMI.xsd 2.5.1 does not declare the attribute, see [ADR 0017]).
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
/// `useSchemaExtensions`). `xsi:type` is never written, but `read` accepts
/// both.
///
/// Every element then carries its id as `xmi:id`. A named element's first
/// child is its name, `<name lang="…" content="…" />`, with each field written
/// only if present, so an empty field is `=""` and an absent one is no
/// attribute; an element with no name has no `name` child. An id is written
/// as it is, escaped, and so are a name's fields. An id that is not an XML `NCName`, such as `1 2`, is written too,
/// but the output then does not validate against an XMI schema. In attribute
/// values `&`, `<`, `>`, `"`, TAB, LF and CR are escaped; everything else is
/// written as it is.
///
/// # Errors
///
/// The document is checked before anything is written, and the first failure
/// is returned:
///
/// 1. a document with no root is [`WriteError::NoRoot`];
/// 2. then each element, in [`Document::elements`] order, with all of an
///    element's checks run before the next element's: first its id, where a
///    character outside XML 1.0's `Char` is
///    [`WriteError::UnrepresentableCharacter`], naming the first such
///    character; then its name's `lang`, then its `content`, the same way;
///    then a non-default `assertionDeclaration`, which is
///    [`WriteError::Unsupported`], because this release does not write it
///    yet.
///
/// # Examples
///
/// ```
/// use sacm::model::Document;
/// use sacm::xmi::{self, ReadOptions, WriteError, WriteOptions};
///
/// let input = br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="arguments"/>"#;
/// let document = xmi::read(input, &ReadOptions::default())?.into_document();
/// let text = xmi::write(&document, &WriteOptions::default())?;
/// assert_eq!(
///     text,
///     "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
///      <sacm:ArgumentPackage xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\" \
///      xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" xmi:version=\"2.0\" \
///      xmi:id=\"arguments\" />\n"
/// );
///
/// // A document with no root would be an empty `xmi:XMI`, which a SACM
/// // reader refuses, so it is refused.
/// let error = xmi::write(&Document::new(), &WriteOptions::default()).unwrap_err();
/// assert_eq!(error, WriteError::NoRoot);
/// assert_eq!(error.to_string(), "the document has no root to write");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn write(document: &Document, options: &WriteOptions) -> Result<String, WriteError> {
    let WriteOptions {} = options;
    output::write(document)
}

/// How [`write`](fn@write) writes a document.
///
/// Empty for now: the save mode (LIB-COMPAT-001) comes later. Use
/// [`WriteOptions::default`].
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct WriteOptions {}

/// Why [`write`](fn@write) refused a document.
///
/// None of the refusals has a catalog code: [`code`](Self::code) is `None`.
/// [`element`](Self::element) names the element refused, where there is one.
///
/// # Display
///
/// `Display` writes one line that starts in lower case, without trailing
/// punctuation. Ids are quoted with `{:?}`, so line breaks in them never break
/// the line. For example:
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
    /// An element's id, or its name's `lang` or `content`, holds a character
    /// that XML 1.0 cannot represent: one outside its `Char` production, such
    /// as U+0000, another C0 control other than TAB, LF and CR, U+FFFE or
    /// U+FFFF.
    #[non_exhaustive]
    UnrepresentableCharacter {
        /// The element whose id or name holds the character.
        element: ElementId,
        /// The first such character in the attribute.
        character: char,
        /// The XMI attribute that holds it: `"xmi:id"`, or the name's
        /// `"lang"` or `"content"`.
        attribute: &'static str,
    },
    /// The document holds content this version cannot write yet, such as a
    /// non-default `assertionDeclaration`. It has no catalog code.
    /// A document returned by this release's [`read`] never holds such
    /// content.
    #[non_exhaustive]
    Unsupported {
        /// The element that holds the content.
        element: ElementId,
        /// What cannot be written.
        reason: String,
    },
}

impl WriteError {
    /// The element refused, if the refusal names one: every variant but
    /// [`NoRoot`](Self::NoRoot).
    #[must_use]
    pub fn element(&self) -> Option<&ElementId> {
        match self {
            Self::NoRoot => None,
            Self::UnrepresentableCharacter { element, .. } | Self::Unsupported { element, .. } => {
                Some(element)
            }
        }
    }

    /// The catalog code of the refusal: `None` for every variant in this
    /// release.
    #[must_use]
    pub fn code(&self) -> Option<Code> {
        match self {
            Self::NoRoot | Self::UnrepresentableCharacter { .. } | Self::Unsupported { .. } => None,
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
            } => {
                let holder = if *attribute == "xmi:id" {
                    ""
                } else {
                    "name's "
                };
                write!(
                    f,
                    "the element {:?} cannot be written: its {holder}{attribute} holds U+{:04X}, which XML 1.0 cannot represent",
                    element.as_str(),
                    u32::from(*character)
                )
            }
            Self::Unsupported { element, reason } => write!(
                f,
                "the writer cannot write the element {:?}: {reason}",
                element.as_str()
            ),
        }
    }
}

impl std::error::Error for WriteError {}

// A refusal on one thread can be reported on another.
const _: () = {
    const fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<WriteError>();
};
