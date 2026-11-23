//! The dialect EMF writes (LIB-COMPAT-002; ADR 0044 D1, D2; D-STRICT).
//!
//! XMI 2.5.1 §7.5.3 (p. 10): "Each version of XMI is unambiguously
//! identified by its unique namespace URI of the form
//! "`http://www.omg.org/spec/XMI/version-namespace`."", and §7.10.2 (p. 23):
//! "Attributes representing Class-typed properties in the metamodel, or XML
//! idref attributes can refer to other XML elements within the same XML file
//! by specifying the target element's XML ID." EMF binds `xmi` to
//! `http://www.omg.org/XMI` and names same-document targets by fragment
//! paths, so its dialect is not SACM 2.3 XMI. A tolerant `xmi::read`:
//!
//! - reads the `XMI` element and the attributes `id`, `type`, `idref`,
//!   `version`, `uuid` and `label` in `http://www.omg.org/XMI` as XMI
//!   2.5.1's, with one SACM-XMI-015 Warning per document at the first name
//!   in that namespace; any other name in it is kept (SACM-XMI-003);
//! - reads a same-document reference target written as an EMF fragment path
//!   as the id of the element it names, with one SACM-XMI-017 Warning per
//!   document, and keeps a path that names nothing as written.
//!
//! A strict read refuses any name in `http://www.omg.org/XMI` as
//! `Unsupported` SACM-XMI-015 and resolves no path. An `XMI` element in any
//! other namespace, or none, is refused in both modes. Both saves write XMI
//! 2.5.1 and ids.
//!
//! G1 is `tests/data/compat/emf-dialect.xmi` and G2, its strict output,
//! `tests/data/compat/golden/emf-dialect.golden.sacm.xmi`; J1 and J2 are
//! `tests/data/golden/serde/emf-dialect-xmi-015.json` and
//! `emf-dialect-xmi-017.json`, its two Warnings as JSON. Codes and templates
//! are compared by their text, and the codes as `Code` constants too.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::Duration;

use sacm::diagnostic::{Argument, Diagnostic, Severity, SourceLocation};
use sacm::model::{Document, ElementId, ReferenceEnd, SacmElement};
use sacm::validation;
use sacm::xmi::{self, Mode, ReadError, ReadOptions, ReadOutput, WriteError, WriteOptions};

const ROW: &str = "LIB-COMPAT-002";

const SACM_NS: &str = "http://www.omg.org/spec/SACM/20220301";
const XMI_NS: &str = "http://www.omg.org/spec/XMI/20131001";
/// The XMI namespace EMF writes (`ExtendedMetaData.XMI_URI`).
const EMF_NS: &str = "http://www.omg.org/XMI";

const MODES: [Mode; 2] = [Mode::Strict, Mode::Tolerant];

/// G1: the EMF fixture.
const G1: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/compat/emf-dialect.xmi"
);
/// G2: G1's strict output.
const G2: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/compat/golden/emf-dialect.golden.sacm.xmi"
);
/// J1: G1's SACM-XMI-015 Warning as JSON.
#[cfg(feature = "serde")]
const J1: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/golden/serde/emf-dialect-xmi-015.json"
);
/// J2: G1's SACM-XMI-017 Warning as JSON.
#[cfg(feature = "serde")]
const J2: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/golden/serde/emf-dialect-xmi-017.json"
);

/// The label of a strict refusal of what LIB-COMPAT-002 reads (ADR 0041 D6).
const TOLERATED: &str = "(strict mode never reads it; LIB-COMPAT-002 reads it in tolerant mode)";

/// How long one read may take: a linear read of the inputs of
/// [`lib_compat_002_path_resolution_and_the_namespace_scan_are_linear`]
/// takes well under that, and one that rescans per token or per attribute
/// takes minutes (the pattern of `lib_compat_004_type_checks_are_linear`).
const DEADLINE: Duration = Duration::from_secs(5);

// --- Helpers ---------------------------------------------------------------------

fn options(mode: Mode) -> ReadOptions {
    ReadOptions::default().with_mode(mode)
}

fn loc(line: u32, column: u32) -> SourceLocation {
    SourceLocation::new(line, column)
        .unwrap_or_else(|| panic!("{ROW}: setup: ({line}, {column}) is no location"))
}

/// The location of the first `needle` in `text`, whose lines end with LF:
/// the line, and the column in characters.
fn position_of(text: &str, needle: &str) -> SourceLocation {
    let Some(offset) = text.find(needle) else {
        panic!("{ROW}: setup: {needle:?} is not in {text}");
    };
    let before = &text[..offset];
    let line = before.matches('\n').count() + 1;
    let column = before.rsplit('\n').next().unwrap_or("").chars().count() + 1;
    loc(
        u32::try_from(line).unwrap_or(u32::MAX),
        u32::try_from(column).unwrap_or(u32::MAX),
    )
}

/// The text of the file `path`.
fn fixture(path: &str) -> String {
    fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("{ROW}: setup: cannot read {path}: {error}"))
}

/// The read of `input` in `mode`, which must succeed.
fn read_ok(what: &str, input: &str, mode: Mode) -> ReadOutput {
    match xmi::read(input.as_bytes(), &options(mode)) {
        Ok(output) => output,
        Err(error) => panic!("{ROW}: {what}: a {mode:?} read refuses it: {error:?} ({error})"),
    }
}

/// The refusal of `input` in `mode`.
fn refusal(what: &str, input: &str, mode: Mode) -> ReadError {
    match xmi::read(input.as_bytes(), &options(mode)) {
        Ok(output) => panic!(
            "{ROW}: {what}: a {mode:?} read returns a document, with {:?}",
            output.diagnostics()
        ),
        Err(error) => error,
    }
}

/// The write of `document` in `mode`, which must succeed.
fn written(what: &str, document: &Document, mode: Mode) -> String {
    xmi::write(document, &WriteOptions::default().with_mode(mode)).unwrap_or_else(|error| {
        panic!("{ROW}: {what}: a {mode:?} write refuses it: {error:?} ({error})")
    })
}

/// The arguments as `(name, text)`; a Count shows as its number.
fn arguments_of(found: &[Argument]) -> Vec<(String, String)> {
    found
        .iter()
        .map(|argument| {
            let value = argument.value();
            let text = value.as_text().map_or_else(
                || {
                    value
                        .as_count()
                        .map_or_else(String::new, |count| count.to_string())
                },
                str::to_owned,
            );
            (argument.name().to_owned(), text)
        })
        .collect()
}

/// The arguments as `(name, is a Count)`, in their order.
fn argument_kinds(found: &[Argument]) -> Vec<(&'static str, bool)> {
    found
        .iter()
        .map(|argument| (argument.name(), argument.value().as_count().is_some()))
        .collect()
}

fn owned(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
        .collect()
}

fn strs(ids: &[ElementId]) -> Vec<&str> {
    ids.iter().map(ElementId::as_str).collect()
}

/// The element `id` of `document`, which must exist.
fn element<'a>(what: &str, document: &'a Document, id: &str) -> SacmElement<'a> {
    document
        .get(id)
        .unwrap_or_else(|| panic!("{ROW}: {what}: the document has no element {id:?}"))
}

/// The diagnostics of `found` with the template `template`.
fn with_template<'a>(found: &'a [Diagnostic], template: &str) -> Vec<&'a Diagnostic> {
    found
        .iter()
        .filter(|diagnostic| diagnostic.template() == template)
        .collect()
}

/// The one diagnostic of `found` with the template `template`.
fn only<'a>(what: &str, found: &'a [Diagnostic], template: &str) -> &'a Diagnostic {
    let all = with_template(found, template);
    let [one] = all.as_slice() else {
        panic!(
            "{ROW}: {what}: not one {template} but {}, among {} diagnostics: {:#?}",
            all.len(),
            found.len(),
            summary(found)
        );
    };
    one
}

