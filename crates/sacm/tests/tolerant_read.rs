//! Tests that a tolerant read opens every well-formed SACM file
//! (LIB-COMPAT-001, SACM23-VAL-006; ADR 0035 D1; the maintainer,
//! 2026-11-16).
//!
//! A strict read is unchanged (ADR 0035 D5): each input here is refused by
//! it, and the tests that pin those refusals stay with their rows. A
//! tolerant read refuses only a DOCTYPE, malformed XML, `InvalidRoot`,
//! `NoPackage`, `DuplicateId` and the limits. Every other strict refusal is
//! kept as preserved content at its construct, with one Warning at the
//! strict refusal's location, affecting the holder:
//!
//! | Strict refusal | Tolerant Warning | Model holds |
//! |---|---|---|
//! | `InvalidLiteral` of a declaration or flag | SACM-ENUM-001 | the default |
//! | `InvalidLiteral` of `xsi:nil` | SACM-ENUM-001 | the property absent |
//! | `RepeatedValue` | SACM-MULT-001, one per preserved occurrence | the first |
//! | `MissingType` | SACM-XMI-005, affecting the parent | no element |
//! | `InvalidId` (empty) | SACM-ID-002 | a generated id, or no part id |
//!
//! Each kept construct writes back tolerantly and reads back equal, and a
//! strict write refuses the document as `PreservedContent`. Conforming
//! content not read yet (SACM-XMI-014) is `unread_content.rs`'s, which
//! needs the slice's new codes.

use sacm::compare::semantic_compare;
use sacm::diagnostic::{self, Code, Diagnostic, Severity, SourceLocation};
use sacm::edit::{self, CreateClaim, EditError, SetAssertionDeclaration, SetFlag};
use sacm::model::{AssertionDeclaration, Document, ElementId, Flag, LangString, SacmElement};
use sacm::xmi::{self, Mode, ReadError, ReadOptions, ReadOutput, WriteError, WriteOptions};

const ROW: &str = "LIB-COMPAT-001";
const VAL_006: &str = "SACM23-VAL-006";

const MODES: [Mode; 2] = [Mode::Strict, Mode::Tolerant];

/// The namespace declarations of a bare root: `sacm`, `xmi`, `xsi` and a
/// vendor `v`.
const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
    " xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"",
    " xmlns:v=\"urn:v\"",
);

// --- Inputs ------------------------------------------------------------------------

/// A named ArgumentPackage root `p` holding `body`.
fn package(body: &str) -> String {
    format!(
        "<sacm:ArgumentPackage{NS} xmi:id=\"p\">\n  <name content=\"P\"/>\n  {body}\n\
         </sacm:ArgumentPackage>"
    )
}

/// A Claim `id`, named after its id, carrying `attributes` and holding
/// `body` after its name.
fn claim(id: &str, attributes: &str, body: &str) -> String {
    format!(
        "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"{id}\"{attributes}>\
         <name content=\"{id}\"/>{body}</argumentElement>"
    )
}

/// An `AssertedInference` `id`, named, carrying `attributes`.
fn inference(id: &str, attributes: &str) -> String {
    format!(
        "<argumentElement xmi:type=\"sacm:AssertedInference\" xmi:id=\"{id}\"{attributes}>\
         <name content=\"{id}\"/></argumentElement>"
    )
}

// --- Reading and writing -------------------------------------------------------------

fn read(mode: Mode, input: &str) -> Result<ReadOutput, ReadError> {
    xmi::read(input.as_bytes(), &ReadOptions::default().with_mode(mode))
}

/// Reads `input` tolerantly, which the test expects to read.
fn tolerant(row: &str, what: &str, input: &str) -> ReadOutput {
    match read(Mode::Tolerant, input) {
        Ok(output) => output,
        Err(error) => panic!("{row}: {what}: a tolerant read refused it: {error:?} ({error})"),
    }
}

/// Reads `input` strictly, which the test expects to refuse it.
fn strict_refusal(row: &str, what: &str, input: &str) -> ReadError {
    match read(Mode::Strict, input) {
        Ok(output) => panic!(
            "{row}: {what}: setup: a strict read returned a document, with {:?}",
            output.diagnostics()
        ),
        Err(error) => error,
    }
}

fn write(doc: &Document, mode: Mode) -> Result<String, WriteError> {
    xmi::write(doc, &WriteOptions::default().with_mode(mode))
}

/// Writes `doc` tolerantly, which the test expects to succeed.
fn write_tolerant(row: &str, what: &str, doc: &Document) -> String {
    match write(doc, Mode::Tolerant) {
        Ok(text) => text,
        Err(error) => panic!("{row}: {what}: a tolerant write refused: {error:?} ({error})"),
    }
}

/// Writes `doc` tolerantly, reads the output back tolerantly and writes that
/// again: the read-back must be equal and the second write byte-identical.
/// Returns the output.
fn assert_round_trips(row: &str, what: &str, doc: &Document) -> String {
    let text = write_tolerant(row, what, doc);
    let back = tolerant(row, &format!("{what}, written"), &text).into_document();
    assert_eq!(
        semantic_compare(doc, &back),
        [],
        "{row}: {what}: the tolerant output does not read back equal\n{text}"
    );
    assert!(
        back == *doc,
        "{row}: {what}: the tolerant output reads back !=\n{text}"
    );
    assert_eq!(
        write_tolerant(row, what, &back),
        text,
        "{row}: {what}: writing the read-back again is not byte-identical"
    );
    text
}

fn element<'d>(row: &str, what: &str, doc: &'d Document, id: &str) -> SacmElement<'d> {
    let Some(found) = doc.get(id) else {
        panic!("{row}: {what}: the document has no element {id:?}");
    };
    found
}

fn ids(values: &[&str]) -> Vec<ElementId> {
    values.iter().map(|&value| ElementId::from(value)).collect()
}

/// The diagnostics of `output` with `code`.
fn with_code(output: &ReadOutput, code: Code) -> Vec<&Diagnostic> {
    output
        .diagnostics()
        .iter()
        .filter(|found| found.code() == code)
        .collect()
}

