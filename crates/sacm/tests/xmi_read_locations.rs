//! Tests that every refusal of `sacm::xmi::read` is located at what it
//! refuses (SACM23-XMI-010; XML 1.0 (Fifth Edition) §2.11; ADR 0011,
//! ADR 0015).
//!
//! One table holds one input per refusal site: every `ReadError` variant, and
//! every `Unsupported` reason for roots, for children, for utility elements
//! and their `content` and `key` roles, for `gid` (ADR 0025), and for the
//! `SACMElement` flags and references (ADR 0026), and for the
//! asserted-relationship features, `metaClaim` and `structure` (ADR 0027), and
//! for the `ArtifactReference` and `ArgumentGroup` ends (ADR 0028); the
//! strict refusals of the C++ name and text shapes (SACM-XMI-012,
//! LIB-COMPAT-008) and the untyped child under an abstract role
//! (SACM-XMI-005, LIB-COMPAT-010). Every row is read in strict mode: a
//! tolerant read keeps what a strict read refuses, except a DOCTYPE,
//! malformed XML, no SACM 2.3 root, a duplicate id and the limits
//! (ADR 0035). Content that does not conform is no refusal site since
//! ADR 0033: the sites where it used to be refused are in [`preserved_rows`],
//! where one SACM-XMI-003 must be at the same construct (LIB-COMPAT-003).
//! The expected
//! location is marked in the input with [`MARK`] (U+2038), which the test
//! removes before reading, and is counted here: lines end at LF, CRLF or a
//! lone CR, columns count characters, and a leading byte order mark takes no
//! column.
//!
//! Each construct is placed after [`LEAD`]: a declaration ended by CRLF, a
//! comment ended by a lone CR, and on line 3 a comment holding `é` (two bytes
//! in UTF-8), so the construct is on line 3 at column 13 or later, with a
//! non-ASCII character before it on its own line. [`check_lead`] enforces
//! this for every row. The rows that cannot have it say so with [`Lead`]:
//! a byte order mark or the first bytes of the input are located at line 1,
//! column 1; and nothing non-ASCII can come before a construct in the XML
//! declaration or before the first non-ASCII character of an ASCII
//! document, so those rows have only the CRLF and the CR before them.
//!
//! Not checked here: the refusals for the parser's capacity limits (more
//! nodes, attributes or namespaces than it can hold), which have no
//! location. Inputs that reach those limits are large, so that `None` is
//! checked by the unit tests of `xmi::input` (`refusal_locates_and_words_parser_errors`).

use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::Duration;

use sacm::diagnostic::{Code, Severity, SourceLocation};
use sacm::model::{ElementId, SacmElement};
use sacm::xmi::{self, Mode, ReadError, ReadOptions, ReadOutput};

const ROW: &str = "SACM23-XMI-010";

/// The row of the preserved-content sites ([`preserved_rows`]).
const COMPAT_003: &str = "LIB-COMPAT-003";

/// Marks, in a test input, where the refusal is expected.
const MARK: char = '\u{2038}';

/// What comes before most constructs: line 1 ends in CRLF, line 2 in a lone
/// CR, and line 3 starts with 12 characters, one of them `é`.
const LEAD: &str = concat!(
    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\r\n",
    "<!-- line 2 -->\r",
    "<!-- \u{e9} -->  ",
);

/// [`LEAD`] for UTF-16 input.
const LEAD_16: &str = concat!(
    "<?xml version=\"1.0\" encoding=\"UTF-16\"?>\r\n",
    "<!-- line 2 -->\r",
    "<!-- \u{e9} -->  ",
);

/// An XML declaration that starts with its own CRLF and CR, so that what
/// follows `version` is on line 3, from column 5.
const SPREAD_DECLARATION: &str = "<?xml\r\n\r    version=\"1.0\"";

/// The namespace declarations of a bare root: `sacm`, `xmi` and `xsi`.
const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
    " xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"",
);

/// The start tag of an XMI wrapper that declares `xmi` and `sacm`.
const WRAPPER: &str = concat!(
    "<xmi:XMI",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
);

// --- Rows ------------------------------------------------------------------

/// How a row's text is turned into bytes.
#[derive(Clone, Copy, Debug)]
enum Form {
    /// UTF-8.
    Utf8,
    /// UTF-8, with the byte 0xFF at the mark.
    InvalidUtf8,
    /// UTF-16LE with a byte order mark.
    Utf16,
    /// UTF-16LE with a byte order mark, with the unpaired surrogate D800 at
    /// the mark.
    Utf16Unpaired,
    /// UTF-16LE with a byte order mark, and one more byte at the end, where
    /// the mark must be.
    Utf16Odd,
    /// The given bytes, refused at line 1, column 1.
    Raw(&'static [u8]),
}

/// What must come before a row's construct.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Lead {
    /// Line 3 or later, column 5 or later, after a CRLF and a lone CR, with
    /// a non-ASCII character before the construct on its line.
    Full,
    /// The same, without the non-ASCII character.
    NoNonAscii,
    /// Line 1, column 1: a byte order mark or the input's first bytes.
    Start,
}

/// The variant a row must be refused with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Variant {
    Doctype,
    Malformed,
    InvalidRoot,
    NoPackage,
    Unsupported,
    DuplicateId,
    InvalidId,
    RepeatedValue,
    InvalidLiteral,
    /// SACM-XMI-012, a C++ shape in strict mode (LIB-COMPAT-008).
    LegacyShape,
    /// SACM-XMI-005, an untyped child under an abstract role
    /// (LIB-COMPAT-010).
    MissingType,
    /// Not refused: read with one SACM-XMI-003 there, the content kept as
    /// preserved content (LIB-COMPAT-003, ADR 0033). Only in
    /// [`preserved_rows`].
    Preserved,
}

/// One refusal site.
struct Row {
    what: &'static str,
    text: String,
    form: Form,
    lead: Lead,
    variant: Variant,
    mode: Mode,
}

fn row(what: &'static str, variant: Variant, text: impl Into<String>) -> Row {
    Row {
        what,
        text: text.into(),
        form: Form::Utf8,
        lead: Lead::Full,
        variant,
        // A tolerant read keeps what a strict read refuses, except a
        // DOCTYPE, malformed XML, no SACM 2.3 root, a duplicate id and the
        // limits (LIB-COMPAT-001, ADR 0035), so every site is read strictly.
        mode: Mode::Strict,
    }
}

impl Row {
    fn form(self, form: Form) -> Self {
        Self { form, ..self }
    }

    fn lead(self, lead: Lead) -> Self {
        Self { lead, ..self }
    }

    /// The row read in strict mode.
    fn strict(self) -> Self {
        Self {
            mode: Mode::Strict,
            ..self
        }
    }
}

/// `body` after [`LEAD`].
fn led(body: &str) -> String {
    format!("{LEAD}{body}")
}

/// `content` in an ArgumentPackage root `p`, after [`LEAD`].
fn in_ap(content: &str) -> String {
    led(&format!(
        "<sacm:ArgumentPackage{NS} xmi:id=\"p\">{content}</sacm:ArgumentPackage>"
    ))
}

/// `content` in an AssuranceCasePackage root `case`, after [`LEAD`].
fn in_acp(content: &str) -> String {
    led(&format!(
        "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\">{content}</sacm:AssuranceCasePackage>"
    ))
}

/// `content` in a Claim `c`, in `argumentElement` of an ArgumentPackage `ap`,
/// in `argumentPackage` of the root `case`: at depth 3.
fn in_claim(content: &str) -> String {
    in_acp(&format!(
        "<argumentPackage xmi:id=\"ap\"><argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c\">\
         {content}</argumentElement></argumentPackage>"
    ))
}

/// `content` in an XMI wrapper, after [`LEAD`].
fn wrapped(content: &str) -> String {
    led(&format!("{WRAPPER}>{content}</xmi:XMI>"))
}