/// The templates of `found` and how often each is reported, for a failure
/// message that stays short when a read reports many diagnostics.
fn summary(found: &[Diagnostic]) -> BTreeMap<&'static str, usize> {
    let mut counts = BTreeMap::new();
    for diagnostic in found {
        *counts.entry(diagnostic.template()).or_insert(0) += 1;
    }
    counts
}

/// The diagnostics of `found` with the code `code`.
fn with_code<'a>(found: &'a [Diagnostic], code: &str) -> Vec<&'a Diagnostic> {
    found
        .iter()
        .filter(|diagnostic| diagnostic.code().as_str() == code)
        .collect()
}

/// The diagnostics as a count per `(code, severity, template)`.
fn tally(found: &[Diagnostic]) -> BTreeMap<(String, &'static str, &'static str), usize> {
    let mut counts = BTreeMap::new();
    for diagnostic in found {
        *counts
            .entry((
                diagnostic.code().as_str().to_owned(),
                diagnostic.severity().as_str(),
                diagnostic.template(),
            ))
            .or_insert(0) += 1;
    }
    counts
}

fn expected_tally(
    rows: &[(&str, &'static str, &'static str, usize)],
) -> BTreeMap<(String, &'static str, &'static str), usize> {
    rows.iter()
        .map(|(code, severity, template, count)| {
            (((*code).to_owned(), *severity, *template), *count)
        })
        .collect()
}

/// Asserts that `found` holds no SACM-XMI-017.
fn assert_no_paths_read(what: &str, found: &[Diagnostic]) {
    assert!(
        with_code(found, "SACM-XMI-017").is_empty(),
        "{ROW}: {what}: a SACM-XMI-017 is reported: {found:#?}"
    );
}

/// Asserts that a tolerant write of `document` reads back tolerantly `==`.
fn assert_writes_back(what: &str, document: &Document) -> String {
    let text = written(what, document, Mode::Tolerant);
    let back = read_ok(&format!("{what}, written back"), &text, Mode::Tolerant).into_document();
    assert!(
        back == *document,
        "{ROW}: {what}: the tolerant write does not read back equal:\n{text}"
    );
    text
}

/// Asserts that `error` is `Unsupported` SACM-XMI-015 with `template`, at
/// `at`, with `arguments`.
fn assert_xmi_015(
    what: &str,
    error: &ReadError,
    template: &str,
    at: SourceLocation,
    arguments: &[(&str, &str)],
) {
    assert!(
        matches!(error, ReadError::Unsupported { .. }),
        "{ROW}: {what}: not Unsupported: {error:?}"
    );
    assert_eq!(
        (error.code().as_str(), error.template(), error.location()),
        ("SACM-XMI-015", template, Some(at)),
        "{ROW}: {what}: the code, template and location of the refusal ({error})"
    );
    assert_eq!(
        arguments_of(error.arguments()),
        owned(arguments),
        "{ROW}: {what}: the arguments of the refusal"
    );
}

fn read_in_time(what: &str, input: String, mode: Mode) -> Result<ReadOutput, ReadError> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        // The receiver is gone only after the deadline has already failed the test.
        let _ = sender.send(xmi::read(input.as_bytes(), &options(mode)));
    });
    match receiver.recv_timeout(DEADLINE) {
        Ok(result) => result,
        Err(RecvTimeoutError::Timeout) => {
            panic!("{ROW}: {what}: a {mode:?} read did not return within {DEADLINE:?}")
        }
        Err(RecvTimeoutError::Disconnected) => panic!("{ROW}: {what}: a {mode:?} read panicked"),
    }
}

/// A root ArgumentPackage `p` in the pinned namespace whose `xmi` prefix is
/// bound to EMF's namespace, holding `body`.
fn emf_root(body: &str) -> String {
    format!(
        "<sacm:ArgumentPackage xmlns:sacm=\"{SACM_NS}\" xmlns:xmi=\"{EMF_NS}\" xmi:id=\"p\">\n  \
         <name content=\"p\"/>\n  {body}\n</sacm:ArgumentPackage>"
    )
}

/// A root ArgumentPackage `p` in the pinned namespaces of SACM 2.3 and XMI
/// 2.5.1, holding `body`, with `v` bound to `urn:v`.
fn pinned_root(body: &str) -> String {
    format!(
        "<sacm:ArgumentPackage xmlns:sacm=\"{SACM_NS}\" xmlns:xmi=\"{XMI_NS}\" xmlns:v=\"urn:v\" \
         xmi:id=\"p\">\n  <name content=\"p\"/>\n  {body}\n</sacm:ArgumentPackage>"
    )
}

// --- 1. G1 -----------------------------------------------------------------------

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one read of G1 checked against every clause of acceptance 1, in order"
)]
fn lib_compat_002_the_emf_fixture_reads_tolerantly_and_writes_its_golden_strict_output() {
    let g1 = fixture(G1);
    let g2 = fixture(G2);
    let output = read_ok("G1", &g1, Mode::Tolerant);
    let found = output.diagnostics();
    assert_eq!(
        tally(found),
        expected_tally(&[
            ("SACM-XMI-004", "info", "SACM-XMI-004/generated-id", 5),
            ("SACM-XMI-008", "warning", "SACM-XMI-008/other-revision", 1),
            ("SACM-XMI-010", "warning", "SACM-XMI-010/xsi-type", 3),
            ("SACM-XMI-011", "info", "SACM-XMI-011/version-on-root", 1),
            (
                "SACM-XMI-013",
                "warning",
                "SACM-XMI-013/package-role-read",
                3
            ),
            (
                "SACM-XMI-015",
                "warning",
                "SACM-XMI-015/emf-xmi-namespace-read",
                1
            ),
            ("SACM-XMI-017", "warning", "SACM-XMI-017/paths-read", 1),
        ]),
        "{ROW}: G1: the diagnostics of a tolerant read, as (code, severity, template) and count: \
         {found:#?}"
    );
    assert_eq!(
        only("G1", found, "SACM-XMI-008/other-revision").location(),
        Some(loc(3, 1)),
        "{ROW}: G1: the location of SACM-XMI-008"
    );
    assert_eq!(
        only("G1", found, "SACM-XMI-011/version-on-root").location(),
        Some(loc(3, 37)),
        "{ROW}: G1: the location of SACM-XMI-011"
    );
    let namespace = only("G1", found, "SACM-XMI-015/emf-xmi-namespace-read");
    assert_eq!(
        (
            namespace.requirement_id(),
            namespace.clause(),
            namespace.location(),
            strs(namespace.affected()),
            arguments_of(namespace.arguments()),
        ),
        (
            ROW,
            Some("7.5.3"),
            Some(loc(3, 37)),
            Vec::<&str>::new(),
            owned(&[("name", "xmi:version")]),
        ),
        "{ROW}: G1: the row, clause, location, affected ids and arguments of SACM-XMI-015"
    );
    let paths = only("G1", found, "SACM-XMI-017/paths-read");
    assert_eq!(
        (
            paths.requirement_id(),
            paths.clause(),
            paths.location(),
            strs(paths.affected()),
            arguments_of(paths.arguments()),
        ),
        (
            ROW,
            Some("7.10.2"),
            Some(loc(13, 70)),
            vec!["asserted_inference_1"],
            owned(&[
                ("count", "2"),
                ("first", "//@argumentPackage.0/@argumentationElement.1"),
            ]),
        ),
        "{ROW}: G1: the row, clause, location, affected ids and arguments of SACM-XMI-017"
    );
    assert_eq!(
        paths.code(),
        sacm::diagnostic::Code::XMI_FRAGMENT_PATH,
        "{ROW}: G1: SACM-XMI-017 is Code::XMI_FRAGMENT_PATH"
    );

    let document = output.document();
    let ids: Vec<&str> = document.elements().map(|e| e.id().as_str()).collect();
    assert_eq!(
        ids,
        [
            "assurance_case_package_1",
            "argument_package_1",
            "claim_1",
            "claim_2",
            "asserted_inference_1"
        ],
        "{ROW}: G1: the ids of the elements"
    );
    let inference = element("G1", document, "asserted_inference_1");
    assert_eq!(
        (strs(inference.sources()), strs(inference.targets())),
        (vec!["claim_2"], vec!["claim_1"]),
        "{ROW}: G1: the paths are not read as the ids of the elements they name"
    );
    let findings = validation::validate(document);
    assert!(
        findings.is_empty(),
        "{ROW}: G1: validate finds something: {findings:#?}"
    );

    for mode in MODES {
        assert_eq!(
            written("G1", document, mode),
            g2,
            "{ROW}: G1: the {mode:?} write is not G2 byte for byte"
        );
    }
    let back = read_ok("G2", &g2, Mode::Strict);
    assert!(
        back.diagnostics().is_empty(),
        "{ROW}: G2: a strict read reports {:#?}",
        back.diagnostics()
    );
    assert!(
        back.document() == document,
        "{ROW}: G2: a strict read is not == the tolerant read of G1"
    );

    // The root's namespace is checked before the namespace scan.
    let strict = refusal("G1", &g1, Mode::Strict);
    assert!(
        matches!(strict, ReadError::Unsupported { .. }),
        "{ROW}: G1: a strict read is not Unsupported: {strict:?}"
    );
    assert_eq!(
        (strict.code().as_str(), strict.location()),
        ("SACM-XMI-008", Some(loc(3, 1))),
        "{ROW}: G1: a strict read is not refused for the root's SACM 2.2 namespace first ({strict})"
    );
}

