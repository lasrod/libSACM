//! Tests for the XML 1.0 and Namespaces 1.0 well-formedness errors that
//! roxmltree accepts, and that `sacm::xmi::read` must refuse as
//! `ReadError::Malformed` (SACM-XML-001) instead of repairing (LIB-SEC-005;
//! ADR 0005 B1, ADR 0016).
//!
//! Every input is an inline byte string. Most put a readable root after the
//! construct under test, so an input that is not refused reads `Ok`, and one
//! that is refused cannot be refused for its root instead.
//!
//! A refusal "at the offending name or value" is expected at the first
//! character of the pseudo-attribute's name, or the first character inside
//! the quotes of its value (as LIB-API-009 locates an encoding name).
//!
//! Not checked here:
//! - Item 5's "the `read` docs state this order": documentation, checked by
//!   the verifier.
//! - Item 6's "the LIB-SEC-002 no-panic inputs still return a value": those
//!   inputs are swept by `xmi_read.rs` (`lib_sec_002_*_never_panic`). This
//!   file sweeps a sample that holds the constructs these checks look at.

use std::error::Error;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::Duration;

use sacm::diagnostic::{Code, SourceLocation};
use sacm::xmi::{self, ReadError, ReadOptions, ReadOutput};

/// The lead of the `Malformed` `Display`, before the location and reason.
const MALFORMED_LEAD: &str = "the input is not well-formed XML";

/// The start tag of a readable ArgumentPackage root, without its `xmi:id`
/// and without its closing `>` or `/>`.
const ROOT_START: &str = concat!(
    "<sacm:ArgumentPackage",
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
);
/// The end tag of [`ROOT_START`].
const ROOT_END: &str = "</sacm:ArgumentPackage>";

/// A complete readable root, with id `p`.
const ROOT: &str = concat!(
    "<sacm:ArgumentPackage",
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
    " xmi:id=\"p\"/>",
);

/// The character references that do not match XML 1.0 [2] Char.
const BAD_REFERENCES: [&str; 5] = [
    "&#xD800;",
    "&#xDFFF;",
    "&#55296;",
    "&#x110000;",
    "&#xFFFFFFFF;",
];

// --- Helpers --------------------------------------------------------------

/// How long one `read` may take. Reading any input here takes milliseconds
/// at most; a scan that stops advancing never finishes. The deadline stays
/// well below cargo-mutants' minimum test timeout of 20 seconds, so a hang
/// fails its test instead of stalling the run (as in xtask's scanner tests).
const DEADLINE: Duration = Duration::from_secs(5);

/// `xmi::read` with the default options, on a worker thread. Fails the test,
/// with a message that starts with `row`, when the read panics or does not
/// return within [`DEADLINE`].
fn read(row: &str, input: &[u8]) -> Result<ReadOutput, ReadError> {
    let owned = input.to_vec();
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        // The receiver is gone only after the deadline has already failed the test.
        let _ = sender.send(xmi::read(&owned, &ReadOptions::default()));
    });
    match receiver.recv_timeout(DEADLINE) {
        Ok(result) => result,
        Err(RecvTimeoutError::Timeout) => panic!(
            "{row}: read did not return within {DEADLINE:?} for {}: a scan does not advance",
            preview(input)
        ),
        Err(RecvTimeoutError::Disconnected) => {
            panic!("{row}: read panicked on {}", preview(input))
        }
    }
}

/// The first bytes of `input`, for assertion messages.
fn preview(input: &[u8]) -> String {
    let end = input.len().min(160);
    let text = String::from_utf8_lossy(&input[..end]);
    if end < input.len() {
        format!("{text:?}... ({} bytes)", input.len())
    } else {
        format!("{text:?}")
    }
}

/// A 1-based location that the test expects `SourceLocation::new` to accept.
fn loc(row: &str, line: u32, column: u32) -> SourceLocation {
    let Some(location) = SourceLocation::new(line, column) else {
        panic!("{row}: SourceLocation::new({line}, {column}) refused a 1-based position");
    };
    location
}

/// Asserts that `input` is refused as `Malformed` (SACM-XML-001) at `line`
/// and `column`, and returns the error.
fn assert_malformed_at(row: &str, input: &[u8], line: u32, column: u32) -> ReadError {
    let Err(error) = read(row, input) else {
        panic!(
            "{row}: read returned a document for {}; expected Malformed",
            preview(input)
        );
    };
    assert!(
        matches!(error, ReadError::Malformed { .. }),
        "{row}: expected Malformed for {}, got {error:?}",
        preview(input)
    );
    assert_eq!(
        error.code(),
        Some(Code::XML_MALFORMED),
        "{row}: the refusal of {} does not carry SACM-XML-001",
        preview(input)
    );
    assert_eq!(
        error.location(),
        Some(loc(row, line, column)),
        "{row}: the refusal of {} is not at line {line}, column {column}: {error}",
        preview(input)
    );
    error
}

