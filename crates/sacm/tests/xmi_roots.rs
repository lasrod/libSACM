//! Tests for the interchange roots that `sacm::xmi::read` reads, and the ones
//! it refuses (SACM23-XMI-001; formal/23-05-08 §2.2, §2.4; XMI 2.5.1 §7.5.3;
//! ADR 0015, ADR 0016), and for the line counting of the SACM-XMI-001
//! refusals (LIB-SEC-002).
//!
//! Every input is an inline byte string, except the files under
//! `tests/data/sacm23/xmi/`. `read` is called with `ReadOptions::default()`.
//!
//! "The same document as the bare form" is compared on a projection of the
//! document (roots in order; every element's id, kind, parent, children and
//! name; the element count), because `Document` has no `PartialEq`. A read
//! reports no diagnostic, except SACM-XMI-010 for an `xsi:type` and
//! SACM-XMI-011 for an `xmi:version` (LIB-COMPAT-001), which the tests that
//! read them expect by code, severity and affected id.
//!
//! An unread attribute is expected to be refused at the first character of
//! its name, the place where it starts, as an element is refused at the `<` of
//! its start tag. Non-white-space text is expected to be refused at its first
//! character; the inputs put no white space before it, so the start of the
//! text and its first non-white-space character are the same place.
//!
//! Not checked here:
//! - Item 14's `#[non_exhaustive]` on `InvalidRoot` and `NoPackage`: a test
//!   cannot show that a struct expression or exhaustive pattern fails to
//!   compile. The patterns here use `..`, as a user must.
//! - Item 13's "the encoding refusals come first" is checked for each kind of
//!   refusal once; the LIB-API-009, LIB-SEC-001, LIB-SEC-002 and LIB-SEC-005
//!   tests check those refusals in full.

use std::error::Error;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::Duration;

use sacm::diagnostic::{Code, Severity, SourceLocation};
use sacm::model::{Document, ElementId, ElementKind, LangString, SacmElement};
use sacm::xmi::{self, ReadError, ReadOptions, ReadOutput};

/// The pinned SACM 2.3 namespace (D-NS-PIN).
const SACM_NS: &str = "http://www.omg.org/spec/SACM/20220301";
/// The XMI 2.5.1 namespace.
const XMI_NS: &str = "http://www.omg.org/spec/XMI/20131001";

/// The namespace declarations of a bare root: `sacm` and `xmi`.
const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
);

/// The start tag of an XMI wrapper that declares `xmi` and `sacm`.
const WRAPPER_OPEN: &str = concat!(
    "<xmi:XMI",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\">",
);
/// The end tag that closes [`WRAPPER_OPEN`].
const WRAPPER_CLOSE: &str = "</xmi:XMI>";

/// Item 1's input, exactly.
const ACP_ROOT: &str = concat!(
    "<sacm:AssuranceCasePackage",
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
    " xmi:id=\"acp_1\"/>",
);

/// Item 2's input, exactly.
const AP_ROOT: &str = concat!(
    "<sacm:ArgumentPackage",
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
    " xmi:id=\"arg_1\"/>",
);

/// The bare ArgumentPackage `a` that the wrapper of item 3 holds.
const BARE_A: &str = concat!(
    "<sacm:ArgumentPackage",
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
    " xmi:id=\"a\"/>",
);

/// Item 8's first input, exactly: the C++ fixture `invalid/root-invalid.sacm.xmi`.
const CLAIM_ROOT: &str = concat!(
    "<sacm:Claim",
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
    " xmi:id=\"c1\"/>",
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
    let end = input.len().min(160);
    let text = String::from_utf8_lossy(&input[..end]);
    if end < input.len() {
        format!("{text:?}... ({} bytes)", input.len())
    } else {
        format!("{text:?}")
    }
}

/// Reads `input`, which the test expects to be read.
fn read_ok(row: &str, input: &[u8]) -> ReadOutput {
    match read(row, input) {
        Ok(output) => output,
        Err(error) => panic!(
            "{row}: read refused {}: {error:?} ({error})",
            preview(input)
        ),
    }
}

