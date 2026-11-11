//! Tests for the safety of the XMI read entry point: `sacm::xmi::read`,
//! `ReadOptions`, `ReadOutput` and `ReadError` (LIB-SEC-001, LIB-SEC-002,
//! LIB-API-009; ADR 0005, ADR 0015).
//!
//! Every input is an inline byte string. `read` is called with
//! `ReadOptions::default()`, the only options there are.
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
//! - "In every mode" (LIB-SEC-001): `ReadOptions` has no mode yet
//!   (LIB-COMPAT-001), so only the default options are exercised.

use std::error::Error;
use std::fmt::{Debug, Display};
use std::hash::Hash;
use std::path::Path;

use sacm::diagnostic::{Code, SourceLocation};
use sacm::xmi::{self, ReadError, ReadOptions, ReadOutput};

/// One mebibyte.
const MIB: usize = 1 << 20;

/// The lead of the `Doctype` `Display`, before the location.
const DOCTYPE_LEAD: &str = "refused a DOCTYPE or ENTITY declaration";
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
    "    <argumentElement xsi:type=\"sacm:Claim\" xmi:id=\"c_1\" assertionDeclaration=\"assumed\"/>\n",
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

// --- Helpers --------------------------------------------------------------

fn read(input: &[u8]) -> Result<ReadOutput, ReadError> {
    xmi::read(input, &ReadOptions::default())
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
    match read(input) {
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

/// `text` encoded as UTF-16 with a byte order mark.
fn utf16_with_bom(text: &str, big_endian: bool) -> Vec<u8> {
    let mut bytes = Vec::new();
    for unit in std::iter::once(0xFEFF_u16).chain(text.encode_utf16()) {
        let pair = if big_endian {
            unit.to_be_bytes()
        } else {
            unit.to_le_bytes()
        };
        bytes.extend_from_slice(&pair);
    }
    bytes
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
        let result = read(&input);
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
        "refused a DOCTYPE or ENTITY declaration at line 2, column 1",
        "{row}: unexpected Doctype Display"
    );
    assert_one_lowercase_line(row, &error.to_string());
    assert!(error.source().is_none(), "{row}: Doctype has a source");

    let indented = assert_doctype_at(row, b"\n\n   <!DOCTYPE r SYSTEM \"r.dtd\">\n<r/>", 3, 4);
    assert_eq!(
        indented.to_string(),
        "refused a DOCTYPE or ENTITY declaration at line 3, column 4",
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

// --- LIB-SEC-002 4. No panic -------------------------------------------------

#[test]
fn lib_sec_002_prefixes_and_byte_replacements_never_panic() {
    let mut count = 0_usize;
    for input in hostile_sweep() {
        // Reaching the next iteration is the assertion: the call returned.
        let _ = read(&input);
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
    let reader = std::thread::Builder::new()
        .name("lib_sec_002_deep_nesting".to_owned())
        .stack_size(2 * MIB)
        .spawn(move || (read(&well_formed).is_ok(), read(&unclosed)))
        .unwrap_or_else(|error| panic!("LIB-SEC-002: cannot spawn the reader thread: {error}"));
    let Ok((_well_formed_read, unclosed_result)) = reader.join() else {
        panic!("LIB-SEC-002: read panicked on 100,000 levels of nesting");
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
    // Byte order marks of encodings the reader refuses; any value will do.
    for input in [
        &b"\xFF\xFE"[..],
        b"\xFE\xFF",
        b"\xFF\xFE\x00\x00",
        b"\x00\x00\xFE\xFF",
    ] {
        assert!(
            read(input).is_err(),
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
        if let Err(error) = read(&input) {
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
fn lib_api_009_well_formed_sacm_document_is_unsupported_at_its_root() {
    let row = "LIB-API-009";
    let on_line_3 = sacm_document(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!-- LIB-API-009 -->\n",
        "<argumentPackage xmi:id=\"arg_1\"/>",
    );
    let error = assert_unsupported(row, &on_line_3);
    assert_eq!(
        error.location(),
        Some(loc(row, 3, 1)),
        "{row}: the refusal is not located at the root element's start tag"
    );
    // `<?xml version="1.0"?>` is 21 characters and `<!-- é -->` 10: the
    // column counts characters, not bytes.
    let after_comment = sacm_document("<?xml version=\"1.0\"?><!-- é -->", "");
    let error = assert_unsupported(row, &after_comment);
    assert_eq!(
        error.location(),
        Some(loc(row, 1, 32)),
        "{row}: the refusal is not located at the root element's start tag"
    );
    // The full sample, too.
    assert_unsupported(row, SAMPLE.as_bytes());
}

#[test]
fn lib_api_009_well_formed_non_sacm_document_is_unsupported_at_its_root() {
    let row = "LIB-API-009";
    let input =
        b"<?xml version=\"1.0\"?>\n\n  <html xmlns=\"http://www.w3.org/1999/xhtml\"><body/></html>";
    let error = assert_unsupported(row, input);
    assert_eq!(
        error.location(),
        Some(loc(row, 3, 3)),
        "{row}: the refusal is not located at the root element's start tag"
    );
    let bare = assert_unsupported(row, b"<r/>");
    assert_eq!(
        bare.location(),
        Some(loc(row, 1, 1)),
        "{row}: the refusal is not located at the root element's start tag"
    );
}

// --- LIB-API-009 2. UTF-16 and UTF-32 ----------------------------------------

#[test]
fn lib_api_009_utf16_and_utf32_input_is_unsupported() {
    let row = "LIB-API-009";
    let utf16_text =
        format!("<?xml version=\"1.0\" encoding=\"UTF-16\"?>\n{SACM_ROOT_OPEN}{SACM_ROOT_CLOSE}");
    let utf32_text =
        format!("<?xml version=\"1.0\" encoding=\"UTF-32\"?>\n{SACM_ROOT_OPEN}{SACM_ROOT_CLOSE}");
    let inputs = [
        ("UTF-16 LE", utf16_with_bom(&utf16_text, false)),
        ("UTF-16 BE", utf16_with_bom(&utf16_text, true)),
        ("UTF-32 LE", utf32_with_bom(&utf32_text, false)),
        ("UTF-32 BE", utf32_with_bom(&utf32_text, true)),
    ];
    for (what, input) in inputs {
        let error = assert_unsupported(row, &input);
        assert!(
            error.code().is_none(),
            "{row}: the {what} refusal carries a code"
        );
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
    for name in utf8 {
        // UTF-8 content beyond ASCII is fine under a UTF-8 declaration.
        let input = sacm_document(
            &format!("<?xml version=\"1.0\" encoding=\"{name}\"?>\n<!-- café -->\n"),
            "",
        );
        assert_not_doctype_or_malformed(row, &input, &read(&input));
    }
    for name in ascii {
        let input = sacm_document(&format!("<?xml version='1.0' encoding='{name}'?>\n"), "");
        assert_not_doctype_or_malformed(row, &input, &read(&input));
    }
}

#[test]
fn lib_api_009_non_ascii_byte_under_ascii_declaration_is_malformed() {
    let row = "LIB-API-009";
    for name in ["US-ASCII", "ascii"] {
        // `é` is valid UTF-8 but not ASCII.
        let input = format!("<?xml version=\"1.0\" encoding=\"{name}\"?>\n<r>café</r>");
        assert_malformed(row, input.as_bytes());
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
        let result = read(input);
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
    let input = b"<?xml version=\"1.0\"?>\n<zqxroot/>";
    let error = assert_unsupported(row, input);
    let text = error.to_string();
    assert!(
        text.starts_with("the reader cannot represent the input at line 2, column 1: "),
        "{row}: Display {text:?} does not name the root's location in the expected shape"
    );
    display_reason(row, &error, UNSUPPORTED_LEAD);
    assert_quoted_wherever_named(row, &text, "zqxroot");

    let sacm = sacm_document("<?xml version=\"1.0\"?>\n", "");
    let error = assert_unsupported(row, &sacm);
    display_reason(row, &error, UNSUPPORTED_LEAD);
    assert_quoted_wherever_named(row, &error.to_string(), "AssuranceCasePackage");

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
