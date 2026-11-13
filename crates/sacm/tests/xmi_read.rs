//! Tests for the safety of the XMI read entry point: `sacm::xmi::read`,
//! `ReadOptions`, `ReadOutput` and `ReadError` (LIB-SEC-001, LIB-SEC-002,
//! LIB-API-009; ADR 0005, ADR 0015).
//!
//! Every input is an inline byte string. `read` is called with
//! `ReadOptions::default()`, which reads in tolerant mode (LIB-COMPAT-001).
//!
//! Not checked here:
//! - LIB-SEC-002 item 6's `#[non_exhaustive]` on `ReadError`: a test cannot
//!   show that an exhaustive match fails to compile. The verifier checks the
//!   attribute in the source and the rustdoc (and on its variants,
//!   `ReadOptions` and `ReadOutput`, which ADR 0015 also marks).
//! - LIB-SEC-001 item 2's "without growing": a test cannot bound memory
//!   portably. The billion-laughs test checks that `read` refuses the input
//!   as `Doctype` and that no expanded text reaches the error.
//! - LIB-SEC-001 item 3's "attribute value": XML does not allow a literal `<`
//!   in an attribute value, so a well-formed document can only carry
//!   `<!DOCTYPE` there escaped, as `&lt;!DOCTYPE`. The test uses that form.
//! - "In every mode" (LIB-SEC-001): the tests here use the default, tolerant,
//!   mode; `load_and_save_modes.rs` (LIB-COMPAT-001) reads a DOCTYPE, an
//!   external entity and malformed input in both modes and checks that the
//!   refusals are the same.
//!
//! Since SACM23-XMI-001 (slice 0021), AssuranceCasePackage and ArgumentPackage
//! roots are read, and other roots are refused as SACM-XMI-001. The tests here
//! that need a refusal at a known place therefore use input that is still
//! `Unsupported` there: an ArtifactPackage root, a package name outside the
//! SACM namespace, or a readable root with a child it cannot read. The roots
//! themselves, and the SACM-XMI-001 locations, are tested in `xmi_roots.rs`.

use std::error::Error;
use std::fmt::{Debug, Display};
use std::hash::Hash;
use std::path::Path;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::Duration;

use sacm::diagnostic::{Code, SourceLocation};
use sacm::xmi::{self, Mode, ReadError, ReadOptions, ReadOutput};

/// One mebibyte.
const MIB: usize = 1 << 20;

/// The lead of the `Doctype` `Display`, before the location.
const DOCTYPE_LEAD: &str = "the input has a DOCTYPE, which is refused";
/// The lead of the `Malformed` `Display`, before the location and reason.
const MALFORMED_LEAD: &str = "the input is not well-formed XML";
/// The lead of the `Unsupported` `Display`, before the location and reason.
const UNSUPPORTED_LEAD: &str = "the reader cannot represent the input";

/// A small, well-formed SACM 2.3 document. It ends exactly at the root's end
/// tag, so every proper prefix of it is not well-formed. It holds a
/// non-ASCII character (a cut can split it), an entity reference and a CDATA
/// section.
const SAMPLE: &str = concat!(
    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
    "<!-- LIB-SEC-002 sample: a small café case -->\n",
    "<sacm:AssuranceCasePackage",
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
    " xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"",
    " xmi:version=\"2.0\" xmi:id=\"case_1\">\n",
    "  <argumentPackage xmi:id=\"arg_1\">\n",
    "    <argumentElement xsi:type=\"sacm:Claim\" xmi:id=\"c_1\" gid=\"g_1\"/>\n",
    "    <argumentElement xsi:type=\"sacm:Claim\" xmi:id=\"c_2\">",
    "<description><content><value>speed &lt; 30 km/h<![CDATA[ & more]]></value></content></description>",
    "</argumentElement>\n",
    "  </argumentPackage>\n",
    "</sacm:AssuranceCasePackage>",
);

/// The SACM root start tag used by the smaller documents, with its
/// namespaces declared.
const SACM_ROOT_OPEN: &str = concat!(
    "<sacm:AssuranceCasePackage",
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
    " xmi:version=\"2.0\" xmi:id=\"case_1\">",
);

/// The SACM root end tag that closes [`SACM_ROOT_OPEN`].
const SACM_ROOT_CLOSE: &str = "</sacm:AssuranceCasePackage>";

/// A root the reader recognises but cannot read yet (ADR 0016 D5): it is
/// refused as `Unsupported` at its start tag.
const ARTIFACT_ROOT: &str = concat!(
    "<sacm:ArtifactPackage",
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
    " xmi:id=\"artifacts_1\"/>",
);

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
    let end = input.len().min(120);
    let text = String::from_utf8_lossy(&input[..end]);
    if end < input.len() {
        format!("{text:?}... ({} bytes)", input.len())
    } else {
        format!("{text:?}")
    }
}

/// Reads `input`, which the test expects to be refused, and returns why.
fn refusal(row: &str, input: &[u8]) -> ReadError {
    match read(row, input) {
        Ok(output) => panic!("{row}: read returned Ok({output:?}) for {}", preview(input)),
        Err(error) => error,
    }
}

/// A 1-based location that the test expects `SourceLocation::new` to accept.
fn loc(row: &str, line: u32, column: u32) -> SourceLocation {
    let Some(location) = SourceLocation::new(line, column) else {
        panic!("{row}: SourceLocation::new({line}, {column}) refused a 1-based position");
    };
    location
}

/// The position just past the last character of `input`: the line after the
/// last line feed, and one column past its last character. Invalid UTF-8
/// counts as one character per replacement, which never moves the end
/// earlier than the first invalid byte.
fn end_of(row: &str, input: &[u8]) -> SourceLocation {
    let text = String::from_utf8_lossy(input);
    let line = text.matches('\n').count() + 1;
    let last_line = text.rsplit('\n').next().unwrap_or("");
    let column = last_line.chars().count() + 1;
    let (Ok(line), Ok(column)) = (u32::try_from(line), u32::try_from(column)) else {
        panic!("{row}: the test input is too large to locate");
    };
    loc(row, line, column)
}

/// Asserts that `input` is refused as `Doctype` (SACM-SEC-001) at `line`
/// and `column`, and returns the error.
fn assert_doctype_at(row: &str, input: &[u8], line: u32, column: u32) -> ReadError {
    let error = refusal(row, input);
    assert!(
        matches!(error, ReadError::Doctype { .. }),
        "{row}: expected Doctype for {}, got {error:?}",
        preview(input)
    );
    assert_eq!(
        error.code(),
        Some(Code::XML_DOCTYPE_REJECTED),
        "{row}: the Doctype refusal of {} does not carry SACM-SEC-001",
        preview(input)
    );
    assert_eq!(
        error.location(),
        Some(loc(row, line, column)),
        "{row}: the Doctype refusal of {} is not located at the `<` of `<!DOCTYPE`",
        preview(input)
    );
    error
}

/// Asserts that `input` is refused as `Malformed` (SACM-XML-001) and returns
/// the error.
fn assert_malformed(row: &str, input: &[u8]) -> ReadError {
    let error = refusal(row, input);
    assert!(
        matches!(error, ReadError::Malformed { .. }),
        "{row}: expected Malformed for {}, got {error:?}",
        preview(input)
    );
    assert_eq!(
        error.code(),
        Some(Code::XML_MALFORMED),
        "{row}: the Malformed refusal of {} does not carry SACM-XML-001",
        preview(input)
    );
    error
}

/// Asserts that `input` is refused as `Unsupported`, with no code, and
/// returns the error.
fn assert_unsupported(row: &str, input: &[u8]) -> ReadError {
    let error = refusal(row, input);
    assert!(
        matches!(error, ReadError::Unsupported { .. }),
        "{row}: expected Unsupported for {}, got {error:?}",
        preview(input)
    );
    assert_eq!(
        error.code(),
        None,
        "{row}: the Unsupported refusal of {} carries a code; Unsupported has none",
        preview(input)
    );
    error
}

