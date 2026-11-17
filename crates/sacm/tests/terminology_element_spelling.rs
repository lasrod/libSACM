//! Tests for the text spelling `TerminologyElement` of a TerminologyPackage's
//! composite role (SACM23-XMI-028; formal/23-05-08 §10.4
//! "TerminologyElement:TerminologyElement[0..*] (composition)", §10.3
//! "terminologyElement[0..*]"; ptc/22-03-13 `Terminology::TerminologyPackage`
//! `terminologyElement`; spec defect D9; D-TEXT-MODEL; the maintainer's M3
//! decision 3; ADR 0034 amendment 3).
//!
//! Under SACM23-XMI-009's rules: a tolerant read reads a `TerminologyElement`
//! child of a TerminologyPackage under the role `terminologyElement`, in
//! document order, with one SACM-XMI-013 Warning at its start tag affecting the
//! package; a strict read keeps it whole as preserved content of the package
//! with one SACM-XMI-013 Error, so a strict write refuses the document.
//! Elsewhere, and as a `TerminologyGroup`'s end, `TerminologyElement` is
//! ordinary unknown content (SACM-XMI-003). Export writes
//! `terminologyElement`.
//!
//! Expected locations are marked in the input with [`MARK`] (U+2038), which
//! the test removes before reading. Inputs end lines with LF only.

use sacm::diagnostic::{Code, Diagnostic, Severity, SourceLocation};
use sacm::model::{Document, ElementId, SacmElement};
use sacm::validation;
use sacm::xmi::{self, Mode, ReadOptions, ReadOutput, WriteError, WriteOptions};

const ROW: &str = "SACM23-XMI-028";

/// Marks, in a test input, where a refusal or an occurrence is expected.
/// Removed before reading.
const MARK: char = '\u{2038}';

/// The namespace declarations of a bare root: `sacm`, `xmi` and `xsi`.
const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
    " xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"",
);

/// The fixture of the row.
const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/sacm23/xmi/terminology-element-spelling.sacm.xmi"
);

/// The exact output of a strict write of the tolerant read of [`FIXTURE`].
const GOLDEN: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/sacm23/xmi/golden/terminology-element-spelling.golden.sacm.xmi"
);

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

fn read_in(what: &str, mode: Mode, input: &str) -> ReadOutput {
    match xmi::read(input.as_bytes(), &ReadOptions::default().with_mode(mode)) {
        Ok(output) => output,
        Err(error) => panic!("{ROW}: {what}: a {mode:?} read refused it: {error:?} ({error})"),
    }
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

/// `(code, severity, location)` of each diagnostic: what two inputs of one
/// layout but other ids are compared by.
fn located(found: &[Diagnostic]) -> Vec<(Code, Severity, Option<SourceLocation>)> {
    found
        .iter()
        .map(|found| (found.code(), found.severity(), found.location()))
        .collect()
}

fn element<'d>(doc: &'d Document, id: &str) -> SacmElement<'d> {
    let Some(found) = doc.get(id) else {
        panic!("{ROW}: the document has no element {id:?}");
    };
    found
}

fn children(doc: &Document, id: &str) -> Vec<String> {
    element(doc, id)
        .children()
        .map(|child| child.id().as_str().to_owned())
        .collect()
}

fn ids(values: &[&str]) -> Vec<String> {
    values.iter().map(|&value| value.to_owned()).collect()
}

fn write_in(what: &str, doc: &Document, mode: Mode) -> String {
    match xmi::write(doc, &WriteOptions::default().with_mode(mode)) {
        Ok(text) => text,
        Err(error) => panic!("{ROW}: {what}: a {mode:?} write refused the document: {error:?}"),
    }
}

fn fixture_text() -> String {
    std::fs::read_to_string(FIXTURE)
        .unwrap_or_else(|error| panic!("{ROW}: cannot read {FIXTURE}: {error}"))
}

fn golden_text() -> String {
    std::fs::read_to_string(GOLDEN)
        .unwrap_or_else(|error| panic!("{ROW}: cannot read {GOLDEN}: {error}"))
}

/// The two spelled children's locations in the fixture: their `<`.
fn spelled_at() -> [SourceLocation; 2] {
    [loc(5, 3), loc(8, 3)]
}

// =====================================================================================
// Item 1: the tolerant read of the fixture, and the golden file
// =====================================================================================

