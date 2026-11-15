//! Tests for preserved content on read, write and compare (LIB-COMPAT-003;
//! ADR 0033).
//!
//! Content that neither the SACM XML Schema nor the XMI 2.5.1 production
//! rules allow where it stands is kept, in both modes, as preserved content of
//! the element it sits in (its holder), with one SACM-XMI-003 per construct:
//! a Warning in tolerant mode, an Error in strict mode. These tests check:
//! - the ADR 0033 worked fixture: its eight diagnostics, its document, its
//!   validation, its exact tolerant output and its round trip;
//! - each class alone, on a root, a Claim, a `UtilityElement` and inside a
//!   `content`, `key` or `value`: the input with it differs from the input
//!   without it only by `[Difference::PreservedContent { holder }]`;
//! - position, values, text, namespaces and ids on read;
//! - `SacmElement::has_preserved_content` and `Difference::PreservedContent`;
//! - a 10,000-deep fragment.
//!
//! Strict writes are SACM23-XMI-008's, in `strict_save_preserved.rs`.
//! Preserved content under edits and validation (`with_id`, references,
//! delete, `NotEmpty`, positions after edits) is in `preserved_edits.rs`.
//!
//! "Without the construct" means without the whole child for a defect inside a
//! `name`, `content`, `key`, `value` or reference element (A3): the whole
//! child is preserved and the property is absent, so the input without that
//! child is the one that reads to the same typed document.

use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::Duration;

use sacm::compare::{Difference, semantic_compare};
use sacm::diagnostic::{Code, Diagnostic, Severity, SourceLocation};
use sacm::edit::{self, CreateArgumentPackage, CreateClaim};
use sacm::model::{Document, ElementId, ElementKind, LangString, SacmElement};
use sacm::validation;
use sacm::xmi::{self, Mode, ReadError, ReadOptions, ReadOutput, WriteOptions};

const ROW: &str = "LIB-COMPAT-003";
/// The pinned SACM 2.3 namespace.
const SACM_NS: &str = "http://www.omg.org/spec/SACM/20220301";
/// The XMI 2.5.1 namespace.
const XMI_NS: &str = "http://www.omg.org/spec/XMI/20131001";
const MODES: [Mode; 2] = [Mode::Strict, Mode::Tolerant];

/// The ADR 0033 worked fixture: classes (1) to (5), A3 and A5 in one
/// ArgumentPackage.
const WORKED: &[u8] =
    include_bytes!("data/sacm23/xmi/invalid/argument-package-unknown-content.sacm.xmi");

/// The tolerant write of [`WORKED`], exactly as the slice 0050 design gives it:
/// each holder or fragment declares the bindings it needs, and `sacm` and
/// `xmi` are declared on the document element only.
const WORKED_OUTPUT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301" xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="p">
  <name content="P" />
  <argumentElement xmi:type="sacm:Claim" xmi:id="c1" xmlns:v="urn:v" v:w="3" isCounter="true">
    <name content="C1" />
    <v:note xmlns:v="urn:v" v:by="ann">Because <v:em>tested</v:em></v:note>
  </argumentElement>
  <argumentElement xmlns:v="urn:v" xmi:type="sacm:Artifact" xmi:id="a1" />
  <argumentElement xmi:type="sacm:Claim" xmi:id="c2" citedElement="">
    <name xmlns:v="urn:v" content="C2" v:style="bold" />
    <abstractForm xmlns:v="urn:v" xmi:idref="c1" role="x" />
  </argumentElement>
  <argumentElement xmi:type="sacm:Claim" xmi:id="c3" isCitation="true" citedElement="a1">
    <name content="C3" />
    Draft: check &amp; see
  </argumentElement>
</sacm:ArgumentPackage>
"#;

/// A document element declaring `sacm`, `xmi` and `v`, with the id `p` and the
/// name `P`; the caller closes it with [`TAIL`].
const HEAD: &str = concat!(
    "<sacm:ArgumentPackage xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"\n",
    "    xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" xmlns:v=\"urn:v\" xmi:id=\"p\">\n",
    "  <name content=\"P\"/>\n",
);
/// The end of a document started with [`HEAD`].
const TAIL: &str = "</sacm:ArgumentPackage>\n";

/// Bounds a test that runs on its own thread.
const DEADLINE: Duration = Duration::from_secs(120);

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn read_in(mode: Mode, input: &[u8]) -> Result<ReadOutput, ReadError> {
    xmi::read(input, &ReadOptions::default().with_mode(mode))
}

/// Reads `input` in `mode`, which the test expects to be read.
fn read_ok(what: &str, mode: Mode, input: &[u8]) -> ReadOutput {
    match read_in(mode, input) {
        Ok(output) => output,
        Err(error) => panic!(
            "{ROW}: {what}: a {mode:?} read refused {:?}: {error} ({error:?})",
            String::from_utf8_lossy(input)
        ),
    }
}

/// Reads `input` tolerantly into a document.
fn doc_of(what: &str, input: &[u8]) -> Document {
    read_ok(what, Mode::Tolerant, input).into_document()
}

/// Reads `input` in `mode`, which the test expects to be refused.
fn read_err(what: &str, mode: Mode, input: &[u8]) -> ReadError {
    match read_in(mode, input) {
        Ok(output) => panic!(
            "{ROW}: {what}: a {mode:?} read accepted {:?}, with diagnostics {:?}",
            String::from_utf8_lossy(input),
            output.diagnostics()
        ),
        Err(error) => error,
    }
}

/// A tolerant write of `doc`, which the test expects to succeed.
fn write_tolerant(what: &str, doc: &Document) -> String {
    match xmi::write(doc, &WriteOptions::default().with_mode(Mode::Tolerant)) {
        Ok(text) => text,
        Err(error) => panic!("{ROW}: {what}: a tolerant write refused the document: {error}"),
    }
}

/// Writes `doc` tolerantly, reads the output back and writes it again: the
/// output reads to an equal document with no difference, and the second
/// write is byte-identical. Returns the output.
fn assert_round_trip(what: &str, doc: &Document) -> String {
    let out = write_tolerant(what, doc);
    let back = read_ok(&format!("{what}, written"), Mode::Tolerant, out.as_bytes()).into_document();
    assert_eq!(
        semantic_compare(doc, &back),
        [],
        "{ROW}: {what}: the written output reads to a different document:\n{out}"
    );
    assert!(
        &back == doc,
        "{ROW}: {what}: the written output does not read to an equal document:\n{out}"
    );
    let again = write_tolerant(&format!("{what}, rewritten"), &back);
    assert_eq!(
        again, out,
        "{ROW}: {what}: writing the read-back document is not byte-identical"
    );
    out
}

/// A 1-based location.
fn loc(line: u32, column: u32) -> SourceLocation {
    let Some(location) = SourceLocation::new(line, column) else {
        panic!("{ROW}: setup: SourceLocation::new({line}, {column}) refused a 1-based position");
    };
    location
}

/// The location of the one occurrence of `needle` in `text` (LF line ends,
/// columns in characters).
fn position_of(text: &str, needle: &str) -> SourceLocation {
    assert!(
        !text.contains('\r'),
        "{ROW}: setup: position_of counts LF line ends only"
    );
    let found: Vec<usize> = text.match_indices(needle).map(|(at, _)| at).collect();
    assert_eq!(
        found.len(),
        1,
        "{ROW}: setup: {needle:?} occurs {} times in {text:?}, not once",
        found.len()
    );
    let before = &text[..found[0]];
    let line = before.matches('\n').count() + 1;
    let column = before.rsplit('\n').next().unwrap_or("").chars().count() + 1;
    let (Ok(line), Ok(column)) = (u32::try_from(line), u32::try_from(column)) else {
        panic!("{ROW}: setup: the test input is too large to locate");
    };
    loc(line, column)
}

/// The severity of SACM-XMI-003 in `mode`.
fn unknown_severity(mode: Mode) -> Severity {
    match mode {
        Mode::Strict => Severity::Error,
        _ => Severity::Warning,
    }
}

/// A diagnostic without its message, requirement and clause.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Finding {
    code: Code,
    severity: Severity,
    location: Option<SourceLocation>,
    affected: Vec<String>,
}

fn finding(diagnostic: &Diagnostic) -> Finding {
    Finding {
        code: diagnostic.code(),
        severity: diagnostic.severity(),
        location: diagnostic.location(),
        affected: diagnostic
            .affected()
            .iter()
            .map(|id| id.as_str().to_owned())
            .collect(),
    }
}

fn findings(diagnostics: &[Diagnostic]) -> Vec<Finding> {
    diagnostics.iter().map(finding).collect()
}

/// The SACM-XMI-003 expected in `mode` at `location`, affecting `holder`.
fn unknown(mode: Mode, location: SourceLocation, holder: &str) -> Finding {
    Finding {
        code: Code::XMI_UNKNOWN_CONTENT,
        severity: unknown_severity(mode),
        location: Some(location),
        affected: vec![holder.to_owned()],
    }
}

fn id(value: &str) -> ElementId {
    ElementId::from(value)
}

