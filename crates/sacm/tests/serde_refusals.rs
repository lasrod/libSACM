//! `ReadError`, `WriteError` and `WriteAttribute` serialize and read back
//! (LIB-API-017, acceptance 7; ADR 0030 amendment 3, R8, R10 and R11).
//!
//! - `tests/data/golden/serde/read-errors.json` and `write-errors.json` hold
//!   one value per variant, and `Unsupported` once per code an input
//!   reaches: SACM-XMI-014, -007, -003, -002, -015 and SACM-XML-002
//!   (SACM-SEC-002 has no input of a test's size). The samples are real
//!   refusals of `xmi::read` and `xmi::write`, because the variants are
//!   `#[non_exhaustive]`. Serializing them pretty gives each file's bytes
//!   plus a final line feed; reading each file gives values `==` to them,
//!   whose `Display` is theirs (amendment 3, "Records").
//! - A refusal is `{"type", "code", "template", "arguments", …its fields}`,
//!   every key required (R10). These are refused, each against a valid twin
//!   so that it fails for the reason named: an unknown `type` or key, or a
//!   missing key; a `code` that is not the variant's and its template's; a
//!   template of another variant, or an unknown one; arguments missing,
//!   extra or of the wrong kind (R8, R11); a `character` that is not one
//!   character. Arguments out of order read as in the template's order
//!   (LIB-API-019; ADR 0030 amendment 5, which relaxes R11).
//! - `WriteAttribute` is its variant name.

#![cfg(feature = "serde")]

use std::fmt::Debug;
use std::path::Path;

use sacm::edit::{self, CreateArgumentPackage};
use sacm::model::{Document, LangString};
use sacm::xmi::{self, Mode, ReadError, ReadOptions, WriteAttribute, WriteError, WriteOptions};
use serde::Serialize;
use serde::de::DeserializeOwned;

const ROW: &str = "LIB-API-017";

/// The directory of the golden JSON files.
const GOLDEN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/golden/serde");

const XMI_NS: &str = "http://www.omg.org/spec/XMI/20131001";
const SACM_NS: &str = "http://www.omg.org/spec/SACM/20220301";

const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
);