#[test]
fn sacm23_xmi_028_the_fixture_reads_tolerantly_with_two_warnings_and_writes_exactly_the_golden_file()
 {
    let output = read_in("the fixture", Mode::Tolerant, &fixture_text());
    assert_eq!(
        reported(output.diagnostics()),
        spelled_at()
            .iter()
            .map(|at| (
                Code::XMI_SPELLING_VARIANT,
                Severity::Warning,
                Some(*at),
                vec![ElementId::from("terms")]
            ))
            .collect::<Reported>(),
        "{ROW}: a tolerant read of the fixture: not two SACM-XMI-013 Warnings affecting terms"
    );
    let doc = output.into_document();
    assert_eq!(
        (
            children(&doc, "terms"),
            doc.has_preserved_content(),
            validation::validate(&doc)
        ),
        (ids(&["blade", "parts", "blade_part"]), false, Vec::new()),
        "{ROW}: the tolerant read: not the three children in order, with no finding"
    );
    let golden = golden_text();
    let text = write_in("the tolerant read", &doc, Mode::Strict);
    assert_eq!(
        text, golden,
        "{ROW}: a strict write of the tolerant read is not the golden file"
    );
    let back = read_in("the golden file", Mode::Strict, &golden);
    assert!(
        back.diagnostics().is_empty(),
        "{ROW}: the golden file reads strictly with {:?}",
        back.diagnostics()
    );
    assert!(
        back.into_document() == doc,
        "{ROW}: the golden file does not read back == to the tolerant read"
    );
}

// =====================================================================================
// Item 2: the strict read of the fixture
// =====================================================================================

#[test]
fn sacm23_xmi_028_a_strict_read_keeps_each_spelled_child_as_preserved_content_with_an_error() {
    let output = read_in("the fixture", Mode::Strict, &fixture_text());
    assert_eq!(
        reported(output.diagnostics()),
        spelled_at()
            .iter()
            .map(|at| (
                Code::XMI_SPELLING_VARIANT,
                Severity::Error,
                Some(*at),
                vec![ElementId::from("terms")]
            ))
            .collect::<Reported>(),
        "{ROW}: a strict read of the fixture: not two SACM-XMI-013 Errors affecting terms"
    );
    assert!(
        output.has_errors(),
        "{ROW}: the strict read: has_errors() is false"
    );
    let doc = output.into_document();
    assert_eq!(
        (
            children(&doc, "terms"),
            element(&doc, "terms").has_preserved_content(),
            doc.get("blade").is_none() && doc.get("parts").is_none()
        ),
        (ids(&["blade_part"]), true, true),
        "{ROW}: the strict read: not only blade_part read, with the rest kept by terms"
    );
    // The references into the kept children are to preserved targets.
    let mut found: Vec<(Code, Severity, Option<String>, Vec<ElementId>)> =
        validation::validate(&doc)
            .iter()
            .map(|found| {
                (
                    found.code(),
                    found.severity(),
                    found.clause().map(str::to_owned),
                    found.affected().to_vec(),
                )
            })
            .collect();
    found.sort_by(|a, b| a.3.cmp(&b.3));
    assert_eq!(
        found,
        [
            (
                Code::REF_PRESERVED_TARGET,
                Severity::Warning,
                Some("10.10".to_owned()),
                vec![ElementId::from("blade_part"), ElementId::from("blade")]
            ),
            (
                Code::REF_PRESERVED_TARGET,
                Severity::Warning,
                Some("10.9".to_owned()),
                vec![ElementId::from("blade_part"), ElementId::from("parts")]
            ),
        ],
        "{ROW}: validate does not give SACM-REF-003 for element blade and category parts"
    );
    // A strict write refuses it; a tolerant write writes the children back as
    // read, spelling and all.
    let refusal = xmi::write(&doc, &WriteOptions::default().with_mode(Mode::Strict));
    assert!(
        matches!(
            &refusal,
            Err(WriteError::PreservedContent { elements, .. })
                if elements.iter().map(ElementId::as_str).collect::<Vec<_>>() == ["terms"]
        ),
        "{ROW}: a strict write is not PreservedContent naming terms: {refusal:?}"
    );
    let text = write_in("the strict read", &doc, Mode::Tolerant);
    for written in [
        "<TerminologyElement xmi:type=\"sacm:Term\" xmi:id=\"blade\" value=\"blade\">",
        "<TerminologyElement xmi:type=\"sacm:Category\" xmi:id=\"parts\">",
        "</TerminologyElement>",
    ] {
        assert!(
            text.contains(written),
            "{ROW}: a tolerant write does not write {written:?} back\n{text}"
        );
    }
    let blade = text.find("xmi:id=\"blade\"");
    let parts = text.find("xmi:id=\"parts\"");
    let blade_part = text.find("xmi:id=\"blade_part\"");
    assert!(
        blade < parts && parts < blade_part,
        "{ROW}: a tolerant write does not keep the children in document order\n{text}"
    );
}

// =====================================================================================
// Item 3: the messages
// =====================================================================================