/// The encoding, declaration and decoding refusals.
fn encoding_rows() -> Vec<Row> {
    use Variant::{Malformed, Unsupported};
    let spread = |rest: &str| format!("{SPREAD_DECLARATION}{rest}");
    vec![
        row("a UTF-32 byte order mark", Unsupported, "")
            .form(Form::Raw(b"\xFF\xFE\x00\x00<\x00\x00\x00r\x00\x00\x00/\x00\x00\x00>\x00\x00\x00"))
            .lead(Lead::Start),
        row("UTF-16 without a byte order mark", Unsupported, "")
            .form(Form::Raw(b"<\x00?\x00x\x00m\x00l\x00 \x00"))
            .lead(Lead::Start),
        row("an unsupported declared encoding", Unsupported, spread(&format!(" encoding=\"{MARK}ISO-8859-1\"?><r/>")))
            .lead(Lead::NoNonAscii),
        row("a version that is not 1.x", Malformed, format!("<?xml\r\n\r    version=\"{MARK}2.0\"?><r/>"))
            .lead(Lead::NoNonAscii),
        row("an invalid encoding name", Malformed, spread(&format!(" encoding=\"{MARK}8bit\"?><r/>")))
            .lead(Lead::NoNonAscii),
        row("an invalid standalone value", Malformed, spread(&format!(" standalone=\"{MARK}maybe\"?><r/>")))
            .lead(Lead::NoNonAscii),
        row("an unknown pseudo-attribute", Malformed, spread(&format!(" {MARK}foo=\"x\"?><r/>")))
            .lead(Lead::NoNonAscii),
        row("a pseudo-attribute without its \"=\"", Malformed, format!("<?xml\r\n\r    version {MARK}\"1.0\"?><r/>"))
            .lead(Lead::NoNonAscii),
        row("an input that ends inside the declaration", Malformed, spread(&MARK.to_string()))
            .lead(Lead::NoNonAscii),
        row("a declared encoding that contradicts the UTF-16 mark", Malformed, spread(&format!(" encoding=\"{MARK}UTF-8\"?><r/>")))
            .form(Form::Utf16)
            .lead(Lead::NoNonAscii),
        row("a non-ASCII character under an ASCII declaration", Malformed,
            format!("<?xml version=\"1.0\" encoding=\"US-ASCII\"?>\r\n<!-- -->\r<!-- {MARK}\u{e9} --><r/>"))
            .lead(Lead::NoNonAscii),
        row("invalid UTF-8", Malformed, in_ap(&format!("<!-- {MARK} -->"))).form(Form::InvalidUtf8),
        row("an unpaired UTF-16 surrogate", Malformed, format!("{LEAD_16}<r>{MARK}</r>")).form(Form::Utf16Unpaired),
        row("an odd number of UTF-16 bytes", Malformed, format!("{LEAD_16}<r/>{MARK}")).form(Form::Utf16Odd),
    ]
}

/// The refusals of the XML parser and of the well-formedness checks after it.
fn well_formedness_rows() -> Vec<Row> {
    use Variant::{Doctype, Malformed};
    vec![
        row(
            "a DOCTYPE",
            Doctype,
            led(&format!("{MARK}<!DOCTYPE r>\r\n<r/>")),
        ),
        row(
            "a mismatched end tag",
            Malformed,
            led(&format!("<r>{MARK}</s>")),
        ),
        row(
            "an unquoted attribute value",
            Malformed,
            led(&format!("<r a={MARK}1/>")),
        ),
        // At the name, not the `<`: the name is where the input stops being
        // well-formed, as for an empty prefix below.
        row(
            "an undeclared prefix",
            Malformed,
            led(&format!("<r><{MARK}p:c/></r>")),
        ),
        row(
            "a truncated document",
            Malformed,
            led(&format!("<r>{MARK}")),
        ),
        row(
            "a character reference to a surrogate in text",
            Malformed,
            led(&format!("<r>{MARK}&#xD800;</r>")),
        ),
        row(
            "a character reference above U+10FFFF in a value",
            Malformed,
            led(&format!("<r a=\"{MARK}&#x110000;\"/>")),
        ),
        row(
            "a declared xmlns prefix",
            Malformed,
            led(&format!("<r {MARK}xmlns:xmlns=\"urn:x\"/>")),
        ),
        row(
            "an undeclared namespace prefix",
            Malformed,
            led(&format!("<r xmlns:p=\"urn:p\"><c {MARK}xmlns:p=\"\"/></r>")),
        ),
        row(
            "an element name with an empty prefix",
            Malformed,
            led(&format!("<r><{MARK}:c/></r>")),
        ),
        row(
            "an end tag name with an empty prefix",
            Malformed,
            led(&format!("<r></{MARK}:r>")),
        ),
        row(
            "an attribute name with an empty prefix",
            Malformed,
            led(&format!("<r {MARK}:a=\"1\"/>")),
        ),
        row(
            "a processing instruction targeting XML",
            Malformed,
            led(&format!("<r>{MARK}<?XML x?></r>")),
        ),
        row(
            "a processing instruction target with a colon",
            Malformed,
            led(&format!("<r>{MARK}<?a:b x?></r>")),
        ),
        row(
            "a processing instruction target without white space",
            Malformed,
            led(&format!("<r><?pi{MARK}#x?></r>")),
        ),
    ]
}

/// The refusals of what is not a root, and of the XMI wrapper.
fn wrapper_rows() -> Vec<Row> {
    use Variant::{InvalidRoot, NoPackage, Unsupported};
    vec![
        row(
            "a bare root that is no package",
            InvalidRoot,
            led(&format!("{MARK}<r/>")),
        ),
        row(
            "a wrapped root that is no package",
            InvalidRoot,
            wrapped(&format!("{MARK}<sacm:Claim xmi:id=\"c\"/>")),
        ),
        row(
            "a wrapper with no root",
            NoPackage,
            led(&format!("{MARK}{WRAPPER}/>")),
        ),
        row(
            "an XMI-namespace document element",
            Unsupported,
            led(&format!(
                "{MARK}<xmi:Documentation xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"/>"
            )),
        ),
        row(
            "an XMI element outside the XMI namespace",
            Unsupported,
            led(&format!("{MARK}<XMI/>")),
        ),
        row(
            "an unread attribute of the wrapper",
            Unsupported,
            led(&format!(
                "{WRAPPER} {MARK}foo=\"1\"><sacm:ArgumentPackage xmi:id=\"a\"/></xmi:XMI>"
            )),
        ),
        row(
            "text in the wrapper",
            Unsupported,
            wrapped(&format!("  {MARK}x<sacm:ArgumentPackage xmi:id=\"a\"/>")),
        ),
        row(
            "an XMI element beside the roots",
            Unsupported,
            wrapped(&format!(
                "<sacm:ArgumentPackage xmi:id=\"a\"/>{MARK}<xmi:Documentation/>"
            )),
        ),
    ]
}

/// The refusals of a root's kind, namespace, id, attributes and content.
fn root_rows() -> Vec<Row> {
    use Variant::{DuplicateId, InvalidId, RepeatedValue, Unsupported};
    let root = |attributes: &str, content: &str| {
        led(&format!(
            "{MARK}<sacm:ArgumentPackage{NS}{attributes}>{content}</sacm:ArgumentPackage>"
        ))
    };
    vec![
        row(
            "a package root this release does not read",
            Unsupported,
            led(&format!(
                "{MARK}<sacm:TerminologyPackage{NS} xmi:id=\"a\"/>"
            )),
        ),
        row(
            "a package root in a foreign namespace",
            Unsupported,
            led(&format!(
                "{MARK}<o:ArgumentPackage xmlns:o=\"urn:o\" xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" xmi:id=\"a\"/>"
            )),
        ),
        row(
            "a package root in no namespace",
            Unsupported,
            led(&format!(
                "{MARK}<ArgumentPackage xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" xmi:id=\"a\"/>"
            )),
        ),
        // XMI metadata this version does not read yet (SACM23-XMI-019). An
        // unqualified id, or another attribute the class lacks, is preserved
        // instead (ADR 0033): `preserved_rows`.
        row(
            "an xmi:uuid on a root",
            Unsupported,
            led(&format!(
                "<sacm:ArgumentPackage{NS} xmi:id=\"a\" {MARK}xmi:uuid=\"u\"/>"
            )),
        ),
        row(
            "a root with an empty id",
            InvalidId,
            root(" xmi:id=\"\"", ""),
        ),
        // SACM23-VAL-001: at the first repeat.
        row(
            "a root repeating an earlier root's id",
            DuplicateId,
            wrapped(&format!(
                "<sacm:ArgumentPackage xmi:id=\"a\"/>{MARK}<sacm:ArgumentPackage xmi:id=\"a\"/>"
            )),
        ),
        // SACM23-XMI-025: an id with white space around an NCName is read
        // trimmed, so it is no refusal.
        row(
            "an xmi:label on a root",
            Unsupported,
            led(&format!(
                "<sacm:ArgumentPackage{NS} xmi:id=\"a\" {MARK}xmi:label=\"l\"/>"
            )),
        ),
        // SACM23-XMI-021: at the second name.
        row(
            "a second name of a root",
            RepeatedValue,
            in_ap(&format!("<name/>{MARK}<name/>")),
        ),
        // SACM23-VAL-001: an id repeated across classes, at the first repeat.
        row(
            "a root with an earlier root's id",
            DuplicateId,
            wrapped(&format!(
                "<sacm:ArgumentPackage xmi:id=\"a\"/>{MARK}<sacm:AssuranceCasePackage xmi:id=\"a\"/>"
            )),
        ),
    ]
}

