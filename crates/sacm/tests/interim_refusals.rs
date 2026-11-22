//! Tests that every `ReadError::Unsupported` names why it is refused
//! (LIB-API-011; ADR 0033; D-PRESERVE; D-STRICT).
//!
//! Conforming content this version does not read yet (the cross-document
//! links SACM23-XMI-029 reads, since slice 0068 reads an `href` with a
//! document part; XMI metadata until slice 0073a read it, SACM23-XMI-019),
//! and input with nowhere to be kept, stays `Unsupported`. Each refusal comes from one
//! crate-private list whose entries are `Interim(row)`, `Tolerance(row)`,
//! `Tolerated(row)` (slice 0072, ADR 0041 D6), `Limit` or `Defect`, and its
//! reason ends with exactly one label:
//!
//! - `(a limit of this version; ROW reads it)` for `Interim(ROW)`;
//! - `(a limit of this version)` for `Limit`;
//! - `(strict mode never reads it; ROW is to read it in tolerant mode)`
//!   for `Tolerance(ROW)`: input strict mode does not read, which need not
//!   be defective, such as an `XMI` element in another XMI namespace;
//! - `(strict mode never reads it; ROW reads it in tolerant mode)` for
//!   `Tolerated(ROW)`: input a tolerant read reads, such as a root in an
//!   older SACM revision's namespace or a SACM-like one (LIB-COMPAT-004,
//!   D-NS-PIN);
//! - `(a defect in the input)` for `Defect`.
//!
//! One input (or a few, where the entry covers several constructs) per entry
//! of the list, read in strict mode, is refused with that entry's label. A
//! tolerant read refuses only the limits, a SACM 2.4 root, an `XMI` element
//! in another namespace, reads a `Tolerated`
//! root, and keeps the rest as preserved content with a Warning
//! (ADR 0035; tested in `tolerant_read.rs`). Two
//! entries have no input: a parser capacity limit (`Capacity`) and a failed
//! insert into the document (`Unheld`), which no well-formed input of a
//! test's size reaches.
//!
//! The xtask test `xtask/tests/interim_refusals.rs` checks the list itself:
//! that `Unsupported` is built nowhere else, and that each row it names is
//! open and, for `Interim`, gates SACM23-CP-001 and the compliance point of
//! the row's own package.

use sacm::diagnostic::{Code, Severity, SourceLocation};
use sacm::xmi::{self, Mode, ReadError, ReadOptions};

mod common;

const ROW: &str = "LIB-API-011";

/// The row the spelling tests trace to: a strict read keeps the text
/// spelling `argumentationElement` as preserved content with one
/// SACM-XMI-013 Error (slice 0057).
const XMI_009: &str = "SACM23-XMI-009";

const MODES: [Mode; 2] = [Mode::Strict, Mode::Tolerant];

const SACM_NS: &str = "http://www.omg.org/spec/SACM/20220301";
const XMI_NS: &str = "http://www.omg.org/spec/XMI/20131001";

/// The namespace declarations of a root: `sacm`, `xmi` and `xsi`.
const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
    " xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"",
);

/// The labels' openings; a reason holds exactly one label.
const LABEL_OPENINGS: [&str; 3] = [
    "(a limit of this version",
    "(strict mode never reads it",
    "(a defect in the input",
];

/// The label of `Tolerance(row)`.
fn tolerance(row: &str) -> String {
    format!("(strict mode never reads it; {row} is to read it in tolerant mode)")
}

/// The label of `Tolerated(row)` (ADR 0041 D6).
fn tolerated(row: &str) -> String {
    format!("(strict mode never reads it; {row} reads it in tolerant mode)")
}

/// The label of `Limit`.
const LIMIT: &str = "(a limit of this version)";

/// The label of `Defect`.
const DEFECT: &str = "(a defect in the input)";

/// A root ArgumentPackage `p` holding `body`.
fn package(body: &str) -> String {
    format!(
        "<sacm:ArgumentPackage{NS} xmi:id=\"p\"><name content=\"p\"/>{body}</sacm:ArgumentPackage>"
    )
}

/// A Claim `id` with the extra attributes `attributes`, holding `body` after
/// its name.
fn claim(id: &str, attributes: &str, body: &str) -> String {
    format!(
        "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"{id}\"{attributes}><name content=\"{id}\"/>{body}</argumentElement>"
    )
}

