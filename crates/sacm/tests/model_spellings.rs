//! Tests for the model and text spellings a tolerant read maps
//! (SACM23-XMI-009; formal/23-05-08 §12.14 "12.14 `ArtifactAssetRelationship`",
//! §12.9 "date: date[0..1]", §11.2 and §11.4 `argumentationElement`;
//! ptc/22-03-13 `Artifact::ArtifactAssertedRelationship`, `Artifact::Event`
//! `occurece`, `Argumentation::ArgumentPackage` and `ArgumentGroup`
//! `argumentElement`; D-TEXT-MODEL, D-STRICT, D-PRESERVE; spec defects D6 and
//! D17; the maintainer's M3 decision 3; ADR 0034 amendment 2).
//!
//! A tolerant `xmi::read` reads three spellings as the names formal/23-05-08
//! gives, with one SACM-XMI-013 Warning per construct read
//! (`Code::XMI_SPELLING_VARIANT`): the type `ArtifactAssertedRelationship`
//! under a role that admits an `ArtifactAssetRelationship`, an Event's
//! `occurece`, and `argumentationElement` as an ArgumentPackage's role and an
//! `ArgumentGroup`'s member end. A value given under both names is read from the
//! resolved name; the spelled one is the repeat (SACM-MULT-001). A strict read
//! never maps a spelling: it keeps the construct as preserved content of the
//! element it sits in, with one SACM-XMI-013 Error, so a strict write refuses
//! the document. Elsewhere a spelling is ordinary content (SACM-XMI-003), and
//! the C++ library's `occurence` is not read. `xmi::write` writes the resolved
//! names.
//!
//! Expected locations are marked in the input with [`MARK`] (U+2038), which
//! the test removes before reading. Inputs end lines with LF only.

use sacm::diagnostic::{Code, Diagnostic, Severity, SourceLocation};
use sacm::edit::{self, CreateArtifact, CreateClaim, EditError};
use sacm::model::LangString;
use sacm::model::{Document, ElementId, SacmElement};
use sacm::validation;
use sacm::xmi::{self, Mode, ReadError, ReadOptions, ReadOutput};
use sacm::xmi::{WriteError, WriteOptions};

const ROW: &str = "SACM23-XMI-009";

/// Both modes, strict first.
const MODES: [Mode; 2] = [Mode::Strict, Mode::Tolerant];

/// A builder of a test input from the attributes and the content of one
/// element.
type Input = fn(&str, &str) -> String;

/// Marks, in a test input, where a diagnostic is expected. Removed before
/// reading.
const MARK: char = '\u{2038}';

/// The namespace declarations of a bare root: `sacm`, `xmi`, `xsi`, a
/// foreign `v`, and `s`, another prefix bound to the SACM namespace.
const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
    " xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"",
    " xmlns:v=\"urn:v\"",
    " xmlns:s=\"http://www.omg.org/spec/SACM/20220301\"",
);

/// The worked fixture of this row.
const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/sacm23/xmi/model-spellings.sacm.xmi"
);

/// The exact output of a strict write of the tolerant read of [`FIXTURE`].
const GOLDEN: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/sacm23/xmi/golden/model-spellings.golden.sacm.xmi"
);

// --- Inputs ------------------------------------------------------------------------

/// The ArgumentPackage root `p`, holding the Claims `c1`, `c2` and `c3`, then
/// `body`, each on its own line.
fn package(body: &str) -> String {
    format!(
        "<sacm:ArgumentPackage{NS} xmi:id=\"p\">\n  <name content=\"P\"/>\n  \
         <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"><name content=\"C1\"/></argumentElement>\n  \
         <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c2\"><name content=\"C2\"/></argumentElement>\n  \
         <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c3\"><name content=\"C3\"/></argumentElement>\n  \
         {body}\n</sacm:ArgumentPackage>"
    )
}

/// [`package`] holding the `ArgumentGroup` `g` with `attributes`, holding
/// `content` after its name.
fn group(attributes: &str, content: &str) -> String {
    package(&format!(
        "<argumentElement xmi:type=\"sacm:ArgumentGroup\" xmi:id=\"g\"{attributes}>\
         <name content=\"G\"/>{content}</argumentElement>"
    ))
}

/// The ArtifactPackage root `pkg`, holding the Artifacts `a` and `b`, then
/// `body`, each on its own line.
fn artifacts(body: &str) -> String {
    format!(
        "<sacm:ArtifactPackage{NS} xmi:id=\"pkg\">\n  <name content=\"Pkg\"/>\n  \
         <artifactElement xmi:type=\"sacm:Artifact\" xmi:id=\"a\"><name content=\"A\"/></artifactElement>\n  \
         <artifactElement xmi:type=\"sacm:Artifact\" xmi:id=\"b\"><name content=\"B\"/></artifactElement>\n  \
         {body}\n</sacm:ArtifactPackage>"
    )
}

/// The relationship `r` (source `a`, target `b`, holding the Property `rp`),
/// its start tag written as `<artifactElement {type_attributes} ...>`.
fn relationship(type_attributes: &str) -> String {
    format!(
        "<artifactElement {type_attributes} xmi:id=\"r\" source=\"a\" target=\"b\">\
         <name content=\"R\"/><property xmi:id=\"rp\"><name content=\"RP\"/></property>\
         </artifactElement>"
    )
}

/// [`artifacts`] holding the Event `ev` with `attributes`, holding `content`
/// after its name.
fn event(attributes: &str, content: &str) -> String {
    artifacts(&format!(
        "<artifactElement xmi:type=\"sacm:Event\" xmi:id=\"ev\"{attributes}>\
         <name content=\"E\"/>{content}</artifactElement>"
    ))
}

fn loc(line: u32, column: u32) -> SourceLocation {
    let Some(location) = SourceLocation::new(line, column) else {
        panic!("{ROW}: setup: SourceLocation::new({line}, {column}) refused a 1-based position");
    };
    location
}