/// Asserts that `input` is refused as `Unsupported` (no code) at `line` and
/// `column`, and returns the error.
fn assert_unsupported_at(row: &str, input: &[u8], line: u32, column: u32) -> ReadError {
    let result = read(row, input);
    let Err(error) = result else {
        panic!(
            "{row}: read returned a document for {}; expected Unsupported",
            preview(input)
        );
    };
    assert!(
        matches!(error, ReadError::Unsupported { .. }),
        "{row}: expected Unsupported for {}, got {error:?}",
        preview(input)
    );
    assert_eq!(
        error.code(),
        None,
        "{row}: the Unsupported refusal of {} carries a code",
        preview(input)
    );
    assert_eq!(
        error.location(),
        Some(loc(row, line, column)),
        "{row}: the refusal of {} is not at line {line}, column {column}: {error}",
        preview(input)
    );
    error
}

/// Reads `input`, which the test expects to be read, and returns the ids of
/// its roots.
fn assert_ok_root_ids(row: &str, input: &[u8]) -> Vec<String> {
    match read(row, input) {
        Ok(output) => output
            .document()
            .roots()
            .map(|root| root.id().as_str().to_owned())
            .collect(),
        Err(error) => panic!("{row}: {} was refused: {error:?} ({error})", preview(input)),
    }
}

/// Asserts that `input` is read, as one root with id `id`.
fn assert_ok_with_id(row: &str, input: &[u8], id: &str) {
    assert_eq!(
        assert_ok_root_ids(row, input),
        [id],
        "{row}: {} does not read as one root with id {id:?}",
        preview(input)
    );
}

/// Asserts that `input` is not refused as `Malformed` or `Doctype`.
fn assert_not_malformed(row: &str, input: &[u8]) {
    let result = read(row, input);
    assert!(
        !matches!(
            result,
            Err(ReadError::Malformed { .. } | ReadError::Doctype { .. })
        ),
        "{row}: {} was refused as not well-formed: {result:?}",
        preview(input)
    );
}

/// `declaration`, a line feed and a readable root.
fn declared(declaration: &str) -> Vec<u8> {
    format!("{declaration}\n{ROOT}").into_bytes()
}

/// The line-break characters that `Display` must never contain.
const LINE_BREAKS: [char; 5] = ['\n', '\r', '\u{85}', '\u{2028}', '\u{2029}'];

/// Asserts that `text` is one line that starts in lowercase and ends without
/// punctuation or whitespace.
fn assert_one_lowercase_line(row: &str, text: &str) {
    for line_break in LINE_BREAKS {
        assert!(
            !text.contains(line_break),
            "{row}: Display {text:?} contains the line break {line_break:?}"
        );
    }
    let Some(first) = text.chars().next() else {
        panic!("{row}: Display is empty");
    };
    assert!(
        first.is_lowercase(),
        "{row}: Display {text:?} does not start in lowercase"
    );
    let Some(last) = text.chars().last() else {
        panic!("{row}: Display is empty");
    };
    assert!(
        !last.is_whitespace() && !matches!(last, '.' | ',' | ';' | ':' | '!' | '?'),
        "{row}: Display {text:?} ends with trailing punctuation or whitespace"
    );
}

/// Whether `text` states a position of its own: `L:C`, `line N` or
/// `column N`.
fn states_a_position(text: &str) -> bool {
    let bytes = text.as_bytes();
    let digit_colon_digit = bytes
        .windows(3)
        .any(|w| w[0].is_ascii_digit() && w[1] == b':' && w[2].is_ascii_digit());
    let lower = text.to_lowercase();
    let word_then_digit = ["line ", "column ", "col ", "position ", "offset "]
        .iter()
        .any(|word| {
            lower.match_indices(word).any(|(index, _)| {
                lower[index + word.len()..]
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_digit())
            })
        });
    digit_colon_digit || word_then_digit
}

/// Asserts the LIB-SEC-002 form of a `Malformed` refusal: one lowercase
/// line, `<lead> at <location>: <reason>` (or `<lead>: <reason>`), a reason
/// with no position of its own, every `name` quoted wherever it appears, and
/// no source.
fn assert_malformed_form(row: &str, error: &ReadError, names: &[&str]) {
    let text = error.to_string();
    assert_one_lowercase_line(row, &text);
    let head = match error.location() {
        Some(location) => format!("{MALFORMED_LEAD} at {location}: "),
        None => format!("{MALFORMED_LEAD}: "),
    };
    let Some(reason) = text.strip_prefix(&head) else {
        panic!("{row}: Display {text:?} does not start with {head:?}");
    };
    assert!(
        !reason.trim().is_empty(),
        "{row}: Display {text:?} has an empty reason"
    );
    assert!(
        !states_a_position(reason),
        "{row}: the reason {reason:?} states a position of its own"
    );
    for name in names {
        for (index, _) in text.match_indices(name) {
            assert!(
                text[..index].matches('"').count() % 2 == 1,
                "{row}: Display {text:?} names {name:?} without quoting it"
            );
        }
    }
    assert!(
        error.source().is_none(),
        "{row}: the refusal {error:?} has a source"
    );
}

// --- LIB-SEC-005 1. Character references -------------------------------------

#[test]
fn lib_sec_005_reference_to_a_non_character_in_an_id_is_malformed_at_the_ampersand() {
    let row = "LIB-SEC-005";
    for reference in BAD_REFERENCES {
        // `xmi:id="p` is 9 characters, so the `&` is at line 2, column 10.
        let input = format!("{ROOT_START}\nxmi:id=\"p{reference}\"/>");
        assert_malformed_at(row, input.as_bytes(), 2, 10);
    }
}