fn ids(values: &[&str]) -> Vec<ElementId> {
    values.iter().copied().map(ElementId::from).collect()
}

/// The ids of `doc`'s elements, in `elements()` order.
fn element_ids(doc: &Document) -> Vec<String> {
    doc.elements()
        .map(|element| element.id().as_str().to_owned())
        .collect()
}

/// A document of [`HEAD`], `body` and [`TAIL`].
fn package(body: &str) -> String {
    format!("{HEAD}{body}{TAIL}")
}

/// Whether the element `holder` of `doc` holds preserved content.
fn holds(doc: &Document, holder: &str) -> bool {
    doc.get(holder)
        .unwrap_or_else(|| panic!("{ROW}: setup: no element {holder:?} was read"))
        .has_preserved_content()
}

// ---------------------------------------------------------------------------
// The codes
// ---------------------------------------------------------------------------

#[test]
fn lib_compat_003_the_unknown_content_and_preserved_target_codes_are_xmi_003_and_ref_003() {
    assert_eq!(
        Code::XMI_UNKNOWN_CONTENT.as_str(),
        "SACM-XMI-003",
        "{ROW}: Code::XMI_UNKNOWN_CONTENT is not SACM-XMI-003"
    );
    assert_eq!(
        Code::REF_PRESERVED_TARGET.as_str(),
        "SACM-REF-003",
        "{ROW}: Code::REF_PRESERVED_TARGET is not SACM-REF-003"
    );
}

// ---------------------------------------------------------------------------
// The worked fixture
// ---------------------------------------------------------------------------

/// The eight SACM-XMI-003 of [`WORKED`], in `mode`, in the reader's order.
fn worked_findings(mode: Mode) -> Vec<Finding> {
    [
        ((4, 54), "c1"),
        ((4, 62), "c1"),
        ((6, 5), "c1"),
        ((8, 3), "p"),
        ((9, 54), "c2"),
        ((10, 5), "c2"),
        ((11, 5), "c2"),
        ((14, 5), "c3"),
    ]
    .into_iter()
    .map(|((line, column), holder)| unknown(mode, loc(line, column), holder))
    .collect()
}

#[test]
fn lib_compat_003_the_worked_fixture_reads_in_both_modes_with_eight_unknown_content_diagnostics() {
    for mode in MODES {
        let output = read_ok("the worked fixture", mode, WORKED);
        assert_eq!(
            findings(output.diagnostics()),
            worked_findings(mode),
            "{ROW}: the worked fixture read in {mode:?} mode does not give the eight SACM-XMI-003 \
             at their locations, affecting their holders"
        );
    }
}

#[test]
fn lib_compat_003_the_worked_fixture_reads_to_the_same_document_in_both_modes() {
    let strict = read_ok("the worked fixture", Mode::Strict, WORKED).into_document();
    let tolerant = read_ok("the worked fixture", Mode::Tolerant, WORKED).into_document();
    assert_eq!(
        semantic_compare(&strict, &tolerant),
        [],
        "{ROW}: the strict and tolerant reads of the worked fixture differ"
    );
    assert!(
        strict == tolerant,
        "{ROW}: the strict and tolerant reads of the worked fixture are not equal"
    );
}

#[test]
fn lib_compat_003_the_worked_fixture_keeps_the_typed_document_without_its_preserved_parts() {
    for mode in MODES {
        let doc = read_ok("the worked fixture", mode, WORKED).into_document();
        assert_eq!(
            element_ids(&doc),
            ["p", "c1", "c2", "c3"],
            "{ROW}: {mode:?}: the worked fixture's elements are not p, c1, c2 and c3"
        );
        assert!(
            doc.get("a1").is_none() && !doc.contains("a1"),
            "{ROW}: {mode:?}: the preserved Artifact a1 is found as an element"
        );
        let c1 = doc.get("c1").expect("checked above");
        assert_eq!(
            c1.kind(),
            ElementKind::Claim,
            "{ROW}: {mode:?}: c1 is not a Claim"
        );
        assert!(
            !c1.is_counter(),
            "{ROW}: {mode:?}: c1's preserved isCounter was read"
        );
        assert_eq!(
            c1.name().and_then(LangString::content),
            Some("C1"),
            "{ROW}: {mode:?}: c1 lost its name"
        );
        let c2 = doc.get("c2").expect("checked above");
        assert!(
            c2.name().is_none(),
            "{ROW}: {mode:?}: c2's preserved name was read: {:?}",
            c2.name()
        );
        assert!(
            c2.abstract_form().is_none(),
            "{ROW}: {mode:?}: c2's preserved abstractForm was read: {:?}",
            c2.abstract_form()
        );
        assert!(
            c2.cited_element().is_none(),
            "{ROW}: {mode:?}: c2's preserved citedElement=\"\" was read: {:?}",
            c2.cited_element()
        );
        let c3 = doc.get("c3").expect("checked above");
        assert!(
            c3.is_citation(),
            "{ROW}: {mode:?}: c3's isCitation was not read"
        );
        assert_eq!(
            c3.cited_element(),
            Some(&id("a1")),
            "{ROW}: {mode:?}: c3's citedElement is not held as a1"
        );
        assert_eq!(
            c3.name().and_then(LangString::content),
            Some("C3"),
            "{ROW}: {mode:?}: c3 lost its name"
        );
    }
}

#[test]
fn lib_compat_003_the_worked_fixture_validates_with_c2_unnamed_and_a_preserved_target_from_c3() {
    let doc = doc_of("the worked fixture", WORKED);
    let found = validation::validate(&doc);
    let mut shown: Vec<(Code, Severity, Vec<String>)> = found
        .iter()
        .map(|diagnostic| {
            let finding = finding(diagnostic);
            (finding.code, finding.severity, finding.affected)
        })
        .collect();
    shown.sort_by(|a, b| a.0.as_str().cmp(b.0.as_str()).then_with(|| a.2.cmp(&b.2)));
    assert_eq!(
        shown,
        [
            (
                Code::MULTIPLICITY_VIOLATION,
                Severity::Error,
                vec!["c2".to_owned()]
            ),
            (
                Code::REF_PRESERVED_TARGET,
                Severity::Warning,
                vec!["c3".to_owned(), "a1".to_owned()]
            ),
        ],
        "{ROW}: validating the worked fixture does not give SACM-MULT-001 on c2 and SACM-REF-003 \
         [c3, a1] alone: {found:?}"
    );
    let preserved_target = found
        .iter()
        .find(|diagnostic| diagnostic.code() == Code::REF_PRESERVED_TARGET)
        .expect("checked above");
    assert_eq!(
        preserved_target.requirement_id(),
        ROW,
        "{ROW}: SACM-REF-003 does not cite LIB-COMPAT-003"
    );
    assert!(
        !found
            .iter()
            .any(|diagnostic| diagnostic.code() == Code::REF_DANGLING
                || diagnostic.code() == Code::REF_WRONG_TYPE),
        "{ROW}: a reference to a preserved id is reported as SACM-REF-001 or SACM-REF-002: {found:?}"
    );
}

#[test]
fn lib_compat_003_the_worked_fixture_writes_exactly_the_expected_text() {
    for mode in MODES {
        let doc = read_ok("the worked fixture", mode, WORKED).into_document();
        let out = write_tolerant("the worked fixture", &doc);
        assert_eq!(
            out, WORKED_OUTPUT,
            "{ROW}: the tolerant write of the worked fixture read in {mode:?} mode is not the \
             expected text"
        );
    }
}

#[test]
fn lib_compat_003_the_worked_fixture_round_trips_and_rewrites_byte_for_byte() {
    let doc = doc_of("the worked fixture", WORKED);
    let out = write_tolerant("the worked fixture", &doc);
    let back = doc_of("the worked fixture, written", out.as_bytes());
    assert_eq!(
        semantic_compare(&back, &doc),
        [],
        "{ROW}: read(write(read(fixture))) differs from read(fixture)"
    );
    assert!(
        back == doc,
        "{ROW}: read(write(read(fixture))) != read(fixture)"
    );
    assert_eq!(
        write_tolerant("the worked fixture, rewritten", &back),
        out,
        "{ROW}: write(read(out)) is not byte-identical to out"
    );
    // The output reads with the same eight constructs, now in the output's places.
    let again = read_ok(
        "the worked fixture, written",
        Mode::Tolerant,
        out.as_bytes(),
    );
    assert_eq!(
        again.diagnostics().len(),
        8,
        "{ROW}: the written worked fixture does not read with eight SACM-XMI-003: {:?}",
        again.diagnostics()
    );
    assert!(
        again
            .diagnostics()
            .iter()
            .all(|diagnostic| diagnostic.code() == Code::XMI_UNKNOWN_CONTENT),
        "{ROW}: the written worked fixture reads with another diagnostic: {:?}",
        again.diagnostics()
    );
}

// ---------------------------------------------------------------------------
// has_preserved_content
// ---------------------------------------------------------------------------