/// `template` without its marks, and the location of each mark in order:
/// lines end at LF, and columns count characters.
fn marked(template: &str) -> (String, Vec<SourceLocation>) {
    let (mut line, mut column) = (1_u32, 1_u32);
    let mut text = String::with_capacity(template.len());
    let mut marks = Vec::new();
    for c in template.chars() {
        if c == MARK {
            marks.push(loc(line, column));
            continue;
        }
        text.push(c);
        if c == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    (text, marks)
}

/// `template` without its one mark, and the mark's location.
fn one_mark(what: &str, template: &str) -> (String, SourceLocation) {
    let (text, marks) = marked(template);
    let [at] = marks.as_slice() else {
        panic!("{ROW}: setup: {what} has {} marks, not one", marks.len());
    };
    (text, *at)
}

// --- Reading -----------------------------------------------------------------------

fn read(mode: Mode, input: &str) -> Result<ReadOutput, ReadError> {
    xmi::read(input.as_bytes(), &ReadOptions::default().with_mode(mode))
}

/// The read of `input` in `mode`, which the test expects to succeed.
fn read_in(what: &str, mode: Mode, input: &str) -> ReadOutput {
    match read(mode, input) {
        Ok(output) => output,
        Err(error) => panic!("{ROW}: {what}: a {mode:?} read refused it: {error:?} ({error})"),
    }
}

/// The tolerant read of `input`, which the test expects to report nothing.
fn read_clean(what: &str, mode: Mode, input: &str) -> Document {
    let output = read_in(what, mode, input);
    assert!(
        output.diagnostics().is_empty(),
        "{ROW}: {what}: reading in {mode:?} mode reported {:?}",
        output.diagnostics()
    );
    output.into_document()
}

/// `(code, severity, location, affected)` of each diagnostic.
type Reported = Vec<(Code, Severity, Option<SourceLocation>, Vec<ElementId>)>;

fn reported(found: &[Diagnostic]) -> Reported {
    found
        .iter()
        .map(|found| {
            (
                found.code(),
                found.severity(),
                found.location(),
                found.affected().to_vec(),
            )
        })
        .collect()
}

fn one(code: Code, severity: Severity, at: SourceLocation, affected: &str) -> Reported {
    vec![(code, severity, Some(at), vec![ElementId::from(affected)])]
}

fn element<'d>(doc: &'d Document, id: &str) -> SacmElement<'d> {
    let Some(found) = doc.get(id) else {
        panic!("{ROW}: the document has no element {id:?}");
    };
    found
}

fn strs(values: &[ElementId]) -> Vec<&str> {
    values.iter().map(ElementId::as_str).collect()
}

fn children(doc: &Document, id: &str) -> Vec<String> {
    element(doc, id)
        .children()
        .map(|child| child.id().as_str().to_owned())
        .collect()
}

/// Asserts that `template`, without its one mark, reads in both modes and
/// keeps one construct as preserved content of `holder`: exactly one
/// SACM-XMI-003 at the mark, affecting `holder`, a Warning in tolerant mode
/// and an Error in strict mode, and the same document in both. Returns the
/// tolerant document.
fn assert_unknown_at(what: &str, template: &str, holder: &str) -> Document {
    let (input, at) = one_mark(what, template);
    let mut documents = Vec::new();
    for (mode, severity) in [
        (Mode::Strict, Severity::Error),
        (Mode::Tolerant, Severity::Warning),
    ] {
        let output = read_in(what, mode, &input);
        assert_eq!(
            reported(output.diagnostics()),
            one(Code::XMI_UNKNOWN_CONTENT, severity, at, holder),
            "{ROW}: {what}: not one SACM-XMI-003 at the construct affecting {holder:?} in \
             {mode:?} mode"
        );
        let doc = output.into_document();
        assert!(
            element(&doc, holder).has_preserved_content(),
            "{ROW}: {what}: {holder:?} holds no preserved content in {mode:?} mode"
        );
        documents.push(doc);
    }
    assert!(
        documents[0] == documents[1],
        "{ROW}: {what}: the modes read different documents"
    );
    documents.swap_remove(1)
}

// =====================================================================================
// Item 1: the type ArtifactAssertedRelationship
// =====================================================================================

