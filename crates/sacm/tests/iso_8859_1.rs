//! ISO-8859-1 input (LIB-COMPAT-016; XML 1.0 Fifth Edition §4.3.3; M6
//! decision Q10; ADR 0041 D7).
//!
//! "All XML processors MUST be able to read entities in both the UTF-8 and
//! UTF-16 encodings", and other encodings are optional; "XML processors
//! SHOULD match character encoding names in a case-insensitive way"; and
//! "it is a fatal error for an entity including an encoding declaration to
//! be presented to the XML processor in an encoding other than that named in
//! the declaration" (§4.3.3). So `xmi::read` reads, in both modes (D-STRICT),
//! input without a byte order mark whose declaration names `ISO-8859-1` in
//! any ASCII letter case, and no alias. Each byte is the scalar value of the
//! same number, and columns count characters. A byte order mark before that
//! declaration contradicts it, so it is `Malformed`. `xmi::write` writes
//! UTF-8.
//!
//! Every input is an inline byte string; Golden 3 of the design brief is
//! [`GOLDEN_3`] and its output [`GOLDEN_3_OUTPUT`]. The other single-byte
//! encodings and the IANA aliases stay `Unsupported` (SACM-XML-002).

use sacm::diagnostic::SourceLocation;
use sacm::model::{Document, LangString, SacmElement};
use sacm::xmi::{self, Mode, ReadError, ReadOptions, ReadOutput, WriteOptions};

const ROW: &str = "LIB-COMPAT-016";

const MODES: [Mode; 2] = [Mode::Strict, Mode::Tolerant];

/// Golden 3, input: an ISO-8859-1 document whose package name holds
/// `0xE9` (é), `0xA9` (©) and `0xFF` (ÿ).
const GOLDEN_3: &[u8] = b"<?xml version=\"1.0\" encoding=\"ISO-8859-1\"?>\n\
<sacm:ArgumentPackage xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\" \
xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" xmi:id=\"p\">\n  \
<name content=\"Caf\xE9 \xA9 \xFF\"/>\n</sacm:ArgumentPackage>\n";

/// Golden 3, output: what both write modes write for it, in UTF-8.
const GOLDEN_3_OUTPUT: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<sacm:ArgumentPackage xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\" \
xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" xmi:id=\"p\">\n  \
<name content=\"Caf\u{E9} \u{A9} \u{FF}\" />\n</sacm:ArgumentPackage>\n";

/// The start tag of a root ArgumentPackage `p`.
const ROOT_START: &str = "<sacm:ArgumentPackage xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\" \
xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" xmi:id=\"p\">";

/// The declaration naming `name`, which puts the name at line 1, column 31.
fn declaration(name: &str) -> String {
    format!("<?xml version=\"1.0\" encoding=\"{name}\"?>")
}

/// `declaration(name)`, a line feed and a root ArgumentPackage `p` whose name
/// content is `content`, as bytes.
fn document(name: &str, content: &[u8]) -> Vec<u8> {
    let mut bytes = format!("{}\n{ROOT_START}<name content=\"", declaration(name)).into_bytes();
    bytes.extend_from_slice(content);
    bytes.extend_from_slice(b"\"/></sacm:ArgumentPackage>");
    bytes
}

fn options(mode: Mode) -> ReadOptions {
    ReadOptions::default().with_mode(mode)
}

fn loc(line: u32, column: u32) -> SourceLocation {
    SourceLocation::new(line, column)
        .unwrap_or_else(|| panic!("{ROW}: setup: ({line}, {column}) is no location"))
}

/// The read of `input` in `mode`, which must succeed with no diagnostic.
fn read_clean(what: &str, input: &[u8], mode: Mode) -> ReadOutput {
    let output = match xmi::read(input, &options(mode)) {
        Ok(output) => output,
        Err(error) => panic!("{ROW}: {what}: a {mode:?} read refuses it: {error:?} ({error})"),
    };
    assert!(
        output.diagnostics().is_empty(),
        "{ROW}: {what}: a {mode:?} read reports {:?}",
        output.diagnostics()
    );
    output
}

/// The refusal of `input` in `mode`.
fn refusal(what: &str, input: &[u8], mode: Mode) -> ReadError {
    match xmi::read(input, &options(mode)) {
        Ok(output) => panic!(
            "{ROW}: {what}: a {mode:?} read returns a document, with {:?}",
            output.diagnostics()
        ),
        Err(error) => error,
    }
}