#[test]
fn lib_compat_003_has_preserved_content_is_true_exactly_for_holders() {
    let doc = doc_of("the worked fixture", WORKED);
    for element in doc.elements() {
        assert!(
            element.has_preserved_content(),
            "{ROW}: the worked fixture's holder {} has no preserved content",
            element.id()
        );
    }
    let input = package(concat!(
        "  <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\" v:w=\"1\">\n",
        "    <name content=\"C1\"/>\n",
        "    <note xmi:id=\"n1\"/>\n",
        "    <note xmi:id=\"n2\" v:w=\"2\"/>\n",
        "  </argumentElement>\n",
        "  <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c2\">\n",
        "    <name content=\"C2\"/>\n",
        "  </argumentElement>\n",
    ));
    let doc = doc_of("two holders", input.as_bytes());
    let holders: Vec<String> = doc
        .elements()
        .filter(|element| element.has_preserved_content())
        .map(|element| element.id().as_str().to_owned())
        .collect();
    assert_eq!(
        holders,
        ["c1", "n2"],
        "{ROW}: has_preserved_content is not true for c1 and n2 alone"
    );
    let back = doc_of(
        "two holders, written",
        write_tolerant("two holders", &doc).as_bytes(),
    );
    for element in back.elements() {
        assert_eq!(
            element.has_preserved_content(),
            ["c1", "n2"].contains(&element.id().as_str()),
            "{ROW}: after a round trip, has_preserved_content of {} changed",
            element.id()
        );
    }
    // A document built with edits holds none.
    let mut built = Document::new();
    if let Err(error) = edit::apply(
        &mut built,
        CreateArgumentPackage::root(LangString::new("P")).with_id("p"),
    ) {
        panic!("{ROW}: setup: create p: {error}");
    }
    if let Err(error) = edit::apply(
        &mut built,
        CreateClaim::new("p", LangString::new("C1")).with_id("c1"),
    ) {
        panic!("{ROW}: setup: create c1: {error}");
    }
    assert!(
        built
            .elements()
            .all(|element| !element.has_preserved_content()),
        "{ROW}: an edit-built element holds preserved content"
    );
}

// ---------------------------------------------------------------------------
// Each class alone
// ---------------------------------------------------------------------------

/// A document with a root `p`, a Claim `c1`, a Note `n1` with a `content` and a
/// `TaggedValue` `t1` with a `key` and a `content`. Each `@SLOT@` is filled by
/// [`fill`]; with the defaults it reads with no diagnostic.
const TEMPLATE: &str = r#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
    xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmlns:v="urn:v" xmi:id="p"@ROOT_ATTR@>
  @ROOT_NAME@@ROOT_CHILD@
  <argumentElement xmi:type="sacm:Claim" xmi:id="c1"@CLAIM_ATTR@>
    @CLAIM_NAME@@CLAIM_CHILD@
    <note xmi:id="n1"@NOTE_ATTR@>@NOTE_CHILD@
      @NOTE_CONTENT@
    </note>
    <taggedValue xmi:id="t1">
      @KEY@
      @TAGGED_CONTENT@
    </taggedValue>
  </argumentElement>
</sacm:ArgumentPackage>
"#;

/// The template's slots and their defaults.
const SLOTS: [(&str, &str); 11] = [
    ("ROOT_ATTR", ""),
    ("ROOT_NAME", "<name content=\"P\"/>"),
    ("ROOT_CHILD", ""),
    ("CLAIM_ATTR", ""),
    ("CLAIM_NAME", "<name content=\"C1\"/>"),
    ("CLAIM_CHILD", ""),
    ("NOTE_ATTR", ""),
    ("NOTE_CHILD", ""),
    ("NOTE_CONTENT", "<content><value content=\"N\"/></content>"),
    ("KEY", "<key><value content=\"K\"/></key>"),
    (
        "TAGGED_CONTENT",
        "<content><value content=\"V\"/></content>",
    ),
];

/// [`TEMPLATE`] with each slot set from `slots`, else its default.
fn fill(slots: &[(&str, &str)]) -> String {
    let mut text = TEMPLATE.to_owned();
    for (slot, default) in SLOTS {
        let value = slots
            .iter()
            .find(|(name, _)| *name == slot)
            .map_or(default, |(_, value)| *value);
        text = text.replace(&format!("@{slot}@"), value);
    }
    for (slot, _) in slots {
        assert!(
            SLOTS.iter().any(|(name, _)| name == slot),
            "{ROW}: setup: the template has no slot {slot}"
        );
    }
    text
}

/// One construct, alone in [`TEMPLATE`].
struct Case {
    /// What it is, for messages.
    what: &'static str,
    /// The element that holds it.
    holder: &'static str,
    /// The slots of the input with the construct.
    with: &'static [(&'static str, &'static str)],
    /// The slots of the input without it; empty for the defaults.
    without: &'static [(&'static str, &'static str)],
    /// The text the one SACM-XMI-003 is located at: once in the input.
    at: &'static str,
}

/// Checks `case`: in both modes the input with the construct reads with one
/// SACM-XMI-003 at it, affecting the holder, to the same document; that
/// document differs from the input without it only by
/// `[PreservedContent { holder }]`; only the holder holds preserved content;
/// and a tolerant write round-trips it.
fn assert_preserved_alone(row_case: &str, case: &Case) {
    let with = fill(case.with);
    let without = fill(case.without);
    let what = format!("{row_case}: {}", case.what);
    let plain = read_ok(&what, Mode::Tolerant, without.as_bytes());
    assert_eq!(
        plain.diagnostics(),
        [],
        "{ROW}: setup: {what}: the input without the construct has diagnostics"
    );
    let plain = plain.into_document();
    let at = position_of(&with, case.at);
    let mut documents = Vec::new();
    for mode in MODES {
        let output = read_ok(&what, mode, with.as_bytes());
        assert_eq!(
            findings(output.diagnostics()),
            [unknown(mode, at, case.holder)],
            "{ROW}: {what}: a {mode:?} read does not give one SACM-XMI-003 at {at}, affecting {}",
            case.holder
        );
        documents.push(output.into_document());
    }
    let tolerant = documents.pop().expect("two modes were read");
    let strict = documents.pop().expect("two modes were read");
    assert!(
        strict == tolerant,
        "{ROW}: {what}: the strict and tolerant reads are not equal"
    );
    assert_eq!(
        semantic_compare(&plain, &tolerant),
        [Difference::PreservedContent {
            id: id(case.holder)
        }],
        "{ROW}: {what}: the input with the construct does not differ from the input without it \
         only by the holder's preserved content"
    );
    assert!(
        plain != tolerant,
        "{ROW}: {what}: the inputs with and without the construct are equal"
    );
    for element in tolerant.elements() {
        assert_eq!(
            element.has_preserved_content(),
            element.id().as_str() == case.holder,
            "{ROW}: {what}: has_preserved_content of {} is wrong",
            element.id()
        );
    }
    assert_round_trip(&what, &tolerant);
}

fn assert_cases(row_case: &str, cases: &[Case]) {
    for case in cases {
        assert_preserved_alone(row_case, case);
    }
}

#[test]
fn lib_compat_003_class_1_an_unread_attribute_alone_differs_only_by_preserved_content() {
    assert_cases(
        "class 1",
        &[
            Case {
                what: "a foreign attribute on a root",
                holder: "p",
                with: &[("ROOT_ATTR", " v:w=\"r\"")],
                without: &[],
                at: "v:w=\"r\"",
            },
            Case {
                what: "an unknown unqualified attribute on a root",
                holder: "p",
                with: &[("ROOT_ATTR", " colour=\"red\"")],
                without: &[],
                at: "colour=\"red\"",
            },
            Case {
                what: "a foreign attribute on a Claim",
                holder: "c1",
                with: &[("CLAIM_ATTR", " v:w=\"c\"")],
                without: &[],
                at: "v:w=\"c\"",
            },
            Case {
                what: "isCounter on a Claim, which lacks it",
                holder: "c1",
                with: &[("CLAIM_ATTR", " isCounter=\"true\"")],
                without: &[],
                at: "isCounter=\"true\"",
            },
            Case {
                what: "xmi:version on a child",
                holder: "c1",
                with: &[("CLAIM_ATTR", " xmi:version=\"2.5.1\"")],
                without: &[],
                at: "xmi:version=\"2.5.1\"",
            },
            Case {
                what: "an attribute in the SACM namespace on a Claim",
                holder: "c1",
                with: &[("CLAIM_ATTR", " sacm:isCitation=\"true\"")],
                without: &[],
                at: "sacm:isCitation=\"true\"",
            },
            Case {
                what: "a foreign attribute on a UtilityElement",
                holder: "n1",
                with: &[("NOTE_ATTR", " v:w=\"n\"")],
                without: &[],
                at: "v:w=\"n\"",
            },
            Case {
                what: "metaClaim on a Note, which lacks it",
                holder: "n1",
                with: &[("NOTE_ATTR", " metaClaim=\"c1\"")],
                without: &[],
                at: "metaClaim=\"c1\"",
            },
            Case {
                what: "a foreign attribute on a content",
                holder: "n1",
                with: &[(
                    "NOTE_CONTENT",
                    "<content v:w=\"nc\"><value content=\"N\"/></content>",
                )],
                without: &[("NOTE_CONTENT", "")],
                at: "<content v:w=\"nc\">",
            },
            Case {
                what: "a foreign attribute on a key",
                holder: "t1",
                with: &[("KEY", "<key v:w=\"k\"><value content=\"K\"/></key>")],
                without: &[("KEY", "")],
                at: "<key v:w=\"k\">",
            },
            Case {
                what: "a foreign attribute on a value",
                holder: "t1",
                with: &[(
                    "TAGGED_CONTENT",
                    "<content><value content=\"V\" v:w=\"tv\"/></content>",
                )],
                without: &[("TAGGED_CONTENT", "")],
                at: "<content><value content=\"V\" v:w=\"tv\"/>",
            },
        ],
    );
}

