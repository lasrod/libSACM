//! Tests that `sacm::xmi::read` matches names by expanded name and resolves
//! type values against the namespaces in scope, so the choice of prefix never
//! changes the result (SACM23-XMI-002; Namespaces in XML 1.0 §4, §6.1-§6.3;
//! XMI 2.5.1 §7.2, rule 1h; XML Schema 1.0 Part 1 §3.15.3, Part 2 §4.3.6;
//! ADR 0016 D2, D6).
//!
//! Every input is an inline string. `read` is called with
//! `ReadOptions::default()`.
//!
//! "The same document" (item 1) is compared on a projection of the document:
//! the roots in order with their ids and kinds; every element's id, kind,
//! parent, children and name; the element count; and the diagnostics apart
//! from their locations (code, severity, requirement id, clause, affected ids
//! and message). The references are SACM23-XMI-001's `sacm:`/`xmi:` inputs,
//! which are read first, so a reference that stops reading fails before any
//! variant is compared with it. A reference has no diagnostics, except the
//! ones with an `xsi:type` or an `xmi:version`, which carry SACM-XMI-010 (a
//! Warning in the default tolerant mode) or SACM-XMI-011 (Info)
//! (LIB-COMPAT-001); a variant with that attribute is compared with them.
//!
//! A root is refused at the `<` of its start tag and an attribute at the
//! first character of its name, as in the SACM23-XMI-001 tests.

use std::mem;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::Duration;

use sacm::diagnostic::{Code, Diagnostic, Severity, SourceLocation};
use sacm::model::{Document, ElementKind, LangString};
use sacm::xmi::{self, ReadError, ReadOptions, ReadOutput};

const ROW: &str = "SACM23-XMI-002";

/// The pinned SACM 2.3 namespace (D-NS-PIN).
const SACM_NS: &str = "http://www.omg.org/spec/SACM/20220301";
/// The XMI 2.5.1 namespace.
const XMI_NS: &str = "http://www.omg.org/spec/XMI/20131001";
/// The XML Schema instance namespace, of `xsi:type`.
const XSI_NS: &str = "http://www.w3.org/2001/XMLSchema-instance";

/// SACM23-XMI-001 item 1's input: the AssuranceCasePackage reference.
const ACP_ROOT: &str = concat!(
    "<sacm:AssuranceCasePackage",
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
    " xmi:id=\"acp_1\"/>",
);

/// SACM23-XMI-001 item 2's input: the ArgumentPackage reference.
const AP_ROOT: &str = concat!(
    "<sacm:ArgumentPackage",
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
    " xmi:id=\"arg_1\"/>",
);

/// SACM23-XMI-001 item 3's bare form: the reference for ArgumentPackage `a`.
const BARE_A: &str = concat!(
    "<sacm:ArgumentPackage",
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
    " xmi:id=\"a\"/>",
);

/// SACM23-XMI-001 item 3's wrapped form, with `sacm` and `xmi`.
const WRAPPED_A: &str = concat!(
    "<xmi:XMI xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\">",
    "<sacm:ArgumentPackage xmi:id=\"a\"/></xmi:XMI>",
);

/// SACM23-XMI-001 item 7's redundant `xsi:type`, on [`BARE_A`]: the reference
/// for a variant with an XSI type, which carries one SACM-XMI-010.
const BARE_A_XSI_TYPE: &str = concat!(
    "<sacm:ArgumentPackage",
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
    " xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"",
    " xmi:id=\"a\" xsi:type=\"sacm:ArgumentPackage\"/>",
);

/// SACM23-XMI-001 item 6's `xmi:version="2.0"` on [`BARE_A`]: the reference
/// for a variant with an XMI version on the root, which carries one
/// SACM-XMI-011.
const BARE_A_VERSION: &str = concat!(
    "<sacm:ArgumentPackage",
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
    " xmi:id=\"a\" xmi:version=\"2.0\"/>",
);

/// SACM23-XMI-001 item 6's `xmi:version="2.0"` on [`WRAPPED_A`]'s wrapper: the
/// reference for a variant with an XMI version on the wrapper, which carries
/// one SACM-XMI-011 with no affected id.
const WRAPPED_A_VERSION: &str = concat!(
    "<xmi:XMI xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\" xmi:version=\"2.0\">",
    "<sacm:ArgumentPackage xmi:id=\"a\"/></xmi:XMI>",
);

// --- Helpers --------------------------------------------------------------

/// How long one `read` may take; see the SACM23-XMI-001 tests.
const DEADLINE: Duration = Duration::from_secs(5);

/// `xmi::read` with the default options, on a worker thread. Fails the test
/// when the read panics or does not return within [`DEADLINE`].
fn read(input: &[u8]) -> Result<ReadOutput, ReadError> {
    let owned = input.to_vec();
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        // The receiver is gone only after the deadline has already failed the test.
        let _ = sender.send(xmi::read(&owned, &ReadOptions::default()));
    });
    match receiver.recv_timeout(DEADLINE) {
        Ok(result) => result,
        Err(RecvTimeoutError::Timeout) => panic!(
            "{ROW}: read did not return within {DEADLINE:?} for {}",
            preview(input)
        ),
        Err(RecvTimeoutError::Disconnected) => {
            panic!("{ROW}: read panicked on {}", preview(input))
        }
    }
}

/// The first bytes of `input`, for assertion messages.
fn preview(input: &[u8]) -> String {
    let end = input.len().min(200);
    let text = String::from_utf8_lossy(&input[..end]);
    if end < input.len() {
        format!("{text:?}... ({} bytes)", input.len())
    } else {
        format!("{text:?}")
    }
}

/// Reads `input`, which the test expects to be read.
fn read_ok(input: &[u8]) -> ReadOutput {
    match read(input) {
        Ok(output) => output,
        Err(error) => panic!(
            "{ROW}: read refused {}: {error:?} ({error})",
            preview(input)
        ),
    }
}

/// Reads `input`, which the test expects to be refused, and returns why.
fn refusal(input: &[u8]) -> ReadError {
    match read(input) {
        Ok(output) => panic!(
            "{ROW}: read returned a document for {}: {:?}",
            preview(input),
            shape(output.document())
        ),
        Err(error) => error,
    }
}

/// A 1-based location that the test expects `SourceLocation::new` to accept.
fn loc(line: u32, column: u32) -> SourceLocation {
    let Some(location) = SourceLocation::new(line, column) else {
        panic!("{ROW}: SourceLocation::new({line}, {column}) refused a 1-based position");
    };
    location
}

