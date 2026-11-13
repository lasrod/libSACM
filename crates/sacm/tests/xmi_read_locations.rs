//! Tests that every refusal of `sacm::xmi::read` is located at what it
//! refuses (SACM23-XMI-010; XML 1.0 (Fifth Edition) §2.11; ADR 0011,
//! ADR 0015).
//!
//! One table holds one input per refusal site: every `ReadError` variant, and
//! every `Unsupported` reason for roots and for children. The expected
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

use sacm::diagnostic::SourceLocation;
use sacm::xmi::{self, ReadError, ReadOptions, ReadOutput};

const ROW: &str = "SACM23-XMI-010";

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
}

/// One refusal site.
struct Row {
    what: &'static str,
    text: String,
    form: Form,
    lead: Lead,
    variant: Variant,
}

fn row(what: &'static str, variant: Variant, text: impl Into<String>) -> Row {
    Row {
        what,
        text: text.into(),
        form: Form::Utf8,
        lead: Lead::Full,
        variant,
    }
}

impl Row {
    fn form(self, form: Form) -> Self {
        Self { form, ..self }
    }

    fn lead(self, lead: Lead) -> Self {
        Self { lead, ..self }
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
            led(&format!("{MARK}<sacm:ArtifactPackage{NS} xmi:id=\"a\"/>")),
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
        // SACM23-XMI-020: a root with no `xmi:id` is read; an unqualified id
        // is an attribute that is not read.
        row(
            "an unqualified id on a root",
            Unsupported,
            led(&format!("<sacm:ArgumentPackage{NS} {MARK}id=\"a\"/>")),
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
            "a root typed as another class",
            Unsupported,
            root(" xmi:id=\"a\" xmi:type=\"sacm:Claim\"", ""),
        ),
        row(
            "an unread attribute of a root",
            Unsupported,
            led(&format!(
                "<sacm:ArgumentPackage{NS} xmi:id=\"a\" {MARK}gid=\"g\"/>"
            )),
        ),
        // SACM23-XMI-021: at the second name.
        row(
            "a second name of a root",
            RepeatedValue,
            in_ap(&format!("<name/>{MARK}<name/>")),
        ),
        row(
            "text in a root",
            Unsupported,
            in_ap(&format!(" \t {MARK}x")),
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
    use Variant::{DuplicateId, InvalidId, Unsupported};
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
        row(
            "a name in a namespace",
            Unsupported,
            in_ap(&format!("{MARK}<sacm:name/>")),
        ),
        row(
            "a name typed as another class",
            Unsupported,
            in_ap(&format!("{MARK}<name xmi:type=\"sacm:Claim\"/>")),
        ),
        row(
            "an unread attribute of a name",
            Unsupported,
            in_ap(&format!("<name content=\"N\" {MARK}xml:lang=\"en\"/>")),
        ),
        // `lang` and `content` are value elements (SACM23-XMI-014); `value`
        // is the legacy form's (LIB-COMPAT-008).
        row(
            "an element in a name",
            Unsupported,
            in_ap(&format!("<name>{MARK}<value/></name>")),
        ),
        row(
            "text in a name",
            Unsupported,
            in_ap(&format!("<name> {MARK}x</name>")),
        ),
    ]
}

/// The refusals of children's roles and types (SACM23-XMI-016).
fn child_role_rows() -> Vec<Row> {
    use Variant::Unsupported as U;
    let element = |attributes: &str| in_ap(&format!("{MARK}<argumentElement{attributes}/>"));
    vec![
        row(
            "a role the parent's class lacks",
            U,
            in_ap(&format!("{MARK}<foo/>")),
        ),
        row(
            "argumentElement in an AssuranceCasePackage",
            U,
            in_acp(&format!(
                "{MARK}<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"x\"/>"
            )),
        ),
        row(
            "argumentationElement",
            U,
            in_ap(&format!(
                "{MARK}<argumentationElement xmi:type=\"sacm:Claim\" xmi:id=\"x\"/>"
            )),
        ),
        row(
            "a namespace-qualified role element",
            U,
            in_ap(&format!(
                "{MARK}<sacm:argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"x\"/>"
            )),
        ),
        row(
            "a role element under a default SACM namespace",
            U,
            led(&format!(
                "<ArgumentPackage xmlns=\"http://www.omg.org/spec/SACM/20220301\" xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" \
             xmi:id=\"p\">{MARK}<argumentElement xmi:type=\"Claim\" xmi:id=\"x\"/></ArgumentPackage>"
            )),
        ),
        row(
            "a type that does not conform to a concrete role",
            U,
            in_acp(&format!(
                "{MARK}<argumentPackage xmi:type=\"sacm:Claim\" xmi:id=\"x\"/>"
            )),
        ),
        row(
            "a type that does not conform to argumentElement",
            U,
            element(" xmi:type=\"sacm:Term\" xmi:id=\"x\""),
        ),
        row(
            "a type naming an abstract class",
            U,
            element(" xsi:type=\"sacm:Assertion\" xmi:id=\"x\""),
        ),
        row(
            "a type naming no SACM class",
            U,
            element(" xmi:type=\"sacm:Goal\" xmi:id=\"x\""),
        ),
        row(
            "a type in a foreign namespace",
            U,
            element(" xmlns:o=\"urn:o\" xmi:type=\"o:Claim\" xmi:id=\"x\""),
        ),
        row(
            "an unresolvable type",
            U,
            element(" xmi:type=\"nope:Claim\" xmi:id=\"x\""),
        ),
        row("an untyped argumentElement", U, element(" xmi:id=\"x\"")),
        row(
            "two type attributes that disagree",
            U,
            element(" xmi:type=\"sacm:Claim\" xsi:type=\"sacm:ArgumentPackage\" xmi:id=\"x\""),
        ),
        row(
            "a conforming kind outside the readable set",
            U,
            element(" xmi:type=\"sacm:AssertedInference\" xmi:id=\"x\""),
        ),
    ]
}

/// The refusals of children's kinds, ids, attributes and content.
fn child_content_rows() -> Vec<Row> {
    use Variant::Unsupported as U;
    let element = |attributes: &str| in_ap(&format!("{MARK}<argumentElement{attributes}/>"));
    vec![
        row(
            "a utility element in a Claim at depth 3",
            U,
            in_claim(&format!("{MARK}<note xmi:id=\"n\"/>")),
        ),
        // SACM23-XMI-020: a child with no `xmi:id` is read; an unqualified id
        // is an attribute that is not read.
        row(
            "an unqualified id on a child",
            U,
            in_ap(&format!(
                "<argumentElement xmi:type=\"sacm:Claim\" {MARK}id=\"x\"/>"
            )),
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
        row(
            "an unread attribute of a child",
            U,
            in_ap(&format!(
                "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"x\" {MARK}gid=\"g\"/>"
            )),
        ),
        row(
            "assertionDeclaration on a child that is not an Assertion",
            U,
            in_acp(&format!(
                "<argumentPackage xmi:id=\"x\" {MARK}assertionDeclaration=\"asserted\"/>"
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
        row("text in a child", U, in_claim(&format!("  {MARK}x"))),
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
    use Variant::{InvalidLiteral, RepeatedValue, Unsupported as U};
    vec![
        row(
            "an attribute of a value element",
            U,
            in_claim(&format!(
                "<name><content {MARK}xmi:id=\"v\">N</content></name>"
            )),
        ),
        row(
            "an element in a value element",
            U,
            in_claim(&format!("<name><lang>en{MARK}<b/></lang></name>")),
        ),
        row(
            "a value element in a namespace",
            U,
            in_claim(&format!(
                "<name>{MARK}<sacm:content>N</sacm:content></name>"
            )),
        ),
        // SACM23-VAL-006: at the value element's `<`.
        row(
            "a bad assertionDeclaration literal in the element form",
            InvalidLiteral,
            in_claim(&format!(
                "{MARK}<assertionDeclaration>Asserted</assertionDeclaration>"
            )),
        ),
        row(
            "an assertionDeclaration element on a package",
            U,
            in_ap(&format!(
                "{MARK}<assertionDeclaration>asserted</assertionDeclaration>"
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

// --- Reading and locating ---------------------------------------------------

/// How long one `read` may take; a hang fails the test instead of stalling.
const DEADLINE: Duration = Duration::from_secs(5);

/// `xmi::read` with the default options, on a worker thread.
fn read(input: &[u8]) -> Result<ReadOutput, ReadError> {
    let owned = input.to_vec();
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let _ = sender.send(xmi::read(&owned, &ReadOptions::default()));
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
        match read(&input) {
            Ok(_) => failures.push(format!("{}: a document was returned", row.what)),
            Err(error)
                if variant(&error) == Some(row.variant) && error.location() == Some(expected) => {}
            Err(error) => failures.push(format!(
                "{}: expected {:?} at {expected}, got {error:?}",
                row.what, row.variant
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
/// start of that construct.
fn marked_text_rows() -> Vec<Row> {
    use Variant::Unsupported as U;
    vec![
        row("\"&#x20;x\" in a root", U, in_ap(&format!("{MARK}&#x20;x"))),
        row(
            "\"&#x20;x\" after literal white space in a Claim",
            U,
            in_claim(&format!(" 	 {MARK}&#x20;x")),
        ),
        row(
            "\"&#x20;x\" in a name",
            U,
            in_ap(&format!("<name>{MARK}&#x20;x</name>")),
        ),
        row(
            "\"&#x20;x\" in the wrapper",
            U,
            wrapped(&format!(
                "{MARK}&#x20;x<sacm:ArgumentPackage xmi:id=\"a\"/>"
            )),
        ),
        row(
            "\"<![CDATA[ x]]>\" in a root",
            U,
            in_ap(&format!("{MARK}<![CDATA[ x]]>")),
        ),
        row(
            "\"<![CDATA[ x]]>\" after literal white space in a Claim",
            U,
            in_claim(&format!("  {MARK}<![CDATA[ x]]>")),
        ),
        row(
            "\"<![CDATA[ x]]>\" in a name",
            U,
            in_ap(&format!("<name>{MARK}<![CDATA[ x]]></name>")),
        ),
        row(
            "\"<![CDATA[ ]]>x\" in a Claim",
            U,
            in_claim(&format!("{MARK}<![CDATA[ ]]>x")),
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
        let Err(error) = read(&input) else {
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
        if matches!(
            error,
            ReadError::DuplicateId { .. }
                | ReadError::RepeatedValue { .. }
                | ReadError::InvalidLiteral { .. }
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
        let Err(error) = read(input) else {
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