/// Reads `input`, which the test expects to be refused, and returns why.
fn refusal(row: &str, input: &[u8]) -> ReadError {
    match read(row, input) {
        Ok(output) => panic!(
            "{row}: read returned a document for {}: {:?}",
            preview(input),
            shape(output.document())
        ),
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

/// The location of the first occurrence of `needle` in `text`, which must
/// end its lines with LF only: the line, and the column in characters.
fn position_of(row: &str, text: &str, needle: &str) -> SourceLocation {
    assert!(
        !text.contains('\r'),
        "{row}: position_of counts LF line ends only"
    );
    let Some(offset) = text.find(needle) else {
        panic!("{row}: the test input {text:?} does not contain {needle:?}");
    };
    let before = &text[..offset];
    let line = before.matches('\n').count() + 1;
    let column = before.rsplit('\n').next().unwrap_or("").chars().count() + 1;
    let (Ok(line), Ok(column)) = (u32::try_from(line), u32::try_from(column)) else {
        panic!("{row}: the test input is too large to locate");
    };
    loc(row, line, column)
}

/// One element of a [`Shape`].
#[derive(Debug, PartialEq, Eq)]
struct Entry {
    id: String,
    kind: ElementKind,
    parent: Option<String>,
    children: Vec<String>,
    name: Option<LangString>,
}

/// What two documents must share to be "the same document".
#[derive(Debug, PartialEq, Eq)]
struct Shape {
    /// The roots, in order: id and kind.
    roots: Vec<(String, ElementKind)>,
    /// Every element, pre-order.
    elements: Vec<Entry>,
    /// `Document::element_count`.
    count: usize,
}

fn shape(document: &Document) -> Shape {
    Shape {
        roots: document
            .roots()
            .map(|root| (root.id().as_str().to_owned(), root.kind()))
            .collect(),
        elements: document
            .elements()
            .map(|element| Entry {
                id: element.id().as_str().to_owned(),
                kind: element.kind(),
                parent: element.parent().map(|p| p.id().as_str().to_owned()),
                children: element
                    .children()
                    .map(|c| c.id().as_str().to_owned())
                    .collect(),
                name: element.name().cloned(),
            })
            .collect(),
        count: document.element_count(),
    }
}

/// The shape of a document whose only elements are the given parentless,
/// unnamed, childless roots, in order.
fn roots_shape(roots: &[(&str, ElementKind)]) -> Shape {
    Shape {
        roots: roots
            .iter()
            .map(|&(id, kind)| (id.to_owned(), kind))
            .collect(),
        elements: roots
            .iter()
            .map(|&(id, kind)| Entry {
                id: id.to_owned(),
                kind,
                parent: None,
                children: Vec::new(),
                name: None,
            })
            .collect(),
        count: roots.len(),
    }
}

/// Reads `input` and asserts that it gives a document of shape `expected`,
/// with no diagnostics.
fn assert_reads_as(row: &str, what: &str, input: &[u8], expected: &Shape) {
    assert_reads_reporting(row, what, input, expected, &[]);
}

/// A read diagnostic as these tests expect it: its code, severity and
/// affected ids.
type Reported = (Code, Severity, &'static [&'static str]);

/// SACM-XMI-011 for an `xmi:version` on the wrapper: Info, no affected id
/// (LIB-COMPAT-001).
const VERSION_ON_WRAPPER: Reported = (Code::XMI_VERSION_IGNORED, Severity::Info, &[]);
/// SACM-XMI-011 for an `xmi:version` on the root `a` (LIB-COMPAT-001).
const VERSION_ON_A: Reported = (Code::XMI_VERSION_IGNORED, Severity::Info, &["a"]);
/// SACM-XMI-010 for an `xsi:type` on the root `a`, a Warning in the default
/// tolerant mode (LIB-COMPAT-001).
const XSI_TYPE_ON_A: Reported = (Code::XMI_XSI_TYPE, Severity::Warning, &["a"]);

/// Reads `input` and asserts that it gives a document of shape `expected`,
/// with exactly the read diagnostics `reported`, in order.
fn assert_reads_reporting(
    row: &str,
    what: &str,
    input: &[u8],
    expected: &Shape,
    reported: &[Reported],
) {
    let output = read_ok(row, input);
    assert_eq!(
        &shape(output.document()),
        expected,
        "{row}: {what}: {} does not give the expected document",
        preview(input)
    );
    let found: Vec<(Code, Severity, Vec<&str>)> = output
        .diagnostics()
        .iter()
        .map(|found| {
            (
                found.code(),
                found.severity(),
                found.affected().iter().map(ElementId::as_str).collect(),
            )
        })
        .collect();
    let wanted: Vec<(Code, Severity, Vec<&str>)> = reported
        .iter()
        .map(|&(code, severity, affected)| (code, severity, affected.to_vec()))
        .collect();
    assert_eq!(
        found,
        wanted,
        "{row}: {what}: reading {} reported other diagnostics: {:?}",
        preview(input),
        output.diagnostics()
    );
}

/// The `InvalidRoot` `Display`, exactly.
fn invalid_root_display(name: &str, location: SourceLocation) -> String {
    format!("the root element {name:?} is not a SACM package at {location}")
}

/// Asserts that `input` is refused as `InvalidRoot` (SACM-XMI-001) at
/// `location`, naming `name` as written, with the exact `Display`.
fn assert_invalid_root_at(
    row: &str,
    input: &[u8],
    location: SourceLocation,
    name: &str,
) -> ReadError {
    let error = refusal(row, input);
    let ReadError::InvalidRoot {
        location: field,
        element_name,
        ..
    } = &error
    else {
        panic!(
            "{row}: expected InvalidRoot for {}, got {error:?}",
            preview(input)
        );
    };
    assert_eq!(
        element_name,
        name,
        "{row}: the InvalidRoot of {} does not name the element as written",
        preview(input)
    );
    assert_eq!(
        *field,
        Some(location),
        "{row}: the InvalidRoot of {} is not located at the `<` of {name:?}",
        preview(input)
    );
    assert_eq!(
        error.location(),
        Some(location),
        "{row}: location() of the InvalidRoot of {} disagrees with its field",
        preview(input)
    );
    assert_eq!(
        error.code(),
        Some(Code::XMI_INVALID_ROOT),
        "{row}: the InvalidRoot of {} does not carry SACM-XMI-001",
        preview(input)
    );
    assert_eq!(
        error.to_string(),
        invalid_root_display(name, location),
        "{row}: unexpected InvalidRoot Display for {}",
        preview(input)
    );
    error
}

/// Asserts that `input` is refused as `NoPackage` (SACM-XMI-001) at
/// `location`, with the exact `Display`.
fn assert_no_package_at(row: &str, input: &[u8], location: SourceLocation) -> ReadError {
    let error = refusal(row, input);
    let ReadError::NoPackage {
        location: field, ..
    } = &error
    else {
        panic!(
            "{row}: expected NoPackage for {}, got {error:?}",
            preview(input)
        );
    };
    assert_eq!(
        *field,
        Some(location),
        "{row}: the NoPackage of {} is not located at the wrapper",
        preview(input)
    );
    assert_eq!(
        error.location(),
        Some(location),
        "{row}: location() of the NoPackage of {} disagrees with its field",
        preview(input)
    );
    assert_eq!(
        error.code(),
        Some(Code::XMI_INVALID_ROOT),
        "{row}: the NoPackage of {} does not carry SACM-XMI-001",
        preview(input)
    );
    assert_eq!(
        error.to_string(),
        format!("the XMI element holds no SACM package at {location}"),
        "{row}: unexpected NoPackage Display for {}",
        preview(input)
    );
    error
}

/// Asserts that `input` is refused as `Unsupported`, with no code, at
/// `location`.
fn assert_unsupported_at(row: &str, input: &[u8], location: SourceLocation) -> ReadError {
    let error = refusal(row, input);
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
        Some(location),
        "{row}: the Unsupported refusal of {} is not at the expected place: {error}",
        preview(input)
    );
    error
}