/// The location of the first occurrence of `needle` in `text`, which must
/// end its lines with LF only: the line, and the column in characters.
fn position_of(text: &str, needle: &str) -> SourceLocation {
    assert!(
        !text.contains('\r'),
        "{ROW}: position_of counts LF line ends only"
    );
    let Some(offset) = text.find(needle) else {
        panic!("{ROW}: the test input {text:?} does not contain {needle:?}");
    };
    let before = &text[..offset];
    let line = before.matches('\n').count() + 1;
    let column = before.rsplit('\n').next().unwrap_or("").chars().count() + 1;
    let (Ok(line), Ok(column)) = (u32::try_from(line), u32::try_from(column)) else {
        panic!("{ROW}: the test input is too large to locate");
    };
    loc(line, column)
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

/// One diagnostic of a [`Shape`], apart from its location.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Unlocated {
    code: Code,
    severity: Severity,
    requirement: String,
    clause: Option<String>,
    affected: Vec<String>,
    message: String,
}

fn unlocated(diagnostic: &Diagnostic) -> Unlocated {
    Unlocated {
        code: diagnostic.code(),
        severity: diagnostic.severity(),
        requirement: diagnostic.requirement_id().to_owned(),
        clause: diagnostic.clause().map(str::to_owned),
        affected: diagnostic
            .affected()
            .iter()
            .map(|id| id.as_str().to_owned())
            .collect(),
        message: diagnostic.message().to_owned(),
    }
}

/// What two documents must share to be "the same document" (item 1).
#[derive(Debug, PartialEq, Eq)]
struct Shape {
    /// The roots, in order: id and kind.
    roots: Vec<(String, ElementKind)>,
    /// Every element, pre-order.
    elements: Vec<Entry>,
    /// `Document::element_count`.
    count: usize,
    /// The read's diagnostics, in order, apart from their locations.
    diagnostics: Vec<Unlocated>,
}

/// The shape of a document, with no diagnostics.
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
        diagnostics: Vec::new(),
    }
}

/// The shape of a document whose only elements are the given parentless,
/// unnamed, childless roots, in order, with no diagnostics.
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
        diagnostics: Vec::new(),
    }
}

/// Reads `input` and returns its shape, with the read's diagnostics.
fn read_shape(input: &str) -> Shape {
    let output = read_ok(input.as_bytes());
    let mut found = shape(output.document());
    found.diagnostics = output.diagnostics().iter().map(unlocated).collect();
    found
}

/// Reads the SACM23-XMI-001 `reference`, asserts that it is `expected` with no
/// diagnostics, and returns its shape.
fn reference(reference: &str, expected: &[(&str, ElementKind)]) -> Shape {
    reported_reference(reference, expected, &[])
}

/// Reads the SACM23-XMI-001 `reference`, asserts that it is `expected` with
/// diagnostics of exactly the codes and severities `reported`, in order, and
/// returns its shape.
fn reported_reference(
    reference: &str,
    expected: &[(&str, ElementKind)],
    reported: &[(Code, Severity)],
) -> Shape {
    let found = read_shape(reference);
    let codes: Vec<(Code, Severity)> = found
        .diagnostics
        .iter()
        .map(|diagnostic| (diagnostic.code, diagnostic.severity))
        .collect();
    assert_eq!(
        codes,
        reported,
        "{ROW}: the reference {} does not report the expected diagnostics: {:?}",
        preview(reference.as_bytes()),
        found.diagnostics
    );
    let mut document = roots_shape(expected);
    document.diagnostics.clone_from(&found.diagnostics);
    assert_eq!(
        found,
        document,
        "{ROW}: the reference {} is not the expected document",
        preview(reference.as_bytes())
    );
    found
}

/// The reference for an ArgumentPackage `a` with a redundant `xsi:type`: one
/// SACM-XMI-010, a Warning in the default tolerant mode (LIB-COMPAT-001).
fn xsi_type_reference() -> Shape {
    reported_reference(
        BARE_A_XSI_TYPE,
        &[("a", ElementKind::ArgumentPackage)],
        &[(Code::XMI_XSI_TYPE, Severity::Warning)],
    )
}

/// The reference for an ArgumentPackage `a` with an `xmi:version`, on the
/// root or on the wrapper: one SACM-XMI-011, Info (LIB-COMPAT-001).
fn version_reference(input: &str) -> Shape {
    reported_reference(
        input,
        &[("a", ElementKind::ArgumentPackage)],
        &[(Code::XMI_VERSION_IGNORED, Severity::Info)],
    )
}

/// Asserts that `input` gives the same document as the reference whose
/// shape is `reference`, with the same diagnostics apart from their
/// locations.
fn assert_same_document(what: &str, input: &str, reference: &Shape) {
    let found = read_shape(input);
    assert_eq!(
        &found,
        reference,
        "{ROW}: {what}: {} does not give the same document as the reference",
        preview(input.as_bytes())
    );
}

/// Asserts that `input` is refused as `Unsupported`, with no code, at
/// `location`, and returns the reason.
fn assert_unsupported_at(what: &str, input: &str, location: SourceLocation) -> String {
    let error = refusal(input.as_bytes());
    let ReadError::Unsupported { reason, .. } = &error else {
        panic!(
            "{ROW}: {what}: expected Unsupported for {}, got {error:?}",
            preview(input.as_bytes())
        );
    };
    assert_eq!(
        error.code(),
        None,
        "{ROW}: {what}: the Unsupported refusal of {} carries a code",
        preview(input.as_bytes())
    );
    assert_eq!(
        error.location(),
        Some(location),
        "{ROW}: {what}: the Unsupported refusal of {} is not at the expected place: {error}",
        preview(input.as_bytes())
    );
    reason.clone()
}

/// Asserts that `input` is refused as `Malformed` (SACM-XML-001) at
/// `location`.
fn assert_malformed_at(what: &str, input: &str, location: SourceLocation) {
    let error = refusal(input.as_bytes());
    assert!(
        matches!(error, ReadError::Malformed { .. }),
        "{ROW}: {what}: expected Malformed for {}, got {error:?}",
        preview(input.as_bytes())
    );
    assert_eq!(
        error.code(),
        Some(Code::XML_MALFORMED),
        "{ROW}: {what}: the Malformed refusal of {} does not carry SACM-XML-001",
        preview(input.as_bytes())
    );
    assert_eq!(
        error.location(),
        Some(location),
        "{ROW}: {what}: the Malformed refusal of {} is not at the second attribute: {error}",
        preview(input.as_bytes())
    );
}