// --- 2. Strict refusals ----------------------------------------------------------

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the six cases (a) to (f) of acceptance 2, in order"
)]
fn lib_compat_002_a_strict_read_refuses_a_name_in_the_emf_xmi_namespace_with_sacm_xmi_015() {
    // (a) An EMF wrapper around a pinned root.
    let what = "(a) an EMF wrapper";
    let input = format!(
        "<xmi:XMI xmlns:xmi=\"{EMF_NS}\" xmlns:sacm=\"{SACM_NS}\" xmi:version=\"2.0\">\n  \
         <sacm:ArgumentPackage xmi:id=\"p\"><name content=\"p\"/></sacm:ArgumentPackage>\n\
         </xmi:XMI>"
    );
    let error = refusal(what, &input, Mode::Strict);
    assert_xmi_015(
        what,
        &error,
        "SACM-XMI-015/not-xmi-namespace",
        loc(1, 1),
        &[("element", "xmi:XMI"), ("row", ROW)],
    );
    assert_eq!(
        error.to_string(),
        "the reader cannot represent the input at line 1, column 1: the element \"xmi:XMI\" is \
         not in the XMI namespace \"http://www.omg.org/spec/XMI/20131001\", so it is not read as \
         the XMI element (strict mode never reads it; LIB-COMPAT-002 reads it in tolerant mode)",
        "{ROW}: {what}: the Display"
    );

    // (b) A pinned bare root whose xmi:id is in EMF's namespace.
    let what = "(b) xmi:id in EMF's namespace";
    let input = format!(
        "<sacm:ArgumentPackage xmlns:sacm=\"{SACM_NS}\" xmlns:xmi=\"{EMF_NS}\" xmi:id=\"p\">\n  \
         <name content=\"p\"/>\n</sacm:ArgumentPackage>"
    );
    let at = position_of(&input, "xmi:id=");
    let error = refusal(what, &input, Mode::Strict);
    assert_xmi_015(
        what,
        &error,
        "SACM-XMI-015/emf-xmi-namespace",
        at,
        &[("name", "xmi:id"), ("row", ROW)],
    );
    assert_eq!(
        error.to_string(),
        format!(
            "the reader cannot represent the input at line {}, column {}: the name \"xmi:id\" is \
             in the namespace \"http://www.omg.org/XMI\", not the XMI 2.5.1 namespace \
             \"http://www.omg.org/spec/XMI/20131001\" {TOLERATED}",
            at.line(),
            at.column()
        ),
        "{ROW}: {what}: the Display"
    );

    // (c) The only EMF name is an attribute of a vendor element in a Claim.
    let what = "(c) e:id on a vendor element";
    let input = format!(
        "<sacm:ArgumentPackage xmlns:sacm=\"{SACM_NS}\" xmlns:xmi=\"{XMI_NS}\" \
         xmlns:v=\"urn:v\" xmlns:e=\"{EMF_NS}\" xmi:id=\"p\">\n  <name content=\"p\"/>\n  \
         <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\">\n    <name content=\"c1\"/>\n    \
         <v:note e:id=\"n1\"/>\n  </argumentElement>\n</sacm:ArgumentPackage>"
    );
    let error = refusal(what, &input, Mode::Strict);
    assert_xmi_015(
        what,
        &error,
        "SACM-XMI-015/emf-xmi-namespace",
        position_of(&input, "e:id="),
        &[("name", "e:id"), ("row", ROW)],
    );

    // (d) A root in an older SACM revision's namespace is refused for that
    // first.
    let what = "(d) a SACM 2.2 root using EMF's namespace";
    let input = format!(
        "<a:ArgumentPackage xmlns:a=\"http://omg.sacm/2.2/argumentation\" xmlns:xmi=\"{EMF_NS}\" \
         xmi:id=\"p\">\n  <name content=\"p\"/>\n</a:ArgumentPackage>"
    );
    let error = refusal(what, &input, Mode::Strict);
    assert!(
        matches!(error, ReadError::Unsupported { .. }),
        "{ROW}: {what}: not Unsupported: {error:?}"
    );
    assert_eq!(
        (error.code().as_str(), error.template(), error.location()),
        (
            "SACM-XMI-008",
            "SACM-XMI-008/other-revision-root",
            Some(loc(1, 1))
        ),
        "{ROW}: {what}: the code, template and location ({error})"
    );

    // (e) A declaration of EMF's namespace alone is no name in it.
    let what = "(e) xmlns:xmi bound to EMF's namespace, and no name in it";
    let input = format!(
        "<sacm:ArgumentPackage xmlns:sacm=\"{SACM_NS}\" xmlns:x=\"{XMI_NS}\" \
         xmlns:xmi=\"{EMF_NS}\" x:id=\"p\">\n  <name content=\"p\"/>\n</sacm:ArgumentPackage>"
    );
    let output = read_ok(what, &input, Mode::Strict);
    assert!(
        output.diagnostics().is_empty(),
        "{ROW}: {what}: a strict read reports {:#?}",
        output.diagnostics()
    );

    // (f) An EMF wrapper whose second root is no package.
    let what = "(f) an EMF wrapper holding an ArgumentReasoning root";
    let input = format!(
        "<xmi:XMI xmlns:xmi=\"{EMF_NS}\" xmlns:sacm=\"{SACM_NS}\">\n  \
         <sacm:ArgumentPackage xmi:id=\"p\"><name content=\"p\"/></sacm:ArgumentPackage>\n  \
         <sacm:ArgumentReasoning xmi:id=\"r\"/>\n</xmi:XMI>"
    );
    let error = refusal(what, &input, Mode::Strict);
    assert_xmi_015(
        &format!("{what}, strict"),
        &error,
        "SACM-XMI-015/not-xmi-namespace",
        loc(1, 1),
        &[("element", "xmi:XMI"), ("row", ROW)],
    );
    let error = refusal(what, &input, Mode::Tolerant);
    assert!(
        matches!(error, ReadError::InvalidRoot { .. }),
        "{ROW}: {what}: a tolerant read is not InvalidRoot: {error:?}"
    );
    assert_eq!(
        (error.code().as_str(), error.location()),
        (
            "SACM-XMI-001",
            Some(position_of(&input, "<sacm:ArgumentReasoning"))
        ),
        "{ROW}: {what}: a tolerant read is not refused at the second root ({error})"
    );
}

// --- 3. An XMI element in another namespace ----------------------------------------