/// Asserts that `result` is neither a `Doctype` nor a `Malformed` refusal.
fn assert_not_doctype_or_malformed(
    row: &str,
    input: &[u8],
    result: &Result<ReadOutput, ReadError>,
) {
    assert!(
        !matches!(
            result,
            Err(ReadError::Doctype { .. } | ReadError::Malformed { .. })
        ),
        "{row}: {} was refused as Doctype or Malformed: {result:?}",
        preview(input)
    );
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

/// Asserts that `error`'s `Display` has the shape `<lead> at <location>:
/// <reason>`, or `<lead>: <reason>` when the location is unknown, is one
/// lowercase line, and returns the reason.
fn display_reason(row: &str, error: &ReadError, lead: &str) -> String {
    let text = error.to_string();
    assert_one_lowercase_line(row, &text);
    let head = match error.location() {
        Some(location) => format!("{lead} at {location}: "),
        None => format!("{lead}: "),
    };
    let Some(reason) = text.strip_prefix(&head) else {
        panic!("{row}: Display {text:?} does not start with {head:?}");
    };
    assert!(
        !reason.trim().is_empty(),
        "{row}: Display {text:?} has an empty reason"
    );
    reason.to_owned()
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

/// Asserts that every occurrence of `name` in `text` is inside double
/// quotes: an odd number of `"` precede it.
fn assert_quoted_wherever_named(row: &str, text: &str, name: &str) {
    for (index, _) in text.match_indices(name) {
        let quotes_before = text[..index].matches('"').count();
        assert!(
            quotes_before % 2 == 1,
            "{row}: Display {text:?} names {name:?} without quoting it"
        );
    }
}

/// Checks what holds for every refusal: the variant's code, the variant's
/// `Display` shape, and no source.
fn assert_refusal_invariants(row: &str, input: &[u8], error: &ReadError) {
    assert!(
        error.source().is_none(),
        "{row}: the refusal of {} has a source: {error:?}",
        preview(input)
    );
    match error {
        ReadError::Doctype { .. } => {
            assert_eq!(
                error.code(),
                Some(Code::XML_DOCTYPE_REJECTED),
                "{row}: a Doctype refusal of {} does not carry SACM-SEC-001",
                preview(input)
            );
            let text = error.to_string();
            assert_one_lowercase_line(row, &text);
            let expected = match error.location() {
                Some(location) => format!("{DOCTYPE_LEAD} at {location}"),
                None => DOCTYPE_LEAD.to_owned(),
            };
            assert_eq!(text, expected, "{row}: unexpected Doctype Display");
        }
        ReadError::Malformed { .. } => {
            assert_eq!(
                error.code(),
                Some(Code::XML_MALFORMED),
                "{row}: a Malformed refusal of {} does not carry SACM-XML-001",
                preview(input)
            );
            let reason = display_reason(row, error, MALFORMED_LEAD);
            assert!(
                !states_a_position(&reason),
                "{row}: the Malformed reason {reason:?} states a position of its own"
            );
        }
        ReadError::Unsupported { .. } => {
            assert_eq!(
                error.code(),
                None,
                "{row}: an Unsupported refusal of {} carries a code",
                preview(input)
            );
            display_reason(row, error, UNSUPPORTED_LEAD);
        }
        _ => assert_one_lowercase_line(row, &error.to_string()),
    }
}

/// Whether the diagnostics catalog has a table row for `code`.
fn catalog_has_row(row: &str, code: Code) -> bool {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/sacm/diagnostics.md");
    let catalog = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{row}: cannot read {}: {error}", path.display()));
    let cell = format!("| {} |", code.as_str());
    catalog
        .lines()
        .any(|line| line.trim_start().starts_with(&cell))
}

/// The sample with byte `index` replaced by `byte`.
fn replaced(index: usize, byte: u8) -> Vec<u8> {
    let mut input = SAMPLE.as_bytes().to_vec();
    input[index] = byte;
    input
}

/// The bytes that the no-panic sweep writes over each byte of the sample.
const REPLACEMENTS: [u8; 4] = [0x00, b'<', b'&', 0xFF];

/// Every input of the hostile sweep: each proper prefix of the sample and
/// each single-byte replacement of it.
fn hostile_sweep() -> impl Iterator<Item = Vec<u8>> {
    let prefixes = (0..SAMPLE.len()).map(|len| SAMPLE.as_bytes()[..len].to_vec());
    let replacements = (0..SAMPLE.len())
        .flat_map(|index| REPLACEMENTS.iter().map(move |&byte| replaced(index, byte)));
    prefixes.chain(replacements)
}

/// A well-formed document with the SACM root after `prolog`.
fn sacm_document(prolog: &str, content: &str) -> Vec<u8> {
    format!("{prolog}{SACM_ROOT_OPEN}{content}{SACM_ROOT_CLOSE}").into_bytes()
}

/// `text` encoded as UTF-32 with a byte order mark.
fn utf32_with_bom(text: &str, big_endian: bool) -> Vec<u8> {
    let mut bytes = Vec::new();
    for c in std::iter::once('\u{FEFF}').chain(text.chars()) {
        let quad = if big_endian {
            u32::from(c).to_be_bytes()
        } else {
            u32::from(c).to_le_bytes()
        };
        bytes.extend_from_slice(&quad);
    }
    bytes
}

// --- LIB-SEC-001 1. Every DOCTYPE is refused at its location ---------------

#[test]
fn lib_sec_001_doctype_without_internal_subset_is_refused_at_its_location() {
    let row = "LIB-SEC-001";
    assert_doctype_at(row, b"<!DOCTYPE r>\n<r/>", 1, 1);
    let after_declaration = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <!DOCTYPE sacm:AssuranceCasePackage>\n\
         {SACM_ROOT_OPEN}{SACM_ROOT_CLOSE}"
    );
    assert_doctype_at(row, after_declaration.as_bytes(), 2, 1);
}

#[test]
fn lib_sec_001_doctype_with_entity_declarations_is_refused_at_its_location() {
    let row = "LIB-SEC-001";
    // An XXE: an external general entity, referenced in content.
    let xxe = b"<?xml version=\"1.0\"?>\n\
        <!DOCTYPE r [\n  <!ENTITY xxe SYSTEM \"http://example.invalid/secret\">\n]>\n\
        <r>&xxe;</r>";
    assert_doctype_at(row, xxe, 2, 1);
    // A billion-laughs chain, after a comment.
    let laughs = billion_laughs();
    assert_doctype_at(row, laughs.as_bytes(), 3, 1);
    // A parameter entity, expanded in the internal subset.
    let parameter = b"<?xml version=\"1.0\"?>\n<!-- prolog -->\n\
        <!DOCTYPE r [ <!ENTITY % p SYSTEM \"http://example.invalid/p.dtd\"> %p; ]>\n\
        <r/>";
    assert_doctype_at(row, parameter, 3, 1);
    // An internal subset cut short: refused before anything in it is read.
    assert_doctype_at(row, b"<!DOCTYPE r [ <!ENTITY x SYS", 1, 1);
}

#[test]
fn lib_sec_001_external_dtd_is_refused_at_its_location() {
    let row = "LIB-SEC-001";
    let public = b"<?xml version=\"1.0\"?>\n\
        <!DOCTYPE r PUBLIC \"-//Example//DTD Example//EN\" \"http://example.invalid/r.dtd\">\n\
        <r/>";
    assert_doctype_at(row, public, 2, 1);
    let system = b"<?xml version=\"1.0\"?>\n\
        <!DOCTYPE r SYSTEM \"http://example.invalid/r.dtd\">\n<r/>";
    assert_doctype_at(row, system, 2, 1);
}

#[test]
fn lib_sec_001_doctype_after_a_prolog_is_located_by_line_and_column() {
    let row = "LIB-SEC-001";
    // Comments and processing instructions first; the DOCTYPE is indented on
    // line 6.
    let prolog = b"<?xml version=\"1.0\"?>\n\
        <!-- first -->\n\
        <?tool hint?>\n\
        <!-- second\n   spans two lines -->\n  <!DOCTYPE r SYSTEM \"r.dtd\">\n<r/>";
    assert_doctype_at(row, prolog, 6, 3);
    // On the declaration's line, after a comment holding a non-ASCII
    // character: the column counts characters, not bytes.
    // `<?xml version="1.0"?>` is 21 characters and `<!--é-->` 8.
    let same_line = "<?xml version=\"1.0\"?><!--é--><!DOCTYPE r>\n<r/>";
    assert_doctype_at(row, same_line.as_bytes(), 1, 30);
}

#[test]
fn lib_sec_001_doctype_after_a_utf8_bom_is_at_line_1_column_1() {
    let row = "LIB-SEC-001";
    // The BOM is an encoding signature and takes no column.
    assert_doctype_at(row, b"\xEF\xBB\xBF<!DOCTYPE r><r/>", 1, 1);
    assert_doctype_at(row, b"\xEF\xBB\xBF<!--p--><!DOCTYPE r><r/>", 1, 9);
    assert_doctype_at(
        row,
        b"\xEF\xBB\xBF<?xml version=\"1.0\"?>\n<!DOCTYPE r><r/>",
        2,
        1,
    );
}

// --- LIB-SEC-001 2. Nothing is resolved or expanded -------------------------

/// A `file:` URL naming the crate's own `Cargo.toml`, a file that exists.
fn manifest_url() -> (String, String) {
    let row = "LIB-SEC-001";
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let contents = std::fs::read_to_string(&manifest)
        .unwrap_or_else(|error| panic!("{row}: cannot read {}: {error}", manifest.display()));
    let path = manifest
        .to_string_lossy()
        .replace('\\', "/")
        .replace(' ', "%20");
    let url = if path.starts_with('/') {
        format!("file://{path}")
    } else {
        format!("file:///{path}")
    };
    (url, contents)
}

/// Asserts that no line of `contents` appears in `text`. Lines shorter than
/// six characters (`]`, `[lints]`-like fragments) are skipped, so that the
/// check does not fire on chance overlaps; at least one line must be
/// checked.
fn assert_no_line_of(row: &str, contents: &str, text: &str, what: &str) {
    let lines: Vec<&str> = contents
        .lines()
        .map(str::trim)
        .filter(|line| line.chars().count() >= 6)
        .collect();
    assert!(!lines.is_empty(), "{row}: the file has no line to look for");
    for line in lines {
        assert!(
            !text.contains(line),
            "{row}: the {what} {text:?} contains the line {line:?} of the referenced file"
        );
    }
}

#[test]
fn lib_sec_001_xxe_naming_an_existing_file_is_not_resolved() {
    let row = "LIB-SEC-001";
    let (url, contents) = manifest_url();
    let general = format!(
        "<?xml version=\"1.0\"?>\n\
         <!DOCTYPE r [\n  <!ENTITY xxe SYSTEM \"{url}\">\n]>\n\
         <r>&xxe;</r>"
    );
    let parameter = format!(
        "<?xml version=\"1.0\"?>\n\
         <!DOCTYPE r [\n  <!ENTITY % xxe SYSTEM \"{url}\">\n  %xxe;\n]>\n\
         <r/>"
    );
    for input in [general, parameter] {
        let error = assert_doctype_at(row, input.as_bytes(), 2, 1);
        assert_no_line_of(row, &contents, &error.to_string(), "Display");
        assert_no_line_of(row, &contents, &format!("{error:?}"), "Debug");
    }
}

/// The classic billion-laughs input: ten levels of ten references each,
/// declared after a comment, so the DOCTYPE is on line 3.
fn billion_laughs() -> String {
    let mut lines = vec![
        "<?xml version=\"1.0\"?>".to_owned(),
        "<!-- laughs -->".to_owned(),
        "<!DOCTYPE lolz [".to_owned(),
        "  <!ENTITY lol \"lol\">".to_owned(),
    ];
    for level in 1..=9 {
        let previous = if level == 1 {
            "lol".to_owned()
        } else {
            format!("lol{}", level - 1)
        };
        let body = format!("&{previous};").repeat(10);
        lines.push(format!("  <!ENTITY lol{level} \"{body}\">"));
    }
    lines.push("]>".to_owned());
    lines.push("<lolz>&lol9;</lolz>".to_owned());
    lines.join("\n")
}

#[test]
fn lib_sec_001_billion_laughs_is_refused_without_expansion() {
    let row = "LIB-SEC-001";
    let input = billion_laughs();
    let error = assert_doctype_at(row, input.as_bytes(), 3, 1);
    for (what, text) in [
        ("Display", error.to_string()),
        ("Debug", format!("{error:?}")),
    ] {
        assert!(
            !text.contains("lollol"),
            "{row}: the {what} {text:?} holds expanded entity text"
        );
        assert!(
            text.len() < 256,
            "{row}: the {what} of the billion-laughs refusal is {} bytes; it should not grow with the input",
            text.len()
        );
    }
}

// --- LIB-SEC-001 3. Declaration text that is not a declaration --------------

#[test]
fn lib_sec_001_declaration_text_in_comment_cdata_or_attribute_is_not_refused() {
    let row = "LIB-SEC-001";
    let declaration = "<!DOCTYPE r [ <!ENTITY xxe SYSTEM \"file:///etc/passwd\"> ]>";
    let inputs = [
        // In a comment, in the prolog and in content.
        sacm_document(
            &format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!-- {declaration} -->\n"),
            &format!("<!-- {declaration} -->"),
        ),
        // In a CDATA section.
        sacm_document(
            "<?xml version=\"1.0\"?>\n",
            &format!("<description><![CDATA[{declaration} <!ENTITY y \"z\">]]></description>"),
        ),
        // In an attribute value (escaped, the only well-formed way).
        sacm_document(
            "<?xml version=\"1.0\"?>\n",
            "<argumentPackage xmi:id=\"arg_1\" \
             name=\"&lt;!DOCTYPE r&gt; &lt;!ENTITY x &quot;y&quot;&gt;\"/>",
        ),
        // In a processing instruction.
        sacm_document(
            &format!("<?xml version=\"1.0\"?>\n<?note {declaration}?>\n"),
            "",
        ),
    ];
    for input in inputs {
        let result = read(row, &input);
        assert_not_doctype_or_malformed(row, &input, &result);
    }
}

// --- LIB-SEC-001 4. Stray declarations and undeclared references ------------

#[test]
fn lib_sec_001_stray_entity_declaration_is_malformed_not_doctype() {
    let row = "LIB-SEC-001";
    let in_prolog = b"<?xml version=\"1.0\"?>\n<!ENTITY x \"y\">\n<r>&x;</r>";
    let in_content = b"<?xml version=\"1.0\"?>\n<r>\n<!ENTITY x \"y\">\n&x;</r>";
    for (input, line) in [(&in_prolog[..], 2), (&in_content[..], 3)] {
        let error = assert_malformed(row, input);
        let Some(location) = error.location() else {
            panic!(
                "{row}: the stray declaration in {} is not located",
                preview(input)
            );
        };
        assert_eq!(
            location.line(),
            line,
            "{row}: the stray declaration in {} is located on the wrong line",
            preview(input)
        );
    }
}

#[test]
fn lib_sec_001_undeclared_entity_reference_is_malformed_and_not_expanded() {
    let row = "LIB-SEC-001";
    let in_content = b"<?xml version=\"1.0\"?>\n<r>&xxe;</r>";
    let in_attribute = b"<?xml version=\"1.0\"?>\n<r a=\"&xxe;\"/>";
    for input in [&in_content[..], &in_attribute[..]] {
        let error = assert_malformed(row, input);
        let Some(location) = error.location() else {
            panic!("{row}: the reference in {} is not located", preview(input));
        };
        assert_eq!(
            location.line(),
            2,
            "{row}: the reference in {} is located on the wrong line",
            preview(input)
        );
    }
}

// --- LIB-SEC-001 5. The code -------------------------------------------------

#[test]
fn lib_sec_001_doctype_rejected_code_is_sacm_sec_001_in_the_catalog() {
    let row = "LIB-SEC-001";
    assert_eq!(
        Code::XML_DOCTYPE_REJECTED.as_str(),
        "SACM-SEC-001",
        "{row}: Code::XML_DOCTYPE_REJECTED is not SACM-SEC-001"
    );
    assert!(
        catalog_has_row(row, Code::XML_DOCTYPE_REJECTED),
        "{row}: docs/sacm/diagnostics.md has no table row for SACM-SEC-001"
    );
}

// --- LIB-SEC-001 6. Display and source ---------------------------------------

#[test]
fn lib_sec_001_doctype_display_is_one_located_line_without_source() {
    let row = "LIB-SEC-001";
    let error = assert_doctype_at(row, b"<?xml version=\"1.0\"?>\n<!DOCTYPE r>\n<r/>", 2, 1);
    assert_eq!(
        error.to_string(),
        "the input has a DOCTYPE, which is refused at line 2, column 1",
        "{row}: unexpected Doctype Display"
    );
    assert_one_lowercase_line(row, &error.to_string());
    assert!(error.source().is_none(), "{row}: Doctype has a source");

    let indented = assert_doctype_at(row, b"\n\n   <!DOCTYPE r SYSTEM \"r.dtd\">\n<r/>", 3, 4);
    assert_eq!(
        indented.to_string(),
        "the input has a DOCTYPE, which is refused at line 3, column 4",
        "{row}: unexpected Doctype Display"
    );
}

// --- LIB-SEC-002 1. Truncated input ------------------------------------------

#[test]
fn lib_sec_002_every_proper_prefix_of_a_document_is_malformed() {
    let row = "LIB-SEC-002";
    let sample = SAMPLE.as_bytes();
    for len in 0..sample.len() {
        let prefix = &sample[..len];
        let error = assert_malformed(row, prefix);
        let end = end_of(row, prefix);
        match error.location() {
            Some(location) => assert!(
                location <= end,
                "{row}: the prefix of {len} bytes is located at {location}, after its end at {end}"
            ),
            None => assert!(
                len == 0,
                "{row}: the prefix of {len} bytes, cut inside the document, is not located"
            ),
        }
    }
}

// --- LIB-SEC-002 2. Invalid UTF-8 --------------------------------------------

#[test]
fn lib_sec_002_invalid_utf8_is_malformed_at_the_first_invalid_byte() {
    let row = "LIB-SEC-002";
    let cases: [(&[u8], u32, u32); 6] = [
        // A lone 0xFF on line 1.
        (b"<r>\xFFok</r>", 1, 4),
        // An overlong encoding after a two-byte character, in an attribute.
        (b"<r a=\"\xC3\xA9\xC0\xAF\"/>", 1, 8),
        // An encoded surrogate.
        (b"<r>\xED\xA0\x80</r>", 1, 4),
        // A lead byte with a bad continuation, after a character, on line 2.
        (b"<?xml version=\"1.0\"?>\n<r>\xC3\xA9\xC3\x28</r>", 2, 5),
        // An invalid byte in an attribute on line 2.
        (b"<r>\n  <c a=\"\xFE\"/>\n</r>", 2, 9),
        // A sequence truncated by the end of the input, on line 2.
        (b"<r>\n\xE2\x82", 2, 1),
    ];
    for (input, line, column) in cases {
        let error = assert_malformed(row, input);
        assert_eq!(
            error.location(),
            Some(loc(row, line, column)),
            "{row}: invalid UTF-8 in {} is not located at its first invalid byte",
            preview(input)
        );
    }
}

// --- LIB-SEC-002 3. Kinds of malformed XML -----------------------------------

#[test]
fn lib_sec_002_each_kind_of_malformed_xml_is_refused_with_a_location() {
    let row = "LIB-SEC-002";
    // Each input, and the line of the offending construct where the parser
    // can know it.
    let cases: [(&str, &[u8], Option<u32>); 8] = [
        (
            "a mismatched end tag",
            b"<?xml version=\"1.0\"?>\n<zqxopen></zqxclose>",
            Some(2),
        ),
        (
            "an unclosed element",
            b"<?xml version=\"1.0\"?>\n<r>\n  <c/>\n",
            None,
        ),
        (
            "two root elements",
            b"<?xml version=\"1.0\"?>\n<r/>\n<s/>",
            Some(3),
        ),
        ("text only", b"just text", None),
        (
            "an undeclared namespace prefix",
            b"<?xml version=\"1.0\"?>\n<zqxprefix:r/>",
            Some(2),
        ),
        (
            "an undeclared entity reference",
            b"<?xml version=\"1.0\"?>\n<r>&zqxentity;</r>",
            Some(2),
        ),
        (
            "a duplicate attribute",
            b"<?xml version=\"1.0\"?>\n<r zqxattr=\"1\" zqxattr=\"2\"/>",
            Some(2),
        ),
        (
            "an attribute duplicated through namespaces",
            b"<?xml version=\"1.0\"?>\n\
              <r xmlns:p=\"urn:x\" xmlns:q=\"urn:x\" p:zqxattr=\"1\" q:zqxattr=\"2\"/>",
            Some(2),
        ),
    ];
    for (what, input, line) in cases {
        let error = assert_malformed(row, input);
        let Some(location) = error.location() else {
            panic!("{row}: {what} is refused without a location");
        };
        assert!(
            location <= end_of(row, input),
            "{row}: {what} is located at {location}, past the end of the input"
        );
        if let Some(line) = line {
            assert_eq!(
                location.line(),
                line,
                "{row}: {what} is located on the wrong line"
            );
        }
    }
}

// --- LIB-SEC-002. Lines end as XML 1.0 §2.11 ends them ------------------------
//
// LF, CRLF and a lone CR each end exactly one line, for every refusal.

#[test]
fn lib_sec_002_doctype_location_counts_lf_crlf_and_lone_cr_as_one_line_each() {
    let row = "LIB-SEC-002";
    let cases: [(&[u8], u32, u32); 5] = [
        (b"<!--p-->\r<!DOCTYPE r><r/>", 2, 1),
        (b"<!--p-->\r\n<!DOCTYPE r><r/>", 2, 1),
        (b"<!--p-->\r  <!DOCTYPE r><r/>", 2, 3),
        (b"<!--a-->\r\n<!--b-->\r<!DOCTYPE r><r/>", 3, 1),
        (b"<!--a-->\r\r\n\n<!DOCTYPE r><r/>", 4, 1),
    ];
    for (input, line, column) in cases {
        assert_doctype_at(row, input, line, column);
    }
}

#[test]
fn lib_sec_002_parser_error_location_counts_lf_crlf_and_lone_cr_as_one_line_each() {
    let row = "LIB-SEC-002";
    // A second root element: the parser stops at its `<`.
    let exact: [(&[u8], u32, u32); 4] = [
        (b"<r/>\r<s/>", 2, 1),
        (b"<r/>\r\n<s/>", 2, 1),
        (b"<r/>\r\r\n  <s/>", 3, 3),
        (b"<!--a-->\r\n<!--b-->\r<!--c-->\n<r/><s/>", 4, 5),
    ];
    for (input, line, column) in exact {
        let error = assert_malformed(row, input);
        assert_eq!(
            error.location(),
            Some(loc(row, line, column)),
            "{row}: the second root in {} is not located at line {line}, column {column}",
            preview(input)
        );
    }
    // Other parser errors on the line after a lone CR or a CRLF.
    let on_line: [(&[u8], u32); 4] = [
        (b"<r>\r</s>", 2),
        (b"<r>\r\n</s>", 2),
        (b"<r>\r&zqxentity;</r>", 2),
        (b"<r\ra=\"1\"\r\na=\"2\"/>", 3),
    ];
    for (input, line) in on_line {
        let error = assert_malformed(row, input);
        let Some(location) = error.location() else {
            panic!("{row}: {} is refused without a location", preview(input));
        };
        assert_eq!(
            location.line(),
            line,
            "{row}: the error in {} is located on the wrong line",
            preview(input)
        );
    }
}

#[test]
fn lib_sec_002_invalid_utf8_location_counts_lf_crlf_and_lone_cr_as_one_line_each() {
    let row = "LIB-SEC-002";
    let cases: [(&[u8], u32, u32); 4] = [
        (b"<r>\r\xFF</r>", 2, 1),
        (b"<r>\r\n\xFF</r>", 2, 1),
        (b"<r>\r ca\xFF</r>", 2, 4),
        (b"<r>\r\r\n\n\xFF</r>", 4, 1),
    ];
    for (input, line, column) in cases {
        let error = assert_malformed(row, input);
        assert_eq!(
            error.location(),
            Some(loc(row, line, column)),
            "{row}: invalid UTF-8 in {} is not located at line {line}, column {column}",
            preview(input)
        );
    }
}

#[test]
fn lib_sec_002_unsupported_root_location_counts_lf_crlf_and_lone_cr_as_one_line_each() {
    let row = "LIB-SEC-002";
    // A root that is recognised but not read yet, after each line ending.
    let roots: [(&str, u32, u32); 5] = [
        ("<!--p-->\r", 2, 1),
        ("<!--p-->\r\n", 2, 1),
        ("<!--p-->\r\r", 3, 1),
        ("<!--p-->\n\r", 3, 1),
        ("<!--p-->\r\n\r  ", 3, 3),
    ];
    for (prolog, line, column) in roots {
        let input = format!("{prolog}{ARTIFACT_ROOT}");
        let error = assert_unsupported(row, input.as_bytes());
        assert_eq!(
            error.location(),
            Some(loc(row, line, column)),
            "{row}: the root of {} is not located at line {line}, column {column}",
            preview(input.as_bytes())
        );
    }
    // A child the reader cannot read, inside a readable root, after each
    // line ending: the refusal is at the child's start tag.
    let children: [(&str, u32, u32); 4] = [
        ("\r", 2, 1),
        ("\r\n", 2, 1),
        ("\r\r\n  ", 3, 3),
        ("\n\r", 3, 1),
    ];
    for (before, line, column) in children {
        let input = sacm_document("", &format!("{before}<note/>"));
        let error = assert_unsupported(row, &input);
        assert_eq!(
            error.location(),
            Some(loc(row, line, column)),
            "{row}: the child of {} is not located at line {line}, column {column}",
            preview(&input)
        );
    }
}

#[test]
fn lib_sec_002_leading_utf8_bom_takes_no_column() {
    let row = "LIB-SEC-002";
    // A root that is recognised but not read yet, on line 1 and after a
    // declaration. (`<r/>` after a BOM is SACM-XMI-001: `xmi_roots.rs`.)
    for (prolog, line, column) in [
        ("\u{FEFF}", 1, 1),
        ("\u{FEFF}<!--p-->", 1, 9),
        ("\u{FEFF}<?xml version=\"1.0\"?>\n", 2, 1),
    ] {
        let input = format!("{prolog}{ARTIFACT_ROOT}");
        assert!(
            input.as_bytes().starts_with(b"\xEF\xBB\xBF"),
            "{row}: the test input does not start with a UTF-8 BOM"
        );
        let error = assert_unsupported(row, input.as_bytes());
        assert_eq!(
            error.location(),
            Some(loc(row, line, column)),
            "{row}: the root of {} is not located at line {line}, column {column}",
            preview(input.as_bytes())
        );
    }
    // A declared encoding, a parser error and invalid UTF-8 on line 1 after
    // the BOM.
    let encoding = assert_unsupported(
        row,
        b"\xEF\xBB\xBF<?xml version=\"1.0\" encoding=\"ISO-8859-1\"?><r/>",
    );
    assert_eq!(
        encoding.location(),
        Some(loc(row, 1, 31)),
        "{row}: the encoding name after a BOM is not at line 1, column 31"
    );
    let parser = assert_malformed(row, b"\xEF\xBB\xBF<r></s>");
    assert_eq!(
        parser.location(),
        Some(loc(row, 1, 4)),
        "{row}: the mismatched end tag after a BOM is not at line 1, column 4"
    );
    let invalid = assert_malformed(row, b"\xEF\xBB\xBF<r>\xFF</r>");
    assert_eq!(
        invalid.location(),
        Some(loc(row, 1, 4)),
        "{row}: the invalid byte after a BOM is not at line 1, column 4"
    );
}

// --- LIB-SEC-002 4. No panic -------------------------------------------------

#[test]
fn lib_sec_002_prefixes_and_byte_replacements_never_panic() {
    let row = "LIB-SEC-002";
    let mut count = 0_usize;
    for input in hostile_sweep() {
        // Reaching the next iteration is the assertion: the call returned.
        let _ = read(row, &input);
        count += 1;
    }
    assert_eq!(
        count,
        SAMPLE.len() * (1 + REPLACEMENTS.len()),
        "LIB-SEC-002: the sweep did not cover every prefix and replacement"
    );
}

#[test]
fn lib_sec_002_deep_nesting_returns_without_recursion() {
    // A thread with an explicit 2 MiB stack, so the result does not depend
    // on RUST_MIN_STACK. 100,000 recursive frames would not fit in it; a
    // stack overflow aborts the test binary, which fails the run.
    const DEPTH: usize = 100_000;
    let well_formed = format!("{}{}", "<a>".repeat(DEPTH), "</a>".repeat(DEPTH)).into_bytes();
    let unclosed = "<a>".repeat(DEPTH).into_bytes();
    // The reader runs on that thread directly; the result comes back on a
    // channel, so a read that never returns fails at the deadline.
    let (sender, receiver) = mpsc::channel();
    std::thread::Builder::new()
        .name("lib_sec_002_deep_nesting".to_owned())
        .stack_size(2 * MIB)
        .spawn(move || {
            let options = ReadOptions::default();
            let results = (
                xmi::read(&well_formed, &options).is_ok(),
                xmi::read(&unclosed, &options),
            );
            // The receiver is gone only after the deadline has already failed the test.
            let _ = sender.send(results);
        })
        .unwrap_or_else(|error| panic!("LIB-SEC-002: cannot spawn the reader thread: {error}"));
    let (_well_formed_read, unclosed_result) = match receiver.recv_timeout(DEADLINE) {
        Ok(results) => results,
        Err(RecvTimeoutError::Timeout) => {
            panic!(
                "LIB-SEC-002: read did not return within {DEADLINE:?} on 100,000 levels of nesting"
            )
        }
        Err(RecvTimeoutError::Disconnected) => {
            panic!("LIB-SEC-002: read panicked on 100,000 levels of nesting")
        }
    };
    assert!(
        unclosed_result.is_err(),
        "LIB-SEC-002: 100,000 unclosed elements were read as a document: {unclosed_result:?}"
    );
}

#[test]
fn lib_sec_002_empty_whitespace_and_bom_only_input_return_a_value() {
    let row = "LIB-SEC-002";
    // Not well-formed: no root element.
    for input in [&b""[..], b"   \n\t\r\n ", b"\xEF\xBB\xBF"] {
        assert_malformed(row, input);
    }
    // Byte order marks alone: Malformed for UTF-16 (SACM23-XMI-018),
    // Unsupported for UTF-32 (LIB-API-009); any refusal will do here.
    for input in [
        &b"\xFF\xFE"[..],
        b"\xFE\xFF",
        b"\xFF\xFE\x00\x00",
        b"\x00\x00\xFE\xFF",
    ] {
        assert!(
            read(row, input).is_err(),
            "LIB-SEC-002: the byte order mark {input:?} alone was read as a document"
        );
    }
}

// --- LIB-SEC-002 5. The Malformed Display ------------------------------------

#[test]
fn lib_sec_002_malformed_display_is_one_located_line() {
    let row = "LIB-SEC-002";
    let input = b"<?xml version=\"1.0\"?>\n<r></s>";
    let error = assert_malformed(row, input);
    let Some(location) = error.location() else {
        panic!("{row}: a mismatched end tag is not located");
    };
    let text = error.to_string();
    let head = format!(
        "the input is not well-formed XML at line {}, column {}: ",
        location.line(),
        location.column()
    );
    assert!(
        text.starts_with(&head),
        "{row}: Display {text:?} does not start with {head:?}"
    );
    let reason = display_reason(row, &error, MALFORMED_LEAD);
    assert!(
        !states_a_position(&reason),
        "{row}: the reason {reason:?} states a position of its own"
    );
    assert_eq!(
        text,
        "the input is not well-formed XML at line 2, column 4: \
         the end tag \"s\" does not match the start tag \"r\"",
        "{row}: unexpected Malformed Display for a mismatched end tag"
    );
}

#[test]
fn lib_sec_002_malformed_display_quotes_names_and_never_breaks_the_line() {
    let row = "LIB-SEC-002";
    let cases: [(&[u8], &[&str]); 8] = [
        (b"<zqxopen>\n</zqxclose\n>", &["zqxopen", "zqxclose"]),
        (b"<zqxprefix:r/>", &["zqxprefix"]),
        (b"<r>&zqxentity;</r>", &["zqxentity"]),
        (b"<r zqxattr=\"1\"\r\nzqxattr=\"2\"/>", &["zqxattr"]),
        (b"<r>\r\n&zqx\r\nentity;</r>", &[]),
        ("<r a=\"1\"\u{2028}/>".as_bytes(), &[]),
        ("<zqxr>\u{85}</zqxs>".as_bytes(), &["zqxr", "zqxs"]),
        ("<r\u{2029}/>\n<!-- a -- b -->".as_bytes(), &[]),
    ];
    for (input, names) in cases {
        let error = assert_malformed(row, input);
        let reason = display_reason(row, &error, MALFORMED_LEAD);
        assert!(
            !states_a_position(&reason),
            "{row}: the reason {reason:?} states a position of its own"
        );
        let text = error.to_string();
        for name in names {
            assert_quoted_wherever_named(row, &text, name);
        }
    }
}

#[test]
fn lib_sec_002_every_refusal_in_the_sweep_has_its_display_code_and_no_source() {
    let row = "LIB-SEC-002";
    for input in hostile_sweep() {
        if let Err(error) = read(row, &input) {
            assert_refusal_invariants(row, &input, &error);
        }
    }
}

// --- LIB-SEC-002 6. Traits ---------------------------------------------------

fn assert_send_sync_static<T: Send + Sync + 'static>() {}
fn assert_error_traits<T: Clone + Eq + Hash + Debug + Display + Error>() {}
fn assert_options_traits<T: Clone + Debug + Default + PartialEq + Eq + Hash>() {}
fn assert_output_traits<T: Clone + Debug>() {}

#[test]
fn lib_sec_002_read_types_are_send_sync_static_with_the_error_traits() {
    assert_send_sync_static::<ReadError>();
    assert_send_sync_static::<ReadOptions>();
    assert_send_sync_static::<ReadOutput>();
    assert_error_traits::<ReadError>();
    // As designed in ADR 0015 and the slice brief.
    assert_options_traits::<ReadOptions>();
    assert_output_traits::<ReadOutput>();
}

#[test]
fn lib_sec_002_each_refusal_is_clone_eq_and_deterministic_without_source() {
    let row = "LIB-SEC-002";
    let inputs: [&[u8]; 3] = [
        b"<!DOCTYPE r>\n<r/>",
        b"<r></s>",
        b"<?xml version=\"1.0\"?>\n<r/>",
    ];
    for input in inputs {
        let error = refusal(row, input);
        assert_eq!(
            error.clone(),
            error,
            "{row}: a clone of {error:?} is not equal to it"
        );
        assert!(error.source().is_none(), "{row}: {error:?} has a source");
        assert_eq!(
            refusal(row, input),
            error,
            "{row}: reading {} twice gave different refusals",
            preview(input)
        );
    }
}

// --- LIB-SEC-002 7. The code -------------------------------------------------

#[test]
fn lib_sec_002_xml_malformed_code_is_sacm_xml_001_in_the_catalog() {
    let row = "LIB-SEC-002";
    assert_eq!(
        Code::XML_MALFORMED.as_str(),
        "SACM-XML-001",
        "{row}: Code::XML_MALFORMED is not SACM-XML-001"
    );
    assert!(
        catalog_has_row(row, Code::XML_MALFORMED),
        "{row}: docs/sacm/diagnostics.md has no table row for SACM-XML-001"
    );
}

// --- LIB-API-009 1. Well-formed input is refused at its root -----------------

#[test]
fn lib_api_009_well_formed_sacm_document_is_unsupported_at_the_construct_it_cannot_read() {
    let row = "LIB-API-009";
    // A readable root whose name's `content` value element holds an element
    // child, which no row reads (a value element holds only character
    // content, SACM23-XMI-014; the legacy `value` form is LIB-COMPAT-008):
    // refused at the `value` start tag, on line 4. `  <name><content>` is 17
    // characters.
    let name_child = sacm_document(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!-- LIB-API-009 -->\n",
        "\n  <name><content><value>case</value></content></name>\n",
    );
    let error = assert_unsupported(row, &name_child);
    assert_eq!(
        error.location(),
        Some(loc(row, 4, 18)),
        "{row}: the refusal is not located at the start tag of the `value` child of `content`"
    );
    // An AssuranceCasePackage root with a `note` child, which no verified row
    // reads: `Unsupported`, with no code and no document, at `<note` (line
    // 4, column 3).
    let note_child = sacm_document(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!-- LIB-API-009 -->\n",
        "\n  <note/>\n",
    );
    let error = assert_unsupported(row, &note_child);
    assert_eq!(
        error.location(),
        Some(loc(row, 4, 3)),
        "{row}: the refusal is not located at the start tag of the `note` child"
    );
    // A root that is recognised but not read yet: refused at its start tag,
    // on line 3.
    let artifact_root = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!-- LIB-API-009 -->\n{ARTIFACT_ROOT}"
    );
    let error = assert_unsupported(row, artifact_root.as_bytes());
    assert_eq!(
        error.location(),
        Some(loc(row, 3, 1)),
        "{row}: the refusal is not located at the root element's start tag"
    );
    // `<?xml version="1.0"?>` is 21 characters and `<!-- é -->` 10: the
    // column counts characters, not bytes.
    let after_comment = format!("<?xml version=\"1.0\"?><!-- é -->{ARTIFACT_ROOT}");
    let error = assert_unsupported(row, after_comment.as_bytes());
    assert_eq!(
        error.location(),
        Some(loc(row, 1, 32)),
        "{row}: the refusal is not located at the root element's start tag"
    );
    // The full sample, too. Its `argumentPackage` child and the Claim `c_1`
    // in it are read (SACM23-XMI-016), so the first content it cannot read
    // is `c_1`'s `gid` attribute (SACM23-BASE-006), refused at the first
    // character of its name: line 5, column 57.
    let error = assert_unsupported(row, SAMPLE.as_bytes());
    assert_eq!(
        error.location(),
        Some(loc(row, 5, 57)),
        "{row}: the sample is not refused at the `gid` of `c_1`"
    );
}