/// The refusals of a root's name.
fn name_rows() -> Vec<Row> {
    use Variant::{DuplicateId, InvalidId, LegacyShape, Unsupported};
    vec![
        // SACM23-XMI-022: a name's id is an id, at the name's start tag.
        row(
            "an empty name id",
            InvalidId,
            in_ap(&format!("{MARK}<name xmi:id=\"\"/>")),
        ),
        row(
            "a name id repeating its owner's id",
            DuplicateId,
            in_ap(&format!("{MARK}<name xmi:id=\"p\"/>")),
        ),
        // An ExpressionLangString is not read yet (SACM23-BASE-009).
        row(
            "a name typed as an ExpressionLangString",
            Unsupported,
            in_ap(&format!(
                "{MARK}<name xmi:type=\"sacm:ExpressionLangString\" content=\"N\"/>"
            )),
        ),
        row(
            "an xmi:uuid on a name",
            Unsupported,
            in_ap(&format!("<name content=\"N\" {MARK}xmi:uuid=\"n\"/>")),
        ),
        // `lang` and `content` are value elements (SACM23-XMI-014); one
        // `value`, and text, are C++ shapes, refused in strict mode
        // (LIB-COMPAT-008).
        row(
            "a value in a name, strict",
            LegacyShape,
            in_ap(&format!("<name>{MARK}<value/></name>")),
        )
        .strict(),
        row(
            "text in a name, strict",
            LegacyShape,
            in_ap(&format!("<name> {MARK}x</name>")),
        )
        .strict(),
    ]
}

/// The refusals of children's roles and types (SACM23-XMI-016).
fn child_role_rows() -> Vec<Row> {
    use Variant::Unsupported as U;
    let element = |attributes: &str| in_ap(&format!("{MARK}<argumentElement{attributes}/>"));
    vec![
        row(
            "argumentationElement",
            U,
            in_ap(&format!(
                "{MARK}<argumentationElement xmi:type=\"sacm:Claim\" xmi:id=\"x\"/>"
            )),
        ),
        row(
            "an untyped argumentElement",
            Variant::MissingType,
            element(" xmi:id=\"x\""),
        ),
        row(
            "a conforming kind outside the readable set",
            U,
            element(" xmi:type=\"sacm:ArgumentPackageInterface\" xmi:id=\"x\""),
        ),
    ]
}

/// The refusals of children's kinds, ids, attributes and content.
fn child_content_rows() -> Vec<Row> {
    use Variant::Unsupported as U;
    let element = |attributes: &str| in_ap(&format!("{MARK}<argumentElement{attributes}/>"));
    vec![
        // XMI metadata in a Claim, at depth 3 (SACM23-XMI-019). A utility
        // element in a Claim is read (SACM23-BASE-004).
        row(
            "an xmi:Extension in a Claim, at depth 3",
            U,
            in_claim(&format!("{MARK}<xmi:Extension/>")),
        ),
        row(
            "a child with an empty id",
            Variant::InvalidId,
            element(" xmi:type=\"sacm:Claim\" xmi:id=\"\""),
        ),
        row(
            "a child with a duplicate id",
            Variant::DuplicateId,
            in_ap(&format!(
                "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"x\"/>{MARK}<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"x\"/>"
            )),
        ),
        // SACM23-VAL-006: a value that is not one of the six literals.
        row(
            "a bad assertionDeclaration literal on a Claim",
            Variant::InvalidLiteral,
            in_ap(&format!(
                "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"x\" {MARK}assertionDeclaration=\"Asserted\"/>"
            )),
        ),
        row(
            "a prefixed unread attribute of a child",
            U,
            in_acp(&format!(
                "<argumentPackage xmi:id=\"x\" {MARK}xmi:uuid=\"u\"/>"
            )),
        ),
        row(
            "a second name of a child",
            Variant::RepeatedValue,
            in_claim(&format!("<name/>{MARK}<name/>")),
        ),
        row(
            "an unread attribute of a child's name",
            U,
            in_claim(&format!("<name {MARK}xmi:uuid=\"n\"/>")),
        ),
    ]
}

/// The refusals of value elements (SACM23-XMI-014).
fn value_element_rows() -> Vec<Row> {
    use Variant::{InvalidLiteral, RepeatedValue};
    vec![
        // SACM23-VAL-006: at the value element's `<`.
        row(
            "a bad assertionDeclaration literal in the element form",
            InvalidLiteral,
            in_claim(&format!(
                "{MARK}<assertionDeclaration>Asserted</assertionDeclaration>"
            )),
        ),
        // At the second occurrence: here the element after the attribute.
        row(
            "a value as an attribute and an element",
            RepeatedValue,
            in_claim(&format!(
                "<name content=\"N\">{MARK}<content>N</content></name>"
            )),
        ),
        row(
            "a value as two elements",
            RepeatedValue,
            in_claim(&format!(
                "<assertionDeclaration>assumed</assertionDeclaration>\
                 {MARK}<assertionDeclaration>assumed</assertionDeclaration>"
            )),
        ),
    ]
}

/// `body` in a Description `d` in the root `p`.
fn in_description(body: &str) -> String {
    in_ap(&format!("<description xmi:id=\"d\">{body}</description>"))
}

/// `body` in the `content` of a Description `d` in the root `p`.
fn in_content(body: &str) -> String {
    in_description(&format!("<content>{body}</content>"))
}

/// `body` in a `taggedValue` `t` in the root `p`.
fn in_tagged(body: &str) -> String {
    in_ap(&format!("<taggedValue xmi:id=\"t\">{body}</taggedValue>"))
}

/// The `Unsupported` refusals of utility elements and their `content` and
/// `key` roles (SACM23-BASE-004, SACM23-BASE-005; ADR 0025), and the strict
/// refusals of their C++ shorthands (LIB-COMPAT-008).
fn utility_rows() -> Vec<Row> {
    use Variant::{LegacyShape as L, Unsupported as U};
    vec![
        // The C++ shorthands (LIB-COMPAT-008): strict refuses each at its
        // first attribute or its text.
        row(
            "the lang and content shorthand on content",
            L,
            in_description(&format!("<content {MARK}lang=\"en\" content=\"x\"/>")),
        )
        .strict(),
        row(
            "the content shorthand on content",
            L,
            in_description(&format!("<content {MARK}content=\"x\"/>")),
        )
        .strict(),
        row(
            "text in content",
            L,
            in_description(&format!("<content>{MARK}x</content>")),
        )
        .strict(),
        row(
            "the lang and content shorthand on key",
            L,
            in_tagged(&format!("<key {MARK}lang=\"en\" content=\"k\"/>")),
        )
        .strict(),
        row("text in key", L, in_tagged(&format!("<key>{MARK}k</key>"))).strict(),
        row(
            "content= on a UtilityElement",
            L,
            in_ap(&format!("<description xmi:id=\"d\" {MARK}content=\"x\"/>")),
        )
        .strict(),
        row(
            "text in a UtilityElement",
            L,
            in_ap(&format!("<description xmi:id=\"d\">{MARK}x</description>")),
        )
        .strict(),
        row(
            "key= on taggedValue",
            L,
            in_ap(&format!("<taggedValue xmi:id=\"t\" {MARK}key=\"k\"/>")),
        )
        .strict(),
        row(
            "value= on taggedValue",
            L,
            in_ap(&format!("<taggedValue xmi:id=\"t\" {MARK}value=\"v\"/>")),
        )
        .strict(),
        // XMI metadata this version does not read yet (SACM23-XMI-019).
        row(
            "an xmi:Extension in a Description",
            U,
            in_description(&format!("<content/>{MARK}<xmi:Extension/>")),
        ),
        // Text in a value is a C++ shape (LIB-COMPAT-008): refused in strict
        // mode at the text.
        row(
            "text in a value, strict",
            L,
            in_content(&format!("<value lang=\"en\">{MARK}x</value>")),
        )
        .strict(),
    ]
}

/// The repeated `content` and `key` roles, and the refused ids of roles and
/// values (SACM23-BASE-004, SACM23-BASE-005; ADR 0025).
fn utility_repeat_and_id_rows() -> Vec<Row> {
    use Variant::{DuplicateId, InvalidId, RepeatedValue};
    vec![
        // SACM23-XMI-021: at the second occurrence.
        row(
            "a second content of a UtilityElement",
            RepeatedValue,
            in_description(&format!("<content/>{MARK}<content/>")),
        ),
        row(
            "a second key",
            RepeatedValue,
            in_tagged(&format!("<key/>{MARK}<key/>")),
        ),
        row(
            "a second content of a TaggedValue, after its key",
            RepeatedValue,
            in_tagged(&format!("<content/><key/>{MARK}<content/>")),
        ),
        // ADR 0025: role and value ids share the file's id space, so a repeat
        // is located at the second start tag.
        row(
            "a content id repeating its owner's id",
            DuplicateId,
            in_ap(&format!(
                "<description xmi:id=\"d\">{MARK}<content xmi:id=\"d\"/></description>"
            )),
        ),
        row(
            "a key id repeating its owner's id",
            DuplicateId,
            in_ap(&format!(
                "<taggedValue xmi:id=\"t\">{MARK}<key xmi:id=\"t\"/></taggedValue>"
            )),
        ),
        row(
            "a value id repeating the content id",
            DuplicateId,
            in_ap(&format!(
                "<description xmi:id=\"d\"><content xmi:id=\"w\">{MARK}<value xmi:id=\"w\"/>\
                 </content></description>"
            )),
        ),
        row(
            "a key value id repeating its TaggedValue's id",
            DuplicateId,
            in_ap(&format!(
                "<taggedValue xmi:id=\"t\"><key>{MARK}<value xmi:id=\"t\"/></key></taggedValue>"
            )),
        ),
        row(
            "an empty content id",
            InvalidId,
            in_description(&format!("{MARK}<content xmi:id=\"\"/>")),
        ),
        row(
            "an empty key id",
            InvalidId,
            in_tagged(&format!("{MARK}<key xmi:id=\"\"/>")),
        ),
        row(
            "an empty value id",
            InvalidId,
            in_content(&format!("{MARK}<value xmi:id=\"\" content=\"x\"/>")),
        ),
    ]
}