/// `text` encoded as UTF-16 LE without a byte order mark.
fn utf16le_without_bom(text: &str) -> Vec<u8> {
    text.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

/// `text` encoded as UTF-32 BE with a byte order mark.
fn utf32be_with_bom(text: &str) -> Vec<u8> {
    let mut bytes = vec![0x00, 0x00, 0xFE, 0xFF];
    bytes.extend(text.chars().flat_map(|c| u32::from(c).to_be_bytes()));
    bytes
}

/// Asserts that `input` (described by `what`) is `Unsupported` in strict
/// mode, with a location, and that its reason ends with `label` and holds
/// no other label. What a tolerant read does with it is
/// `lib_api_011_each_kept_entry_reads_tolerantly_with_its_code_and_row`'s
/// and `lib_api_011_the_refused_entries_are_unsupported_in_both_modes`'s
/// (ADR 0035).
fn assert_refused_with(entry: &str, what: &str, input: &[u8], label: &str) {
    {
        let mode = Mode::Strict;
        let result = xmi::read(input, &ReadOptions::default().with_mode(mode));
        let error = match result {
            Ok(output) => panic!(
                "{ROW}: {entry}: {what}: a {mode:?} read accepted it, with {:?}",
                output.diagnostics()
            ),
            Err(error) => error,
        };
        let ReadError::Unsupported {
            reason, location, ..
        } = &error
        else {
            panic!(
                "{ROW}: {entry}: {what}: a {mode:?} read refused it as {error:?}, not as Unsupported"
            );
        };
        assert!(
            reason.ends_with(label),
            "{ROW}: {entry}: {what}: in {mode:?} mode the reason {reason:?} does not end with {label:?}"
        );
        assert!(
            !reason.contains("  "),
            "{ROW}: {entry}: {what}: in {mode:?} mode the reason {reason:?} holds a run of spaces"
        );
        let labels: usize = LABEL_OPENINGS
            .iter()
            .map(|opening| reason.matches(opening).count())
            .sum();
        assert_eq!(
            labels, 1,
            "{ROW}: {entry}: {what}: in {mode:?} mode the reason {reason:?} holds {labels} labels"
        );
        assert!(
            location.is_some(),
            "{ROW}: {entry}: {what}: in {mode:?} mode the refusal has no location"
        );
        assert_eq!(
            error.code().as_str(),
            crate::common::unsupported_code(&error.to_string()),
            "{ROW}: {entry}: {what}: Unsupported does not have the code of its label"
        );
        assert!(
            error.to_string().ends_with(label),
            "{ROW}: {entry}: {what}: the message {:?} does not end with the label",
            error.to_string()
        );
    }
}

/// Strict read options.
fn strict() -> ReadOptions {
    ReadOptions::default().with_mode(Mode::Strict)
}

fn assert_text_refused_with(entry: &str, what: &str, input: &str, label: &str) {
    assert_refused_with(entry, what, input.as_bytes(), label);
}

// --- Interim: none since slice 0073b reads the cross-document links (SACM23-XMI-029) -

#[test]
fn lib_api_011_no_cross_document_link_is_refused_as_not_read_yet() {
    // SACM23-XMI-027 reads an `href` with a document part on a reference
    // element (slice 0068), and SACM23-XMI-029 an XPointer fragment (slice
    // 0073b; ADR 0040 amendment 1): the list has no `Interim` entry left.
    // XLink attributes on a reference element do not conform: the element is
    // kept with SACM-XMI-003, which is no refusal.
    let cases = [
        (
            "an XPointer href to another document",
            package(&claim(
                "c1",
                "",
                "<metaClaim href=\"other.sacm.xmi#xpointer(id('c'))\"/>",
            )),
            None,
        ),
        (
            "XLink attributes on a reference element",
            package(&format!(
                "{}{}",
                claim(
                    "c1",
                    "",
                    "<metaClaim href=\"#c2\" xmlns:xlink=\"http://www.w3.org/1999/xlink\" xlink:type=\"simple\"/>"
                ),
                claim("c2", "", "")
            )),
            Some("SACM-XMI-003/element-kept"),
        ),
    ];
    for (what, input, kept) in cases {
        for mode in MODES {
            let output = match xmi::read(input.as_bytes(), &ReadOptions::default().with_mode(mode))
            {
                Ok(output) => output,
                Err(error) => panic!("{ROW}: {what}: a {mode:?} read refuses it: {error}"),
            };
            let severity = if mode == Mode::Strict {
                Severity::Error
            } else {
                Severity::Warning
            };
            let expected: Vec<(Severity, &str)> =
                kept.map(|kept| (severity, kept)).into_iter().collect();
            assert_eq!(
                output
                    .diagnostics()
                    .iter()
                    .map(|one| (one.severity(), one.template()))
                    .collect::<Vec<_>>(),
                expected,
                "{ROW}: {what}: the diagnostics of a {mode:?} read"
            );
            let c1 = output.document().get("c1");
            let hrefs: Vec<&str> = c1
                .map(|c1| {
                    c1.hrefs(sacm::model::ReferenceEnd::MetaClaim)
                        .iter()
                        .map(sacm::model::Href::as_str)
                        .collect()
                })
                .unwrap_or_default();
            let expected: Vec<&str> = if kept.is_some() {
                Vec::new()
            } else {
                vec!["other.sacm.xmi#xpointer(id('c'))"]
            };
            assert_eq!(
                (
                    hrefs,
                    c1.map(sacm::model::SacmElement::has_preserved_content)
                ),
                (expected, Some(kept.is_some())),
                "{ROW}: {what}: what a {mode:?} read holds in c1"
            );
        }
    }
}

// --- Tolerated: other SACM namespaces (LIB-COMPAT-004) ------------------------------

#[test]
fn lib_api_011_a_root_outside_the_sacm_2_3_namespace_names_lib_compat_004() {
    // Strict mode reads only the pinned SACM 2.3 namespace. A root in an
    // older SACM revision's namespace or a SACM-like one is not defective,
    // and since slice 0072 tolerant mode reads it: both are
    // `Tolerated("LIB-COMPAT-004")` (D-NS-PIN; ADR 0041 D4, D6), and the
    // label says strict mode never reads it. A SACM 2.4 root is a `Limit`
    // (LIB-COMPAT-018).
    let root = |uri: &str| {
        format!(
            "<sacm:ArgumentPackage xmlns:sacm=\"{uri}\" xmlns:xmi=\"{XMI_NS}\" xmi:id=\"p\">\
             <name content=\"p\"/></sacm:ArgumentPackage>"
        )
    };
    assert_text_refused_with(
        "OtherRevision",
        "a root in a SACM 2.2 namespace",
        &root("http://www.omg.org/spec/SACM/2.2/argumentation"),
        &tolerated("LIB-COMPAT-004"),
    );
    assert_text_refused_with(
        "OtherSacmNamespace",
        "a root in a SACM-like namespace",
        &root("urn:example:sacm"),
        &tolerated("LIB-COMPAT-004"),
    );
    assert_text_refused_with(
        "Sacm24",
        "a root in the SACM 2.4 namespace",
        &root("http://www.omg.org/spec/SACM/2.4"),
        LIMIT,
    );
}

// --- Tolerance: LIB-COMPAT-002; the spellings SACM23-XMI-009 reads -----------------

#[test]
fn lib_api_011_an_xmi_element_outside_the_xmi_2_5_1_namespace_names_lib_compat_002() {
    let input = format!(
        "<xmi:XMI xmlns:xmi=\"http://www.omg.org/XMI\" xmlns:x=\"{XMI_NS}\" xmlns:sacm=\"{SACM_NS}\">\
         <sacm:ArgumentPackage x:id=\"p\"><name content=\"p\"/></sacm:ArgumentPackage></xmi:XMI>"
    );
    assert_text_refused_with(
        "OtherXmiNamespace",
        "an XMI element in the XMI 2.0 namespace",
        &input,
        &tolerance("LIB-COMPAT-002"),
    );
}

/// Asserts that `input`, which holds the spelling `argumentationElement`
/// (described by `what`), is no longer refused (SACM23-XMI-009, slice 0057):
/// a strict read returns the document with one SACM-XMI-013 Error at the
/// spelling's start tag, where the refusal was, naming SACM23-XMI-009, whose
/// message is `reason` pinned whole, one sentence with single spaces.
fn assert_spelling_kept(what: &str, input: &str, reason: &str) {
    let output = match xmi::read(input.as_bytes(), &strict()) {
        Ok(output) => output,
        Err(error) => panic!("{XMI_009}: {what}: a strict read refused it: {error:?}"),
    };
    let Some(offset) = input.find("<argumentationElement") else {
        panic!("{XMI_009}: {what}: setup: no spelling in the input");
    };
    let column = u32::try_from(offset + 1).unwrap_or(u32::MAX);
    let found: Vec<_> = output
        .diagnostics()
        .iter()
        .map(|found| {
            (
                found.code(),
                found.severity(),
                found.requirement_id().to_owned(),
                found.location(),
                found.message().to_owned(),
            )
        })
        .collect();
    assert_eq!(
        found,
        [(
            Code::XMI_SPELLING_VARIANT,
            Severity::Error,
            XMI_009.to_owned(),
            SourceLocation::new(1, column),
            reason.to_owned(),
        )],
        "{XMI_009}: {what}: not one SACM-XMI-013 Error with the reason"
    );
    assert!(
        !reason.contains("  ")
            && !LABEL_OPENINGS
                .iter()
                .any(|opening| reason.contains(opening)),
        "{XMI_009}: {what}: the message holds a run of spaces or a refusal label"
    );
}

#[test]
fn sacm23_xmi_009_a_strict_read_keeps_argumentation_element_in_a_package_with_one_error() {
    let input = package(
        "<argumentationElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"><name content=\"c1\"/></argumentationElement>",
    );
    assert_spelling_kept(
        "argumentationElement in an ArgumentPackage",
        &input,
        "the element \"argumentationElement\" in the root element \"sacm:ArgumentPackage\" is \
         spelled as in the text of §11.4, and the role is argumentElement; it is kept as \
         preserved content, and a tolerant read reads the name as argumentElement",
    );
}

#[test]
fn sacm23_xmi_009_a_strict_read_keeps_argumentation_element_in_a_group_with_one_error() {
    let input = package(
        "<argumentElement xmi:type=\"sacm:ArgumentGroup\" xmi:id=\"g\"><name content=\"g\"/>\
         <argumentationElement xmi:idref=\"c1\"/></argumentElement>\
         <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"><name content=\"c1\"/></argumentElement>",
    );
    // The group's end is §11.2's, where the refusal named §11.4.
    assert_spelling_kept(
        "argumentationElement in an ArgumentGroup",
        &input,
        "\"argumentationElement\" in the element \"argumentElement\" is spelled as in the text \
         of §11.2, and the end of an ArgumentGroup is argumentElement; it is kept as preserved \
         content, and a tolerant read reads the name as argumentElement",
    );
}

// --- Limit: encodings ----------------------------------------------------------------

#[test]
fn lib_api_011_an_unread_encoding_is_a_limit_of_this_version() {
    let root = package("");
    let cases: Vec<(&str, Vec<u8>)> = vec![
        (
            "UTF-16 LE without a byte order mark",
            utf16le_without_bom(&format!(
                "<?xml version=\"1.0\" encoding=\"UTF-16LE\"?>{root}"
            )),
        ),
        ("UTF-32 BE with a byte order mark", utf32be_with_bom(&root)),
        // ISO-8859-1 is read since slice 0072 (LIB-COMPAT-016).
        (
            "a declared ISO-8859-2",
            format!("<?xml version=\"1.0\" encoding=\"ISO-8859-2\"?>{root}").into_bytes(),
        ),
    ];
    for (what, input) in cases {
        assert_refused_with("Encoding", what, &input, LIMIT);
    }
}

// --- Defect: content on the wrapper, and an unwritable prefix -----------------------

/// The `xmi:XMI` wrapper with `attributes` and `body` before its one root.
fn wrapper_with(attributes: &str, body: &str) -> String {
    format!(
        "<xmi:XMI xmlns:xmi=\"{XMI_NS}\" xmlns:sacm=\"{SACM_NS}\" xmi:version=\"2.5.1\"{attributes}>{body}\
         <sacm:ArgumentPackage xmi:id=\"p\"><name content=\"p\"/></sacm:ArgumentPackage></xmi:XMI>"
    )
}

#[test]
fn lib_api_011_content_on_the_xmi_wrapper_is_a_defect_in_the_input() {
    let cases = [
        ("an unqualified attribute", wrapper_with(" foo=\"bar\"", "")),
        (
            "a foreign attribute",
            wrapper_with(" xmlns:v=\"urn:example:v\" v:w=\"1\"", ""),
        ),
        ("text before the root", wrapper_with("", "stray")),
        // An XMI-namespace element XMI 2.5.1 does not define there (slice
        // 0073a, SACM23-XMI-019, ADR 0042 D4); the XMI metadata elements are
        // read.
        (
            "an XMI-namespace element XMI does not define",
            wrapper_with("", "<xmi:foo/>"),
        ),
    ];
    for (what, input) in cases {
        assert_text_refused_with("WrapperContent", what, &input, DEFECT);
    }
}

#[test]
fn lib_api_011_a_preserved_attribute_binding_sacm_or_xmi_elsewhere_is_a_defect_in_the_input() {
    let sacm_elsewhere = package(&format!(
        "<argumentElement xmlns:s=\"{SACM_NS}\" xmlns:sacm=\"urn:example:other\" \
         xmi:type=\"s:Claim\" xmi:id=\"c1\" sacm:extra=\"1\"><name content=\"c1\"/></argumentElement>"
    ));
    assert_text_refused_with(
        "UnwritablePrefix",
        "an attribute whose prefix sacm is bound to another URI",
        &sacm_elsewhere,
        DEFECT,
    );
    let xmi_elsewhere = package(&format!(
        "<argumentElement xmlns:x=\"{XMI_NS}\" xmlns:xmi=\"urn:example:other\" \
         x:type=\"sacm:Claim\" x:id=\"c1\" xmi:extra=\"1\"><name content=\"c1\"/></argumentElement>"
    ));
    assert_text_refused_with(
        "UnwritablePrefix",
        "an attribute whose prefix xmi is bound to another URI",
        &xmi_elsewhere,
        DEFECT,
    );
}

// --- The entries a tolerant read refuses too (ADR 0035 D1) --------------------------

/// The refused set of `Unread::tolerant()`: an encoding, a SACM 2.4 root
/// (slice 0072; a root in another SACM namespace is read tolerantly since,
/// LIB-COMPAT-004) and an `XMI` element outside XMI 2.5.1 are `Unsupported`
/// in both modes, with the same refusal. An XMI-namespace document element
/// is `InvalidRoot` since slice 0073a (SACM23-XMI-019, ADR 0042 D4).
/// `Capacity` and `Unheld` have no input of a test's size. Every other entry
/// is kept by a tolerant read (`tolerant_read.rs`).
#[test]
fn lib_api_011_the_refused_entries_are_unsupported_in_both_modes() {
    let root = package("");
    let cases: Vec<(&str, Vec<u8>, String)> = vec![
        (
            "Encoding: UTF-32 BE with a byte order mark",
            utf32be_with_bom(&root),
            LIMIT.to_owned(),
        ),
        (
            "Encoding: a declared ISO-8859-2",
            format!("<?xml version=\"1.0\" encoding=\"ISO-8859-2\"?>{root}").into_bytes(),
            LIMIT.to_owned(),
        ),
        (
            "Sacm24: a root in the SACM 2.4 namespace",
            format!(
                "<sacm:ArgumentPackage xmlns:sacm=\"http://www.omg.org/spec/SACM/20260504\" \
                 xmlns:xmi=\"{XMI_NS}\" xmi:id=\"p\"><name content=\"p\"/></sacm:ArgumentPackage>"
            )
            .into_bytes(),
            LIMIT.to_owned(),
        ),
        (
            "OtherXmiNamespace: an XMI element in the XMI 2.0 namespace",
            format!(
                "<xmi:XMI xmlns:xmi=\"http://www.omg.org/XMI\" xmlns:x=\"{XMI_NS}\" xmlns:sacm=\"{SACM_NS}\">\
                 <sacm:ArgumentPackage x:id=\"p\"><name content=\"p\"/></sacm:ArgumentPackage></xmi:XMI>"
            )
            .into_bytes(),
            tolerance("LIB-COMPAT-002"),
        ),
    ];
    for (what, input, label) in &cases {
        let mut errors = Vec::new();
        for mode in MODES {
            match xmi::read(input, &ReadOptions::default().with_mode(mode)) {
                Ok(output) => panic!(
                    "{ROW}: {what}: a {mode:?} read returned a document, with {:?}",
                    output.diagnostics()
                ),
                Err(error) => {
                    assert!(
                        matches!(
                            &error,
                            ReadError::Unsupported { reason, location, .. }
                                if reason.ends_with(label.as_str()) && location.is_some()
                        ),
                        "{ROW}: {what}: a {mode:?} read is not Unsupported labelled {label:?}: \
                         {error:?}"
                    );
                    errors.push(error);
                }
            }
        }
        assert_eq!(
            errors[0], errors[1],
            "{ROW}: {what}: the modes refuse differently"
        );
    }
}