/// Asserts that `input` is refused as `InvalidRoot` (SACM-XMI-001) at
/// `location`, naming `name` as written, with the exact `Display`.
fn assert_invalid_root_at(input: &str, location: SourceLocation, name: &str) {
    let error = refusal(input.as_bytes());
    let ReadError::InvalidRoot {
        location: field,
        element_name,
        ..
    } = &error
    else {
        panic!(
            "{ROW}: expected InvalidRoot for {}, got {error:?}",
            preview(input.as_bytes())
        );
    };
    assert_eq!(
        element_name,
        name,
        "{ROW}: the InvalidRoot of {} does not name the element as written",
        preview(input.as_bytes())
    );
    assert_eq!(
        *field,
        Some(location),
        "{ROW}: the InvalidRoot of {} is not at the `<` of {name:?}",
        preview(input.as_bytes())
    );
    assert_eq!(
        error.code(),
        Some(Code::XMI_INVALID_ROOT),
        "{ROW}: the InvalidRoot of {} does not carry SACM-XMI-001",
        preview(input.as_bytes())
    );
    assert_eq!(
        error.to_string(),
        format!("the root element {name:?} is not a SACM package at {location}"),
        "{ROW}: unexpected InvalidRoot Display for {}",
        preview(input.as_bytes())
    );
}

/// Asserts that `input` is refused as `NoPackage` (SACM-XMI-001) at
/// `location`.
fn assert_no_package_at(input: &str, location: SourceLocation) {
    let error = refusal(input.as_bytes());
    let ReadError::NoPackage {
        location: field, ..
    } = &error
    else {
        panic!(
            "{ROW}: expected NoPackage for {}, got {error:?}",
            preview(input.as_bytes())
        );
    };
    assert_eq!(
        *field,
        Some(location),
        "{ROW}: the NoPackage of {} is not at the wrapper",
        preview(input.as_bytes())
    );
    assert_eq!(
        error.code(),
        Some(Code::XMI_INVALID_ROOT),
        "{ROW}: the NoPackage of {} does not carry SACM-XMI-001",
        preview(input.as_bytes())
    );
}

/// A bare ArgumentPackage `a` with `sacm`, `xmi` and `xsi` declared and the
/// type `value` in `attribute` (`xsi:type` or `xmi:type`), all on line 1.
fn typed_root(attribute: &str, value: &str) -> String {
    format!(
        "<sacm:ArgumentPackage xmlns:sacm=\"{SACM_NS}\" xmlns:xmi=\"{XMI_NS}\" \
         xmlns:xsi=\"{XSI_NS}\" xmi:id=\"a\" {attribute}=\"{value}\"/>"
    )
}

// --- 1. The references --------------------------------------------------------

#[test]
fn sacm23_xmi_002_the_references_are_parentless_unnamed_roots_without_diagnostics() {
    // `reference` asserts the ids and kinds, parent `None`, name `None`, the
    // element count and empty diagnostics. The references with an attribute
    // that is reported carry exactly that one diagnostic (LIB-COMPAT-001).
    reference(ACP_ROOT, &[("acp_1", ElementKind::AssuranceCasePackage)]);
    reference(AP_ROOT, &[("arg_1", ElementKind::ArgumentPackage)]);
    reference(BARE_A, &[("a", ElementKind::ArgumentPackage)]);
    reference(WRAPPED_A, &[("a", ElementKind::ArgumentPackage)]);
    let typed = xsi_type_reference();
    assert_eq!(
        typed.diagnostics[0].affected,
        ["a"],
        "{ROW}: the SACM-XMI-010 of the xsi:type reference does not affect the root"
    );
    let on_root = version_reference(BARE_A_VERSION);
    assert_eq!(
        on_root.diagnostics[0].affected,
        ["a"],
        "{ROW}: the SACM-XMI-011 of xmi:version on the root does not affect the root"
    );
    let on_wrapper = version_reference(WRAPPED_A_VERSION);
    assert_eq!(
        on_wrapper.diagnostics[0].affected,
        Vec::<String>::new(),
        "{ROW}: the SACM-XMI-011 of xmi:version on the wrapper affects an element"
    );
}

// --- 2. The SACM prefix -------------------------------------------------------

#[test]
fn sacm23_xmi_002_any_sacm_prefix_gives_the_same_document() {
    let acp = reference(ACP_ROOT, &[("acp_1", ElementKind::AssuranceCasePackage)]);
    let ap = reference(AP_ROOT, &[("arg_1", ElementKind::ArgumentPackage)]);
    let both = reference(
        &format!(
            "<xmi:XMI xmlns:xmi=\"{XMI_NS}\" xmlns:sacm=\"{SACM_NS}\">\
             <sacm:AssuranceCasePackage xmi:id=\"acp_1\"/>\
             <sacm:ArgumentPackage xmi:id=\"arg_1\"/></xmi:XMI>"
        ),
        &[
            ("acp_1", ElementKind::AssuranceCasePackage),
            ("arg_1", ElementKind::ArgumentPackage),
        ],
    );
    for p in ["s", "ns0", "SACM", "ü"] {
        let declarations = format!(" xmlns:{p}=\"{SACM_NS}\" xmlns:xmi=\"{XMI_NS}\"");
        assert_same_document(
            &format!("the prefix {p:?} on an AssuranceCasePackage"),
            &format!("<{p}:AssuranceCasePackage{declarations} xmi:id=\"acp_1\"/>"),
            &acp,
        );
        assert_same_document(
            &format!("the prefix {p:?} on an ArgumentPackage"),
            &format!("<{p}:ArgumentPackage{declarations} xmi:id=\"arg_1\"/>"),
            &ap,
        );
        assert_same_document(
            &format!("the prefix {p:?} in a wrapper"),
            &format!(
                "<xmi:XMI{declarations}><{p}:AssuranceCasePackage xmi:id=\"acp_1\"/>\
                 <{p}:ArgumentPackage xmi:id=\"arg_1\"/></xmi:XMI>"
            ),
            &both,
        );
    }
}

// --- 3. Default namespaces ----------------------------------------------------

