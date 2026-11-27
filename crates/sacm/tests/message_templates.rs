//! Every read and write refusal has a code, a template and arguments
//! (LIB-API-017, acceptance 4; ADR 0030 amendment 3).
//!
//! One input per refusal template of `docs/sacm/diagnostics.md`'s
//! `## Message templates` table: a strict read, or a strict write of a
//! document built through the public API. For each, the table states the
//! template id, its arguments by name, kind and value, the English pattern
//! the template names (as the catalog lists it), and the whole `Display`.
//!
//! - `lib_api_017_each_refusal_template_input_gives_its_0_1_0_display` runs
//!   on today's API. The `Display` of each input is pinned byte for byte as
//!   0.1.0 wrote it, and the table is checked against itself: the pattern,
//!   rendered with the arguments, is the variable part of the `Display`;
//!   every placeholder names an argument of the right kind; the arguments
//!   come in the order the pattern first names them, `row` last and only on
//!   the `Unsupported` templates of SACM-XMI-002 and -015 but
//!   `SACM-XMI-015/not-xmi-element`, and on
//!   `SACM-XMI-008/other-revision-root`, whose `Display` ends with the label
//!   of their code; and the inputs and the templates no input reaches
//!   together are the 104 refusal templates, once each.
//! - `lib_api_017_each_refusal_template_has_its_code_template_and_arguments`
//!   checks that `code()`, `template()` and `arguments()` are the
//!   table's.
//!
//! Eleven templates have no input through the public API, and are built by
//! a unit test of their crate-private constructors instead (see
//! [`NO_INPUT`]): the SACM-SEC-002 nodes and attributes capacity limits
//! (`/namespaces` is reached by 65,536 namespace prefixes, as LIB-SEC-003's
//! test reaches it) and the three
//! `SACM-SEC-002/unheld-*` refusals, which acceptance 4 names; five
//! SACM-XML-001 forms that need a DTD's entity declarations, which every
//! read refuses first as a DOCTYPE (SACM-SEC-001); and
//! `SACM-XMI-012/name-value-text`, whose shape always follows the
//! `name-value` shape of the same `value`, so a strict read, which refuses
//! the first shape, never refuses it.
//!
//! Slice 0065 reads `ExpressionLangString` (SACM23-BASE-009) and removes the
//! template `SACM-XMI-014/expression-lang-string`, which leaves 102. Slice
//! 0067 reads the last package kinds and every package's `interface` end
//! and removes the four templates no input reaches any more
//! (`SACM-XMI-014/interface-element`, `/unread-kind`, `/unread-kind-in-root`
//! and `/unread-root`; ADR 0035 amendment 1), which leaves 98. Slice 0068
//! reads an `href` with a document part (SACM23-XMI-027, ADR 0040): the four
//! SACM-XMI-007 inputs name SACM23-XMI-029, which reads the links left, and
//! `SACM-XMI-007/href` has new English (its id and arguments unchanged) and
//! an `XPointer` as its input.
//!
//! Slice 0072 (LIB-SEC-003, LIB-COMPAT-004, LIB-COMPAT-016; ADR 0041) adds
//! eleven and removes one, which gives 109:
//! - the seven `SACM-SEC-003` templates of `ReadError::LimitExceeded`, whose
//!   inputs are read under `Limits` set for the case, and whose `Display`
//!   is the rendered template and its location, when known;
//! - `SACM-XML-001/encoding-contradicts-utf8-bom`;
//! - `SACM-XMI-001/not-a-sacm-namespace` and `/no-namespace`, which a root
//!   outside any SACM namespace gives instead of `SACM-XMI-002/no-namespace`,
//!   removed;
//! - `SACM-XMI-008/other-revision-root` and `/sacm-2-4`.
//!
//! Slice 0073a reads XMI metadata (SACM23-XMI-019, ADR 0042): it removes the
//! seven `Unsupported` templates of SACM-XMI-014, whose code no input gives
//! any more, and adds `SACM-XMI-001/xmi-namespace-root` and
//! `SACM-XMI-003/wrapper-element`, which gives 104.
//!
//! Slice 0073b reads the cross-document links left (SACM23-XMI-029, ADR 0040
//! amendment 1): it removes the four `Unsupported` templates of
//! SACM-XMI-007, whose code no input gives any more, which gives 100. Their
//! inputs read: `refusal_codes.rs::sacm23_xmi_029_no_entry_is_interim`.
//!
//! Slice 0075a reads the dialect EMF writes (LIB-COMPAT-002, ADR 0044 D1): it
//! adds `SACM-XMI-015/not-xmi-element`, for an `XMI` element in neither XMI
//! namespace, a limit with no row, and `SACM-XMI-015/emf-xmi-namespace`, for
//! any other name in EMF's XMI namespace read strictly, which gives 102. The
//! label of SACM-XMI-015 with a row is `(strict mode never reads it; ROW
//! reads it in tolerant mode)` since.
//!
//! Slice 0077 limits the attributes of an element (LIB-SEC-007, ADR 0041
//! amendment 2): it adds `SACM-SEC-003/attributes`, an eighth `SACM-SEC-003`
//! template, which gives 103.
//!
//! Slice 0082 gives a write refusal in a value its position (LIB-API-013):
//! it adds `SACM-XML-003/unrepresentable-character-in-value`, which gives
//! 104.
//!
//! Slice 0072 also changes the English of the three `SACM-XML-002` templates, which
//! name ISO-8859-1 now, and the label of SACM-XMI-002 and SACM-XMI-008 with
//! a row, `(strict mode never reads it; ROW reads it in tolerant mode)`
//! (ADR 0041 D6). Their `Display`s here are the new ones; the input of
//! `SACM-XMI-002/other-namespace` is a SACM-like namespace, and that of
//! `SACM-XML-002/declared-encoding` declares ISO-8859-2.
//!
//! Slice 0081 makes five arguments here prose (LIB-DIAG-004): `encoding` of
//! `SACM-XML-002/byte-order-mark` and `/detected-encoding`, `expected` of
//! `SACM-XML-001/declaration-character` and `/unexpected-character`, and
//! `attribute` of `SACM-XML-003/unrepresentable-character`. A prose
//! argument is neither Text nor a Count, and its `Display` is unchanged.

use std::fmt::Write as _;

use sacm::edit::{self, CreateArgumentPackage, CreateNote, Operation};
use sacm::model::{Document, LangString, MultiLangString};
use sacm::xmi::{self, Mode, ReadError, ReadOptions, WriteError, WriteOptions};

const ROW: &str = "LIB-API-017";

const SACM_NS: &str = "http://www.omg.org/spec/SACM/20220301";
const XMI_NS: &str = "http://www.omg.org/spec/XMI/20131001";

/// The namespace declarations of a root: `sacm` and `xmi`.
const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
);

/// The number of refusal templates (acceptance 4): 102 once slice 0065
/// removes `SACM-XMI-014/expression-lang-string` (SACM23-BASE-009), 98 once
/// slice 0067 removes the four of unread kinds and ends, 109 once slice
/// 0072 adds eleven and removes `SACM-XMI-002/no-namespace`, and 104 once
/// slice 0073a removes the seven `Unsupported` templates of SACM-XMI-014
/// and adds `SACM-XMI-001/xmi-namespace-root` and
/// `SACM-XMI-003/wrapper-element` (SACM23-XMI-019, ADR 0042), and 100 once
/// slice 0073b removes the four `Unsupported` templates of SACM-XMI-007,
/// whose links it reads (SACM23-XMI-029, ADR 0040 amendment 1), and 102 once
/// slice 0075a adds `SACM-XMI-015/not-xmi-element` and
/// `SACM-XMI-015/emf-xmi-namespace` (LIB-COMPAT-002, ADR 0044 D1), and 103
/// once slice 0077 adds `SACM-SEC-003/attributes` (LIB-SEC-007, ADR 0041
/// amendment 2), and 104 once slice 0082 adds
/// `SACM-XML-003/unrepresentable-character-in-value` (LIB-API-013).
const REFUSAL_TEMPLATES: usize = 104;

/// The refusal templates no input reaches through the public API, and why.
/// A unit test of their crate-private constructors builds each.
const NO_INPUT: [(&str, &str); 11] = [
    (
        "SACM-SEC-002/nodes",
        "the parser's node limit is beyond a test's size: a node limit below 2^32 is \
         LimitExceeded",
    ),
    (
        "SACM-SEC-002/attributes",
        "the parser's attribute limit is beyond a test's size",
    ),
    (
        "SACM-SEC-002/unheld-outside",
        "checked input never fails to be held",
    ),
    (
        "SACM-SEC-002/unheld-root",
        "checked input never fails to be held",
    ),
    (
        "SACM-SEC-002/unheld-element",
        "checked input never fails to be held",
    ),
    (
        "SACM-XML-001/entity-closes-element",
        "it needs an entity declared in a DTD, which is refused first",
    ),
    (
        "SACM-XML-001/entity-loop",
        "it needs an entity declared in a DTD, which is refused first",
    ),
    (
        "SACM-XML-001/lt-in-attribute-value",
        "it needs an entity declared in a DTD, which is refused first",
    ),
    (
        "SACM-XML-001/invalid-external-id",
        "it is inside a DOCTYPE, which is refused first",
    ),
    (
        "SACM-XML-001/unresolved-entity",
        "it needs an external entity declared in a DTD, which is refused first",
    ),
    (
        "SACM-XMI-012/name-value-text",
        "the name-value shape of the same value comes first, and is the one refused",
    ),
];