/// How many times `needle` occurs in `text`.
fn count(text: &str, needle: &str) -> usize {
    text.matches(needle).count()
}

/// The written start tag of the element whose id is `id`.
fn start_tag<'t>(row: &str, text: &'t str, id: &str) -> &'t str {
    let needle = format!("xmi:id=\"{id}\"");
    let Some(line) = text.lines().find(|line| line.contains(&needle)) else {
        panic!("{row}: {id:?} is not written\n{text}");
    };
    line.trim()
}

// =====================================================================================
// The ReadError table
// =====================================================================================

/// One strict refusal and what a tolerant read keeps of it.
struct Case {
    what: &'static str,
    input: String,
    /// The code of the strict refusal and of the tolerant Warning.
    code: Code,
    /// The ids the Warning affects.
    affected: &'static [&'static str],
    /// The element that holds the kept construct, or `None` when nothing is
    /// kept (an empty id is not kept, and a C++ shape is read).
    holder: Option<&'static str>,
    /// Whether the kept construct is also SACM-XMI-003, as a vendor element
    /// holding an empty id is.
    also_unknown: bool,
    /// What the model holds, and its description.
    holds: (fn(&Document) -> bool, &'static str),
}

#[expect(
    clippy::too_many_lines,
    reason = "one flat table is easier to review than several"
)]
fn cases() -> Vec<Case> {
    vec![
        Case {
            what: "assertionDeclaration=\"Assumed\"",
            input: package(&claim("c1", " assertionDeclaration=\"Assumed\"", "")),
            code: Code::ENUM_INVALID_LITERAL,
            affected: &["c1"],
            holder: Some("c1"),
            also_unknown: false,
            holds: (
                |doc| {
                    doc.get("c1").and_then(SacmElement::assertion_declaration)
                        == Some(AssertionDeclaration::Asserted)
                },
                "c1 asserted",
            ),
        },
        Case {
            what: "an assertionDeclaration element \" assumed\"",
            input: package(&claim(
                "c1",
                "",
                "<assertionDeclaration> assumed</assertionDeclaration>",
            )),
            code: Code::ENUM_INVALID_LITERAL,
            affected: &["c1"],
            holder: Some("c1"),
            also_unknown: false,
            holds: (
                |doc| {
                    doc.get("c1").and_then(SacmElement::assertion_declaration)
                        == Some(AssertionDeclaration::Asserted)
                },
                "c1 asserted",
            ),
        },
        Case {
            what: "isCitation=\"yes\"",
            input: package(&claim("c1", " isCitation=\"yes\"", "")),
            code: Code::ENUM_INVALID_LITERAL,
            affected: &["c1"],
            holder: Some("c1"),
            also_unknown: false,
            holds: (
                |doc| doc.get("c1").is_some_and(|c1| !c1.is_citation()),
                "c1 not a citation",
            ),
        },
        Case {
            what: "an isAbstract element TRUE",
            input: package(&claim("c1", "", "<isAbstract>TRUE</isAbstract>")),
            code: Code::ENUM_INVALID_LITERAL,
            affected: &["c1"],
            holder: Some("c1"),
            also_unknown: false,
            holds: (
                |doc| doc.get("c1").is_some_and(|c1| !c1.is_abstract()),
                "c1 not abstract",
            ),
        },
        Case {
            what: "isCounter=\"maybe\" on an AssertedInference",
            input: package(&format!(
                "{}{}{}",
                claim("c1", "", ""),
                claim("c2", "", ""),
                inference("i1", " isCounter=\"maybe\" source=\"c1\" target=\"c2\"")
            )),
            code: Code::ENUM_INVALID_LITERAL,
            affected: &["i1"],
            holder: Some("i1"),
            also_unknown: false,
            holds: (
                |doc| doc.get("i1").is_some_and(|i1| !i1.is_counter()),
                "i1 not a counter",
            ),
        },
        Case {
            what: "xsi:nil=\"yes\" on a gid element",
            input: package(&claim("c1", "", "<gid xsi:nil=\"yes\"/>")),
            code: Code::ENUM_INVALID_LITERAL,
            affected: &["c1"],
            holder: Some("c1"),
            also_unknown: false,
            holds: (
                |doc| doc.get("c1").is_some_and(|c1| c1.gid().is_none()),
                "c1 with no gid",
            ),
        },
        Case {
            what: "a gid attribute and a gid element",
            input: package(&claim("c1", " gid=\"a\"", "<gid>b</gid>")),
            code: Code::MULTIPLICITY_VIOLATION,
            affected: &["c1"],
            holder: Some("c1"),
            also_unknown: false,
            holds: (
                |doc| doc.get("c1").and_then(SacmElement::gid) == Some("a"),
                "c1's gid a",
            ),
        },
        Case {
            what: "target=\"c2 c3\"",
            input: package(&format!(
                "{}{}{}{}",
                claim("c1", "", ""),
                claim("c2", "", ""),
                claim("c3", "", ""),
                inference("i1", " source=\"c1\" target=\"c2 c3\"")
            )),
            code: Code::MULTIPLICITY_VIOLATION,
            affected: &["i1"],
            holder: Some("i1"),
            also_unknown: false,
            holds: (
                |doc| doc.get("i1").is_some_and(|i1| i1.target().is_none()),
                "i1 with no target",
            ),
        },
        Case {
            what: "a content attribute and a content element in a name",
            input: package(
                "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\">\
                 <name content=\"A\"><content>A</content></name></argumentElement>",
            ),
            code: Code::MULTIPLICITY_VIOLATION,
            affected: &["c1"],
            holder: Some("c1"),
            also_unknown: false,
            holds: (
                |doc| doc.get("c1").is_some_and(|c1| c1.name().is_none()),
                "c1 with no name",
            ),
        },
        Case {
            what: "two names",
            input: package(
                "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\">\
                 <name content=\"A\"/><name content=\"B\"/></argumentElement>",
            ),
            code: Code::MULTIPLICITY_VIOLATION,
            affected: &["c1"],
            holder: Some("c1"),
            also_unknown: false,
            holds: (
                |doc| doc.get("c1").and_then(SacmElement::name) == Some(&LangString::new("A")),
                "c1 named A",
            ),
        },
        Case {
            what: "an untyped argumentElement",
            input: package(
                "<argumentElement xmi:id=\"x1\"><name content=\"X\"/></argumentElement>",
            ),
            code: Code::XMI_MISSING_TYPE,
            affected: &["p"],
            holder: Some("p"),
            also_unknown: false,
            holds: (|doc| doc.get("x1").is_none(), "no element x1"),
        },
        Case {
            what: "an empty xmi:id on a Claim",
            input: package(
                "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"\"><name content=\"C\"/>\
                 </argumentElement>",
            ),
            code: Code::ID_INVALID,
            affected: &["claim_1"],
            holder: None,
            also_unknown: false,
            holds: (
                |doc| {
                    doc.get("claim_1")
                        .and_then(SacmElement::name)
                        .and_then(LangString::content)
                        == Some("C")
                },
                "the Claim as claim_1",
            ),
        },
        Case {
            what: "an empty xmi:id on a name",
            input: package(&claim("c1", "", "").replace(
                "<name content=\"c1\"/>",
                "<name xmi:id=\"\" content=\"c1\"/>",
            )),
            code: Code::ID_INVALID,
            affected: &["c1"],
            holder: None,
            also_unknown: false,
            holds: (
                |doc| {
                    doc.get("c1").is_some_and(|c1| {
                        c1.name_id().is_none() && c1.name() == Some(&LangString::new("c1"))
                    })
                },
                "c1 named c1 with no name id",
            ),
        },
        Case {
            what: "an empty xmi:id inside a vendor element",
            input: package(&claim("c1", "", "<v:x xmi:id=\"\"/>")),
            code: Code::ID_INVALID,
            affected: &["c1"],
            holder: Some("c1"),
            also_unknown: true,
            holds: (|doc| doc.get("c1").is_some(), "c1"),
        },
        Case {
            what: "a C++ name attribute",
            input: format!("<sacm:ArgumentPackage{NS} xmi:id=\"p\" name=\"P\"/>"),
            code: Code::XMI_LEGACY_SHAPE,
            affected: &["p"],
            holder: None,
            also_unknown: false,
            holds: (
                |doc| doc.get("p").and_then(SacmElement::name) == Some(&LangString::new("P")),
                "p named P",
            ),
        },
    ]
}

