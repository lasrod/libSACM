//! Tests that every `ReadError::Unsupported` names why it is refused
//! (LIB-API-011; ADR 0033; D-PRESERVE; D-STRICT).
//!
//! Conforming content this version does not read yet, and input with
//! nowhere to be kept, stays `Unsupported`. Each refusal comes from one
//! crate-private list whose entries are `Interim(row)`, `Tolerance(row)`,
//! `Limit` or `Defect`, and its reason ends with exactly one label:
//!
//! - `(a limit of this version; ROW reads it)` for `Interim(ROW)`;
//! - `(a limit of this version)` for `Limit`;
//! - `(strict mode never reads it; ROW is to read it in tolerant mode)`
//!   for `Tolerance(ROW)`: input strict mode does not read, which need not
//!   be defective, such as a document in another SACM namespace (D-NS-PIN);
//! - `(a defect in the input)` for `Defect`.
//!
//! One input (or a few, where the entry covers several constructs) per entry
//! of the list, read in strict mode, is refused with that entry's label. A
//! tolerant read refuses only the limits, the other namespaces and an XMI
//! document element, and keeps the rest as preserved content with a Warning
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

/// The declaration of `xsi` alone.
const XSI: &str = "xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"";

/// The labels' openings; a reason holds exactly one label.
const LABEL_OPENINGS: [&str; 3] = [
    "(a limit of this version",
    "(strict mode never reads it",
    "(a defect in the input",
];

/// The label of `Interim(row)`.
fn interim(row: &str) -> String {
    format!("(a limit of this version; {row} reads it)")
}