/// The golden file `name`, as text.
fn golden(name: &str) -> String {
    let path = Path::new(GOLDEN).join(name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{ROW}: cannot read {}: {error}", path.display()))
}

/// Serializing `values` pretty gives the bytes of the golden file `name`, and
/// reading the file gives values `==` to `values`, each with its `Display`.
fn assert_golden<T>(name: &str, values: &[T])
where
    T: Serialize + DeserializeOwned + PartialEq + Debug + std::fmt::Display,
{
    let expected = golden(name);
    let written = match serde_json::to_string_pretty(values) {
        Ok(text) => text + "\n",
        Err(error) => panic!("{ROW}: the samples of {name} do not serialize: {error}"),
    };
    assert_eq!(
        written, expected,
        "{ROW}: serializing the samples does not give the bytes of {name}"
    );
    let read: Vec<T> = match serde_json::from_str(&expected) {
        Ok(read) => read,
        Err(error) => panic!("{ROW}: {name} does not read back: {error}"),
    };
    assert_eq!(
        read, values,
        "{ROW}: {name} reads back to values other than the samples"
    );
    let shown = |values: &[T]| values.iter().map(ToString::to_string).collect::<Vec<_>>();
    assert_eq!(
        shown(&read),
        shown(values),
        "{ROW}: a value read from {name} does not display as its sample"
    );
}

/// `valid` reads as a `T`, and `valid` with its first `from` replaced by `to`
/// is refused as data (not as malformed JSON): the change, which `what`
/// names, causes the refusal.
fn assert_refused<T: DeserializeOwned + Debug>(what: &str, valid: &str, from: &str, to: &str) {
    let invalid = valid.replacen(from, to, 1);
    assert_ne!(
        invalid, valid,
        "{ROW}: setup: {what}: {from:?} is not in {valid}"
    );
    if let Err(error) = serde_json::from_str::<T>(valid) {
        panic!("{ROW}: setup: {what}: the valid twin {valid} does not read: {error}");
    }
    match serde_json::from_str::<T>(&invalid) {
        Ok(value) => panic!("{ROW}: {what}: {invalid} was read as {value:?}"),
        Err(error) => assert!(
            error.is_data(),
            "{ROW}: {what}: {invalid} is refused as malformed JSON, not as data: {error}"
        ),
    }
}

// --- The samples -----------------------------------------------------------------------

fn strict_refusal(input: &str) -> ReadError {
    match xmi::read(
        input.as_bytes(),
        &ReadOptions::default().with_mode(Mode::Strict),
    ) {
        Ok(output) => panic!(
            "{ROW}: setup: a strict read accepts {input}, with {:?}",
            output.diagnostics()
        ),
        Err(error) => error,
    }
}

/// One refusal per `ReadError` variant, in declaration order, with
/// `Unsupported` once per code an input reaches.
fn read_samples() -> Vec<ReadError> {
    [
        "<!DOCTYPE r>\n<r/>".to_owned(),
        "<r></s>".to_owned(),
        "<r/>".to_owned(),
        format!("<xmi:XMI xmlns:xmi=\"{XMI_NS}\"/>"),
        format!(
            "<sacm:ArgumentPackage{NS} xmi:id=\"arguments\">\n  \
             <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"/>\n  \
             <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"/>\n</sacm:ArgumentPackage>"
        ),
        format!(
            "<sacm:ArgumentPackage{NS} xmi:id=\"arguments\">\n  \
             <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"\"/>\n</sacm:ArgumentPackage>"
        ),
        // Unsupported: SACM-XMI-014, -007, -003, -002, -015, SACM-XML-002.
        // A TerminologyPackageInterface in the case until slice 0067 read it
        // (SACM23-TERM-007).
        format!(
            "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\">\n  <name content=\"case\"/>\n  \
             <xmi:Extension/>\n</sacm:AssuranceCasePackage>"
        ),
        format!(
            "<sacm:ArgumentPackage{NS} xmi:id=\"arguments\">\n  \
             <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\">\n    \
             <metaClaim href=\"other.sacm.xmi#xpointer(id('c'))\"/>\n  </argumentElement>\n\
             </sacm:ArgumentPackage>"
        ),
        format!(
            "<xmi:XMI xmlns:xmi=\"{XMI_NS}\" xmlns:sacm=\"{SACM_NS}\" foo=\"bar\">\n  \
             <sacm:ArgumentPackage xmi:id=\"p\"/>\n</xmi:XMI>"
        ),
        format!(
            "<sacm:ArgumentPackage xmlns:sacm=\"http://www.omg.org/spec/SACM/2.2/argumentation\" \
             xmlns:xmi=\"{XMI_NS}\" xmi:id=\"p\"/>"
        ),
        format!(
            "<xmi:XMI xmlns:xmi=\"http://www.omg.org/XMI\" xmlns:sacm=\"{SACM_NS}\">\n  \
             <sacm:ArgumentPackage/>\n</xmi:XMI>"
        ),
        "<?xml version=\"1.0\" encoding=\"ISO-8859-1\"?>\n<r/>".to_owned(),
        format!(
            "<sacm:ArgumentPackage{NS} xmi:id=\"arguments\">\n  <name content=\"A\">\n    \
             <content>A</content>\n  </name>\n</sacm:ArgumentPackage>"
        ),
        format!(
            "<sacm:ArgumentPackage{NS} xmi:id=\"arguments\">\n  \
             <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\" assertionDeclaration=\"Assumed\"/>\n\
             </sacm:ArgumentPackage>"
        ),
        format!("<sacm:ArgumentPackage{NS} xmi:id=\"arguments\" name=\"Arguments\"/>"),
        format!(
            "<sacm:ArgumentPackage{NS} xmi:id=\"arguments\">\n  \
             <argumentElement xmi:id=\"c1\"/>\n</sacm:ArgumentPackage>"
        ),
    ]
    .iter()
    .map(|input| strict_refusal(input))
    .collect()
}

/// The variant name of `error`.
fn read_variant(error: &ReadError) -> &'static str {
    match error {
        ReadError::Doctype { .. } => "Doctype",
        ReadError::Malformed { .. } => "Malformed",
        ReadError::InvalidRoot { .. } => "InvalidRoot",
        ReadError::NoPackage { .. } => "NoPackage",
        ReadError::DuplicateId { .. } => "DuplicateId",
        ReadError::InvalidId { .. } => "InvalidId",
        ReadError::Unsupported { .. } => "Unsupported",
        ReadError::RepeatedValue { .. } => "RepeatedValue",
        ReadError::InvalidLiteral { .. } => "InvalidLiteral",
        ReadError::LegacyShape { .. } => "LegacyShape",
        ReadError::MissingType { .. } => "MissingType",
        _ => "<a variant this test does not know>",
    }
}