/// The refusals of `gid` in the wrong place or form, and given twice
/// (SACM23-BASE-006; ADR 0025).
fn gid_rows() -> Vec<Row> {
    use Variant::RepeatedValue;
    vec![
        // SACM23-XMI-021: at the second occurrence.
        row(
            "a gid as an attribute and an element on a root",
            RepeatedValue,
            led(&format!(
                "<sacm:ArgumentPackage{NS} xmi:id=\"p\" gid=\"g\">{MARK}<gid>g</gid></sacm:ArgumentPackage>"
            )),
        ),
        row(
            "a gid as an attribute and an element on a child",
            RepeatedValue,
            in_ap(&format!(
                "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"x\" gid=\"g\">\
                 {MARK}<gid>g</gid></argumentElement>"
            )),
        ),
        row(
            "two gid elements on a child",
            RepeatedValue,
            in_claim(&format!("<gid>g</gid>{MARK}<gid>g</gid>")),
        ),
        row(
            "two gid elements on a UtilityElement",
            RepeatedValue,
            in_ap(&format!("<note xmi:id=\"n\"><gid/>{MARK}<gid/></note>")),
        ),
    ]
}

/// A Claim `x` in the root `p` with the start-tag `attributes`, holding
/// `content`.
fn claim_x(attributes: &str, content: &str) -> String {
    in_ap(&format!(
        "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"x\"{attributes}>{content}</argumentElement>"
    ))
}

/// The refusals of the `SACMElement` flags `isCitation` and `isAbstract`
/// (SACM23-BASE-007/008; ADR 0026): a value that is no Boolean literal, a
/// repeat, and a flag in the wrong place or form.
fn flag_rows() -> Vec<Row> {
    use Variant::{InvalidLiteral, RepeatedValue};
    vec![
        // At the attribute's name, or the value element's `<`.
        row(
            "a bad isCitation literal on a child",
            InvalidLiteral,
            claim_x(&format!(" {MARK}isCitation=\"yes\""), ""),
        ),
        row(
            "a bad isAbstract literal on a root",
            InvalidLiteral,
            led(&format!(
                "<sacm:ArgumentPackage{NS} xmi:id=\"a\" {MARK}isAbstract=\"True\"/>"
            )),
        ),
        row(
            "a bad isAbstract literal in the element form",
            InvalidLiteral,
            in_claim(&format!("{MARK}<isAbstract>t rue</isAbstract>")),
        ),
        row(
            "an empty isCitation element",
            InvalidLiteral,
            in_ap(&format!("<note xmi:id=\"n\">{MARK}<isCitation/></note>")),
        ),
        // At the second occurrence.
        row(
            "isCitation as an attribute and an element",
            RepeatedValue,
            claim_x(
                " isCitation=\"true\"",
                &format!("{MARK}<isCitation>true</isCitation>"),
            ),
        ),
        row(
            "two isAbstract elements",
            RepeatedValue,
            in_claim(&format!(
                "<isAbstract>1</isAbstract>{MARK}<isAbstract>1</isAbstract>"
            )),
        ),
    ]
}

/// The refusals of the references `citedElement` and `abstractForm`, in the
/// attribute form (SACM23-XMI-005) and the element form (SACM23-XMI-015;
/// ADR 0026).
fn reference_rows() -> Vec<Row> {
    use Variant::{RepeatedValue, Unsupported as U};
    let link = |element: &str| claim_x("", element);
    vec![
        // Two ids on a single-valued end: at the second id, its own column
        // in the value.
        row(
            "two tokens in citedElement",
            RepeatedValue,
            claim_x(&format!(" citedElement=\"c {MARK}x\""), ""),
        ),
        // At the second occurrence: the element after the attribute.
        row(
            "citedElement as an attribute and an element",
            RepeatedValue,
            claim_x(
                " citedElement=\"c\"",
                &format!("{MARK}<citedElement xmi:idref=\"c\"/>"),
            ),
        ),
        row(
            "two abstractForm elements",
            RepeatedValue,
            in_claim(&format!(
                "<abstractForm xmi:idref=\"c\"/>{MARK}<abstractForm href=\"#c\"/>"
            )),
        ),
        // Malformed reference elements.
        row(
            "an xmi:id on a reference element",
            U,
            link(&format!(
                "<citedElement {MARK}xmi:id=\"r\" xmi:idref=\"c\"/>"
            )),
        ),
        row(
            "an xmi:type on a reference element",
            U,
            link(&format!(
                "<abstractForm xmi:idref=\"c\" {MARK}xmi:type=\"sacm:Claim\"/>"
            )),
        ),
        // An href that is not `#id` (SACM23-XMI-027): at the attribute's name.
        row(
            "a cross-document href",
            U,
            link(&format!("<citedElement {MARK}href=\"other.xmi#c\"/>")),
        ),
        row(
            "an href without a fragment",
            U,
            link(&format!("<abstractForm {MARK}href=\"c\"/>")),
        ),
        row(
            "an XPointer href",
            U,
            link(&format!(
                "<citedElement {MARK}href=\"#xpointer(id('c'))\"/>"
            )),
        ),
        row(
            "an empty fragment href",
            U,
            link(&format!("<citedElement {MARK}href=\"#\"/>")),
        ),
        row(
            "an empty href",
            U,
            link(&format!("<abstractForm {MARK}href=\"\"/>")),
        ),
    ]
}

/// An `AssertedInference` `x` in the root `p` with the start-tag `attributes`,
/// holding `content`.
fn inference_x(attributes: &str, content: &str) -> String {
    in_ap(&format!(
        "<argumentElement xmi:type=\"sacm:AssertedInference\" xmi:id=\"x\"{attributes}>{content}         </argumentElement>"
    ))
}

/// The refusals of the asserted-relationship features `isCounter`, `source`,
/// `target` and `reasoning`, of `metaClaim` and of `structure`
/// (SACM23-ARG-004, -005, -007; ADR 0027): a feature on a kind that lacks
/// it, a bad literal, and a second value of a single-valued end, or an
/// attribute and an element for a multi-valued one.
fn relationship_rows() -> Vec<Row> {
    use Variant::{InvalidLiteral, RepeatedValue};
    vec![
        // A bad literal: at the attribute's name.
        row(
            "a bad isCounter literal",
            InvalidLiteral,
            inference_x(&format!(" {MARK}isCounter=\"yes\""), ""),
        ),
        row(
            "a bad isCounter literal in the element form",
            InvalidLiteral,
            inference_x("", &format!("{MARK}<isCounter>t rue</isCounter>")),
        ),
        // Two ids on a single-valued end: at the second id, its own column
        // in the value.
        row(
            "two tokens in target",
            RepeatedValue,
            inference_x(&format!(" source=\"c\" target=\"c {MARK}d\""), ""),
        ),
        // At the second occurrence.
        row(
            "two target elements",
            RepeatedValue,
            inference_x(
                "",
                &format!("<target xmi:idref=\"c\"/>{MARK}<target href=\"#d\"/>"),
            ),
        ),
        row(
            "two reasoning elements",
            RepeatedValue,
            inference_x(
                "",
                &format!("<reasoning xmi:idref=\"r\"/>{MARK}<reasoning xmi:idref=\"r\"/>"),
            ),
        ),
        row(
            "source as an attribute and an element",
            RepeatedValue,
            inference_x(" source=\"c\"", &format!("{MARK}<source xmi:idref=\"d\"/>")),
        ),
        row(
            "metaClaim as an attribute and an element",
            RepeatedValue,
            claim_x(
                " metaClaim=\"c\"",
                &format!("{MARK}<metaClaim xmi:idref=\"d\"/>"),
            ),
        ),
        row(
            "isCounter as an attribute and an element",
            RepeatedValue,
            inference_x(
                " isCounter=\"true\"",
                &format!("{MARK}<isCounter>true</isCounter>"),
            ),
        ),
    ]
}

/// An element `x` of `kind` in the root `p` with the start-tag `attributes`,
/// holding `content`.
fn typed_x(kind: &str, attributes: &str, content: &str) -> String {
    in_ap(&format!(
        "<argumentElement xmi:type=\"sacm:{kind}\" xmi:id=\"x\"{attributes}>{content}</argumentElement>"
    ))
}