#[test]
fn lib_api_009_well_formed_non_sacm_document_is_unsupported_at_its_root() {
    let row = "LIB-API-009";
    // A package-kind name outside the SACM namespace (ADR 0016 D6). Roots
    // that are not package kinds at all (`<html>`, `<r/>`) are SACM-XMI-001,
    // at the same locations: `xmi_roots.rs`.
    let input = b"<?xml version=\"1.0\"?>\n\n  <foo:ArgumentPackage xmlns:foo=\"urn:x\" \
        xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" xmi:id=\"p\"/>";
    let error = assert_unsupported(row, input);
    assert_eq!(
        error.location(),
        Some(loc(row, 3, 3)),
        "{row}: the refusal is not located at the root element's start tag"
    );
    let bare = assert_unsupported(
        row,
        b"<ArgumentPackage xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" xmi:id=\"p\"/>",
    );
    assert_eq!(
        bare.location(),
        Some(loc(row, 1, 1)),
        "{row}: the refusal is not located at the root element's start tag"
    );
}

// --- LIB-API-009 2. UTF-32, and UTF-16 or UCS-4 without a BOM ----------------
//
// UTF-16 with a byte order mark is read since SACM23-XMI-018: `xmi_utf16.rs`.

#[test]
fn lib_api_009_utf32_input_with_a_bom_is_unsupported() {
    let row = "LIB-API-009";
    let utf32_text =
        format!("<?xml version=\"1.0\" encoding=\"UTF-32\"?>\n{SACM_ROOT_OPEN}{SACM_ROOT_CLOSE}");
    let inputs = [
        ("UTF-32 LE", utf32_with_bom(&utf32_text, false)),
        ("UTF-32 BE", utf32_with_bom(&utf32_text, true)),
        ("UTF-32 LE BOM only", vec![0xFF, 0xFE, 0x00, 0x00]),
        ("UTF-32 BE BOM only", vec![0x00, 0x00, 0xFE, 0xFF]),
    ];
    for (what, input) in inputs {
        let error = assert_unsupported(row, &input);
        assert!(
            error.code().is_none(),
            "{row}: the {what} refusal carries a code"
        );
        assert_refusal_invariants(row, &input, &error);
    }
}