#[test]
fn lib_compat_002_an_xmi_element_in_another_namespace_is_refused_in_both_modes() {
    let root = format!(
        "<sacm:ArgumentPackage xmlns:sacm=\"{SACM_NS}\" xmlns:xmi=\"{XMI_NS}\" xmi:id=\"p\">\
         <name content=\"p\"/></sacm:ArgumentPackage>"
    );
    let cases = [
        (
            "x:XMI in XMI 2.4.1's namespace",
            format!("<x:XMI xmlns:x=\"http://www.omg.org/spec/XMI/20110701\">{root}</x:XMI>"),
            "x:XMI",
        ),
        ("a bare XMI", format!("<XMI>{root}</XMI>"), "XMI"),
    ];
    for (what, input, element) in &cases {
        let strict = refusal(what, input, Mode::Strict);
        let tolerant = refusal(what, input, Mode::Tolerant);
        assert_eq!(
            tolerant, strict,
            "{ROW}: {what}: the modes refuse differently"
        );
        assert_xmi_015(
            what,
            &strict,
            "SACM-XMI-015/not-xmi-element",
            loc(1, 1),
            &[("element", element)],
        );
        assert!(
            strict
                .to_string()
                .ends_with("so it is not read as the XMI element (a limit of this version)"),
            "{ROW}: {what}: the Display does not end with the limit label: {strict}"
        );
    }
}

// --- 4. The XMI names EMF writes ---------------------------------------------------

#[test]
fn lib_compat_002_the_emf_xmi_names_are_read_as_xmi_2_5_1_with_one_sacm_xmi_015() {
    let with_vendor = format!(
        "<sacm:ArgumentPackage xmlns:sacm=\"{SACM_NS}\" xmlns:xmi=\"{EMF_NS}\" \
         xmlns:v=\"urn:v\" xmlns:e=\"{EMF_NS}\" xmi:version=\"2.0\" xmi:id=\"p\" \
         xmi:uuid=\"u-p\" xmi:label=\"Arguments\">\n  <name content=\"Arguments\"/>\n  \
         <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\">\n    <name content=\"C1\"/>\n    \
         <v:note e:id=\"n1\"/>\n  </argumentElement>\n  \
         <argumentElement xmi:type=\"sacm:AssertedInference\" xmi:id=\"i1\" isCitation=\"true\" \
         citedElement=\"n1\">\n    <source xmi:idref=\"c1\"/>\n  </argumentElement>\n\
         </sacm:ArgumentPackage>"
    );
    let what = "EMF names with a vendor element";
    let output = read_ok(what, &with_vendor, Mode::Tolerant);
    let document = output.document();
    let ids: Vec<&str> = document.elements().map(|e| e.id().as_str()).collect();
    assert_eq!(ids, ["p", "c1", "i1"], "{ROW}: {what}: the ids");
    let p = element(what, document, "p");
    assert_eq!(
        (p.uuid(), p.label()),
        (Some("u-p"), Some("Arguments")),
        "{ROW}: {what}: the uuid and label of p"
    );
    assert_eq!(
        strs(element(what, document, "i1").sources()),
        ["c1"],
        "{ROW}: {what}: the sources of i1, by xmi:idref"
    );
    let found = output.diagnostics();
    let namespace = with_code(found, "SACM-XMI-015");
    let at = position_of(&with_vendor, "xmi:version=");
    assert!(
        namespace.len() == 1 && namespace[0].location() == Some(at),
        "{ROW}: {what}: not exactly one SACM-XMI-015, at xmi:version {at:?}: {found:#?}"
    );
    let version = with_code(found, "SACM-XMI-011");
    assert!(
        version.len() == 1 && version[0].severity() == Severity::Info,
        "{ROW}: {what}: not one SACM-XMI-011 Info: {found:#?}"
    );
    // `e:id="n1"` is an id of the document: citing it names preserved
    // content (SACM-REF-003), not nothing (SACM-REF-001).
    let findings = validation::validate(document);
    assert!(
        with_code(&findings, "SACM-REF-001").is_empty(),
        "{ROW}: {what}: n1 is not in the id space: {findings:#?}"
    );
    assert!(
        with_code(&findings, "SACM-REF-003")
            .iter()
            .any(|finding| strs(finding.affected()).contains(&"i1")),
        "{ROW}: {what}: the citation of n1 is not SACM-REF-003: {findings:#?}"
    );

    let what = "EMF names alone";
    let input = format!(
        "<sacm:ArgumentPackage xmlns:sacm=\"{SACM_NS}\" xmlns:xmi=\"{EMF_NS}\" \
         xmi:version=\"2.0\" xmi:id=\"p\" xmi:uuid=\"u-p\" xmi:label=\"Arguments\">\n  \
         <name content=\"Arguments\"/>\n  \
         <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\">\n    <name content=\"C1\"/>\n  \
         </argumentElement>\n  \
         <argumentElement xmi:type=\"sacm:AssertedInference\" xmi:id=\"i1\">\n    \
         <source xmi:idref=\"c1\"/>\n    <target xmi:idref=\"c1\"/>\n  </argumentElement>\n\
         </sacm:ArgumentPackage>"
    );
    let document = read_ok(what, &input, Mode::Tolerant).into_document();
    assert!(
        !document.has_preserved_content(),
        "{ROW}: {what}: the document holds preserved content"
    );
    let text = written(what, &document, Mode::Strict);
    let bindings: Vec<&str> = text
        .match_indices("xmlns:xmi=")
        .map(|(at, _)| &text[at..])
        .collect();
    assert!(
        bindings.len() == 1 && bindings[0].starts_with(&format!("xmlns:xmi=\"{XMI_NS}\"")),
        "{ROW}: {what}: the strict write does not bind xmi once, to XMI 2.5.1's namespace:\n{text}"
    );
    assert!(
        text.contains("xmi:uuid=\"u-p\" xmi:label=\"Arguments\""),
        "{ROW}: {what}: the strict write does not write the uuid and label:\n{text}"
    );
    assert!(
        !text.contains(EMF_NS),
        "{ROW}: {what}: the strict write names EMF's namespace:\n{text}"
    );
    let back = read_ok(&format!("{what}, written"), &text, Mode::Strict);
    assert!(
        back.diagnostics().is_empty(),
        "{ROW}: {what}: the strict write reads back strictly with {:#?}",
        back.diagnostics()
    );
    assert!(
        back.document() == &document,
        "{ROW}: {what}: the strict write does not read back =="
    );
}

// --- 5. Other names in EMF's namespace -------------------------------------------------