/// The refusals of the `ArtifactReference` and `ArgumentGroup` ends
/// (SACM23-ARG-006, -013; ADR 0028): C++'s `referencedArtifact` alias, the
/// text's spelling `argumentationElement`, a nested element under a group's
/// `argumentElement`, an `argumentElement` attribute on a package, an empty
/// end, an end on a kind that lacks it, and an attribute with an element.
fn artifact_rows() -> Vec<Row> {
    use Variant::{RepeatedValue, Unsupported as U};
    let reference =
        |attributes: &str, content: &str| typed_x("ArtifactReference", attributes, content);
    let group = |attributes: &str, content: &str| typed_x("ArgumentGroup", attributes, content);
    vec![
        row(
            "referencedArtifactElement as an attribute and an element",
            RepeatedValue,
            reference(
                " referencedArtifactElement=\"c\"",
                &format!("{MARK}<referencedArtifactElement xmi:idref=\"d\"/>"),
            ),
        ),
        // A group's argumentElement is a reference end: a nested element is
        // refused as a reference element, at the attribute no reference
        // element has.
        row(
            "a nested element under a group's argumentElement",
            U,
            group(
                "",
                &format!("<argumentElement {MARK}xmi:type=\"sacm:Claim\" xmi:id=\"c9\"/>"),
            ),
        ),
        row(
            "argumentationElement on a group",
            U,
            group(&format!(" {MARK}argumentationElement=\"c\""), ""),
        ),
        row(
            "an argumentationElement element in a group",
            U,
            group(
                "",
                &format!("{MARK}<argumentationElement xmi:idref=\"c\"/>"),
            ),
        ),
        row(
            "a group's argumentElement as an attribute and an element",
            RepeatedValue,
            group(
                " argumentElement=\"c\"",
                &format!("{MARK}<argumentElement xmi:idref=\"d\"/>"),
            ),
        ),
    ]
}

// --- Reading and locating ---------------------------------------------------

/// How long one `read` may take; a hang fails the test instead of stalling.
const DEADLINE: Duration = Duration::from_secs(5);

/// `xmi::read` in `mode`, on a worker thread.
fn read(input: &[u8], mode: Mode) -> Result<ReadOutput, ReadError> {
    let owned = input.to_vec();
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let _ = sender.send(xmi::read(&owned, &ReadOptions::default().with_mode(mode)));
    });
    match receiver.recv_timeout(DEADLINE) {
        Ok(result) => result,
        Err(RecvTimeoutError::Timeout) => panic!(
            "{ROW}: read did not return within {DEADLINE:?} for {:?}",
            String::from_utf8_lossy(input)
        ),
        Err(RecvTimeoutError::Disconnected) => {
            panic!(
                "{ROW}: read panicked on {:?}",
                String::from_utf8_lossy(input)
            )
        }
    }
}

/// A 1-based location.
fn loc(line: u32, column: u32) -> SourceLocation {
    let Some(location) = SourceLocation::new(line, column) else {
        panic!("{ROW}: SourceLocation::new({line}, {column}) refused a 1-based position");
    };
    location
}

/// The location after `before`: lines end at LF, CRLF or a lone CR, and
/// columns count characters.
fn location_after(before: &str) -> SourceLocation {
    let (mut line, mut column, mut after_cr) = (1_u32, 1_u32, false);
    for c in before.chars() {
        match c {
            '\n' if after_cr => {}
            '\n' | '\r' => {
                line += 1;
                column = 1;
            }
            _ => column += 1,
        }
        after_cr = c == '\r';
    }
    loc(line, column)
}

/// UTF-16LE with a byte order mark.
fn utf16(text: &str) -> Vec<u8> {
    "\u{FEFF}"
        .encode_utf16()
        .chain(text.encode_utf16())
        .flat_map(u16::to_le_bytes)
        .collect()
}

/// The bytes of `row`, and where its refusal must be.
fn input_of(row: &Row) -> (Vec<u8>, SourceLocation) {
    if let Form::Raw(bytes) = row.form {
        return (bytes.to_vec(), loc(1, 1));
    }
    let Some((before, after)) = row.text.split_once(MARK) else {
        panic!("{ROW}: setup: the row {:?} has no mark", row.what);
    };
    assert!(
        !after.contains(MARK),
        "{ROW}: setup: the row {:?} has two marks",
        row.what
    );
    let text = format!("{before}{after}");
    let bytes = match row.form {
        Form::Utf8 => text.into_bytes(),
        Form::InvalidUtf8 => [before.as_bytes(), b"\xFF", after.as_bytes()].concat(),
        Form::Utf16 => utf16(&text),
        Form::Utf16Unpaired => {
            [utf16(before), vec![0x00, 0xD8], utf16(after)[2..].to_vec()].concat()
        }
        Form::Utf16Odd => {
            assert!(
                after.is_empty(),
                "{ROW}: setup: {:?} is not marked at its end",
                row.what
            );
            [utf16(before), vec![0x20]].concat()
        }
        Form::Raw(_) => unreachable!("handled above"),
    };
    (bytes, location_after(before))
}

/// Checks that `row`'s construct comes after what its [`Lead`] requires.
fn check_lead(row: &Row, location: SourceLocation) {
    if row.lead == Lead::Start {
        return;
    }
    let before = row.text.split_once(MARK).map_or("", |(before, _)| before);
    let line = before.rsplit(['\r', '\n']).next().unwrap_or_default();
    let lone_cr = before
        .char_indices()
        .any(|(index, c)| c == '\r' && !before[index + 1..].starts_with('\n'));
    let ok = location.line() >= 3
        && location.column() >= 5
        && before.contains("\r\n")
        && lone_cr
        && (row.lead == Lead::NoNonAscii || !line.is_ascii());
    assert!(
        ok,
        "{ROW}: setup: the row {:?} at {location} does not follow its lead {:?}",
        row.what, row.lead
    );
}

/// The variant of `error`.
fn variant(error: &ReadError) -> Option<Variant> {
    match error {
        ReadError::Doctype { .. } => Some(Variant::Doctype),
        ReadError::Malformed { .. } => Some(Variant::Malformed),
        ReadError::InvalidRoot { .. } => Some(Variant::InvalidRoot),
        ReadError::NoPackage { .. } => Some(Variant::NoPackage),
        ReadError::Unsupported { .. } => Some(Variant::Unsupported),
        ReadError::DuplicateId { .. } => Some(Variant::DuplicateId),
        ReadError::InvalidId { .. } => Some(Variant::InvalidId),
        ReadError::RepeatedValue { .. } => Some(Variant::RepeatedValue),
        ReadError::InvalidLiteral { .. } => Some(Variant::InvalidLiteral),
        _ if error.code() == Some(Code::XMI_LEGACY_SHAPE) => Some(Variant::LegacyShape),
        _ if error.code() == Some(Code::XMI_MISSING_TYPE) => Some(Variant::MissingType),
        _ => None,
    }
}

/// Reads every row and fails, listing each row that is not refused with its
/// variant at its location.
fn assert_located(group: &str, rows: &[Row]) {
    let mut failures = Vec::new();
    for row in rows {
        let (input, expected) = input_of(row);
        check_lead(row, expected);
        match read(&input, row.mode) {
            Ok(_) => failures.push(format!("{}: a document was returned", row.what)),
            Err(error)
                if variant(&error) == Some(row.variant) && error.location() == Some(expected) => {}
            Err(error) => failures.push(format!(
                "{}: expected {:?} at {expected} in {:?} mode, got {error:?}",
                row.what, row.variant, row.mode
            )),
        }
    }
    assert!(
        failures.is_empty(),
        "{ROW}: {} of {} {group} rows are not located as expected:\n{}",
        failures.len(),
        rows.len(),
        failures.join("\n")
    );
}

/// Text after white space written as a reference or a CDATA section: at the
/// start of that construct. In an element, such text is preserved instead
/// (ADR 0033), at the same place: [`preserved_rows`].
fn marked_text_rows() -> Vec<Row> {
    use Variant::Unsupported as U;
    vec![
        row(
            "\"&#x20;x\" in a name, strict",
            Variant::LegacyShape,
            in_ap(&format!("<name>{MARK}&#x20;x</name>")),
        )
        .strict(),
        row(
            "\"&#x20;x\" in the wrapper",
            U,
            wrapped(&format!(
                "{MARK}&#x20;x<sacm:ArgumentPackage xmi:id=\"a\"/>"
            )),
        ),
        row(
            "\"<![CDATA[ x]]>\" in the wrapper",
            U,
            wrapped(&format!(
                "{MARK}<![CDATA[ x]]><sacm:ArgumentPackage xmi:id=\"a\"/>"
            )),
        ),
        row(
            "\"<![CDATA[ x]]>\" in a name, strict",
            Variant::LegacyShape,
            in_ap(&format!("<name>{MARK}<![CDATA[ x]]></name>")),
        )
        .strict(),
    ]
}