#[test]
fn lib_compat_003_class_2_a_foreign_or_unknown_role_child_alone_differs_only_by_preserved_content()
{
    assert_cases(
        "class 2",
        &[
            Case {
                what: "a foreign child of a root",
                holder: "p",
                with: &[("ROOT_CHILD", "\n  <v:x/>")],
                without: &[],
                at: "<v:x/>",
            },
            Case {
                what: "a role element in the SACM namespace",
                holder: "p",
                with: &[(
                    "ROOT_CHILD",
                    "\n  <sacm:argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c9\"/>",
                )],
                without: &[],
                at: "<sacm:argumentElement",
            },
            Case {
                what: "a role a Claim lacks",
                holder: "c1",
                with: &[(
                    "CLAIM_CHILD",
                    "\n    <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c9\"/>",
                )],
                without: &[],
                at: "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c9\"/>",
            },
            Case {
                what: "a foreign child of a Claim",
                holder: "c1",
                with: &[("CLAIM_CHILD", "\n    <v:y/>")],
                without: &[],
                at: "<v:y/>",
            },
            Case {
                what: "a Description in a Note",
                holder: "n1",
                with: &[("NOTE_CHILD", "\n      <description xmi:id=\"d9\"/>")],
                without: &[],
                at: "<description",
            },
            Case {
                what: "a foreign child of a Note",
                holder: "n1",
                with: &[("NOTE_CHILD", "\n      <v:z/>")],
                without: &[],
                at: "<v:z/>",
            },
            Case {
                what: "a foreign element in a content",
                holder: "n1",
                with: &[(
                    "NOTE_CONTENT",
                    "<content><value content=\"N\"/><v:x/></content>",
                )],
                without: &[("NOTE_CONTENT", "")],
                at: "<content><value content=\"N\"/><v:x/>",
            },
            Case {
                what: "a foreign element in a key",
                holder: "t1",
                with: &[("KEY", "<key><value content=\"K\"/><v:k/></key>")],
                without: &[("KEY", "")],
                at: "<key>",
            },
            Case {
                what: "a foreign element in a value",
                holder: "t1",
                with: &[(
                    "TAGGED_CONTENT",
                    "<content><value content=\"V\"><v:x/></value></content>",
                )],
                without: &[("TAGGED_CONTENT", "")],
                at: "<content><value content=\"V\"><v:x/>",
            },
        ],
    );
}

#[test]
fn lib_compat_003_class_3_a_child_with_a_type_problem_alone_differs_only_by_preserved_content() {
    assert_cases(
        "class 3",
        &[
            Case {
                what: "an abstract type under argumentElement",
                holder: "p",
                with: &[(
                    "ROOT_CHILD",
                    "\n  <argumentElement xmi:type=\"sacm:Assertion\" xmi:id=\"x1\"/>",
                )],
                without: &[],
                at: "<argumentElement xmi:type=\"sacm:Assertion\"",
            },
            Case {
                what: "a type naming no SACM class",
                holder: "p",
                with: &[(
                    "ROOT_CHILD",
                    "\n  <argumentElement xmi:type=\"sacm:Nope\" xmi:id=\"x2\"/>",
                )],
                without: &[],
                at: "<argumentElement xmi:type=\"sacm:Nope\"",
            },
            Case {
                what: "an Artifact, which argumentElement does not admit",
                holder: "p",
                with: &[(
                    "ROOT_CHILD",
                    "\n  <argumentElement xmi:type=\"sacm:Artifact\" xmi:id=\"x3\"/>",
                )],
                without: &[],
                at: "<argumentElement xmi:type=\"sacm:Artifact\"",
            },
            Case {
                what: "two type attributes that disagree",
                holder: "p",
                with: &[(
                    "ROOT_CHILD",
                    "\n  <argumentElement xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" \
                     xmi:type=\"sacm:Claim\" xsi:type=\"sacm:ArgumentReasoning\" xmi:id=\"x4\"/>",
                )],
                without: &[],
                at: "<argumentElement xmlns:xsi",
            },
            Case {
                what: "a Claim under the note role of a Claim",
                holder: "c1",
                with: &[(
                    "CLAIM_CHILD",
                    "\n    <note xmi:type=\"sacm:Claim\" xmi:id=\"n9\"/>",
                )],
                without: &[],
                at: "<note xmi:type=\"sacm:Claim\"",
            },
            Case {
                what: "a content typed as another class",
                holder: "n1",
                with: &[(
                    "NOTE_CONTENT",
                    "<content xmi:type=\"sacm:LangString\"><value content=\"N\"/></content>",
                )],
                without: &[("NOTE_CONTENT", "")],
                at: "<content xmi:type=\"sacm:LangString\">",
            },
            Case {
                what: "a key typed as another class",
                holder: "t1",
                with: &[(
                    "KEY",
                    "<key xmi:type=\"sacm:Claim\"><value content=\"K\"/></key>",
                )],
                without: &[("KEY", "")],
                at: "<key xmi:type=\"sacm:Claim\">",
            },
            Case {
                what: "a value typed as another class",
                holder: "t1",
                with: &[(
                    "TAGGED_CONTENT",
                    "<content><value xmi:type=\"sacm:Claim\" content=\"V\"/></content>",
                )],
                without: &[("TAGGED_CONTENT", "")],
                at: "<content><value xmi:type=\"sacm:Claim\"",
            },
        ],
    );
}

#[test]
fn lib_compat_003_class_4_a_defect_inside_a_name_or_reference_element_preserves_the_whole_child() {
    assert_cases(
        "class 4",
        &[
            Case {
                what: "a foreign attribute on a root's name",
                holder: "p",
                with: &[("ROOT_NAME", "<name content=\"P\" v:w=\"pn\"/>")],
                without: &[("ROOT_NAME", "")],
                at: "<name content=\"P\" v:w=\"pn\"/>",
            },
            Case {
                what: "a foreign element in a Claim's name",
                holder: "c1",
                with: &[("CLAIM_NAME", "<name content=\"C1\"><v:x/></name>")],
                without: &[("CLAIM_NAME", "")],
                at: "<name content=\"C1\"><v:x/>",
            },
            Case {
                what: "an unknown attribute on a Claim's reference element",
                holder: "c1",
                with: &[(
                    "CLAIM_CHILD",
                    "\n    <abstractForm xmi:idref=\"p\" role=\"x\"/>",
                )],
                without: &[],
                at: "<abstractForm",
            },
            Case {
                what: "an element in a Note's reference element",
                holder: "n1",
                with: &[(
                    "NOTE_CHILD",
                    "\n      <citedElement xmi:idref=\"c1\"><v:x/></citedElement>",
                )],
                without: &[],
                at: "<citedElement",
            },
            Case {
                what: "text in a root's reference element",
                holder: "p",
                with: &[(
                    "ROOT_CHILD",
                    "\n  <citedElement xmi:idref=\"c1\">text</citedElement>",
                )],
                without: &[],
                at: "<citedElement",
            },
        ],
    );
}

#[test]
fn lib_compat_003_class_5_text_in_element_only_content_alone_differs_only_by_preserved_content() {
    assert_cases(
        "class 5",
        &[
            Case {
                what: "text in a root",
                holder: "p",
                with: &[("ROOT_CHILD", "\n  Stray root text")],
                without: &[],
                at: "Stray root text",
            },
            Case {
                what: "text in a Claim",
                holder: "c1",
                with: &[("CLAIM_CHILD", "\n    Stray claim text")],
                without: &[],
                at: "Stray claim text",
            },
            Case {
                what: "text beside the values of a content",
                holder: "n1",
                with: &[(
                    "NOTE_CONTENT",
                    "<content><value content=\"N\"/>stray</content>",
                )],
                without: &[("NOTE_CONTENT", "")],
                at: "<content><value content=\"N\"/>stray",
            },
            Case {
                what: "text beside the values of a key",
                holder: "t1",
                with: &[("KEY", "<key><value content=\"K\"/>stray</key>")],
                without: &[("KEY", "")],
                at: "<key>",
            },
        ],
    );
}