#[test]
fn sacm23_xmi_009_the_type_spelling_reads_as_an_artifact_asset_relationship_with_one_warning() {
    let reference = read_clean(
        "the relationship typed as in the text",
        Mode::Tolerant,
        &artifacts(&relationship("xmi:type=\"sacm:ArtifactAssetRelationship\"")),
    );
    for (what, type_attributes, xsi) in [
        (
            "xmi:type",
            "xmi:type=\"sacm:ArtifactAssertedRelationship\"",
            false,
        ),
        (
            "xsi:type",
            "xsi:type=\"sacm:ArtifactAssertedRelationship\"",
            true,
        ),
        (
            "another prefix bound to the SACM namespace",
            "xmi:type=\"s:ArtifactAssertedRelationship\"",
            false,
        ),
        (
            "the spelling in xmi:type and the class in xsi:type",
            "xmi:type=\"sacm:ArtifactAssertedRelationship\" \
             xsi:type=\"sacm:ArtifactAssetRelationship\"",
            true,
        ),
    ] {
        let (input, at) = one_mark(
            what,
            &artifacts(&format!("{MARK}{}", relationship(type_attributes))),
        );
        let output = read_in(what, Mode::Tolerant, &input);
        let found = reported(output.diagnostics());
        let spelled: Reported = found
            .iter()
            .filter(|found| found.0 == Code::XMI_SPELLING_VARIANT)
            .cloned()
            .collect();
        assert_eq!(
            spelled,
            one(Code::XMI_SPELLING_VARIANT, Severity::Warning, at, "r"),
            "{ROW}: {what}: not one SACM-XMI-013 Warning at the start tag affecting r"
        );
        let others: Vec<(Code, Severity)> = found
            .iter()
            .filter(|found| found.0 != Code::XMI_SPELLING_VARIANT)
            .map(|found| (found.0, found.1))
            .collect();
        let expected_others = if xsi {
            vec![(Code::XMI_XSI_TYPE, Severity::Warning)]
        } else {
            Vec::new()
        };
        assert_eq!(
            others, expected_others,
            "{ROW}: {what}: the other diagnostics"
        );
        let doc = output.into_document();
        assert!(
            doc == reference,
            "{ROW}: {what}: does not read == to the relationship typed ArtifactAssetRelationship"
        );
        let found = element(&doc, "r");
        assert_eq!(
            (
                found.kind(),
                strs(found.sources()),
                strs(found.targets()),
                children(&doc, "r")
            ),
            (
                sacm::model::ElementKind::ArtifactAssetRelationship,
                vec!["a"],
                vec!["b"],
                vec!["rp".to_owned()]
            ),
            "{ROW}: {what}: not read under the rules of ArtifactAssetRelationship"
        );
    }
    // In a nested ArtifactPackage, the Warning affects the relationship too.
    let (input, at) = one_mark(
        "in a nested package",
        &artifacts(&format!(
            "<artifactElement xmi:type=\"sacm:ArtifactPackage\" xmi:id=\"inner\">\
             <name content=\"Inner\"/>{MARK}{}</artifactElement>",
            relationship("xmi:type=\"sacm:ArtifactAssertedRelationship\"")
        )),
    );
    let output = read_in("in a nested package", Mode::Tolerant, &input);
    assert_eq!(
        reported(output.diagnostics()),
        one(Code::XMI_SPELLING_VARIANT, Severity::Warning, at, "r"),
        "{ROW}: in a nested package: not one SACM-XMI-013 Warning"
    );
    assert_eq!(
        children(output.document(), "inner"),
        ["r"],
        "{ROW}: in a nested package: the relationship is not read in it"
    );
}

#[test]
fn sacm23_xmi_009_the_type_spelling_elsewhere_is_unknown_content_or_an_invalid_root() {
    // In another namespace it names no SACM class.
    let doc = assert_unknown_at(
        "v:ArtifactAssertedRelationship",
        &artifacts(&format!(
            "{MARK}{}",
            relationship("xmi:type=\"v:ArtifactAssertedRelationship\"")
        )),
        "pkg",
    );
    assert!(
        doc.get("r").is_none(),
        "{ROW}: v:ArtifactAssertedRelationship was read as an element"
    );
    // Under a role that admits no ArtifactAssetRelationship.
    let doc = assert_unknown_at(
        "the spelling under argumentElement",
        &package(&format!(
            "{MARK}<argumentElement xmi:type=\"sacm:ArtifactAssertedRelationship\" xmi:id=\"r\" \
             source=\"c1\" target=\"c2\"><name content=\"R\"/></argumentElement>"
        )),
        "p",
    );
    assert!(
        doc.get("r").is_none(),
        "{ROW}: the spelling under argumentElement was read as an element"
    );
    // As the document element: no package, so no root.
    let input = format!(
        "<sacm:ArtifactAssertedRelationship{NS} xmi:id=\"r\"><name content=\"R\"/>\
         </sacm:ArtifactAssertedRelationship>"
    );
    for mode in MODES {
        let answer = read(mode, &input);
        assert!(
            matches!(&answer, Err(ReadError::InvalidRoot { .. })),
            "{ROW}: the spelling as the document element in {mode:?} mode is not InvalidRoot: \
             {:?}",
            answer.map(|output| output.diagnostics().to_vec())
        );
    }
}

// =====================================================================================
// Item 2: occurece
// =====================================================================================

#[test]
fn sacm23_xmi_009_occurece_reads_as_the_date_of_an_event_with_one_warning() {
    for (what, attributes, content, reference_attributes, reference_content, date) in [
        (
            "occurece as an attribute",
            format!(" {MARK}occurece=\"2024-03-05T14:00:00+01:00\""),
            String::new(),
            " date=\"2024-03-05T14:00:00+01:00\"",
            "",
            Some("2024-03-05T14:00:00+01:00"),
        ),
        (
            "occurece as a value element",
            String::new(),
            format!("{MARK}<occurece>spring 2024</occurece>"),
            "",
            "<date>spring 2024</date>",
            Some("spring 2024"),
        ),
        (
            "occurece as a nil value element",
            String::new(),
            format!("{MARK}<occurece xsi:nil=\"true\"/>"),
            "",
            "",
            None,
        ),
    ] {
        let (input, at) = one_mark(what, &event(&attributes, &content));
        let output = read_in(what, Mode::Tolerant, &input);
        assert_eq!(
            reported(output.diagnostics()),
            one(Code::XMI_SPELLING_VARIANT, Severity::Warning, at, "ev"),
            "{ROW}: {what}: not one SACM-XMI-013 Warning affecting the Event"
        );
        let doc = output.into_document();
        assert_eq!(
            (
                element(&doc, "ev").date(),
                element(&doc, "ev").has_preserved_content()
            ),
            (date, false),
            "{ROW}: {what}: the date, exactly as written, and nothing preserved"
        );
        let reference = read_clean(
            what,
            Mode::Tolerant,
            &event(reference_attributes, reference_content),
        );
        assert!(
            doc == reference,
            "{ROW}: {what}: does not read == to the Event with date"
        );
    }
    // As an attribute and an element: the attribute is read, and the element
    // is the repeat.
    let what = "occurece as an attribute and an element";
    let (input, marks) = marked(&event(
        &format!(" {MARK}occurece=\"2024-03-05\""),
        &format!("{MARK}<occurece>2024-03-06</occurece>"),
    ));
    let output = read_in(what, Mode::Tolerant, &input);
    assert_eq!(
        reported(output.diagnostics()),
        [
            (
                Code::XMI_SPELLING_VARIANT,
                Severity::Warning,
                Some(marks[0]),
                vec![ElementId::from("ev")]
            ),
            (
                Code::MULTIPLICITY_VIOLATION,
                Severity::Warning,
                Some(marks[1]),
                vec![ElementId::from("ev")]
            ),
        ],
        "{ROW}: {what}: not a SACM-XMI-013 at the attribute and a SACM-MULT-001 at the element"
    );
    let doc = output.into_document();
    assert_eq!(
        (
            element(&doc, "ev").date(),
            element(&doc, "ev").has_preserved_content()
        ),
        (Some("2024-03-05"), true),
        "{ROW}: {what}: the attribute is not read, or the element not kept"
    );
}