#[test]
fn sacm23_xmi_002_a_default_sacm_or_xmi_namespace_gives_the_same_document() {
    let a = reference(BARE_A, &[("a", ElementKind::ArgumentPackage)]);
    let acp = reference(ACP_ROOT, &[("acp_1", ElementKind::AssuranceCasePackage)]);
    let cases = [
        // The row's two inputs, exactly.
        (
            "the default namespace SACM on a bare root",
            format!("<ArgumentPackage xmlns=\"{SACM_NS}\" xmlns:xmi=\"{XMI_NS}\" xmi:id=\"a\"/>"),
        ),
        (
            "the default namespace XMI on the wrapper",
            format!(
                "<XMI xmlns=\"{XMI_NS}\" xmlns:s=\"{SACM_NS}\">\
                 <s:ArgumentPackage xmlns:x=\"{XMI_NS}\" x:id=\"a\"/></XMI>"
            ),
        ),
        (
            "the default namespace SACM declared on the wrapper",
            format!(
                "<xmi:XMI xmlns:xmi=\"{XMI_NS}\" xmlns=\"{SACM_NS}\">\
                 <ArgumentPackage xmi:id=\"a\"/></xmi:XMI>"
            ),
        ),
    ];
    for (what, input) in &cases {
        assert_same_document(what, input, &a);
    }
    assert_same_document(
        "the default namespace SACM on an AssuranceCasePackage",
        &format!(
            "<AssuranceCasePackage xmlns=\"{SACM_NS}\" xmlns:xmi=\"{XMI_NS}\" xmi:id=\"acp_1\"/>"
        ),
        &acp,
    );
}

// --- 4. The XMI and XSI prefixes ------------------------------------------------

#[test]
fn sacm23_xmi_002_any_xmi_and_xsi_prefixes_give_the_same_document() {
    let a = reference(BARE_A, &[("a", ElementKind::ArgumentPackage)]);
    let a_versioned = version_reference(BARE_A_VERSION);
    let wrapper_versioned = version_reference(WRAPPED_A_VERSION);
    let a_typed = xsi_type_reference();
    let x = format!(" xmlns:sacm=\"{SACM_NS}\" xmlns:x=\"{XMI_NS}\"");
    let cases = [
        ("x:id", format!("<sacm:ArgumentPackage{x} x:id=\"a\"/>"), &a),
        (
            "x:version",
            format!("<sacm:ArgumentPackage{x} x:id=\"a\" x:version=\"2.0\"/>"),
            &a_versioned,
        ),
        (
            "x:type",
            format!("<sacm:ArgumentPackage{x} x:id=\"a\" x:type=\"sacm:ArgumentPackage\"/>"),
            &a,
        ),
        (
            "an x:XMI wrapper",
            format!("<x:XMI{x}><sacm:ArgumentPackage x:id=\"a\"/></x:XMI>"),
            &a,
        ),
        (
            "x:version on an x:XMI wrapper",
            format!("<x:XMI{x} x:version=\"2.0\"><sacm:ArgumentPackage x:id=\"a\"/></x:XMI>"),
            &wrapper_versioned,
        ),
        (
            "i:type",
            format!(
                "<sacm:ArgumentPackage{x} xmlns:i=\"{XSI_NS}\" x:id=\"a\" \
                 i:type=\"sacm:ArgumentPackage\"/>"
            ),
            &a_typed,
        ),
    ];
    for (what, input, reference) in &cases {
        assert_same_document(what, input, reference);
    }
}

// --- 5. Declaration placement ---------------------------------------------------

#[test]
fn sacm23_xmi_002_declarations_on_the_wrapper_or_the_root_give_the_same_document() {
    let a = reference(BARE_A, &[("a", ElementKind::ArgumentPackage)]);
    let a_typed = xsi_type_reference();
    let cases = [
        (
            "the SACM declaration on the root only",
            format!(
                "<xmi:XMI xmlns:xmi=\"{XMI_NS}\">\
                 <sacm:ArgumentPackage xmlns:sacm=\"{SACM_NS}\" xmi:id=\"a\"/></xmi:XMI>"
            ),
        ),
        (
            "the XMI declaration of the id on the root only",
            format!(
                "<x:XMI xmlns:x=\"{XMI_NS}\" xmlns:sacm=\"{SACM_NS}\">\
                 <sacm:ArgumentPackage xmlns:xmi=\"{XMI_NS}\" xmi:id=\"a\"/></x:XMI>"
            ),
        ),
        (
            "the XSI declaration on the wrapper only",
            format!(
                "<xmi:XMI xmlns:xmi=\"{XMI_NS}\" xmlns:xsi=\"{XSI_NS}\">\
                 <sacm:ArgumentPackage xmlns:sacm=\"{SACM_NS}\" xmi:id=\"a\" \
                 xsi:type=\"sacm:ArgumentPackage\"/></xmi:XMI>"
            ),
        ),
        (
            "the type-value prefix declared on the wrapper only",
            format!(
                "<xmi:XMI xmlns:xmi=\"{XMI_NS}\" xmlns:t=\"{SACM_NS}\">\
                 <sacm:ArgumentPackage xmlns:sacm=\"{SACM_NS}\" xmlns:xsi=\"{XSI_NS}\" \
                 xmi:id=\"a\" xsi:type=\"t:ArgumentPackage\"/></xmi:XMI>"
            ),
        ),
        (
            "two prefixes bound to SACM, one for the element, one in xsi:type",
            format!(
                "<p:ArgumentPackage xmlns:p=\"{SACM_NS}\" xmlns:q=\"{SACM_NS}\" \
                 xmlns:xmi=\"{XMI_NS}\" xmlns:xsi=\"{XSI_NS}\" xmi:id=\"a\" \
                 xsi:type=\"q:ArgumentPackage\"/>"
            ),
        ),
    ];
    for (what, input) in &cases {
        let reference = if input.contains("xsi:type") {
            &a_typed
        } else {
            &a
        };
        assert_same_document(what, input, reference);
    }
}

// --- 6. Rebinding ---------------------------------------------------------------

