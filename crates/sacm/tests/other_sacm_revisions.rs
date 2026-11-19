//! Roots in another SACM revision's namespace or a SACM-like one
//! (LIB-COMPAT-004; D-VERSION; D-NS-PIN; M6 decision Q5; ADR 0041 D4, D5).
//!
//! XMI 2.5.1 §7.8.1 (p. 16): "the schema generator must use the namespace URI
//! specified by the `Package::URI` property on the package representing the
//! metamodel"; ptc/22-03-13 sets none (spec defect D7), so no SACM revision
//! has a standard namespace, and files use many. A tolerant `xmi::read`:
//! - reads a package-kind root in a recognised older revision's namespace as
//!   SACM 2.3, with one SACM-XMI-008 Warning per root, and an Event's
//!   `occurence` (the 2.0 to 2.2 spelling) as its `date`, with one more;
//! - reads one in any other namespace whose URI contains `sacm` in any letter
//!   case with one SACM-XMI-002 Warning per root;
//! - refuses a SACM 2.4 root (LIB-COMPAT-018), as a strict read does, and a
//!   root in no namespace or a non-SACM one as `InvalidRoot` (SACM-XMI-001).
//!
//! A strict read refuses the older revisions and the SACM-like namespaces as
//! `Unsupported`, labelled `(strict mode never reads it; LIB-COMPAT-004 reads
//! it in tolerant mode)`. In a root, the namespaces of its family count as
//! SACM; every other is foreign (SACM-XMI-003). `xmi::write` writes SACM 2.3
//! in the pinned namespace.
//!
//! `R` is the design brief's root, [`r_in`]; Goldens 1 and 2 are
//! [`GOLDEN_1`] and [`GOLDEN_2`], with their outputs. Codes are compared by
//! their text, so the file needs no API slice 0072 adds.

use sacm::diagnostic::{Diagnostic, Severity, SourceLocation};
use sacm::model::{Document, ElementId, ElementKind, SacmElement};
use sacm::xmi::{self, Mode, ReadError, ReadOptions, ReadOutput, WriteOptions};

const ROW: &str = "LIB-COMPAT-004";

const SACM_NS: &str = "http://www.omg.org/spec/SACM/20220301";
const XMI_NS: &str = "http://www.omg.org/spec/XMI/20131001";

const MODES: [Mode; 2] = [Mode::Strict, Mode::Tolerant];

/// The label of a strict refusal of a root a tolerant read reads.
const TOLERATED: &str = "(strict mode never reads it; LIB-COMPAT-004 reads it in tolerant mode)";

/// The recognised older revisions: each base alone, and four with a
/// package, with the revision SACM-XMI-008 names.
const OLDER: [(&str, &str); 14] = [
    ("http://www.omg.org/spec/SACM/20170843", "2.0"),
    ("http://www.omg.org/spec/SACM/20190235", "2.1"),
    ("http://www.omg.org/spec/SACM/20210218", "2.2"),
    ("http://www.omg.org/spec/SACM/2.0", "2.0"),
    ("http://www.omg.org/spec/SACM/2.1", "2.1"),
    ("http://www.omg.org/spec/SACM/2.2", "2.2"),
    ("http://omg.sacm/2.0", "2.0"),
    ("http://omg.sacm/2.1", "2.1"),
    ("http://omg.sacm/2.2", "2.2"),
    ("http://SACM_ACEditor/sacm/2.1", "2.1"),
    ("http://www.omg.org/spec/SACM/2.2/Argumentation", "2.2"),
    ("http://www.omg.org/spec/SACM/2.0/argumentation", "2.0"),
    ("http://omg.sacm/2.2/argumentation", "2.2"),
    ("http://omg.sacm/2.0/assurancecase", "2.0"),
];

/// SACM-like namespaces: neither pinned nor a recognised revision, but
/// containing `sacm` in some letter case.
const SACM_LIKE: [&str; 7] = [
    "http://www.omg.org/spec/SACM/2.3",
    "http://www.omg.org/spec/SACM/2.3/Argumentation",
    "urn:example:SaCm",
    "http://example.com/mysacm/v1",
    "http://www.omg.org/spec/SACM/20220301/Argumentation",
    "https://www.omg.org/spec/SACM/20220301",
    "http://www.omg.org/spec/SACM/2.2/",
];