#[test]
fn sacm23_xmi_009_a_date_beside_occurece_is_read_and_occurece_is_the_repeat() {
    for (what, attributes, content) in [
        (
            "date then occurece, as attributes",
            format!(" date=\"2024-03-06\" {MARK}occurece=\"2024-03-05\""),
            String::new(),
        ),
        (
            "occurece then date, as attributes",
            format!(" {MARK}occurece=\"2024-03-05\" date=\"2024-03-06\""),
            String::new(),
        ),
        (
            "a date attribute and an occurece element",
            " date=\"2024-03-06\"".to_owned(),
            format!("{MARK}<occurece>2024-03-05</occurece>"),
        ),
        (
            "an occurece attribute and a date element",
            format!(" {MARK}occurece=\"2024-03-05\""),
            "<date>2024-03-06</date>".to_owned(),
        ),
        (
            "an occurece element before a date element",
            String::new(),
            format!("{MARK}<occurece>2024-03-05</occurece><date>2024-03-06</date>"),
        ),
        (
            "a date element before an occurece element",
            String::new(),
            format!("<date>2024-03-06</date>{MARK}<occurece>2024-03-05</occurece>"),
        ),
    ] {
        let (input, at) = one_mark(what, &event(&attributes, &content));
        let output = read_in(what, Mode::Tolerant, &input);
        assert_eq!(
            reported(output.diagnostics()),
            one(Code::MULTIPLICITY_VIOLATION, Severity::Warning, at, "ev"),
            "{ROW}: {what}: not one SACM-MULT-001 Warning at occurece and no SACM-XMI-013"
        );
        let doc = output.into_document();
        assert_eq!(
            (
                element(&doc, "ev").date(),
                element(&doc, "ev").has_preserved_content()
            ),
            (Some("2024-03-06"), true),
            "{ROW}: {what}: the date is not read from date, or occurece is not kept"
        );
    }
}

#[test]
fn sacm23_xmi_009_occurece_elsewhere_and_occurence_are_unknown_content() {
    // occurece on an Artifact, which has its own date (§12.7).
    for (what, attributes, content) in [
        (
            "occurece on an Artifact, as an attribute",
            format!(" {MARK}occurece=\"2024-03-05\""),
            String::new(),
        ),
        (
            "occurece on an Artifact, as an element",
            String::new(),
            format!("{MARK}<occurece>2024-03-05</occurece>"),
        ),
    ] {
        let doc = assert_unknown_at(
            what,
            &artifacts(&format!(
                "<artifactElement xmi:type=\"sacm:Artifact\" xmi:id=\"x\"{attributes}>\
                 <name content=\"X\"/>{content}</artifactElement>"
            )),
            "x",
        );
        assert_eq!(
            element(&doc, "x").date(),
            None,
            "{ROW}: {what}: read as the Artifact's date"
        );
    }
    // The C++ library's spelling is not read.
    for (what, attributes, content) in [
        (
            "occurence on an Event, as an attribute",
            format!(" {MARK}occurence=\"2024-03-05\""),
            String::new(),
        ),
        (
            "occurence on an Event, as an element",
            String::new(),
            format!("{MARK}<occurence>2024-03-05</occurence>"),
        ),
    ] {
        let doc = assert_unknown_at(what, &event(&attributes, &content), "ev");
        assert_eq!(
            element(&doc, "ev").date(),
            None,
            "{ROW}: {what}: read as the Event's date"
        );
    }
}

// =====================================================================================
// Item 3: argumentationElement
// =====================================================================================