#[test]
fn lib_sec_005_reference_to_a_non_character_in_another_attribute_is_malformed_at_the_ampersand() {
    let row = "LIB-SEC-005";
    for reference in BAD_REFERENCES {
        // An attribute the reader cannot read: Malformed comes first.
        // `foo="` is 5 characters, so the `&` is at line 2, column 6.
        let input = format!("{ROOT_START} xmi:id=\"p\"\nfoo=\"{reference}\"/>");
        assert_malformed_at(row, input.as_bytes(), 2, 6);
        // On a child element, too: `  <c a="` is 8 characters.
        let on_child = format!("{ROOT_START} xmi:id=\"p\">\n  <c a=\"{reference}\"/>\n{ROOT_END}");
        assert_malformed_at(row, on_child.as_bytes(), 2, 9);
    }
}

#[test]
fn lib_sec_005_reference_to_a_non_character_in_text_is_malformed_at_the_ampersand() {
    let row = "LIB-SEC-005";
    for reference in BAD_REFERENCES {
        // In a child element's text: `<name>` is 6 characters.
        let in_child = format!("{ROOT_START} xmi:id=\"p\">\n<name>{reference}</name>{ROOT_END}");
        assert_malformed_at(row, in_child.as_bytes(), 2, 7);
        // In the root's own text, after other text on the same line.
        let in_root = format!("{ROOT_START} xmi:id=\"p\">\nab{reference}{ROOT_END}");
        assert_malformed_at(row, in_root.as_bytes(), 2, 3);
    }
}

#[test]
fn lib_sec_005_references_to_characters_at_the_edges_of_the_ranges_are_read() {
    let row = "LIB-SEC-005";
    for (reference, id) in [
        ("&#xD7FF;", "p\u{D7FF}"),
        ("&#xE000;", "p\u{E000}"),
        ("&#x10FFFF;", "p\u{10FFFF}"),
    ] {
        let input = format!("{ROOT_START} xmi:id=\"p{reference}\"/>");
        assert_ok_with_id(row, input.as_bytes(), id);
    }
}

#[test]
fn lib_sec_005_reference_text_that_is_not_a_reference_is_not_refused() {
    let row = "LIB-SEC-005";
    // In a comment and a PI, in the prolog and in the root: read.
    let comment_and_pi = format!(
        "<!-- &#xD800; -->\n<?tool &#xD800;?>\n{ROOT_START} xmi:id=\"p\">\
         <!-- &#x110000; --><?tool &#xFFFFFFFF;?>{ROOT_END}"
    );
    assert_ok_with_id(row, comment_and_pi.as_bytes(), "p");
    // In a CDATA section: text the root cannot hold, but well-formed.
    let cdata = format!("{ROOT_START} xmi:id=\"p\"><![CDATA[&#xD800;]]>{ROOT_END}");
    assert_not_malformed(row, cdata.as_bytes());
    // Escaped: the id is the text `p&#xD800;`.
    let escaped = format!("{ROOT_START} xmi:id=\"p&amp;#xD800;\"/>");
    assert_ok_with_id(row, escaped.as_bytes(), "p&#xD800;");
}

// --- LIB-SEC-005 2. Prefix undeclaring ---------------------------------------

#[test]
fn lib_sec_005_undeclaring_a_prefix_is_malformed_at_the_declaration() {
    let row = "LIB-SEC-005";
    // On the root: line 2, after two spaces.
    let on_root = format!("{ROOT_START} xmi:id=\"p\"\n  xmlns:p=\"\"/>");
    assert_malformed_at(row, on_root.as_bytes(), 2, 3);
    // On a child: `  <c ` is 5 characters.
    let on_child = format!("{ROOT_START} xmi:id=\"p\">\n  <c xmlns:p=\"\"/>\n{ROOT_END}");
    assert_malformed_at(row, on_child.as_bytes(), 2, 6);
    // Undeclaring a prefix the parent declared.
    let declared_above =
        format!("{ROOT_START} xmlns:p=\"urn:p\" xmi:id=\"p\">\n  <c xmlns:p=\"\"/>\n{ROOT_END}");
    assert_malformed_at(row, declared_above.as_bytes(), 2, 6);
}

#[test]
fn lib_sec_005_an_empty_default_namespace_declaration_is_not_refused() {
    let row = "LIB-SEC-005";
    let on_root = format!("{ROOT_START} xmlns=\"\" xmi:id=\"p\"/>");
    assert_ok_with_id(row, on_root.as_bytes(), "p");
    let on_child = format!("{ROOT_START} xmi:id=\"p\"><c xmlns=\"\"/>{ROOT_END}");
    assert_not_malformed(row, on_child.as_bytes());
}

// --- LIB-SEC-005 3. The XML declaration --------------------------------------