#[test]
fn lib_compat_002_other_names_in_the_emf_namespace_are_kept_as_written() {
    // xmi:foo is no XMI attribute: it is kept, under a renamed prefix,
    // since the written element binds xmi to XMI 2.5.1's namespace.
    let what = "xmi:foo on a Claim";
    let input = emf_root("<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\" xmi:foo=\"1\"/>");
    let output = read_ok(what, &input, Mode::Tolerant);
    let renamed = only(what, output.diagnostics(), "SACM-XMI-003/prefix-renamed");
    let arguments = arguments_of(renamed.arguments());
    assert!(
        arguments.contains(&("attribute".to_owned(), "xmi:foo".to_owned()))
            && arguments.contains(&("renamed_prefix".to_owned(), "ns1".to_owned()))
            && arguments.contains(&("holder_id".to_owned(), "c1".to_owned())),
        "{ROW}: {what}: the arguments of prefix-renamed: {arguments:?}"
    );
    assert!(
        element(what, output.document(), "c1").has_preserved_content(),
        "{ROW}: {what}: c1 holds no preserved content"
    );
    assert_writes_back(what, output.document());

    // xmi:Extension in EMF's namespace is no XMI metadata.
    let what = "xmi:Extension in a Claim";
    let input = emf_root(
        "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"><name content=\"c1\"/>\
         <xmi:Extension extender=\"x\"/></argumentElement>",
    );
    let output = read_ok(what, &input, Mode::Tolerant);
    let kept = only(what, output.diagnostics(), "SACM-XMI-003/element-kept");
    assert_eq!(
        (kept.severity(), arguments_of(kept.arguments())),
        (
            Severity::Warning,
            owned(&[
                ("element", "xmi:Extension"),
                ("holder_class", "Claim"),
                ("holder_id", "c1")
            ])
        ),
        "{ROW}: {what}: the severity and arguments of element-kept"
    );
    assert!(
        element(what, output.document(), "c1").has_preserved_content(),
        "{ROW}: {what}: c1 holds no preserved content"
    );
    assert_writes_back(what, output.document());

    // xmi:Documentation in EMF's namespace, under an EMF wrapper, is the
    // document's content.
    let what = "xmi:Documentation under an EMF wrapper";
    let input = format!(
        "<xmi:XMI xmlns:xmi=\"{EMF_NS}\" xmlns:sacm=\"{SACM_NS}\">\n  \
         <xmi:Documentation exporter=\"x\"/>\n  \
         <sacm:ArgumentPackage xmi:id=\"p\"><name content=\"p\"/></sacm:ArgumentPackage>\n\
         </xmi:XMI>"
    );
    let output = read_ok(what, &input, Mode::Tolerant);
    let kept = only(
        what,
        output.diagnostics(),
        "SACM-XMI-003/wrapper-element-kept",
    );
    assert_eq!(
        (kept.location(), arguments_of(kept.arguments())),
        (
            Some(position_of(&input, "<xmi:Documentation")),
            owned(&[("element", "xmi:Documentation"), ("container", "xmi:XMI")])
        ),
        "{ROW}: {what}: the location and arguments of wrapper-element-kept"
    );
    let document = output.document();
    assert!(
        document.has_preserved_content()
            && document
                .elements()
                .all(|element| !element.has_preserved_content()),
        "{ROW}: {what}: the document, and no element, does not hold preserved content"
    );
    let roots: Vec<&str> = document.roots().map(|root| root.id().as_str()).collect();
    assert_eq!(roots, ["p"], "{ROW}: {what}: the roots");
    assert_writes_back(what, document);

    // An id in both namespaces: the first in document order is read.
    let what = "x:id then xmi:id on a Claim";
    let input = format!(
        "<sacm:ArgumentPackage xmlns:sacm=\"{SACM_NS}\" xmlns:xmi=\"{EMF_NS}\" \
         xmlns:x=\"{XMI_NS}\" xmi:id=\"p\">\n  <name content=\"p\"/>\n  \
         <argumentElement xmi:type=\"sacm:Claim\" x:id=\"c2\" xmi:id=\"c2b\"/>\n\
         </sacm:ArgumentPackage>"
    );
    let output = read_ok(what, &input, Mode::Tolerant);
    let document = output.document();
    let ids: Vec<&str> = document.elements().map(|e| e.id().as_str()).collect();
    assert_eq!(ids, ["p", "c2"], "{ROW}: {what}: the ids");
    assert!(
        element(what, document, "c2").has_preserved_content(),
        "{ROW}: {what}: the other id is not kept by c2"
    );
    assert!(
        with_code(output.diagnostics(), "SACM-XMI-003")
            .iter()
            .any(|kept| strs(kept.affected()).contains(&"c2")),
        "{ROW}: {what}: no SACM-XMI-003 names c2: {:#?}",
        output.diagnostics()
    );
    let text = assert_writes_back(what, document);
    assert!(
        text.contains("\"c2b\""),
        "{ROW}: {what}: the tolerant write does not write the other id back:\n{text}"
    );
}

// --- 6. An EMF wrapper, and root paths ------------------------------------------------

#[test]
fn lib_compat_002_an_emf_xmi_wrapper_holds_the_roots_and_root_paths_name_them() {
    let what = "an EMF wrapper of two packages";
    let input = format!(
        "<xmi:XMI xmlns:xmi=\"{EMF_NS}\" xmlns:sacm=\"{SACM_NS}\" xmi:version=\"2.0\">\n  \
         <sacm:ArgumentPackage abstractForm=\"/1\"><name content=\"Concrete\"/></sacm:ArgumentPackage>\n  \
         <sacm:ArgumentPackage isAbstract=\"true\" isCitation=\"true\" citedElement=\"/\">\
         <name content=\"Pattern\"/></sacm:ArgumentPackage>\n</xmi:XMI>"
    );
    let output = read_ok(what, &input, Mode::Tolerant);
    let document = output.document();
    let roots: Vec<SacmElement<'_>> = document.roots().collect();
    let [first, second] = roots.as_slice() else {
        panic!("{ROW}: {what}: not two roots: {roots:?}");
    };
    assert_eq!(
        (first.id().as_str(), second.id().as_str()),
        ("argument_package_1", "argument_package_2"),
        "{ROW}: {what}: the ids of the roots"
    );
    assert_eq!(
        (
            first.abstract_form().map(ElementId::as_str),
            second.cited_element().map(ElementId::as_str)
        ),
        (Some("argument_package_2"), Some("argument_package_1")),
        "{ROW}: {what}: /1 and / do not name the roots"
    );
    let found = output.diagnostics();
    let version = only(what, found, "SACM-XMI-011/version-on-xmi-element");
    assert_eq!(
        version.severity(),
        Severity::Info,
        "{ROW}: {what}: the severity of SACM-XMI-011"
    );
    let namespace = only(what, found, "SACM-XMI-015/emf-xmi-namespace-read");
    assert_eq!(
        (namespace.location(), arguments_of(namespace.arguments())),
        (Some(loc(1, 1)), owned(&[("name", "xmi:XMI")])),
        "{ROW}: {what}: the location and arguments of SACM-XMI-015"
    );
    let paths = only(what, found, "SACM-XMI-017/paths-read");
    assert_eq!(
        arguments_of(paths.arguments()),
        owned(&[("count", "2"), ("first", "/1")]),
        "{ROW}: {what}: the arguments of SACM-XMI-017"
    );
    assert_eq!(
        tally(found),
        expected_tally(&[
            ("SACM-XMI-004", "info", "SACM-XMI-004/generated-id", 2),
            (
                "SACM-XMI-011",
                "info",
                "SACM-XMI-011/version-on-xmi-element",
                1
            ),
            (
                "SACM-XMI-015",
                "warning",
                "SACM-XMI-015/emf-xmi-namespace-read",
                1
            ),
            ("SACM-XMI-017", "warning", "SACM-XMI-017/paths-read", 1),
        ]),
        "{ROW}: {what}: the diagnostics: {found:#?}"
    );
}

// --- 7. Steps by role and position ---------------------------------------------------

#[test]
fn lib_compat_002_a_path_steps_by_role_name_and_position_through_the_elements_as_written() {
    let what = "a package of seven children";
    let input = pinned_root(
        "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c0\"/>\n  \
         <v:argumentElement/>\n  \
         <argumentElement xmi:type=\"v:Unknown\" xmi:id=\"k1\"/>\n  \
         <argumentationElement xmi:type=\"sacm:Claim\" xmi:id=\"c2\"/>\n  \
         <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c3\"/>\n  \
         <argumentElement xmi:type=\"sacm:AssertedInference\" xmi:id=\"i1\" \
         source=\"//@argumentElement.2 //@argumentationElement.0\" target=\"//@argumentElement.0\"/>\n  \
         <argumentElement xmi:type=\"sacm:AssertedInference\" xmi:id=\"i2\" \
         source=\"//@argumentElement.1\" target=\"/0/@argumentElement.0\"/>",
    );
    let output = read_ok(what, &input, Mode::Tolerant);
    let document = output.document();
    let i1 = element(what, document, "i1");
    let i2 = element(what, document, "i2");
    assert_eq!(
        (
            strs(i1.sources()),
            i1.target().map(ElementId::as_str),
            strs(i2.sources()),
            i2.target().map(ElementId::as_str),
        ),
        (vec!["c3", "c2"], Some("c0"), vec!["k1"], Some("c0")),
        "{ROW}: {what}: the sources and targets of i1 and i2"
    );
    let paths = only(what, output.diagnostics(), "SACM-XMI-017/paths-read");
    assert_eq!(
        arguments_of(paths.arguments()),
        owned(&[("count", "5"), ("first", "//@argumentElement.2")]),
        "{ROW}: {what}: the arguments of SACM-XMI-017"
    );
    assert_eq!(
        with_code(output.diagnostics(), "SACM-XMI-017").len(),
        1,
        "{ROW}: {what}: not one SACM-XMI-017"
    );
    let findings = validation::validate(document);
    assert!(
        with_code(&findings, "SACM-REF-003")
            .iter()
            .any(|finding| strs(finding.affected()).contains(&"i2")),
        "{ROW}: {what}: the source k1 of i2 is not SACM-REF-003: {findings:#?}"
    );
    assert!(
        with_code(&findings, "SACM-REF-001").is_empty(),
        "{ROW}: {what}: a reference dangles: {findings:#?}"
    );
}