#[test]
fn lib_compat_003_a5_an_empty_cited_element_alone_differs_only_by_preserved_content() {
    assert_cases(
        "A5",
        &[
            Case {
                what: "citedElement=\"\" on a root",
                holder: "p",
                with: &[("ROOT_ATTR", " citedElement=\"\"")],
                without: &[],
                at: "citedElement=\"\"",
            },
            Case {
                what: "citedElement=\"\" on a Claim",
                holder: "c1",
                with: &[("CLAIM_ATTR", " citedElement=\"\"")],
                without: &[],
                at: "citedElement=\"\"",
            },
            Case {
                what: "citedElement=\"\" on a Note",
                holder: "n1",
                with: &[("NOTE_ATTR", " citedElement=\"\"")],
                without: &[],
                at: "citedElement=\"\"",
            },
        ],
    );
}

#[test]
fn lib_compat_003_a_root_type_naming_another_class_is_preserved_without_an_xsi_type_diagnostic() {
    let plain = concat!(
        "<sacm:ArgumentPackage xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"\n",
        "    xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" xmi:id=\"p\">\n",
        "  <name content=\"P\"/>\n",
        "</sacm:ArgumentPackage>\n",
    );
    let plain_doc = doc_of("a root", plain.as_bytes());
    for (what, attribute, written) in [
        (
            "an xmi:type",
            " xmi:type=\"sacm:Claim\"",
            "xmi:type=\"sacm:Claim\"",
        ),
        (
            "an xsi:type",
            " xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xsi:type=\"sacm:Claim\"",
            "xsi:type=\"sacm:Claim\"",
        ),
    ] {
        let input = plain.replacen(" xmi:id=\"p\">", &format!(" xmi:id=\"p\"{attribute}>"), 1);
        let at = position_of(&input, written);
        for mode in MODES {
            let output = read_ok(what, mode, input.as_bytes());
            assert_eq!(
                findings(output.diagnostics()),
                [unknown(mode, at, "p")],
                "{ROW}: a root with {what} naming a Claim, read in {mode:?} mode, does not give \
                 one SACM-XMI-003 at the attribute and no SACM-XMI-010"
            );
            let doc = output.into_document();
            assert_eq!(
                doc.get("p").map(SacmElement::kind),
                Some(ElementKind::ArgumentPackage),
                "{ROW}: a root <sacm:ArgumentPackage> with {what} naming a Claim is not read as \
                 an ArgumentPackage"
            );
            assert_eq!(
                semantic_compare(&plain_doc, &doc),
                [Difference::PreservedContent { id: id("p") }],
                "{ROW}: a root with {what} naming another class differs by more than its \
                 preserved content"
            );
        }
        let doc = doc_of(what, input.as_bytes());
        let out = assert_round_trip(what, &doc);
        assert!(
            out.contains(written),
            "{ROW}: the root's {what} is not written back:\n{out}"
        );
    }
    // A type naming the root's own class conforms: nothing is preserved.
    let own = plain.replacen(
        " xmi:id=\"p\">",
        " xmi:id=\"p\" xmi:type=\"sacm:ArgumentPackage\">",
        1,
    );
    let output = read_ok(
        "a root typed as its own class",
        Mode::Strict,
        own.as_bytes(),
    );
    assert_eq!(
        output.diagnostics(),
        [],
        "{ROW}: a root typed as its own class has a diagnostic"
    );
    assert!(
        !holds(output.document(), "p"),
        "{ROW}: a root typed as its own class holds preserved content"
    );
}

#[test]
fn lib_compat_003_a_preserved_part_is_never_a_repeated_value_occurrence() {
    for (what, body, holder) in [
        (
            "a preserved name before a typed name",
            concat!(
                "  <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\">\n",
                "    <name content=\"Old\" v:style=\"bold\"/>\n",
                "    <name content=\"C1\"/>\n",
                "  </argumentElement>\n",
            ),
            "c1",
        ),
        (
            "a preserved name after a typed name",
            concat!(
                "  <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\">\n",
                "    <name content=\"C1\"/>\n",
                "    <name content=\"Old\" v:style=\"bold\"/>\n",
                "  </argumentElement>\n",
            ),
            "c1",
        ),
    ] {
        for mode in MODES {
            let output = read_ok(what, mode, package(body).as_bytes());
            assert_eq!(
                output.diagnostics().len(),
                1,
                "{ROW}: {what}, {mode:?}: not one diagnostic: {:?}",
                output.diagnostics()
            );
            let doc = output.into_document();
            assert_eq!(
                doc.get(holder)
                    .and_then(SacmElement::name)
                    .and_then(LangString::content),
                Some("C1"),
                "{ROW}: {what}, {mode:?}: the typed name is not read"
            );
        }
    }
    // citedElement="" beside a citedElement element: the attribute is preserved.
    let body = concat!(
        "  <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\" citedElement=\"\">\n",
        "    <name content=\"C1\"/>\n",
        "    <citedElement xmi:idref=\"p\"/>\n",
        "  </argumentElement>\n",
    );
    for mode in MODES {
        let output = read_ok(
            "citedElement=\"\" and an element",
            mode,
            package(body).as_bytes(),
        );
        assert_eq!(
            output.diagnostics().len(),
            1,
            "{ROW}: citedElement=\"\" beside a citedElement element, {mode:?}: not one \
             diagnostic: {:?}",
            output.diagnostics()
        );
        assert_eq!(
            output
                .document()
                .get("c1")
                .and_then(SacmElement::cited_element),
            Some(&id("p")),
            "{ROW}: citedElement=\"\" beside a citedElement element, {mode:?}: the element form \
             is not read"
        );
    }
}

// ---------------------------------------------------------------------------
// Position
// ---------------------------------------------------------------------------

/// The byte offset of the one occurrence of `needle` in `out`.
fn offset_in(what: &str, out: &str, needle: &str) -> usize {
    let found: Vec<usize> = out.match_indices(needle).map(|(at, _)| at).collect();
    assert_eq!(
        found.len(),
        1,
        "{ROW}: {what}: {needle:?} is not written once:\n{out}"
    );
    found[0]
}

/// Asserts that `needles` are written once each, in this order.
fn assert_written_in_order(what: &str, out: &str, needles: &[&str]) {
    let offsets: Vec<usize> = needles
        .iter()
        .map(|needle| offset_in(what, out, needle))
        .collect();
    assert!(
        offsets.windows(2).all(|pair| pair[0] < pair[1]),
        "{ROW}: {what}: {needles:?} are not written in this order:\n{out}"
    );
}

#[test]
fn lib_compat_003_a_preserved_child_between_typed_children_is_written_between_them() {
    let body = concat!(
        "  <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"a\"/>\n",
        "  <v:between/>\n",
        "  <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"b\"/>\n",
    );
    let doc = doc_of("between", package(body).as_bytes());
    let out = assert_round_trip("between", &doc);
    assert_written_in_order(
        "a preserved child between a and b",
        &out,
        &["xmi:id=\"a\"", "<v:between", "xmi:id=\"b\""],
    );
}

#[test]
fn lib_compat_003_a_preserved_child_before_the_name_is_written_after_it() {
    let body = concat!(
        "  <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\">\n",
        "    <v:first/>\n",
        "    <name content=\"C1\"/>\n",
        "    <note xmi:id=\"n1\"/>\n",
        "  </argumentElement>\n",
    );
    let doc = doc_of("before the name", package(body).as_bytes());
    let out = assert_round_trip("before the name", &doc);
    assert_written_in_order(
        "a preserved child before the name",
        &out,
        &["<name content=\"C1\" />", "<v:first", "xmi:id=\"n1\""],
    );
}

#[test]
fn lib_compat_003_a_preserved_child_after_all_children_is_written_before_the_end_tag() {
    let body = concat!(
        "  <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"a\"/>\n",
        "  <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"b\"/>\n",
        "  <v:last/>\n",
    );
    let doc = doc_of("after all children", package(body).as_bytes());
    let out = assert_round_trip("after all children", &doc);
    assert_written_in_order(
        "a preserved child after all children",
        &out,
        &[
            "xmi:id=\"a\"",
            "xmi:id=\"b\"",
            "<v:last",
            "</sacm:ArgumentPackage>",
        ],
    );
    // In a Claim with no typed child, it is written after the name.
    let body = concat!(
        "  <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\">\n",
        "    <name content=\"C1\"/>\n",
        "    <v:last/>\n",
        "  </argumentElement>\n",
    );
    let doc = doc_of("after the name", package(body).as_bytes());
    let out = assert_round_trip("after the name", &doc);
    assert_written_in_order(
        "a preserved child after a Claim's name",
        &out,
        &["<name content=\"C1\" />", "<v:last", "</argumentElement>"],
    );
}

#[test]
fn lib_compat_003_preserved_attributes_follow_the_typed_ones_in_input_order() {
    let body = concat!(
        "  <argumentElement v:b=\"2\" xmi:type=\"sacm:Claim\" v:a=\"1\" xmi:id=\"c1\" ",
        "isCitation=\"true\" citedElement=\"p\">\n",
        "    <name content=\"C1\"/>\n",
        "  </argumentElement>\n",
    );
    let doc = doc_of("attribute order", package(body).as_bytes());
    let out = assert_round_trip("attribute order", &doc);
    assert!(
        out.contains(
            "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\" isCitation=\"true\" \
             citedElement=\"p\" xmlns:v=\"urn:v\" v:b=\"2\" v:a=\"1\">\n"
        ),
        "{ROW}: the preserved attributes are not written after the typed ones, in input \
         order:\n{out}"
    );
}