#[test]
fn sacm23_xmi_028_the_messages_name_the_spelling_the_role_the_row_and_the_clause() {
    let reason = "the element \"TerminologyElement\" in the root element \
                  \"sacm:TerminologyPackage\" is spelled as in the text of §10.4, and the role is \
                  terminologyElement";
    for (mode, ending) in [
        (
            Mode::Tolerant,
            "; it is read as terminologyElement, which write writes",
        ),
        (
            Mode::Strict,
            "; it is kept as preserved content, and a tolerant read reads the name as \
             terminologyElement",
        ),
    ] {
        let output = read_in("the fixture", mode, &fixture_text());
        let found: Vec<(&str, &str, Option<&str>)> = output
            .diagnostics()
            .iter()
            .map(|found| (found.message(), found.requirement_id(), found.clause()))
            .collect();
        let message = format!("{reason}{ending}");
        assert_eq!(
            found,
            [
                (message.as_str(), ROW, Some("10.4")),
                (message.as_str(), ROW, Some("10.4"))
            ],
            "{ROW}: the messages, requirement and clause in {mode:?} mode"
        );
    }
}

// =====================================================================================
// Item 4: the other inputs, against the spelling SACM23-XMI-009 reads
// =====================================================================================

/// A TerminologyPackage root `t` holding `child` on line 3, after a Category.
fn terminology(child: &str) -> String {
    format!(
        "<sacm:TerminologyPackage{NS} xmi:id=\"t\">\n  <name content=\"T\"/>\n  {child}\n\
         </sacm:TerminologyPackage>"
    )
}

/// An ArgumentPackage root `t` holding `child` on line 3: the same layout.
fn argumentation(child: &str) -> String {
    format!(
        "<sacm:ArgumentPackage{NS} xmi:id=\"t\">\n  <name content=\"T\"/>\n  {child}\n\
         </sacm:ArgumentPackage>"
    )
}

#[test]
fn sacm23_xmi_028_an_untyped_or_misplaced_spelled_child_is_reported_as_argumentation_element_is() {
    for (what, spelled, analog) in [
        (
            "an untyped spelled child",
            terminology("<TerminologyElement xmi:id=\"x\"/>"),
            argumentation("<argumentationElement xmi:id=\"x\"/>"),
        ),
        (
            "an untyped spelled child with a name",
            terminology(
                "<TerminologyElement xmi:id=\"x\"><name content=\"X\"/></TerminologyElement>",
            ),
            argumentation(
                "<argumentationElement xmi:id=\"x\"><name content=\"X\"/></argumentationElement>",
            ),
        ),
    ] {
        for mode in [Mode::Strict, Mode::Tolerant] {
            let found = read_in(what, mode, &spelled);
            let expected = read_in(&format!("{what}, as argumentationElement"), mode, &analog);
            assert_eq!(
                located(found.diagnostics()),
                located(expected.diagnostics()),
                "{ROW}: {what} in {mode:?} mode: not reported as argumentationElement is"
            );
            // A strict read keeps the spelled child whole, with SACM-XMI-013; a
            // tolerant one reads the name and then reports what the role
            // reports (SACM-XMI-005), as for argumentationElement.
            let codes: Vec<Code> = found.diagnostics().iter().map(Diagnostic::code).collect();
            let expected_codes = match mode {
                Mode::Strict => vec![Code::XMI_SPELLING_VARIANT],
                _ => vec![Code::XMI_MISSING_TYPE],
            };
            assert_eq!(
                codes, expected_codes,
                "{ROW}: {what} in {mode:?} mode: the codes"
            );
        }
    }
    // A Claim, which a TerminologyPackage's role does not admit: a strict read
    // keeps it as the analog is kept, with one SACM-XMI-013 Error; a tolerant
    // read reads the spelling and then keeps the Claim as content the role
    // does not admit (SACM-XMI-003), as it does an argumentationElement typed
    // with a kind its role does not admit (a Term). An argumentationElement
    // typed sacm:Claim is admitted, so it is no analog in tolerant mode.
    let claim = terminology(
        "<TerminologyElement xmi:type=\"sacm:Claim\" xmi:id=\"x\"><name content=\"X\"/></TerminologyElement>",
    );
    let strict = read_in("a spelled Claim", Mode::Strict, &claim);
    let strict_analog = read_in(
        "an argumentationElement Claim",
        Mode::Strict,
        &argumentation(
            "<argumentationElement xmi:type=\"sacm:Claim\" xmi:id=\"x\"><name content=\"X\"/></argumentationElement>",
        ),
    );
    assert_eq!(
        (
            located(strict.diagnostics()),
            located(strict_analog.diagnostics())
        ),
        (
            vec![(Code::XMI_SPELLING_VARIANT, Severity::Error, Some(loc(3, 3)))],
            vec![(Code::XMI_SPELLING_VARIANT, Severity::Error, Some(loc(3, 3)))]
        ),
        "{ROW}: a spelled Claim in strict mode: not one SACM-XMI-013 Error, as the analog"
    );
    let tolerant = read_in("a spelled Claim", Mode::Tolerant, &claim);
    let tolerant_analog = read_in(
        "an argumentationElement Term",
        Mode::Tolerant,
        &argumentation(
            "<argumentationElement xmi:type=\"sacm:Term\" xmi:id=\"x\" value=\"x\"><name content=\"X\"/></argumentationElement>",
        ),
    );
    assert_eq!(
        located(tolerant.diagnostics()),
        located(tolerant_analog.diagnostics()),
        "{ROW}: a spelled Claim in tolerant mode: not reported as an argumentationElement of a \
         kind the role does not admit"
    );
    assert_eq!(
        located(tolerant.diagnostics()),
        [(
            Code::XMI_UNKNOWN_CONTENT,
            Severity::Warning,
            Some(loc(3, 3))
        )],
        "{ROW}: a spelled Claim in tolerant mode: not one SACM-XMI-003 Warning at it"
    );
    let doc = tolerant.into_document();
    assert!(
        doc.get("x").is_none() && element(&doc, "t").has_preserved_content(),
        "{ROW}: a spelled Claim in tolerant mode is read as a child"
    );
}