#[test]
fn sacm23_xmi_002_a_prefix_rebound_to_sacm_on_the_root_takes_the_inner_binding() {
    let a = reference(WRAPPED_A, &[("a", ElementKind::ArgumentPackage)]);
    let a_typed = xsi_type_reference();
    let cases = [
        // The row's input, exactly: `p` is XMI on the wrapper, SACM on the root.
        (
            "p rebound from XMI to SACM",
            format!(
                "<p:XMI xmlns:p=\"{XMI_NS}\">\
                 <p:ArgumentPackage xmlns:p=\"{SACM_NS}\" xmlns:x=\"{XMI_NS}\" x:id=\"a\"/></p:XMI>"
            ),
        ),
        (
            "t rebound from urn:other to SACM in the type",
            format!(
                "<xmi:XMI xmlns:xmi=\"{XMI_NS}\" xmlns:sacm=\"{SACM_NS}\" \
                 xmlns:xsi=\"{XSI_NS}\" xmlns:t=\"urn:other\">\
                 <sacm:ArgumentPackage xmlns:t=\"{SACM_NS}\" xmi:id=\"a\" \
                 xsi:type=\"t:ArgumentPackage\"/></xmi:XMI>"
            ),
        ),
    ];
    for (what, input) in &cases {
        let reference = if input.contains("xsi:type") {
            &a_typed
        } else {
            &a
        };
        assert_same_document(what, input, reference);
    }
}

#[test]
fn sacm23_xmi_002_a_prefix_rebound_away_from_sacm_on_the_root_is_unsupported_at_the_root() {
    let type_away = format!(
        "<xmi:XMI xmlns:xmi=\"{XMI_NS}\" xmlns:sacm=\"{SACM_NS}\" \
         xmlns:xsi=\"{XSI_NS}\" xmlns:t=\"{SACM_NS}\">\n\
         <sacm:ArgumentPackage xmlns:t=\"urn:other\" xmi:id=\"a\" \
         xsi:type=\"t:ArgumentPackage\"/>\n</xmi:XMI>"
    );
    assert_unsupported_at(
        "t rebound from SACM to urn:other in the type",
        &type_away,
        position_of(&type_away, "<sacm:ArgumentPackage"),
    );
    let element_away = format!(
        "<xmi:XMI xmlns:xmi=\"{XMI_NS}\" xmlns:s=\"{SACM_NS}\">\n\
         <s:ArgumentPackage xmlns:s=\"urn:other\" xmi:id=\"a\"/>\n</xmi:XMI>"
    );
    assert_unsupported_at(
        "s rebound from SACM to urn:other on the root",
        &element_away,
        position_of(&element_away, "<s:ArgumentPackage"),
    );
}

// --- 7. Type values -------------------------------------------------------------

#[test]
fn sacm23_xmi_002_an_unprefixed_type_under_a_default_sacm_namespace_reads() {
    let a = xsi_type_reference();
    let input = format!(
        "<ArgumentPackage xmlns=\"{SACM_NS}\" xmlns:xmi=\"{XMI_NS}\" xmlns:xsi=\"{XSI_NS}\" \
         xmi:id=\"a\" xsi:type=\"ArgumentPackage\"/>"
    );
    assert_same_document("an unprefixed type under a default SACM", &input, &a);
}

#[test]
fn sacm23_xmi_002_white_space_around_a_type_value_is_collapsed_before_it_is_resolved() {
    let a = reference(BARE_A, &[("a", ElementKind::ArgumentPackage)]);
    let a_typed = xsi_type_reference();
    // A literal tab or line feed in an attribute value is a space after XML
    // attribute-value normalization (XML 1.0 §3.3.3); a character reference
    // keeps the character, so both are given. XSD collapse removes leading and
    // trailing #x9, #xA, #xD and #x20 (Part 2 §4.3.6).
    let values = [
        " sacm:ArgumentPackage ",
        "sacm:ArgumentPackage ",
        "  sacm:ArgumentPackage",
        "\tsacm:ArgumentPackage\t",
        "\nsacm:ArgumentPackage\n",
        "&#9;sacm:ArgumentPackage&#9;",
        "&#10;sacm:ArgumentPackage&#10;",
        "&#13;&#10; sacm:ArgumentPackage &#x9;",
    ];
    for value in values {
        for (attribute, reference) in [("xsi:type", &a_typed), ("xmi:type", &a)] {
            assert_same_document(
                &format!("{attribute}={value:?}"),
                &typed_root(attribute, value),
                reference,
            );
        }
    }
    // Unprefixed, under a default SACM namespace.
    let unprefixed = format!(
        "<ArgumentPackage xmlns=\"{SACM_NS}\" xmlns:xmi=\"{XMI_NS}\" xmlns:xsi=\"{XSI_NS}\" \
         xmi:id=\"a\" xsi:type=\"&#9; ArgumentPackage&#10;\"/>"
    );
    assert_same_document(
        "an unprefixed type with white space under a default SACM",
        &unprefixed,
        &a_typed,
    );
}

#[test]
fn sacm23_xmi_002_a_type_value_that_is_not_a_sacm_qname_of_the_root_class_is_unsupported_at_the_root()
 {
    let declarations = format!(
        " xmlns:sacm=\"{SACM_NS}\" xmlns:xmi=\"{XMI_NS}\" xmlns:xsi=\"{XSI_NS}\" xmi:id=\"a\""
    );
    let cases = [
        // The row's cases.
        (
            "an undeclared prefix",
            typed_root("xsi:type", "zz:ArgumentPackage"),
        ),
        (
            "an unprefixed value under xmlns=\"\"",
            format!(
                "<sacm:ArgumentPackage{declarations} xmlns=\"\" xsi:type=\"ArgumentPackage\"/>"
            ),
        ),
        // Under a default SACM namespace, so an empty prefix cannot fall back
        // to it.
        (
            "an empty prefix",
            format!(
                "<sacm:ArgumentPackage{declarations} xmlns=\"{SACM_NS}\" \
                 xsi:type=\":ArgumentPackage\"/>"
            ),
        ),
        ("an empty local name", typed_root("xsi:type", "sacm:")),
        // With `a` bound to SACM, so neither split of the value names SACM's
        // ArgumentPackage.
        (
            "two colons",
            format!(
                "<sacm:ArgumentPackage{declarations} xmlns:a=\"{SACM_NS}\" \
                 xsi:type=\"a:sacm:ArgumentPackage\"/>"
            ),
        ),
        (
            "white space inside the value",
            typed_root("xsi:type", "sacm:Argument Package"),
        ),
        // Collapsing white space must not make these readable.
        (
            "another class with white space around it",
            typed_root("xsi:type", " sacm:Claim "),
        ),
        (
            "an undeclared prefix with white space around it",
            typed_root("xsi:type", "&#9;zz:ArgumentPackage "),
        ),
        (
            "an unprefixed value with no default namespace, with white space around it",
            typed_root("xsi:type", " ArgumentPackage "),
        ),
        ("white space only", typed_root("xsi:type", " &#9; ")),
        // XSD collapse removes #x20, #x9, #xA and #xD only (Part 2 §4.3.6), so
        // other Unicode white space stays and the value names no class. One
        // side at a time, so trimming only the start or only the end is caught.
        (
            "U+00A0 before the value",
            typed_root("xsi:type", "&#xA0;sacm:ArgumentPackage"),
        ),
        (
            "U+00A0 after the value",
            typed_root("xsi:type", "sacm:ArgumentPackage&#xA0;"),
        ),
        (
            "U+0085 on both sides",
            typed_root("xsi:type", "&#x85;sacm:ArgumentPackage&#x85;"),
        ),
        (
            "an undeclared prefix in xmi:type",
            typed_root("xmi:type", "zz:ArgumentPackage"),
        ),
    ];
    for (what, input) in &cases {
        assert_unsupported_at(what, input, loc(1, 1));
    }
}