fn tolerant_document(input: &str) -> Document {
    match xmi::read(input.as_bytes(), &ReadOptions::default()) {
        Ok(output) => output.into_document(),
        Err(error) => panic!("{ROW}: setup: a tolerant read refuses {input}: {error}"),
    }
}

/// One refusal per `WriteError` variant, in declaration order.
fn write_samples() -> Vec<WriteError> {
    let mut unrepresentable = Document::new();
    if let Err(error) = edit::apply(
        &mut unrepresentable,
        CreateArgumentPackage::root(LangString::new("bad\u{1}")).with_id("arguments"),
    ) {
        panic!("{ROW}: setup: {error}");
    }
    let invalid = tolerant_document(&format!(
        "<sacm:ArgumentPackage{NS} xmi:id=\"p\"><name content=\"p\"/>\
         <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\" isCitation=\"true\" \
         citedElement=\"nope\"><name content=\"c1\"/></argumentElement></sacm:ArgumentPackage>"
    ));
    let preserved = tolerant_document(&format!(
        "<sacm:ArgumentPackage{NS} xmlns:v=\"urn:vendor\" xmi:id=\"p\" v:colour=\"red\">\
         <name content=\"p\"/></sacm:ArgumentPackage>"
    ));
    [Document::new(), unrepresentable, invalid, preserved]
        .iter()
        .map(
            |document| match xmi::write(document, &WriteOptions::default()) {
                Ok(text) => panic!("{ROW}: setup: a strict write writes {text}"),
                Err(error) => error,
            },
        )
        .collect()
}

fn write_variant(error: &WriteError) -> &'static str {
    match error {
        WriteError::NoRoot => "NoRoot",
        WriteError::UnrepresentableCharacter { .. } => "UnrepresentableCharacter",
        WriteError::Invalid { .. } => "Invalid",
        WriteError::PreservedContent { .. } => "PreservedContent",
        _ => "<a variant this test does not know>",
    }
}

#[test]
fn lib_api_017_read_errors_json_holds_one_value_per_variant_and_reads_back() {
    let samples = read_samples();
    let variants: Vec<(&str, &str)> = samples
        .iter()
        .map(|error| (read_variant(error), error.code().as_str()))
        .collect();
    assert_eq!(
        variants,
        [
            ("Doctype", "SACM-SEC-001"),
            ("Malformed", "SACM-XML-001"),
            ("InvalidRoot", "SACM-XMI-001"),
            ("NoPackage", "SACM-XMI-001"),
            ("DuplicateId", "SACM-ID-001"),
            ("InvalidId", "SACM-ID-002"),
            ("Unsupported", "SACM-XMI-014"),
            ("Unsupported", "SACM-XMI-007"),
            ("Unsupported", "SACM-XMI-003"),
            ("Unsupported", "SACM-XMI-002"),
            ("Unsupported", "SACM-XMI-015"),
            ("Unsupported", "SACM-XML-002"),
            ("RepeatedValue", "SACM-MULT-001"),
            ("InvalidLiteral", "SACM-ENUM-001"),
            ("LegacyShape", "SACM-XMI-012"),
            ("MissingType", "SACM-XMI-005"),
        ],
        "{ROW}: setup: the samples are not one per variant, Unsupported once per code"
    );
    assert_golden("read-errors.json", &samples);
}

#[test]
fn lib_api_017_write_errors_json_holds_one_value_per_variant_and_reads_back() {
    let samples = write_samples();
    let variants: Vec<(&str, &str)> = samples
        .iter()
        .map(|error| (write_variant(error), error.code().as_str()))
        .collect();
    assert_eq!(
        variants,
        [
            ("NoRoot", "SACM-XMI-001"),
            ("UnrepresentableCharacter", "SACM-XML-003"),
            ("Invalid", "SACM-XMI-016"),
            ("PreservedContent", "SACM-XMI-006"),
        ],
        "{ROW}: setup: the samples are not one per variant"
    );
    assert_golden("write-errors.json", &samples);
}