// ---------------------------------------------------------------------------
// Values, text and namespaces
// ---------------------------------------------------------------------------

#[test]
fn lib_compat_003_attribute_values_and_fragment_text_survive_exactly() {
    let body = concat!(
        "  <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\" ",
        "v:w=\"TAB&#9;LF&#10;CR&#13;lt&lt;gt&gt;amp&amp;quot&quot;\u{e9}\u{2713}\">\n",
        "    <name content=\"C1\"/>\n",
        "    <v:t v:a=\"&#9;&#10;&#13;&lt;&amp;&quot;\u{5b89}\">",
        "TAB\tLF\nCR&#13;lt&lt;gt&gt;amp&amp;quot\"\u{e9}\u{2713}</v:t>\n",
        "  </argumentElement>\n",
    );
    let doc = doc_of("exact values", package(body).as_bytes());
    let out = assert_round_trip("exact values", &doc);
    assert!(
        out.contains("v:w=\"TAB&#9;LF&#10;CR&#13;lt&lt;gt&gt;amp&amp;quot&quot;\u{e9}\u{2713}\""),
        "{ROW}: a preserved attribute value is not written with the writer's escapes:\n{out}"
    );
    assert!(
        out.contains("v:a=\"&#9;&#10;&#13;&lt;&amp;&quot;\u{5b89}\""),
        "{ROW}: a fragment attribute value is not written with the writer's escapes:\n{out}"
    );
    assert!(
        out.contains("TAB\tLF\nCR"),
        "{ROW}: fragment text does not keep its TAB and LF as written:\n{out}"
    );
    assert!(
        out.contains("lt&lt;gt&gt;amp&amp;quot\"\u{e9}\u{2713}</v:t>"),
        "{ROW}: fragment text does not escape & < > and keep the rest:\n{out}"
    );
    assert!(
        !out.contains('\r'),
        "{ROW}: a CR is written raw, so a reader reads it as LF:\n{out:?}"
    );
    // The values differ from the same input with the CR as an LF.
    let changed = body.replacen("CR&#13;lt", "CR&#10;lt", 1);
    let other = doc_of("exact values, CR as LF", package(&changed).as_bytes());
    assert_eq!(
        semantic_compare(&doc, &other),
        [Difference::PreservedContent { id: id("c1") }],
        "{ROW}: a CR and an LF in a preserved value compare equal"
    );
}

#[test]
fn lib_compat_003_a_prefix_declared_on_an_ancestor_is_declared_by_the_holder() {
    let body = concat!(
        "  <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\" v:w=\"1\">\n",
        "    <name content=\"C1\"/>\n",
        "  </argumentElement>\n",
    );
    let doc = doc_of("an ancestor's prefix", package(body).as_bytes());
    let out = assert_round_trip("an ancestor's prefix", &doc);
    assert!(
        out.contains(
            "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\" xmlns:v=\"urn:v\" v:w=\"1\">"
        ),
        "{ROW}: the holder does not declare the prefix its preserved attribute uses:\n{out}"
    );
    let document_element = document_element_tag(&out);
    assert!(
        !document_element.contains("urn:v"),
        "{ROW}: the document element declares a binding only preserved content needs: \
         {document_element}"
    );
}

/// The document element's start tag in `out`, after the XML declaration.
fn document_element_tag(out: &str) -> &str {
    let after_declaration = out.find("?>").map_or(0, |at| at + 2);
    let rest = &out[after_declaration..];
    let start = rest.find('<').unwrap_or(0);
    let rest = &rest[start..];
    &rest[..=rest.find('>').unwrap_or(0)]
}

#[test]
fn lib_compat_003_sacm_rebound_in_a_fragment_is_kept_and_sacm_and_xmi_are_declared_on_the_document_element_only()
 {
    let body = concat!(
        "  <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\">\n",
        "    <name content=\"C1\"/>\n",
        "    <v:s xmlns:sacm=\"urn:other\"><sacm:Claim sacm:x=\"1\"/></v:s>\n",
        "    <v:r><v:in xmlns:sacm=\"urn:inner\"><sacm:Claim/></v:in></v:r>\n",
        "  </argumentElement>\n",
    );
    let doc = doc_of("sacm rebound", package(body).as_bytes());
    let out = assert_round_trip("sacm rebound", &doc);
    assert_eq!(
        out.matches(&format!("xmlns:sacm=\"{SACM_NS}\"")).count(),
        1,
        "{ROW}: the SACM namespace is declared more than once:\n{out}"
    );
    assert_eq!(
        out.matches(&format!("xmlns:xmi=\"{XMI_NS}\"")).count(),
        1,
        "{ROW}: the XMI namespace is declared more than once:\n{out}"
    );
    let tag = document_element_tag(&out);
    assert!(
        tag.contains(&format!("xmlns:sacm=\"{SACM_NS}\""))
            && tag.contains(&format!("xmlns:xmi=\"{XMI_NS}\"")),
        "{ROW}: the document element does not declare sacm and xmi: {tag}"
    );
    assert!(
        out.contains("xmlns:sacm=\"urn:other\"") && out.contains("xmlns:sacm=\"urn:inner\""),
        "{ROW}: a fragment's rebinding of sacm is not written:\n{out}"
    );
    // The rebinding is part of the content: the same fragment in the SACM
    // namespace is a different fragment.
    let changed = body.replacen(" xmlns:sacm=\"urn:other\"", "", 1);
    let other = doc_of("sacm not rebound", package(&changed).as_bytes());
    assert_eq!(
        semantic_compare(&doc, &other),
        [Difference::PreservedContent { id: id("c1") }],
        "{ROW}: a fragment with sacm rebound and one without compare equal"
    );
}

#[test]
fn lib_compat_003_a_default_namespace_and_its_undeclaration_survive() {
    let body = concat!(
        "  <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\">\n",
        "    <name content=\"C1\"/>\n",
        "    <d:x xmlns:d=\"urn:d\" xmlns=\"urn:default\"><y/><z xmlns=\"\"/></d:x>\n",
        "    <plain/>\n",
        "  </argumentElement>\n",
    );
    let doc = doc_of("a default namespace", package(body).as_bytes());
    let out = assert_round_trip("a default namespace", &doc);
    assert!(
        out.contains("xmlns=\"urn:default\""),
        "{ROW}: a fragment's default namespace is not written:\n{out}"
    );
    // Each change of namespace is a difference: `z` in no namespace is not
    // `z` in the default one, and `y` in the default one is not `y` in none.
    for (what, from, to) in [
        ("z without its undeclaration", "<z xmlns=\"\"/>", "<z/>"),
        ("no default namespace", " xmlns=\"urn:default\"", ""),
    ] {
        let changed = body.replacen(from, to, 1);
        let other = doc_of(what, package(&changed).as_bytes());
        assert_eq!(
            semantic_compare(&doc, &other),
            [Difference::PreservedContent { id: id("c1") }],
            "{ROW}: {what} compares equal to the original"
        );
    }
}

#[test]
fn lib_compat_003_a_binding_in_scope_but_used_only_in_a_value_is_declared_on_the_fragment() {
    let input = concat!(
        "<sacm:ArgumentPackage xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"\n",
        "    xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" xmlns:v=\"urn:v\" ",
        "xmlns:u=\"urn:u\" xmi:id=\"p\">\n",
        "  <name content=\"P\"/>\n",
        "  <v:q ref=\"u:thing\"/>\n",
        "</sacm:ArgumentPackage>\n",
    );
    let doc = doc_of("a QName value", input.as_bytes());
    let out = assert_round_trip("a QName value", &doc);
    let start = offset_in("a QName value", &out, "<v:q");
    let tag = &out[start..];
    let tag = &tag[..tag.find('>').unwrap_or(0)];
    assert!(
        tag.contains("xmlns:u=\"urn:u\"") && tag.contains("xmlns:v=\"urn:v\""),
        "{ROW}: a top fragment does not declare every binding in scope: {tag}"
    );
    assert!(
        !tag.contains("xmlns:sacm") && !tag.contains("xmlns:xmi"),
        "{ROW}: a top fragment re-declares sacm or xmi: {tag}"
    );
}