#[test]
fn sacm23_xmi_009_argumentation_element_reads_as_an_argument_package_s_role_in_document_order() {
    let spelled = |role: &str| {
        format!(
            "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c4\"><name content=\"C4\"/>\
             </argumentElement>\n  {MARK}<{role} xmi:type=\"sacm:Claim\" xmi:id=\"c5\">\
             <name content=\"C5\"/><description xmi:id=\"d5\"><content><value content=\"D\"/>\
             </content></description></{role}>\n  \
             <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c6\"><name content=\"C6\"/>\
             </argumentElement>"
        )
    };
    let reference = read_clean(
        "the role as ptc/22-03-13 spells it",
        Mode::Tolerant,
        &package(&spelled("argumentElement").replace(MARK, "")),
    );
    let (input, at) = one_mark(
        "argumentationElement",
        &package(&spelled("argumentationElement")),
    );
    let output = read_in("argumentationElement", Mode::Tolerant, &input);
    assert_eq!(
        reported(output.diagnostics()),
        one(Code::XMI_SPELLING_VARIANT, Severity::Warning, at, "p"),
        "{ROW}: argumentationElement: not one SACM-XMI-013 Warning affecting the package"
    );
    let doc = output.into_document();
    assert_eq!(
        (
            children(&doc, "p"),
            element(&doc, "c5").kind(),
            children(&doc, "c5")
        ),
        (
            ["c1", "c2", "c3", "c4", "c5", "c6"]
                .map(str::to_owned)
                .to_vec(),
            sacm::model::ElementKind::Claim,
            vec!["d5".to_owned()]
        ),
        "{ROW}: argumentationElement: not read as a child in document order"
    );
    assert!(
        doc == reference,
        "{ROW}: argumentationElement: does not read == to argumentElement"
    );
    // In a nested ArgumentPackage and in an AssuranceCasePackage's
    // argumentPackage, the Warning affects that package.
    let (input, at) = one_mark(
        "in a nested package",
        &package(&format!(
            "<argumentElement xmi:type=\"sacm:ArgumentPackage\" xmi:id=\"q\"><name content=\"Q\"/>\
             {MARK}<argumentationElement xmi:type=\"sacm:Claim\" xmi:id=\"c7\"><name content=\"C7\"/>\
             </argumentationElement></argumentElement>"
        )),
    );
    let output = read_in("in a nested package", Mode::Tolerant, &input);
    assert_eq!(
        (
            reported(output.diagnostics()),
            children(output.document(), "q")
        ),
        (
            one(Code::XMI_SPELLING_VARIANT, Severity::Warning, at, "q"),
            vec!["c7".to_owned()]
        ),
        "{ROW}: in a nested package"
    );
    let (input, at) = one_mark(
        "in an AssuranceCasePackage's argumentPackage",
        &format!(
            "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\"><name content=\"Case\"/>\
             <argumentPackage xmi:id=\"ap\"><name content=\"AP\"/>\
             {MARK}<argumentationElement xmi:type=\"sacm:Claim\" xmi:id=\"c8\"><name content=\"C8\"/>\
             </argumentationElement></argumentPackage></sacm:AssuranceCasePackage>"
        ),
    );
    let output = read_in("in an argumentPackage", Mode::Tolerant, &input);
    assert_eq!(
        (
            reported(output.diagnostics()),
            children(output.document(), "ap")
        ),
        (
            one(Code::XMI_SPELLING_VARIANT, Severity::Warning, at, "ap"),
            vec!["c8".to_owned()]
        ),
        "{ROW}: in an AssuranceCasePackage's argumentPackage"
    );
}

#[test]
fn sacm23_xmi_009_argumentation_element_reads_as_an_argument_group_s_members() {
    for (what, attributes, content, members) in [
        (
            "the attribute",
            format!(" {MARK}argumentationElement=\"c1 c2\""),
            String::new(),
            vec!["c1", "c2"],
        ),
        (
            "an xmi:idref element",
            String::new(),
            format!("{MARK}<argumentationElement xmi:idref=\"c1\"/>"),
            vec!["c1"],
        ),
        (
            "two elements, xmi:idref and href",
            String::new(),
            format!(
                "{MARK}<argumentationElement xmi:idref=\"c2\"/>\
                 {MARK}<argumentationElement href=\"#c1\"/>"
            ),
            vec!["c2", "c1"],
        ),
        (
            "elements under both names, in document order",
            String::new(),
            format!(
                "<argumentElement xmi:idref=\"c1\"/>{MARK}<argumentationElement xmi:idref=\"c2\"/>\
                 <argumentElement href=\"#c3\"/>"
            ),
            vec!["c1", "c2", "c3"],
        ),
    ] {
        let (input, marks) = marked(&group(&attributes, &content));
        let output = read_in(what, Mode::Tolerant, &input);
        let expected: Reported = marks
            .iter()
            .map(|at| {
                (
                    Code::XMI_SPELLING_VARIANT,
                    Severity::Warning,
                    Some(*at),
                    vec![ElementId::from("g")],
                )
            })
            .collect();
        assert_eq!(
            reported(output.diagnostics()),
            expected,
            "{ROW}: {what}: not one SACM-XMI-013 Warning per attribute or element, affecting the \
             group"
        );
        let doc = output.into_document();
        assert_eq!(
            (
                strs(element(&doc, "g").group_members()),
                children(&doc, "g")
            ),
            (members.clone(), Vec::<String>::new()),
            "{ROW}: {what}: not read as the members {members:?}"
        );
        let reference = read_clean(
            what,
            Mode::Tolerant,
            &group(&format!(" argumentElement=\"{}\"", members.join(" ")), ""),
        );
        assert!(
            doc == reference,
            "{ROW}: {what}: does not read == to the group with argumentElement"
        );
    }
    // Both names as attributes: the resolved name is read, the spelled one
    // is the repeat.
    let what = "argumentElement and argumentationElement attributes";
    let (input, at) = one_mark(
        what,
        &group(
            &format!(" argumentElement=\"c1\" {MARK}argumentationElement=\"c2\""),
            "",
        ),
    );
    let output = read_in(what, Mode::Tolerant, &input);
    assert_eq!(
        reported(output.diagnostics()),
        one(Code::MULTIPLICITY_VIOLATION, Severity::Warning, at, "g"),
        "{ROW}: {what}: not one SACM-MULT-001 Warning at the spelled attribute"
    );
    let doc = output.into_document();
    assert_eq!(
        (
            strs(element(&doc, "g").group_members()),
            element(&doc, "g").has_preserved_content()
        ),
        (vec!["c1"], true),
        "{ROW}: {what}: not [c1] with the spelled attribute kept"
    );
}