// --- 8. Paths that name nothing --------------------------------------------------------

#[test]
fn lib_compat_002_a_path_that_names_no_element_is_kept_as_written() {
    let tokens = [
        "//@argumentElement.9",
        "/1",
        "//@argumentElement",
        "//@argumentElement.x",
        "//@argumentElement.-1",
        "/a",
        "//argumentElement.0",
        "/0/",
        "//@argumentElement[name='x']",
        "/?c0",
        "//@name.0",
    ];
    let what = "eleven paths that name nothing";
    let input = pinned_root(&format!(
        "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c0\"/>\n  \
         <argumentElement xmi:type=\"sacm:AssertedInference\" xmi:id=\"i1\" source=\"{}\" \
         target=\"c0\"/>",
        tokens.join(" ")
    ));
    let output = read_ok(what, &input, Mode::Tolerant);
    let document = output.document();
    assert_eq!(
        strs(element(what, document, "i1").sources()),
        tokens,
        "{ROW}: {what}: the sources are not held as written, in order"
    );
    let paths = only(what, output.diagnostics(), "SACM-XMI-017/paths-partly-read");
    assert_eq!(
        (
            paths.location(),
            strs(paths.affected()),
            arguments_of(paths.arguments())
        ),
        (
            Some(position_of(&input, "source=")),
            vec!["i1"],
            owned(&[
                ("count", "11"),
                ("first", "//@argumentElement.9"),
                ("kept", "11")
            ])
        ),
        "{ROW}: {what}: the location, affected ids and arguments of SACM-XMI-017"
    );
    let findings = validation::validate(document);
    let mut dangling: Vec<String> = with_code(&findings, "SACM-REF-001")
        .iter()
        .map(|finding| {
            arguments_of(finding.arguments())
                .into_iter()
                .find(|(name, _)| name == "target")
                .map_or_else(String::new, |(_, target)| target)
        })
        .collect();
    dangling.sort_unstable();
    let mut expected: Vec<String> = tokens.iter().map(|token| (*token).to_owned()).collect();
    expected.sort_unstable();
    assert_eq!(
        dangling, expected,
        "{ROW}: {what}: validate does not give one SACM-REF-001 per token: {findings:#?}"
    );
    let text = assert_writes_back(what, document);
    assert!(
        text.contains(&format!("source=\"{}\"", tokens.join(" "))),
        "{ROW}: {what}: the tolerant write does not write the paths back:\n{text}"
    );
}

// --- 9. hrefs and expressions -------------------------------------------------------

#[test]
fn lib_compat_002_paths_in_same_document_hrefs_and_expressions_are_resolved() {
    let what = "paths in expressions and hrefs";
    let term = "//@terminologyPackage.0/@terminologyElement.0";
    let claim = "//@argumentPackage.0/@argumentElement.0";
    let input = format!(
        "<sacm:AssuranceCasePackage xmlns:sacm=\"{SACM_NS}\" xmlns:xmi=\"{XMI_NS}\" xmi:id=\"case\">\n  \
         <name content=\"case\"/>\n  \
         <terminologyPackage xmi:id=\"terms\">\n    <name content=\"terms\"/>\n    \
         <terminologyElement xmi:type=\"sacm:Term\" xmi:id=\"speed\" value=\"speed\"/>\n  \
         </terminologyPackage>\n  \
         <argumentPackage xmi:id=\"arguments\">\n    <name content=\"arguments\"/>\n    \
         <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c0\">\n      \
         <name xmi:type=\"sacm:ExpressionLangString\" lang=\"en\" expression=\"{term}\"/>\n      \
         <description xmi:id=\"d1\">\n        <content>\n          \
         <value xmi:type=\"sacm:ExpressionLangString\" lang=\"en\" content=\"\" expression=\"{term}\"/>\n        \
         </content>\n      </description>\n    </argumentElement>\n    \
         <argumentElement xmi:type=\"sacm:AssertedInference\" xmi:id=\"i1\">\n      \
         <source href=\"#{claim}\"/>\n      <target href=\"#{claim}\"/>\n    </argumentElement>\n    \
         <argumentElement xmi:type=\"sacm:AssertedInference\" xmi:id=\"i2\" target=\"c0\">\n      \
         <source href=\"other.sacm#{claim}\"/>\n    </argumentElement>\n  </argumentPackage>\n\
         </sacm:AssuranceCasePackage>"
    );
    let output = read_ok(what, &input, Mode::Tolerant);
    let document = output.document();
    assert_eq!(
        (
            strs(element(what, document, "c0").expressions()),
            strs(element(what, document, "d1").expressions()),
        ),
        (vec!["speed"], vec!["speed"]),
        "{ROW}: {what}: the expressions of the name of c0 and of the content of d1"
    );
    let i1 = element(what, document, "i1");
    assert_eq!(
        (strs(i1.sources()), i1.target().map(ElementId::as_str)),
        (vec!["c0"], Some("c0")),
        "{ROW}: {what}: the same-document hrefs of i1"
    );
    let i2 = element(what, document, "i2");
    let hrefs: Vec<(&str, &str)> = i2
        .hrefs(ReferenceEnd::Source)
        .iter()
        .map(|href| (href.document(), href.fragment()))
        .collect();
    assert_eq!(
        (hrefs, strs(i2.sources())),
        (vec![("other.sacm", claim)], Vec::<&str>::new()),
        "{ROW}: {what}: the cross-document href of i2 is not held unchanged"
    );
    let paths = only(what, output.diagnostics(), "SACM-XMI-017/paths-read");
    assert_eq!(
        (
            paths.location(),
            strs(paths.affected()),
            arguments_of(paths.arguments())
        ),
        (
            Some(position_of(&input, "expression=")),
            vec!["c0"],
            owned(&[("count", "4"), ("first", term)])
        ),
        "{ROW}: {what}: the location, affected ids and arguments of SACM-XMI-017"
    );
}

// --- 10. What is no path -------------------------------------------------------------

#[test]
fn lib_compat_002_a_token_that_is_an_xmi_id_a_string_attribute_or_an_idref_is_no_path() {
    let what = "an id, two string attributes and an xmi:idref that begin with /";
    let input = format!(
        "<xmi:XMI xmlns:xmi=\"{XMI_NS}\" xmlns:sacm=\"{SACM_NS}\">\n  \
         <sacm:ArgumentPackage xmi:id=\"p\">\n    <name content=\"p\"/>\n    \
         <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"/0\"/>\n    \
         <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\" isCitation=\"true\" citedElement=\"/0\"/>\n    \
         <argumentElement xmi:type=\"sacm:AssertedInference\" xmi:id=\"i1\" target=\"c1\">\n      \
         <source xmi:idref=\"//@argumentElement.0\"/>\n    </argumentElement>\n  \
         </sacm:ArgumentPackage>\n  \
         <sacm:TerminologyPackage xmi:id=\"terms\">\n    <name content=\"terms\"/>\n    \
         <terminologyElement xmi:type=\"sacm:Term\" xmi:id=\"t1\" value=\"system\" \
         externalReference=\"/faulttree/system.xml\"/>\n  </sacm:TerminologyPackage>\n  \
         <sacm:ArtifactPackage xmi:id=\"evidence\">\n    <name content=\"evidence\"/>\n    \
         <artifactElement xmi:type=\"sacm:Artifact\" xmi:id=\"a1\" version=\"/1\"/>\n  \
         </sacm:ArtifactPackage>\n</xmi:XMI>"
    );
    let output = read_ok(what, &input, Mode::Tolerant);
    let document = output.document();
    assert!(
        document.contains("/0"),
        "{ROW}: {what}: setup: the element /0 is not read"
    );
    assert_eq!(
        element(what, document, "c1")
            .cited_element()
            .map(ElementId::as_str),
        Some("/0"),
        "{ROW}: {what}: citedElement=\"/0\" is not the id /0"
    );
    assert_eq!(
        element(what, document, "t1").external_reference(),
        Some("/faulttree/system.xml"),
        "{ROW}: {what}: the externalReference changed"
    );
    assert_eq!(
        element(what, document, "a1").version(),
        Some("/1"),
        "{ROW}: {what}: the version changed"
    );
    assert_eq!(
        strs(element(what, document, "i1").sources()),
        ["//@argumentElement.0"],
        "{ROW}: {what}: the xmi:idref is not held as written"
    );
    assert_no_paths_read(what, output.diagnostics());
}