#[test]
fn sacm23_xmi_028_terminology_element_elsewhere_is_unknown_content_with_no_spelling_warning() {
    let spelled = |id: &str| {
        format!(
            "{MARK}<TerminologyElement xmi:type=\"sacm:Term\" xmi:id=\"{id}\" value=\"x\">\
             <name content=\"X\"/></TerminologyElement>"
        )
    };
    let cases = [
        (
            "in an AssuranceCasePackage",
            format!(
                "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\">\n  <name content=\"C\"/>\n  \
                 {}\n</sacm:AssuranceCasePackage>",
                spelled("x")
            ),
            "case",
        ),
        (
            "in an ArtifactPackage",
            format!(
                "<sacm:ArtifactPackage{NS} xmi:id=\"a\">\n  <name content=\"A\"/>\n  {}\n\
                 </sacm:ArtifactPackage>",
                spelled("x")
            ),
            "a",
        ),
        (
            "as an element in a TerminologyGroup",
            terminology(&format!(
                "<terminologyElement xmi:type=\"sacm:Term\" xmi:id=\"m\" value=\"m\">\
                 <name content=\"M\"/></terminologyElement>\
                 <terminologyElement xmi:type=\"sacm:TerminologyGroup\" xmi:id=\"g\">\
                 <name content=\"G\"/>{MARK}<TerminologyElement xmi:idref=\"m\"/>\
                 </terminologyElement>"
            )),
            "g",
        ),
        (
            "as an attribute on a TerminologyGroup",
            terminology(&format!(
                "<terminologyElement xmi:type=\"sacm:Term\" xmi:id=\"m\" value=\"m\">\
                 <name content=\"M\"/></terminologyElement>\
                 <terminologyElement xmi:type=\"sacm:TerminologyGroup\" xmi:id=\"g\" \
                 {MARK}TerminologyElement=\"m\"><name content=\"G\"/></terminologyElement>"
            )),
            "g",
        ),
    ];
    for (what, template, holder) in cases {
        let (input, marks) = marked(&template);
        let [at] = marks.as_slice() else {
            panic!("{ROW}: setup: {what} has not one mark");
        };
        for (mode, severity) in [
            (Mode::Strict, Severity::Error),
            (Mode::Tolerant, Severity::Warning),
        ] {
            let output = read_in(what, mode, &input);
            assert_eq!(
                reported(output.diagnostics()),
                [(
                    Code::XMI_UNKNOWN_CONTENT,
                    severity,
                    Some(*at),
                    vec![ElementId::from(holder)]
                )],
                "{ROW}: {what} in {mode:?} mode: not one SACM-XMI-003 affecting {holder} and no \
                 SACM-XMI-013"
            );
            let doc = output.into_document();
            assert!(
                element(&doc, holder).has_preserved_content() && doc.get("x").is_none(),
                "{ROW}: {what} in {mode:?} mode: not kept by {holder}"
            );
            if let Some(group) = doc.get("g") {
                assert!(
                    group.group_members().is_empty(),
                    "{ROW}: {what} in {mode:?} mode: read as members"
                );
            }
        }
    }
}

#[test]
fn sacm23_xmi_028_export_writes_terminology_element_for_every_child() {
    let output = read_in("the fixture", Mode::Tolerant, &fixture_text());
    let text = write_in("the tolerant read", &output.into_document(), Mode::Tolerant);
    assert!(
        !text.contains("TerminologyElement") && text.matches("<terminologyElement ").count() == 3,
        "{ROW}: the export does not write terminologyElement for each of the three children\n{text}"
    );
}