/// Each strict refusal of the table is one Warning of the same code at the
/// same location in a tolerant read, which keeps the construct and holds the
/// stated value. A tolerant write reads back equal and rewrites
/// byte-identical; a strict write is `PreservedContent`.
#[test]
fn lib_compat_001_tolerant_keeps_each_strict_refusal_with_its_warning_at_the_same_location() {
    let mut failures = Vec::new();
    for case in cases() {
        let what = case.what;
        let strict = strict_refusal(ROW, what, &case.input);
        assert_eq!(
            strict.code(),
            Some(case.code),
            "{ROW}: {what}: setup: the strict refusal is not {}: {strict:?}",
            case.code
        );
        let output = match read(Mode::Tolerant, &case.input) {
            Ok(output) => output,
            Err(error) => {
                failures.push(format!("{what}: a tolerant read refused it: {error:?}"));
                continue;
            }
        };
        let found = with_code(&output, case.code);
        let [one] = found.as_slice() else {
            failures.push(format!(
                "{what}: not one {}: {:?}",
                case.code,
                output.diagnostics()
            ));
            continue;
        };
        if (one.severity(), one.location(), one.affected())
            != (
                Severity::Warning,
                strict.location(),
                ids(case.affected).as_slice(),
            )
        {
            failures.push(format!(
                "{what}: the {} is not a Warning at {:?} affecting {:?}: {one:?}",
                case.code,
                strict.location(),
                case.affected
            ));
        }
        let others: Vec<&Diagnostic> = output
            .diagnostics()
            .iter()
            .filter(|found| found.code() != case.code)
            .collect();
        let others_allowed = others.iter().all(|found| {
            case.also_unknown
                && found.code() == Code::XMI_UNKNOWN_CONTENT
                && found.severity() == Severity::Warning
        });
        if !others_allowed || (case.also_unknown && others.len() != 1) {
            failures.push(format!("{what}: other diagnostics: {others:?}"));
        }
        if diagnostic::has_errors(output.diagnostics()) {
            failures.push(format!("{what}: a tolerant read reports an Error"));
        }
        let doc = output.into_document();
        let (holds, holds_what) = case.holds;
        if !holds(&doc) {
            failures.push(format!("{what}: the model does not hold {holds_what}"));
        }
        let holders: Vec<&str> = doc
            .elements()
            .filter(|element| element.has_preserved_content())
            .map(|element| element.id().as_str())
            .collect();
        let expected_holders: Vec<&str> = case.holder.into_iter().collect();
        if holders != expected_holders {
            failures.push(format!(
                "{what}: the holders are {holders:?}, not {expected_holders:?}"
            ));
        }
        assert_round_trips(ROW, what, &doc);
        match (case.holder, write(&doc, Mode::Strict)) {
            (Some(holder), Err(WriteError::PreservedContent { elements, .. }))
                if elements == ids(&[holder]) => {}
            (None, Ok(_)) => {}
            (_, other) => failures.push(format!(
                "{what}: the strict write is not PreservedContent [{:?}] (or Ok when nothing \
                 is kept): {other:?}",
                case.holder
            )),
        }
    }
    assert!(
        failures.is_empty(),
        "{ROW}: {} table rows are not kept as stated:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

// =====================================================================================
// What both modes refuse
// =====================================================================================

/// A DOCTYPE, malformed XML, `InvalidRoot`, `NoPackage` and `DuplicateId`
/// are refused alike in both modes, even where a tolerant read now keeps
/// content around them (ADR 0035 D1).
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one flat table is easier to review than several"
)]
fn lib_compat_001_the_fatal_refusals_are_the_same_in_both_modes() {
    let wrapper = |body: &str| {
        format!(
            "<xmi:XMI xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" \
             xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\">{body}</xmi:XMI>"
        )
    };
    let cases: Vec<(&str, String, Code)> = vec![
        (
            "a DOCTYPE",
            "<?xml version=\"1.0\"?>\n<!DOCTYPE r>\n<r/>".to_owned(),
            Code::XML_DOCTYPE_REJECTED,
        ),
        (
            "a DOCTYPE before a SACM root",
            format!("<!DOCTYPE r>\n{}", package("")),
            Code::XML_DOCTYPE_REJECTED,
        ),
        ("mismatched tags", "<r></s>".to_owned(), Code::XML_MALFORMED),
        (
            "a truncated root",
            format!("<sacm:ArgumentPackage{NS} xmi:id=\"p\""),
            Code::XML_MALFORMED,
        ),
        (
            "malformed XML after a bad literal",
            package(&claim("c1", " assertionDeclaration=\"Assumed\"", ""))
                .replace("</sacm:", "</x:"),
            Code::XML_MALFORMED,
        ),
        (
            "a root that is not SACM",
            "<r/>".to_owned(),
            Code::XMI_INVALID_ROOT,
        ),
        (
            "a Claim root",
            format!("<sacm:Claim{NS} xmi:id=\"c1\"/>"),
            Code::XMI_INVALID_ROOT,
        ),
        (
            "a wrapper holding no package",
            wrapper(""),
            Code::XMI_INVALID_ROOT,
        ),
        (
            "a wrapper holding only xmi:Documentation",
            wrapper("<xmi:Documentation/>"),
            Code::XMI_INVALID_ROOT,
        ),
        (
            "two Claims with one id",
            package(&format!("{}{}", claim("c1", "", ""), claim("c1", "", ""))),
            Code::ID_DUPLICATE,
        ),
        (
            "a repeat carrying xmi:uuid",
            package(&format!(
                "{}{}",
                claim("c1", "", ""),
                claim("c1", " xmi:uuid=\"u\"", "")
            )),
            Code::ID_DUPLICATE,
        ),
        (
            "a vendor element's id repeating a Claim's",
            package(&claim("c1", "", "<v:x xmi:id=\"c1\"/>")),
            Code::ID_DUPLICATE,
        ),
    ];
    let mut failures = Vec::new();
    // A strict read refuses the first defect in document order, so where a
    // construct a tolerant read keeps comes first, the strict read refuses
    // that, and the tolerant read still refuses the duplicate after it.
    for (what, input) in [
        (
            "a repeat after a bad literal",
            package(&format!(
                "{}{}",
                claim("c1", " assertionDeclaration=\"Assumed\"", ""),
                claim("c1", "", "")
            )),
        ),
        (
            "an untyped child whose id repeats a Claim's",
            package(&format!(
                "{}<argumentElement xmi:id=\"c1\"/>",
                claim("c1", "", "")
            )),
        ),
        (
            "a repeat after a TerminologyPackage child",
            format!(
                "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\"><name content=\"C\"/>\
                 <terminologyPackage xmi:id=\"t\"><name content=\"T\"/></terminologyPackage>\
                 <argumentPackage xmi:id=\"t\"><name content=\"A\"/></argumentPackage>\
                 </sacm:AssuranceCasePackage>"
            ),
        ),
    ] {
        if read(Mode::Strict, &input).is_ok() {
            failures.push(format!("{what}: setup: a strict read returned a document"));
        }
        match read(Mode::Tolerant, &input) {
            Err(error) if error.code() == Some(Code::ID_DUPLICATE) => {}
            other => failures.push(format!(
                "{what}: a tolerant read is not DuplicateId: {:?}",
                other.map(|output| output.diagnostics().to_vec())
            )),
        }
    }
    for (what, input, code) in &cases {
        let refusals: Vec<Result<ReadOutput, ReadError>> =
            MODES.iter().map(|&mode| read(mode, input)).collect();
        match (&refusals[0], &refusals[1]) {
            (Err(strict), Err(tolerant)) => {
                if strict.code() != Some(*code) {
                    failures.push(format!(
                        "{what}: the strict refusal is not {code}: {strict:?}"
                    ));
                }
                if strict != tolerant || strict.to_string() != tolerant.to_string() {
                    failures.push(format!(
                        "{what}: the refusals differ:\nstrict {strict:?}\ntolerant {tolerant:?}"
                    ));
                }
            }
            (strict, tolerant) => failures.push(format!(
                "{what}: not refused in both modes:\nstrict {:?}\ntolerant {:?}",
                strict.as_ref().err(),
                tolerant.as_ref().err()
            )),
        }
    }
    assert!(
        failures.is_empty(),
        "{ROW}: {} fatal inputs are not refused alike:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

// =====================================================================================
// Repeats, empty ids and untyped children
// =====================================================================================

/// The first occurrence is read; each later one is preserved with one
/// SACM-MULT-001 Warning at it.
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one input per acceptance item, in the order the row gives them"
)]
fn lib_compat_001_a_repeated_value_keeps_the_first_and_preserves_the_rest() {
    let what = "a gid attribute and a gid element";
    let output = tolerant(
        ROW,
        what,
        &package(&claim("c1", " gid=\"a\"", "<gid>b</gid>")),
    );
    assert_eq!(
        output.document().get("c1").and_then(SacmElement::gid),
        Some("a"),
        "{ROW}: {what}: not the attribute's gid"
    );

    let what = "two names";
    let output = tolerant(
        ROW,
        what,
        &package(
            "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\">\
             <name content=\"A\"/><name content=\"B\"/></argumentElement>",
        ),
    );
    assert_eq!(
        output
            .document()
            .get("c1")
            .and_then(SacmElement::name)
            .and_then(LangString::content),
        Some("A"),
        "{ROW}: {what}: not the first name"
    );

    let what = "three gid elements";
    let input = package(&claim("c1", "", "<gid>a</gid><gid>b</gid><gid>c</gid>"));
    let strict = strict_refusal(ROW, what, &input);
    let output = tolerant(ROW, what, &input);
    let repeats: Vec<(Severity, Option<SourceLocation>)> =
        with_code(&output, Code::MULTIPLICITY_VIOLATION)
            .iter()
            .map(|found| (found.severity(), found.location()))
            .collect();
    let later: Vec<(Severity, Option<SourceLocation>)> = strict.locations()[1..]
        .iter()
        .map(|&at| (Severity::Warning, Some(at)))
        .collect();
    assert_eq!(
        repeats, later,
        "{ROW}: {what}: not one SACM-MULT-001 Warning at each later occurrence"
    );
    assert_eq!(
        output.document().get("c1").and_then(SacmElement::gid),
        Some("a"),
        "{ROW}: {what}: not the first gid"
    );

    let what = "target=\"c2 c3\"";
    let input = package(&format!(
        "{}{}{}{}",
        claim("c1", "", ""),
        claim("c2", "", ""),
        claim("c3", "", ""),
        inference("i1", " source=\"c1\" target=\"c2 c3\"")
    ));
    let strict = strict_refusal(ROW, what, &input);
    let output = tolerant(ROW, what, &input);
    let repeats: Vec<Option<SourceLocation>> = with_code(&output, Code::MULTIPLICITY_VIOLATION)
        .iter()
        .map(|found| found.location())
        .collect();
    assert_eq!(
        repeats,
        [strict.locations().get(1).copied()],
        "{ROW}: {what}: not one SACM-MULT-001 at c3"
    );
    assert_eq!(
        output.document().get("i1").and_then(SacmElement::target),
        None,
        "{ROW}: {what}: a target is held"
    );

    let what = "a metaClaim attribute and a metaClaim element";
    let input = package(&format!(
        "{}{}{}",
        claim("m1", "", ""),
        claim("m2", "", ""),
        claim("c1", " metaClaim=\"m1\"", "<metaClaim xmi:idref=\"m2\"/>")
    ));
    let output = tolerant(ROW, what, &input);
    assert_eq!(
        output
            .document()
            .get("c1")
            .map(SacmElement::meta_claims)
            .map(<[ElementId]>::to_vec),
        Some(ids(&["m1"])),
        "{ROW}: {what}: not the attribute's ids"
    );
    assert_eq!(
        with_code(&output, Code::MULTIPLICITY_VIOLATION).len(),
        1,
        "{ROW}: {what}: not one SACM-MULT-001"
    );

    let what = "a content attribute and a content element in a name";
    let output = tolerant(
        ROW,
        what,
        &package(
            "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\">\
             <name content=\"A\"><content>A</content></name></argumentElement>",
        ),
    );
    let c1 = element(ROW, what, output.document(), "c1");
    assert!(
        c1.name().is_none() && c1.has_preserved_content(),
        "{ROW}: {what}: a name is held, or the name is not preserved"
    );
}

