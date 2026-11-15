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
//! - `(a defect in the input; ROW is to read it in tolerant mode)` for
//!   `Tolerance(ROW)`;
//! - `(a defect in the input)` for `Defect`.
//!
//! One input (or a few, where the entry covers several constructs) per entry
//! of the list, read in both modes, is refused with that entry's label. Two
//! entries have no input: a parser capacity limit (`Capacity`) and a failed
//! insert into the document (`Unheld`), which no well-formed input of a
//! test's size reaches.
//!
//! The xtask test `xtask/tests/interim_refusals.rs` checks the list itself:
//! that `Unsupported` is built nowhere else, and that each row it names is
//! open and, for `Interim`, gates every compliance point.

use sacm::xmi::{self, Mode, ReadError, ReadOptions};

const ROW: &str = "LIB-API-011";

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

/// The two labels' openings; a reason holds exactly one label.
const LABEL_OPENINGS: [&str; 2] = ["(a limit of this version", "(a defect in the input"];

/// The label of `Interim(row)`.
fn interim(row: &str) -> String {
    format!("(a limit of this version; {row} reads it)")
}

/// The label of `Tolerance(row)`.
fn tolerance(row: &str) -> String {
    format!("(a defect in the input; {row} is to read it in tolerant mode)")
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

/// Asserts that `input` (described by `what`) is `Unsupported` in both
/// modes, with a location, and that its reason ends with `label` and holds
/// no other label.
fn assert_refused_with(entry: &str, what: &str, input: &[u8], label: &str) {
    for mode in MODES {
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
            error.code(),
            None,
            "{ROW}: {entry}: {what}: Unsupported has no catalog code"
        );
        assert!(
            error.to_string().ends_with(label),
            "{ROW}: {entry}: {what}: the message {:?} does not end with the label",
            error.to_string()
        );
    }
}

fn assert_text_refused_with(entry: &str, what: &str, input: &str, label: &str) {
    assert_refused_with(entry, what, input.as_bytes(), label);
}

// --- Interim: the ten package, interface and binding kinds --------------------------

#[test]
fn lib_api_011_each_unread_package_interface_and_binding_kind_names_its_row() {
    let kinds = [
        ("ArtifactPackage", "SACM23-ART-001"),
        ("ArtifactPackageInterface", "SACM23-ART-012"),
        ("ArtifactPackageBinding", "SACM23-ART-013"),
        ("TerminologyPackage", "SACM23-TERM-001"),
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
    // ArgumentPackage, an ArtifactPackage in an AssuranceCasePackage.
    let interface = package(
        "<argumentElement xmi:type=\"sacm:ArgumentPackageInterface\" xmi:id=\"i\"><name content=\"i\"/></argumentElement>",
    );
    assert_text_refused_with(
        "ArgumentPackageInterface",
        "in an ArgumentPackage",
        &interface,
        &interim("SACM23-ARG-014"),
    );
    let artifacts = format!(
        "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\"><name content=\"case\"/>\
         <artifactPackage xmi:id=\"a\"><name content=\"a\"/></artifactPackage>\
         </sacm:AssuranceCasePackage>"
    );
    assert_text_refused_with(
        "ArtifactPackage",
        "in an AssuranceCasePackage",
        &artifacts,
        &interim("SACM23-ART-001"),
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
            "href on an object",
            package(&format!(
                "{}{}",
                claim("c1", " href=\"#c2\"", ""),
                claim("c2", "", "")
            )),
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
    // A root in another namespace is not SACM 2.3, so it does not conform:
    // LIB-COMPAT-004 is a tolerance (the maintainer's decision, 2026-11-16).
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

// --- Interim: ExpressionLangString (SACM23-BASE-009) --------------------------------

#[test]
fn lib_api_011_an_expression_lang_string_names_sacm23_base_009() {
    let label = interim("SACM23-BASE-009");
    let on_name = format!(
        "<sacm:ArgumentPackage{NS} xmi:id=\"p\"><name xmi:type=\"sacm:ExpressionLangString\" content=\"p\"/></sacm:ArgumentPackage>"
    );
    assert_text_refused_with("ExpressionLangString", "as a name", &on_name, &label);
    let on_value = package(&claim(
        "c1",
        "",
        "<note xmi:id=\"n\"><content><value xmi:type=\"sacm:ExpressionLangString\" content=\"v\"/></content></note>",
    ));
    assert_text_refused_with(
        "ExpressionLangString",
        "as a value of a content",
        &on_value,
        &label,
    );
}

// --- Tolerance: LIB-COMPAT-002 and SACM23-XMI-009 -----------------------------------

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

#[test]
fn lib_api_011_the_text_spelling_argumentation_element_names_sacm23_xmi_009() {
    let input = package(
        "<argumentationElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"><name content=\"c1\"/></argumentationElement>",
    );
    assert_text_refused_with(
        "TextSpelling",
        "argumentationElement in an ArgumentPackage",
        &input,
        &tolerance("SACM23-XMI-009"),
    );
    // The reason, pinned whole: one sentence, single spaces.
    let error = xmi::read(input.as_bytes(), &ReadOptions::default()).unwrap_err();
    let ReadError::Unsupported { reason, .. } = &error else {
        panic!("{ROW}: TextSpelling: not Unsupported: {error:?}");
    };
    assert_eq!(
        reason,
        "the element \"argumentationElement\" in the root element \"sacm:ArgumentPackage\" is \
         spelled as in the text of §11.4, and the role is argumentElement (a defect in the \
         input; SACM23-XMI-009 is to read it in tolerant mode)",
        "{ROW}: TextSpelling: the reason"
    );
}

#[test]
fn lib_api_011_the_text_spelling_in_an_argument_group_names_sacm23_xmi_009() {
    let input = package(
        "<argumentElement xmi:type=\"sacm:ArgumentGroup\" xmi:id=\"g\"><name content=\"g\"/>\
         <argumentationElement xmi:idref=\"c1\"/></argumentElement>\
         <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"><name content=\"c1\"/></argumentElement>",
    );
    assert_text_refused_with(
        "TextSpelling",
        "argumentationElement in an ArgumentGroup",
        &input,
        &tolerance("SACM23-XMI-009"),
    );
    // The reason, pinned whole: one sentence, single spaces.
    let error = xmi::read(input.as_bytes(), &ReadOptions::default()).unwrap_err();
    let ReadError::Unsupported { reason, .. } = &error else {
        panic!("{ROW}: TextSpelling: not Unsupported: {error:?}");
    };
    assert_eq!(
        reason,
        "\"argumentationElement\" in the element \"argumentElement\" is spelled as in the text \
         of §11.4, and the end of an ArgumentGroup is argumentElement (a defect in the input; \
         SACM23-XMI-009 is to read it in tolerant mode)",
        "{ROW}: TextSpelling in an ArgumentGroup: the reason"
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