/// A wrapper around `content`.
fn wrapped(content: &str) -> String {
    format!("{WRAPPER_OPEN}{content}{WRAPPER_CLOSE}")
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

/// Whether the diagnostics catalog has a table row for `code`.
fn catalog_has_row(row: &str, code: Code) -> bool {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/sacm/diagnostics.md");
    let catalog = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{row}: cannot read {}: {error}", path.display()));
    let cell = format!("| {} |", code.as_str());
    catalog
        .lines()
        .any(|line| line.trim_start().starts_with(&cell))
}

// --- SACM23-XMI-001 1-2. A bare package root ---------------------------------

/// Checks items 1 and 2 on one input: a single root of `kind` with id `id`.
fn assert_single_root(row: &str, input: &[u8], id: &str, kind: ElementKind) {
    let output = read_ok(row, input);
    let document = output.document();
    let roots: Vec<_> = document.roots().collect();
    assert_eq!(
        roots.len(),
        1,
        "{row}: {} does not give exactly one root: {:?}",
        preview(input),
        shape(document)
    );
    let root = roots[0];
    assert_eq!(root.id().as_str(), id, "{row}: the root's id is not {id:?}");
    assert_eq!(root.kind(), kind, "{row}: the root is not an {kind}");
    assert!(
        root.parent().is_none(),
        "{row}: the root {id:?} has a parent"
    );
    assert_eq!(
        root.children().count(),
        0,
        "{row}: the root {id:?} has children"
    );
    assert!(root.name().is_none(), "{row}: the root {id:?} has a name");
    assert_eq!(
        document.element_count(),
        1,
        "{row}: the document of {} does not have exactly 1 element",
        preview(input)
    );
    assert_eq!(
        document.get(id).map(SacmElement::kind),
        Some(kind),
        "{row}: Document::get({id:?}) does not find the root"
    );
    assert!(
        output.diagnostics().is_empty(),
        "{row}: reading {} reported diagnostics: {:?}",
        preview(input),
        output.diagnostics()
    );
}

#[test]
fn sacm23_xmi_001_assurance_case_package_root_reads_as_one_parentless_element() {
    let row = "SACM23-XMI-001";
    assert_single_root(
        row,
        ACP_ROOT.as_bytes(),
        "acp_1",
        ElementKind::AssuranceCasePackage,
    );
    assert_single_root(
        row,
        include_bytes!("data/sacm23/xmi/assurance-case-package-root-valid.sacm.xmi"),
        "acp_1",
        ElementKind::AssuranceCasePackage,
    );
}

#[test]
fn sacm23_xmi_001_argument_package_root_reads_as_one_parentless_element() {
    let row = "SACM23-XMI-001";
    assert_single_root(
        row,
        AP_ROOT.as_bytes(),
        "arg_1",
        ElementKind::ArgumentPackage,
    );
    assert_single_root(
        row,
        include_bytes!("data/sacm23/xmi/argument-package-root-valid.sacm.xmi"),
        "arg_1",
        ElementKind::ArgumentPackage,
    );
}

// --- SACM23-XMI-001 3. The wrapper ------------------------------------------

#[test]
fn sacm23_xmi_001_wrapper_gives_the_same_document_as_the_bare_form() {
    let row = "SACM23-XMI-001";
    let expected = roots_shape(&[("a", ElementKind::ArgumentPackage)]);
    // The bare form is the reference.
    assert_reads_as(row, "the bare form", BARE_A.as_bytes(), &expected);
    // The wrapper, exactly as the row states it: it is not an element.
    let input = concat!(
        "<xmi:XMI xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
        " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\">",
        "<sacm:ArgumentPackage xmi:id=\"a\"/></xmi:XMI>",
    );
    assert_reads_as(row, "the wrapped form", input.as_bytes(), &expected);
    let bare = read_ok(row, BARE_A.as_bytes());
    let wrapper = read_ok(row, input.as_bytes());
    assert_eq!(
        shape(wrapper.document()),
        shape(bare.document()),
        "{row}: the wrapped form is not the same document as the bare form"
    );
}

// --- SACM23-XMI-001 4. Several roots ----------------------------------------

#[test]
fn sacm23_xmi_001_several_wrapped_roots_keep_their_kinds_in_document_order() {
    let row = "SACM23-XMI-001";
    let expected = roots_shape(&[
        ("n1", ElementKind::AssuranceCasePackage),
        ("a1", ElementKind::ArgumentPackage),
        ("n2", ElementKind::AssuranceCasePackage),
    ]);
    let input = wrapped(concat!(
        "<sacm:AssuranceCasePackage xmi:id=\"n1\"/>",
        "<sacm:ArgumentPackage xmi:id=\"a1\"/>",
        "<sacm:AssuranceCasePackage xmi:id=\"n2\"/>",
    ));
    assert_reads_as(row, "three wrapped roots", input.as_bytes(), &expected);
    assert_reads_as(
        row,
        "the fixture",
        include_bytes!("data/sacm23/xmi/wrapped-roots-valid.sacm.xmi"),
        &expected,
    );
}

// --- SACM23-XMI-001 5. Ignorable content ------------------------------------

#[test]
fn sacm23_xmi_001_ignorable_content_around_a_bare_root_gives_the_same_document() {
    let row = "SACM23-XMI-001";
    let bare = shape(read_ok(row, BARE_A.as_bytes()).document());
    assert_eq!(
        bare,
        roots_shape(&[("a", ElementKind::ArgumentPackage)]),
        "{row}: the bare form is not the expected document"
    );
    let open = format!("<sacm:ArgumentPackage{NS} xmi:id=\"a\"");
    let close = "</sacm:ArgumentPackage>";
    let bare_forms = [
        (
            "an XML declaration",
            format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n{BARE_A}"),
        ),
        (
            "a comment and a PI before the root",
            format!("<!-- before -->\n<?tool before?>\n{BARE_A}"),
        ),
        (
            "a comment and a PI inside the root",
            format!("{open}><!-- inside --><?tool inside?></sacm:ArgumentPackage>"),
        ),
        (
            "a comment and a PI after the root",
            format!("{BARE_A}\n<!-- after -->\n<?tool after?>\n"),
        ),
        (
            "white space inside the root",
            format!("{open}>\n  \t\r\n  {close}"),
        ),
        (
            "white space around the root",
            format!("\n\n  {BARE_A}  \n\n"),
        ),
        (
            "xmi:version 2.0 on the root",
            format!("{open} xmi:version=\"2.0\"/>"),
        ),
        (
            "xmi:version 2.1 on the root",
            format!("{open} xmi:version=\"2.1\"/>"),
        ),
        (
            "an unused namespace declaration",
            format!("{open} xmlns:foo=\"urn:x\"/>"),
        ),
        (
            "an xsi namespace declaration",
            format!("{open} xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"/>"),
        ),
        (
            "all of them at once",
            format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!-- c -->\n<?tool a?>\n\
                 {open} xmi:version=\"2.0\" xmlns:foo=\"urn:x\" \
                 xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">\n  \
                 <!-- c --><?tool b?>\n{close}\n<!-- c -->\n"
            ),
        ),
    ];
    for (what, input) in &bare_forms {
        // The document is the same; an `xmi:version` on the root is reported
        // as SACM-XMI-011 (LIB-COMPAT-001).
        let reported: &[Reported] = if input.contains("xmi:version") {
            &[VERSION_ON_A]
        } else {
            &[]
        };
        assert_reads_reporting(row, what, input.as_bytes(), &bare, reported);
    }
}

#[test]
fn sacm23_xmi_001_ignorable_content_in_and_around_a_wrapper_gives_the_same_document() {
    let row = "SACM23-XMI-001";
    // A wrapper holding two roots, with nothing ignorable, is the reference.
    let two = roots_shape(&[
        ("a", ElementKind::ArgumentPackage),
        ("n", ElementKind::AssuranceCasePackage),
    ]);
    let plain =
        wrapped("<sacm:ArgumentPackage xmi:id=\"a\"/><sacm:AssuranceCasePackage xmi:id=\"n\"/>");
    assert_reads_as(row, "the plain wrapper", plain.as_bytes(), &two);
    let wrapper_open = WRAPPER_OPEN.trim_end_matches('>');
    let wrapped_forms = [
        (
            "comments, PIs and white space between wrapper children",
            wrapped(
                "\n  <!-- c -->\n  <sacm:ArgumentPackage xmi:id=\"a\"/>\n  <?tool x?>\n  \
                 <!-- c -->\n  <sacm:AssuranceCasePackage xmi:id=\"n\"/>\n  <?tool y?>\n",
            ),
        ),
        (
            "xmi:version 2.0 on the wrapper",
            format!(
                "{wrapper_open} xmi:version=\"2.0\"><sacm:ArgumentPackage xmi:id=\"a\"/>\
                 <sacm:AssuranceCasePackage xmi:id=\"n\"/>{WRAPPER_CLOSE}"
            ),
        ),
        (
            "xmi:version 2.1 on the wrapper",
            format!(
                "{wrapper_open} xmi:version=\"2.1\"><sacm:ArgumentPackage xmi:id=\"a\"/>\
                 <sacm:AssuranceCasePackage xmi:id=\"n\"/>{WRAPPER_CLOSE}"
            ),
        ),
        (
            "unused and xsi namespace declarations on the wrapper",
            format!(
                "{wrapper_open} xmlns:foo=\"urn:x\" \
                 xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">\
                 <sacm:ArgumentPackage xmi:id=\"a\"/>\
                 <sacm:AssuranceCasePackage xmi:id=\"n\"/>{WRAPPER_CLOSE}"
            ),
        ),
        (
            "a declaration, comments and PIs around the wrapper",
            format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!-- c -->\n<?tool a?>\n{}\n<!-- c -->",
                wrapped(
                    "<sacm:ArgumentPackage xmi:id=\"a\" xmi:version=\"2.0\"><!-- c --></sacm:ArgumentPackage>\
                     <sacm:AssuranceCasePackage xmi:id=\"n\"/>"
                )
            ),
        ),
    ];
    for (what, input) in &wrapped_forms {
        // The document is the same; an `xmi:version` is reported as
        // SACM-XMI-011, with no affected id on the wrapper (LIB-COMPAT-001).
        // The selector reads the input, not the case's name.
        let reported: &[Reported] = if input.contains(&format!("{wrapper_open} xmi:version")) {
            &[VERSION_ON_WRAPPER]
        } else if input.contains("xmi:version") {
            &[VERSION_ON_A]
        } else {
            &[]
        };
        assert_reads_reporting(row, what, input.as_bytes(), &two, reported);
    }
}

// --- SACM23-XMI-001 6. A type attribute ------------------------------------