#[test]
fn lib_sec_005_a_declaration_with_an_unknown_or_misplaced_pseudo_attribute_is_malformed_at_its_name()
 {
    let row = "LIB-SEC-005";
    // `<?xml ` is 6 characters; `<?xml version="1.0" ` is 20.
    let cases: [(&str, u32); 6] = [
        ("<?xml versionX=\"1.0\"?>", 7),
        ("<?xml version=\"1.0\" encodingX=\"UTF-8\"?>", 21),
        ("<?xml version=\"1.0\" standaloneX=\"yes\"?>", 21),
        ("<?xml version=\"1.0\" foo=\"bar\"?>", 21),
        // Out of order: version must come first, and standalone last.
        ("<?xml encoding=\"UTF-8\" version=\"1.0\"?>", 7),
        (
            "<?xml version=\"1.0\" standalone=\"yes\" encoding=\"UTF-8\"?>",
            38,
        ),
    ];
    for (declaration, column) in cases {
        assert_malformed_at(row, &declared(declaration), 1, column);
    }
}

#[test]
fn lib_sec_005_a_declaration_with_a_bad_value_is_malformed_at_the_value() {
    let row = "LIB-SEC-005";
    // The value starts after `<?xml version="` (15 characters), after
    // `<?xml version="1.0" standalone="` (32) and after
    // `<?xml version="1.0" encoding="` (30).
    let cases: [(&str, u32); 7] = [
        ("<?xml version=\"2.0\"?>", 16),
        ("<?xml version=\"1.\"?>", 16),
        ("<?xml version=\"abc\"?>", 16),
        ("<?xml version=\"1.0\" standalone=\"maybe\"?>", 33),
        // These fail [81] before the supported-encoding check, so they are
        // Malformed, not Unsupported.
        ("<?xml version=\"1.0\" encoding=\"\"?>", 31),
        ("<?xml version=\"1.0\" encoding=\"8bit\"?>", 31),
        ("<?xml version=\"1.0\" encoding=\"UTF 8\"?>", 31),
    ];
    for (declaration, column) in cases {
        assert_malformed_at(row, &declared(declaration), 1, column);
    }
}

#[test]
fn lib_sec_005_a_declaration_with_tab_cr_or_lf_after_xml_is_checked_by_the_same_rules() {
    let row = "LIB-SEC-005";
    // `<?xml` and a TAB are 6 characters, as with a space.
    assert_malformed_at(row, &declared("<?xml\tversion=\"2.0\"?>"), 1, 16);
    // After a LF or a lone CR, `version="` is 9 characters on line 2.
    assert_malformed_at(row, &declared("<?xml\nversion=\"2.0\"?>"), 2, 10);
    assert_malformed_at(row, &declared("<?xml\rversion=\"2.0\"?>"), 2, 10);
    assert_malformed_at(
        row,
        &declared("<?xml\r\nversion=\"1.0\"\r\nfoo=\"1\"?>"),
        3,
        1,
    );
    // Valid ones are read.
    for declaration in [
        "<?xml\tversion=\"1.0\"?>",
        "<?xml\nversion=\"1.0\"?>",
        "<?xml\rversion=\"1.0\" standalone=\"yes\"?>",
        "<?xml\r\nversion=\"1.0\"\r\nencoding=\"UTF-8\"?>",
        "<?xml\tversion=\"1.0\"\tencoding=\"UTF-8\"\tstandalone=\"no\"\t?>",
    ] {
        assert_ok_with_id(row, &declared(declaration), "p");
    }
}

#[test]
fn lib_sec_005_valid_declarations_and_version_1_1_are_not_refused() {
    let row = "LIB-SEC-005";
    for declaration in [
        "<?xml version=\"1.0\"?>",
        "<?xml version='1.0'?>",
        "<?xml version=\"1.1\"?>",
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>",
        "<?xml version=\"1.0\" encoding='utf-8' standalone='yes'?>",
        "<?xml version = \"1.0\" encoding = \"US-ASCII\" standalone = \"no\" ?>",
        "<?xml version=\"1.0\" standalone=\"no\"?>",
    ] {
        assert_ok_with_id(row, &declared(declaration), "p");
    }
}

// --- LIB-SEC-005 4. `xml` processing-instruction targets ---------------------

#[test]
fn lib_sec_005_a_pi_targeted_xml_in_any_case_is_malformed_at_its_start() {
    let row = "LIB-SEC-005";
    // In the prolog.
    assert_malformed_at(row, format!("<?XML x?>\n{ROOT}").as_bytes(), 1, 1);
    assert_malformed_at(
        row,
        format!("<?xml version=\"1.0\"?>\n<?XML x?>\n{ROOT}").as_bytes(),
        2,
        1,
    );
    // Inside the root, after two spaces on line 2.
    for pi in ["<?xml\tfoo?>", "<?Xml?>", "<?XmL x?>"] {
        let input = format!("{ROOT_START} xmi:id=\"p\">\n  {pi}\n{ROOT_END}");
        assert_malformed_at(row, input.as_bytes(), 2, 3);
    }
}

#[test]
fn lib_sec_005_a_pi_whose_target_only_starts_with_xml_is_not_refused() {
    let row = "LIB-SEC-005";
    let in_prolog = format!("<?xml version=\"1.0\"?>\n<?xml-stylesheet href=\"a\"?>\n{ROOT}");
    assert_ok_with_id(row, in_prolog.as_bytes(), "p");
    let in_root = format!("{ROOT_START} xmi:id=\"p\"><?xml-stylesheet href=\"a\"?>{ROOT_END}");
    assert_ok_with_id(row, in_root.as_bytes(), "p");
}

// --- LIB-SEC-005 5. Order ----------------------------------------------------