/// The SACM 2.4 namespaces, alone or with a package.
const SACM_24: [&str; 4] = [
    "http://www.omg.org/spec/SACM/20260504",
    "http://www.omg.org/spec/SACM/20260504/Argumentation",
    "http://www.omg.org/spec/SACM/2.4",
    "http://www.omg.org/spec/SACM/2.4/Argumentation",
];

/// Golden 1, input: an ArgumentPackage in SACM 2.2's
/// `…/2.2/Argumentation`, with the text spelling `argumentationElement`.
const GOLDEN_1: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<sacm:ArgumentPackage xmlns:sacm=\"http://www.omg.org/spec/SACM/2.2/Argumentation\" xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" xmi:id=\"p\">
  <name content=\"Older\"/>
  <argumentationElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\">
    <name content=\"C1\"/>
  </argumentationElement>
</sacm:ArgumentPackage>
";

/// Golden 1, output, in both write modes.
const GOLDEN_1_OUTPUT: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<sacm:ArgumentPackage xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\" xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" xmi:id=\"p\">
  <name content=\"Older\" />
  <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\">
    <name content=\"C1\" />
  </argumentElement>
</sacm:ArgumentPackage>
";

/// Golden 2, input: an ArtifactPackage in SACM 2.2's `…/20210218`, with an
/// Event's `occurence`.
const GOLDEN_2: &str = "<sacm:ArtifactPackage xmlns:sacm=\"http://www.omg.org/spec/SACM/20210218\" xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" xmi:id=\"arts\">
  <artifactElement xmi:type=\"sacm:Event\" xmi:id=\"e1\" occurence=\"2021-02-18\"/>
</sacm:ArtifactPackage>";

/// Golden 2, output, in both write modes.
const GOLDEN_2_OUTPUT: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<sacm:ArtifactPackage xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\" xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" xmi:id=\"arts\">
  <artifactElement xmi:type=\"sacm:Event\" xmi:id=\"e1\" date=\"2021-02-18\" />
</sacm:ArtifactPackage>
";

/// `R` of the design brief in the namespace `uri`: a root ArgumentPackage
/// `a`, bound to `p`, holding the Claim `c`.
fn r_in(uri: &str) -> String {
    format!(
        "<p:ArgumentPackage xmlns:p=\"{uri}\" xmlns:xmi=\"{XMI_NS}\" xmi:id=\"a\">\
         <argumentElement xmi:type=\"p:Claim\" xmi:id=\"c\"/></p:ArgumentPackage>"
    )
}

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