#[test]
fn sacm23_xmi_001_a_type_naming_the_root_class_gives_the_same_document() {
    let row = "SACM23-XMI-001";
    let expected = roots_shape(&[("a", ElementKind::ArgumentPackage)]);
    let xsi = " xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"";
    // The document is the same; an `xsi:type` is reported as SACM-XMI-010
    // (LIB-COMPAT-001).
    let inputs: [(String, &[Reported]); 2] = [
        (
            format!("<sacm:ArgumentPackage{NS} xmi:id=\"a\" xmi:type=\"sacm:ArgumentPackage\"/>"),
            &[],
        ),
        (
            format!(
                "<sacm:ArgumentPackage{NS}{xsi} xmi:id=\"a\" xsi:type=\"sacm:ArgumentPackage\"/>"
            ),
            &[XSI_TYPE_ON_A],
        ),
    ];
    for (input, reported) in &inputs {
        assert_reads_reporting(
            row,
            "a redundant type",
            input.as_bytes(),
            &expected,
            reported,
        );
    }
}

#[test]
fn sacm23_xmi_001_a_type_naming_another_class_is_unsupported_at_the_root() {
    let row = "SACM23-XMI-001";
    let xsi = " xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"";
    let inputs = [
        // A subtype (ADR 0016 D2 and D5).
        format!(
            "<sacm:ArgumentPackage{NS}{xsi} xmi:id=\"a\" xsi:type=\"sacm:ArgumentPackageInterface\"/>"
        ),
        // A class that is not a package.
        format!("<sacm:ArgumentPackage{NS}{xsi} xmi:id=\"a\" xsi:type=\"sacm:Claim\"/>"),
        format!("<sacm:ArgumentPackage{NS} xmi:id=\"a\" xmi:type=\"sacm:Claim\"/>"),
        // Another package class.
        format!(
            "<sacm:AssuranceCasePackage{NS}{xsi} xmi:id=\"a\" xsi:type=\"sacm:ArgumentPackage\"/>"
        ),
        // A prefix bound to another namespace.
        format!(
            "<sacm:ArgumentPackage{NS}{xsi} xmlns:o=\"urn:other\" xmi:id=\"a\" xsi:type=\"o:ArgumentPackage\"/>"
        ),
        // An unprefixed type, with no default namespace.
        format!("<sacm:ArgumentPackage{NS}{xsi} xmi:id=\"a\" xsi:type=\"ArgumentPackage\"/>"),
    ];
    for input in &inputs {
        assert_unsupported_at(row, input.as_bytes(), loc(row, 1, 1));
    }
}

// --- SACM23-XMI-001 7. Ids --------------------------------------------------

#[test]
fn sacm23_xmi_001_ids_are_kept_exactly() {
    let row = "SACM23-XMI-001";
    for (written, id) in [("Ünï.c-1", "Ünï.c-1"), ("a&#x42;", "aB")] {
        let input = format!("<sacm:ArgumentPackage{NS} xmi:id=\"{written}\"/>");
        assert_reads_as(
            row,
            &format!("the id {written:?}"),
            input.as_bytes(),
            &roots_shape(&[(id, ElementKind::ArgumentPackage)]),
        );
    }
}

// --- SACM23-XMI-001 8. SACM-XMI-001 -----------------------------------------

#[test]
fn sacm23_xmi_001_claim_root_is_invalid_root_with_the_exact_display() {
    let row = "SACM23-XMI-001";
    let error = assert_invalid_root_at(row, CLAIM_ROOT.as_bytes(), loc(row, 1, 1), "sacm:Claim");
    assert_eq!(
        error.to_string(),
        "the root element \"sacm:Claim\" is not a SACM package at line 1, column 1",
        "{row}: unexpected Display for a Claim root"
    );
    assert_invalid_root_at(
        row,
        include_bytes!("data/sacm23/xmi/invalid/root-claim-invalid-root.sacm.xmi"),
        loc(row, 2, 1),
        "sacm:Claim",
    );
}

#[test]
fn sacm23_xmi_001_a_root_that_is_not_a_package_kind_is_invalid_root_at_its_start_tag() {
    let row = "SACM23-XMI-001";
    let cases: [(String, &str); 7] = [
        (CLAIM_ROOT.to_owned(), "sacm:Claim"),
        (
            format!("<sacm:ArgumentGroup{NS} xmi:id=\"g\"/>"),
            "sacm:ArgumentGroup",
        ),
        // A package-kind name in the wrong letter case is not a package kind.
        (
            format!("<sacm:assuranceCasePackage{NS} xmi:id=\"p\"/>"),
            "sacm:assuranceCasePackage",
        ),
        (format!("<sacm:Foo xmlns:sacm=\"{SACM_NS}\"/>"), "sacm:Foo"),
        ("<r/>".to_owned(), "r"),
        (
            "<html xmlns=\"http://www.w3.org/1999/xhtml\"><body/></html>".to_owned(),
            "html",
        ),
        (
            "<uml:Model xmlns:uml=\"http://www.omg.org/spec/UML/20131001\"/>".to_owned(),
            "uml:Model",
        ),
    ];
    for (root, name) in &cases {
        // At line 1, column 1, and after a prolog at line 3, column 3.
        assert_invalid_root_at(row, root.as_bytes(), loc(row, 1, 1), name);
        let later = format!("<?xml version=\"1.0\"?>\n\n  {root}");
        assert_invalid_root_at(row, later.as_bytes(), loc(row, 3, 3), name);
    }
}

#[test]
fn sacm23_xmi_001_a_wrapped_claim_is_invalid_root_and_no_document_is_returned() {
    let row = "SACM23-XMI-001";
    let input = wrapped("\n  <sacm:ArgumentPackage xmi:id=\"a\"/>\n  <sacm:Claim xmi:id=\"c\"/>\n");
    // `refusal` fails the test if a document comes back.
    assert_invalid_root_at(row, input.as_bytes(), loc(row, 3, 3), "sacm:Claim");
    // The Claim first, too.
    let first = wrapped("<sacm:Claim xmi:id=\"c\"/>\n<sacm:ArgumentPackage xmi:id=\"a\"/>");
    let column = u32::try_from(WRAPPER_OPEN.chars().count() + 1)
        .unwrap_or_else(|_| panic!("{row}: the wrapper start tag is too long"));
    assert_invalid_root_at(row, first.as_bytes(), loc(row, 1, column), "sacm:Claim");
}

#[test]
fn sacm23_xmi_001_a_wrapper_with_no_package_is_no_package_at_the_wrapper() {
    let row = "SACM23-XMI-001";
    let empty = format!("<xmi:XMI xmlns:xmi=\"{XMI_NS}\"/>");
    let error = assert_no_package_at(row, empty.as_bytes(), loc(row, 1, 1));
    assert_eq!(
        error.to_string(),
        "the XMI element holds no SACM package at line 1, column 1",
        "{row}: unexpected NoPackage Display"
    );
    let after_declaration = format!("<?xml version=\"1.0\"?>\n  {empty}");
    assert_no_package_at(row, after_declaration.as_bytes(), loc(row, 2, 3));
    let documentation_only =
        format!("<xmi:XMI xmlns:xmi=\"{XMI_NS}\">\n  <xmi:documentation/>\n</xmi:XMI>");
    assert_no_package_at(row, documentation_only.as_bytes(), loc(row, 1, 1));
    let comment_only = format!("<xmi:XMI xmlns:xmi=\"{XMI_NS}\">\n  <!-- c -->\n</xmi:XMI>");
    assert_no_package_at(row, comment_only.as_bytes(), loc(row, 1, 1));
    assert_no_package_at(
        row,
        include_bytes!("data/sacm23/xmi/invalid/wrapper-empty-no-package.sacm.xmi"),
        loc(row, 2, 1),
    );
}

// --- SACM23-XMI-001 9. SACM-XMI-001 takes precedence -------------------------