// --- 8. Spelling confers nothing ------------------------------------------------

#[test]
fn sacm23_xmi_002_a_prefix_spelled_xmi_or_xsi_but_bound_elsewhere_confers_nothing() {
    // `xmi:id` in urn:not-xmi is no XMI id: an attribute that is not read,
    // refused at its name. Without it the root has no XMI id, and is read
    // with a generated one (SACM23-XMI-020), so the attribute is the refusal.
    let not_xmi = format!(
        "<sacm:ArgumentPackage xmlns:sacm=\"{SACM_NS}\" xmlns:xmi=\"urn:not-xmi\" xmi:id=\"a\"/>"
    );
    let no_id =
        format!("<sacm:ArgumentPackage xmlns:sacm=\"{SACM_NS}\" xmlns:xmi=\"urn:not-xmi\"/>");
    assert_unsupported_at(
        "xmi bound to urn:not-xmi",
        &not_xmi,
        position_of(&not_xmi, "xmi:id"),
    );
    let read_without = read_ok(no_id.as_bytes());
    assert!(
        read_without.document().get("a").is_none() && read_without.document().element_count() == 1,
        "{ROW}: a root with no XMI id is not read with a generated id"
    );

    // `xsi:type` in urn:not-xsi is an attribute that is not read, at its name.
    let foreign_type = format!(
        "<sacm:ArgumentPackage xmlns:sacm=\"{SACM_NS}\" xmlns:xmi=\"{XMI_NS}\" \
         xmlns:xsi=\"urn:not-xsi\" xmi:id=\"a\"\n  xsi:type=\"sacm:ArgumentPackage\"/>"
    );
    assert_unsupported_at("xsi bound to urn:not-xsi", &foreign_type, loc(2, 3));

    // An unprefixed attribute is in no namespace, whatever the default
    // namespace (Namespaces §6.2).
    let default_xmi =
        format!("<s:ArgumentPackage xmlns=\"{XMI_NS}\" xmlns:s=\"{SACM_NS}\" id=\"a\"/>");
    let default_xmi_no_id =
        format!("<s:ArgumentPackage xmlns=\"{XMI_NS}\" xmlns:s=\"{SACM_NS}\"/>");
    assert_unsupported_at(
        "an unprefixed id under a default XMI",
        &default_xmi,
        position_of(&default_xmi, "id=\"a\""),
    );
    let read_without = read_ok(default_xmi_no_id.as_bytes());
    assert!(
        read_without.document().get("a").is_none() && read_without.document().element_count() == 1,
        "{ROW}: a root with no XMI id under a default XMI is not read with a generated id"
    );
    // The root element itself in the default namespace XMI.
    let root_in_xmi = format!("<ArgumentPackage xmlns=\"{XMI_NS}\" id=\"a\"/>");
    assert_unsupported_at(
        "a root in the default namespace XMI",
        &root_in_xmi,
        loc(1, 1),
    );
}

#[test]
fn sacm23_xmi_002_prefixes_spelled_after_another_namespace_are_that_namespace() {
    let a = reference(BARE_A, &[("a", ElementKind::ArgumentPackage)]);
    // `xmi:type` in XSI is an XSI type, so it carries SACM-XMI-010.
    let a_typed = xsi_type_reference();
    let cases = [
        (
            "xsi bound to SACM and sacm bound to XMI",
            format!(
                "<xsi:ArgumentPackage xmlns:xsi=\"{SACM_NS}\" xmlns:sacm=\"{XMI_NS}\" sacm:id=\"a\"/>"
            ),
            &a,
        ),
        (
            "xmi bound to XSI is a type",
            format!(
                "<s:ArgumentPackage xmlns:s=\"{SACM_NS}\" xmlns:x=\"{XMI_NS}\" \
                 xmlns:xmi=\"{XSI_NS}\" x:id=\"a\" xmi:type=\"s:ArgumentPackage\"/>"
            ),
            &a_typed,
        ),
        (
            "a sacm:XMI wrapper in XMI holding an xmi: root in SACM",
            format!(
                "<sacm:XMI xmlns:sacm=\"{XMI_NS}\" xmlns:xmi=\"{SACM_NS}\">\
                 <xmi:ArgumentPackage sacm:id=\"a\"/></sacm:XMI>"
            ),
            &a,
        ),
    ];
    for (what, input, reference) in &cases {
        assert_same_document(what, input, reference);
    }
    // `xmi:type` in XSI is a type, so naming another class is refused at the root.
    let other_class = format!(
        "<s:ArgumentPackage xmlns:s=\"{SACM_NS}\" xmlns:x=\"{XMI_NS}\" \
         xmlns:xmi=\"{XSI_NS}\" x:id=\"a\" xmi:type=\"s:Claim\"/>"
    );
    assert_unsupported_at("xmi bound to XSI, naming a Claim", &other_class, loc(1, 1));
}

// --- 9. Exact URIs --------------------------------------------------------------