/// The arguments of `found` as `(name, text)`; a Count shows as its number.
fn arguments_of(found: &[sacm::diagnostic::Argument]) -> Vec<(String, String)> {
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

/// `pairs` as owned strings.
fn owned(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
        .collect()
}

/// A diagnostic as `(code, severity, row, location, affected, template)`.
type Seen = (
    String,
    Severity,
    String,
    Option<SourceLocation>,
    Vec<ElementId>,
    &'static str,
);

fn seen(found: &Diagnostic) -> Seen {
    (
        found.code().as_str().to_owned(),
        found.severity(),
        found.requirement_id().to_owned(),
        found.location(),
        found.affected().to_vec(),
        found.template(),
    )
}

#[test]
fn lib_compat_004_each_older_revision_reads_as_sacm_2_3_with_one_sacm_xmi_008_per_root() {
    let pinned = read_ok("R in the pinned namespace", &r_in(SACM_NS), Mode::Tolerant);
    assert!(
        pinned.diagnostics().is_empty(),
        "{ROW}: setup: R in the pinned namespace reports {:?}",
        pinned.diagnostics()
    );
    let pinned = pinned.into_document();
    for (uri, revision) in OLDER {
        let input = r_in(uri);
        let output = read_ok(uri, &input, Mode::Tolerant);
        let found: Vec<Seen> = output.diagnostics().iter().map(seen).collect();
        assert_eq!(
            found,
            [(
                "SACM-XMI-008".to_owned(),
                Severity::Warning,
                ROW.to_owned(),
                Some(loc(1, 1)),
                vec![ElementId::from("a")],
                "SACM-XMI-008/other-revision",
            )],
            "{ROW}: {uri}: not one SACM-XMI-008 Warning at the root"
        );
        let Some(warning) = output.diagnostics().first() else {
            panic!("{ROW}: {uri}: no diagnostic");
        };
        assert_eq!(
            arguments_of(warning.arguments()),
            owned(&[
                ("class", "ArgumentPackage"),
                ("id", "a"),
                ("namespace", uri),
                ("revision", revision),
            ]),
            "{ROW}: {uri}: the arguments of SACM-XMI-008"
        );
        assert_eq!(
            warning.message(),
            format!(
                "the ArgumentPackage \"a\" is in the namespace {uri:?} of SACM {revision}, so it \
                 is read as SACM 2.3: content SACM 2.3 does not have is kept as preserved \
                 content, and a write writes SACM 2.3"
            ),
            "{ROW}: {uri}: the message of SACM-XMI-008"
        );
        assert!(
            output.into_document() == pinned,
            "{ROW}: {uri}: R does not read as it does in the pinned namespace"
        );
    }
}

#[test]
fn lib_compat_004_a_strict_read_refuses_an_older_revision_with_sacm_xmi_008() {
    for (uri, revision) in OLDER {
        let error = refusal(uri, &r_in(uri), Mode::Strict);
        assert!(
            matches!(error, ReadError::Unsupported { .. }),
            "{ROW}: {uri}: a strict read does not refuse it as Unsupported: {error:?}"
        );
        assert_eq!(
            (error.code().as_str(), error.template(), error.location()),
            (
                "SACM-XMI-008",
                "SACM-XMI-008/other-revision-root",
                Some(loc(1, 1))
            ),
            "{ROW}: {uri}: the code, template and location"
        );
        assert_eq!(
            arguments_of(error.arguments()),
            owned(&[
                ("element", "p:ArgumentPackage"),
                ("namespace", uri),
                ("revision", revision),
                ("row", ROW),
            ]),
            "{ROW}: {uri}: the arguments"
        );
        assert!(
            error.to_string().ends_with(TOLERATED),
            "{ROW}: {uri}: the Display does not end with the label: {error}"
        );
    }
    let uri = "http://www.omg.org/spec/SACM/2.2/Argumentation";
    assert_eq!(
        refusal(uri, &r_in(uri), Mode::Strict).to_string(),
        "the reader cannot represent the input at line 1, column 1: the root element \
         \"p:ArgumentPackage\" is in the namespace \"http://www.omg.org/spec/SACM/2.2/Argumentation\" \
         of SACM 2.2, not the SACM 2.3 namespace \"http://www.omg.org/spec/SACM/20220301\" \
         (strict mode never reads it; LIB-COMPAT-004 reads it in tolerant mode)",
        "{ROW}: the Display of the strict refusal of /2.2/Argumentation"
    );
}

#[test]
fn lib_compat_004_a_sacm_like_namespace_reads_with_sacm_xmi_002_in_tolerant_mode_only() {
    let pinned =
        read_ok("R in the pinned namespace", &r_in(SACM_NS), Mode::Tolerant).into_document();
    for uri in SACM_LIKE {
        let input = r_in(uri);
        let output = read_ok(uri, &input, Mode::Tolerant);
        let found: Vec<Seen> = output.diagnostics().iter().map(seen).collect();
        assert_eq!(
            found,
            [(
                "SACM-XMI-002".to_owned(),
                Severity::Warning,
                ROW.to_owned(),
                Some(loc(1, 1)),
                vec![ElementId::from("a")],
                "SACM-XMI-002/sacm-like",
            )],
            "{ROW}: {uri}: not one SACM-XMI-002 Warning at the root"
        );
        let Some(warning) = output.diagnostics().first() else {
            panic!("{ROW}: {uri}: no diagnostic");
        };
        assert_eq!(
            arguments_of(warning.arguments()),
            owned(&[
                ("class", "ArgumentPackage"),
                ("id", "a"),
                ("namespace", uri)
            ]),
            "{ROW}: {uri}: the arguments of SACM-XMI-002"
        );
        assert!(
            output.into_document() == pinned,
            "{ROW}: {uri}: R does not read as it does in the pinned namespace"
        );
        let error = refusal(uri, &input, Mode::Strict);
        assert!(
            matches!(error, ReadError::Unsupported { .. }),
            "{ROW}: {uri}: a strict read does not refuse it as Unsupported: {error:?}"
        );
        assert_eq!(
            (error.code().as_str(), error.template(), error.location()),
            (
                "SACM-XMI-002",
                "SACM-XMI-002/other-namespace",
                Some(loc(1, 1))
            ),
            "{ROW}: {uri}: the strict code, template and location"
        );
        assert!(
            error.to_string().ends_with(TOLERATED),
            "{ROW}: {uri}: the strict Display does not end with the label: {error}"
        );
    }
}

#[test]
fn lib_compat_004_a_root_outside_any_sacm_namespace_is_invalid_root_in_both_modes() {
    let cases = [
        (
            "urn:x",
            r_in("urn:x"),
            "SACM-XMI-001/not-a-sacm-namespace",
            "p:ArgumentPackage",
            Some(
                "the root element \"p:ArgumentPackage\" is in the namespace \"urn:x\", which does \
                 not name SACM at line 1, column 1",
            ),
        ),
        (
            "no namespace",
            format!("<ArgumentPackage xmlns:xmi=\"{XMI_NS}\" xmi:id=\"a\"/>"),
            "SACM-XMI-001/no-namespace",
            "ArgumentPackage",
            Some(
                "the root element \"ArgumentPackage\" is in no namespace, so it is not a SACM \
                 package at line 1, column 1",
            ),
        ),
        (
            "xmlns=\"\"",
            format!("<ArgumentPackage xmlns=\"\" xmlns:xmi=\"{XMI_NS}\" xmi:id=\"a\"/>"),
            "SACM-XMI-001/no-namespace",
            "ArgumentPackage",
            None,
        ),
    ];
    for (what, input, template, name, display) in &cases {
        let strict = refusal(what, input, Mode::Strict);
        let tolerant = refusal(what, input, Mode::Tolerant);
        assert_eq!(
            tolerant, strict,
            "{ROW}: {what}: the modes refuse differently"
        );
        let ReadError::InvalidRoot { element_name, .. } = &strict else {
            panic!("{ROW}: {what}: not InvalidRoot: {strict:?}");
        };
        assert_eq!(
            (
                strict.code().as_str(),
                strict.template(),
                strict.location(),
                element_name.as_str()
            ),
            ("SACM-XMI-001", *template, Some(loc(1, 1)), *name),
            "{ROW}: {what}: the code, template, location and element name"
        );
        if let Some(display) = display {
            assert_eq!(strict.to_string(), *display, "{ROW}: {what}: the Display");
        }
    }
    // In a wrapper, each root is classified: the second, in `urn:x`, is
    // InvalidRoot.
    let wrapper = |first: &str| {
        format!(
            "<xmi:XMI xmlns:xmi=\"{XMI_NS}\">\n  \
             <s:ArgumentPackage xmlns:s=\"{first}\" xmi:id=\"p1\"/>\n  \
             <x:ArgumentPackage xmlns:x=\"urn:x\" xmi:id=\"p2\"/>\n</xmi:XMI>"
        )
    };
    let pinned_first = wrapper(SACM_NS);
    let at = position_of(&pinned_first, "<x:ArgumentPackage");
    for mode in MODES {
        let error = refusal("[pinned, urn:x]", &pinned_first, mode);
        assert!(
            matches!(error, ReadError::InvalidRoot { .. }) && error.location() == Some(at),
            "{ROW}: [pinned, urn:x]: a {mode:?} read is not InvalidRoot at the second root: \
             {error:?}"
        );
    }
    // InvalidRoot takes precedence: a strict read of [2.2, urn:x] is
    // InvalidRoot at the second root, not Unsupported at the first.
    let older_first = wrapper("http://www.omg.org/spec/SACM/2.2");
    let error = refusal("[2.2, urn:x]", &older_first, Mode::Strict);
    assert!(
        matches!(error, ReadError::InvalidRoot { .. })
            && error.location() == Some(position_of(&older_first, "<x:ArgumentPackage")),
        "{ROW}: [2.2, urn:x]: a strict read is not InvalidRoot at the second root: {error:?}"
    );
}

#[test]
fn lib_compat_004_sacm_2_4_is_refused_in_both_modes() {
    for uri in SACM_24 {
        let input = r_in(uri);
        let strict = refusal(uri, &input, Mode::Strict);
        let tolerant = refusal(uri, &input, Mode::Tolerant);
        assert_eq!(
            tolerant, strict,
            "{ROW}: {uri}: the modes refuse differently"
        );
        assert!(
            matches!(strict, ReadError::Unsupported { .. }),
            "{ROW}: {uri}: not Unsupported: {strict:?}"
        );
        assert_eq!(
            (strict.code().as_str(), strict.template(), strict.location()),
            ("SACM-XMI-008", "SACM-XMI-008/sacm-2-4", Some(loc(1, 1))),
            "{ROW}: {uri}: the code, template and location"
        );
        assert_eq!(
            arguments_of(strict.arguments()),
            owned(&[("element", "p:ArgumentPackage"), ("namespace", uri)]),
            "{ROW}: {uri}: the arguments, with no row"
        );
        assert!(
            strict.to_string().ends_with(
                "so it is not read as SACM 2.3 until LIB-COMPAT-018 decides how (a limit of this \
                 version)"
            ),
            "{ROW}: {uri}: the Display does not end with the 2.4 reason: {strict}"
        );
    }
}

#[test]
fn lib_compat_004_one_family_counts_as_one_namespace() {
    let input = format!(
        "<ac:AssuranceCasePackage xmlns:ac=\"http://omg.sacm/2.2/assurancecase\" \
         xmlns:arg=\"http://omg.sacm/2.2/argumentation\" \
         xmlns:old=\"http://www.omg.org/spec/SACM/2.0/Argumentation\" \
         xmlns:sacm=\"{SACM_NS}\" xmlns:xmi=\"{XMI_NS}\" xmi:id=\"case\">\n  \
         <argumentPackage xmi:type=\"arg:ArgumentPackage\" xmi:id=\"ap\">\n    \
         <argumentElement xmi:type=\"arg:Claim\" xmi:id=\"c1\"/>\n    \
         <argumentElement xmi:type=\"old:Claim\" xmi:id=\"c2\"/>\n    \
         <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c3\"/>\n  \
         </argumentPackage>\n</ac:AssuranceCasePackage>"
    );
    let output = read_ok("the family root", &input, Mode::Tolerant);
    let found: Vec<(String, Severity, Option<SourceLocation>)> = output
        .diagnostics()
        .iter()
        .map(|found| {
            (
                found.code().as_str().to_owned(),
                found.severity(),
                found.location(),
            )
        })
        .collect();
    let c2 = position_of(&input, "<argumentElement xmi:type=\"old:Claim\"");
    let c3 = position_of(&input, "<argumentElement xmi:type=\"sacm:Claim\"");
    let mut expected = vec![
        (
            "SACM-XMI-008".to_owned(),
            Severity::Warning,
            Some(loc(1, 1)),
        ),
        ("SACM-XMI-003".to_owned(), Severity::Warning, Some(c2)),
        ("SACM-XMI-003".to_owned(), Severity::Warning, Some(c3)),
    ];
    let mut sorted = found.clone();
    sorted.sort_by_key(|(code, _, location)| {
        (code.clone(), location.map(|at| (at.line(), at.column())))
    });
    expected.sort_by_key(|(code, _, location)| {
        (code.clone(), location.map(|at| (at.line(), at.column())))
    });
    assert_eq!(
        sorted, expected,
        "{ROW}: not one SACM-XMI-008 at the root and one SACM-XMI-003 at each foreign Claim"
    );
    let document = output.into_document();
    assert_eq!(
        document.get("c1").map(SacmElement::kind),
        Some(ElementKind::Claim),
        "{ROW}: arg:Claim, in the root's family, is not read as a Claim"
    );
    assert!(
        document.get("c2").is_none() && document.get("c3").is_none(),
        "{ROW}: old:Claim or sacm:Claim, outside the root's family, is read as an element"
    );
    assert!(
        document
            .get("ap")
            .is_some_and(SacmElement::has_preserved_content),
        "{ROW}: ap does not hold c2 and c3 as preserved content"
    );
    let written = match xmi::write(
        &document,
        &WriteOptions::default().with_mode(Mode::Tolerant),
    ) {
        Ok(written) => written,
        Err(error) => panic!("{ROW}: a tolerant write refuses the family document: {error}"),
    };
    assert!(
        written.contains("xmi:id=\"c2\"") && written.contains("xmi:id=\"c3\""),
        "{ROW}: c2 and c3 are not kept whole: {written}"
    );
    // In a pinned root, arg:Claim is foreign, as today.
    let pinned = format!(
        "<sacm:ArgumentPackage xmlns:sacm=\"{SACM_NS}\" \
         xmlns:arg=\"http://omg.sacm/2.2/argumentation\" xmlns:xmi=\"{XMI_NS}\" xmi:id=\"p\">\
         <argumentElement xmi:type=\"arg:Claim\" xmi:id=\"c1\"/></sacm:ArgumentPackage>"
    );
    let output = read_ok("arg:Claim in a pinned root", &pinned, Mode::Tolerant);
    let codes: Vec<&str> = output
        .diagnostics()
        .iter()
        .map(|found| found.code().as_str())
        .collect();
    assert_eq!(
        codes,
        ["SACM-XMI-003"],
        "{ROW}: arg:Claim in a pinned root is not one SACM-XMI-003"
    );
    assert!(
        output.document().get("c1").is_none(),
        "{ROW}: arg:Claim in a pinned root is read as an element"
    );
}

#[test]
fn lib_compat_004_an_older_events_occurence_reads_as_its_date() {
    let output = read_ok("Golden 2", GOLDEN_2, Mode::Tolerant);
    let found: Vec<Seen> = output.diagnostics().iter().map(seen).collect();
    assert_eq!(
        found,
        [
            (
                "SACM-XMI-008".to_owned(),
                Severity::Warning,
                ROW.to_owned(),
                Some(loc(1, 1)),
                vec![ElementId::from("arts")],
                "SACM-XMI-008/other-revision",
            ),
            (
                "SACM-XMI-008".to_owned(),
                Severity::Warning,
                ROW.to_owned(),
                Some(loc(2, 54)),
                vec![ElementId::from("e1")],
                "SACM-XMI-008/occurence",
            ),
        ],
        "{ROW}: Golden 2 does not give its two SACM-XMI-008 Warnings"
    );
    let Some(occurence) = output.diagnostics().get(1) else {
        panic!("{ROW}: Golden 2: no second diagnostic");
    };
    assert_eq!(
        occurence.message(),
        "the Event \"e1\" gives its date as the attribute \"occurence\", the SACM 2.2 spelling, \
         which is read as its date",
        "{ROW}: the message of the occurence Warning"
    );
    let document = output.into_document();
    assert_eq!(
        document.get("e1").and_then(SacmElement::date),
        Some("2021-02-18"),
        "{ROW}: the Event's occurence is not read as its date"
    );
    assert!(
        !document
            .get("e1")
            .is_some_and(SacmElement::has_preserved_content),
        "{ROW}: the Event keeps occurence as preserved content too"
    );
    // In a pinned or SACM-like root, `occurence` is preserved with
    // SACM-XMI-003, as today.
    for (uri, before) in [
        (SACM_NS, &[][..]),
        ("urn:example:sacm", &["SACM-XMI-002"][..]),
    ] {
        let input = GOLDEN_2.replacen("http://www.omg.org/spec/SACM/20210218", uri, 1);
        let output = read_ok(uri, &input, Mode::Tolerant);
        let codes: Vec<&str> = output
            .diagnostics()
            .iter()
            .map(|found| found.code().as_str())
            .collect();
        let mut expected = before.to_vec();
        expected.push("SACM-XMI-003");
        assert_eq!(
            codes, expected,
            "{ROW}: {uri}: occurence is not preserved with one SACM-XMI-003"
        );
        let document = output.into_document();
        assert!(
            document
                .get("e1")
                .is_some_and(|e1| e1.date().is_none() && e1.has_preserved_content()),
            "{ROW}: {uri}: occurence is read as the date, or not kept"
        );
    }
}

#[test]
fn lib_compat_004_a_write_writes_sacm_2_3_and_reads_back_strictly() {
    for (what, input, expected) in [
        ("Golden 1", GOLDEN_1, GOLDEN_1_OUTPUT),
        ("Golden 2", GOLDEN_2, GOLDEN_2_OUTPUT),
    ] {
        let document: Document = read_ok(what, input, Mode::Tolerant).into_document();
        for mode in MODES {
            let written = match xmi::write(&document, &WriteOptions::default().with_mode(mode)) {
                Ok(written) => written,
                Err(error) => panic!("{ROW}: {what}: a {mode:?} write refuses it: {error}"),
            };
            assert_eq!(
                written, expected,
                "{ROW}: {what}: a {mode:?} write does not write SACM 2.3 in the pinned namespace"
            );
            let back = read_ok(what, &written, Mode::Strict);
            assert!(
                back.diagnostics().is_empty(),
                "{ROW}: {what}: the strict read of the output reports {:?}",
                back.diagnostics()
            );
            assert!(
                back.into_document() == document,
                "{ROW}: {what}: the output does not read back to the tolerant read"
            );
        }
    }
    // Golden 1's diagnostics: SACM-XMI-008 at the root, and SACM23-XMI-009's
    // SACM-XMI-013 for argumentationElement.
    let output = read_ok("Golden 1", GOLDEN_1, Mode::Tolerant);
    let found: Vec<(String, Option<SourceLocation>, Vec<ElementId>)> = output
        .diagnostics()
        .iter()
        .map(|found| {
            (
                found.code().as_str().to_owned(),
                found.location(),
                found.affected().to_vec(),
            )
        })
        .collect();
    assert_eq!(
        found.first(),
        Some(&(
            "SACM-XMI-008".to_owned(),
            Some(loc(2, 1)),
            vec![ElementId::from("p")]
        )),
        "{ROW}: Golden 1 does not give SACM-XMI-008 at its root: {found:?}"
    );
    assert_eq!(
        found
            .iter()
            .map(|(code, location, _)| (code.as_str(), *location))
            .collect::<Vec<_>>(),
        [
            ("SACM-XMI-008", Some(loc(2, 1))),
            ("SACM-XMI-013", Some(loc(4, 3)))
        ],
        "{ROW}: Golden 1's diagnostics"
    );
}

#[test]
fn lib_compat_004_each_wrapped_root_is_classified_on_its_own() {
    let input = format!(
        "<xmi:XMI xmlns:xmi=\"{XMI_NS}\">\n  \
         <s:ArgumentPackage xmlns:s=\"{SACM_NS}\" xmi:id=\"p1\"/>\n  \
         <o:ArgumentPackage xmlns:o=\"http://www.omg.org/spec/SACM/2.2\" xmi:id=\"p2\"/>\n  \
         <x:ArgumentPackage xmlns:x=\"urn:example:sacm\" xmi:id=\"p3\"/>\n</xmi:XMI>"
    );
    let p2 = position_of(&input, "<o:ArgumentPackage");
    let p3 = position_of(&input, "<x:ArgumentPackage");
    let output = read_ok("[pinned, 2.2, urn:example:sacm]", &input, Mode::Tolerant);
    let found: Vec<(String, Option<SourceLocation>, Vec<ElementId>)> = output
        .diagnostics()
        .iter()
        .map(|found| {
            (
                found.code().as_str().to_owned(),
                found.location(),
                found.affected().to_vec(),
            )
        })
        .collect();
    assert_eq!(
        found,
        [
            (
                "SACM-XMI-008".to_owned(),
                Some(p2),
                vec![ElementId::from("p2")]
            ),
            (
                "SACM-XMI-002".to_owned(),
                Some(p3),
                vec![ElementId::from("p3")]
            ),
        ],
        "{ROW}: the tolerant read does not give one Warning per root outside the pinned \
         namespace, at its start tag"
    );
    let roots: Vec<String> = output
        .document()
        .roots()
        .map(|root| root.id().to_string())
        .collect();
    assert_eq!(roots, ["p1", "p2", "p3"], "{ROW}: the three roots");
    let error = refusal("[pinned, 2.2, urn:example:sacm]", &input, Mode::Strict);
    assert!(
        matches!(error, ReadError::Unsupported { .. })
            && error.code().as_str() == "SACM-XMI-008"
            && error.location() == Some(p2),
        "{ROW}: a strict read is not Unsupported with SACM-XMI-008 at p2: {error:?}"
    );
}