#[test]
fn sacm23_xmi_009_argumentation_element_on_a_kind_without_argument_element_is_unknown_content() {
    let claim = |attributes: &str, content: &str| {
        package(&format!(
            "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"x\"{attributes}>\
             <name content=\"X\"/>{content}</argumentElement>"
        ))
    };
    let artifact_group = |attributes: &str, content: &str| {
        artifacts(&format!(
            "<artifactElement xmi:type=\"sacm:ArtifactGroup\" xmi:id=\"x\"{attributes}>\
             <name content=\"X\"/>{content}</artifactElement>"
        ))
    };
    let case = |attributes: &str, content: &str| {
        format!(
            "<sacm:AssuranceCasePackage{NS} xmi:id=\"x\"{attributes}>\n  <name content=\"X\"/>\
             {content}\n</sacm:AssuranceCasePackage>"
        )
    };
    let holders: [(&str, Input); 3] = [
        ("a Claim", claim),
        ("an ArtifactGroup", artifact_group),
        ("an AssuranceCasePackage", case),
    ];
    for (kind, holder) in holders {
        for (form, attributes, content) in [
            (
                "an attribute",
                format!(" {MARK}argumentationElement=\"a\""),
                String::new(),
            ),
            (
                "a reference element",
                String::new(),
                format!("{MARK}<argumentationElement xmi:idref=\"a\"/>"),
            ),
            (
                "a typed element",
                String::new(),
                format!(
                    "{MARK}<argumentationElement xmi:type=\"sacm:Claim\" xmi:id=\"c9\">\
                     <name content=\"C9\"/></argumentationElement>"
                ),
            ),
        ] {
            let what = format!("argumentationElement as {form} on {kind}");
            let doc = assert_unknown_at(&what, &holder(&attributes, &content), "x");
            assert!(
                doc.get("c9").is_none() && element(&doc, "x").group_members().is_empty(),
                "{ROW}: {what}: read as a child or as members"
            );
        }
    }
}

#[test]
fn sacm23_xmi_009_a_spelled_end_attribute_read_and_an_end_element_are_an_attribute_and_elements() {
    // The spelled attribute, once read as the end, is the end's attribute:
    // an element of the end beside it, under either name, is the repeat of
    // XMI 2.5.1 rule 2h (SACM-MULT-001), kept as preserved content, as it
    // is beside `argumentElement="…"`.
    for element_name in ["argumentationElement", "argumentElement"] {
        let what = format!("argumentationElement=\"c1\" and a <{element_name}> element");
        let (input, marks) = marked(&group(
            &format!(" {MARK}argumentationElement=\"c1\""),
            &format!("{MARK}<{element_name} xmi:idref=\"c2\"/>"),
        ));
        if element_name == "argumentElement" {
            // The resolved name is read first: the spelled attribute is the
            // repeat, and the element is read.
            let output = read_in(&what, Mode::Tolerant, &input);
            assert_eq!(
                reported(output.diagnostics()),
                one(
                    Code::MULTIPLICITY_VIOLATION,
                    Severity::Warning,
                    marks[0],
                    "g"
                ),
                "{ROW}: {what}: not one SACM-MULT-001 Warning at the spelled attribute"
            );
            assert_eq!(
                strs(element(output.document(), "g").group_members()),
                ["c2"],
                "{ROW}: {what}: the element is not read"
            );
            continue;
        }
        let output = read_in(&what, Mode::Tolerant, &input);
        assert_eq!(
            reported(output.diagnostics()),
            [
                (
                    Code::XMI_SPELLING_VARIANT,
                    Severity::Warning,
                    Some(marks[0]),
                    vec![ElementId::from("g")]
                ),
                (
                    Code::MULTIPLICITY_VIOLATION,
                    Severity::Warning,
                    Some(marks[1]),
                    vec![ElementId::from("g")]
                ),
            ],
            "{ROW}: {what}: not a SACM-XMI-013 at the attribute and a SACM-MULT-001 at the \
             element"
        );
        let doc = output.into_document();
        assert_eq!(
            (
                strs(element(&doc, "g").group_members()),
                element(&doc, "g").has_preserved_content()
            ),
            (vec!["c1"], true),
            "{ROW}: {what}: not [c1] with the element kept"
        );
    }
}

#[test]
fn sacm23_xmi_009_a_generated_id_is_none_that_a_spelled_end_names() {
    // A generated id is no id a reference may name (ADR 0026), the tokens of
    // the spelled end included: the Claim without an xmi:id is not given the
    // id the group's dangling member names.
    let input = group(" argumentationElement=\"claim_1\"", "").replacen(
        "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c3\">",
        "<argumentElement xmi:type=\"sacm:Claim\">",
        1,
    );
    let doc = read_in("a Claim without an id", Mode::Tolerant, &input).into_document();
    assert!(
        doc.get("claim_1").is_none(),
        "{ROW}: the Claim without an id was given the id the spelled end names"
    );
    assert_eq!(
        strs(element(&doc, "g").group_members()),
        ["claim_1"],
        "{ROW}: the spelled end is not read"
    );
    assert!(
        validation::validate(&doc)
            .iter()
            .any(|found| found.code() == Code::REF_DANGLING),
        "{ROW}: the member does not dangle"
    );
}

// =====================================================================================
// Item 4: a strict read keeps each spelling
// =====================================================================================

/// One construct of items 1 to 3: what it is, the input with one mark at
/// the construct, the element it sits in, and what the tolerant write must
/// write back.
struct Spelled {
    what: &'static str,
    template: String,
    holder: &'static str,
    written: &'static str,
}

fn spelled_constructs() -> Vec<Spelled> {
    vec![
        Spelled {
            what: "the type ArtifactAssertedRelationship",
            template: artifacts(&format!(
                "{MARK}{}",
                relationship("xmi:type=\"sacm:ArtifactAssertedRelationship\"")
            )),
            holder: "pkg",
            written: "xmi:type=\"sacm:ArtifactAssertedRelationship\"",
        },
        Spelled {
            what: "occurece as an attribute",
            template: event(&format!(" {MARK}occurece=\"2024-03-05\""), ""),
            holder: "ev",
            written: "occurece=\"2024-03-05\"",
        },
        Spelled {
            what: "occurece as an element",
            template: event("", &format!("{MARK}<occurece>2024-03-05</occurece>")),
            holder: "ev",
            written: ">2024-03-05</occurece>",
        },
        Spelled {
            what: "occurece as a nil element",
            template: event("", &format!("{MARK}<occurece xsi:nil=\"true\"/>")),
            holder: "ev",
            written: "xsi:nil=\"true\" />",
        },
        Spelled {
            what: "argumentationElement as a role",
            template: package(&format!(
                "{MARK}<argumentationElement xmi:type=\"sacm:Claim\" xmi:id=\"c5\">\
                 <name content=\"C5\"/></argumentationElement>"
            )),
            holder: "p",
            written: "xmi:type=\"sacm:Claim\" xmi:id=\"c5\">",
        },
        Spelled {
            what: "argumentationElement as the group's attribute",
            template: group(&format!(" {MARK}argumentationElement=\"c1 c2\""), ""),
            holder: "g",
            written: "argumentationElement=\"c1 c2\"",
        },
        Spelled {
            what: "argumentationElement as the group's element",
            template: group(
                "",
                &format!("{MARK}<argumentationElement xmi:idref=\"c1\"/>"),
            ),
            holder: "g",
            written: "xmi:idref=\"c1\" />",
        },
    ]
}