#[test]
fn lib_sec_005_a_doctype_comes_before_the_reference_prefix_and_pi_checks() {
    let row = "LIB-SEC-005";
    // An `xml` PI before the DOCTYPE, and a surrogate reference after it.
    for input in [
        format!("<?XML x?>\n<!DOCTYPE r>\n{ROOT}"),
        format!("<!DOCTYPE r>\n{ROOT_START} xmi:id=\"&#xD800;\"/>"),
        format!("<!DOCTYPE r>\n{ROOT_START} xmlns:p=\"\" xmi:id=\"p\"/>"),
    ] {
        let result = read(row, input.as_bytes());
        assert!(
            matches!(result, Err(ReadError::Doctype { .. })),
            "{row}: {} is not refused for its DOCTYPE first: {result:?}",
            preview(input.as_bytes())
        );
    }
}

#[test]
fn lib_sec_005_a_parser_error_comes_before_these_checks() {
    let row = "LIB-SEC-005";
    // A mismatched end tag on line 2, after a surrogate reference and an
    // `xml` PI on line 1: the parser's error, at the end tag, comes first.
    let after_reference = format!("{ROOT_START} xmi:id=\"&#xD800;\">\n</sacm:Claim>");
    assert_malformed_at(row, after_reference.as_bytes(), 2, 1);
    let after_pi = format!("<?XML x?>{ROOT_START} xmi:id=\"p\">\n</sacm:Claim>");
    assert_malformed_at(row, after_pi.as_bytes(), 2, 1);
    let after_undeclaring = format!("{ROOT_START} xmlns:p=\"\" xmi:id=\"p\">\n</sacm:Claim>");
    assert_malformed_at(row, after_undeclaring.as_bytes(), 2, 1);
}

#[test]
fn lib_sec_005_the_earliest_offending_construct_wins() {
    let row = "LIB-SEC-005";
    // An `xml` PI (line 1) before a surrogate reference (line 2).
    let pi_first = format!("<?XML x?>\n{ROOT_START} xmi:id=\"&#xD800;\"/>");
    assert_malformed_at(row, pi_first.as_bytes(), 1, 1);
    // A surrogate reference (line 2) before prefix undeclaring (line 3).
    let reference_first =
        format!("{ROOT_START}\nxmi:id=\"p&#xD800;\">\n  <c xmlns:p=\"\"/>\n{ROOT_END}");
    assert_malformed_at(row, reference_first.as_bytes(), 2, 10);
    // Prefix undeclaring (line 2) before a surrogate reference (line 3).
    let undeclaring_first =
        format!("{ROOT_START} xmi:id=\"p\"\n  xmlns:q=\"\"\n  foo=\"&#xDFFF;\"/>");
    assert_malformed_at(row, undeclaring_first.as_bytes(), 2, 3);
    // An `xml` PI (line 2) before a reference above U+10FFFF (line 3), and the
    // other way round.
    let pi_then_reference =
        format!("{ROOT_START} xmi:id=\"p\">\n  <?Xml?>\n  <c a=\"&#x110000;\"/>\n{ROOT_END}");
    assert_malformed_at(row, pi_then_reference.as_bytes(), 2, 3);
    let reference_then_pi =
        format!("{ROOT_START} xmi:id=\"p\">\n  <c a=\"&#x110000;\"/>\n  <?Xml?>\n{ROOT_END}");
    assert_malformed_at(row, reference_then_pi.as_bytes(), 2, 9);
}

#[test]
fn lib_sec_005_the_declaration_grammar_comes_before_the_declared_encoding() {
    let row = "LIB-SEC-005";
    // A bad version beside an unsupported encoding: VersionInfo is checked
    // first, so Malformed at the version.
    assert_malformed_at(
        row,
        &declared("<?xml version=\"2.0\" encoding=\"ISO-8859-1\"?>"),
        1,
        16,
    );
    // Encoding names that fail [81] are Malformed at the name, never
    // Unsupported for the encoding.
    for name in ["8bit", "", "UTF 8", "-latin"] {
        let declaration = format!("<?xml version=\"1.0\" encoding=\"{name}\"?>");
        assert_malformed_at(row, &declared(&declaration), 1, 31);
    }
}

#[test]
fn lib_sec_005_the_rest_of_the_declaration_is_checked_only_for_a_supported_encoding() {
    let row = "LIB-SEC-005";
    // After an unsupported encoding, SDDecl and `?>` are not checked: the
    // encoding is refused, at its name (line 1, column 31).
    for declaration in [
        &b"<?xml version=\"1.0\" encoding=\"ISO-8859-1\" standalone=\"maybe\"?>"[..],
        b"<?xml version=\"1.0\" encoding=\"ISO-8859-1\" foo=\"1\"?>",
        b"<?xml version=\"1.0\" encoding=\"ISO-8859-1\" \xE9?>",
    ] {
        let mut input = declaration.to_vec();
        input.push(b'\n');
        input.extend_from_slice(ROOT.as_bytes());
        assert_unsupported_at(row, &input, 1, 31);
    }
    // The same declarations with a supported encoding are Malformed at the
    // offending value or name. `<?xml version="1.0" encoding="UTF-8" ` is 37
    // characters, the US-ASCII form 40, and `standalone="` 12.
    let cases: [(&str, u32); 4] = [
        (
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"maybe\"?>",
            50,
        ),
        (
            "<?xml version=\"1.0\" encoding=\"US-ASCII\" standalone=\"maybe\"?>",
            53,
        ),
        ("<?xml version=\"1.0\" encoding=\"UTF-8\" foo=\"1\"?>", 38),
        ("<?xml version=\"1.0\" encoding=\"utf-8\" foo=\"1\"?>", 38),
    ];
    for (declaration, column) in cases {
        assert_malformed_at(row, &declared(declaration), 1, column);
    }
}