/// An empty `xmi:id` reads as no `xmi:id`: a generated id, with one
/// SACM-ID-002 Warning and no SACM-XMI-004. The empty value is not kept, so
/// two of them are no `DuplicateId`.
#[test]
fn lib_compat_001_an_empty_id_gets_a_generated_id_with_one_id_002_warning() {
    let what = "an empty xmi:id on a Claim";
    let output = tolerant(
        ROW,
        what,
        &package(&claim("", "", "").replace("<name content=\"\"/>", "<name content=\"C\"/>")),
    );
    let claim_1 = element(ROW, what, output.document(), "claim_1");
    assert!(
        claim_1.name() == Some(&LangString::new("C")) && !claim_1.has_preserved_content(),
        "{ROW}: {what}: claim_1 is not the Claim, or holds preserved content"
    );
    let found: Vec<(Code, Severity, Vec<ElementId>)> = output
        .diagnostics()
        .iter()
        .map(|found| (found.code(), found.severity(), found.affected().to_vec()))
        .collect();
    assert_eq!(
        found,
        [(Code::ID_INVALID, Severity::Warning, ids(&["claim_1"]))],
        "{ROW}: {what}: not one SACM-ID-002 Warning on claim_1 (and no SACM-XMI-004)"
    );

    let what = "two empty xmi:ids";
    let output = tolerant(
        ROW,
        what,
        &package(&format!(
            "{}{}",
            claim("", "", "").replace("<name content=\"\"/>", "<name content=\"A\"/>"),
            claim("", "", "").replace("<name content=\"\"/>", "<name content=\"B\"/>")
        )),
    );
    let ids_read: Vec<&str> = output
        .document()
        .elements()
        .map(|element| element.id().as_str())
        .collect();
    assert_eq!(
        ids_read,
        ["p", "claim_1", "claim_2"],
        "{ROW}: {what}: not two generated ids"
    );
    assert_eq!(
        with_code(&output, Code::ID_INVALID).len(),
        2,
        "{ROW}: {what}: not two SACM-ID-002"
    );
    assert!(
        with_code(&output, Code::XMI_MISSING_ID).is_empty(),
        "{ROW}: {what}: an empty id is reported as a missing one"
    );
}