#[test]
fn sacm23_xmi_009_a_strict_read_keeps_each_spelling_as_preserved_content_with_one_error() {
    for case in spelled_constructs() {
        let what = case.what;
        let (input, at) = one_mark(what, &case.template);
        let output = read_in(what, Mode::Strict, &input);
        assert_eq!(
            reported(output.diagnostics()),
            one(Code::XMI_SPELLING_VARIANT, Severity::Error, at, case.holder),
            "{ROW}: {what}: not one SACM-XMI-013 Error at the construct affecting {} and no \
             SACM-XMI-003",
            case.holder
        );
        assert!(output.has_errors(), "{ROW}: {what}: has_errors() is false");
        let doc = output.into_document();
        assert!(
            element(&doc, case.holder).has_preserved_content(),
            "{ROW}: {what}: {} holds no preserved content",
            case.holder
        );
        // Nothing is read from it.
        assert!(
            doc.get("r").is_none() && doc.get("c5").is_none(),
            "{ROW}: {what}: an element was read from the construct"
        );
        if let Some(ev) = doc.get("ev") {
            assert_eq!(
                ev.date(),
                None,
                "{ROW}: {what}: a date was read from occurece"
            );
        }
        if let Some(g) = doc.get("g") {
            assert!(
                g.group_members().is_empty(),
                "{ROW}: {what}: members were read from the construct"
            );
        }
        // A strict write refuses it; a tolerant write writes it back.
        let refusal = xmi::write(&doc, &WriteOptions::default().with_mode(Mode::Strict));
        assert!(
            matches!(
                &refusal,
                Err(WriteError::PreservedContent { elements, .. })
                    if strs(elements) == [case.holder]
            ),
            "{ROW}: {what}: a strict write is not PreservedContent naming {}: {refusal:?}",
            case.holder
        );
        let text = match xmi::write(&doc, &WriteOptions::default().with_mode(Mode::Tolerant)) {
            Ok(text) => text,
            Err(error) => panic!("{ROW}: {what}: a tolerant write refused it: {error:?}"),
        };
        assert!(
            text.contains(case.written),
            "{ROW}: {what}: a tolerant write does not write it back as read\n{text}"
        );
    }
    // The ids inside a kept construct join the id space.
    for (what, template, operation) in [
        (
            "the role element c5",
            package(&format!(
                "{MARK}<argumentationElement xmi:type=\"sacm:Claim\" xmi:id=\"c5\">\
                 <name content=\"C5\"/></argumentationElement>"
            )),
            edit::Operation::from(CreateClaim::new("p", LangString::new("N")).with_id("c5")),
        ),
        (
            "the relationship r",
            artifacts(&format!(
                "{MARK}{}",
                relationship("xmi:type=\"sacm:ArtifactAssertedRelationship\"")
            )),
            edit::Operation::from(CreateArtifact::new("pkg", LangString::new("N")).with_id("r")),
        ),
    ] {
        let (input, _) = one_mark(what, &template);
        let mut doc = read_in(what, Mode::Strict, &input).into_document();
        let answer = edit::apply(&mut doc, operation);
        assert!(
            matches!(&answer, Err(EditError::DuplicateId { .. })),
            "{ROW}: {what}: an id inside it is not in the id space: {answer:?}"
        );
    }
    // A date beside occurece reads; occurece is kept.
    let what = "date beside occurece";
    let (input, at) = one_mark(
        what,
        &event(
            &format!(" date=\"2024-03-06\" {MARK}occurece=\"2024-03-05\""),
            "",
        ),
    );
    let output = read_in(what, Mode::Strict, &input);
    assert_eq!(
        reported(output.diagnostics()),
        one(Code::XMI_SPELLING_VARIANT, Severity::Error, at, "ev"),
        "{ROW}: {what}: not one SACM-XMI-013 Error at occurece"
    );
    assert_eq!(
        output.document().get("ev").and_then(SacmElement::date),
        Some("2024-03-06"),
        "{ROW}: {what}: the date is not read"
    );
}

// =====================================================================================
// Item 5: the messages, the code and the catalog
// =====================================================================================