#[test]
fn lib_sec_005_a_bad_declaration_comes_before_a_doctype() {
    let row = "LIB-SEC-005";
    // The declaration is checked before parsing, so it is refused at the
    // offending name or value, not as a DOCTYPE on line 2.
    let cases: [(&str, u32); 4] = [
        ("<?xml version=\"1.0\" encodingX=\"UTF-8\"?>", 21),
        ("<?xml version=\"2.0\"?>", 16),
        ("<?xml version=\"1.0\" standalone=\"maybe\"?>", 33),
        ("<?xml version=\"1.0\" encoding=\"8bit\"?>", 31),
    ];
    for (declaration, column) in cases {
        let input = format!("{declaration}\n<!DOCTYPE r>\n{ROOT}");
        assert_malformed_at(row, input.as_bytes(), 1, column);
    }
}

// --- LIB-SEC-005 6. Display and no panic -------------------------------------

#[test]
fn lib_sec_005_each_refusal_display_has_the_malformed_form_and_quotes_names() {
    let row = "LIB-SEC-005";
    let cases: Vec<(Vec<u8>, &[&str])> = vec![
        (
            format!("{ROOT_START} xmi:id=\"&#xD800;\"/>").into_bytes(),
            &["xmi:id"],
        ),
        (
            format!("{ROOT_START} xmi:id=\"p\">&#x110000;{ROOT_END}").into_bytes(),
            &[],
        ),
        (
            format!("{ROOT_START} xmi:id=\"p\" xmlns:zqxp=\"\"/>").into_bytes(),
            &["zqxp"],
        ),
        (
            declared("<?xml version=\"1.0\" zqxpseudo=\"1\"?>"),
            &["zqxpseudo"],
        ),
        (declared("<?xml version=\"zqx9\"?>"), &["zqx9"]),
        (
            declared("<?xml version=\"1.0\" standalone=\"zqxmaybe\"?>"),
            &["zqxmaybe"],
        ),
        (
            declared("<?xml version=\"1.0\" encoding=\"zqx enc\"?>"),
            &["zqx enc"],
        ),
        // Line breaks in a value or around a name never break the line.
        (declared("<?xml version=\"1.0\" encoding=\"x\ny\"?>"), &[]),
        (
            declared("<?xml version=\"1.0\" encoding=\"x\u{2028}y\"?>"),
            &[],
        ),
        (
            declared("<?xml version=\"1.0\"\r\nzqxline\r\n=\"1\"?>"),
            &["zqxline"],
        ),
        (format!("<?XmL zqx?>\n{ROOT}").into_bytes(), &["XmL"]),
    ];
    for (input, names) in &cases {
        let result = read(row, input);
        let Err(error) = result else {
            panic!("{row}: {} was read; expected Malformed", preview(input));
        };
        assert!(
            matches!(error, ReadError::Malformed { .. }),
            "{row}: expected Malformed for {}, got {error:?}",
            preview(input)
        );
        assert!(
            error.location().is_some(),
            "{row}: the refusal of {} has no location",
            preview(input)
        );
        assert_malformed_form(row, &error, names);
    }
}

/// A small document holding each construct the checks look at: a declaration
/// with all three pseudo-attributes, a PI, character references in an
/// attribute and in text, a default namespace declaration, a comment and a
/// CDATA section.
const SAMPLE: &str = concat!(
    "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n",
    "<?tool a?>\n",
    "<sacm:ArgumentPackage",
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
    " xmlns=\"\" xmi:id=\"p&#x42;&#xD7FF;\">",
    "<!-- &#xD800; --><?xml-stylesheet href=\"a\"?>",
    "<c a=\"&#x10FFFF;&#65;\">&#xE000;<![CDATA[&#xD800;]]></c>",
    "</sacm:ArgumentPackage>",
);

/// The bytes that the no-panic sweep writes over each byte of the sample.
const REPLACEMENTS: [u8; 9] = [0x00, b'<', b'&', 0xFF, b'?', b'"', b'x', b'\r', b'#'];