#[test]
fn lib_compat_003_top_level_text_is_trimmed_and_fragment_text_is_kept_exactly() {
    let body = concat!(
        "  <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\">\n",
        "    \t Draft  text \n",
        "    <name content=\"C1\"/>\n",
        "    <v:x>\n      <v:y/>\n    </v:x>\n",
        "    <v:t>  spaced  </v:t>\n",
        "  </argumentElement>\n",
    );
    let doc = doc_of("text", package(body).as_bytes());
    let out = assert_round_trip("text", &doc);
    assert!(
        out.contains("\n    Draft  text\n"),
        "{ROW}: top-level text is not written trimmed, on its own line:\n{out}"
    );
    assert!(
        out.contains("<v:x xmlns:v=\"urn:v\">\n      <v:y") && out.contains("\n    </v:x>"),
        "{ROW}: the white space inside a fragment is not kept exactly:\n{out}"
    );
    assert!(
        out.contains("<v:t xmlns:v=\"urn:v\">  spaced  </v:t>"),
        "{ROW}: fragment text is trimmed:\n{out}"
    );
    // Top-level text differs from the same text with other white space around
    // it in nothing; fragment text does.
    let respaced = body.replacen("    \t Draft  text \n", "Draft  text", 1);
    let other = doc_of("text, respaced", package(&respaced).as_bytes());
    assert_eq!(
        semantic_compare(&doc, &other),
        [],
        "{ROW}: top-level text is not trimmed of XML white space"
    );
    let respaced = body.replacen("  spaced  ", "spaced", 1);
    let other = doc_of("fragment text, respaced", package(&respaced).as_bytes());
    assert_eq!(
        semantic_compare(&doc, &other),
        [Difference::PreservedContent { id: id("c1") }],
        "{ROW}: fragment text is not kept exactly"
    );
}

#[test]
fn lib_compat_003_top_level_text_split_by_a_comment_round_trips() {
    let body = concat!(
        "  <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\">\n",
        "    one <!-- dropped --> two\n",
        "    <name content=\"C1\"/>\n",
        "  </argumentElement>\n",
    );
    let doc = doc_of("split text", package(body).as_bytes());
    assert!(
        holds(&doc, "c1"),
        "{ROW}: split top-level text is not preserved"
    );
    let out = assert_round_trip("split text", &doc);
    assert!(
        !out.contains("<!--"),
        "{ROW}: a comment is written back:\n{out}"
    );
}

#[test]
fn lib_compat_003_cdata_is_merged_and_comments_and_processing_instructions_are_dropped() {
    let body = concat!(
        "  <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\">\n",
        "    <name content=\"C1\"/>\n",
        "    <v:t>a<![CDATA[<b>&]]>c</v:t>\n",
        "    <v:u>a<!--x-->b<?pi d?>c</v:u>\n",
        "  </argumentElement>\n",
    );
    let doc = doc_of("CDATA and comments", package(body).as_bytes());
    let out = assert_round_trip("CDATA and comments", &doc);
    assert!(
        out.contains("<v:t xmlns:v=\"urn:v\">a&lt;b&gt;&amp;c</v:t>"),
        "{ROW}: a CDATA section is not merged into the text and escaped:\n{out}"
    );
    assert!(
        out.contains("<v:u xmlns:v=\"urn:v\">abc</v:u>"),
        "{ROW}: the text around a comment and a processing instruction is not joined:\n{out}"
    );
    assert!(
        !out.contains("<!--") && !out.contains("<?pi"),
        "{ROW}: a comment or processing instruction in preserved content is written:\n{out}"
    );
    // Comments and processing instructions are not content.
    let plain = body.replacen("a<!--x-->b<?pi d?>c", "abc", 1);
    let other = doc_of("no comments", package(&plain).as_bytes());
    assert!(
        semantic_compare(&doc, &other).is_empty() && doc == other,
        "{ROW}: a comment or processing instruction in preserved content makes a difference"
    );
}

// ---------------------------------------------------------------------------
// Ids on read
// ---------------------------------------------------------------------------

#[test]
fn lib_compat_003_a_preserved_id_repeating_a_typed_one_is_a_duplicate_id_naming_both() {
    let claim = "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"/>";
    for (what, body, repeated, first, second) in [
        (
            "a preserved element after the typed one",
            format!("  {claim}\n  <v:x xmi:id=\"c1\"/>\n"),
            "c1",
            claim,
            "<v:x",
        ),
        (
            "a preserved element before the typed one",
            format!("  <v:x xmi:id=\"c1\"/>\n  {claim}\n"),
            "c1",
            "<v:x",
            claim,
        ),
        (
            "an element deep in a fragment",
            format!("  {claim}\n  <v:x><v:y><v:z xmi:id=\"c1\"/></v:y></v:x>\n"),
            "c1",
            claim,
            "<v:z",
        ),
        (
            "a role child with a type the role does not admit",
            format!("  {claim}\n  <argumentElement xmi:type=\"sacm:Artifact\" xmi:id=\"c1\"/>\n"),
            "c1",
            claim,
            "<argumentElement xmi:type=\"sacm:Artifact\"",
        ),
        (
            "the id of a name",
            "  <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\">\
             <name xmi:id=\"nm\" content=\"C1\"/></argumentElement>\n  <v:x xmi:id=\"nm\"/>\n"
                .to_owned(),
            "nm",
            "<name xmi:id=\"nm\"",
            "<v:x",
        ),
        (
            "two preserved elements",
            "  <v:x xmi:id=\"z\"/>\n  <v:y xmi:id=\"z\"/>\n".to_owned(),
            "z",
            "<v:x",
            "<v:y",
        ),
    ] {
        let input = package(&body);
        let expected = vec![position_of(&input, first), position_of(&input, second)];
        for mode in MODES {
            let error = read_err(what, mode, input.as_bytes());
            assert!(
                matches!(&error, ReadError::DuplicateId { id, .. } if id.as_str() == repeated),
                "{ROW}: {what}, {mode:?}: not DuplicateId of {repeated:?}: {error:?}"
            );
            assert_eq!(
                error.code(),
                Some(Code::ID_DUPLICATE),
                "{ROW}: {what}, {mode:?}: the refusal is not SACM-ID-001"
            );
            assert_eq!(
                error.locations(),
                expected.as_slice(),
                "{ROW}: {what}, {mode:?}: the refusal does not name both carriers"
            );
        }
    }
}

#[test]
fn lib_compat_003_an_empty_preserved_id_is_an_invalid_id() {
    for (what, body, at) in [
        ("a foreign element", "  <v:x xmi:id=\"\"/>\n", "<v:x"),
        (
            "an element deep in a fragment",
            "  <v:x><v:y xmi:id=\"\"/></v:x>\n",
            "<v:y",
        ),
        (
            "a role child with a type the role does not admit",
            "  <argumentElement xmi:type=\"sacm:Artifact\" xmi:id=\"\"/>\n",
            "<argumentElement",
        ),
    ] {
        let input = package(body);
        let expected = position_of(&input, at);
        for mode in MODES {
            let error = read_err(what, mode, input.as_bytes());
            assert!(
                matches!(&error, ReadError::InvalidId { id, .. } if id.is_empty()),
                "{ROW}: {what}, {mode:?}: an empty preserved xmi:id is not InvalidId: {error:?}"
            );
            assert_eq!(
                (error.code(), error.location()),
                (Some(Code::ID_INVALID), Some(expected)),
                "{ROW}: {what}, {mode:?}: the refusal is not SACM-ID-002 at the element"
            );
        }
    }
}

#[test]
fn lib_compat_003_nothing_inside_a_preserved_subtree_is_read_or_reported() {
    let body = concat!(
        "  <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\">\n",
        "    <name content=\"C1\"/>\n",
        "    <v:x>\n",
        "      <argumentElement xmi:type=\"sacm:Claim\"/>\n",
        "      <argumentElement xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" ",
        "xsi:type=\"sacm:Claim\" xmi:id=\"c7\"/>\n",
        "      <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c8\" xmi:version=\"1\" ",
        "isCitation=\"maybe\" assertionDeclaration=\"Assumed\"/>\n",
        "      <argumentElement xmi:id=\"c9\"/>\n",
        "      <name content=\"a\"/><name content=\"b\"/>\n",
        "      <name>legacy text</name>\n",
        "      <argumentElement xmi:type=\"sacm:ArgumentPackageInterface\" xmi:id=\"i1\"/>\n",
        "      <xmi:Extension extender=\"v\"/>\n",
        "      <v:y xmi:uuid=\"u1\" citedElement=\"c404\"/>\n",
        "    </v:x>\n",
        "  </argumentElement>\n",
    );
    let input = package(body);
    let at = position_of(&input, "<v:x>");
    for mode in MODES {
        let output = read_ok("a preserved subtree", mode, input.as_bytes());
        assert_eq!(
            findings(output.diagnostics()),
            [unknown(mode, at, "c1")],
            "{ROW}: {mode:?}: something inside a preserved subtree is read or reported"
        );
        let doc = output.into_document();
        assert_eq!(
            element_ids(&doc),
            ["p", "c1"],
            "{ROW}: {mode:?}: an element inside a preserved subtree is read"
        );
        let found = validation::validate(&doc);
        assert!(
            !found
                .iter()
                .any(|diagnostic| diagnostic.code() == Code::REF_DANGLING),
            "{ROW}: {mode:?}: a reference inside a preserved subtree is validated: {found:?}"
        );
    }
}