#[test]
fn sacm23_xmi_001_invalid_root_takes_precedence_over_unsupported() {
    let row = "SACM23-XMI-001";
    // Each wrapper holds something `Unsupported` on line 2, then a Claim on
    // line 3.
    let before_claim = [
        // The row's case: a package kind not read yet.
        "<sacm:ArtifactPackage xmi:id=\"f\"/>",
        // An XMI-namespace child.
        "<xmi:documentation/>",
        // A readable root with content it cannot read.
        "<sacm:ArgumentPackage xmi:id=\"a\"><note/></sacm:ArgumentPackage>",
        // A readable root with an attribute it cannot read.
        "<sacm:ArgumentPackage xmi:id=\"a\" gid=\"g\"/>",
        // A root without an id.
        "<sacm:ArgumentPackage/>",
    ];
    for first in before_claim {
        let input = wrapped(&format!("\n  {first}\n  <sacm:Claim xmi:id=\"c\"/>\n"));
        assert_invalid_root_at(row, input.as_bytes(), loc(row, 3, 3), "sacm:Claim");
    }
    // A repeated id before the Claim.
    let repeated = wrapped(
        "\n  <sacm:ArgumentPackage xmi:id=\"a\"/>\n  <sacm:ArgumentPackage xmi:id=\"a\"/>\n  \
         <sacm:Claim xmi:id=\"c\"/>\n",
    );
    assert_invalid_root_at(row, repeated.as_bytes(), loc(row, 4, 3), "sacm:Claim");
    // A bare Claim root with content and attributes that would be Unsupported.
    let claim = format!("<sacm:Claim{NS} gid=\"g\"><name/>text</sacm:Claim>");
    assert_invalid_root_at(row, claim.as_bytes(), loc(row, 1, 1), "sacm:Claim");
}

// --- SACM23-XMI-001 10. Roots not readable yet -------------------------------

#[test]
fn sacm23_xmi_001_the_other_ten_package_kinds_are_unsupported_at_the_root() {
    let row = "SACM23-XMI-001";
    let others: Vec<ElementKind> = ElementKind::ALL
        .iter()
        .copied()
        .filter(|kind| kind.is_package())
        .filter(|kind| {
            !matches!(
                kind,
                ElementKind::AssuranceCasePackage | ElementKind::ArgumentPackage
            )
        })
        .collect();
    assert_eq!(
        others.len(),
        10,
        "{row}: expected 10 other package kinds, found {others:?}"
    );
    for kind in others {
        let input = format!("<sacm:{}{NS} xmi:id=\"p\"/>", kind.name());
        let error = assert_unsupported_at(row, input.as_bytes(), loc(row, 1, 1));
        assert!(
            !matches!(error, ReadError::InvalidRoot { .. }),
            "{row}: the {kind} root is SACM-XMI-001"
        );
        // Inside a wrapper, at the child.
        let wrapped_input = wrapped(&format!("\n<sacm:{} xmi:id=\"p\"/>\n", kind.name()));
        assert_unsupported_at(row, wrapped_input.as_bytes(), loc(row, 2, 1));
    }
}

#[test]
fn sacm23_xmi_001_a_package_name_outside_the_pinned_namespace_is_unsupported_at_the_root() {
    let row = "SACM23-XMI-001";
    let inputs = [
        format!("<foo:ArgumentPackage xmlns:foo=\"urn:x\" xmlns:xmi=\"{XMI_NS}\" xmi:id=\"p\"/>"),
        format!("<ArgumentPackage xmlns:xmi=\"{XMI_NS}\" xmi:id=\"p\"/>"),
        format!(
            "<sacm:ArgumentPackage xmlns:sacm=\"http://www.omg.org/spec/SACM/2.2/argumentation\" \
             xmlns:xmi=\"{XMI_NS}\" xmi:id=\"p\"/>"
        ),
        format!("<AssuranceCasePackage xmlns=\"urn:x\" xmlns:xmi=\"{XMI_NS}\" xmi:id=\"p\"/>"),
    ];
    for input in &inputs {
        assert_unsupported_at(row, input.as_bytes(), loc(row, 1, 1));
    }
    // Wrapped, at the child.
    let wrapped_input = wrapped("\n  <foo:ArgumentPackage xmlns:foo=\"urn:x\" xmi:id=\"p\"/>\n");
    assert_unsupported_at(row, wrapped_input.as_bytes(), loc(row, 2, 3));
}

#[test]
fn sacm23_xmi_001_an_xmi_element_outside_the_xmi_namespace_is_unsupported_at_the_wrapper() {
    let row = "SACM23-XMI-001";
    let inputs = [
        format!(
            "<xmi:XMI xmlns:xmi=\"http://www.omg.org/XMI\" xmlns:sacm=\"{SACM_NS}\">\
             <sacm:ArgumentPackage xmlns:xmi=\"{XMI_NS}\" xmi:id=\"a\"/></xmi:XMI>"
        ),
        format!(
            "<XMI xmlns:sacm=\"{SACM_NS}\" xmlns:xmi=\"{XMI_NS}\">\
             <sacm:ArgumentPackage xmi:id=\"a\"/></XMI>"
        ),
    ];
    for input in &inputs {
        let error = assert_unsupported_at(row, input.as_bytes(), loc(row, 1, 1));
        assert!(
            !matches!(error, ReadError::InvalidRoot { .. }),
            "{row}: a foreign XMI wrapper is SACM-XMI-001"
        );
    }
}

#[test]
fn sacm23_xmi_001_an_xmi_namespace_element_as_root_or_wrapper_child_is_unsupported_there() {
    let row = "SACM23-XMI-001";
    let extension_root = format!("<xmi:Extension xmlns:xmi=\"{XMI_NS}\"/>");
    assert_unsupported_at(row, extension_root.as_bytes(), loc(row, 1, 1));
    for child in ["<xmi:documentation/>", "<xmi:extension/>"] {
        let before = wrapped(&format!(
            "\n  {child}\n  <sacm:ArgumentPackage xmi:id=\"a\"/>\n"
        ));
        assert_unsupported_at(row, before.as_bytes(), loc(row, 2, 3));
        let after = wrapped(&format!(
            "\n  <sacm:ArgumentPackage xmi:id=\"a\"/>\n  {child}\n"
        ));
        assert_unsupported_at(row, after.as_bytes(), loc(row, 3, 3));
    }
}

// --- SACM23-XMI-001 11. Unread content ---------------------------------------

/// The start of a readable root that declares `xsi` and `ext` too, without
/// its closing `>` or `/>`, all on line 1.
fn root_start(kind: &str) -> String {
    format!(
        "<sacm:{kind}{NS} xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" \
         xmlns:ext=\"urn:ext\" xmi:id=\"p\""
    )
}

#[test]
fn sacm23_xmi_001_an_unread_attribute_of_a_root_is_unsupported_at_the_attribute() {
    let row = "SACM23-XMI-001";
    let attributes = [
        "gid=\"g\"",
        "gid=\"\"",
        "isCitation=\"false\"",
        "isAbstract=\"true\"",
        "citedElement=\"x\"",
        "abstractForm=\"x\"",
        "interface=\"x\"",
        "xmi:uuid=\"u\"",
        "xmi:label=\"l\"",
        "href=\"x\"",
        "xmi:idref=\"x\"",
        "xsi:schemaLocation=\"urn:x x.xsd\"",
        "xml:lang=\"en\"",
        "foo=\"1\"",
        "ext:a=\"1\"",
    ];
    for kind in ["ArgumentPackage", "AssuranceCasePackage"] {
        for attribute in attributes {
            // The attribute starts line 2, after two spaces.
            let input = format!("{}\n  {attribute}/>", root_start(kind));
            assert_unsupported_at(row, input.as_bytes(), loc(row, 2, 3));
        }
    }
}