/// How the holder of a [`preserved_rows`] construct is known.
#[derive(Clone, Copy, Debug)]
enum Holder {
    /// The element whose `xmi:id` comes last before the mark.
    LastIdBeforeMark,
    /// An element with a generated id: the construct is an id that is not an
    /// `xmi:id` (SACM23-XMI-020).
    Generated,
}

/// The sites of content that does not conform and that a refusal used to be
/// located at: since ADR 0033 each is kept as preserved content, with one
/// SACM-XMI-003 at the same construct (LIB-COMPAT-003, by SACM23-XMI-023's
/// rules: an attribute at its name, an element at its `<`, text at its
/// first character that is not literal white space). A defect inside a
/// `name`, `content`, `key`, value element or reference element keeps that
/// whole child, so it is marked at the child's start tag. The rows read in
/// tolerant mode; the test reads each in strict mode too.
#[expect(
    clippy::too_many_lines,
    reason = "one flat table is easier to review than several"
)]
fn preserved_rows() -> Vec<(Row, Holder)> {
    use Holder::{Generated, LastIdBeforeMark as Last};
    use Variant::Preserved as P;
    let element = |attributes: &str| in_ap(&format!("{MARK}<argumentElement{attributes}/>"));
    let link = |element: &str| claim_x("", element);
    let reference =
        |attributes: &str, content: &str| typed_x("ArtifactReference", attributes, content);
    vec![
        // Roots.
        (
            row(
                "an unqualified id on a root",
                P,
                led(&format!("<sacm:ArgumentPackage{NS} {MARK}id=\"a\"/>")),
            ),
            Generated,
        ),
        (
            row(
                "a root typed as another class",
                P,
                led(&format!(
                    "<sacm:ArgumentPackage{NS} xmi:id=\"a\" {MARK}xmi:type=\"sacm:Claim\"/>"
                )),
            ),
            Last,
        ),
        (
            row(
                "an unread attribute of a root",
                P,
                led(&format!(
                    "<sacm:ArgumentPackage{NS} xmi:id=\"a\" {MARK}foo=\"1\"/>"
                )),
            ),
            Last,
        ),
        (
            row("text in a root", P, in_ap(&format!(" \t {MARK}x"))),
            Last,
        ),
        (
            row(
                "target on an ArgumentPackage root",
                P,
                led(&format!(
                    "<sacm:ArgumentPackage{NS} xmi:id=\"a\" {MARK}target=\"a\"/>"
                )),
            ),
            Last,
        ),
        (
            row(
                "an argumentElement attribute on an ArgumentPackage root",
                P,
                led(&format!(
                    "<sacm:ArgumentPackage{NS} xmi:id=\"a\" {MARK}argumentElement=\"a\"/>"
                )),
            ),
            Last,
        ),
        // Names: the whole name, at its start tag.
        (
            row(
                "a name in a namespace",
                P,
                in_ap(&format!("{MARK}<sacm:name/>")),
            ),
            Last,
        ),
        (
            row(
                "an unread attribute of a name",
                P,
                in_ap(&format!("{MARK}<name content=\"N\" xml:lang=\"en\"/>")),
            ),
            Last,
        ),
        (
            row(
                "an element in a name",
                P,
                in_ap(&format!("{MARK}<name><foo/></name>")),
            ),
            Last,
        ),
        (
            row(
                "a name typed as another class",
                P,
                in_ap(&format!("{MARK}<name xmi:type=\"sacm:Claim\"/>")),
            ),
            Last,
        ),
        // Children's roles and types: the whole child.
        (
            row(
                "a role the parent's class lacks",
                P,
                in_ap(&format!("{MARK}<foo/>")),
            ),
            Last,
        ),
        (
            row(
                "argumentElement in an AssuranceCasePackage",
                P,
                in_acp(&format!(
                    "{MARK}<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"x\"/>"
                )),
            ),
            Last,
        ),
        (
            row(
                "a namespace-qualified role element",
                P,
                in_ap(&format!(
                    "{MARK}<sacm:argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"x\"/>"
                )),
            ),
            Last,
        ),
        (
            row(
                "a role element under a default SACM namespace",
                P,
                led(&format!(
                    "<ArgumentPackage xmlns=\"http://www.omg.org/spec/SACM/20220301\" xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" \
                     xmi:id=\"p\">{MARK}<argumentElement xmi:type=\"Claim\" xmi:id=\"x\"/></ArgumentPackage>"
                )),
            ),
            Last,
        ),
        (
            row(
                "a type that does not conform to a concrete role",
                P,
                in_acp(&format!(
                    "{MARK}<argumentPackage xmi:type=\"sacm:Claim\" xmi:id=\"x\"/>"
                )),
            ),
            Last,
        ),
        (
            row(
                "a type that does not conform to argumentElement",
                P,
                element(" xmi:type=\"sacm:Term\" xmi:id=\"x\""),
            ),
            Last,
        ),
        (
            row(
                "a type naming an abstract class",
                P,
                element(" xsi:type=\"sacm:Assertion\" xmi:id=\"x\""),
            ),
            Last,
        ),
        (
            row(
                "a type naming no SACM class",
                P,
                element(" xmi:type=\"sacm:Goal\" xmi:id=\"x\""),
            ),
            Last,
        ),
        (
            row(
                "a type in a foreign namespace",
                P,
                element(" xmlns:o=\"urn:o\" xmi:type=\"o:Claim\" xmi:id=\"x\""),
            ),
            Last,
        ),
        (
            row(
                "an unresolvable type",
                P,
                element(" xmi:type=\"nope:Claim\" xmi:id=\"x\""),
            ),
            Last,
        ),
        (
            row(
                "two type attributes that disagree",
                P,
                element(" xmi:type=\"sacm:Claim\" xsi:type=\"sacm:ArgumentPackage\" xmi:id=\"x\""),
            ),
            Last,
        ),
        // Children's content and attributes.
        (
            row(
                "a role a Claim lacks, at depth 3",
                P,
                in_claim(&format!(
                    "{MARK}<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"n\"/>"
                )),
            ),
            Last,
        ),
        (
            row(
                "an unqualified id on a child",
                P,
                in_ap(&format!(
                    "<argumentElement xmi:type=\"sacm:Claim\" {MARK}id=\"x\"/>"
                )),
            ),
            Generated,
        ),
        (
            row(
                "an unread attribute of a child",
                P,
                in_ap(&format!(
                    "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"x\" {MARK}foo=\"1\"/>"
                )),
            ),
            Last,
        ),
        (
            row(
                "assertionDeclaration on a child that is not an Assertion",
                P,
                in_acp(&format!(
                    "<argumentPackage xmi:id=\"x\" {MARK}assertionDeclaration=\"asserted\"/>"
                )),
            ),
            Last,
        ),
        (
            row("text in a child", P, in_claim(&format!("  {MARK}x"))),
            Last,
        ),
        // Value elements: the whole name, or the element itself.
        (
            row(
                "an attribute of a value element",
                P,
                in_claim(&format!(
                    "{MARK}<name><content xmi:id=\"v\">N</content></name>"
                )),
            ),
            Last,
        ),
        (
            row(
                "an element in a value element",
                P,
                in_claim(&format!("{MARK}<name><lang>en<b/></lang></name>")),
            ),
            Last,
        ),
        (
            row(
                "a value element in a namespace",
                P,
                in_claim(&format!(
                    "{MARK}<name><sacm:content>N</sacm:content></name>"
                )),
            ),
            Last,
        ),
        (
            row(
                "an assertionDeclaration element on a package",
                P,
                in_ap(&format!(
                    "{MARK}<assertionDeclaration>asserted</assertionDeclaration>"
                )),
            ),
            Last,
        ),
        // Text written as a reference or CDATA: at the start of that
        // construct.
        (
            row("\"&#x20;x\" in a root", P, in_ap(&format!("{MARK}&#x20;x"))),
            Last,
        ),
        (
            row(
                "\"&#x20;x\" after literal white space in a Claim",
                P,
                in_claim(&format!(" \t {MARK}&#x20;x")),
            ),
            Last,
        ),
        (
            row(
                "\"<![CDATA[ x]]>\" in a root",
                P,
                in_ap(&format!("{MARK}<![CDATA[ x]]>")),
            ),
            Last,
        ),
        (
            row(
                "\"<![CDATA[ x]]>\" after literal white space in a Claim",
                P,
                in_claim(&format!("  {MARK}<![CDATA[ x]]>")),
            ),
            Last,
        ),
        (
            row(
                "\"<![CDATA[ ]]>x\" in a Claim",
                P,
                in_claim(&format!("{MARK}<![CDATA[ ]]>x")),
            ),
            Last,
        ),
        // Utility elements and their roles: the whole `content` or `key`.
        (
            row(
                "a name on a UtilityElement",
                P,
                in_description(&format!("{MARK}<name content=\"x\"/>")),
            ),
            Last,
        ),
        (
            row(
                "a child content cannot hold",
                P,
                in_description(&format!(
                    "{MARK}<content><value content=\"a\"/><foo/></content>"
                )),
            ),
            Last,
        ),
        (
            row(
                "content in a namespace",
                P,
                in_description(&format!("{MARK}<sacm:content/>")),
            ),
            Last,
        ),
        (
            row(
                "value in a namespace",
                P,
                in_description(&format!(
                    "{MARK}<content><sacm:value content=\"x\"/></content>"
                )),
            ),
            Last,
        ),
        (
            row(
                "an unread attribute of content",
                P,
                in_description(&format!("{MARK}<content foo=\"1\"/>")),
            ),
            Last,
        ),
        (
            row(
                "an unread attribute of a value",
                P,
                in_description(&format!(
                    "{MARK}<content><value content=\"x\" xml:lang=\"en\"/></content>"
                )),
            ),
            Last,
        ),
        // gid in the wrong place or form.
        (
            row(
                "sacm:gid as an attribute",
                P,
                in_ap(&format!(
                    "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"x\" {MARK}sacm:gid=\"g\"/>"
                )),
            ),
            Last,
        ),
        (
            row(
                "sacm:gid as an element",
                P,
                in_claim(&format!("{MARK}<sacm:gid>g</sacm:gid>")),
            ),
            Last,
        ),
        (
            row(
                "gid on a name",
                P,
                in_claim(&format!("{MARK}<name content=\"N\" gid=\"g\"/>")),
            ),
            Last,
        ),
        (
            row(
                "a gid element in a name",
                P,
                in_claim(&format!("{MARK}<name content=\"N\"><gid>g</gid></name>")),
            ),
            Last,
        ),
        (
            row(
                "gid on content",
                P,
                in_description(&format!("{MARK}<content gid=\"g\"/>")),
            ),
            Last,
        ),
        (
            row(
                "a gid element in content",
                P,
                in_description(&format!("{MARK}<content><gid>g</gid></content>")),
            ),
            Last,
        ),
        (
            row(
                "gid on key",
                P,
                in_tagged(&format!("{MARK}<key gid=\"g\"/>")),
            ),
            Last,
        ),
        (
            row(
                "gid on a value",
                P,
                in_description(&format!(
                    "{MARK}<content><value gid=\"g\" content=\"x\"/></content>"
                )),
            ),
            Last,
        ),
        (
            row(
                "a gid element in a value",
                P,
                in_description(&format!(
                    "{MARK}<content><value><gid>g</gid></value></content>"
                )),
            ),
            Last,
        ),
        (
            row(
                "an xmi:id on a gid element",
                P,
                in_claim(&format!("{MARK}<gid xmi:id=\"g1\">g</gid>")),
            ),
            Last,
        ),
        (
            row(
                "an element in a gid element",
                P,
                in_claim(&format!("{MARK}<gid>g<b/></gid>")),
            ),
            Last,
        ),
        // The SACMElement flags in the wrong place or form.
        (
            row(
                "sacm:isCitation as an attribute",
                P,
                claim_x(&format!(" {MARK}sacm:isCitation=\"true\""), ""),
            ),
            Last,
        ),
        (
            row(
                "sacm:isAbstract as an element",
                P,
                in_claim(&format!("{MARK}<sacm:isAbstract>true</sacm:isAbstract>")),
            ),
            Last,
        ),
        (
            row(
                "an attribute of an isCitation element",
                P,
                in_claim(&format!("{MARK}<isCitation xmi:id=\"v\">true</isCitation>")),
            ),
            Last,
        ),
        (
            row(
                "an element in an isAbstract element",
                P,
                in_claim(&format!("{MARK}<isAbstract>true<b/></isAbstract>")),
            ),
            Last,
        ),
        (
            row(
                "isCitation on a name",
                P,
                in_claim(&format!("{MARK}<name content=\"N\" isCitation=\"true\"/>")),
            ),
            Last,
        ),
        (
            row(
                "an isAbstract element in a value",
                P,
                in_description(&format!(
                    "{MARK}<content><value><isAbstract>true</isAbstract></value></content>"
                )),
            ),
            Last,
        ),
        // citedElement and abstractForm: an attribute that holds no id, and a
        // defect inside a reference element (the whole element).
        (
            row(
                "an empty citedElement",
                P,
                claim_x(&format!(" {MARK}citedElement=\"\""), ""),
            ),
            Last,
        ),
        (
            row(
                "an abstractForm of white space",
                P,
                claim_x(&format!(" {MARK}abstractForm=\" &#9; \""), ""),
            ),
            Last,
        ),
        (
            row(
                "sacm:citedElement as an attribute",
                P,
                claim_x(&format!(" {MARK}sacm:citedElement=\"c\""), ""),
            ),
            Last,
        ),
        (
            row(
                "citedElement on a content",
                P,
                in_description(&format!("{MARK}<content citedElement=\"c\"/>")),
            ),
            Last,
        ),
        (
            row(
                "a reference element with both link attributes",
                P,
                link(&format!(
                    "{MARK}<citedElement xmi:idref=\"c\" href=\"#c\"/>"
                )),
            ),
            Last,
        ),
        (
            row(
                "a reference element with no link attribute",
                P,
                link(&format!("{MARK}<abstractForm/>")),
            ),
            Last,
        ),
        (
            row(
                "another attribute on a reference element",
                P,
                link(&format!("{MARK}<citedElement href=\"#c\" foo=\"1\"/>")),
            ),
            Last,
        ),
        (
            row(
                "an unqualified idref",
                P,
                link(&format!("{MARK}<citedElement idref=\"c\"/>")),
            ),
            Last,
        ),
        (
            row(
                "an xmi:idref with a space inside",
                P,
                link(&format!("{MARK}<citedElement xmi:idref=\" c x \"/>")),
            ),
            Last,
        ),
        (
            row(
                "an xmi:idref with a tab inside",
                P,
                link(&format!("{MARK}<abstractForm xmi:idref=\"c&#9;x\"/>")),
            ),
            Last,
        ),
        (
            row(
                "an element in a reference element",
                P,
                link(&format!(
                    "{MARK}<citedElement xmi:idref=\"c\"><b/></citedElement>"
                )),
            ),
            Last,
        ),
        (
            row(
                "text in a reference element",
                P,
                link(&format!(
                    "{MARK}<abstractForm xmi:idref=\"c\"> c</abstractForm>"
                )),
            ),
            Last,
        ),
        (
            row(
                "a reference element in a namespace",
                P,
                link(&format!("{MARK}<sacm:citedElement xmi:idref=\"c\"/>")),
            ),
            Last,
        ),
        // Relationship features, metaClaim and structure on a kind that
        // lacks them, and an empty end.
        (
            row(
                "isCounter on a Claim",
                P,
                claim_x(&format!(" {MARK}isCounter=\"true\""), ""),
            ),
            Last,
        ),
        (
            row(
                "an isCounter element in a Claim",
                P,
                claim_x("", &format!("{MARK}<isCounter>true</isCounter>")),
            ),
            Last,
        ),
        (
            row(
                "source on a Claim",
                P,
                claim_x(&format!(" {MARK}source=\"c\""), ""),
            ),
            Last,
        ),
        (
            row(
                "a source element in a Claim",
                P,
                claim_x("", &format!("{MARK}<source xmi:idref=\"c\"/>")),
            ),
            Last,
        ),
        (
            row(
                "structure on a Claim",
                P,
                claim_x(&format!(" {MARK}structure=\"p\""), ""),
            ),
            Last,
        ),
        (
            row(
                "metaClaim on a Note",
                P,
                in_ap(&format!("<note xmi:id=\"n\" {MARK}metaClaim=\"c\"/>")),
            ),
            Last,
        ),
        (
            row(
                "sacm:isCounter on a relationship",
                P,
                inference_x(&format!(" {MARK}sacm:isCounter=\"true\""), ""),
            ),
            Last,
        ),
        (
            row(
                "an empty source",
                P,
                inference_x(&format!(" {MARK}source=\"\""), ""),
            ),
            Last,
        ),
        // The ArtifactReference end.
        (
            row(
                "referencedArtifact on an ArtifactReference",
                P,
                reference(&format!(" {MARK}referencedArtifact=\"c\""), ""),
            ),
            Last,
        ),
        (
            row(
                "a referencedArtifact element in an ArtifactReference",
                P,
                reference("", &format!("{MARK}<referencedArtifact xmi:idref=\"c\"/>")),
            ),
            Last,
        ),
        (
            row(
                "an empty referencedArtifactElement",
                P,
                reference(&format!(" {MARK}referencedArtifactElement=\"\""), ""),
            ),
            Last,
        ),
        (
            row(
                "referencedArtifactElement on a Claim",
                P,
                claim_x(&format!(" {MARK}referencedArtifactElement=\"c\""), ""),
            ),
            Last,
        ),
    ]
}