#[test]
fn sacm23_xmi_002_namespace_uris_compare_as_exact_strings() {
    let near_sacm = [
        "http://www.omg.org/spec/SACM/20220301/",
        "HTTP://www.omg.org/spec/SACM/20220301",
    ];
    for uri in near_sacm {
        for kind in ["ArgumentPackage", "AssuranceCasePackage"] {
            let bare =
                format!("<sacm:{kind} xmlns:sacm=\"{uri}\" xmlns:xmi=\"{XMI_NS}\" xmi:id=\"a\"/>");
            assert_unsupported_at(&format!("{kind} in {uri:?}"), &bare, loc(1, 1));
        }
        let wrapped = format!(
            "<xmi:XMI xmlns:xmi=\"{XMI_NS}\" xmlns:sacm=\"{uri}\">\n\
             <sacm:ArgumentPackage xmi:id=\"a\"/>\n</xmi:XMI>"
        );
        assert_unsupported_at(&format!("a wrapped root in {uri:?}"), &wrapped, loc(2, 1));
    }
    // The XMI and XSI URIs too: a near XMI URI gives no XMI id, so its
    // `xmi:id` is an attribute that is not read, refused at its name (the root
    // itself would get a generated id, SACM23-XMI-020), or no XMI wrapper;
    // and a near XSI URI gives an attribute that is not read.
    let near_xmi = "http://www.omg.org/spec/XMI/20131001/";
    let id = format!(
        "<sacm:ArgumentPackage xmlns:sacm=\"{SACM_NS}\" xmlns:xmi=\"{near_xmi}\" xmi:id=\"a\"/>"
    );
    assert_unsupported_at("an id in a near XMI URI", &id, position_of(&id, "xmi:id"));
    let wrapper = format!(
        "<xmi:XMI xmlns:xmi=\"{near_xmi}\" xmlns:sacm=\"{SACM_NS}\">\
         <sacm:ArgumentPackage xmlns:x=\"{XMI_NS}\" x:id=\"a\"/></xmi:XMI>"
    );
    assert_unsupported_at("a wrapper in a near XMI URI", &wrapper, loc(1, 1));
    let type_in_near_xsi = format!(
        "<sacm:ArgumentPackage xmlns:sacm=\"{SACM_NS}\" xmlns:xmi=\"{XMI_NS}\" \
         xmlns:xsi=\"HTTP://www.w3.org/2001/XMLSchema-instance\" xmi:id=\"a\"\n  \
         xsi:type=\"sacm:ArgumentPackage\"/>"
    );
    assert_unsupported_at("a type in a near XSI URI", &type_in_near_xsi, loc(2, 3));
}

// --- 10. No namespace -----------------------------------------------------------

/// Asserts that `reason` says the root is in no namespace, and does not call
/// the empty string a namespace.
fn assert_says_no_namespace(what: &str, reason: &str) {
    assert!(
        !reason.contains("namespace \"\""),
        "{ROW}: {what}: the reason {reason:?} calls the empty string a namespace; \
         xmlns=\"\" means no namespace (Namespaces §6.2)"
    );
    assert!(
        reason.contains("no namespace"),
        "{ROW}: {what}: the reason {reason:?} does not say the element is in no namespace"
    );
}

#[test]
fn sacm23_xmi_002_a_root_under_an_empty_default_namespace_is_refused_as_in_no_namespace() {
    // The row's input, exactly.
    let bare = format!("<ArgumentPackage xmlns=\"\" xmlns:xmi=\"{XMI_NS}\" xmi:id=\"a\"/>");
    let reason = assert_unsupported_at("xmlns=\"\" on a bare root", &bare, loc(1, 1));
    assert_says_no_namespace("xmlns=\"\" on a bare root", &reason);
    // An AssuranceCasePackage, too.
    let acp = format!("<AssuranceCasePackage xmlns=\"\" xmlns:xmi=\"{XMI_NS}\" xmi:id=\"a\"/>");
    let reason = assert_unsupported_at("xmlns=\"\" on an AssuranceCasePackage", &acp, loc(1, 1));
    assert_says_no_namespace("xmlns=\"\" on an AssuranceCasePackage", &reason);
    // A default SACM namespace on the wrapper, undeclared on the root.
    let undeclared = format!(
        "<xmi:XMI xmlns:xmi=\"{XMI_NS}\" xmlns=\"{SACM_NS}\">\n\
         <ArgumentPackage xmlns=\"\" xmi:id=\"a\"/>\n</xmi:XMI>"
    );
    let reason = assert_unsupported_at(
        "a default SACM undeclared on a wrapped root",
        &undeclared,
        loc(2, 1),
    );
    assert_says_no_namespace("a default SACM undeclared on a wrapped root", &reason);
}

// --- 11. Duplicate expanded names ---------------------------------------------

#[test]
fn sacm23_xmi_002_one_attribute_under_two_prefixes_is_malformed_at_the_second() {
    let start = format!("<sacm:ArgumentPackage xmlns:sacm=\"{SACM_NS}\"");
    let cases = [
        // The row's attributes, exactly.
        (
            "a:id and b:id with the same value",
            format!("{start} xmlns:a=\"{XMI_NS}\" xmlns:b=\"{XMI_NS}\" a:id=\"p\"\n  b:id=\"p\"/>"),
        ),
        (
            "a:id and b:id with different values",
            format!("{start} xmlns:a=\"{XMI_NS}\" xmlns:b=\"{XMI_NS}\" a:id=\"p\"\n  b:id=\"q\"/>"),
        ),
        (
            "xmi:id and x:id",
            format!(
                "{start} xmlns:xmi=\"{XMI_NS}\" xmlns:x=\"{XMI_NS}\" xmi:id=\"p\"\n  x:id=\"p\"/>"
            ),
        ),
    ];
    for (what, input) in &cases {
        assert_malformed_at(what, input, loc(2, 3));
    }
    let wrapped = format!(
        "<xmi:XMI xmlns:xmi=\"{XMI_NS}\" xmlns:sacm=\"{SACM_NS}\" xmlns:x=\"{XMI_NS}\">\n\
         <sacm:ArgumentPackage xmi:id=\"p\" x:id=\"p\"/>\n</xmi:XMI>"
    );
    assert_malformed_at(
        "xmi:id and x:id on a wrapped root",
        &wrapped,
        position_of(&wrapped, "x:id=\"p\"/>"),
    );
}

// --- 12. Refusals do not depend on the prefix ---------------------------------

#[test]
fn sacm23_xmi_002_invalid_root_names_the_element_with_the_prefix_as_written() {
    assert_invalid_root_at(
        &format!("<s:Claim xmlns:s=\"{SACM_NS}\" xmlns:xmi=\"{XMI_NS}\" xmi:id=\"c\"/>"),
        loc(1, 1),
        "s:Claim",
    );
    assert_invalid_root_at(
        &format!("<Claim xmlns=\"{SACM_NS}\" xmlns:xmi=\"{XMI_NS}\" xmi:id=\"c\"/>"),
        loc(1, 1),
        "Claim",
    );
    assert_invalid_root_at(
        &format!("<ü:Claim xmlns:ü=\"{SACM_NS}\" xmlns:xmi=\"{XMI_NS}\" xmi:id=\"c\"/>"),
        loc(1, 1),
        "ü:Claim",
    );
}