// --- 11. Strict -------------------------------------------------------------------

#[test]
fn lib_compat_002_a_strict_read_resolves_no_path() {
    let what = "a path, read strictly";
    let input = pinned_root(
        "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c0\"/>\n  \
         <argumentElement xmi:type=\"sacm:AssertedInference\" xmi:id=\"i1\" \
         source=\"//@argumentElement.0\" target=\"c0\"/>",
    );
    let output = read_ok(what, &input, Mode::Strict);
    assert_no_paths_read(what, output.diagnostics());
    let document = output.document();
    assert_eq!(
        strs(element(what, document, "i1").sources()),
        ["//@argumentElement.0"],
        "{ROW}: {what}: the source is not held as written"
    );
    let findings = validation::validate(document);
    assert!(
        with_code(&findings, "SACM-REF-001")
            .iter()
            .any(|finding| strs(finding.affected()).contains(&"i1")),
        "{ROW}: {what}: validate does not report the path as SACM-REF-001: {findings:#?}"
    );
    let error = match xmi::write(document, &WriteOptions::default()) {
        Ok(text) => panic!("{ROW}: {what}: the strict write writes it:\n{text}"),
        Err(error) => error,
    };
    assert!(
        matches!(&error, WriteError::Invalid { diagnostics, .. }
            if diagnostics.iter().any(|d| d.code().as_str() == "SACM-REF-001")),
        "{ROW}: {what}: the strict write is not Invalid with SACM-REF-001: {error:?}"
    );
}

// --- 12. Preserved content ---------------------------------------------------------

#[test]
fn lib_compat_002_paths_inside_preserved_content_are_kept_as_written() {
    let kept = "<argumentElement xmi:type=\"v:Unknown\" source=\"//@argumentElement.0\"/>";
    let what = "a path inside a kept element";
    let input = pinned_root(&format!(
        "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c0\"/>\n  {kept}"
    ));
    let output = read_ok(what, &input, Mode::Tolerant);
    assert_no_paths_read(what, output.diagnostics());
    let text = written(what, output.document(), Mode::Tolerant);
    assert_eq!(
        text,
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <sacm:ArgumentPackage xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\" \
         xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" xmi:id=\"p\">\n  \
         <name content=\"p\" />\n  \
         <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c0\" />\n  \
         <argumentElement xmlns:v=\"urn:v\" xmi:type=\"v:Unknown\" source=\"//@argumentElement.0\" />\n\
         </sacm:ArgumentPackage>\n",
        "{ROW}: {what}: the kept element is not written back as written"
    );

    let what = "a typed path naming a kept element without an id";
    let input = pinned_root(&format!(
        "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c0\"/>\n  {kept}\n  \
         <argumentElement xmi:type=\"sacm:AssertedInference\" xmi:id=\"i1\" \
         source=\"//@argumentElement.1\" target=\"c0\"/>"
    ));
    let output = read_ok(what, &input, Mode::Tolerant);
    assert_eq!(
        strs(element(what, output.document(), "i1").sources()),
        ["//@argumentElement.1"],
        "{ROW}: {what}: the source is not held as written"
    );
    let paths = only(what, output.diagnostics(), "SACM-XMI-017/paths-partly-read");
    assert_eq!(
        (
            paths.location(),
            strs(paths.affected()),
            arguments_of(paths.arguments())
        ),
        (
            Some(position_of(&input, "source=\"//@argumentElement.1\"")),
            vec!["i1"],
            owned(&[
                ("count", "1"),
                ("first", "//@argumentElement.1"),
                ("kept", "1")
            ])
        ),
        "{ROW}: {what}: the location, affected ids and arguments of SACM-XMI-017"
    );
}

// --- 13. Write-back ---------------------------------------------------------------

#[test]
fn lib_compat_002_emf_files_write_back_tolerantly_and_read_back_equal() {
    let g1 = fixture(G1);
    let document = read_ok("G1", &g1, Mode::Tolerant).into_document();
    assert_eq!(
        written("G1", &document, Mode::Tolerant),
        fixture(G2),
        "{ROW}: G1: the tolerant write is not G2"
    );

    // A SACM 2.0 file as EMF writes it, with the 2.0 role argumentAsset,
    // which SACM 2.3 does not have.
    let what = "an EMF SACM 2.0 root with an argumentAsset";
    let input = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <argumentation:ArgumentPackage xmi:version=\"2.0\" xmlns:xmi=\"{EMF_NS}\" \
         xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" \
         xmlns:argumentation=\"http://omg.sacm/2.0/argumentation\" xmi:id=\"p\">\n  \
         <name lang=\"en\" content=\"Arguments\"/>\n  \
         <argumentAsset xsi:type=\"argumentation:Claim\" xmi:id=\"c1\">\n    \
         <name lang=\"en\" content=\"Kept\"/>\n  </argumentAsset>\n\
         </argumentation:ArgumentPackage>\n"
    );
    let output = read_ok(what, &input, Mode::Tolerant);
    assert_eq!(
        with_code(output.diagnostics(), "SACM-XMI-015").len(),
        1,
        "{ROW}: {what}: not one SACM-XMI-015: {:#?}",
        output.diagnostics()
    );
    let document = output.document();
    assert!(
        element(what, document, "p").has_preserved_content(),
        "{ROW}: {what}: setup: the argumentAsset is not kept"
    );
    let text = written(what, document, Mode::Tolerant);
    let back = read_ok(&format!("{what}, written back"), &text, Mode::Tolerant);
    let differences = sacm::compare::semantic_compare(document, back.document());
    assert!(
        differences.is_empty() && back.document() == document,
        "{ROW}: {what}: the tolerant write does not read back equal: {differences:?}\n{text}"
    );
    let Some(root_tag) = text
        .lines()
        .find(|line| line.starts_with("<sacm:ArgumentPackage"))
    else {
        panic!("{ROW}: {what}: the tolerant write has no root start tag:\n{text}");
    };
    assert!(
        !root_tag.contains("ns1"),
        "{ROW}: {what}: the written root carries an ns1 binding: {root_tag}"
    );
}

// --- 14. Linearity -----------------------------------------------------------------