/// Every row of the table.
fn all_rows() -> Vec<Row> {
    [
        encoding_rows(),
        well_formedness_rows(),
        wrapper_rows(),
        root_rows(),
        name_rows(),
        child_role_rows(),
        child_content_rows(),
        value_element_rows(),
        marked_text_rows(),
        utility_rows(),
        utility_repeat_and_id_rows(),
        gid_rows(),
        flag_rows(),
        reference_rows(),
        relationship_rows(),
        artifact_rows(),
    ]
    .into_iter()
    .flatten()
    .collect()
}

// --- Tests -------------------------------------------------------------------

#[test]
fn sacm23_xmi_010_encoding_declaration_and_decoding_refusals_are_located() {
    assert_located("encoding", &encoding_rows());
}

#[test]
fn sacm23_xmi_010_doctype_and_well_formedness_refusals_are_located() {
    assert_located("well-formedness", &well_formedness_rows());
}

#[test]
fn sacm23_xmi_010_invalid_root_no_package_and_wrapper_refusals_are_located() {
    assert_located("wrapper", &wrapper_rows());
}

#[test]
fn sacm23_xmi_010_root_refusals_are_located() {
    assert_located("root", &root_rows());
}

#[test]
fn sacm23_xmi_010_name_refusals_are_located() {
    assert_located("name", &name_rows());
}