/// Asserts that `input` is `Malformed` in both modes alike, with
/// `template` when one is given, at `location`, and returns the refusal.
fn assert_malformed(
    what: &str,
    input: &[u8],
    template: Option<&str>,
    location: SourceLocation,
) -> ReadError {
    let strict = refusal(what, input, Mode::Strict);
    let tolerant = refusal(what, input, Mode::Tolerant);
    assert_eq!(
        tolerant, strict,
        "{ROW}: {what}: the modes refuse differently"
    );
    assert!(
        matches!(strict, ReadError::Malformed { .. }),
        "{ROW}: {what}: not Malformed: {strict:?}"
    );
    assert_eq!(
        (strict.code().as_str(), strict.location()),
        ("SACM-XML-001", Some(location)),
        "{ROW}: {what}: the code and location of {strict}"
    );
    if let Some(template) = template {
        assert_eq!(
            strict.template(),
            template,
            "{ROW}: {what}: the template of {strict}"
        );
    }
    strict
}

/// The content of the name of `p` in `document`.
fn name_of_p(document: &Document) -> Option<String> {
    document
        .get("p")
        .and_then(SacmElement::name)
        .and_then(LangString::content)
        .map(str::to_owned)
}

#[test]
fn lib_compat_016_reads_each_byte_as_its_code_point_in_both_modes() {
    let mut documents = Vec::new();
    for mode in MODES {
        let document = read_clean("Golden 3", GOLDEN_3, mode).into_document();
        assert_eq!(
            name_of_p(&document).as_deref(),
            Some("Caf\u{E9} \u{A9} \u{FF}"),
            "{ROW}: a {mode:?} read does not give the name Café © ÿ"
        );
        documents.push(document);
    }
    assert!(
        documents[0] == documents[1],
        "{ROW}: the modes read Golden 3 differently"
    );
}

#[test]
fn lib_compat_016_the_name_is_compared_case_insensitively() {
    let content = b"Caf\xE9";
    for mode in MODES {
        let documents: Vec<Document> = ["ISO-8859-1", "iso-8859-1", "Iso-8859-1"]
            .iter()
            .map(|name| read_clean(name, &document(name, content), mode).into_document())
            .collect();
        assert_eq!(
            name_of_p(&documents[0]).as_deref(),
            Some("Caf\u{E9}"),
            "{ROW}: a {mode:?} read of ISO-8859-1 does not decode 0xE9 as é"
        );
        assert!(
            documents[1] == documents[0] && documents[2] == documents[0],
            "{ROW}: in {mode:?} mode, iso-8859-1 and Iso-8859-1 do not read as ISO-8859-1"
        );
    }
}

#[test]
fn lib_compat_016_aliases_and_other_encodings_stay_unsupported() {
    let names = [
        "latin1",
        "l1",
        "ISO_8859-1",
        "IBM819",
        "CP819",
        "ISO-8859-2",
        "ISO-8859-15",
        "windows-1252",
    ];
    for name in names {
        // ASCII bytes, so the refusal is the encoding's, not a decoding error.
        let input = document(name, b"Cafe");
        for mode in MODES {
            let error = refusal(name, &input, mode);
            assert!(
                matches!(error, ReadError::Unsupported { .. }),
                "{ROW}: {name}: a {mode:?} read does not refuse it as Unsupported: {error:?}"
            );
            assert_eq!(
                (error.code().as_str(), error.template(), error.location()),
                (
                    "SACM-XML-002",
                    "SACM-XML-002/declared-encoding",
                    Some(loc(1, 31))
                ),
                "{ROW}: {name}: the code, template and location in {mode:?} mode"
            );
            let shown = error.to_string();
            assert!(
                shown.ends_with(
                    "only UTF-8, ASCII and ISO-8859-1 are read (a limit of this version)"
                ),
                "{ROW}: {name}: the Display does not name ISO-8859-1 among the encodings read: \
                 {shown}"
            );
        }
    }
}

#[test]
fn lib_compat_016_locations_count_characters() {
    // `<r>éé</s>`: the end tag's `<` is the sixth character of line 2, and
    // the eighth byte once é is UTF-8.
    let mut input = declaration("ISO-8859-1").into_bytes();
    input.extend_from_slice(b"\n<r>\xE9\xE9</s>");
    assert_malformed(
        "an end-tag mismatch after two é",
        &input,
        Some("SACM-XML-001/end-tag-mismatch"),
        loc(2, 6),
    );
}