/// The label of `Tolerance(row)`.
fn tolerance(row: &str) -> String {
    format!("(strict mode never reads it; {row} is to read it in tolerant mode)")
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

/// A root of the class `kind`, `x`, named.
fn root_of(kind: &str) -> String {
    format!("<sacm:{kind}{NS} xmi:id=\"x\"><name content=\"x\"/></sacm:{kind}>")
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

// --- Interim: the eight package, interface and binding kinds ------------------------

#[test]
fn lib_api_011_each_unread_package_interface_and_binding_kind_names_its_row() {
    // An ArtifactPackage is read since slice 0052 (SACM23-ART-001), and a
    // TerminologyPackage since slice 0062 (SACM23-TERM-001): eight kinds.
    let kinds = [
        ("ArtifactPackageInterface", "SACM23-ART-012"),
        ("ArtifactPackageBinding", "SACM23-ART-013"),
        ("TerminologyPackageInterface", "SACM23-TERM-007"),
        ("TerminologyPackageBinding", "SACM23-TERM-008"),
        ("AssuranceCasePackageInterface", "SACM23-AC-002"),
        ("AssuranceCasePackageBinding", "SACM23-AC-003"),
        ("ArgumentPackageInterface", "SACM23-ARG-014"),
        ("ArgumentPackageBinding", "SACM23-ARG-015"),
    ];
    for (kind, row) in kinds {
        assert_text_refused_with(kind, "as the root", &root_of(kind), &interim(row));
    }
}

#[test]
fn lib_api_011_an_unread_kind_inside_a_package_names_its_row() {
    // The same kinds, as children: an ArgumentPackageInterface in an
    // ArgumentPackage, a TerminologyPackageInterface typed in the
    // terminologyPackage role of an AssuranceCasePackage (a TerminologyPackage
    // is read since slice 0062).
    let interface = package(
        "<argumentElement xmi:type=\"sacm:ArgumentPackageInterface\" xmi:id=\"i\"><name content=\"i\"/></argumentElement>",
    );
    assert_text_refused_with(
        "ArgumentPackageInterface",
        "in an ArgumentPackage",
        &interface,
        &interim("SACM23-ARG-014"),
    );
    let terminology = format!(
        "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\"><name content=\"case\"/>\
         <terminologyPackage xmi:type=\"sacm:TerminologyPackageInterface\" xmi:id=\"f\">\
         <name content=\"f\"/></terminologyPackage></sacm:AssuranceCasePackage>"
    );
    assert_text_refused_with(
        "TerminologyPackageInterface",
        "in an AssuranceCasePackage",
        &terminology,
        &interim("SACM23-TERM-007"),
    );
}

// --- Interim: the kinds an ArtifactPackage holds that are not read yet --------------

/// A root ArtifactPackage `a` holding `body`.
fn artifact_package(body: &str) -> String {
    format!(
        "<sacm:ArtifactPackage{NS} xmi:id=\"a\"><name content=\"a\"/>{body}</sacm:ArtifactPackage>"
    )
}

#[test]
fn lib_api_011_each_unread_artifact_element_kind_names_its_row() {
    // The kinds that an ArtifactPackage can hold since slice 0052 reads it
    // (ADR 0034), and that are not read yet, in its artifactElement: one
    // entry each. Slice 0055 reads Artifact, Resource, Activity and Event
    // (ADR 0034 amendment 1), and slice 0057 ArtifactAssetRelationship
    // (amendment 2), and slice 0062 the Terminology kinds (amendment 3), whose
    // entries are gone; the interface and binding kinds, refused as roots
    // above, are sampled here as children.
    let kinds = [
        ("ArtifactPackageInterface", "SACM23-ART-012"),
        ("TerminologyPackageInterface", "SACM23-TERM-007"),
        ("TerminologyPackageBinding", "SACM23-TERM-008"),
    ];
    for (kind, row) in kinds {
        let input = artifact_package(&format!(
            "<artifactElement xmi:type=\"sacm:{kind}\" xmi:id=\"x\"><name content=\"x\"/></artifactElement>"
        ));
        assert_text_refused_with(kind, "in an ArtifactPackage", &input, &interim(row));
    }
}

#[test]
fn lib_api_011_an_unread_kind_in_a_nested_artifact_package_names_its_row() {
    // The typed child this test first used was the model's spelling
    // `ArtifactAssertedRelationship`, which slice 0057 reads in tolerant mode
    // and keeps as preserved content in strict mode (SACM23-XMI-009,
    // model_spellings.rs). An unread kind one package deeper is refused the
    // same way.
    let input = artifact_package(
        "<artifactElement xmi:type=\"sacm:ArtifactPackage\" xmi:id=\"n\"><name content=\"n\"/>\
         <artifactElement xmi:type=\"sacm:ArtifactPackageInterface\" xmi:id=\"r\"><name content=\"r\"/>\
         </artifactElement></artifactElement>",
    );
    assert_text_refused_with(
        "ArtifactPackageInterface",
        "in an ArtifactPackage nested in an ArtifactPackage",
        &input,
        &interim("SACM23-ART-012"),
    );
}

// --- Interim: XMI document metadata (SACM23-XMI-019) --------------------------------

#[test]
fn lib_api_011_xmi_metadata_names_sacm23_xmi_019() {
    let label = interim("SACM23-XMI-019");
    let wrapper = |attributes: &str, body: &str| {
        format!(
            "<xmi:XMI xmlns:xmi=\"{XMI_NS}\" xmlns:sacm=\"{SACM_NS}\"{attributes}>{body}\
             <sacm:ArgumentPackage xmi:id=\"p\"><name content=\"p\"/></sacm:ArgumentPackage></xmi:XMI>"
        )
    };
    let cases = [
        (
            "xmi:uuid on an object",
            package(&claim("c1", " xmi:uuid=\"u1\"", "")),
        ),
        (
            "xmi:label on an object",
            package(&claim("c1", " xmi:label=\"l\"", "")),
        ),
        (
            "xmi:idref on an object",
            package(&format!(
                "{}{}",
                claim("c1", " xmi:idref=\"c2\"", ""),
                claim("c2", "", "")
            )),
        ),
        (
            // On a root, which is no role element: a typed role element
            // with an unqualified `href` is a proxy, which names
            // SACM23-XMI-027 (untyped_abstract_roles.rs).
            "href on an object (the root)",
            format!(
                "<sacm:ArgumentPackage{NS} xmi:id=\"p\" href=\"#c2\"><name content=\"p\"/>{}</sacm:ArgumentPackage>",
                claim("c2", "", "")
            ),
        ),
        (
            "xmi:Extension in an object",
            package(&claim("c1", "", "<xmi:Extension extender=\"v\"/>")),
        ),
        (
            "xmi:Documentation in the wrapper",
            wrapper("", "<xmi:Documentation/>"),
        ),
        (
            "xmi:Documentation as the document element",
            format!("<xmi:Documentation xmlns:xmi=\"{XMI_NS}\"/>"),
        ),
        ("xmi:id on the wrapper", wrapper(" xmi:id=\"w\"", "")),
        (
            "xmi:type on the wrapper",
            wrapper(" xmi:type=\"xmi:XMI\"", ""),
        ),
        ("xmi:uuid on the wrapper", wrapper(" xmi:uuid=\"u\"", "")),
        ("xmi:label on the wrapper", wrapper(" xmi:label=\"l\"", "")),
        ("xmi:idref on the wrapper", wrapper(" xmi:idref=\"p\"", "")),
        ("href on the wrapper", wrapper(" href=\"#p\"", "")),
        (
            "xsi:schemaLocation on the wrapper",
            wrapper(
                &format!(" {XSI} xsi:schemaLocation=\"{SACM_NS} SACM.xsd\""),
                "",
            ),
        ),
        (
            "xsi:noNamespaceSchemaLocation on the wrapper",
            wrapper(
                &format!(" {XSI} xsi:noNamespaceSchemaLocation=\"SACM.xsd\""),
                "",
            ),
        ),
        (
            "xmi:id on a reference element",
            package(&format!(
                "{}{}",
                claim("c1", "", "<metaClaim xmi:idref=\"c2\" xmi:id=\"r\"/>"),
                claim("c2", "", "")
            )),
        ),
        (
            "xmi:uuid on a reference element",
            package(&format!(
                "{}{}",
                claim("c1", "", "<metaClaim xmi:idref=\"c2\" xmi:uuid=\"u\"/>"),
                claim("c2", "", "")
            )),
        ),
        (
            "xsi:schemaLocation on the root",
            format!(
                "<sacm:ArgumentPackage{NS} xmi:id=\"p\" xsi:schemaLocation=\"{SACM_NS} SACM.xsd\"><name content=\"p\"/></sacm:ArgumentPackage>"
            ),
        ),
        (
            "xsi:noNamespaceSchemaLocation on the root",
            format!(
                "<sacm:ArgumentPackage{NS} xmi:id=\"p\" xsi:noNamespaceSchemaLocation=\"SACM.xsd\"><name content=\"p\"/></sacm:ArgumentPackage>"
            ),
        ),
    ];
    for (what, input) in cases {
        assert_text_refused_with("XmiMetadata", what, &input, &label);
    }
}

// --- Interim: cross-document references (SACM23-XMI-027) ----------------------------

#[test]
fn lib_api_011_a_cross_document_href_names_sacm23_xmi_027() {
    let label = interim("SACM23-XMI-027");
    let cases = [
        (
            "href to another document",
            package(&claim("c1", "", "<metaClaim href=\"other.sacm.xmi#c\"/>")),
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
        ),
    ];
    for (what, input) in cases {
        assert_text_refused_with("CrossDocumentHref", what, &input, &label);
    }
}

// --- Tolerance: other SACM namespaces (LIB-COMPAT-004) ------------------------------

#[test]
fn lib_api_011_a_root_outside_the_sacm_2_3_namespace_names_lib_compat_004() {
    // Strict mode reads only the pinned SACM 2.3 namespace. A root in
    // another one is not defective, but only tolerant mode is to read it:
    // LIB-COMPAT-004 is a tolerance (D-NS-PIN; the maintainer, 2026-11-16),
    // and its label says strict mode never reads it.
    let other = format!(
        "<sacm:ArgumentPackage xmlns:sacm=\"http://www.omg.org/spec/SACM/2.2/argumentation\" \
         xmlns:xmi=\"{XMI_NS}\" xmi:id=\"p\"><name content=\"p\"/></sacm:ArgumentPackage>"
    );
    assert_text_refused_with(
        "OtherSacmNamespace",
        "a root in a SACM 2.2 namespace",
        &other,
        &tolerance("LIB-COMPAT-004"),
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
        (
            "a declared ISO-8859-1",
            format!("<?xml version=\"1.0\" encoding=\"ISO-8859-1\"?>{root}").into_bytes(),
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

/// The refused set of `Unread::tolerant()`: an encoding, a root outside
/// SACM 2.3, an `XMI` element outside XMI 2.5.1 and an XMI-namespace
/// document element are `Unsupported` in both modes, with the same refusal.
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
            "Encoding: a declared ISO-8859-1",
            format!("<?xml version=\"1.0\" encoding=\"ISO-8859-1\"?>{root}").into_bytes(),
            LIMIT.to_owned(),
        ),
        (
            "OtherSacmNamespace: a root in a SACM 2.2 namespace",
            format!(
                "<sacm:ArgumentPackage xmlns:sacm=\"http://www.omg.org/spec/SACM/2.2/argumentation\" \
                 xmlns:xmi=\"{XMI_NS}\" xmi:id=\"p\"><name content=\"p\"/></sacm:ArgumentPackage>"
            )
            .into_bytes(),
            tolerance("LIB-COMPAT-004"),
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
        (
            "XmiMetadata: xmi:Documentation as the document element",
            format!("<xmi:Documentation xmlns:xmi=\"{XMI_NS}\"/>").into_bytes(),
            interim("SACM23-XMI-019"),
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
