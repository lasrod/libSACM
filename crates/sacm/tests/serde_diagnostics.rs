//! A `Diagnostic` serializes with its template and arguments, and reads back
//! (LIB-API-018, acceptance 5; ADR 0030 amendment 3, R8, R9, R11 and
//! Records).
//!
//! - `tests/data/golden/serde/diagnostics.json` holds two diagnostics built
//!   with `test-support`'s `new`, whose template is their code's text and
//!   which have no arguments, and the one finding of a real `validate`
//!   (acceptance 2). Serializing them pretty gives the file's bytes plus a
//!   final line feed, every value writes `template` and `arguments` right
//!   after `message`, and reading the file gives values `==` to them.
//! - `diagnostics-0.1.0.json` is the file as 0.1.0 wrote it, byte for byte:
//!   no `template` and no `arguments`. It still reads (R9), each value with
//!   its code's text as template and no arguments, and serializes again as
//!   the first two values of `diagnostics.json`.
//! - These are refused, each against a valid twin so that it fails for the
//!   reason named, with the reason the design gives: `template` without
//!   `arguments` or the reverse, or either `null`; an unknown template, a
//!   refusal's, or another code's; the code's text with arguments; arguments
//!   missing, extra, out of order or of the wrong kind (R8, R11); and a
//!   refusal record that names a diagnostic template.
//! - The `Invalid` value of `write-errors.json` carries its diagnostic's
//!   template and arguments.
//!
#![cfg(feature = "serde")]

use std::fmt::Debug;
use std::path::Path;

use sacm::diagnostic::{ArgumentValue, Code, Diagnostic, Severity, SourceLocation};
use sacm::model::ElementId;
use sacm::validation;
use sacm::xmi::{self, ReadError, ReadOptions, WriteError, WriteOptions};
use serde::de::DeserializeOwned;

const ROW: &str = "LIB-API-018";

/// The directory of the golden JSON files.
const GOLDEN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/golden/serde");

const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
);

/// The keys of a serialized diagnostic, in order.
const KEYS: [&str; 9] = [
    "code",
    "severity",
    "requirement_id",
    "clause",
    "affected",
    "location",
    "message",
    "template",
    "arguments",
];

/// The golden file `name`, as text.
fn golden(name: &str) -> String {
    let path = Path::new(GOLDEN).join(name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{ROW}: cannot read {}: {error}", path.display()))
}

/// `text` read as a `T`, which it must be.
fn from_json<T: DeserializeOwned>(what: &str, text: &str) -> T {
    serde_json::from_str(text)
        .unwrap_or_else(|error| panic!("{ROW}: {what}: {text} does not read: {error}"))
}

/// `value` as pretty JSON, with a final line feed.
fn pretty<T: serde::Serialize + Debug>(value: &T) -> String {
    match serde_json::to_string_pretty(value) {
        Ok(text) => text + "\n",
        Err(error) => panic!("{ROW}: {value:?} does not serialize: {error}"),
    }
}

/// `valid` reads as a `T`, and `valid` with its first `from` replaced by `to`
/// is refused as data (not as malformed JSON) with a message that holds
/// `reason`: the change, which `what` names, causes the refusal.
fn assert_refused<T: DeserializeOwned + Debug>(
    what: &str,
    valid: &str,
    from: &str,
    to: &str,
    reason: &str,
) {
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
        Err(error) => {
            assert!(
                error.is_data(),
                "{ROW}: {what}: {invalid} is refused as malformed JSON, not as data: {error}"
            );
            assert!(
                error.to_string().contains(reason),
                "{ROW}: {what}: the refusal {error} does not say {reason:?}"
            );
        }
    }
}

/// The arguments of `diagnostic` by name and value.
fn named(diagnostic: &Diagnostic) -> Vec<(&'static str, ArgumentValue)> {
    diagnostic
        .arguments()
        .iter()
        .map(|argument| (argument.name(), argument.value().clone()))
        .collect()
}

fn text(value: &str) -> ArgumentValue {
    ArgumentValue::Text(value.to_owned())
}

/// The keys of each object of the pretty JSON array `json`, in order: the
/// lines indented by four spaces that start a key.
fn keys_of_each_value(json: &str) -> Vec<Vec<String>> {
    let mut values: Vec<Vec<String>> = Vec::new();
    for line in json.lines() {
        if line == "  {" {
            values.push(Vec::new());
        } else if let Some(rest) = line.strip_prefix("    \"") {
            if let (Some((key, _)), Some(keys)) = (rest.split_once("\":"), values.last_mut()) {
                keys.push(key.to_owned());
            }
        }
    }
    values
}

