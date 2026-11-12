//! Reading and writing SACM 2.3 XMI documents.
//!
//! [`read`] takes the bytes of an XMI document and returns a [`ReadOutput`] or
//! a [`ReadError`]. [`write`](fn@write) takes a [`Document`] and returns its XMI text or
//! a [`WriteError`]. Neither does I/O: the caller reads and writes the file.
//!
//! In this release `read` reads `AssuranceCasePackage` and `ArgumentPackage`
//! roots, bare or inside an `xmi:XMI` element, and what they hold of three
//! kinds: `AssuranceCasePackage` and `ArgumentPackage` elements in an
//! `AssuranceCasePackage`, and `ArgumentPackage` and `Claim` elements in an
//! `ArgumentPackage`, each with its id and name, and a Claim's
//! `assertionDeclaration` (SACM 2.3 §2.2, §2.4, §9.2, §11.4, §11.10). It
//! refuses input that is unsafe ([`ReadError::Doctype`], SACM-SEC-001), not
//! well-formed ([`ReadError::Malformed`], SACM-XML-001) or not rooted in a
//! SACM package ([`ReadError::InvalidRoot`] and [`ReadError::NoPackage`],
//! SACM-XMI-001). Content it cannot represent yet is refused as
//! [`ReadError::Unsupported`], never dropped.
//!
//! `write` writes every element of a document, with its name and, on an
//! Assertion, its `assertionDeclaration`, in document order, in one
//! deterministic form. Its only refusals are a document with no root
//! ([`WriteError::NoRoot`]) and an id or name that XML 1.0 cannot represent
//! ([`WriteError::UnrepresentableCharacter`]). A document returned by this
//! release's `read` always writes, and what `write` writes for a document
//! built with [`edit::apply`](crate::edit::apply) reads back with the same
//! ids, kinds, names, declarations, parents and order.
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
//!     <argumentPackage xmi:id="arguments">
//!       <argumentElement xmi:type="sacm:Claim" xmi:id="c1"/>
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
/// attribute values exactly as XML 1.0 gives them, never trimmed (a literal
/// TAB, LF or CR in a value is already a space by XML 1.0 §3.3.3; write
/// `&#9;`, `&#10;` or `&#13;` to keep one). A field whose attribute is not
/// written is absent. A root without a `name` has no name.
///
/// Under each root, `read` reads these children, in document order, each
/// with its `xmi:id` and its name. A child's element name is its role, in no
/// namespace:
///
/// | Parent | Role element | Reads | Needs `xmi:type` |
/// |---|---|---|---|
/// | AssuranceCasePackage (§9.2) | `assuranceCasePackage` | an AssuranceCasePackage | no |
/// | AssuranceCasePackage (§9.2) | `argumentPackage` | an ArgumentPackage | no |
/// | ArgumentPackage (§11.4) | `argumentElement` | an ArgumentPackage or a Claim | yes: the role's type, `ArgumentationElement`, is abstract (§11.3) |
///
/// The class is the one the child's `xmi:type` or `xsi:type` names (both, if
/// present, must name the same class); without one, it is the role's declared
/// type. §11.4's text spells the ArgumentPackage role `argumentationElement`;
/// this release refuses that spelling as [`ReadError::Unsupported`].
///
/// A Claim's `assertionDeclaration` attribute (§11.10), in no namespace, is
/// read as its declaration. Its value must be one of the six
/// [`AssertionDeclaration`](crate::model::AssertionDeclaration) literals:
/// `asserted`, `needsSupport`, `assumed`, `axiomatic`, `defeated` or
/// `asCited`. Matching is exact: case-sensitive, with no white space trimmed.
/// A literal tab or line break in the value is already a space (XML 1.0
/// §3.3.3), so it does not match either. Without the attribute, or with
/// `"asserted"`, the Claim is `asserted`. Any other value is
/// [`ReadError::Unsupported`] at the attribute's name, and the reason quotes
/// the value. It is never read as `asserted`, because that would silently
/// change what the case declares. Fix: correct the spelling. This release
/// gives no catalog code; a later release reports it as SACM-ENUM-001.
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
/// declarations, `xmi:id`, `xmi:version` and an `xmi:type` or `xsi:type`
/// naming its own class, and hold at most one `name` and otherwise only white
/// space, comments, processing instructions and the children above. A child
/// may carry only namespace declarations, `xmi:id`, its type attributes and,
/// on a Claim, `assertionDeclaration`, and hold the same content as a root.
/// The `name` must be in no namespace, carry only `lang`, `content` and an
/// `xmi:type` or `xsi:type` naming `sacm:LangString`, and hold only white
/// space, comments and processing instructions. Anything else is refused as
/// [`ReadError::Unsupported`] until a later release reads it.
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
///    attributes, each root with its content, and the content beside it
///    (`Unsupported`). An element's start tag (its role, type, kind and id)
///    is checked before its other attributes, and those before its content.
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
///   version cannot represent it. It is also the refusal of an
///   `assertionDeclaration` value that is not a SACM literal, until a later
///   release reports it as SACM-ENUM-001; that is an error in the input, not
///   a gap in this version. Examples are UTF-16 without a byte order
///   mark, UTF-32 or UCS-4 input, a declared encoding other than UTF-8 or
///   ASCII in input without a UTF-16 mark, more nodes, attributes or
///   namespaces than the parser can hold, the other ten package kinds as
///   roots, a package name outside the SACM namespace, an element without an
///   `xmi:id` or with the id of an earlier element, an `XMI` element outside the
///   XMI 2.5.1 namespace (such as an older XMI revision's), an XMI-namespace
///   element as the document element (such as `xmi:Documentation`) or beside
///   the roots (such as `xmi:documentation`), an attribute of `xmi:XMI` other
///   than `xmi:version`, a second `name`, any other attribute or content of a
///   `name` (such as `xmi:id`, `xml:lang` or a `content` element), a `name` in
///   a namespace (such as `sacm:name`), and any other attribute or content of
///   a root or child. For a child, that includes a role its parent's class
///   lacks, a role element in a namespace, a type that names no concrete
///   SACM class or that the role does not admit, two types that disagree,
///   no type under a role of an abstract type, any other kind (such as a
///   `description`, a `note` or an `ArgumentReasoning`), text, attributes
///   such as `gid`; an `assertionDeclaration` whose value is not one of the
///   six literals; and an `assertionDeclaration` that is on a package (even
///   `"asserted"`), in a namespace, or written as a child element.
///
/// # Examples
///
/// ```
/// use sacm::diagnostic::Code;
/// use sacm::model::{AssertionDeclaration, ElementKind};
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
/// // A value that is not a literal (here the wrong letter case) is refused at
/// // the attribute's name, line 3, column 54, and never read as `asserted`.
/// let misspelt = br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="arguments">
///   <argumentElement xmi:type="sacm:Claim" xmi:id="c1" assertionDeclaration="Assumed"/>
/// </sacm:ArgumentPackage>"#;
/// let error = xmi::read(misspelt, &options).unwrap_err();
/// assert!(matches!(error, ReadError::Unsupported { .. }));
/// assert_eq!(error.code(), None);
/// assert_eq!(error.location().map(|at| (at.line(), at.column())), Some((3, 54)));
/// assert!(error
///     .to_string()
///     .contains("has the value \"Assumed\", which is not an AssertionDeclaration literal"));
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
/// Every refusal has a [`SourceLocation`] except the parser's capacity-limit
/// refusals, and [`code`](Self::code) gives its catalog code. Lines end as XML 1.0 §2.11
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
    ///
    /// It is also the refusal of an `assertionDeclaration` value that is not
    /// a SACM literal, until a later release reports it as SACM-ENUM-001.
    /// That is an error in the input, not a gap in this version.
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

    /// Where in the input the refusal is: an element at the `<` of its start
    /// tag, an attribute at the first character of its name, text at its
    /// first character that is not literal white space (a CDATA section or
    /// reference at its start). `None` only when the input exceeds the
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
/// Every element then carries its id as `xmi:id`. After `xmi:id`, an
/// Assertion whose `assertionDeclaration` is not `asserted` carries
/// `assertionDeclaration="<literal>"`, spelled as
/// [`AssertionDeclaration::as_str`](crate::model::AssertionDeclaration::as_str)
/// gives it. `asserted`, the default, is not written (XMI 2.5.1 §9.4.1). A
/// named element's first child is its name, `<name lang="…" content="…" />`,
/// with each field written only if present, so an empty field is `=""` and an
/// absent one is no attribute; an element with no name has no `name` child. An id is written
/// as it is, escaped, and so are a name's fields. An id that is not an XML
/// `NCName`, such as `1 2`, is written too, but the output then does not
/// validate against an XMI schema. In attribute values `&`, `<`, `>`, `"`,
/// TAB, LF and CR are escaped; everything else is written as it is.
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
///    [`WriteError::UnrepresentableCharacter`], naming the attribute and its
///    first such character; then its name's `lang`, then its name's
///    `content`, the same way.
///
/// Nothing else a document holds is refused.
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
/// // A name is written as the element's first child.
/// let named = br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="arguments">
///   <name lang="en" content="Arguments"/>
/// </sacm:ArgumentPackage>"#;
/// let document = xmi::read(named, &ReadOptions::default())?.into_document();
/// let text = xmi::write(&document, &WriteOptions::default())?;
/// assert!(text.ends_with(
///     "xmi:id=\"arguments\">\n  <name lang=\"en\" content=\"Arguments\" />\n</sacm:ArgumentPackage>\n"
/// ));
///
/// // A `needsSupport` Claim carries its declaration after its id; an
/// // `asserted` one carries none.
/// let claims = br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="arguments">
///   <argumentElement xmi:type="sacm:Claim" xmi:id="c1" assertionDeclaration="needsSupport"/>
///   <argumentElement xmi:type="sacm:Claim" xmi:id="c2" assertionDeclaration="asserted"/>
/// </sacm:ArgumentPackage>"#;
/// let document = xmi::read(claims, &ReadOptions::default())?.into_document();
/// let text = xmi::write(&document, &WriteOptions::default())?;
/// assert!(text.contains(
///     "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\" assertionDeclaration=\"needsSupport\" />\n"
/// ));
/// assert!(text.contains("<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c2\" />\n"));
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
/// The writer writes everything a [`Document`] holds, so it refuses only a
/// document with no root and a character XML 1.0 cannot represent. None of
/// the refusals has a catalog code: [`code`](Self::code) is `None`.
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
        /// The element whose id or name holds the character.
        element: ElementId,
        /// The first such character in the attribute.
        character: char,
        /// The attribute that holds it.
        attribute: WriteAttribute,
    },
}

impl WriteError {
    /// The element refused, if the refusal names one: every variant but
    /// [`NoRoot`](Self::NoRoot).
    #[must_use]
    pub fn element(&self) -> Option<&ElementId> {
        match self {
            Self::NoRoot => None,
            Self::UnrepresentableCharacter { element, .. } => Some(element),
        }
    }

    /// The catalog code of the refusal: `None` for every variant in this
    /// release.
    #[must_use]
    pub fn code(&self) -> Option<Code> {
        match self {
            Self::NoRoot | Self::UnrepresentableCharacter { .. } => None,
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
}

impl fmt::Display for WriteAttribute {
    /// Writes `xmi:id`, `name's lang` or `name's content`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Id => "xmi:id",
            Self::NameLang => "name's lang",
            Self::NameContent => "name's content",
        })
    }
}

// A refusal on one thread can be reported on another.
const _: () = {
    const fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<WriteError>();
};