#[test]
fn sacm23_xmi_009_the_messages_name_the_spelling_the_resolved_name_the_row_and_the_clause() {
    let role = "the element \"argumentationElement\" in the root element \"sacm:ArgumentPackage\" \
                is spelled as in the text of §11.4, and the role is argumentElement";
    let end = "\"argumentationElement\" in the element \"argumentElement\" is spelled as in the \
               text of §11.2, and the end of an ArgumentGroup is argumentElement";
    let date = "\"occurece\" in the element \"artifactElement\" is the machine-readable model's \
                spelling, and the attribute of an Event is date (§12.9)";
    let kind = "the element \"artifactElement\" in the root element \"sacm:ArtifactPackage\" is \
                typed ArtifactAssertedRelationship, the machine-readable model's spelling of \
                ArtifactAssetRelationship";
    let cases = [
        (
            "the type",
            artifacts(&relationship(
                "xmi:type=\"sacm:ArtifactAssertedRelationship\"",
            )),
            kind,
            "ArtifactAssetRelationship",
            "12.14",
        ),
        (
            "occurece",
            event(" occurece=\"2024-03-05\"", ""),
            date,
            "date",
            "12.9",
        ),
        (
            "occurece as an element",
            event("", "<occurece>2024-03-05</occurece>"),
            date,
            "date",
            "12.9",
        ),
        (
            "the role",
            package(
                "<argumentationElement xmi:type=\"sacm:Claim\" xmi:id=\"c5\"><name content=\"C5\"/></argumentationElement>",
            ),
            role,
            "argumentElement",
            "11.4",
        ),
        (
            "the group's attribute",
            group(" argumentationElement=\"c1\"", ""),
            end,
            "argumentElement",
            "11.2",
        ),
        (
            "the group's element",
            group("", "<argumentationElement xmi:idref=\"c1\"/>"),
            end,
            "argumentElement",
            "11.2",
        ),
    ];
    for (what, input, reason, resolved, clause) in cases {
        for (mode, ending) in [
            (
                Mode::Tolerant,
                format!("; it is read as {resolved}, which write writes"),
            ),
            (
                Mode::Strict,
                format!(
                    "; it is kept as preserved content, and a tolerant read reads it as {resolved}"
                ),
            ),
        ] {
            let output = read_in(what, mode, &input);
            let [found] = output.diagnostics() else {
                panic!(
                    "{ROW}: {what}: not one diagnostic in {mode:?} mode: {:?}",
                    output.diagnostics()
                );
            };
            assert_eq!(
                (
                    found.code(),
                    found.message(),
                    found.requirement_id(),
                    found.clause()
                ),
                (
                    Code::XMI_SPELLING_VARIANT,
                    format!("{reason}{ending}").as_str(),
                    ROW,
                    Some(clause)
                ),
                "{ROW}: {what}: the message, requirement and clause in {mode:?} mode"
            );
        }
    }
}

#[test]
fn sacm23_xmi_009_the_code_is_sacm_xmi_013_and_its_catalog_row_names_both_severities_and_the_row() {
    assert_eq!(
        Code::XMI_SPELLING_VARIANT.as_str(),
        "SACM-XMI-013",
        "{ROW}: Code::XMI_SPELLING_VARIANT is not SACM-XMI-013"
    );
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/sacm/diagnostics.md");
    let catalog = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{ROW}: cannot read {}: {error}", path.display()));
    let rows: Vec<&str> = catalog
        .lines()
        .filter(|line| line.trim_start().starts_with("| SACM-XMI-013 |"))
        .collect();
    let [row] = rows.as_slice() else {
        panic!(
            "{ROW}: the catalog has {} rows for SACM-XMI-013, not one",
            rows.len()
        );
    };
    let cells: Vec<&str> = row.split('|').map(str::trim).collect();
    let (severity, meaning) = match cells.as_slice() {
        [_, _, severity, meaning, ..] => (*severity, *meaning),
        _ => panic!("{ROW}: the catalog row of SACM-XMI-013 has no severity and meaning: {row}"),
    };
    assert!(
        severity.contains("Error") && severity.contains("Warning"),
        "{ROW}: the catalog row of SACM-XMI-013 does not name both severities: {severity:?}"
    );
    for needle in ["`Code::XMI_SPELLING_VARIANT`", "SACM23-XMI-009"] {
        assert!(
            meaning.contains(needle),
            "{ROW}: the catalog row of SACM-XMI-013 does not name {needle}: {row}"
        );
    }
}

// =====================================================================================
// Item 6: the fixture and the golden file
// =====================================================================================

#[test]
fn sacm23_xmi_009_the_fixture_reads_with_five_warnings_and_writes_exactly_the_golden_file() {
    let input = std::fs::read_to_string(FIXTURE)
        .unwrap_or_else(|error| panic!("{ROW}: cannot read {FIXTURE}: {error}"));
    let golden = std::fs::read_to_string(GOLDEN)
        .unwrap_or_else(|error| panic!("{ROW}: cannot read {GOLDEN}: {error}"));
    let sorted = |found: &[Diagnostic]| {
        let mut found = reported(found);
        found.sort_by_key(|found| found.2);
        found
    };
    let at = |line, column, severity, affected: &str| {
        (
            Code::XMI_SPELLING_VARIANT,
            severity,
            Some(loc(line, column)),
            vec![ElementId::from(affected)],
        )
    };
    let tolerant = read_in("the fixture", Mode::Tolerant, &input);
    assert_eq!(
        sorted(tolerant.diagnostics()),
        [
            at(6, 5, Severity::Warning, "arguments"),
            at(12, 5, Severity::Warning, "arguments"),
            at(12, 69, Severity::Warning, "g1"),
            at(21, 62, Severity::Warning, "released"),
            at(24, 5, Severity::Warning, "release_of"),
        ],
        "{ROW}: a tolerant read of the fixture: not five SACM-XMI-013 Warnings"
    );
    let doc = tolerant.into_document();
    assert_eq!(
        validation::validate(&doc),
        [],
        "{ROW}: the tolerant read of the fixture does not validate cleanly"
    );
    let text = match xmi::write(&doc, &WriteOptions::default().with_mode(Mode::Strict)) {
        Ok(text) => text,
        Err(error) => panic!("{ROW}: a strict write of the fixture refused it: {error:?}"),
    };
    assert_eq!(
        text, golden,
        "{ROW}: a strict write of the fixture is not the golden file"
    );
    let back = read_clean("the golden file", Mode::Strict, &golden);
    assert!(
        back == doc,
        "{ROW}: the golden file does not read back == to the tolerant read"
    );
    let strict = read_in("the fixture", Mode::Strict, &input);
    assert_eq!(
        sorted(strict.diagnostics()),
        [
            at(6, 5, Severity::Error, "arguments"),
            at(12, 5, Severity::Error, "arguments"),
            at(21, 62, Severity::Error, "released"),
            at(24, 5, Severity::Error, "evidence"),
        ],
        "{ROW}: a strict read of the fixture: not four SACM-XMI-013 Errors"
    );
}