#[test]
fn lib_compat_003_generated_ids_on_read_skip_preserved_ids_and_preserved_references() {
    let body = concat!(
        "  <argumentElement xmi:type=\"sacm:Claim\"><name content=\"A\"/></argumentElement>\n",
        "  <v:a xmi:id=\"claim_1\"/>\n",
        "  <v:b xmi:idref=\"claim_2\"/>\n",
        "  <v:c href=\"#claim_3\"/>\n",
        "  <v:d citedElement=\"claim_4\"/>\n",
    );
    let output = read_ok("generated ids", Mode::Tolerant, package(body).as_bytes());
    let generated: Vec<String> = output
        .diagnostics()
        .iter()
        .filter(|diagnostic| diagnostic.code() == Code::XMI_MISSING_ID)
        .flat_map(|diagnostic| {
            diagnostic
                .affected()
                .iter()
                .map(|id| id.as_str().to_owned())
        })
        .collect();
    assert_eq!(
        generated,
        ["claim_5"],
        "{ROW}: the generated id does not skip the preserved id and the ids preserved \
         references name"
    );
    assert_eq!(
        output.document().get("claim_5").map(SacmElement::kind),
        Some(ElementKind::Claim),
        "{ROW}: the Claim without an xmi:id is not read as claim_5"
    );
}

// ---------------------------------------------------------------------------
// Difference::PreservedContent
// ---------------------------------------------------------------------------

#[test]
fn lib_compat_003_a_changed_prefix_attribute_order_value_or_index_is_one_preserved_content_difference()
 {
    let claims = concat!(
        "  <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"a\"/>\n",
        "  <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"b\"/>\n",
    );
    for (what, holder, left, right) in [
        (
            "a prefix",
            "p",
            format!("{claims}  <v:x xmlns:v=\"urn:v\"/>\n"),
            format!("{claims}  <w:x xmlns:w=\"urn:v\"/>\n"),
        ),
        (
            "the attribute order",
            "p",
            format!("{claims}  <v:x v:a=\"1\" v:b=\"2\"/>\n"),
            format!("{claims}  <v:x v:b=\"2\" v:a=\"1\"/>\n"),
        ),
        (
            "a value",
            "p",
            format!("{claims}  <v:x v:a=\"1\"/>\n"),
            format!("{claims}  <v:x v:a=\"2\"/>\n"),
        ),
        (
            "a preserved attribute's value",
            "a",
            claims.replacen("xmi:id=\"a\"/>", "xmi:id=\"a\" v:w=\"1\"/>", 1),
            claims.replacen("xmi:id=\"a\"/>", "xmi:id=\"a\" v:w=\"2\"/>", 1),
        ),
        (
            "the index",
            "p",
            claims.replacen("\n  <argumentElement", "\n  <v:x/>\n  <argumentElement", 1),
            format!("{claims}  <v:x/>\n"),
        ),
    ] {
        let left_doc = doc_of(what, package(&left).as_bytes());
        let right_doc = doc_of(what, package(&right).as_bytes());
        assert_eq!(
            semantic_compare(&left_doc, &right_doc),
            [Difference::PreservedContent { id: id(holder) }],
            "{ROW}: a changed {what} is not one preserved-content difference"
        );
        assert_eq!(
            semantic_compare(&right_doc, &left_doc),
            [Difference::PreservedContent { id: id(holder) }],
            "{ROW}: a changed {what}, swapped, is not one preserved-content difference"
        );
        assert!(
            left_doc != right_doc,
            "{ROW}: documents with a changed {what} are equal"
        );
        assert!(
            left_doc == left_doc.clone(),
            "{ROW}: a document with preserved content is not equal to its clone"
        );
    }
}

#[test]
fn lib_compat_003_a_preserved_content_difference_names_and_displays_its_element() {
    let difference = Difference::PreservedContent { id: id("c1") };
    assert_eq!(
        difference.element_id(),
        Some(&id("c1")),
        "{ROW}: Difference::PreservedContent does not name its element"
    );
    assert_eq!(
        difference.to_string(),
        "element \"c1\" differs in its preserved content",
        "{ROW}: Difference::PreservedContent's Display is not the pinned text"
    );
}

#[test]
fn lib_compat_003_a_preserved_content_difference_comes_last_for_its_element() {
    let left = concat!(
        "<sacm:ArgumentPackage xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"\n",
        "    xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" xmlns:v=\"urn:v\" ",
        "xmi:id=\"p\" v:w=\"1\">\n",
        "  <name content=\"P\"/>\n",
        "  <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"a\"/>\n",
        "  <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"b\" v:w=\"3\"/>\n",
        "</sacm:ArgumentPackage>\n",
    );
    let right = concat!(
        "<sacm:ArgumentPackage xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"\n",
        "    xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" xmlns:v=\"urn:v\" ",
        "xmi:id=\"p\" v:w=\"2\">\n",
        "  <name content=\"Q\"/>\n",
        "  <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"b\" v:w=\"4\"/>\n",
        "  <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"a\"/>\n",
        "</sacm:ArgumentPackage>\n",
    );
    let left = doc_of("left", left.as_bytes());
    let right = doc_of("right", right.as_bytes());
    assert_eq!(
        semantic_compare(&left, &right),
        [
            Difference::Name {
                id: id("p"),
                left: Some(LangString::new("P")),
                right: Some(LangString::new("Q")),
            },
            Difference::ChildOrder {
                id: id("p"),
                left: ids(&["a", "b"]),
                right: ids(&["b", "a"]),
            },
            Difference::PreservedContent { id: id("p") },
            Difference::PreservedContent { id: id("b") },
        ],
        "{ROW}: a preserved-content difference is not reported last for its element, after \
         ChildOrder"
    );
}

// ---------------------------------------------------------------------------
// A deep fragment
// ---------------------------------------------------------------------------

#[test]
fn lib_compat_003_a_ten_thousand_deep_fragment_reads_clones_compares_writes_and_drops() {
    // A thread with an explicit 512 KiB stack: 10,000 recursive frames of a
    // read, clone, comparison, write or drop of a nested tree do not fit in it,
    // and a stack overflow aborts the test binary, which fails the run.
    const DEPTH: usize = 10_000;
    const STACK: usize = 512 * 1024;
    let fragment = |innermost: &str| {
        format!(
            "{}{innermost}{}",
            "<v:x>".repeat(DEPTH - 1),
            "</v:x>".repeat(DEPTH - 1)
        )
    };
    let input = package(&format!(
        "  <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"><name content=\"C1\"/>{}\
         </argumentElement>\n",
        fragment("<v:x/>")
    ));
    let changed = package(&format!(
        "  <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"><name content=\"C1\"/>{}\
         </argumentElement>\n",
        fragment("<v:y/>")
    ));
    let (sender, receiver) = mpsc::channel();
    std::thread::Builder::new()
        .name("lib_compat_003_deep_fragment".to_owned())
        .stack_size(STACK)
        .spawn(move || {
            let result = (|| -> Result<(), String> {
                let options = ReadOptions::default();
                let doc = xmi::read(input.as_bytes(), &options)
                    .map_err(|error| format!("the read refused it: {error}"))?
                    .into_document();
                if !doc
                    .get("c1")
                    .is_some_and(SacmElement::has_preserved_content)
                {
                    return Err("c1 holds no preserved content".to_owned());
                }
                let clone = doc.clone();
                if clone != doc || !semantic_compare(&doc, &clone).is_empty() {
                    return Err("the clone is not equal".to_owned());
                }
                let other = xmi::read(changed.as_bytes(), &options)
                    .map_err(|error| {
                        format!("the read of the changed fragment refused it: {error}")
                    })?
                    .into_document();
                let found = semantic_compare(&doc, &other);
                if found
                    != [Difference::PreservedContent {
                        id: ElementId::from("c1"),
                    }]
                {
                    return Err(format!("the changed innermost element gives {found:?}"));
                }
                let out = xmi::write(&doc, &WriteOptions::default().with_mode(Mode::Tolerant))
                    .map_err(|error| format!("the write refused it: {error}"))?;
                if out.matches("<v:x").count() != DEPTH {
                    return Err("the write does not hold every level".to_owned());
                }
                let back = xmi::read(out.as_bytes(), &options)
                    .map_err(|error| format!("the read of the output refused it: {error}"))?
                    .into_document();
                if back != doc {
                    return Err("the output does not read to an equal document".to_owned());
                }
                drop((doc, clone, other, back));
                Ok(())
            })();
            // The receiver is gone only after the deadline has already failed the test.
            let _ = sender.send(result);
        })
        .unwrap_or_else(|error| panic!("{ROW}: cannot spawn the deep-fragment thread: {error}"));
    match receiver.recv_timeout(DEADLINE) {
        Ok(Ok(())) => {}
        Ok(Err(reason)) => panic!("{ROW}: a {DEPTH}-deep fragment: {reason}"),
        Err(RecvTimeoutError::Timeout) => {
            panic!("{ROW}: a {DEPTH}-deep fragment was not handled within {DEADLINE:?}")
        }
        Err(RecvTimeoutError::Disconnected) => {
            panic!("{ROW}: handling a {DEPTH}-deep fragment panicked")
        }
    }
}