#[test]
fn sacm23_xmi_010_child_role_and_type_refusals_are_located() {
    assert_located("child role", &child_role_rows());
}

#[test]
fn sacm23_xmi_010_child_kind_id_attribute_and_content_refusals_are_located() {
    assert_located("child content", &child_content_rows());
}

#[test]
fn sacm23_xmi_010_value_element_and_repeated_value_refusals_are_located() {
    assert_located("value element", &value_element_rows());
}

#[test]
fn sacm23_xmi_010_text_after_white_space_written_as_a_reference_or_cdata_is_located_at_that_construct()
 {
    assert_located("marked text", &marked_text_rows());
}

#[test]
fn sacm23_xmi_010_utility_element_and_multi_lang_string_refusals_are_located() {
    let rows = [utility_rows(), utility_repeat_and_id_rows()]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    assert_located("utility element", &rows);
}

#[test]
fn sacm23_xmi_010_gid_refusals_are_located() {
    assert_located("gid", &gid_rows());
}

#[test]
fn sacm23_xmi_010_is_citation_and_is_abstract_refusals_are_located() {
    assert_located("flag", &flag_rows());
}

#[test]
fn sacm23_xmi_010_cited_element_and_abstract_form_refusals_are_located() {
    assert_located("reference", &reference_rows());
}

#[test]
fn sacm23_xmi_010_relationship_meta_claim_and_structure_refusals_are_located() {
    assert_located("relationship", &relationship_rows());
}

#[test]
fn sacm23_xmi_010_artifact_reference_and_argument_group_refusals_are_located() {
    assert_located("artifact reference and group", &artifact_rows());
}

#[test]
fn sacm23_xmi_010_the_table_covers_every_read_error_variant() {
    let rows = all_rows();
    for wanted in [
        Variant::Doctype,
        Variant::Malformed,
        Variant::InvalidRoot,
        Variant::NoPackage,
        Variant::Unsupported,
        Variant::DuplicateId,
        Variant::InvalidId,
        Variant::RepeatedValue,
        Variant::InvalidLiteral,
        Variant::LegacyShape,
        Variant::MissingType,
    ] {
        assert!(
            rows.iter().any(|row| row.variant == wanted),
            "{ROW}: the table has no {wanted:?} row"
        );
    }
}

#[test]
fn sacm23_xmi_010_display_shows_the_location_for_every_refusal_site() {
    let mut failures = Vec::new();
    for row in all_rows() {
        let (input, _) = input_of(&row);
        let Err(error) = read(&input, row.mode) else {
            failures.push(format!("{}: a document was returned", row.what));
            continue;
        };
        let Some(location) = error.location() else {
            failures.push(format!("{}: {error:?} has no location", row.what));
            continue;
        };
        let shown = error.to_string();
        // A duplicate id names every element carrying it (SACM23-VAL-001),
        // and a repeated value every occurrence (SACM23-XMI-014). A bad
        // literal gives its location among the literals it lists
        // (SACM23-VAL-006), not necessarily at the end.
        // The variants of SACM-XMI-012 and SACM-XMI-005 (LIB-COMPAT-008,
        // LIB-COMPAT-010) are known here only by their code, so only that
        // the location is shown is checked.
        if matches!(
            error,
            ReadError::DuplicateId { .. }
                | ReadError::RepeatedValue { .. }
                | ReadError::InvalidLiteral { .. }
        ) || matches!(
            variant(&error),
            Some(Variant::LegacyShape | Variant::MissingType)
        ) {
            for location in error.locations() {
                if !shown.contains(&location.to_string()) {
                    failures.push(format!("{}: {shown:?} does not show {location}", row.what));
                }
            }
            continue;
        }
        // The reason, where the variant has one, follows the location after
        // ": " (the `ReadError` Display rules).
        let before_reason = match &error {
            ReadError::Malformed { reason, .. } | ReadError::Unsupported { reason, .. } => {
                shown.strip_suffix(&format!(": {reason}"))
            }
            _ => Some(shown.as_str()),
        };
        let suffix = format!(" at line {}, column {}", location.line(), location.column());
        if !before_reason.is_some_and(|text| text.ends_with(&suffix)) {
            failures.push(format!(
                "{}: {shown:?} does not show {suffix:?} before its reason",
                row.what
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{ROW}: {} rows do not show their location:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn lib_compat_003_content_once_refused_at_a_site_of_the_table_is_preserved_at_that_construct() {
    let rows = preserved_rows();
    let mut failures = Vec::new();
    for (row, holder) in &rows {
        let (input, expected) = input_of(row);
        check_lead(row, expected);
        let before = row.text.split(MARK).next().unwrap_or_default();
        let last_id = before
            .rsplit("xmi:id=\"")
            .next()
            .filter(|_| before.contains("xmi:id=\""))
            .and_then(|rest| rest.split('"').next());
        let written: Vec<&str> = row
            .text
            .split("id=\"")
            .skip(1)
            .filter_map(|rest| rest.split('"').next())
            .collect();
        for (mode, severity) in [
            (Mode::Tolerant, Severity::Warning),
            (Mode::Strict, Severity::Error),
        ] {
            let output = match read(&input, mode) {
                Ok(output) => output,
                Err(error) => {
                    failures.push(format!(
                        "{}: a {mode:?} read refused it instead of preserving it: {error:?}",
                        row.what
                    ));
                    continue;
                }
            };
            let found: Vec<_> = output
                .diagnostics()
                .iter()
                .filter(|found| found.code() == Code::XMI_UNKNOWN_CONTENT)
                .collect();
            let holds = |id: &ElementId| {
                output
                    .document()
                    .get(id.as_str())
                    .is_some_and(SacmElement::has_preserved_content)
                    && match holder {
                        Holder::LastIdBeforeMark => last_id == Some(id.as_str()),
                        Holder::Generated => !written.contains(&id.as_str()),
                    }
            };
            match found.as_slice() {
                [found]
                    if found.severity() == severity
                        && found.location() == Some(expected)
                        && matches!(found.affected(), [id] if holds(id)) => {}
                _ => failures.push(format!(
                    "{}: expected one SACM-XMI-003 at {expected} affecting {holder:?} \
                     {last_id:?} in {mode:?} mode, got {:?}",
                    row.what,
                    output.diagnostics()
                )),
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{COMPAT_003}: {} failures (each row read in both modes) in {} preserved-content rows:\n{}",
        failures.len(),
        rows.len(),
        failures.join("\n")
    );
}

#[test]
fn sacm23_xmi_010_hand_counted_locations_after_crlf_a_lone_cr_and_non_ascii() {
    // Counted by hand, as a check on `location_after`: line 3 starts with
    // `<!-- é -->` and two spaces, 12 characters (13 bytes).
    let cases: [(&[u8], u32, u32); 4] = [
        // The root, right after the lead.
        (
            b"<?xml version=\"1.0\"?>\r\n<!-- -->\r<!-- \xC3\xA9 -->  <r/>",
            3,
            13,
        ),
        // A DOCTYPE there.
        (
            b"<?xml version=\"1.0\"?>\r\n<!-- -->\r<!-- \xC3\xA9 -->  <!DOCTYPE r><r/>",
            3,
            13,
        ),
        // An astral character (four bytes, two UTF-16 units) is one column.
        (b"\r\n\r<!-- \xF0\x9F\x98\x80 -->  <r/>", 3, 13),
        // A UTF-8 byte order mark takes no column.
        (b"\xEF\xBB\xBF<!-- \xC3\xA9 -->\r\n\r\n    <r/>", 3, 5),
    ];
    for (input, line, column) in cases {
        let Err(error) = read(input, Mode::Tolerant) else {
            panic!("{ROW}: {:?} was read", String::from_utf8_lossy(input));
        };
        assert_eq!(
            error.location(),
            Some(loc(line, column)),
            "{ROW}: {:?} is not located by hand count",
            String::from_utf8_lossy(input)
        );
    }
}