// --- The samples -------------------------------------------------------------------------

/// A Claim with two descriptions: exactly one finding (acceptance 2).
const CLM_132: &str = concat!(
    "<sacm:ArgumentPackage xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\" ",
    "xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" xmi:id=\"p\"><name content=\"P\"/>\n",
    "  <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"CLM_132\"><name content=\"C\"/>",
    "<description xmi:id=\"d1\"/><description xmi:id=\"d2\"/></argumentElement>\n",
    "</sacm:ArgumentPackage>",
);

/// The one finding of `validate` for [`CLM_132`].
fn two_descriptions_finding() -> Diagnostic {
    let document = match xmi::read(CLM_132.as_bytes(), &ReadOptions::default()) {
        Ok(output) => output.into_document(),
        Err(error) => panic!("{ROW}: setup: {CLM_132} does not read: {error}"),
    };
    let mut found = validation::validate(&document);
    assert_eq!(found.len(), 1, "{ROW}: setup: CLM_132 gives {found:?}");
    found.remove(0)
}

/// The values of `diagnostics.json`, as `serde.rs` builds them: two built
/// with `new`, and the finding of `CLM_132`.
fn samples() -> Vec<Diagnostic> {
    let location =
        SourceLocation::new(4, 20).unwrap_or_else(|| panic!("{ROW}: setup: 4:20 is a location"));
    vec![
        Diagnostic::new(
            Code::REF_DANGLING,
            Severity::Error,
            "SACM23-VAL-003",
            "Claim \"c1\" has the metaClaim \"x\", which names no element of the document",
        )
        .with_clause("11.10")
        .with_affected([ElementId::from("c1"), ElementId::from("x")])
        .with_location(location),
        Diagnostic::new(
            Code::XMI_MISSING_ID,
            Severity::Info,
            "SACM23-XMI-020",
            "a Claim has no xmi:id; it is read as claim_1",
        ),
        two_descriptions_finding(),
    ]
}

/// The finding of `CLM_132` as a compact record.
const TOO_MANY: &str = r#"{"code":"SACM-MULT-001","severity":"warning","requirement_id":"SACM23-VAL-011","clause":"8.6","affected":["CLM_132"],"location":null,"message":"Claim \"CLM_132\" has 2 descriptions (description:Description[0..1])","template":"SACM-MULT-001/too-many","arguments":{"class":"Claim","id":"CLM_132","count":2,"property":"description","type":"Description","multiplicity":"0..1"}}"#;

/// The arguments of [`TOO_MANY`].
const TOO_MANY_ARGUMENTS: &str = r#""arguments":{"class":"Claim","id":"CLM_132","count":2,"property":"description","type":"Description","multiplicity":"0..1"}"#;

/// A diagnostic record as 0.1.0 wrote it: no template, no arguments.
const BARE: &str = r#"{"code":"SACM-MULT-001","severity":"error","requirement_id":"SACM23-VAL-018","clause":"8.6","affected":["c1"],"location":null,"message":"Claim \"c1\" has no name (name:LangString[1])"}"#;

// --- What is written and read back -------------------------------------------------------