#[test]
fn sacm23_xmi_001_an_unread_attribute_of_the_wrapper_is_unsupported_at_the_attribute() {
    let row = "SACM23-XMI-001";
    for attribute in ["foo=\"1\"", "xmi:id=\"w\"", "xmi:uuid=\"u\""] {
        let input = format!(
            "<xmi:XMI xmlns:xmi=\"{XMI_NS}\" xmlns:sacm=\"{SACM_NS}\"\n  {attribute}>\
             <sacm:ArgumentPackage xmi:id=\"a\"/></xmi:XMI>"
        );
        assert_unsupported_at(row, input.as_bytes(), loc(row, 2, 3));
    }
}

#[test]
fn sacm23_xmi_001_unread_content_of_a_root_is_unsupported_at_that_content() {
    let row = "SACM23-XMI-001";
    let children = [
        "<note/>",
        "<argumentPackage/>",
        "<argumentElement xsi:type=\"sacm:Claim\"/>",
        "<xmi:Extension/>",
        "<gid>g</gid>",
    ];
    for kind in ["ArgumentPackage", "AssuranceCasePackage"] {
        for child in children {
            // The child starts line 2, after two spaces.
            let input = format!("{}>\n  {child}\n</sacm:{kind}>", root_start(kind));
            assert_unsupported_at(row, input.as_bytes(), loc(row, 2, 3));
        }
        // Non-white-space text, at its first character: line 2, column 2.
        let text = format!("{}\n>x</sacm:{kind}>", root_start(kind));
        assert_unsupported_at(row, text.as_bytes(), loc(row, 2, 2));
        // Text after a comment, too: line 2, column 9.
        let after_comment = format!("{}>\n<!--c-->x</sacm:{kind}>", root_start(kind));
        assert_unsupported_at(row, after_comment.as_bytes(), loc(row, 2, 9));
    }
    // Non-white-space text in the wrapper, at its first character.
    let wrapper_text = wrapped("x\n<sacm:ArgumentPackage xmi:id=\"a\"/>");
    let at = position_of(row, &wrapper_text, "x\n<sacm:");
    assert_unsupported_at(row, wrapper_text.as_bytes(), at);
}

// --- SACM23-XMI-001 12. Ids -------------------------------------------------

/// Asserts that `input` is refused as `DuplicateId` (SACM-ID-001) for `id`,
/// naming exactly `locations`, with `location()` the second of them
/// (ADR 0015 amendment 2).
fn assert_duplicate_id_at(row: &str, input: &[u8], id: &str, locations: &[SourceLocation]) {
    let error = refusal(row, input);
    let ReadError::DuplicateId {
        id: found,
        locations: found_locations,
        ..
    } = &error
    else {
        panic!(
            "{row}: expected DuplicateId for {}, got {error:?}",
            preview(input)
        );
    };
    assert_eq!(
        found.as_str(),
        id,
        "{row}: the DuplicateId of {} names another id",
        preview(input)
    );
    assert_eq!(
        found_locations.as_slice(),
        locations,
        "{row}: the DuplicateId of {} does not name every element carrying {id:?}",
        preview(input)
    );
    assert_eq!(
        error.location(),
        locations.get(1).copied(),
        "{row}: the DuplicateId of {} is not located at the first repeat",
        preview(input)
    );
    assert_eq!(
        error.code(),
        Some(Code::ID_DUPLICATE),
        "{row}: the DuplicateId of {} does not carry SACM-ID-001",
        preview(input)
    );
}

/// Asserts that `input` is refused as `InvalidId` (SACM-ID-002) for the
/// empty id at `location` (ADR 0015 amendment 2).
fn assert_empty_id_at(row: &str, input: &[u8], location: SourceLocation) {
    let error = refusal(row, input);
    assert!(
        matches!(&error, ReadError::InvalidId { id, .. } if id.is_empty()),
        "{row}: expected InvalidId for {}, got {error:?}",
        preview(input)
    );
    assert_eq!(
        (error.code(), error.location()),
        (Some(Code::ID_INVALID), Some(location)),
        "{row}: the empty id of {} is not SACM-ID-002 at {location}: {error}",
        preview(input)
    );
}

#[test]
fn sacm23_xmi_001_an_empty_root_id_is_invalid_id_at_the_root() {
    let row = "SACM23-XMI-001";
    for kind in ["ArgumentPackage", "AssuranceCasePackage"] {
        let bare = format!("<sacm:{kind}{NS} xmi:id=\"\"/>");
        assert_empty_id_at(row, bare.as_bytes(), loc(row, 1, 1));
    }
    let in_wrapper = wrapped("\n  <sacm:ArgumentPackage xmi:id=\"\"/>\n");
    assert_empty_id_at(row, in_wrapper.as_bytes(), loc(row, 2, 3));
}

#[test]
fn sacm23_xmi_001_a_missing_or_foreign_root_id_is_unsupported_at_the_root() {
    let row = "SACM23-XMI-001";
    let inputs = [
        format!("<sacm:ArgumentPackage{NS}/>"),
        format!("<sacm:ArgumentPackage{NS} id=\"p\"/>"),
        format!("<sacm:ArgumentPackage{NS} xmlns:o=\"urn:o\" o:id=\"p\"/>"),
        format!("<sacm:AssuranceCasePackage{NS}/>"),
    ];
    for input in &inputs {
        assert_unsupported_at(row, input.as_bytes(), loc(row, 1, 1));
    }
    // Wrapped, at the root without an id.
    let wrapped_input = wrapped("\n  <sacm:ArgumentPackage/>\n");
    assert_unsupported_at(row, wrapped_input.as_bytes(), loc(row, 2, 3));
}

#[test]
fn sacm23_xmi_001_a_repeated_root_id_is_duplicate_id_at_the_second_root() {
    let row = "SACM23-XMI-001";
    let same_kind = wrapped(
        "\n  <sacm:ArgumentPackage xmi:id=\"p\"/>\n  <sacm:ArgumentPackage xmi:id=\"p\"/>\n",
    );
    assert_duplicate_id_at(
        row,
        same_kind.as_bytes(),
        "p",
        &[loc(row, 2, 3), loc(row, 3, 3)],
    );
    let other_kind = wrapped(
        "\n  <sacm:AssuranceCasePackage xmi:id=\"p\"/>\n  <sacm:ArgumentPackage xmi:id=\"q\"/>\n  \
         <sacm:ArgumentPackage xmi:id=\"p\"/>\n",
    );
    assert_duplicate_id_at(
        row,
        other_kind.as_bytes(),
        "p",
        &[loc(row, 2, 3), loc(row, 4, 3)],
    );
}

#[test]
fn sacm23_xmi_001_a_repeated_root_id_is_refused_at_the_root_before_its_attributes_and_content() {
    let row = "SACM23-XMI-001";
    // The second root also has an unread attribute, or unread content,
    // either of which would be refused first if the id were checked later.
    let inputs = [
        "\n  <sacm:ArgumentPackage xmi:id=\"a\"/>\n  <sacm:ArgumentPackage xmi:id=\"a\" gid=\"g\"/>\n",
        "\n  <sacm:ArgumentPackage xmi:id=\"a\"/>\n  <sacm:ArgumentPackage gid=\"g\" xmi:id=\"a\"/>\n",
        "\n  <sacm:ArgumentPackage xmi:id=\"a\"/>\n  <sacm:ArgumentPackage xmi:id=\"a\"><foo/></sacm:ArgumentPackage>\n",
        "\n  <sacm:ArgumentPackage xmi:id=\"a\"/>\n  <sacm:AssuranceCasePackage xmi:id=\"a\">\
         <argumentPackage xmi:id=\"x\" gid=\"g\"/></sacm:AssuranceCasePackage>\n",
    ];
    for content in inputs {
        assert_duplicate_id_at(
            row,
            wrapped(content).as_bytes(),
            "a",
            &[loc(row, 2, 3), loc(row, 3, 3)],
        );
    }
}