// --- What is refused -------------------------------------------------------------------

const MALFORMED: &str = r#"{"type":"Malformed","code":"SACM-XML-001","template":"SACM-XML-001/end-tag-mismatch","arguments":{"end_tag":"s","start_tag":"r"},"location":{"line":1,"column":4},"reason":"the end tag \"s\" does not match the start tag \"r\""}"#;

const INVALID_ROOT: &str = r#"{"type":"InvalidRoot","code":"SACM-XMI-001","template":"SACM-XMI-001/not-a-package","arguments":{"element":"r"},"location":{"line":1,"column":1},"element_name":"r"}"#;

const NO_PACKAGE: &str = r#"{"type":"NoPackage","code":"SACM-XMI-001","template":"SACM-XMI-001/no-package","arguments":{},"location":{"line":1,"column":1}}"#;

const DUPLICATE_ID: &str = r#"{"type":"DuplicateId","code":"SACM-ID-001","template":"SACM-ID-001/duplicate-id","arguments":{"id":"c1","count":2},"id":"c1","locations":[{"line":2,"column":3},{"line":3,"column":3}]}"#;

/// An `xmi:Extension` document element (a `TerminologyPackageInterface` root
/// until slice 0067 read it, SACM23-TERM-007).
const UNSUPPORTED: &str = r#"{"type":"Unsupported","code":"SACM-XMI-014","template":"SACM-XMI-014/xmi-root","arguments":{"element":"xmi:Extension","row":"SACM23-XMI-019"},"location":{"line":1,"column":1},"reason":"the XMI element \"xmi:Extension\" is not read as a root in this release (a limit of this version; SACM23-XMI-019 reads it)"}"#;

const NO_ROOT: &str = r#"{"type":"NoRoot","code":"SACM-XMI-001","template":"SACM-XMI-001/no-root-to-write","arguments":{}}"#;

const UNREPRESENTABLE: &str = r#"{"type":"UnrepresentableCharacter","code":"SACM-XML-003","template":"SACM-XML-003/unrepresentable-character","arguments":{"element":"arguments","attribute":"name's content","code_point":1},"element":"arguments","character":"\u0001","attribute":"NameContent"}"#;

const PRESERVED: &str = r#"{"type":"PreservedContent","code":"SACM-XMI-006","template":"SACM-XMI-006/one-holder","arguments":{"id":"p"},"elements":["p"]}"#;