#[test]
fn lib_sec_005_the_checks_never_panic_and_every_malformed_refusal_has_its_form() {
    let row = "LIB-SEC-005";
    let sample = SAMPLE.as_bytes();
    let prefixes = (0..=sample.len()).map(|len| sample[..len].to_vec());
    let replacements = (0..sample.len()).flat_map(|index| {
        REPLACEMENTS.iter().map(move |&byte| {
            let mut input = sample.to_vec();
            input[index] = byte;
            input
        })
    });
    let edges: Vec<Vec<u8>> = [
        "<?xml",
        "<?xml?>",
        "<?xml ?><r/>",
        "<?xml version=",
        "<?xml version=\"",
        "<?xml version=\"1.0\" encoding=\"",
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=",
        "<?xml\t\r\n?>",
        "<?XML?>",
        "<r a=\"&#x;\"/>",
        "<r a=\"&#;\"/>",
        "<r a=\"&#xD800\"/>",
        "<r>&#xFFFFFFFFFFFFFFFF;</r>",
        "<r>&#99999999999999999999;</r>",
        "<r xmlns:=\"\"/>",
        "<r xmlns:p=\"\"",
        "<r><?xml",
    ]
    .iter()
    .map(|text| text.as_bytes().to_vec())
    .collect();
    let mut count = 0_usize;
    for input in prefixes.chain(replacements).chain(edges) {
        // Returning is the first assertion: a panic fails the test.
        if let Err(error @ ReadError::Malformed { .. }) = read(row, &input) {
            assert_eq!(
                error.code(),
                Some(Code::XML_MALFORMED),
                "{row}: a Malformed refusal of {} does not carry SACM-XML-001",
                preview(&input)
            );
            assert_malformed_form(row, &error, &[]);
        }
        count += 1;
    }
    assert!(
        count > sample.len() * REPLACEMENTS.len(),
        "{row}: the sweep did not cover every prefix and replacement"
    );
    // The sample itself is well-formed: it is refused, if at all, for its
    // content, never as Malformed.
    assert_not_malformed(row, sample);
}

// --- LIB-SEC-005. Exact results ------------------------------------------------
//
// Each input below has one exact result: the variant, the location and the
// reason. They tell apart scans that would stop early, skip a construct or
// word a refusal for another case.

/// The readable root with `content` inside: `<sacm:ArgumentPackage …
/// xmi:id="p">content</sacm:ArgumentPackage>`. Its start tag is 133
/// characters, so `content` starts at line 1, column 134.
fn root_holding(content: &str) -> Vec<u8> {
    format!("{ROOT_START} xmi:id=\"p\">{content}{ROOT_END}").into_bytes()
}

/// Asserts that `input` is refused as `Malformed` at `line` and `column`
/// with exactly `reason`, and that the `Display` is exactly the
/// LIB-SEC-002 form around it.
fn assert_malformed_exactly(row: &str, input: &[u8], line: u32, column: u32, reason: &str) {
    let error = assert_malformed_at(row, input, line, column);
    let ReadError::Malformed { reason: found, .. } = &error else {
        panic!(
            "{row}: expected Malformed for {}, got {error:?}",
            preview(input)
        );
    };
    assert_eq!(
        found,
        reason,
        "{row}: the refusal of {} has the wrong reason",
        preview(input)
    );
    assert_eq!(
        error.to_string(),
        format!("{MALFORMED_LEAD} at line {line}, column {column}: {reason}"),
        "{row}: unexpected Display for {}",
        preview(input)
    );
}

#[test]
fn lib_sec_005_a_declaration_cut_after_an_unknown_name_is_refused_for_that_name() {
    let row = "LIB-SEC-005";
    // The name is refused where it starts, not as the end of the input.
    assert_malformed_exactly(
        row,
        b"<?xml version=\"1.0\" foo",
        1,
        21,
        "the XML declaration has the unknown pseudo-attribute \"foo\"",
    );
}

#[test]
fn lib_sec_005_a_bad_declaration_comes_before_invalid_utf8() {
    let row = "LIB-SEC-005";
    // The invalid byte is at line 2, column 4; the declaration is checked
    // first.
    assert_malformed_exactly(
        row,
        b"<?xml version=\"1.0\" foo=\"1\"?>\n<r>\xFF</r>",
        1,
        21,
        "the XML declaration has the unknown pseudo-attribute \"foo\"",
    );
}

#[test]
fn lib_sec_005_unknown_and_out_of_order_pseudo_attributes_have_their_own_reasons() {
    let row = "LIB-SEC-005";
    assert_malformed_exactly(
        row,
        b"<?xml version=\"1.0\" foo=\"1\"?>\n<r/>",
        1,
        21,
        "the XML declaration has the unknown pseudo-attribute \"foo\"",
    );
    assert_malformed_exactly(
        row,
        b"<?xml encoding=\"UTF-8\" version=\"1.0\"?>\n<r/>",
        1,
        7,
        "the pseudo-attribute \"encoding\" is out of order in the XML declaration",
    );
    assert_malformed_exactly(
        row,
        b"<?xml version=\"1.0\" standalone=\"yes\" encoding=\"UTF-8\"?>\n<r/>",
        1,
        38,
        "the pseudo-attribute \"encoding\" is out of order in the XML declaration",
    );
}

#[test]
fn lib_sec_005_an_xml_pi_after_pi_text_in_a_comment_or_cdata_is_found() {
    let row = "LIB-SEC-005";
    let reason = "a processing instruction has the reserved target \"Xml\"";
    // `<!--x<?-->` is 10 characters and `<![CDATA[x<?]]>` 15.
    assert_malformed_exactly(row, &root_holding("<!--x<?--><?Xml?>"), 1, 144, reason);
    assert_malformed_exactly(row, &root_holding("<![CDATA[x<?]]><?Xml?>"), 1, 149, reason);
}