/// `text` encoded as UTF-16 without a byte order mark.
fn utf16_without_bom(text: &str, big_endian: bool) -> Vec<u8> {
    let mut bytes = Vec::new();
    for unit in text.encode_utf16() {
        let pair = if big_endian {
            unit.to_be_bytes()
        } else {
            unit.to_le_bytes()
        };
        bytes.extend_from_slice(&pair);
    }
    bytes
}

/// `text` encoded as UCS-4 without a byte order mark, its four bytes in
/// `order`: each entry is the index of a big-endian byte, so `[0, 1, 2, 3]`
/// is big-endian (1234) and `[3, 2, 1, 0]` little-endian (4321).
fn ucs4_without_bom(text: &str, order: [usize; 4]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for c in text.chars() {
        let big = u32::from(c).to_be_bytes();
        bytes.extend(order.iter().map(|&index| big[index]));
    }
    bytes
}

#[test]
fn lib_api_009_utf16_and_ucs4_without_bom_are_unsupported_by_their_first_bytes() {
    let row = "LIB-API-009";
    let document = |encoding: &str| {
        format!(
            "<?xml version=\"1.0\" encoding=\"{encoding}\"?>\n{SACM_ROOT_OPEN}{SACM_ROOT_CLOSE}"
        )
    };
    // XML 1.0 Appendix F: the first four bytes of `<?xml` in each encoding.
    let inputs: [(&str, [u8; 4], Vec<u8>); 6] = [
        (
            "UTF-16LE",
            [0x3C, 0x00, 0x3F, 0x00],
            utf16_without_bom(&document("UTF-16LE"), false),
        ),
        (
            "UTF-16BE",
            [0x00, 0x3C, 0x00, 0x3F],
            utf16_without_bom(&document("UTF-16BE"), true),
        ),
        (
            "UCS-4 4321",
            [0x3C, 0x00, 0x00, 0x00],
            ucs4_without_bom(&document("ISO-10646-UCS-4"), [3, 2, 1, 0]),
        ),
        (
            "UCS-4 1234",
            [0x00, 0x00, 0x00, 0x3C],
            ucs4_without_bom(&document("ISO-10646-UCS-4"), [0, 1, 2, 3]),
        ),
        (
            "UCS-4 2143",
            [0x00, 0x00, 0x3C, 0x00],
            ucs4_without_bom(&document("ISO-10646-UCS-4"), [1, 0, 3, 2]),
        ),
        (
            "UCS-4 3412",
            [0x00, 0x3C, 0x00, 0x00],
            ucs4_without_bom(&document("ISO-10646-UCS-4"), [2, 3, 0, 1]),
        ),
    ];
    for (what, first_bytes, input) in inputs {
        assert_eq!(
            input.get(..4),
            Some(&first_bytes[..]),
            "{row}: the {what} test input does not start with its Appendix F bytes"
        );
        let result = read(row, &input);
        assert!(
            matches!(result, Err(ReadError::Unsupported { .. })),
            "{row}: BOM-less {what} input was not refused as Unsupported: {result:?}"
        );
        let error = assert_unsupported(row, &input);
        assert_refusal_invariants(row, &input, &error);
        let text = error.to_string().to_lowercase();
        for claim in ["bom", "byte order mark", "byte-order mark"] {
            assert!(
                !text.contains(claim),
                "{row}: the refusal of BOM-less {what} input claims a byte order mark: {text:?}"
            );
        }
    }
}