// --- The table -----------------------------------------------------------------------

/// An argument's expected value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Value {
    Text(&'static str),
    Count(u64),
    /// English prose that the crate or its XML parser wrote (LIB-DIAG-004).
    Prose(&'static str),
}

fn t(text: &'static str) -> Value {
    Value::Text(text)
}

/// A prose argument (LIB-DIAG-004): the catalog marks it `(prose)`.
fn p(text: &'static str) -> Value {
    Value::Prose(text)
}

fn n(count: u64) -> Value {
    Value::Count(count)
}

/// What is refused: the bytes of a strict read, or a document a strict write
/// refuses.
enum Input {
    Read(Vec<u8>),
    /// A strict read under limits set for the case (LIB-SEC-003).
    ReadUnder(Vec<u8>, xmi::Limits),
    Write(Document),
}

/// One refusal template and its input.
struct Case {
    /// The template id, `<CODE>/<form>`.
    template: &'static str,
    input: Input,
    /// The arguments, in the template's order.
    arguments: Vec<(&'static str, Value)>,
    /// The template's English pattern: the variable part of the `Display`.
    english: &'static str,
    /// The whole `Display`, byte for byte as 0.1.0 wrote it.
    display: &'static str,
}

fn read(
    template: &'static str,
    input: impl Into<Vec<u8>>,
    arguments: &[(&'static str, Value)],
    english: &'static str,
    display: &'static str,
) -> Case {
    Case {
        template,
        input: Input::Read(input.into()),
        arguments: arguments.to_vec(),
        english,
        display,
    }
}

fn write(
    template: &'static str,
    document: Document,
    arguments: &[(&'static str, Value)],
    english: &'static str,
    display: &'static str,
) -> Case {
    Case {
        template,
        input: Input::Write(document),
        arguments: arguments.to_vec(),
        english,
        display,
    }
}

/// A root ArgumentPackage `p` holding `body`.
fn package(body: &str) -> String {
    format!(
        "<sacm:ArgumentPackage{NS} xmi:id=\"p\"><name content=\"p\"/>{body}</sacm:ArgumentPackage>"
    )
}

/// A root ArgumentPackage `p` whose name element is `name`.
fn named_package(name: &str) -> String {
    format!("<sacm:ArgumentPackage{NS} xmi:id=\"p\">{name}</sacm:ArgumentPackage>")
}

/// A Claim `id` with the extra attributes `attributes`, holding `body` after
/// its name.
fn claim(id: &str, attributes: &str, body: &str) -> String {
    format!(
        "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"{id}\"{attributes}><name content=\"{id}\"/>{body}</argumentElement>"
    )
}

/// The `xmi:XMI` wrapper with `attributes` and `body` before its one root.
fn wrapper(attributes: &str, body: &str) -> String {
    format!(
        "<xmi:XMI xmlns:xmi=\"{XMI_NS}\" xmlns:sacm=\"{SACM_NS}\"{attributes}>{body}\
         <sacm:ArgumentPackage xmi:id=\"p\"><name content=\"p\"/></sacm:ArgumentPackage></xmi:XMI>"
    )
}

/// `text` encoded as UTF-16 LE, with a byte order mark or without.
fn utf16le(text: &str, mark: bool) -> Vec<u8> {
    let mut bytes = if mark { vec![0xFF, 0xFE] } else { Vec::new() };
    bytes.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
    bytes
}

/// The document a tolerant read of `input` returns.
fn tolerant_document(input: &str) -> Document {
    match xmi::read(
        input.as_bytes(),
        &ReadOptions::default().with_mode(Mode::Tolerant),
    ) {
        Ok(output) => output.into_document(),
        Err(error) => panic!("{ROW}: setup: a tolerant read refuses {input}: {error}"),
    }
}

/// A document holding the root ArgumentPackage `arguments`, whose name holds
/// U+0001. An edit refuses it (LIB-EDIT-025), so the test seam creates it.
fn unrepresentable_name() -> Document {
    let mut document = Document::new();
    let name = LangString::new("bad\u{1}");
    if let Err(error) = edit::apply_unchecked_text(
        &mut document,
        CreateArgumentPackage::root(name).with_id("arguments"),
    ) {
        panic!("{ROW}: setup: the package is not created: {error}");
    }
    document
}

/// A document holding the root ArgumentPackage `p` and its Note `n1`, whose
/// second content value holds U+0004 (slice 0082, LIB-API-013), built with
/// the test seam.
fn unrepresentable_value() -> Document {
    let mut document = Document::new();
    let content: MultiLangString = [LangString::new("ok"), LangString::new("bad\u{4}")]
        .into_iter()
        .collect();
    for operation in [
        Operation::from(CreateArgumentPackage::root(LangString::new("P")).with_id("p")),
        CreateNote::new("p", content).with_id("n1").into(),
    ] {
        if let Err(error) = edit::apply_unchecked_text(&mut document, operation) {
            panic!("{ROW}: setup: the note is not created: {error}");
        }
    }
    document
}

/// The 94 templates an input reaches with [`limit_cases`] (98 until slice 0073a, 93 until slice
/// 0082), mostly in the catalog's grouping.
#[expect(
    clippy::too_many_lines,
    reason = "one table, one row per template, reads best whole"
)]
fn cases() -> Vec<Case> {
    let dangling = |id: &str| {
        format!(
            "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"{id}\" isCitation=\"true\" \
             citedElement=\"nope\"><name content=\"{id}\"/></argumentElement>"
        )
    };
    let mut cases = vec![
        // --- ReadError::Doctype --------------------------------------------------------
        read(
            "SACM-SEC-001/doctype",
            "<!DOCTYPE r>\n<r/>",
            &[],
            "the input has a DOCTYPE, which is refused",
            "the input has a DOCTYPE, which is refused at line 1, column 1",
        ),
        // --- ReadError::Unsupported, SACM-XML-002 --------------------------------------
        // The English names ISO-8859-1 since slice 0072 (LIB-COMPAT-016).
        read(
            "SACM-XML-002/byte-order-mark",
            {
                let mut bytes = vec![0x00, 0x00, 0xFE, 0xFF];
                bytes.extend("<r/>".chars().flat_map(|c| u32::from(c).to_be_bytes()));
                bytes
            },
            &[("encoding", p("UTF-32 (big-endian)"))],
            "the input starts with a {encoding} byte order mark, and only UTF-8, ASCII, ISO-8859-1 and UTF-16 are read",
            "the reader cannot represent the input at line 1, column 1: the input starts with a UTF-32 (big-endian) byte order mark, and only UTF-8, ASCII, ISO-8859-1 and UTF-16 are read (a limit of this version)",
        ),
        read(
            "SACM-XML-002/detected-encoding",
            utf16le("<?xml version=\"1.0\" encoding=\"UTF-16LE\"?><r/>", false),
            &[("encoding", p("UTF-16 (little-endian)"))],
            "the input is encoded in {encoding}, and only UTF-8, ASCII, ISO-8859-1 and UTF-16 starting with U+FEFF are read",
            "the reader cannot represent the input at line 1, column 1: the input is encoded in UTF-16 (little-endian), and only UTF-8, ASCII, ISO-8859-1 and UTF-16 starting with U+FEFF are read (a limit of this version)",
        ),
        read(
            "SACM-XML-002/declared-encoding",
            "<?xml version=\"1.0\" encoding=\"ISO-8859-2\"?><r/>",
            &[("encoding", t("ISO-8859-2"))],
            "the declared encoding {encoding:?} is not supported: without a UTF-16 byte order mark, only UTF-8, ASCII and ISO-8859-1 are read",
            "the reader cannot represent the input at line 1, column 31: the declared encoding \"ISO-8859-2\" is not supported: without a UTF-16 byte order mark, only UTF-8, ASCII and ISO-8859-1 are read (a limit of this version)",
        ),
        // --- ReadError::Unsupported, SACM-SEC-002 (slice 0072) -------------------------
        // 65,536 distinct prefixes, one per child, pass the parser's table of
        // namespaces (LIB-SEC-003, acceptance 8). No limit is passed: each
        // child has one binding in scope, and 65,536 copies.
        read(
            "SACM-SEC-002/namespaces",
            many_prefixes(),
            &[],
            "the document declares more namespaces than the parser can hold",
            "the reader cannot represent the input: the document declares more namespaces than the parser can hold (a limit of this version)",
        ),
        // --- WriteError::UnrepresentableCharacter --------------------------------------
        write(
            "SACM-XML-003/unrepresentable-character",
            unrepresentable_name(),
            &[
                ("element", t("arguments")),
                ("attribute", p("name's content")),
                ("code_point", n(1)),
            ],
            "the element {element:?} cannot be written: its {attribute} holds U+{code_point:04X}, which XML 1.0 cannot represent",
            "the element \"arguments\" cannot be written: its name's content holds U+0001, which XML 1.0 cannot represent",
        ),
        // A character in a value, with the value's position (slice 0082,
        // LIB-API-013).
        write(
            "SACM-XML-003/unrepresentable-character-in-value",
            unrepresentable_value(),
            &[
                ("element", t("n1")),
                ("attribute", p("content value's content")),
                ("position", n(1)),
                ("code_point", n(4)),
            ],
            "the element {element:?} cannot be written: its {attribute} at index {position} holds U+{code_point:04X}, which XML 1.0 cannot represent",
            "the element \"n1\" cannot be written: its content value's content at index 1 holds U+0004, which XML 1.0 cannot represent",
        ),
        // --- SACM-XMI-001 --------------------------------------------------------------
        read(
            "SACM-XMI-001/not-a-package",
            "<r/>",
            &[("element", t("r"))],
            "the root element {element:?} is not a SACM package",
            "the root element \"r\" is not a SACM package at line 1, column 1",
        ),
        read(
            "SACM-XMI-001/no-package",
            format!("<xmi:XMI xmlns:xmi=\"{XMI_NS}\"/>"),
            &[],
            "the XMI element holds no SACM package",
            "the XMI element holds no SACM package at line 1, column 1",
        ),
        // An XMI-namespace document element other than `xmi:XMI` (slice 0073a,
        // SACM23-XMI-019, ADR 0042 D4): `SACM-XMI-014/xmi-root` until then.
        read(
            "SACM-XMI-001/xmi-namespace-root",
            format!("<xmi:Extension xmlns:xmi=\"{XMI_NS}\"/>"),
            &[("element", t("xmi:Extension"))],
            "the document element {element:?} is in the XMI namespace but is not xmi:XMI, so the document holds no SACM package",
            "the document element \"xmi:Extension\" is in the XMI namespace but is not xmi:XMI, so the document holds no SACM package at line 1, column 1",
        ),
        // A package-kind root outside any SACM namespace (slice 0072, ADR 0041
        // D4).
        read(
            "SACM-XMI-001/not-a-sacm-namespace",
            format!("<p:ArgumentPackage xmlns:p=\"urn:x\" xmlns:xmi=\"{XMI_NS}\" xmi:id=\"p\"/>"),
            &[
                ("element", t("p:ArgumentPackage")),
                ("namespace", t("urn:x")),
            ],
            "the root element {element:?} is in the namespace {namespace:?}, which does not name SACM",
            "the root element \"p:ArgumentPackage\" is in the namespace \"urn:x\", which does not name SACM at line 1, column 1",
        ),
        read(
            "SACM-XMI-001/no-namespace",
            format!("<ArgumentPackage xmlns:xmi=\"{XMI_NS}\" xmi:id=\"p\"/>"),
            &[("element", t("ArgumentPackage"))],
            "the root element {element:?} is in no namespace, so it is not a SACM package",
            "the root element \"ArgumentPackage\" is in no namespace, so it is not a SACM package at line 1, column 1",
        ),
        write(
            "SACM-XMI-001/no-root-to-write",
            Document::new(),
            &[],
            "the document has no root to write",
            "the document has no root to write",
        ),
        // --- ReadError::Unsupported, SACM-XMI-002 --------------------------------------
        // A SACM-like namespace since slice 0072 (a SACM 2.2 one until then,
        // which is SACM-XMI-008 since); `SACM-XMI-002/no-namespace` is
        // removed, as a root in no namespace is SACM-XMI-001.
        read(
            "SACM-XMI-002/other-namespace",
            format!(
                "<sacm:ArgumentPackage xmlns:sacm=\"urn:example:sacm\" \
                 xmlns:xmi=\"{XMI_NS}\" xmi:id=\"p\"/>"
            ),
            &[
                ("element", t("sacm:ArgumentPackage")),
                ("namespace", t("urn:example:sacm")),
                ("row", t("LIB-COMPAT-004")),
            ],
            "the root element {element:?} is in the namespace {namespace:?}, not the SACM 2.3 namespace \"http://www.omg.org/spec/SACM/20220301\"",
            "the reader cannot represent the input at line 1, column 1: the root element \"sacm:ArgumentPackage\" is in the namespace \"urn:example:sacm\", not the SACM 2.3 namespace \"http://www.omg.org/spec/SACM/20220301\" (strict mode never reads it; LIB-COMPAT-004 reads it in tolerant mode)",
        ),
        // --- ReadError::Unsupported, SACM-XMI-008 (slice 0072) -------------------------
        read(
            "SACM-XMI-008/other-revision-root",
            format!(
                "<sacm:ArgumentPackage xmlns:sacm=\"http://www.omg.org/spec/SACM/2.2/argumentation\" \
                 xmlns:xmi=\"{XMI_NS}\" xmi:id=\"p\"/>"
            ),
            &[
                ("element", t("sacm:ArgumentPackage")),
                (
                    "namespace",
                    t("http://www.omg.org/spec/SACM/2.2/argumentation"),
                ),
                ("revision", t("2.2")),
                ("row", t("LIB-COMPAT-004")),
            ],
            "the root element {element:?} is in the namespace {namespace:?} of SACM {revision}, not the SACM 2.3 namespace \"http://www.omg.org/spec/SACM/20220301\"",
            "the reader cannot represent the input at line 1, column 1: the root element \"sacm:ArgumentPackage\" is in the namespace \"http://www.omg.org/spec/SACM/2.2/argumentation\" of SACM 2.2, not the SACM 2.3 namespace \"http://www.omg.org/spec/SACM/20220301\" (strict mode never reads it; LIB-COMPAT-004 reads it in tolerant mode)",
        ),
        read(
            "SACM-XMI-008/sacm-2-4",
            format!(
                "<sacm:ArgumentPackage xmlns:sacm=\"http://www.omg.org/spec/SACM/20260504\" \
                 xmlns:xmi=\"{XMI_NS}\" xmi:id=\"p\"/>"
            ),
            &[
                ("element", t("sacm:ArgumentPackage")),
                ("namespace", t("http://www.omg.org/spec/SACM/20260504")),
            ],
            "the root element {element:?} is in the SACM 2.4 namespace {namespace:?}, and SACM 2.4 restructures the metamodel, so it is not read as SACM 2.3 until LIB-COMPAT-018 decides how",
            "the reader cannot represent the input at line 1, column 1: the root element \"sacm:ArgumentPackage\" is in the SACM 2.4 namespace \"http://www.omg.org/spec/SACM/20260504\", and SACM 2.4 restructures the metamodel, so it is not read as SACM 2.3 until LIB-COMPAT-018 decides how (a limit of this version)",
        ),
        // --- ReadError::Unsupported, SACM-XMI-003 --------------------------------------
        read(
            "SACM-XMI-003/unread-attribute",
            wrapper(" foo=\"bar\"", ""),
            &[("attribute", t("foo")), ("element", t("xmi:XMI"))],
            "the attribute {attribute:?} of the element {element:?} is not read in this release",
            "the reader cannot represent the input at line 1, column 110: the attribute \"foo\" of the element \"xmi:XMI\" is not read in this release (a defect in the input)",
        ),
        read(
            "SACM-XMI-003/wrapper-text",
            wrapper("", "stray"),
            &[("element", t("xmi:XMI"))],
            "the element {element:?} holds text, which has no element to be kept in",
            "the reader cannot represent the input at line 1, column 110: the element \"xmi:XMI\" holds text, which has no element to be kept in (a defect in the input)",
        ),
        // An XMI-namespace element XMI 2.5.1 does not define under `xmi:XMI`
        // (slice 0073a, SACM23-XMI-019, ADR 0042 D4).
        read(
            "SACM-XMI-003/wrapper-element",
            wrapper("", "<xmi:foo/>"),
            &[("element", t("xmi:foo")), ("container", t("xmi:XMI"))],
            "the element {element:?} in {container:?} is in the XMI namespace, but XMI 2.5.1 defines no such element there, and it has no element to be kept in",
            "the reader cannot represent the input at line 1, column 110: the element \"xmi:foo\" in \"xmi:XMI\" is in the XMI namespace, but XMI 2.5.1 defines no such element there, and it has no element to be kept in (a defect in the input)",
        ),
        read(
            "SACM-XMI-003/unwritable-prefix",
            package(&format!(
                "<argumentElement xmlns:s=\"{SACM_NS}\" xmlns:sacm=\"urn:example:other\" \
                 xmi:type=\"s:Claim\" xmi:id=\"c1\" sacm:extra=\"1\"><name content=\"c1\"/></argumentElement>"
            )),
            &[
                ("attribute", t("sacm:extra")),
                ("element", t("argumentElement")),
                ("prefix", t("sacm")),
                ("namespace", t("urn:example:other")),
                ("pinned_namespace", t(SACM_NS)),
            ],
            "the attribute {attribute:?} of the element {element:?} is not SACM 2.3, and cannot be kept: its prefix {prefix:?} is bound to {namespace:?}, and the written element binds it to {pinned_namespace:?}",
            "the reader cannot represent the input at line 1, column 280: the attribute \"sacm:extra\" of the element \"argumentElement\" is not SACM 2.3, and cannot be kept: its prefix \"sacm\" is bound to \"urn:example:other\", and the written element binds it to \"http://www.omg.org/spec/SACM/20220301\" (a defect in the input)",
        ),
        // --- ReadError::MissingType ----------------------------------------------------
        read(
            "SACM-XMI-005/missing-type",
            package("<argumentElement xmi:id=\"c1\"/>"),
            &[
                ("role", t("argumentElement")),
                ("type", t("ArgumentationElement")),
            ],
            "the element {role:?} has no xmi:type, and its role has the abstract type {type}",
            "the element \"argumentElement\" has no xmi:type, and its role has the abstract type ArgumentationElement at line 1, column 153: give it an xmi:type that names its class",
        ),
        // --- WriteError::PreservedContent ----------------------------------------------
        write(
            "SACM-XMI-006/one-holder",
            tolerant_document(&format!(
                "<sacm:ArgumentPackage{NS} xmlns:v=\"urn:vendor\" xmi:id=\"p\" v:colour=\"red\">\
                 <name content=\"p\"/></sacm:ArgumentPackage>"
            )),
            &[("id", t("p"))],
            "the document cannot be written in strict mode: the element {id:?} holds preserved content",
            "the document cannot be written in strict mode: the element \"p\" holds preserved content",
        ),
        write(
            "SACM-XMI-006/holders",
            tolerant_document(&format!(
                "<sacm:ArgumentPackage{NS} xmlns:v=\"urn:vendor\" xmi:id=\"p\" v:colour=\"red\">\
                 <name content=\"p\"/>{}</sacm:ArgumentPackage>",
                claim("c1", " v:size=\"2\"", "")
            )),
            &[("count", n(2)), ("id", t("p"))],
            "the document cannot be written in strict mode: {count} elements hold preserved content, the first is {id:?}",
            "the document cannot be written in strict mode: 2 elements hold preserved content, the first is \"p\"",
        ),
        write(
            "SACM-XMI-006/outside-elements",
            tolerant_document(&wrapper("", "stray")),
            &[],
            "the document cannot be written in strict mode: the document holds preserved content outside its elements",
            "the document cannot be written in strict mode: the document holds preserved content outside its elements",
        ),
        // --- ReadError::LegacyShape, SACM-XMI-012 --------------------------------------
        read(
            "SACM-XMI-012/name-attribute",
            format!("<sacm:ArgumentPackage{NS} xmi:id=\"arguments\" name=\"Arguments\"/>"),
            &[("class", t("ArgumentPackage")), ("id", t("arguments"))],
            "the name of the {class} {id:?} is given as the attribute \"name\"",
            "the name of the ArgumentPackage \"arguments\" is given as the attribute \"name\", a shape of the assurance-forge C++ library at line 1, column 142: a strict read refuses it; a tolerant read reads it as its standard form, which write writes",
        ),
        read(
            "SACM-XMI-012/content-on-element",
            package(&claim("c1", "", "<note xmi:id=\"n\" content=\"x\"/>")),
            &[("class", t("Note")), ("id", t("n"))],
            "the content of the {class} {id:?} is given on the element itself",
            "the content of the Note \"n\" is given on the element itself, a shape of the assurance-forge C++ library at line 1, column 241: a strict read refuses it; a tolerant read reads it as its standard form, which write writes",
        ),
        read(
            "SACM-XMI-012/key-attribute",
            package(&claim("c1", "", "<taggedValue xmi:id=\"t\" key=\"k\"/>")),
            &[("class", t("TaggedValue")), ("id", t("t"))],
            "the key of the {class} {id:?} is given as the attribute \"key\"",
            "the key of the TaggedValue \"t\" is given as the attribute \"key\", a shape of the assurance-forge C++ library at line 1, column 248: a strict read refuses it; a tolerant read reads it as its standard form, which write writes",
        ),
        read(
            "SACM-XMI-012/shorthand",
            package(&claim(
                "c1",
                "",
                "<note xmi:id=\"n\"><content content=\"x\" lang=\"en\"/></note>",
            )),
            &[("role", t("content")), ("class", t("Note")), ("id", t("n"))],
            "the {role} of the {class} {id:?} is given in the single-language shorthand",
            "the content of the Note \"n\" is given in the single-language shorthand, a shape of the assurance-forge C++ library at line 1, column 250: a strict read refuses it; a tolerant read reads it as its standard form, which write writes",
        ),
        read(
            "SACM-XMI-012/name-text",
            named_package("<name>p</name>"),
            &[("class", t("ArgumentPackage")), ("id", t("p"))],
            "the name of the {class} {id:?} has its content as text",
            "the name of the ArgumentPackage \"p\" has its content as text, a shape of the assurance-forge C++ library at line 1, column 140: a strict read refuses it; a tolerant read reads it as its standard form, which write writes",
        ),
        read(
            "SACM-XMI-012/value-text",
            package(&claim(
                "c1",
                "",
                "<note xmi:id=\"n\"><content><value>x</value></content></note>",
            )),
            &[("role", t("content")), ("class", t("Note")), ("id", t("n"))],
            "a value of the {role} of the {class} {id:?} has its content as text",
            "a value of the content of the Note \"n\" has its content as text, a shape of the assurance-forge C++ library at line 1, column 257: a strict read refuses it; a tolerant read reads it as its standard form, which write writes",
        ),
        read(
            "SACM-XMI-012/name-content-lang",
            named_package("<name><content lang=\"en\">p</content></name>"),
            &[("class", t("ArgumentPackage")), ("id", t("p"))],
            "the name of the {class} {id:?} holds a content carrying lang",
            "the name of the ArgumentPackage \"p\" holds a content carrying lang, a shape of the assurance-forge C++ library at line 1, column 149: a strict read refuses it; a tolerant read reads it as its standard form, which write writes",
        ),
        read(
            "SACM-XMI-012/name-value",
            named_package("<name><value content=\"p\"/></name>"),
            &[("class", t("ArgumentPackage")), ("id", t("p"))],
            "the name of the {class} {id:?} holds a value",
            "the name of the ArgumentPackage \"p\" holds a value, a shape of the assurance-forge C++ library at line 1, column 140: a strict read refuses it; a tolerant read reads it as its standard form, which write writes",
        ),
        // --- ReadError::Unsupported, SACM-XMI-015 --------------------------------------
        read(
            "SACM-XMI-015/not-xmi-namespace",
            format!(
                "<xmi:XMI xmlns:xmi=\"http://www.omg.org/XMI\" xmlns:x=\"{XMI_NS}\" xmlns:sacm=\"{SACM_NS}\">\
                 <sacm:ArgumentPackage x:id=\"p\"><name content=\"p\"/></sacm:ArgumentPackage></xmi:XMI>"
            ),
            &[("element", t("xmi:XMI")), ("row", t("LIB-COMPAT-002"))],
            "the element {element:?} is not in the XMI namespace \"http://www.omg.org/spec/XMI/20131001\", so it is not read as the XMI element",
            "the reader cannot represent the input at line 1, column 1: the element \"xmi:XMI\" is not in the XMI namespace \"http://www.omg.org/spec/XMI/20131001\", so it is not read as the XMI element (strict mode never reads it; LIB-COMPAT-002 reads it in tolerant mode)",
        ),
        read(
            "SACM-XMI-015/not-xmi-element",
            format!(
                "<x:XMI xmlns:x=\"http://www.omg.org/spec/XMI/20110701\" xmlns:xmi=\"{XMI_NS}\" xmlns:sacm=\"{SACM_NS}\">\
                 <sacm:ArgumentPackage xmi:id=\"p\"><name content=\"p\"/></sacm:ArgumentPackage></x:XMI>"
            ),
            &[("element", t("x:XMI"))],
            "the element {element:?} is in neither the XMI 2.5.1 namespace \"http://www.omg.org/spec/XMI/20131001\" nor \"http://www.omg.org/XMI\", so it is not read as the XMI element",
            "the reader cannot represent the input at line 1, column 1: the element \"x:XMI\" is in neither the XMI 2.5.1 namespace \"http://www.omg.org/spec/XMI/20131001\" nor \"http://www.omg.org/XMI\", so it is not read as the XMI element (a limit of this version)",
        ),
        read(
            "SACM-XMI-015/emf-xmi-namespace",
            format!(
                "<sacm:ArgumentPackage xmlns:sacm=\"{SACM_NS}\" xmlns:xmi=\"http://www.omg.org/XMI\" xmi:id=\"p\">\
                 <name content=\"p\"/></sacm:ArgumentPackage>"
            ),
            &[("name", t("xmi:id")), ("row", t("LIB-COMPAT-002"))],
            "the name {name:?} is in the namespace \"http://www.omg.org/XMI\", not the XMI 2.5.1 namespace \"http://www.omg.org/spec/XMI/20131001\"",
            "the reader cannot represent the input at line 1, column 109: the name \"xmi:id\" is in the namespace \"http://www.omg.org/XMI\", not the XMI 2.5.1 namespace \"http://www.omg.org/spec/XMI/20131001\" (strict mode never reads it; LIB-COMPAT-002 reads it in tolerant mode)",
        ),
        // --- WriteError::Invalid, SACM-XMI-016 -----------------------------------------
        write(
            "SACM-XMI-016/schema-error",
            tolerant_document(&package(&dangling("c1"))),
            &[("first_code", t("SACM-REF-001")), ("first_id", t("c1"))],
            "the document cannot be written in strict mode: 1 schema error, {first_code} on {first_id:?}",
            "the document cannot be written in strict mode: 1 schema error, SACM-REF-001 on \"c1\"",
        ),
        write(
            "SACM-XMI-016/schema-errors",
            tolerant_document(&package(&format!("{}{}", dangling("c1"), dangling("c2")))),
            &[
                ("count", n(2)),
                ("first_code", t("SACM-REF-001")),
                ("first_id", t("c1")),
            ],
            "the document cannot be written in strict mode: {count} schema errors, the first is {first_code} on {first_id:?}",
            "the document cannot be written in strict mode: 2 schema errors, the first is SACM-REF-001 on \"c1\"",
        ),
        // --- ReadError::DuplicateId, InvalidId -----------------------------------------
        read(
            "SACM-ID-001/duplicate-id",
            package(&format!("{}{}", claim("c1", "", ""), claim("c1", "", ""))),
            &[("id", t("c1")), ("count", n(2))],
            "the xmi:id {id:?} is carried by {count} elements",
            "the xmi:id \"c1\" is carried by 2 elements, at line 1, column 153; line 1, column 242",
        ),
        read(
            "SACM-ID-002/read-id-not-an-ncname",
            package("<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"\"/>"),
            &[("id", t(""))],
            "the xmi:id {id:?} is not an NCName",
            "the xmi:id \"\" is not an NCName at line 1, column 153",
        ),
        // --- ReadError::RepeatedValue, InvalidLiteral ----------------------------------
        read(
            "SACM-MULT-001/both-forms",
            package(
                "<argumentElement xmi:type=\"sacm:AssertedInference\" xmi:id=\"i1\" source=\"c2 c3\">\
                 <source xmi:idref=\"c4\"/></argumentElement>",
            ),
            &[("property", t("source")), ("element", t("i1"))],
            "the property {property:?} of the element {element:?} is given both as an attribute and as elements, which XMI 2.5.1 rule 2h forbids",
            "the property \"source\" of the element \"i1\" is given both as an attribute and as elements, which XMI 2.5.1 rule 2h forbids, at line 1, column 216; line 1, column 231",
        ),
        read(
            "SACM-MULT-001/repeated-value",
            format!(
                "<sacm:ArgumentPackage{NS} xmi:id=\"arguments\"><name content=\"A\"><content>A</content></name></sacm:ArgumentPackage>"
            ),
            &[
                ("property", t("name.content")),
                ("element", t("arguments")),
                ("count", n(2)),
            ],
            "the single-valued property {property:?} of the element {element:?} is given {count} times",
            "the single-valued property \"name.content\" of the element \"arguments\" is given 2 times, at line 1, column 148; line 1, column 160",
        ),
        read(
            "SACM-ENUM-001/not-a-literal",
            package(&claim("c1", " assertionDeclaration=\"Assumed\"", "")),
            &[
                ("value", t("Assumed")),
                ("property", t("assertionDeclaration")),
                ("element", t("c1")),
            ],
            "the value {value:?} of the property {property:?} of the element {element:?} is not a literal",
            "the value \"Assumed\" of the property \"assertionDeclaration\" of the element \"c1\" is not a literal at line 1, column 204: the literals are asserted, needsSupport, assumed, axiomatic, defeated and asCited",
        ),
        // --- ReadError::Malformed, SACM-XML-001 ----------------------------------------
        read(
            "SACM-XML-001/invalid-utf8",
            &b"<r>\xFF</r>"[..],
            &[],
            "a byte sequence is not valid UTF-8",
            "the input is not well-formed XML at line 1, column 4: a byte sequence is not valid UTF-8",
        ),
        read(
            "SACM-XML-001/unpaired-surrogate",
            {
                let mut bytes = utf16le("<r>", true);
                bytes.extend([0x00, 0xD8]);
                bytes.extend(utf16le("</r>", false));
                bytes
            },
            &[("unit", n(0xD800))],
            "the UTF-16 code unit {unit:04X} is an unpaired surrogate",
            "the input is not well-formed XML at line 1, column 4: the UTF-16 code unit D800 is an unpaired surrogate",
        ),
        read(
            "SACM-XML-001/odd-utf16-byte",
            {
                let mut bytes = utf16le("<r/>", true);
                bytes.push(0x20);
                bytes
            },
            &[],
            "the UTF-16 input ends with a byte that is not a whole code unit",
            "the input is not well-formed XML at line 1, column 5: the UTF-16 input ends with a byte that is not a whole code unit",
        ),
        read(
            "SACM-XML-001/encoding-contradicts-bom",
            utf16le("<?xml version=\"1.0\" encoding=\"UTF-8\"?><r/>", true),
            &[("encoding", t("UTF-8"))],
            "the declared encoding {encoding:?} contradicts the UTF-16 byte order mark",
            "the input is not well-formed XML at line 1, column 31: the declared encoding \"UTF-8\" contradicts the UTF-16 byte order mark",
        ),
        // ISO-8859-1 after a UTF-8 byte order mark (slice 0072, LIB-COMPAT-016).
        read(
            "SACM-XML-001/encoding-contradicts-utf8-bom",
            &b"\xEF\xBB\xBF<?xml version=\"1.0\" encoding=\"ISO-8859-1\"?><r/>"[..],
            &[("encoding", t("ISO-8859-1"))],
            "the declared encoding {encoding:?} contradicts the UTF-8 byte order mark",
            "the input is not well-formed XML at line 1, column 31: the declared encoding \"ISO-8859-1\" contradicts the UTF-8 byte order mark",
        ),
        read(
            "SACM-XML-001/non-ascii-character",
            "<?xml version=\"1.0\" encoding=\"US-ASCII\"?><r>\u{E9}</r>",
            &[("encoding", t("US-ASCII")), ("code_point", n(0xE9))],
            "the declared encoding is {encoding:?}, but the input holds the non-ASCII character U+{code_point:04X}",
            "the input is not well-formed XML at line 1, column 45: the declared encoding is \"US-ASCII\", but the input holds the non-ASCII character U+00E9",
        ),
        read(
            "SACM-XML-001/xml-version",
            "<?xml version=\"2.0\"?><r/>",
            &[("version", t("2.0"))],
            "the XML version {version:?} is not \"1.\" followed by digits",
            "the input is not well-formed XML at line 1, column 16: the XML version \"2.0\" is not \"1.\" followed by digits",
        ),
        read(
            "SACM-XML-001/encoding-name",
            "<?xml version=\"1.0\" encoding=\"8bit\"?><r/>",
            &[("encoding", t("8bit"))],
            "the encoding name {encoding:?} is not a valid encoding name",
            "the input is not well-formed XML at line 1, column 31: the encoding name \"8bit\" is not a valid encoding name",
        ),
        read(
            "SACM-XML-001/standalone",
            "<?xml version=\"1.0\" standalone=\"maybe\"?><r/>",
            &[("value", t("maybe"))],
            "the standalone value {value:?} is not \"yes\" or \"no\"",
            "the input is not well-formed XML at line 1, column 33: the standalone value \"maybe\" is not \"yes\" or \"no\"",
        ),
        read(
            "SACM-XML-001/unknown-pseudo-attribute",
            "<?xml version=\"1.0\" foo=\"bar\"?><r/>",
            &[("name", t("foo"))],
            "the XML declaration has the unknown pseudo-attribute {name:?}",
            "the input is not well-formed XML at line 1, column 21: the XML declaration has the unknown pseudo-attribute \"foo\"",
        ),
        read(
            "SACM-XML-001/pseudo-attribute-order",
            "<?xml version=\"1.0\" standalone=\"yes\" encoding=\"UTF-8\"?><r/>",
            &[("name", t("encoding"))],
            "the pseudo-attribute {name:?} is out of order in the XML declaration",
            "the input is not well-formed XML at line 1, column 38: the pseudo-attribute \"encoding\" is out of order in the XML declaration",
        ),
        read(
            "SACM-XML-001/pseudo-attribute-space",
            "<?xml version=\"1.0\"encoding=\"UTF-8\"?><r/>",
            &[("name", t("encoding"))],
            "the pseudo-attribute {name:?} does not follow white space in the XML declaration",
            "the input is not well-formed XML at line 1, column 20: the pseudo-attribute \"encoding\" does not follow white space in the XML declaration",
        ),
        read(
            "SACM-XML-001/declaration-character",
            "<?xml ?><r/>",
            &[("expected", p("\"version\""))],
            "an unexpected character in the XML declaration where {expected} was expected",
            "the input is not well-formed XML at line 1, column 7: an unexpected character in the XML declaration where \"version\" was expected",
        ),
        read(
            "SACM-XML-001/declaration-end",
            "<?xml version=\"1.0\"",
            &[],
            "the input ends inside the XML declaration",
            "the input is not well-formed XML at line 1, column 20: the input ends inside the XML declaration",
        ),
        read(
            "SACM-XML-001/reserved-pi-target",
            "<r><?Xml?></r>",
            &[("target", t("Xml"))],
            "a processing instruction has the reserved target {target:?}",
            "the input is not well-formed XML at line 1, column 4: a processing instruction has the reserved target \"Xml\"",
        ),
        read(
            "SACM-XML-001/pi-target-colon",
            "<r><?a:b?></r>",
            &[("target", t("a:b"))],
            "the processing instruction target {target:?} contains a colon",
            "the input is not well-formed XML at line 1, column 4: the processing instruction target \"a:b\" contains a colon",
        ),
        read(
            "SACM-XML-001/pi-target-end",
            "<r><?pi\u{A0}x?></r>",
            &[("target", t("pi"))],
            "the processing instruction target {target:?} is not followed by white space or \"?>\"",
            "the input is not well-formed XML at line 1, column 8: the processing instruction target \"pi\" is not followed by white space or \"?>\"",
        ),
        read(
            "SACM-XML-001/empty-prefix",
            "<r :a=\"1\"/>",
            &[("name", t(":a"))],
            "the name {name:?} has an empty namespace prefix",
            "the input is not well-formed XML at line 1, column 4: the name \":a\" has an empty namespace prefix",
        ),
        read(
            "SACM-XML-001/xmlns-prefix-declared",
            "<r xmlns:xmlns=\"x\"/>",
            &[],
            "the namespace prefix \"xmlns\" is declared, and it cannot be declared",
            "the input is not well-formed XML at line 1, column 4: the namespace prefix \"xmlns\" is declared, and it cannot be declared",
        ),
        read(
            "SACM-XML-001/prefix-undeclared",
            "<r xmlns:p=\"\"/>",
            &[("prefix", t("p"))],
            "the namespace prefix {prefix:?} is declared with an empty value, and a prefix cannot be undeclared",
            "the input is not well-formed XML at line 1, column 4: the namespace prefix \"p\" is declared with an empty value, and a prefix cannot be undeclared",
        ),
        read(
            "SACM-XML-001/reference-to-surrogate",
            "<r a=\"&#xD800;\"/>",
            &[("reference", t("&#xD800;"))],
            "the character reference {reference:?} refers to a surrogate, which is not a character",
            "the input is not well-formed XML at line 1, column 7: the character reference \"&#xD800;\" refers to a surrogate, which is not a character",
        ),
        read(
            "SACM-XML-001/reference-above-max",
            "<r a=\"&#x110000;\"/>",
            &[("reference", t("&#x110000;"))],
            "the character reference {reference:?} refers to a code point above U+10FFFF, which is not a character",
            "the input is not well-formed XML at line 1, column 7: the character reference \"&#x110000;\" refers to a code point above U+10FFFF, which is not a character",
        ),
        read(
            "SACM-XML-001/no-root",
            "<!-- c -->",
            &[],
            "the document has no root element",
            "the input is not well-formed XML at line 1, column 11: the document has no root element",
        ),
        read(
            "SACM-XML-001/unclosed-root",
            "<r>",
            &[],
            "the root element is not closed",
            "the input is not well-formed XML at line 1, column 4: the root element is not closed",
        ),
        read(
            "SACM-XML-001/unexpected-end",
            "<r a=",
            &[],
            "the input ends before the document is complete",
            "the input is not well-formed XML at line 1, column 6: the input ends before the document is complete",
        ),
        read(
            "SACM-XML-001/xml-prefix-uri",
            "<r xmlns:xml=\"urn:x\"/>",
            &[],
            "the \"xml\" prefix is bound to a namespace other than the XML namespace",
            "the input is not well-formed XML at line 1, column 4: the \"xml\" prefix is bound to a namespace other than the XML namespace",
        ),
        read(
            "SACM-XML-001/xml-uri-prefix",
            "<r xmlns:p=\"http://www.w3.org/XML/1998/namespace\"/>",
            &[],
            "the XML namespace is bound to a prefix other than \"xml\"",
            "the input is not well-formed XML at line 1, column 4: the XML namespace is bound to a prefix other than \"xml\"",
        ),
        read(
            "SACM-XML-001/xmlns-uri",
            "<r xmlns:p=\"http://www.w3.org/2000/xmlns/\"/>",
            &[],
            "the \"xmlns\" namespace is declared, which is not allowed",
            "the input is not well-formed XML at line 1, column 4: the \"xmlns\" namespace is declared, which is not allowed",
        ),
        read(
            "SACM-XML-001/xmlns-element-prefix",
            "<xmlns:r/>",
            &[],
            "an element name has the prefix \"xmlns\"",
            "the input is not well-formed XML at line 1, column 2: an element name has the prefix \"xmlns\"",
        ),
        read(
            "SACM-XML-001/duplicate-prefix",
            "<r xmlns:p=\"a\" xmlns:p=\"b\"/>",
            &[("prefix", t("p"))],
            "the namespace prefix {prefix:?} is declared twice on one element",
            "the input is not well-formed XML at line 1, column 16: the namespace prefix \"p\" is declared twice on one element",
        ),
        read(
            "SACM-XML-001/unknown-prefix",
            "<p:r/>",
            &[("prefix", t("p"))],
            "the namespace prefix {prefix:?} is not declared",
            "the input is not well-formed XML at line 1, column 2: the namespace prefix \"p\" is not declared",
        ),
        read(
            "SACM-XML-001/end-tag-mismatch",
            "<r></s>",
            &[("end_tag", t("s")), ("start_tag", t("r"))],
            "the end tag {end_tag:?} does not match the start tag {start_tag:?}",
            "the input is not well-formed XML at line 1, column 4: the end tag \"s\" does not match the start tag \"r\"",
        ),
        read(
            "SACM-XML-001/unknown-entity",
            "<r>&foo;</r>",
            &[("entity", t("foo"))],
            "the entity {entity:?} is not declared",
            "the input is not well-formed XML at line 1, column 4: the entity \"foo\" is not declared",
        ),
        read(
            "SACM-XML-001/malformed-reference",
            "<r>& </r>",
            &[],
            "a \"&\" does not start an entity or character reference",
            "the input is not well-formed XML at line 1, column 4: a \"&\" does not start an entity or character reference",
        ),
        read(
            "SACM-XML-001/duplicate-attribute",
            "<r a=\"1\" a=\"2\"/>",
            &[("attribute", t("a"))],
            "an element has the attribute {attribute:?} more than once",
            "the input is not well-formed XML at line 1, column 10: an element has the attribute \"a\" more than once",
        ),
        read(
            "SACM-XML-001/misplaced-declaration",
            "<r/><?xml version=\"1.0\"?>",
            &[],
            "an XML declaration is not at the start of the document",
            "the input is not well-formed XML at line 1, column 5: an XML declaration is not at the start of the document",
        ),
        read(
            "SACM-XML-001/invalid-name",
            "<r><1/></r>",
            &[],
            "a name is not valid",
            "the input is not well-formed XML at line 1, column 5: a name is not valid",
        ),
        read(
            "SACM-XML-001/non-xml-character",
            "<r>\u{1}</r>",
            &[("code_point", n(1))],
            "the character U+{code_point:04X} is not allowed in XML",
            "the input is not well-formed XML at line 1, column 4: the character U+0001 is not allowed in XML",
        ),
        read(
            "SACM-XML-001/unexpected-character",
            "<r a/>",
            &[("expected", p("\"=\""))],
            "an unexpected character where {expected} was expected",
            "the input is not well-formed XML at line 1, column 5: an unexpected character where \"=\" was expected",
        ),
        read(
            "SACM-XML-001/expected-string",
            "<r><!--",
            &[("expected", t("-->"))],
            "{expected:?} was expected",
            "the input is not well-formed XML at line 1, column 8: \"-->\" was expected",
        ),
        read(
            "SACM-XML-001/invalid-comment",
            "<r><!-- a -- b --></r>",
            &[],
            "a comment holds \"--\" or ends with \"-\"",
            "the input is not well-formed XML at line 1, column 4: a comment holds \"--\" or ends with \"-\"",
        ),
        read(
            "SACM-XML-001/cdata-end-in-text",
            "<r>]]></r>",
            &[],
            "character data holds \"]]>\"",
            "the input is not well-formed XML at line 1, column 7: character data holds \"]]>\"",
        ),
        read(
            "SACM-XML-001/unknown-token",
            "<r/>x",
            &[],
            "the input has markup or text that is not allowed here",
            "the input is not well-formed XML at line 1, column 5: the input has markup or text that is not allowed here",
        ),
    ];
    cases.extend(limit_cases());
    cases
}

/// A root `r` with 65,536 children, each declaring its own prefix `pN`: one
/// more distinct binding than roxmltree's namespace table holds with `xml`
/// (LIB-SEC-003, acceptance 8).
fn many_prefixes() -> Vec<u8> {
    let mut input = String::from("<r>");
    for n in 0..65_536 {
        let _ = write!(input, "<x xmlns:p{n}=\"u\"/>");
    }
    input.push_str("</r>");
    input.into_bytes()
}

/// A root ArgumentPackage `p` that binds `v` to a 200-character URI and
/// holds one preserved `<v:x/>` at line 1, column 345: 373 bytes, which keep
/// 411 bytes of namespace text, the binding (1 + 200 + 10) and the name's
/// URI (200) (ADR 0041 D2; the design brief of slice 0072).
fn kept_namespaces() -> Vec<u8> {
    let uri = format!("urn:{}", "v".repeat(196));
    let input = format!(
        "<sacm:ArgumentPackage{NS} xmlns:v=\"{uri}\" xmi:id=\"p\"><v:x/></sacm:ArgumentPackage>"
    );
    assert_eq!(
        (uri.len(), input.len(), input.find("<v:x").map(|at| at + 1)),
        (200, 373, Some(345)),
        "LIB-SEC-003: setup: the kept-namespaces input"
    );
    input.into_bytes()
}

/// A root ArgumentPackage whose `xmi:id` is 200 `p`, holding one preserved
/// `<x/>` at line 1, column 333: 359 bytes. The `x` keeps no namespace text,
/// and its SACM-XMI-003 Error keeps 416 bytes of text: `x` (1),
/// `ArgumentPackage` (15), the id (200) and the affected id (200) (ADR 0041
/// amendment 1).
fn kept_text() -> Vec<u8> {
    let id = "p".repeat(200);
    let input = format!("<sacm:ArgumentPackage{NS} xmi:id=\"{id}\"><x/></sacm:ArgumentPackage>");
    assert_eq!(
        (input.len(), input.find("<x").map(|at| at + 1)),
        (359, Some(333)),
        "LIB-SEC-003: setup: the kept-text input"
    );
    input.into_bytes()
}

/// The eight `SACM-SEC-003` templates of `ReadError::LimitExceeded` (slice
/// 0072, LIB-SEC-003; `/attributes`, slice 0077, LIB-SEC-007), each read
/// strictly under the limits set for it.
fn limit_cases() -> Vec<Case> {
    use sacm::xmi::Limits;
    let under = |template: &'static str,
                 input: &[u8],
                 limits: Limits,
                 arguments: &[(&'static str, Value)],
                 english: &'static str,
                 display: &'static str| Case {
        template,
        input: Input::ReadUnder(input.to_vec(), limits),
        arguments: arguments.to_vec(),
        english,
        display,
    };
    let declaring = b"<r xmlns:a=\"u\"><x xmlns:b=\"v\"/></r>";
    vec![
        under(
            "SACM-SEC-003/input-bytes",
            b"<r/>",
            Limits::default().with_max_input_bytes(3),
            &[("size", n(4)), ("max", n(3))],
            "the input is {size} bytes long, over the size limit of {max} bytes",
            "the input is 4 bytes long, over the size limit of 3 bytes",
        ),
        under(
            "SACM-SEC-003/kept-namespaces",
            &kept_namespaces(),
            Limits::default().with_max_input_bytes(373),
            &[("max", n(373))],
            "the namespace names and bindings that preserved content keeps come to more than the size limit of {max} bytes",
            "the namespace names and bindings that preserved content keeps come to more than the size limit of 373 bytes at line 1, column 345",
        ),
        under(
            "SACM-SEC-003/kept-text",
            &kept_text(),
            Limits::default().with_max_input_bytes(359),
            &[("max", n(359))],
            "the namespace text and diagnostics that the read keeps come to more than the size limit of {max} bytes",
            "the namespace text and diagnostics that the read keeps come to more than the size limit of 359 bytes at line 1, column 333",
        ),
        under(
            "SACM-SEC-003/depth",
            b"<a><b/></a>",
            Limits::default().with_max_depth(1),
            &[("max", n(1))],
            "an element is nested deeper than the depth limit of {max}",
            "an element is nested deeper than the depth limit of 1 at line 1, column 4",
        ),
        under(
            "SACM-SEC-003/nodes",
            b"<r><x/></r>",
            Limits::default().with_max_nodes(2),
            &[("max", n(2))],
            "the input has more XML nodes than the node limit of {max}",
            "the input has more XML nodes than the node limit of 2",
        ),
        under(
            "SACM-SEC-003/namespace-copies",
            declaring,
            Limits::default().with_max_nodes(2),
            &[("max", n(2))],
            "the elements that declare namespaces have more bindings in scope in all than the node limit of {max}",
            "the elements that declare namespaces have more bindings in scope in all than the node limit of 2 at line 1, column 16",
        ),
        under(
            "SACM-SEC-003/namespace-bindings",
            declaring,
            Limits::default().with_max_namespace_bindings(1),
            &[("max", n(1))],
            "an element has more namespace bindings in scope than the limit of {max}",
            "an element has more namespace bindings in scope than the limit of 1 at line 1, column 16",
        ),
        under(
            "SACM-SEC-003/attributes",
            b"<a b=\"1\" c=\"2\"/>",
            Limits::default().with_max_attributes(1),
            &[("max", n(1))],
            "an element has more attributes than the limit of {max}",
            "an element has more attributes than the limit of 1 at line 1, column 1",
        ),
    ]
}

// --- Running a case ------------------------------------------------------------------

/// A refusal of either kind.
enum Refusal {
    Read(ReadError),
    Write(WriteError),
}

impl Refusal {
    fn display(&self) -> String {
        match self {
            Self::Read(error) => error.to_string(),
            Self::Write(error) => error.to_string(),
        }
    }
}

/// The refusal of `case`'s input: a strict read, or a strict write.
fn refusal(case: &Case) -> Refusal {
    let template = case.template;
    match &case.input {
        Input::Read(bytes) => {
            match xmi::read(bytes, &ReadOptions::default().with_mode(Mode::Strict)) {
                Ok(output) => panic!(
                    "{ROW}: {template}: a strict read accepts the input, with {:?}",
                    output.diagnostics()
                ),
                Err(error) => Refusal::Read(error),
            }
        }
        Input::ReadUnder(bytes, limits) => {
            let options = ReadOptions::default()
                .with_mode(Mode::Strict)
                .with_limits(*limits);
            match xmi::read(bytes, &options) {
                Ok(output) => panic!(
                    "{ROW}: {template}: a strict read under {limits:?} accepts the input, with {:?}",
                    output.diagnostics()
                ),
                Err(error) => Refusal::Read(error),
            }
        }
        Input::Write(document) => match xmi::write(document, &WriteOptions::default()) {
            Ok(text) => panic!("{ROW}: {template}: a strict write writes the document: {text}"),
            Err(error) => Refusal::Write(error),
        },
    }
}

/// The code text of `template`: the part before the `/`.
fn code_of(template: &str) -> &str {
    template.split_once('/').map_or(template, |(code, _)| code)
}

/// The placeholders of `english` in order, with their format: `""`, `":?"` or
/// `":04X"`.
fn placeholders(english: &str) -> Vec<(&str, &str)> {
    let mut out = Vec::new();
    let mut rest = english;
    while let Some(open) = rest.find('{') {
        let after = &rest[open + 1..];
        let Some(close) = after.find('}') else {
            break;
        };
        let inner = &after[..close];
        let (name, format) = inner.find(':').map_or((inner, ""), |at| inner.split_at(at));
        out.push((name, format));
        rest = &after[close + 1..];
    }
    out
}

/// `english` with each placeholder replaced by its argument: Text or Prose
/// as it is for `{n}`, Text Debug-quoted for `{n:?}`, a Count as at least four uppercase
/// hex digits for `{n:04X}` and in decimal for `{n}`.
fn render(template: &str, english: &str, arguments: &[(&str, Value)]) -> String {
    let mut out = String::new();
    let mut rest = english;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let Some(close) = after.find('}') else {
            panic!("{ROW}: {template}: an unclosed placeholder in {english:?}");
        };
        let inner = &after[..close];
        let (name, format) = inner.find(':').map_or((inner, ""), |at| inner.split_at(at));
        let Some(&(_, value)) = arguments.iter().find(|(given, _)| *given == name) else {
            panic!("{ROW}: {template}: the placeholder {name:?} names no argument");
        };
        match (format, value) {
            ("", Value::Text(text) | Value::Prose(text)) => out.push_str(text),
            (":?", Value::Text(text)) => {
                let _ = write!(out, "{text:?}");
            }
            ("", Value::Count(count)) => out.push_str(&count.to_string()),
            (":04X", Value::Count(count)) => {
                let _ = write!(out, "{count:04X}");
            }
            _ => panic!("{ROW}: {template}: {{{inner}}} does not take {value:?}"),
        }
        rest = &after[close + 1..];
    }
    out.push_str(rest);
    out
}

/// The label an `Unsupported` refusal of `code` ends with, if its code is
/// one of an `Unsupported` refusal, given its `row` argument.
fn label(code: &str, row: Option<&str>) -> Option<String> {
    match (code, row) {
        // Since slice 0072 a tolerant read reads them (ADR 0041 D6).
        // Since slice 0075a SACM-XMI-015 with a row is read too (ADR 0044 D1).
        ("SACM-XMI-002" | "SACM-XMI-008" | "SACM-XMI-015", Some(row)) => Some(format!(
            "(strict mode never reads it; {row} reads it in tolerant mode)"
        )),
        // SACM-XMI-008 without a row: a SACM 2.4 root; SACM-XMI-015 without
        // one: an `XMI` element in neither XMI namespace.
        ("SACM-XML-002" | "SACM-SEC-002" | "SACM-XMI-008" | "SACM-XMI-015", None) => {
            Some("(a limit of this version)".to_owned())
        }
        ("SACM-XMI-003", None) => Some("(a defect in the input)".to_owned()),
        _ => None,
    }
}

/// Whether `template`'s refusal is `ReadError::Unsupported`: its code is one
/// only `Unsupported` carries.
fn is_unsupported(template: &str) -> bool {
    matches!(
        code_of(template),
        "SACM-XMI-002"
            | "SACM-XMI-008"
            | "SACM-XMI-015"
            | "SACM-XML-002"
            | "SACM-SEC-002"
            | "SACM-XMI-003"
    )
}

/// Checks the table row of `case` against itself: the placeholders and the
/// arguments agree in names, kinds and order, and `row` is last exactly on
/// the `Unsupported` templates of SACM-XMI-002 and -015, and on
/// `SACM-XMI-008/other-revision-root`.
fn check_row(case: &Case) {
    let template = case.template;
    let mut first_seen: Vec<&str> = Vec::new();
    for (name, format) in placeholders(case.english) {
        let Some(&(_, value)) = case.arguments.iter().find(|(given, _)| *given == name) else {
            panic!("{ROW}: {template}: the placeholder {name:?} names no argument");
        };
        let fits = matches!(
            (format, value),
            ("" | ":?", Value::Text(_)) | ("" | ":04X", Value::Count(_)) | ("", Value::Prose(_))
        );
        assert!(
            fits,
            "{ROW}: {template}: {{{name}{format}}} does not take {value:?}"
        );
        if !first_seen.contains(&name) {
            first_seen.push(name);
        }
    }
    let declared: Vec<&str> = case.arguments.iter().map(|(name, _)| *name).collect();
    let without_row: Vec<&str> = declared
        .iter()
        .copied()
        .filter(|name| *name != "row")
        .collect();
    assert_eq!(
        without_row, first_seen,
        "{ROW}: {template}: the arguments other than row are not those the pattern names, in the \
         order it first names them"
    );
    // Of SACM-XMI-008, only a root of an older revision names its row; a
    // SACM 2.4 root is a limit (slice 0072).
    let takes_row = (matches!(code_of(template), "SACM-XMI-002" | "SACM-XMI-015")
        && template != "SACM-XMI-015/not-xmi-element")
        || template == "SACM-XMI-008/other-revision-root";
    if takes_row {
        assert_eq!(
            declared.last(),
            Some(&"row"),
            "{ROW}: {template}: row is not the last argument"
        );
        assert_eq!(
            declared.iter().filter(|name| **name == "row").count(),
            1,
            "{ROW}: {template}: row is declared more than once"
        );
    } else {
        assert!(
            !declared.contains(&"row"),
            "{ROW}: {template}: only the Unsupported templates of SACM-XMI-002 and -015, and \
             SACM-XMI-008/other-revision-root, take row"
        );
    }
}

#[test]
fn lib_api_017_each_refusal_template_input_gives_its_0_1_0_display() {
    let cases = cases();
    // The inputs and the templates without one are the 104, once each.
    let mut ids: Vec<&str> = cases.iter().map(|case| case.template).collect();
    ids.extend(NO_INPUT.iter().map(|(template, _)| *template));
    let count = ids.len();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(
        (count, ids.len()),
        (REFUSAL_TEMPLATES, REFUSAL_TEMPLATES),
        "{ROW}: the table and NO_INPUT do not hold the {REFUSAL_TEMPLATES} refusal templates \
         once each"
    );
    for case in &cases {
        let template = case.template;
        check_row(case);
        let display = refusal(case).display();
        assert_eq!(
            display, case.display,
            "{ROW}: {template}: the Display is not 0.1.0's"
        );
        let variable = render(template, case.english, &case.arguments);
        assert!(
            display.contains(&variable),
            "{ROW}: {template}: the Display {display:?} does not hold the rendered pattern \
             {variable:?}"
        );
        if is_unsupported(template) {
            let row = case
                .arguments
                .iter()
                .find_map(|&(name, value)| match (name, value) {
                    ("row", Value::Text(row)) => Some(row),
                    _ => None,
                });
            let Some(label) = label(code_of(template), row) else {
                panic!("{ROW}: {template}: no label for its code and row {row:?}");
            };
            assert!(
                display.ends_with(&format!("{variable} {label}")),
                "{ROW}: {template}: the Display {display:?} does not end with the pattern and \
                 the label {label:?}"
            );
        }
    }
}

/// Text in the `value` of a `name` is a C++ shape of its own: a tolerant
/// read reports it, after the `name-value` shape of the `value` itself, with
/// the English of `SACM-XMI-012/name-value-text`, the template no strict
/// read refuses (LIB-API-017, acceptance 4).
#[test]
fn lib_api_017_text_in_the_value_of_a_name_is_reported_with_its_template_s_english() {
    let input = named_package("<name><value>A</value></name>");
    let output = match xmi::read(
        input.as_bytes(),
        &ReadOptions::default().with_mode(Mode::Tolerant),
    ) {
        Ok(output) => output,
        Err(error) => panic!("{ROW}: a tolerant read refuses {input}: {error}"),
    };
    let found: Vec<(Option<(u32, u32)>, &str)> = output
        .diagnostics()
        .iter()
        .filter(|found| found.code() == sacm::diagnostic::Code::XMI_LEGACY_SHAPE)
        .map(|found| {
            (
                found.location().map(|at| (at.line(), at.column())),
                found.message(),
            )
        })
        .collect();
    let column = |what: &str| {
        input
            .find(what)
            .and_then(|offset| u32::try_from(offset + 1).ok())
            .unwrap_or_else(|| panic!("{ROW}: setup: no {what:?} in {input}"))
    };
    assert_eq!(
        (column("<value"), column("A</value>")),
        (140, 147),
        "{ROW}: setup"
    );
    assert_eq!(
        found,
        [
            (
                Some((1, 140)),
                "the name of the ArgumentPackage \"p\" holds a value, a shape of the \
                 assurance-forge C++ library that XMI 2.5.1 does not define; it is read as its \
                 standard form, which write writes",
            ),
            (
                Some((1, 147)),
                "the value in the name of the ArgumentPackage \"p\" has its content as text, a \
                 shape of the assurance-forge C++ library that XMI 2.5.1 does not define; it is \
                 read as its standard form, which write writes",
            ),
        ],
        "{ROW}: the two SACM-XMI-012 Warnings of text in a name's value"
    );
}

mod api {
    use super::{Case, ROW, Refusal, Value, cases, code_of, refusal};
    use sacm::diagnostic::{Argument, ArgumentValue};

    /// The expected arguments of `case`, as the API states them: each Text
    /// an `ArgumentValue::Text`, each Count an `ArgumentValue::Count`, and
    /// each Prose an `ArgumentValue::Prose` (LIB-DIAG-004).
    fn expected(case: &Case) -> Vec<(&'static str, ArgumentValue)> {
        case.arguments
            .iter()
            .map(|&(name, value)| {
                let value = match value {
                    Value::Text(text) => ArgumentValue::Text(text.to_owned()),
                    Value::Count(count) => ArgumentValue::Count(count),
                    Value::Prose(text) => ArgumentValue::Prose(text.to_owned()),
                };
                (name, value)
            })
            .collect()
    }

    fn named(arguments: &[Argument]) -> Vec<(&'static str, ArgumentValue)> {
        arguments
            .iter()
            .map(|argument| (argument.name(), argument.value().clone()))
            .collect()
    }

    #[test]
    fn lib_api_017_each_refusal_template_has_its_code_template_and_arguments() {
        for case in cases() {
            let template = case.template;
            let (code, id, arguments) = match refusal(&case) {
                Refusal::Read(error) => (error.code(), error.template(), named(error.arguments())),
                Refusal::Write(error) => (error.code(), error.template(), named(error.arguments())),
            };
            assert_eq!(
                code.as_str(),
                code_of(template),
                "{ROW}: {template}: code()"
            );
            assert_eq!(id, template, "{ROW}: {template}: template()");
            assert_eq!(
                arguments,
                expected(&case),
                "{ROW}: {template}: arguments() by name, kind and value"
            );
        }
    }
}