#[test]
fn lib_api_017_a_refusal_with_an_unknown_type_or_key_or_a_missing_key_is_refused() {
    assert_refused::<ReadError>(
        "an unknown type",
        MALFORMED,
        r#""type":"Malformed""#,
        r#""type":"Malformd""#,
    );
    assert_refused::<WriteError>(
        "an unknown type",
        NO_ROOT,
        r#""type":"NoRoot""#,
        r#""type":"NoRoots""#,
    );
    assert_refused::<ReadError>(
        "an unknown key",
        MALFORMED,
        r#","reason":"#,
        r#","extra":null,"reason":"#,
    );
    assert_refused::<WriteError>(
        "an unknown key",
        NO_ROOT,
        r#""arguments":{}"#,
        r#""arguments":{},"elements":[]"#,
    );
    for (what, key) in [
        ("a missing type", r#""type":"Malformed","#),
        ("a missing code", r#""code":"SACM-XML-001","#),
        (
            "a missing template",
            r#""template":"SACM-XML-001/end-tag-mismatch","#,
        ),
        (
            "missing arguments",
            r#""arguments":{"end_tag":"s","start_tag":"r"},"#,
        ),
        ("a missing location", r#""location":{"line":1,"column":4},"#),
    ] {
        assert_refused::<ReadError>(what, MALFORMED, key, "");
    }
    assert_refused::<ReadError>(
        "a missing reason",
        MALFORMED,
        r#","reason":"the end tag \"s\" does not match the start tag \"r\"""#,
        "",
    );
    assert_refused::<ReadError>(
        "missing locations",
        DUPLICATE_ID,
        r#","locations":[{"line":2,"column":3},{"line":3,"column":3}]"#,
        "",
    );
    assert_refused::<WriteError>("missing arguments", NO_ROOT, r#","arguments":{}"#, "");
    assert_refused::<WriteError>("missing elements", PRESERVED, r#","elements":["p"]"#, "");
}

#[test]
fn lib_api_017_a_refusal_whose_code_is_not_its_variant_s_and_its_template_s_is_refused() {
    assert_refused::<ReadError>(
        "a code of another variant",
        MALFORMED,
        r#""code":"SACM-XML-001""#,
        r#""code":"SACM-XMI-001""#,
    );
    // SACM-XMI-007 is a code of Unsupported, but not of this template.
    assert_refused::<ReadError>(
        "a code of the variant that is not the template's",
        UNSUPPORTED,
        r#""code":"SACM-XMI-014""#,
        r#""code":"SACM-XMI-007""#,
    );
    assert_refused::<WriteError>(
        "a code of another variant",
        NO_ROOT,
        r#""code":"SACM-XMI-001""#,
        r#""code":"SACM-XMI-006""#,
    );
    assert_refused::<WriteError>(
        "an unknown code",
        PRESERVED,
        r#""code":"SACM-XMI-006""#,
        r#""code":"SACM-XMI-999""#,
    );
}

/// R11's order: the code is checked against the variant before the template
/// is looked up, so a record with both wrong is refused for its code.
#[test]
fn lib_api_017_a_code_is_checked_against_its_variant_before_the_template() {
    let both = MALFORMED.replacen(
        r#""code":"SACM-XML-001","template":"SACM-XML-001/end-tag-mismatch""#,
        r#""code":"SACM-XMI-001","template":"SACM-XML-001/no-such-form""#,
        1,
    );
    assert_ne!(
        both, MALFORMED,
        "{ROW}: setup: the code and template are not in the twin"
    );
    let message = match serde_json::from_str::<ReadError>(&both) {
        Ok(value) => panic!("{ROW}: {both} was read as {value:?}"),
        Err(error) => error.to_string(),
    };
    assert!(
        message.contains("the code SACM-XMI-001 is not one of this refusal's codes"),
        "{ROW}: the refusal does not name the code first: {message}"
    );
}

#[test]
fn lib_api_017_a_template_of_another_variant_or_an_unknown_template_is_refused() {
    // InvalidRoot and NoPackage share SACM-XMI-001: the code is the variant's
    // and the template's, and the arguments are the template's.
    assert_refused::<ReadError>(
        "the template of NoPackage on InvalidRoot",
        INVALID_ROOT,
        r#""template":"SACM-XMI-001/not-a-package","arguments":{"element":"r"}"#,
        r#""template":"SACM-XMI-001/no-package","arguments":{}"#,
    );
    assert_refused::<ReadError>(
        "the template of InvalidRoot on NoPackage",
        NO_PACKAGE,
        r#""template":"SACM-XMI-001/no-package","arguments":{}"#,
        r#""template":"SACM-XMI-001/not-a-package","arguments":{"element":"r"}"#,
    );
    // NoRoot shares SACM-XMI-001 with two ReadError templates.
    assert_refused::<WriteError>(
        "a ReadError template on NoRoot",
        NO_ROOT,
        r#""template":"SACM-XMI-001/no-root-to-write""#,
        r#""template":"SACM-XMI-001/no-package""#,
    );
    assert_refused::<ReadError>(
        "a WriteError template on NoPackage",
        NO_PACKAGE,
        r#""template":"SACM-XMI-001/no-package""#,
        r#""template":"SACM-XMI-001/no-root-to-write""#,
    );
    assert_refused::<ReadError>(
        "an unknown template",
        MALFORMED,
        r#""template":"SACM-XML-001/end-tag-mismatch""#,
        r#""template":"SACM-XML-001/no-such-form""#,
    );
    // The bare code text is the default template of a Diagnostic only (R9).
    assert_refused::<ReadError>(
        "the code's text as the template",
        MALFORMED,
        r#""template":"SACM-XML-001/end-tag-mismatch""#,
        r#""template":"SACM-XML-001""#,
    );
    assert_refused::<WriteError>(
        "the code's text as the template",
        PRESERVED,
        r#""template":"SACM-XMI-006/one-holder""#,
        r#""template":"SACM-XMI-006""#,
    );
}

#[test]
fn lib_api_017_arguments_missing_extra_or_of_the_wrong_kind_are_refused() {
    let arguments = r#""arguments":{"end_tag":"s","start_tag":"r"}"#;
    for (what, to) in [
        ("a missing argument", r#""arguments":{"end_tag":"s"}"#),
        (
            "an extra argument",
            r#""arguments":{"end_tag":"s","start_tag":"r","extra":"x"}"#,
        ),
        (
            "a Text given a number",
            r#""arguments":{"end_tag":1,"start_tag":"r"}"#,
        ),
        (
            "a Text given null",
            r#""arguments":{"end_tag":null,"start_tag":"r"}"#,
        ),
        ("arguments as an array", r#""arguments":["s","r"]"#),
    ] {
        assert_refused::<ReadError>(what, MALFORMED, arguments, to);
    }
    for (what, to) in [
        ("a Count given a string", r#""count":"2""#),
        ("a negative Count", r#""count":-1"#),
        ("a fractional Count", r#""count":2.5"#),
    ] {
        assert_refused::<ReadError>(what, DUPLICATE_ID, r#""count":2"#, to);
    }
    assert_refused::<ReadError>(
        "an Unsupported template without its row",
        UNSUPPORTED,
        r#","row":"SACM23-XMI-019""#,
        "",
    );
    assert_refused::<WriteError>(
        "an argument for a template that takes none",
        NO_ROOT,
        r#""arguments":{}"#,
        r#""arguments":{"id":"p"}"#,
    );
    assert_refused::<WriteError>(
        "a Count given as text",
        UNREPRESENTABLE,
        r#""code_point":1"#,
        r#""code_point":"1""#,
    );
}

/// The case "arguments out of order" of the test above, refused by R11 until
/// ADR 0030 amendment 5: the record reads, `==` to its twin.
#[test]
fn lib_api_019_a_malformed_record_with_its_arguments_swapped_reads_as_its_twin() {
    let swapped = MALFORMED.replacen(
        r#""arguments":{"end_tag":"s","start_tag":"r"}"#,
        r#""arguments":{"start_tag":"r","end_tag":"s"}"#,
        1,
    );
    assert_ne!(swapped, MALFORMED, "LIB-API-019: setup: nothing swapped");
    let twin: ReadError = match serde_json::from_str(MALFORMED) {
        Ok(twin) => twin,
        Err(error) => panic!("LIB-API-019: setup: the twin does not read: {error}"),
    };
    match serde_json::from_str::<ReadError>(&swapped) {
        Ok(read) => assert_eq!(
            read, twin,
            "LIB-API-019: the record with its arguments swapped reads as another value"
        ),
        Err(error) => panic!("LIB-API-019: {swapped} is refused: {error}"),
    }
}

#[test]
fn lib_api_017_a_character_that_is_not_one_character_is_refused() {
    for (what, to) in [
        ("two characters", r#""character":"ab""#),
        ("no character", r#""character":"""#),
        ("a number", r#""character":1"#),
    ] {
        assert_refused::<WriteError>(what, UNREPRESENTABLE, r#""character":"\u0001""#, to);
    }
    assert_refused::<WriteError>(
        "an unknown attribute",
        UNREPRESENTABLE,
        r#""attribute":"NameContent""#,
        r#""attribute":"NameContents""#,
    );
}

/// A record keeps its fields as given, and its `Display` renders its
/// template and arguments, not its `reason` (ADR 0030 amendment 3,
/// "Records").
#[test]
fn lib_api_017_a_record_s_display_renders_its_template_not_its_reason() {
    for (what, valid, reason, display_end) in [
        (
            "Malformed",
            MALFORMED,
            r#""reason":"the end tag \"s\" does not match the start tag \"r\"""#,
            "the input is not well-formed XML at line 1, column 4: the end tag \"s\" does not \
             match the start tag \"r\"",
        ),
        (
            "Unsupported",
            UNSUPPORTED,
            r#""reason":"the XMI element \"xmi:Extension\" is not read as a root in this release (a limit of this version; SACM23-XMI-019 reads it)""#,
            "is not read as a root in this release (a limit of this version; SACM23-XMI-019 reads it)",
        ),
    ] {
        let other = valid.replacen(reason, r#""reason":"other""#, 1);
        assert_ne!(
            other, valid,
            "{ROW}: setup: {what}: the reason is not in the twin"
        );
        let error = match serde_json::from_str::<ReadError>(&other) {
            Ok(error) => error,
            Err(error) => panic!("{ROW}: {what}: a record with another reason is refused: {error}"),
        };
        let kept = match &error {
            ReadError::Malformed { reason, .. } | ReadError::Unsupported { reason, .. } => {
                reason.as_str()
            }
            other => panic!("{ROW}: {what}: read as {other:?}"),
        };
        assert_eq!(
            kept, "other",
            "{ROW}: {what}: the reason is not kept as given"
        );
        assert!(
            error.to_string().ends_with(display_end),
            "{ROW}: {what}: the Display {:?} does not render the template",
            error.to_string()
        );
        if what == "Malformed" {
            assert_eq!(error.to_string(), display_end, "{ROW}: {what}: the Display");
        }
    }
}

/// The arguments of a sample display as `name=value`, a Text Debug-quoted,
/// and the serde refusals of an argument value and of the arguments say
/// what they expect.
#[test]
fn lib_api_017_arguments_display_by_name_and_serde_says_what_it_expects() {
    let error: ReadError = match serde_json::from_str(DUPLICATE_ID) {
        Ok(error) => error,
        Err(error) => panic!("{ROW}: setup: {error}"),
    };
    let shown: Vec<String> = error.arguments().iter().map(ToString::to_string).collect();
    assert_eq!(
        shown,
        [r#"id="c1""#, "count=2"],
        "{ROW}: the arguments' Display"
    );
    for (what, from, to, expected) in [
        (
            "a Text given null",
            r#""end_tag":"s""#,
            r#""end_tag":null"#,
            "a string or a non-negative integer",
        ),
        (
            "arguments as an array",
            r#""arguments":{"end_tag":"s","start_tag":"r"}"#,
            r#""arguments":["s","r"]"#,
            "an object of message arguments",
        ),
    ] {
        let invalid = MALFORMED.replacen(from, to, 1);
        assert_ne!(invalid, MALFORMED, "{ROW}: setup: {what}");
        match serde_json::from_str::<ReadError>(&invalid) {
            Ok(value) => panic!("{ROW}: {what}: read as {value:?}"),
            Err(error) => assert!(
                error.to_string().contains(expected),
                "{ROW}: {what}: the refusal {error} does not say {expected:?}"
            ),
        }
    }
}

#[test]
fn lib_api_017_write_attribute_is_its_variant_name() {
    let all = [
        (WriteAttribute::Id, "Id"),
        (WriteAttribute::NameLang, "NameLang"),
        (WriteAttribute::NameContent, "NameContent"),
        (WriteAttribute::Gid, "Gid"),
        (WriteAttribute::ContentValueLang, "ContentValueLang"),
        (WriteAttribute::ContentValueContent, "ContentValueContent"),
        (WriteAttribute::KeyValueLang, "KeyValueLang"),
        (WriteAttribute::KeyValueContent, "KeyValueContent"),
        (WriteAttribute::Version, "Version"),
        (WriteAttribute::Date, "Date"),
        (WriteAttribute::StartTime, "StartTime"),
        (WriteAttribute::EndTime, "EndTime"),
        (WriteAttribute::Value, "Value"),
        (WriteAttribute::ExternalReference, "ExternalReference"),
        (WriteAttribute::LocationValueLang, "LocationValueLang"),
        (WriteAttribute::LocationValueContent, "LocationValueContent"),
    ];
    for (attribute, name) in all {
        let json = format!("{name:?}");
        assert_eq!(
            serde_json::to_string(&attribute).ok(),
            Some(json.clone()),
            "{ROW}: WriteAttribute::{name} serializes as its name"
        );
        assert_eq!(
            serde_json::from_str::<WriteAttribute>(&json).ok(),
            Some(attribute),
            "{ROW}: WriteAttribute::{name} reads back"
        );
    }
    assert!(
        serde_json::from_str::<WriteAttribute>("\"name's content\"").is_err(),
        "{ROW}: the Display of a WriteAttribute is no name"
    );
}