#[test]
fn sacm23_xmi_001_a_root_that_repeats_an_earlier_roots_child_id_is_refused_at_the_root() {
    let row = "SACM23-XMI-001";
    let first = "\n  <sacm:ArgumentPackage xmi:id=\"a\">\
                 <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c\"/></sacm:ArgumentPackage>\n";
    let seconds = [
        "  <sacm:ArgumentPackage xmi:id=\"c\"/>\n",
        "  <sacm:ArgumentPackage xmi:id=\"c\"><foo/></sacm:ArgumentPackage>\n",
        "  <sacm:AssuranceCasePackage xmi:id=\"c\" gid=\"g\"/>\n",
    ];
    for second in seconds {
        let input = wrapped(&format!("{first}{second}"));
        let child = position_of(row, &input, "<argumentElement");
        assert_duplicate_id_at(row, input.as_bytes(), "c", &[child, loc(row, 3, 3)]);
    }
    // A grandchild's id, at depth 3.
    let deep = wrapped(
        "\n  <sacm:AssuranceCasePackage xmi:id=\"a\"><argumentPackage xmi:id=\"b\">\
         <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c\"/></argumentPackage></sacm:AssuranceCasePackage>\n  \
         <sacm:ArgumentPackage xmi:id=\"c\"><foo/></sacm:ArgumentPackage>\n",
    );
    let grandchild = position_of(row, &deep, "<argumentElement");
    assert_duplicate_id_at(row, deep.as_bytes(), "c", &[grandchild, loc(row, 3, 3)]);
}

// --- SACM23-XMI-001 13. Order and determinism --------------------------------

/// `text` encoded as UTF-16 LE without a byte order mark, which stays
/// refused when UTF-16 with a mark is read (SACM23-XMI-018).
fn utf16le_without_bom(text: &str) -> Vec<u8> {
    let mut bytes = Vec::new();
    for unit in text.encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    bytes
}

#[test]
fn sacm23_xmi_001_input_checks_come_before_root_classification() {
    let row = "SACM23-XMI-001";
    let claim = |attributes: &str| format!("<sacm:Claim{NS} xmi:id=\"c\"{attributes}/>");
    // Encoding refusals (LIB-API-009): Unsupported, not SACM-XMI-001.
    let utf16 = utf16le_without_bom(&format!(
        "<?xml version=\"1.0\" encoding=\"UTF-16LE\"?>{}",
        claim("")
    ));
    assert_unsupported_at(row, &utf16, loc(row, 1, 1));
    let latin1 = format!(
        "<?xml version=\"1.0\" encoding=\"ISO-8859-1\"?>{}",
        claim("")
    );
    assert_unsupported_at(row, latin1.as_bytes(), loc(row, 1, 31));
    let empty_wrapper_latin1 =
        format!("<?xml version=\"1.0\" encoding=\"ISO-8859-1\"?><xmi:XMI xmlns:xmi=\"{XMI_NS}\"/>");
    assert_unsupported_at(row, empty_wrapper_latin1.as_bytes(), loc(row, 1, 31));

    // DOCTYPE (LIB-SEC-001), then Malformed (LIB-SEC-002, LIB-SEC-005).
    let doctype = format!("<!DOCTYPE sacm:Claim>\n{}", claim(""));
    let doctype_wrapper = format!("<!DOCTYPE x>\n<xmi:XMI xmlns:xmi=\"{XMI_NS}\"/>");
    for input in [doctype, doctype_wrapper] {
        let error = refusal(row, input.as_bytes());
        assert!(
            matches!(error, ReadError::Doctype { .. }),
            "{row}: {} is not refused for its DOCTYPE first: {error:?}",
            preview(input.as_bytes())
        );
    }
    let mut invalid_utf8 = claim("").into_bytes();
    invalid_utf8.splice(1..1, [0xFF]);
    let malformed: Vec<(&str, Vec<u8>)> = vec![
        ("invalid UTF-8", invalid_utf8),
        (
            "non-ASCII under an ASCII declaration",
            format!(
                "<?xml version=\"1.0\" encoding=\"US-ASCII\"?>\n<sacm:Claim{NS} xmi:id=\"cé\"/>"
            )
            .into_bytes(),
        ),
        (
            "a mismatched end tag",
            format!("<sacm:Claim{NS} xmi:id=\"c\"></sacm:Clam>").into_bytes(),
        ),
        (
            "an undeclared prefix",
            b"<sacm:Claim xmi:id=\"c\"/>".to_vec(),
        ),
        (
            "a surrogate reference",
            format!("<sacm:Claim{NS} xmi:id=\"&#xD800;\"/>").into_bytes(),
        ),
        (
            "a bad XML declaration",
            format!("<?xml version=\"2.0\"?>{}", claim("")).into_bytes(),
        ),
        ("prefix undeclaring", claim(" xmlns:p=\"\"").into_bytes()),
        (
            "an `xml` PI target",
            format!("<?XML x?>{}", claim("")).into_bytes(),
        ),
        (
            "a surrogate reference in an empty wrapper",
            format!("<xmi:XMI xmlns:xmi=\"{XMI_NS}\" foo=\"&#xD800;\"/>").into_bytes(),
        ),
    ];
    for (what, input) in &malformed {
        let error = refusal(row, input);
        assert!(
            matches!(error, ReadError::Malformed { .. }),
            "{row}: {what}: {} is not refused as Malformed before its root is classified: {error:?}",
            preview(input)
        );
        assert_eq!(
            error.code(),
            Some(Code::XML_MALFORMED),
            "{row}: {what}: the refusal does not carry SACM-XML-001"
        );
    }
}

#[test]
fn sacm23_xmi_001_the_same_input_always_gives_the_same_result() {
    let row = "SACM23-XMI-001";
    let inputs = [
        ACP_ROOT.to_owned(),
        AP_ROOT.to_owned(),
        wrapped("<sacm:AssuranceCasePackage xmi:id=\"n1\"/><sacm:ArgumentPackage xmi:id=\"a1\"/>"),
        CLAIM_ROOT.to_owned(),
        format!("<xmi:XMI xmlns:xmi=\"{XMI_NS}\"/>"),
        format!("<sacm:ArtifactPackage{NS} xmi:id=\"f\"/>"),
        wrapped("<sacm:ArtifactPackage xmi:id=\"f\"/><sacm:Claim xmi:id=\"c\"/>"),
    ];
    for input in &inputs {
        let first = read(row, input.as_bytes());
        for _ in 0..3 {
            let again = read(row, input.as_bytes());
            match (&first, &again) {
                (Ok(a), Ok(b)) => assert_eq!(
                    shape(a.document()),
                    shape(b.document()),
                    "{row}: reading {} twice gave different documents",
                    preview(input.as_bytes())
                ),
                (Err(a), Err(b)) => assert_eq!(
                    a,
                    b,
                    "{row}: reading {} twice gave different refusals",
                    preview(input.as_bytes())
                ),
                _ => panic!(
                    "{row}: reading {} twice gave {first:?} and {again:?}",
                    preview(input.as_bytes())
                ),
            }
        }
    }
}

// --- SACM23-XMI-001 14. ReadOutput and the SACM-XMI-001 refusals -------------

fn assert_send_sync_static<T: Send + Sync + 'static>() {}