#[test]
fn lib_sec_005_checks_after_sibling_elements_find_the_construct() {
    let row = "LIB-SEC-005";
    // `<c></c> <d ` is 11 characters.
    assert_malformed_exactly(
        row,
        &root_holding("<c></c> <d xmlns:p=\"\"/>"),
        1,
        145,
        "the namespace prefix \"p\" is declared with an empty value, and a prefix cannot be undeclared",
    );
    let reason = "a processing instruction has the reserved target \"Xml\"";
    // `<cc/>` is 5 characters and `<c/>` 4.
    assert_malformed_exactly(row, &root_holding("<cc/><?Xml?>"), 1, 139, reason);
    assert_malformed_exactly(row, &root_holding("<c/><?Xml?>"), 1, 138, reason);
}

#[test]
fn lib_sec_005_character_reference_refusals_quote_the_reference_and_name_its_kind() {
    let row = "LIB-SEC-005";
    let surrogate = "refers to a surrogate, which is not a character";
    let above = "refers to a code point above U+10FFFF, which is not a character";
    for (reference, kind) in [
        ("&#xD800;", surrogate),
        ("&#xDFFF;", surrogate),
        ("&#55296;", surrogate),
        ("&#x110000;", above),
        ("&#xFFFFFFFF;", above),
    ] {
        // The whole reference, `;` included, is quoted.
        let reason = format!("the character reference \"{reference}\" {kind}");
        assert_malformed_exactly(row, &root_holding(reference), 1, 134, &reason);
    }
}

// --- LIB-SEC-005. Comments, PIs and CDATA are skipped whole --------------------
//
// The checks that run after the parse skip comments, processing
// instructions and CDATA sections. Each must end where XML ends it: a
// comment at the first `-->` after `<!--`, so `<!-->` and `<!--->` do not end
// there; a PI at the first `?>` after `<?`; a CDATA section at the first
// `]]>`. A scan that ends one early reads its text as markup, refusing what
// is not there or missing what is.

/// An ArtifactPackage root: recognised but not read yet, so `Unsupported` at
/// its start tag (ADR 0016 D5).
const ARTIFACT_ROOT: &str = concat!(
    "<sacm:ArtifactPackage",
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
    " xmi:id=\"f\"/>",
);

#[test]
fn lib_sec_005_a_comment_starting_with_gt_or_dash_gt_is_skipped_whole() {
    let row = "LIB-SEC-005";
    let skipped = [
        // Comments whose text starts with `>` or `->`.
        "<!-->&#xD800;-->",
        "<!--->&#xD800;-->",
        "<!--><?xml x?>-->",
        "<!--> <r xmlns:p=''>-->",
        // A PI whose content starts with `>`.
        "<?pi >&#xD800;?>",
    ];
    for prolog in skipped {
        // Before a readable root: read.
        let before_root = format!("{prolog}\n{ROOT}");
        assert_ok_with_id(row, before_root.as_bytes(), "p");
        // Inside a readable root: read.
        assert_ok_with_id(row, &root_holding(prolog), "p");
        // Before a root not read yet: refused at that root (line 2,
        // column 1), never for the comment's or PI's text.
        let before_artifact = format!("{prolog}\n{ARTIFACT_ROOT}");
        assert_unsupported_at(row, before_artifact.as_bytes(), 2, 1);
    }
}

#[test]
fn lib_sec_005_a_comment_starting_with_gt_does_not_hide_a_later_construct() {
    let row = "LIB-SEC-005";
    // Ended at the `-->` inside `<!-->`, this comment would leave `<x a='`
    // open, and the quote would hide what follows up to the next `'`.
    let prolog = "<!--> <x a='-->";
    // `xmi:id="a` is 9 characters: the `&` is at line 3, column 10.
    let reference = format!("{prolog}\n{ROOT_START}\nxmi:id=\"a&#xD800;\"/>");
    assert_malformed_exactly(
        row,
        reference.as_bytes(),
        3,
        10,
        "the character reference \"&#xD800;\" refers to a surrogate, which is not a character",
    );
    let undeclaring = format!("{prolog}\n{ROOT_START} xmi:id=\"p\"\n  xmlns:p=\"\"/>");
    assert_malformed_exactly(
        row,
        undeclaring.as_bytes(),
        3,
        3,
        "the namespace prefix \"p\" is declared with an empty value, and a prefix cannot be undeclared",
    );
    let pi = format!("{prolog}\n{ROOT_START} xmi:id=\"p\">\n  <?XML x?>\n{ROOT_END}");
    assert_malformed_exactly(
        row,
        pi.as_bytes(),
        3,
        3,
        "a processing instruction has the reserved target \"XML\"",
    );
}

#[test]
fn lib_sec_005_cdata_with_spaces_and_quotes_is_skipped_whole() {
    let row = "LIB-SEC-005";
    // The CDATA section is text: the root cannot hold it yet, so it is
    // Unsupported at the section, never Malformed for what the section
    // holds.
    let input = root_holding("<![CDATA[ a='&#xD800;' xmlns:p='' <?xml?>]]>");
    let error = assert_unsupported_at(row, &input, 1, 134);
    assert_eq!(
        error.to_string(),
        "the reader cannot represent the input at line 1, column 134: \
         the element \"sacm:ArgumentPackage\" holds text, which is not read in this release",
        "{row}: unexpected Display for {}",
        preview(&input)
    );
}