// --- LIB-API-009 3. Declared encodings ---------------------------------------

#[test]
fn lib_api_009_declared_non_utf8_encoding_is_unsupported() {
    let row = "LIB-API-009";
    // Every byte is ASCII, so the input is valid UTF-8 too.
    let declarations = [
        "<?xml version=\"1.0\" encoding=\"ISO-8859-1\"?>",
        "<?xml version=\"1.0\" encoding=\"iso-8859-1\"?>",
        "<?xml version=\"1.0\" encoding='windows-1252'?>",
        "<?xml version=\"1.0\" encoding = \"Shift_JIS\" standalone=\"yes\"?>",
    ];
    for declaration in declarations {
        let input = sacm_document(&format!("{declaration}\n"), "");
        assert_unsupported(row, &input);
    }
}

#[test]
fn lib_api_009_utf8_and_ascii_declarations_in_any_case_are_not_refused() {
    let row = "LIB-API-009";
    let utf8 = ["UTF-8", "utf-8", "Utf-8"];
    let ascii = ["US-ASCII", "us-ascii", "ASCII", "ascii"];
    // A readable root under each declaration reads `Ok`.
    for name in utf8 {
        // UTF-8 content beyond ASCII is fine under a UTF-8 declaration.
        let input = sacm_document(
            &format!("<?xml version=\"1.0\" encoding=\"{name}\"?>\n<!-- café -->\n"),
            "",
        );
        let result = read(row, &input);
        assert!(
            result.is_ok(),
            "{row}: a readable root under encoding {name:?} was refused: {result:?}"
        );
    }
    for name in ascii {
        let input = sacm_document(&format!("<?xml version='1.0' encoding='{name}'?>\n"), "");
        let result = read(row, &input);
        assert!(
            result.is_ok(),
            "{row}: a readable root under encoding {name:?} was refused: {result:?}"
        );
    }
}