#[test]
fn sacm23_xmi_001_read_output_gives_its_document_and_diagnostics() {
    let row = "SACM23-XMI-001";
    assert_send_sync_static::<ReadOutput>();
    let output = read_ok(row, ACP_ROOT.as_bytes());
    let expected = roots_shape(&[("acp_1", ElementKind::AssuranceCasePackage)]);
    assert_eq!(
        shape(output.document()),
        expected,
        "{row}: document() is not the document read"
    );
    let diagnostics: &[sacm::diagnostic::Diagnostic] = output.diagnostics();
    assert!(
        diagnostics.is_empty(),
        "{row}: diagnostics() is not empty for an input with neither xsi:type nor xmi:version: {diagnostics:?}"
    );
    let copy = output.clone();
    let document: Document = output.into_document();
    assert_eq!(
        shape(&document),
        expected,
        "{row}: into_document() is not the document read"
    );
    assert_eq!(
        shape(copy.document()),
        expected,
        "{row}: a clone of the ReadOutput holds another document"
    );
}

#[test]
fn sacm23_xmi_001_refusals_are_one_located_line_quoting_names_without_source() {
    let row = "SACM23-XMI-001";
    let refusals = [
        (
            "<?xml version=\"1.0\"?>\n<zqx:Root xmlns:zqx=\"urn:zqx\"/>".to_owned(),
            Some("zqx:Root"),
        ),
        (CLAIM_ROOT.to_owned(), Some("sacm:Claim")),
        (
            wrapped("<sacm:ArgumentPackage xmi:id=\"a\"/><zqxchild/>"),
            Some("zqxchild"),
        ),
        (format!("<xmi:XMI xmlns:xmi=\"{XMI_NS}\"/>"), None),
    ];
    for (input, name) in &refusals {
        let error = refusal(row, input.as_bytes());
        assert!(
            matches!(
                error,
                ReadError::InvalidRoot { .. } | ReadError::NoPackage { .. }
            ),
            "{row}: {} is not refused as SACM-XMI-001: {error:?}",
            preview(input.as_bytes())
        );
        assert_eq!(
            error.code(),
            Some(Code::XMI_INVALID_ROOT),
            "{row}: the refusal of {} does not carry SACM-XMI-001",
            preview(input.as_bytes())
        );
        assert!(
            error.location().is_some(),
            "{row}: the refusal of {} has no location",
            preview(input.as_bytes())
        );
        assert!(
            error.source().is_none(),
            "{row}: the refusal of {} has a source",
            preview(input.as_bytes())
        );
        let text = error.to_string();
        assert_one_lowercase_line(row, &text);
        if let Some(name) = name {
            let quoted = format!("{name:?}");
            assert!(
                text.contains(&quoted),
                "{row}: Display {text:?} does not quote {name:?}"
            );
            for (index, _) in text.match_indices(name) {
                assert!(
                    text[..index].matches('"').count() % 2 == 1,
                    "{row}: Display {text:?} names {name:?} without quoting it"
                );
            }
        }
        assert_eq!(
            error.clone(),
            error,
            "{row}: a clone of {error:?} is not equal to it"
        );
    }
}

#[test]
fn sacm23_xmi_001_xmi_invalid_root_code_is_sacm_xmi_001_in_the_catalog() {
    let row = "SACM23-XMI-001";
    assert_eq!(
        Code::XMI_INVALID_ROOT.as_str(),
        "SACM-XMI-001",
        "{row}: Code::XMI_INVALID_ROOT is not SACM-XMI-001"
    );
    assert!(
        catalog_has_row(row, Code::XMI_INVALID_ROOT),
        "{row}: docs/sacm/diagnostics.md has no table row for SACM-XMI-001"
    );
}

// --- LIB-SEC-002 8. The SACM-XMI-001 locations count lines as XML does -------

#[test]
fn lib_sec_002_invalid_root_and_no_package_locations_count_lines_as_xml_does() {
    let row = "LIB-SEC-002";
    // The inputs of the former Unsupported-root test: `<r/>` is SACM-XMI-001
    // now, at the same place.
    let line_ends: [(&str, u32, u32); 5] = [
        ("<!--p-->\r", 2, 1),
        ("<!--p-->\r\n", 2, 1),
        ("<!--p-->\r\r", 3, 1),
        ("<!--p-->\n\r", 3, 1),
        ("<!--p-->\r\n\r  ", 3, 3),
    ];
    let empty_wrapper = format!("<xmi:XMI xmlns:xmi=\"{XMI_NS}\"/>");
    for (prolog, line, column) in line_ends {
        let root = format!("{prolog}<r/>");
        assert_invalid_root_at(row, root.as_bytes(), loc(row, line, column), "r");
        let wrapper = format!("{prolog}{empty_wrapper}");
        assert_no_package_at(row, wrapper.as_bytes(), loc(row, line, column));
    }
    // A wrapped Claim after a lone CR and a CRLF.
    let claim = wrapped("\r<sacm:ArgumentPackage xmi:id=\"a\"/>\r\n  <sacm:Claim xmi:id=\"c\"/>");
    assert_invalid_root_at(row, claim.as_bytes(), loc(row, 3, 3), "sacm:Claim");
}

#[test]
fn lib_sec_002_leading_utf8_bom_takes_no_column_in_invalid_root_and_no_package() {
    let row = "LIB-SEC-002";
    // The row's example: the root of `\xEF\xBB\xBF<r/>` is at line 1, column 1.
    assert_invalid_root_at(row, b"\xEF\xBB\xBF<r/>", loc(row, 1, 1), "r");
    assert_invalid_root_at(row, b"\xEF\xBB\xBF<!--p--><r/>", loc(row, 1, 9), "r");
    assert_invalid_root_at(
        row,
        b"\xEF\xBB\xBF<?xml version=\"1.0\"?>\n<r/>",
        loc(row, 2, 1),
        "r",
    );
    let empty_wrapper = format!("\u{FEFF}<xmi:XMI xmlns:xmi=\"{XMI_NS}\"/>");
    assert_no_package_at(row, empty_wrapper.as_bytes(), loc(row, 1, 1));
}

// --- SACM23-XMI-001. Exact results ---------------------------------------------

/// Asserts that `input` is refused as `Unsupported` at `location` with
/// exactly `reason` and the exact `Display`.
fn assert_unsupported_exactly(row: &str, input: &[u8], location: SourceLocation, reason: &str) {
    let error = assert_unsupported_at(row, input, location);
    let ReadError::Unsupported { reason: found, .. } = &error else {
        panic!(
            "{row}: expected Unsupported for {}, got {error:?}",
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
        format!("the reader cannot represent the input at {location}: {reason}"),
        "{row}: unexpected Display for {}",
        preview(input)
    );
}

#[test]
fn sacm23_xmi_001_an_empty_id_is_refused_before_an_unread_attribute() {
    let row = "SACM23-XMI-001";
    // The empty id decides, at the root, even with `foo` after it.
    let input = format!("<sacm:ArgumentPackage{NS} xmi:id=\"\" foo=\"1\"/>");
    assert_empty_id_at(row, input.as_bytes(), loc(row, 1, 1));
}

#[test]
fn sacm23_xmi_001_text_after_white_space_is_refused_at_its_first_character() {
    let row = "SACM23-XMI-001";
    // The text node starts on line 1; its first non-white-space character is
    // at line 2, column 3.
    let input = format!("<sacm:ArgumentPackage{NS} xmi:id=\"p\">\n  x</sacm:ArgumentPackage>");
    assert_unsupported_exactly(
        row,
        input.as_bytes(),
        loc(row, 2, 3),
        "the element \"sacm:ArgumentPackage\" holds text, which is not read in this release",
    );
}