#[test]
fn lib_api_018_diagnostics_json_holds_templates_and_arguments_and_reads_back() {
    let samples = samples();
    let expected = golden("diagnostics.json");
    assert_eq!(
        pretty(&samples),
        expected,
        "{ROW}: serializing the samples does not give the bytes of diagnostics.json"
    );
    for (index, keys) in keys_of_each_value(&expected).iter().enumerate() {
        assert_eq!(
            keys, &KEYS,
            "{ROW}: value {index} of diagnostics.json does not write template and arguments after \
             message"
        );
    }
    let read: Vec<Diagnostic> = from_json("diagnostics.json", &expected);
    assert_eq!(
        read, samples,
        "{ROW}: diagnostics.json reads back to values other than the samples"
    );
    let shown: Vec<(&str, usize)> = read
        .iter()
        .map(|diagnostic| (diagnostic.template(), diagnostic.arguments().len()))
        .collect();
    assert_eq!(
        shown,
        [
            ("SACM-REF-001", 0),
            ("SACM-XMI-004", 0),
            ("SACM-MULT-001/too-many", 6),
        ],
        "{ROW}: the templates and the number of arguments read back"
    );
    // The third comes from a real validate (acceptance 2).
    assert_eq!(
        named(&read[2]),
        [
            ("class", text("Claim")),
            ("id", text("CLM_132")),
            ("count", ArgumentValue::Count(2)),
            ("property", text("description")),
            ("type", text("Description")),
            ("multiplicity", text("0..1")),
        ],
        "{ROW}: the arguments of the finding of CLM_132 read back"
    );
    assert_eq!(
        read[2],
        two_descriptions_finding(),
        "{ROW}: the record of CLM_132's finding is not == to a real one"
    );
    // A record keeps its message as given: it is never rendered again from
    // the template, nor checked against it (amendment 3, "Records").
    let other = TOO_MANY.replacen(
        r#""message":"Claim \"CLM_132\" has 2 descriptions (description:Description[0..1])""#,
        r#""message":"other""#,
        1,
    );
    assert_ne!(
        other, TOO_MANY,
        "{ROW}: setup: the message is not in the twin"
    );
    let read: Diagnostic = from_json("another message", &other);
    assert_eq!(
        (read.message(), read.template(), read.arguments().len()),
        ("other", "SACM-MULT-001/too-many", 6),
        "{ROW}: a record with another message"
    );
    assert!(
        read.to_string().ends_with(": other"),
        "{ROW}: the Display of a record writes its message as given: {read}"
    );
}

#[test]
fn lib_api_018_diagnostics_0_1_0_json_reads_with_the_code_as_template_and_no_arguments() {
    let old = golden("diagnostics-0.1.0.json");
    assert!(
        !old.contains("\"template\"") && !old.contains("\"arguments\""),
        "{ROW}: setup: diagnostics-0.1.0.json is not the 0.1.0 file"
    );
    let read: Vec<Diagnostic> = from_json("diagnostics-0.1.0.json", &old);
    assert_eq!(
        read.len(),
        2,
        "{ROW}: diagnostics-0.1.0.json holds two values"
    );
    for diagnostic in &read {
        assert_eq!(
            diagnostic.template(),
            diagnostic.code().as_str(),
            "{ROW}: a 0.1.0 value reads with its code's text as template"
        );
        assert!(
            diagnostic.arguments().is_empty(),
            "{ROW}: a 0.1.0 value reads with no arguments: {:?}",
            diagnostic.arguments()
        );
    }
    assert_eq!(
        read,
        samples()[..2],
        "{ROW}: the 0.1.0 values read back to other values than the built samples"
    );
    // Written again, they are the first two values of diagnostics.json, as
    // its text writes them: the file up to the end of its second value.
    let current = golden("diagnostics.json");
    let Some(second_end) = current
        .match_indices("\n  },\n  {\n")
        .nth(1)
        .map(|(at, _)| at + "\n  }".len())
    else {
        panic!("{ROW}: setup: diagnostics.json has no third value");
    };
    let first_two = format!("{}\n]\n", &current[..second_end]);
    assert_eq!(
        pretty(&read),
        first_two,
        "{ROW}: the 0.1.0 values do not serialize as the first two values of diagnostics.json"
    );
    let bare: Diagnostic = from_json("a 0.1.0 record", BARE);
    assert_eq!(
        (bare.template(), bare.arguments().len(), bare.message()),
        (
            "SACM-MULT-001",
            0,
            "Claim \"c1\" has no name (name:LangString[1])"
        ),
        "{ROW}: a 0.1.0 record"
    );
}

// --- What is refused ---------------------------------------------------------------------