/// An untyped child under an abstract role is preserved whole in its
/// parent, with SACM-XMI-005 affecting the parent; its `xmi:id` stays in the
/// id space.
#[test]
fn lib_compat_001_an_untyped_abstract_role_child_is_preserved_with_xmi_005() {
    let what = "an untyped argumentElement c1";
    let input = package("<argumentElement xmi:id=\"c1\"/>");
    let strict = strict_refusal(ROW, what, &input);
    let output = tolerant(ROW, what, &input);
    let found: Vec<(Code, Severity, Vec<ElementId>, Option<SourceLocation>)> = output
        .diagnostics()
        .iter()
        .map(|found| {
            (
                found.code(),
                found.severity(),
                found.affected().to_vec(),
                found.location(),
            )
        })
        .collect();
    assert_eq!(
        found,
        [(
            Code::XMI_MISSING_TYPE,
            Severity::Warning,
            ids(&["p"]),
            strict.location()
        )],
        "{ROW}: {what}: not one SACM-XMI-005 Warning affecting p at the start tag"
    );
    let mut doc = output.into_document();
    assert!(
        doc.get("c1").is_none() && element(ROW, what, &doc, "p").has_preserved_content(),
        "{ROW}: {what}: c1 is read, or p does not hold it"
    );
    let answer = edit::apply(
        &mut doc,
        CreateClaim::new("p", LangString::new("New")).with_id("c1"),
    );
    assert!(
        matches!(&answer, Err(EditError::DuplicateId { id, .. }) if id.as_str() == "c1"),
        "{ROW}: {what}: with_id(\"c1\") is not DuplicateId: {answer:?}"
    );
}