#[test]
fn lib_api_009_non_ascii_byte_under_ascii_declaration_is_malformed() {
    let row = "LIB-API-009";
    for name in ["US-ASCII", "ascii"] {
        // `é` is valid UTF-8 but not ASCII.
        let input = format!("<?xml version=\"1.0\" encoding=\"{name}\"?>\n<r>café</r>");
        let error = assert_malformed(row, input.as_bytes());
        assert_eq!(
            error.location(),
            Some(loc(row, 2, 7)),
            "{row}: `é` under {name:?} is not located at that character"
        );
    }
}

#[test]
fn lib_api_009_utf8_and_ascii_declarations_in_any_case_are_refused_at_the_root_not_the_encoding() {
    let row = "LIB-API-009";
    let names = [
        "UTF-8", "utf-8", "Utf-8", "US-ASCII", "us-ascii", "ASCII", "ascii",
    ];
    for name in names {
        // The root start tag of a root that is not read yet is at line 2,
        // column 1; the encoding name at line 1, column 31.
        let input = format!("<?xml version=\"1.0\" encoding=\"{name}\"?>\n{ARTIFACT_ROOT}");
        let error = assert_unsupported(row, input.as_bytes());
        assert_eq!(
            error.location(),
            Some(loc(row, 2, 1)),
            "{row}: a declaration of {name:?} is refused away from the root start tag: {error:?}"
        );
    }
}