#[test]
fn lib_compat_002_path_resolution_and_the_namespace_scan_are_linear() {
    // 100,000 paths into a package of 100,000 elements: indexing the
    // children by role once per (element, role) keeps the resolution
    // linear; rescanning per token does not.
    let what = "100,000 paths";
    let mut input = format!(
        "<sacm:ArgumentPackage xmlns:sacm=\"{SACM_NS}\" xmlns:xmi=\"{EMF_NS}\" \
         xmi:version=\"2.0\" xmi:id=\"p\">"
    );
    for _ in 0..50_000 {
        input.push_str("<argumentElement xmi:type=\"sacm:Claim\"/>");
    }
    for _ in 0..50_000 {
        input.push_str(
            "<argumentElement xmi:type=\"sacm:AssertedInference\" \
             source=\"//@argumentElement.49999\" target=\"//@argumentElement.0\"/>",
        );
    }
    input.push_str("</sacm:ArgumentPackage>");
    let output = match read_in_time(what, input, Mode::Tolerant) {
        Ok(output) => output,
        Err(error) => panic!("{ROW}: {what}: refused: {error}"),
    };
    let paths = only(what, output.diagnostics(), "SACM-XMI-017/paths-read");
    assert_eq!(
        arguments_of(paths.arguments()),
        owned(&[("count", "100000"), ("first", "//@argumentElement.49999")]),
        "{ROW}: {what}: the arguments of SACM-XMI-017"
    );

    // 100,000 attributes outside EMF's namespace before the one in it: the
    // scan compares each name once. They sit 10 per element because
    // roxmltree's duplicate-attribute check is quadratic per element, which
    // is LIB-SEC-007's concern, not this row's.
    let what = "100,000 attributes, then one in EMF's namespace";
    let mut input = format!(
        "<sacm:ArgumentPackage xmlns:sacm=\"{SACM_NS}\" xmlns:xmi=\"{XMI_NS}\" \
         xmlns:v=\"urn:v\" xmlns:e=\"{EMF_NS}\" xmi:id=\"p\"><name content=\"p\"/>"
    );
    for n in 0..10_000 {
        input.push_str("<v:x");
        for k in 0..10 {
            let _ = write!(input, " v:a{k}=\"{n}\"");
        }
        input.push_str("/>");
    }
    input.push_str("<v:y e:z=\"1\"/></sacm:ArgumentPackage>");
    let at = position_of(&input, "e:z=");
    let error = match read_in_time(what, input, Mode::Strict) {
        Ok(output) => panic!(
            "{ROW}: {what}: a strict read returns a document, with {} diagnostics",
            output.diagnostics().len()
        ),
        Err(error) => error,
    };
    assert_xmi_015(
        what,
        &error,
        "SACM-XMI-015/emf-xmi-namespace",
        at,
        &[("name", "e:z"), ("row", ROW)],
    );
}

// --- 15. The new templates ---------------------------------------------------------

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the five new templates and J1 and J2, one after the other"
)]
fn lib_compat_002_the_new_templates_render_with_their_arguments_in_sentence_order() {
    // SACM-XMI-015/not-xmi-element (Unsupported).
    let input = "<x:XMI xmlns:x=\"http://www.omg.org/spec/XMI/20110701\"/>";
    let error = refusal("not-xmi-element", input, Mode::Strict);
    assert_eq!(
        (error.template(), argument_kinds(error.arguments())),
        ("SACM-XMI-015/not-xmi-element", vec![("element", false)]),
        "{ROW}: not-xmi-element: the template and its arguments"
    );
    assert_eq!(
        error.to_string(),
        "the reader cannot represent the input at line 1, column 1: the element \"x:XMI\" is in \
         neither the XMI 2.5.1 namespace \"http://www.omg.org/spec/XMI/20131001\" nor \
         \"http://www.omg.org/XMI\", so it is not read as the XMI element (a limit of this \
         version)",
        "{ROW}: not-xmi-element: the Display"
    );

    // SACM-XMI-015/emf-xmi-namespace (Unsupported).
    let input = format!(
        "<sacm:ArgumentPackage xmlns:sacm=\"{SACM_NS}\" xmlns:xmi=\"{EMF_NS}\" xmi:id=\"p\"/>"
    );
    let at = position_of(&input, "xmi:id=");
    let error = refusal("emf-xmi-namespace", &input, Mode::Strict);
    assert_eq!(
        (error.template(), argument_kinds(error.arguments())),
        (
            "SACM-XMI-015/emf-xmi-namespace",
            vec![("name", false), ("row", false)]
        ),
        "{ROW}: emf-xmi-namespace: the template and its arguments"
    );
    assert_eq!(
        error.to_string(),
        format!(
            "the reader cannot represent the input at line 1, column {}: the name \"xmi:id\" is \
             in the namespace \"http://www.omg.org/XMI\", not the XMI 2.5.1 namespace \
             \"http://www.omg.org/spec/XMI/20131001\" {TOLERATED}",
            at.column()
        ),
        "{ROW}: emf-xmi-namespace: the Display"
    );

    // SACM-XMI-015/emf-xmi-namespace-read and SACM-XMI-017/paths-read (G1).
    let output = read_ok("G1", &fixture(G1), Mode::Tolerant);
    let namespace = only(
        "G1",
        output.diagnostics(),
        "SACM-XMI-015/emf-xmi-namespace-read",
    );
    assert_eq!(
        (namespace.severity(), argument_kinds(namespace.arguments())),
        (Severity::Warning, vec![("name", false)]),
        "{ROW}: emf-xmi-namespace-read: the severity and arguments"
    );
    assert_eq!(
        namespace.message(),
        "the name \"xmi:version\" is in the namespace \"http://www.omg.org/XMI\", not the XMI \
         2.5.1 namespace \"http://www.omg.org/spec/XMI/20131001\", so the XMI names of the \
         document in it are read as XMI 2.5.1's, which write writes",
        "{ROW}: emf-xmi-namespace-read: the message"
    );
    let paths = only("G1", output.diagnostics(), "SACM-XMI-017/paths-read");
    assert_eq!(
        (paths.severity(), argument_kinds(paths.arguments())),
        (Severity::Warning, vec![("count", true), ("first", false)]),
        "{ROW}: paths-read: the severity and arguments"
    );
    assert_eq!(
        paths.message(),
        "the document names reference targets by EMF fragment paths, which XMI 2.5.1 does not \
         define (2 in all, the first \"//@argumentPackage.0/@argumentationElement.1\"); each \
         names an element of the document, so it is read as that element's id, which write \
         writes",
        "{ROW}: paths-read: the message"
    );
    assert_eq!(
        (namespace.code(), paths.code()),
        (
            sacm::diagnostic::Code::XMI_WRAPPER_NAMESPACE,
            sacm::diagnostic::Code::XMI_FRAGMENT_PATH
        ),
        "{ROW}: G1: the codes are Code::XMI_WRAPPER_NAMESPACE and Code::XMI_FRAGMENT_PATH"
    );

    // SACM-XMI-017/paths-partly-read.
    let input = pinned_root(
        "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c0\"/>\n  \
         <argumentElement xmi:type=\"sacm:AssertedInference\" xmi:id=\"i1\" \
         source=\"//@argumentElement.0 //@argumentElement.9\" target=\"c0\"/>",
    );
    let output = read_ok("paths-partly-read", &input, Mode::Tolerant);
    let partly = only(
        "paths-partly-read",
        output.diagnostics(),
        "SACM-XMI-017/paths-partly-read",
    );
    assert_eq!(
        (
            partly.severity(),
            partly.requirement_id(),
            partly.clause(),
            argument_kinds(partly.arguments())
        ),
        (
            Severity::Warning,
            ROW,
            Some("7.10.2"),
            vec![("count", true), ("first", false), ("kept", true)]
        ),
        "{ROW}: paths-partly-read: the severity, row, clause and arguments"
    );
    assert_eq!(
        partly.message(),
        "the document names reference targets by EMF fragment paths, which XMI 2.5.1 does not \
         define (2 in all, the first \"//@argumentElement.0\"); each that names an element of the \
         document is read as that element's id, which write writes, and those that name none (1) \
         are kept as written",
        "{ROW}: paths-partly-read: the message"
    );

    // J1 and J2: G1's two Warnings as JSON, which read back equal.
    #[cfg(feature = "serde")]
    for (golden, diagnostic) in [(J1, namespace), (J2, paths)] {
        let expected = fixture(golden);
        let json = serde_json::to_string(diagnostic)
            .unwrap_or_else(|error| panic!("{ROW}: {golden}: does not serialize: {error}"));
        assert_eq!(
            format!("{json}\n"),
            expected,
            "{ROW}: {golden}: the JSON of the diagnostic"
        );
        let back: Diagnostic = serde_json::from_str(&expected)
            .unwrap_or_else(|error| panic!("{ROW}: {golden}: does not read back: {error}"));
        assert_eq!(
            &back, diagnostic,
            "{ROW}: {golden}: does not read back equal"
        );
    }
}