#[test]
fn sacm23_xmi_002_a_wrapper_with_only_xmi_content_under_any_prefix_is_no_package() {
    assert_no_package_at(
        &format!("<x:XMI xmlns:x=\"{XMI_NS}\">\n  <x:documentation/>\n</x:XMI>"),
        loc(1, 1),
    );
    assert_no_package_at(
        &format!("<XMI xmlns=\"{XMI_NS}\">\n  <documentation/>\n</XMI>"),
        loc(1, 1),
    );
}

/// An input that SACM23-XMI-001 refuses, written with the given SACM prefix
/// and XMI prefix.
type Template = fn(&str, &str) -> String;

/// The refusals that must not depend on the prefix: what each is, its
/// template, and where it is refused. Each is at line 1, column 1, or on a
/// later line after a fixed indent, so its location does not depend on the
/// prefix length.
const REFUSALS: [(&str, Template, (u32, u32)); 6] = [
    (
        "a package kind not read yet",
        |s, x| {
            format!(
                "<{s}:ArtifactPackage xmlns:{s}=\"{SACM_NS}\" xmlns:{x}=\"{XMI_NS}\" {x}:id=\"f\"/>"
            )
        },
        (1, 1),
    ),
    // A root with no id is read (SACM23-XMI-020); an id in no namespace is
    // an attribute that is not read.
    (
        "a root with an id in no namespace",
        |s, x| {
            format!(
                "<{s}:ArgumentPackage xmlns:{s}=\"{SACM_NS}\" xmlns:{x}=\"{XMI_NS}\"\n  id=\"a\"/>"
            )
        },
        (2, 3),
    ),
    (
        "a type naming another class",
        |s, x| {
            format!(
                "<{s}:ArgumentPackage xmlns:{s}=\"{SACM_NS}\" xmlns:{x}=\"{XMI_NS}\" \
                     {x}:id=\"a\" {x}:type=\"{s}:Claim\"/>"
            )
        },
        (1, 1),
    ),
    (
        "an attribute not read",
        |s, x| {
            format!(
                "<{s}:ArgumentPackage xmlns:{s}=\"{SACM_NS}\" xmlns:{x}=\"{XMI_NS}\" \
                     {x}:id=\"a\"\n  isAbstract=\"true\"/>"
            )
        },
        (2, 3),
    ),
    (
        "content not read",
        |s, x| {
            format!(
                "<{s}:ArgumentPackage xmlns:{s}=\"{SACM_NS}\" xmlns:{x}=\"{XMI_NS}\" \
                     {x}:id=\"a\">\n  <foo/></{s}:ArgumentPackage>"
            )
        },
        (2, 3),
    ),
    (
        "an XMI element beside a root",
        |s, x| {
            format!(
                "<{x}:XMI xmlns:{x}=\"{XMI_NS}\" xmlns:{s}=\"{SACM_NS}\">\n  \
                     <{s}:ArgumentPackage {x}:id=\"a\"/>\n  \
                     <{x}:documentation/>\n</{x}:XMI>"
            )
        },
        (3, 3),
    ),
];

#[test]
fn sacm23_xmi_002_a_repeated_id_is_the_same_duplicate_id_under_any_prefix() {
    // The id compares by its value, whatever prefix the XMI namespace has
    // (SACM23-VAL-001; ADR 0015 amendment 2).
    let template = |s: &str, x: &str| {
        format!(
            "<{x}:XMI xmlns:{x}=\"{XMI_NS}\" xmlns:{s}=\"{SACM_NS}\">\n  \
             <{s}:ArgumentPackage {x}:id=\"p\"/>\n  \
             <{s}:ArgumentPackage {x}:id=\"p\"/>\n</{x}:XMI>"
        )
    };
    let reference = template("sacm", "xmi");
    let expected = refusal(reference.as_bytes());
    assert!(
        matches!(&expected, ReadError::DuplicateId { id, .. } if id.as_str() == "p"),
        "{ROW}: the reference {} is not DuplicateId for \"p\": {expected:?}",
        preview(reference.as_bytes())
    );
    assert_eq!(
        (expected.code(), expected.locations()),
        (Some(Code::ID_DUPLICATE), [loc(2, 3), loc(3, 3)].as_slice()),
        "{ROW}: the reference is not SACM-ID-001 at both roots: {expected}"
    );
    for (s, x) in [("s", "x"), ("ü", "ns0"), ("SACM", "XMI"), ("xmi", "sacm")] {
        let input = template(s, x);
        assert_eq!(
            refusal(input.as_bytes()),
            expected,
            "{ROW}: with {s:?} and {x:?}, {} is not refused as the reference is",
            preview(input.as_bytes())
        );
    }
}

#[test]
fn sacm23_xmi_002_unsupported_refusals_have_the_same_kind_code_and_location_under_any_prefix() {
    for (what, template, (line, column)) in REFUSALS {
        let reference = template("sacm", "xmi");
        let expected = refusal(reference.as_bytes());
        assert!(
            matches!(expected, ReadError::Unsupported { .. }),
            "{ROW}: {what}: the reference {} is not Unsupported: {expected:?}",
            preview(reference.as_bytes())
        );
        assert_eq!(
            expected.location(),
            Some(loc(line, column)),
            "{ROW}: {what}: the reference {} is not refused at the expected place: {expected}",
            preview(reference.as_bytes())
        );
        for (s, x) in [("s", "x"), ("ü", "ns0"), ("SACM", "XMI"), ("xmi", "sacm")] {
            let input = template(s, x);
            let error = refusal(input.as_bytes());
            assert_eq!(
                mem::discriminant(&error),
                mem::discriminant(&expected),
                "{ROW}: {what}: with {s:?} and {x:?}, {} is not refused as the reference is: \
                 {error:?} against {expected:?}",
                preview(input.as_bytes())
            );
            assert_eq!(
                error.code(),
                expected.code(),
                "{ROW}: {what}: with {s:?} and {x:?}, the refusal has another code"
            );
            assert_eq!(
                error.location(),
                expected.location(),
                "{ROW}: {what}: with {s:?} and {x:?}, the refusal is elsewhere: {error}"
            );
        }
    }
}