#[test]
fn lib_api_009_declared_non_utf8_encoding_is_located_at_the_encoding_name() {
    let row = "LIB-API-009";
    // `<?xml version="1.0" ` is 20 characters and `encoding="` 10, so the
    // name starts at column 31.
    let cases: [(&str, &[u8], u32, u32); 6] = [
        (
            "ISO-8859-1",
            b"<?xml version=\"1.0\" encoding=\"ISO-8859-1\"?><r/>",
            1,
            31,
        ),
        (
            "ISO-8859-1 with a 0xE9 byte",
            b"<?xml version=\"1.0\" encoding=\"ISO-8859-1\"?><r>caf\xE9</r>",
            1,
            31,
        ),
        (
            "single-quoted windows-1252",
            b"<?xml version=\"1.0\" encoding='windows-1252'?><r/>",
            1,
            31,
        ),
        (
            "Shift_JIS with spaces around `=`",
            b"<?xml version=\"1.0\" encoding = \"Shift_JIS\"?><r/>",
            1,
            33,
        ),
        (
            "a lone CR before `encoding`",
            b"<?xml version=\"1.0\"\rencoding=\"ISO-8859-1\"?><r/>",
            2,
            11,
        ),
        (
            "a CRLF and two spaces before `encoding`",
            b"<?xml version=\"1.0\"\r\n  encoding=\"ISO-8859-1\"?><r/>",
            2,
            13,
        ),
    ];
    for (what, input, line, column) in cases {
        let error = assert_unsupported(row, input);
        assert_eq!(
            error.location(),
            Some(loc(row, line, column)),
            "{row}: {what}: the refusal is not located at the encoding name"
        );
    }
}

#[test]
fn lib_api_009_bom_and_first_bytes_refusals_are_located_at_line_1_column_1() {
    let row = "LIB-API-009";
    let text =
        format!("<?xml version=\"1.0\" encoding=\"UTF-16\"?>\n{SACM_ROOT_OPEN}{SACM_ROOT_CLOSE}");
    // UTF-16 with a byte order mark is read since SACM23-XMI-018, and a
    // UTF-16 mark alone is Malformed at 1:1 there (`xmi_utf16.rs`).
    let inputs = [
        ("UTF-32 LE with a BOM", utf32_with_bom(&text, false)),
        ("UTF-32 BE with a BOM", utf32_with_bom(&text, true)),
        ("UTF-32 LE BOM only", vec![0xFF, 0xFE, 0x00, 0x00]),
        ("UTF-32 BE BOM only", vec![0x00, 0x00, 0xFE, 0xFF]),
        ("BOM-less UTF-16LE", utf16_without_bom(&text, false)),
        ("BOM-less UTF-16BE", utf16_without_bom(&text, true)),
        ("BOM-less UCS-4 4321", ucs4_without_bom(&text, [3, 2, 1, 0])),
        ("BOM-less UCS-4 1234", ucs4_without_bom(&text, [0, 1, 2, 3])),
        ("BOM-less UCS-4 2143", ucs4_without_bom(&text, [1, 0, 3, 2])),
        ("BOM-less UCS-4 3412", ucs4_without_bom(&text, [2, 3, 0, 1])),
    ];
    for (what, input) in inputs {
        let error = assert_unsupported(row, &input);
        assert_eq!(
            error.location(),
            Some(loc(row, 1, 1)),
            "{row}: the {what} refusal is not located at line 1, column 1"
        );
    }
}