#[test]
fn lib_compat_016_a_utf_8_byte_order_mark_contradicts_it() {
    let mut input = vec![0xEF, 0xBB, 0xBF];
    input.extend_from_slice(&document("ISO-8859-1", b"Cafe"));
    let error = assert_malformed(
        "a UTF-8 byte order mark before ISO-8859-1",
        &input,
        Some("SACM-XML-001/encoding-contradicts-utf8-bom"),
        loc(1, 31),
    );
    assert_eq!(
        error.to_string(),
        "the input is not well-formed XML at line 1, column 31: the declared encoding \
         \"ISO-8859-1\" contradicts the UTF-8 byte order mark",
        "{ROW}: the Display of the contradiction"
    );
    // A UTF-16 mark before it stays Malformed, as SACM23-XMI-018 states.
    let text = format!(
        "{}\n{ROOT_START}</sacm:ArgumentPackage>",
        declaration("ISO-8859-1")
    );
    let mut utf16 = vec![0xFF, 0xFE];
    utf16.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
    assert_malformed(
        "a UTF-16 byte order mark before ISO-8859-1",
        &utf16,
        Some("SACM-XML-001/encoding-contradicts-bom"),
        loc(1, 31),
    );
}

#[test]
fn lib_compat_016_c1_bytes_read_and_non_characters_are_malformed() {
    // 0x80 to 0x9F are the C1 controls, which XML 1.0's Char allows.
    let c1: Vec<u8> = (0x80..=0x9F).collect();
    let expected: String = (0x80..=0x9F_u32).filter_map(char::from_u32).collect();
    for mode in MODES {
        let document = read_clean("C1 bytes", &document("ISO-8859-1", &c1), mode).into_document();
        assert_eq!(
            name_of_p(&document),
            Some(expected.clone()),
            "{ROW}: a {mode:?} read does not decode 0x80..0x9F as U+0080..U+009F"
        );
    }
    // 0x01 is U+0001, which is no XML Char, at its column on line 2.
    let input = document("ISO-8859-1", b"a\x01b");
    let line_2 = format!("{ROOT_START}<name content=\"a");
    let column = u32::try_from(line_2.chars().count() + 1).unwrap_or(u32::MAX);
    assert_malformed(
        "a 0x01 byte",
        &input,
        Some("SACM-XML-001/non-xml-character"),
        loc(2, column),
    );
}

#[test]
fn lib_compat_016_the_declaration_is_checked_on_the_decoded_text() {
    // An é after the encoding is an unknown pseudo-attribute `é`, at its
    // character, column 43 (the design brief and the matrix said 44, off by one: the
    // declaration up to the closing quote of the encoding is 41 characters).
    let mut input = b"<?xml version=\"1.0\" encoding=\"ISO-8859-1\" \xE9?>\n".to_vec();
    input.extend_from_slice(ROOT_START.as_bytes());
    input.extend_from_slice(b"</sacm:ArgumentPackage>");
    assert_malformed(
        "an é in the declaration after ISO-8859-1",
        &input,
        None,
        loc(1, 43),
    );
    // Before the encoding name the encoding is not known yet: invalid UTF-8.
    let mut input = b"<?xml version=\"1.\xE9\" encoding=\"ISO-8859-1\"?>\n".to_vec();
    input.extend_from_slice(ROOT_START.as_bytes());
    input.extend_from_slice(b"</sacm:ArgumentPackage>");
    assert_malformed(
        "an é in the version, before the encoding",
        &input,
        Some("SACM-XML-001/invalid-utf8"),
        loc(1, 18),
    );
}

#[test]
fn lib_compat_016_writes_utf_8_and_reads_back_equal() {
    let document = read_clean("Golden 3", GOLDEN_3, Mode::Strict).into_document();
    for mode in MODES {
        let written = match xmi::write(&document, &WriteOptions::default().with_mode(mode)) {
            Ok(written) => written,
            Err(error) => panic!("{ROW}: a {mode:?} write refuses Golden 3: {error}"),
        };
        assert_eq!(
            written, GOLDEN_3_OUTPUT,
            "{ROW}: a {mode:?} write of Golden 3 is not its UTF-8 output"
        );
        let back =
            read_clean("the output of Golden 3", written.as_bytes(), Mode::Strict).into_document();
        assert!(
            back == document,
            "{ROW}: the output of a {mode:?} write does not read back to the same document"
        );
    }
}