#[test]
fn lib_api_018_template_and_arguments_are_given_together_and_never_null() {
    let together = "a diagnostic gives template and arguments together, or neither";
    assert_refused::<Diagnostic>(
        "a template without arguments",
        TOO_MANY,
        &format!(",{TOO_MANY_ARGUMENTS}"),
        "",
        together,
    );
    assert_refused::<Diagnostic>(
        "arguments without a template",
        TOO_MANY,
        r#","template":"SACM-MULT-001/too-many""#,
        "",
        together,
    );
    assert_refused::<Diagnostic>(
        "a null template",
        TOO_MANY,
        r#""template":"SACM-MULT-001/too-many""#,
        r#""template":null"#,
        "invalid type: null",
    );
    assert_refused::<Diagnostic>(
        "null arguments",
        TOO_MANY,
        TOO_MANY_ARGUMENTS,
        r#""arguments":null"#,
        "invalid type: null",
    );
    assert_refused::<Diagnostic>(
        "a null template and null arguments",
        TOO_MANY,
        &format!(r#""template":"SACM-MULT-001/too-many",{TOO_MANY_ARGUMENTS}"#),
        r#""template":null,"arguments":null"#,
        "invalid type: null",
    );
    // The code's text as template takes `{}`, never `null`.
    let bare = BARE.replacen(
        r#""message":"Claim \"c1\" has no name (name:LangString[1])""#,
        r#""message":"Claim \"c1\" has no name (name:LangString[1])","template":"SACM-MULT-001","arguments":{}"#,
        1,
    );
    assert_refused::<Diagnostic>(
        "null arguments with the code's text",
        &bare,
        r#""arguments":{}"#,
        r#""arguments":null"#,
        "invalid type: null",
    );
    assert_refused::<Diagnostic>(
        "the code's text without arguments",
        &bare,
        r#","arguments":{}"#,
        "",
        together,
    );
    assert_refused::<Diagnostic>(
        "an unknown key",
        TOO_MANY,
        TOO_MANY_ARGUMENTS,
        &format!("{TOO_MANY_ARGUMENTS},\"details\":null"),
        "",
    );
}

#[test]
fn lib_api_018_an_unknown_template_a_refusal_s_or_another_code_s_is_refused() {
    assert_refused::<Diagnostic>(
        "an unknown template",
        TOO_MANY,
        r#""template":"SACM-MULT-001/too-many""#,
        r#""template":"SACM-MULT-001/no-such-form""#,
        "the template \"SACM-MULT-001/no-such-form\" is unknown",
    );
    // Another code's text is no template at all.
    assert_refused::<Diagnostic>(
        "another code's text",
        TOO_MANY,
        r#""template":"SACM-MULT-001/too-many""#,
        r#""template":"SACM-REF-001""#,
        "the template \"SACM-REF-001\" is unknown",
    );
    // SACM-MULT-001/repeated-value is the template of ReadError::RepeatedValue.
    assert_refused::<Diagnostic>(
        "a refusal's template",
        TOO_MANY,
        &format!(r#""template":"SACM-MULT-001/too-many",{TOO_MANY_ARGUMENTS}"#),
        r#""template":"SACM-MULT-001/repeated-value","arguments":{"property":"name","element":"c1","count":2}"#,
        "the template \"SACM-MULT-001/repeated-value\" is a refusal's, not a diagnostic's",
    );
    assert_refused::<Diagnostic>(
        "another code's template",
        TOO_MANY,
        &format!(r#""template":"SACM-MULT-001/too-many",{TOO_MANY_ARGUMENTS}"#),
        r#""template":"SACM-REF-001/dangling","arguments":{"class":"Claim","id":"c1","end":"citedElement","target":"nope","type":"SACMElement"}"#,
        "the code SACM-MULT-001 is not the code of the template \"SACM-REF-001/dangling\"",
    );
    assert_refused::<Diagnostic>(
        "another code's refusal template",
        TOO_MANY,
        &format!(r#""template":"SACM-MULT-001/too-many",{TOO_MANY_ARGUMENTS}"#),
        r#""template":"SACM-XMI-014/xmi-root","arguments":{"element":"xmi:Extension","row":"SACM23-XMI-019"}"#,
        "the template \"SACM-XMI-014/xmi-root\" is a refusal's, not a diagnostic's",
    );
}

#[test]
fn lib_api_018_the_code_s_text_takes_no_arguments() {
    let bare = BARE.replacen(
        r#""message":"Claim \"c1\" has no name (name:LangString[1])""#,
        r#""message":"Claim \"c1\" has no name (name:LangString[1])","template":"SACM-MULT-001","arguments":{}"#,
        1,
    );
    let read: Diagnostic = from_json("the code's text and no arguments", &bare);
    assert_eq!(
        (read.template(), read.arguments().len()),
        ("SACM-MULT-001", 0),
        "{ROW}: the code's text as template, with no arguments"
    );
    assert_eq!(
        read,
        from_json::<Diagnostic>("a 0.1.0 record", BARE),
        "{ROW}: the code's text as template reads as a record that gives neither"
    );
    assert_refused::<Diagnostic>(
        "the code's text with an argument",
        &bare,
        r#""arguments":{}"#,
        r#""arguments":{"class":"Claim"}"#,
        "the template \"SACM-MULT-001\" takes 0 arguments, not 1",
    );
    assert_refused::<Diagnostic>(
        "the code's text with the arguments of a template of its code",
        &bare,
        r#""arguments":{}"#,
        r#""arguments":{"class":"Claim","id":"c1","property":"name","type":"LangString","multiplicity":"1"}"#,
        "the template \"SACM-MULT-001\" takes 0 arguments, not 5",
    );
}

#[test]
fn lib_api_018_arguments_that_do_not_match_the_template_are_refused() {
    let template = "\"SACM-MULT-001/too-many\"";
    for (what, to, reason) in [
        (
            "a missing argument",
            r#""arguments":{"class":"Claim","id":"CLM_132","count":2,"property":"description","type":"Description"}"#,
            format!("the template {template} takes 6 arguments, not 5"),
        ),
        (
            "an extra argument",
            r#""arguments":{"class":"Claim","id":"CLM_132","count":2,"property":"description","type":"Description","multiplicity":"0..1","extra":"x"}"#,
            format!("the template {template} takes 6 arguments, not 7"),
        ),
        (
            "arguments out of order",
            r#""arguments":{"id":"CLM_132","class":"Claim","count":2,"property":"description","type":"Description","multiplicity":"0..1"}"#,
            format!("the argument \"id\" is not \"class\", the next argument of {template}"),
        ),
        (
            "a Count given as text",
            r#""arguments":{"class":"Claim","id":"CLM_132","count":"2","property":"description","type":"Description","multiplicity":"0..1"}"#,
            "the argument \"count\" is not of the kind Count".to_owned(),
        ),
        (
            "a Text given a number",
            r#""arguments":{"class":1,"id":"CLM_132","count":2,"property":"description","type":"Description","multiplicity":"0..1"}"#,
            "the argument \"class\" is not of the kind Text".to_owned(),
        ),
        (
            "a Text given null",
            r#""arguments":{"class":null,"id":"CLM_132","count":2,"property":"description","type":"Description","multiplicity":"0..1"}"#,
            "a string or a non-negative integer".to_owned(),
        ),
        (
            "a negative Count",
            r#""arguments":{"class":"Claim","id":"CLM_132","count":-1,"property":"description","type":"Description","multiplicity":"0..1"}"#,
            "a string or a non-negative integer".to_owned(),
        ),
        (
            "arguments as an array",
            r#""arguments":["Claim","CLM_132",2,"description","Description","0..1"]"#,
            "an object of message arguments".to_owned(),
        ),
    ] {
        assert_refused::<Diagnostic>(what, TOO_MANY, TOO_MANY_ARGUMENTS, to, &reason);
    }
}

#[test]
fn lib_api_018_a_refusal_record_naming_a_diagnostic_template_is_refused() {
    // An unread root and its kept form until slice 0067 read the last
    // package kinds (ADR 0035 amendment 1).
    let unsupported = r#"{"type":"Unsupported","code":"SACM-XMI-014","template":"SACM-XMI-014/nested-xmi-element","arguments":{"element":"xmi:Documentation","container":"xmi:XMI","row":"SACM23-XMI-019"},"location":{"line":1,"column":1},"reason":"the XMI element \"xmi:Documentation\" in \"xmi:XMI\" is not read in this release (a limit of this version; SACM23-XMI-019 reads it)"}"#;
    assert_refused::<ReadError>(
        "an Unsupported record naming a kept form",
        unsupported,
        r#""template":"SACM-XMI-014/nested-xmi-element","arguments":{"element":"xmi:Documentation","container":"xmi:XMI","row":"SACM23-XMI-019"}"#,
        r#""template":"SACM-XMI-014/nested-xmi-element-kept-by-document","arguments":{"element":"xmi:Documentation","container":"xmi:XMI","row":"SACM23-XMI-019"}"#,
        "the template \"SACM-XMI-014/nested-xmi-element-kept-by-document\" is a diagnostic's, not a refusal's",
    );
    let legacy = r#"{"type":"LegacyShape","code":"SACM-XMI-012","template":"SACM-XMI-012/name-attribute","arguments":{"class":"ArgumentPackage","id":"arguments"},"element":"arguments","shape":"the name of the ArgumentPackage \"arguments\" is given as the attribute \"name\"","location":{"line":1,"column":142}}"#;
    assert_refused::<ReadError>(
        "a LegacyShape record naming its -read form",
        legacy,
        r#""template":"SACM-XMI-012/name-attribute""#,
        r#""template":"SACM-XMI-012/name-attribute-read""#,
        "the template \"SACM-XMI-012/name-attribute-read\" is a diagnostic's, not a refusal's",
    );
    let invalid_id = r#"{"type":"InvalidId","code":"SACM-ID-002","template":"SACM-ID-002/read-id-not-an-ncname","arguments":{"id":""},"id":"","location":{"line":2,"column":3}}"#;
    assert_refused::<ReadError>(
        "an InvalidId record naming a validation template",
        invalid_id,
        r#""template":"SACM-ID-002/read-id-not-an-ncname","arguments":{"id":""}"#,
        r#""template":"SACM-ID-002/not-an-ncname","arguments":{"class":"Claim","id":"1c"}"#,
        "the template \"SACM-ID-002/not-an-ncname\" is a diagnostic's, not a refusal's",
    );
    assert_refused::<ReadError>(
        "an InvalidId record naming another refusal's template",
        invalid_id,
        r#""template":"SACM-ID-002/read-id-not-an-ncname","arguments":{"id":""}"#,
        r#""template":"SACM-MULT-001/repeated-value","arguments":{"property":"name","element":"c1","count":2}"#,
        "the template \"SACM-MULT-001/repeated-value\" is another refusal's",
    );
}

#[test]
fn lib_api_018_the_invalid_write_error_s_diagnostics_carry_their_templates() {
    let input = format!(
        "<sacm:ArgumentPackage{NS} xmi:id=\"p\"><name content=\"p\"/>\
         <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\" isCitation=\"true\" \
         citedElement=\"nope\"><name content=\"c1\"/></argumentElement></sacm:ArgumentPackage>"
    );
    let document = match xmi::read(input.as_bytes(), &ReadOptions::default()) {
        Ok(output) => output.into_document(),
        Err(error) => panic!("{ROW}: setup: {input} does not read: {error}"),
    };
    let error = match xmi::write(&document, &WriteOptions::default()) {
        Ok(text) => panic!("{ROW}: setup: a strict write writes {text}"),
        Err(error) => error,
    };
    assert!(
        matches!(error, WriteError::Invalid { .. }),
        "{ROW}: setup: the refusal is {error:?}"
    );
    let expected = vec![
        ("class", text("Claim")),
        ("id", text("c1")),
        ("end", text("citedElement")),
        ("target", text("nope")),
        ("type", text("SACMElement")),
    ];
    let [diagnostic] = error.diagnostics() else {
        panic!("{ROW}: the refusal has {:?}", error.diagnostics());
    };
    assert_eq!(
        (diagnostic.template(), named(diagnostic)),
        ("SACM-REF-001/dangling", expected.clone()),
        "{ROW}: the template and arguments of the Invalid refusal's diagnostic"
    );
    // The third value of write-errors.json is this refusal.
    let values: Vec<serde_json::Value> =
        from_json("write-errors.json", &golden("write-errors.json"));
    let Some(invalid) = values.get(2) else {
        panic!("{ROW}: write-errors.json has no third value");
    };
    let written: serde_json::Value = match serde_json::to_value(&error) {
        Ok(value) => value,
        Err(error) => panic!("{ROW}: the refusal does not serialize: {error}"),
    };
    assert_eq!(
        &written, invalid,
        "{ROW}: the Invalid refusal is not the third value of write-errors.json"
    );
    // Read from the file's text, whose keys keep their written order (a
    // `serde_json::Value` sorts them).
    let all: Vec<WriteError> = from_json("write-errors.json", &golden("write-errors.json"));
    let Some(read) = all.get(2) else {
        panic!("{ROW}: write-errors.json has no third value");
    };
    let [diagnostic] = read.diagnostics() else {
        panic!("{ROW}: the refusal read back has {:?}", read.diagnostics());
    };
    assert_eq!(
        (diagnostic.template(), named(diagnostic)),
        ("SACM-REF-001/dangling", expected),
        "{ROW}: the Invalid value read back carries its diagnostic's template and arguments"
    );
}