#[test]
fn lib_api_009_declared_non_utf8_encoding_with_invalid_utf8_bytes_is_unsupported() {
    let row = "LIB-API-009";
    // Real single-byte files: 0xE9 is `é` in ISO-8859-1 and 0x93 a quotation
    // mark in windows-1252; neither is valid UTF-8. The declaration is read
    // before the UTF-8 check, so the encoding is what is refused.
    let inputs: [(&str, &[u8]); 5] = [
        (
            "0xE9 in content",
            b"<?xml version=\"1.0\" encoding=\"ISO-8859-1\"?><r>caf\xE9</r>",
        ),
        (
            "0xE9 just after the declaration's end",
            b"<?xml version=\"1.0\" encoding=\"ISO-8859-1\"?><!--\xE9--><r/>",
        ),
        (
            "0xE9 before the declaration's end",
            b"<?xml version=\"1.0\" encoding=\"ISO-8859-1\" \xE9?><r/>",
        ),
        (
            "0xE9 on a later line, lowercase name",
            b"<?xml version='1.0' encoding='iso-8859-1'?>\n<r>\n  caf\xE9\n</r>",
        ),
        (
            "0x93 under windows-1252",
            b"<?xml version=\"1.0\" encoding=\"windows-1252\"?>\n<r>\x93quoted\x94</r>",
        ),
    ];
    for (what, input) in inputs {
        let result = read(row, input);
        assert!(
            matches!(result, Err(ReadError::Unsupported { .. })),
            "{row}: {what}: expected Unsupported, got {result:?}"
        );
        assert_unsupported(row, input);
    }
}

#[test]
fn lib_api_009_invalid_utf8_without_or_under_a_utf8_or_ascii_declaration_stays_malformed() {
    let row = "LIB-API-009";
    let cases: [(&str, &[u8], u32, u32); 4] = [
        ("no declaration", b"<r>caf\xE9</r>", 1, 7),
        (
            "a declaration without an encoding",
            b"<?xml version=\"1.0\"?>\n<r>caf\xE9</r>",
            2,
            7,
        ),
        (
            "a UTF-8 declaration",
            b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<r>caf\xE9</r>",
            2,
            7,
        ),
        (
            "an ASCII declaration",
            b"<?xml version=\"1.0\" encoding=\"US-ASCII\"?>\n<r>caf\xE9</r>",
            2,
            7,
        ),
    ];
    for (what, input, line, column) in cases {
        let error = assert_malformed(row, input);
        assert_eq!(
            error.location(),
            Some(loc(row, line, column)),
            "{row}: {what}: the invalid byte is not located at line {line}, column {column}"
        );
    }
}

// --- LIB-API-009 4. The Unsupported Display ----------------------------------

#[test]
fn lib_api_009_unsupported_display_is_one_located_line_quoting_names() {
    let row = "LIB-API-009";
    // A package-kind name outside the SACM namespace, refused at its root.
    let input = b"<?xml version=\"1.0\"?>\n<zqxroot:ArgumentPackage xmlns:zqxroot=\"urn:zqx\" \
        xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" xmi:id=\"p\"/>";
    let error = assert_unsupported(row, input);
    let text = error.to_string();
    assert!(
        text.starts_with("the reader cannot represent the input at line 2, column 1: "),
        "{row}: Display {text:?} does not name the root's location in the expected shape"
    );
    display_reason(row, &error, UNSUPPORTED_LEAD);
    assert_quoted_wherever_named(row, &text, "zqxroot");

    // A root that is recognised but not read yet.
    let artifact = format!("<?xml version=\"1.0\"?>\n{ARTIFACT_ROOT}");
    let error = assert_unsupported(row, artifact.as_bytes());
    display_reason(row, &error, UNSUPPORTED_LEAD);
    assert_quoted_wherever_named(row, &error.to_string(), "ArtifactPackage");

    // A child and an attribute that a readable root cannot carry.
    let child = sacm_document("<?xml version=\"1.0\"?>\n", "<zqxchild/>");
    let error = assert_unsupported(row, &child);
    display_reason(row, &error, UNSUPPORTED_LEAD);
    assert_quoted_wherever_named(row, &error.to_string(), "zqxchild");
    let attribute = b"<?xml version=\"1.0\"?>\n<sacm:ArgumentPackage \
        xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\" \
        xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" xmi:id=\"p\" zqxattr=\"1\"/>";
    let error = assert_unsupported(row, attribute);
    display_reason(row, &error, UNSUPPORTED_LEAD);
    assert_quoted_wherever_named(row, &error.to_string(), "zqxattr");

    let encoding = sacm_document("<?xml version=\"1.0\" encoding=\"x-zqx-enc\"?>\n", "");
    let error = assert_unsupported(row, &encoding);
    display_reason(row, &error, UNSUPPORTED_LEAD);
    assert_quoted_wherever_named(row, &error.to_string(), "x-zqx-enc");
}

#[test]
fn lib_api_009_refusal_of_an_encoding_name_with_line_breaks_stays_one_line() {
    let row = "LIB-API-009";
    let names = ["ISO\u{2028}8859", "x\ny", "x\r\u{85}y"];
    for name in names {
        let input = sacm_document(
            &format!("<?xml version=\"1.0\" encoding=\"{name}\"?>\n"),
            "",
        );
        let error = refusal(row, &input);
        assert_refusal_invariants(row, &input, &error);
    }
}

#[test]
fn lib_sec_002_a_deep_readable_package_chain_reads_without_recursion() {
    // A chain the reader accepts, so the content reader (not only the
    // parser) walks all of it: a root ArgumentPackage `p0`, and in each
    // package's `argumentElement` the next one, down to `p100000`. The
    // thread has an explicit 2 MiB stack, as in
    // `lib_sec_002_deep_nesting_returns_without_recursion`; a stack overflow
    // aborts the test binary, which fails the run. Each document is dropped
    // on that thread, so a recursive drop would overflow there too.
    const DEPTH: usize = 100_000;
    /// Longer than `DEADLINE`, as this reads 100,001 elements twice (about
    /// 4 s in a debug build), but still below cargo-mutants' minimum test
    /// timeout of 20 seconds.
    const CHAIN_DEADLINE: Duration = Duration::from_secs(15);
    let mut input = String::from(concat!(
        "<sacm:ArgumentPackage",
        " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
        " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
        " xmi:id=\"p0\">",
    ));
    for n in 1..=DEPTH {
        input.push_str("<argumentElement xmi:type=\"sacm:ArgumentPackage\" xmi:id=\"p");
        input.push_str(&n.to_string());
        input.push_str("\">");
    }
    input.push_str(&"</argumentElement>".repeat(DEPTH));
    input.push_str("</sacm:ArgumentPackage>");
    let input = input.into_bytes();
    let deepest = format!("p{DEPTH}");
    let above = format!("p{}", DEPTH - 1);

    let (sender, receiver) = mpsc::channel();
    thread::Builder::new()
        .name("lib_sec_002_deep_readable_chain".to_owned())
        .stack_size(2 * MIB)
        .spawn(move || {
            let mut results = Vec::new();
            for mode in [Mode::Tolerant, Mode::Strict] {
                let result =
                    xmi::read(&input, &ReadOptions::default().with_mode(mode)).map(|output| {
                        let document = output.into_document();
                        let roots: Vec<String> =
                            document.roots().map(|root| root.id().to_string()).collect();
                        let parent = document
                            .get(deepest.as_str())
                            .and_then(sacm::model::SacmElement::parent)
                            .map(|parent| parent.id().to_string());
                        let summary = (document.element_count(), roots, parent);
                        // The document drops here, on the 2 MiB thread.
                        drop(document);
                        summary
                    });
                results.push((mode, result));
            }
            // The receiver is gone only after the deadline has already failed the test.
            let _ = sender.send(results);
        })
        .unwrap_or_else(|error| panic!("LIB-SEC-002: cannot spawn the reader thread: {error}"));
    let results = match receiver.recv_timeout(CHAIN_DEADLINE) {
        Ok(results) => results,
        Err(RecvTimeoutError::Timeout) => panic!(
            "LIB-SEC-002: read did not return within {CHAIN_DEADLINE:?} on a 100,000-deep package chain"
        ),
        Err(RecvTimeoutError::Disconnected) => {
            panic!("LIB-SEC-002: read or drop panicked on a 100,000-deep package chain")
        }
    };
    assert_eq!(results.len(), 2, "LIB-SEC-002: not both modes were read");
    for (mode, result) in results {
        let (count, roots, parent) = result.unwrap_or_else(|error| {
            panic!("LIB-SEC-002: {mode:?}: the 100,000-deep package chain was refused: {error}")
        });
        assert_eq!(
            count,
            DEPTH + 1,
            "LIB-SEC-002: {mode:?}: the document does not hold every package of the chain"
        );
        assert_eq!(roots, ["p0"], "LIB-SEC-002: {mode:?}: roots");
        assert_eq!(
            parent.as_deref(),
            Some(above.as_str()),
            "LIB-SEC-002: {mode:?}: the deepest package is missing or not in its parent"
        );
    }
}