/// No tolerant read of a table input, nor of conforming content not read
/// yet, reports an Error (ADR 0035: "A tolerant read never reports an
/// Error").
#[test]
fn lib_compat_001_a_tolerant_read_reports_no_error() {
    let mut inputs: Vec<(String, String)> = cases()
        .into_iter()
        .map(|case| (case.what.to_owned(), case.input))
        .collect();
    let wrapper = |attributes: &str, body: &str| {
        format!(
            "<xmi:XMI xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" \
             xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\" \
             xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:v=\"urn:v\"{attributes}>\
             {body}<sacm:ArgumentPackage xmi:id=\"p\"><name content=\"P\"/></sacm:ArgumentPackage>\
             </xmi:XMI>"
        )
    };
    for (what, input) in [
        (
            "a TerminologyPackage root",
            format!(
                "<sacm:TerminologyPackage{NS} xmi:id=\"t\"><name content=\"T\"/></sacm:TerminologyPackage>"
            ),
        ),
        (
            "an ArtifactPackageInterface in an ArtifactPackage",
            format!(
                "<sacm:ArtifactPackage{NS} xmi:id=\"a\"><name content=\"A\"/>\
                 <artifactElement xmi:type=\"sacm:ArtifactPackageInterface\" xmi:id=\"x\">\
                 <name content=\"X\"/></artifactElement></sacm:ArtifactPackage>"
            ),
        ),
        (
            "xmi:uuid on a Claim",
            package(&claim("c1", " xmi:uuid=\"u\"", "")),
        ),
        (
            "xmi:Extension in a Claim",
            package(&claim("c1", "", "<xmi:Extension extender=\"v\"/>")),
        ),
        (
            "xsi:schemaLocation on the wrapper",
            wrapper(
                " xsi:schemaLocation=\"http://www.omg.org/spec/SACM/20220301 SACM.xsd\"",
                "",
            ),
        ),
        (
            "xmi:Documentation in the wrapper",
            wrapper("", "<xmi:Documentation/>"),
        ),
        (
            "a vendor attribute on the wrapper",
            wrapper(" v:w=\"1\"", ""),
        ),
        ("text in the wrapper", wrapper("", "stray")),
        (
            "a cross-document href",
            package(&claim("c1", "", "<metaClaim href=\"other.sacm.xmi#c\"/>")),
        ),
        (
            "an ExpressionLangString name",
            format!(
                "<sacm:ArgumentPackage{NS} xmi:id=\"p\">\
                 <name xmi:type=\"sacm:ExpressionLangString\" content=\"p\"/></sacm:ArgumentPackage>"
            ),
        ),
        (
            "the argumentationElement spelling",
            package("<argumentationElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"/>"),
        ),
        (
            "an interface end",
            package("").replace("xmi:id=\"p\">", "xmi:id=\"p\" interface=\"i\">"),
        ),
    ] {
        inputs.push((what.to_owned(), input));
    }
    let mut failures = Vec::new();
    for (what, input) in &inputs {
        match read(Mode::Tolerant, input) {
            Ok(output) if !diagnostic::has_errors(output.diagnostics()) => {}
            Ok(output) => failures.push(format!("{what}: Errors: {:?}", output.diagnostics())),
            Err(error) => failures.push(format!("{what}: refused: {error:?}")),
        }
    }
    assert!(
        failures.is_empty(),
        "{ROW}: {} of {} tolerant reads refuse or report an Error:\n{}",
        failures.len(),
        inputs.len(),
        failures.join("\n")
    );
}

// =====================================================================================
// SACM23-VAL-006: bad literals in tolerant mode
// =====================================================================================

/// A bad `assertionDeclaration` is preserved as written, with one
/// SACM-ENUM-001 Warning, and the Assertion holds `asserted`; the tolerant
/// write gives the attribute back once.
#[test]
fn sacm23_val_006_a_tolerant_read_preserves_a_bad_literal_and_holds_the_default() {
    let what = "assertionDeclaration=\"Assumed\"";
    let input = package(&claim("c1", " assertionDeclaration=\"Assumed\"", ""));
    let strict = strict_refusal(VAL_006, what, &input);
    let output = tolerant(VAL_006, what, &input);
    let found: Vec<(Code, Severity, Vec<ElementId>, Option<SourceLocation>)> = output
        .diagnostics()
        .iter()
        .map(|found| {
            (
                found.code(),
                found.severity(),
                found.affected().to_vec(),
                found.location(),
            )
        })
        .collect();
    assert_eq!(
        found,
        [(
            Code::ENUM_INVALID_LITERAL,
            Severity::Warning,
            ids(&["c1"]),
            strict.location()
        )],
        "{VAL_006}: {what}: not one SACM-ENUM-001 Warning on c1 at the attribute"
    );
    let doc = output.into_document();
    let c1 = element(VAL_006, what, &doc, "c1");
    assert_eq!(
        (c1.assertion_declaration(), c1.has_preserved_content()),
        (Some(AssertionDeclaration::Asserted), true),
        "{VAL_006}: {what}: not asserted with the value preserved"
    );
    let text = assert_round_trips(VAL_006, what, &doc);
    let tag = start_tag(VAL_006, &text, "c1");
    assert_eq!(
        (
            count(tag, "assertionDeclaration="),
            count(tag, "assertionDeclaration=\"Assumed\"")
        ),
        (1, 1),
        "{VAL_006}: {what}: the tolerant write does not give the attribute back once: {tag}"
    );

    // The element form: preserved whole, at its start tag.
    let what = "an assertionDeclaration element \"Assumed\"";
    let input = package(&claim(
        "c1",
        "",
        "<assertionDeclaration>Assumed</assertionDeclaration>",
    ));
    let strict = strict_refusal(VAL_006, what, &input);
    let output = tolerant(VAL_006, what, &input);
    let found = with_code(&output, Code::ENUM_INVALID_LITERAL);
    assert!(
        found.len() == 1
            && found[0].severity() == Severity::Warning
            && found[0].location() == strict.location(),
        "{VAL_006}: {what}: not one SACM-ENUM-001 Warning at the element: {:?}",
        output.diagnostics()
    );
    assert_eq!(
        output
            .document()
            .get("c1")
            .and_then(SacmElement::assertion_declaration),
        Some(AssertionDeclaration::Asserted),
        "{VAL_006}: {what}: not asserted"
    );
}

/// An input, the element holding the bad value, and what that element
/// holds.
type FlagCase = (
    &'static str,
    String,
    &'static str,
    fn(SacmElement<'_>) -> bool,
);

/// A flag that is no `xsd:boolean` reads as false, and a bad `xsi:nil`
/// reads as the property absent, each preserved with SACM-ENUM-001.
#[test]
fn sacm23_val_006_a_bad_flag_and_xsi_nil_are_preserved_in_tolerant_mode() {
    let cases: [FlagCase; 4] = [
        (
            "isCitation=\"yes\"",
            package(&claim("c1", " isCitation=\"yes\"", "")),
            "c1",
            |c1| !c1.is_citation(),
        ),
        (
            "isAbstract=\"\"",
            package(&claim("c1", " isAbstract=\"\"", "")),
            "c1",
            |c1| !c1.is_abstract(),
        ),
        (
            "isCounter=\"TRUE\" on an AssertedInference",
            package(&format!(
                "{}{}{}",
                claim("c1", "", ""),
                claim("c2", "", ""),
                inference("i1", " isCounter=\"TRUE\" source=\"c1\" target=\"c2\"")
            )),
            "i1",
            |i1| !i1.is_counter(),
        ),
        (
            "xsi:nil=\"yes\" on an isCitation element",
            package(&claim(
                "c1",
                "",
                "<isCitation xsi:nil=\"yes\">true</isCitation>",
            )),
            "c1",
            |c1| !c1.is_citation(),
        ),
    ];
    for (what, input, holder, holds) in &cases {
        let strict = strict_refusal(VAL_006, what, input);
        let output = tolerant(VAL_006, what, input);
        let found: Vec<(Code, Severity, Vec<ElementId>, Option<SourceLocation>)> = output
            .diagnostics()
            .iter()
            .map(|found| {
                (
                    found.code(),
                    found.severity(),
                    found.affected().to_vec(),
                    found.location(),
                )
            })
            .collect();
        assert_eq!(
            found,
            [(
                Code::ENUM_INVALID_LITERAL,
                Severity::Warning,
                ids(&[holder]),
                strict.location()
            )],
            "{VAL_006}: {what}: not one SACM-ENUM-001 Warning at the strict location"
        );
        let doc = output.into_document();
        let found = element(VAL_006, what, &doc, holder);
        assert!(
            holds(found) && found.has_preserved_content(),
            "{VAL_006}: {what}: not the default with the value preserved"
        );
        assert_round_trips(VAL_006, what, &doc);
    }
}

/// `SetAssertionDeclaration` and `SetFlag` remove a preserved attribute, in
/// no namespace, named after their property, whatever value they set, and
/// report the element changed.
#[test]
fn sacm23_val_006_set_assertion_declaration_and_set_flag_replace_the_preserved_attribute() {
    let read_doc = |what: &str, input: &str| tolerant(VAL_006, what, input).into_document();

    let what = "SetAssertionDeclaration(c1, Assumed) after \"Assumed\"";
    let mut doc = read_doc(
        what,
        &package(&claim("c1", " assertionDeclaration=\"Assumed\"", "")),
    );
    let mutation = match edit::apply(
        &mut doc,
        SetAssertionDeclaration::new("c1", AssertionDeclaration::Assumed),
    ) {
        Ok(mutation) => mutation,
        Err(error) => panic!("{VAL_006}: {what}: refused: {error:?}"),
    };
    assert_eq!(
        mutation.changed_ids(),
        ids(&["c1"]),
        "{VAL_006}: {what}: c1 is not reported changed"
    );
    let c1 = element(VAL_006, what, &doc, "c1");
    assert_eq!(
        (c1.assertion_declaration(), c1.has_preserved_content()),
        (Some(AssertionDeclaration::Assumed), false),
        "{VAL_006}: {what}: not assumed with nothing preserved"
    );
    assert!(
        !doc.has_preserved_content(),
        "{VAL_006}: {what}: the repaired document still holds preserved content"
    );
    let text = match write(&doc, Mode::Strict) {
        Ok(text) => text,
        Err(error) => {
            panic!("{VAL_006}: {what}: the repaired document does not write strictly: {error:?}")
        }
    };
    let tag = start_tag(VAL_006, &text, "c1");
    assert!(
        count(tag, "assertionDeclaration=") == 1
            && tag.contains("assertionDeclaration=\"assumed\""),
        "{VAL_006}: {what}: not assertionDeclaration=\"assumed\" once: {tag}"
    );

    // Setting the default removes it too: nothing is written.
    let what = "SetAssertionDeclaration(c1, Asserted) after \"Assumed\"";
    let mut doc = read_doc(
        what,
        &package(&claim("c1", " assertionDeclaration=\"Assumed\"", "")),
    );
    let mutation = edit::apply(
        &mut doc,
        SetAssertionDeclaration::new("c1", AssertionDeclaration::Asserted),
    )
    .unwrap_or_else(|error| panic!("{VAL_006}: {what}: refused: {error:?}"));
    assert_eq!(
        mutation.changed_ids(),
        ids(&["c1"]),
        "{VAL_006}: {what}: c1 is not reported changed"
    );
    let text = write_tolerant(VAL_006, what, &doc);
    assert!(
        !start_tag(VAL_006, &text, "c1").contains("assertionDeclaration"),
        "{VAL_006}: {what}: an assertionDeclaration is written\n{text}"
    );

    for (flag, attribute, value) in [
        (Flag::IsCitation, "isCitation", true),
        (Flag::IsAbstract, "isAbstract", false),
    ] {
        let what = format!("SetFlag(c1, {attribute}, {value}) after {attribute}=\"yes\"");
        let mut doc = read_doc(
            &what,
            &package(&claim("c1", &format!(" {attribute}=\"yes\""), "")),
        );
        let mutation = edit::apply(&mut doc, SetFlag::new("c1", flag, value))
            .unwrap_or_else(|error| panic!("{VAL_006}: {what}: refused: {error:?}"));
        assert_eq!(
            mutation.changed_ids(),
            ids(&["c1"]),
            "{VAL_006}: {what}: c1 is not reported changed"
        );
        assert!(
            !element(VAL_006, &what, &doc, "c1").has_preserved_content(),
            "{VAL_006}: {what}: the preserved attribute is kept"
        );
        let text = write_tolerant(VAL_006, &what, &doc);
        let tag = start_tag(VAL_006, &text, "c1");
        assert_eq!(
            count(tag, &format!("{attribute}=")),
            usize::from(value),
            "{VAL_006}: {what}: not the set value alone: {tag}"
        );
        assert!(
            !tag.contains("\"yes\""),
            "{VAL_006}: {what}: the old value is written: {tag}"
        );
    }

    // A namespaced attribute of the same local name is not the property.
    let what = "SetFlag(c1, isCitation, true) beside v:isCitation";
    let mut doc = read_doc(what, &package(&claim("c1", " v:isCitation=\"x\"", "")));
    edit::apply(&mut doc, SetFlag::new("c1", Flag::IsCitation, true))
        .unwrap_or_else(|error| panic!("{VAL_006}: {what}: refused: {error:?}"));
    assert!(
        element(VAL_006, what, &doc, "c1").has_preserved_content(),
        "{VAL_006}: {what}: the vendor attribute is removed"
    );
}

/// A bad value counts as the property's first occurrence, so a read never
/// gives a typed value next to a preserved attribute of the same name, and
/// a tolerant write never writes that attribute twice.
#[test]
fn sacm23_val_006_a_tolerant_write_never_emits_a_property_attribute_twice() {
    let what = "isAbstract=\"yes\" and an isAbstract element true";
    let input = package(&claim(
        "c1",
        " isAbstract=\"yes\"",
        "<isAbstract>true</isAbstract>",
    ));
    let strict = strict_refusal(VAL_006, what, &input);
    assert_eq!(
        strict.code(),
        Some(Code::ENUM_INVALID_LITERAL),
        "{VAL_006}: {what}: setup: the strict refusal is not the literal first"
    );
    let output = tolerant(VAL_006, what, &input);
    let literal = with_code(&output, Code::ENUM_INVALID_LITERAL);
    let repeat = with_code(&output, Code::MULTIPLICITY_VIOLATION);
    assert!(
        literal.len() == 1
            && literal[0].location() == strict.location()
            && repeat.len() == 1
            && repeat[0].location() > strict.location(),
        "{VAL_006}: {what}: not SACM-ENUM-001 at the attribute and SACM-MULT-001 at the \
         element: {:?}",
        output.diagnostics()
    );
    let doc = output.into_document();
    assert!(
        !element(VAL_006, what, &doc, "c1").is_abstract(),
        "{VAL_006}: {what}: the element's true is read"
    );
    let text = assert_round_trips(VAL_006, what, &doc);
    assert_eq!(
        count(start_tag(VAL_006, &text, "c1"), "isAbstract="),
        1,
        "{VAL_006}: {what}: isAbstract is not written once as an attribute\n{text}"
    );
    // After SetFlag, the typed value is written, once.
    let mut doc = doc;
    edit::apply(&mut doc, SetFlag::new("c1", Flag::IsAbstract, true))
        .unwrap_or_else(|error| panic!("{VAL_006}: {what}: SetFlag refused: {error:?}"));
    let text = write_tolerant(VAL_006, what, &doc);
    let tag = start_tag(VAL_006, &text, "c1");
    assert!(
        count(tag, "isAbstract=") == 1 && tag.contains("isAbstract=\"true\""),
        "{VAL_006}: {what}: after SetFlag, not isAbstract=\"true\" once: {tag}"
    );
}

/// A declaration attribute that is no literal counts as the first
/// occurrence, so a later declaration element is a repeat: the Assertion
/// holds `asserted`, and the element's value is never read.
#[test]
fn sacm23_val_006_a_bad_declaration_attribute_counts_as_the_first_occurrence() {
    let what = "assertionDeclaration=\"Assumed\" and an element axiomatic";
    let input = package(&claim(
        "c1",
        " assertionDeclaration=\"Assumed\"",
        "<assertionDeclaration>axiomatic</assertionDeclaration>",
    ));
    let strict = strict_refusal(VAL_006, what, &input);
    assert_eq!(
        strict.code(),
        Some(Code::ENUM_INVALID_LITERAL),
        "{VAL_006}: {what}: setup: the strict refusal is not the literal"
    );
    let output = tolerant(VAL_006, what, &input);
    let literal = with_code(&output, Code::ENUM_INVALID_LITERAL);
    let repeat = with_code(&output, Code::MULTIPLICITY_VIOLATION);
    assert!(
        literal.len() == 1
            && literal[0].location() == strict.location()
            && repeat.len() == 1
            && repeat[0].location() > strict.location(),
        "{VAL_006}: {what}: not SACM-ENUM-001 at the attribute and SACM-MULT-001 at the \
         element: {:?}",
        output.diagnostics()
    );
    assert_eq!(
        output.diagnostics().len(),
        2,
        "{VAL_006}: {what}: other diagnostics: {:?}",
        output.diagnostics()
    );
    let doc = output.into_document();
    assert_eq!(
        element(VAL_006, what, &doc, "c1").assertion_declaration(),
        Some(AssertionDeclaration::Asserted),
        "{VAL_006}: {what}: the element's axiomatic is read"
    );
    let text = assert_round_trips(VAL_006, what, &doc);
    assert_eq!(
        count(start_tag(VAL_006, &text, "c1"), "assertionDeclaration="),
        1,
        "{VAL_006}: {what}: assertionDeclaration is not written once as an attribute\n{text}"
    );
}
