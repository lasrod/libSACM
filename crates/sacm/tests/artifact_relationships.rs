//! Tests for the three asserted relationships whose clauses constrain their
//! ends to `ArtifactReference`s: `AssertedEvidence` (SACM23-ARG-009,
//! formal/23-05-08 §11.15), `AssertedArtifactSupport` (SACM23-ARG-011,
//! §11.17) and `AssertedArtifactContext` (SACM23-ARG-012, §11.18)
//! (ptc/22-03-13; ADR 0019, ADR 0027, ADR 0028).
//!
//! Each has SACM23-ARG-007's features and no feature of its own. It is read
//! under `argumentElement` of an ArgumentPackage, written in the order
//! `... abstractForm assertionDeclaration metaClaim isCounter reasoning source
//! target`, and round-tripped; a golden file pins the bytes. It is created by
//! `CreateAssertedEvidence`, `CreateAssertedArtifactSupport` and
//! `CreateAssertedArtifactContext`, shaped like `CreateAssertedInference`,
//! which refuse no source, white space, an unknown id, and an end whose
//! target does not conform to its declared type
//! (`EditError::WrongReferenceType`, SACM-REF-002). A create does not refuse
//! the clauses' `ArtifactReference` rules, which are content (SACM23-VAL-016,
//! `reference_typing.rs`).
//!
//! Each test body runs on one kind; each row has its own test functions.

use std::fmt::Display;
use std::path::Path;

use sacm::compare::semantic_compare;
use sacm::diagnostic::{Code, Diagnostic, Severity, SourceLocation};
use sacm::edit::{
    self, CreateArgumentPackage, CreateArgumentReasoning, CreateArtifactReference,
    CreateAssertedArtifactContext, CreateAssertedArtifactSupport, CreateAssertedEvidence,
    CreateAssuranceCasePackage, CreateClaim, CreateNote, EditError, Operation,
};
use sacm::model::{
    AbstractClass, AssertionDeclaration, Document, ElementId, ElementKind, LangString, SacmElement,
};
use sacm::validation;
use sacm::xmi::{self, Mode, ReadError, ReadOptions, ReadOutput, WriteOptions};

const ARG_009: &str = "SACM23-ARG-009";
const ARG_011: &str = "SACM23-ARG-011";
const ARG_012: &str = "SACM23-ARG-012";

const MODES: [Mode; 2] = [Mode::Strict, Mode::Tolerant];

/// Marks, in a test input, where a refusal is expected. Removed before
/// reading.
const MARK: char = '\u{2038}';

/// The namespace declarations of a bare root: `sacm`, `xmi` and `xsi`.
const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
    " xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"",
);

/// The valid fixture of the slice.
const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/sacm23/xmi/artifact-relationships-valid.sacm.xmi"
);

/// The golden file of SACM23-ARG-006, -009, -011, -012 and -013.
const GOLDEN: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/sacm23/xmi/golden/artifact-relationships.golden.sacm.xmi"
);

/// The clause of each kind.
fn clause(kind: ElementKind) -> &'static str {
    match kind {
        ElementKind::AssertedEvidence => "11.15",
        ElementKind::AssertedArtifactSupport => "11.17",
        ElementKind::AssertedArtifactContext => "11.18",
        other => panic!("setup: {other} is not a kind of this file"),
    }
}

/// The target a well-formed relationship of `kind` has in [`in_ap`] and
/// [`built`]: a Claim for an `AssertedEvidence`, whose target §11.15 does not
/// narrow, and an `ArtifactReference` for the other two (§11.17, §11.18).
fn good_target(kind: ElementKind) -> &'static str {
    if kind == ElementKind::AssertedEvidence {
        "c1"
    } else {
        "a2"
    }
}

/// The id prefix `edit::apply` generates for `kind`.
fn prefix(kind: ElementKind) -> &'static str {
    match kind {
        ElementKind::AssertedEvidence => "asserted_evidence",
        ElementKind::AssertedArtifactSupport => "asserted_artifact_support",
        ElementKind::AssertedArtifactContext => "asserted_artifact_context",
        other => panic!("setup: no create operation for {other}"),
    }
}

// --- Inputs ------------------------------------------------------------------------

/// A named element `id` of `kind` under `argumentElement`, with `attributes`.
fn typed(kind: &str, id: &str, attributes: &str) -> String {
    format!(
        "<argumentElement xmi:type=\"sacm:{kind}\" xmi:id=\"{id}\"{attributes}>\
         <name content=\"N\"/></argumentElement>"
    )
}

/// An ArgumentPackage root `p` named "P", holding the named Claims c1 (with
/// the Note n1), c2 and m1, the `ArgumentReasoning` r1 and the
/// `ArtifactReference`s a1 and a2, then `body`, each on its own line.
fn in_ap(body: &str) -> String {
    format!(
        "<sacm:ArgumentPackage{NS} xmi:id=\"p\">\n  <name content=\"P\"/>\n  \
         <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"><name content=\"C\"/>\
         <note xmi:id=\"n1\"/></argumentElement>\n  {}\n  {}\n  {}\n  {}\n  {}\n  {body}\n\
         </sacm:ArgumentPackage>",
        typed("Claim", "c2", ""),
        typed("Claim", "m1", ""),
        typed("ArgumentReasoning", "r1", ""),
        typed(
            "ArtifactReference",
            "a1",
            " referencedArtifactElement=\"c2\""
        ),
        typed("ArtifactReference", "a2", ""),
    )
}

/// A relationship `x` of `kind`, named "X", with `attributes` and holding
/// `content` after its name, in [`in_ap`].
fn x_in_ap(kind: ElementKind, attributes: &str, content: &str) -> String {
    in_ap(&format!(
        "<argumentElement xmi:type=\"sacm:{kind}\" xmi:id=\"x\"{attributes}>\
         <name content=\"X\"/>{content}</argumentElement>"
    ))
}

fn loc(row: &str, line: u32, column: u32) -> SourceLocation {
    let Some(location) = SourceLocation::new(line, column) else {
        panic!("{row}: SourceLocation::new({line}, {column}) refused a 1-based position");
    };
    location
}

/// `template` without its marks, and the location of each mark in order.
fn marked(row: &str, template: &str) -> (String, Vec<SourceLocation>) {
    let (mut line, mut column, mut after_cr) = (1_u32, 1_u32, false);
    let mut text = String::with_capacity(template.len());
    let mut marks = Vec::new();
    for c in template.chars() {
        if c == MARK {
            marks.push(loc(row, line, column));
            continue;
        }
        text.push(c);
        match c {
            '\n' if after_cr => {}
            '\n' | '\r' => {
                line += 1;
                column = 1;
            }
            _ => column += 1,
        }
        after_cr = c == '\r';
    }
    (text, marks)
}

// --- Reading, writing and validating --------------------------------------------------

fn read(mode: Mode, input: &str) -> Result<ReadOutput, ReadError> {
    xmi::read(input.as_bytes(), &ReadOptions::default().with_mode(mode))
}

/// Reads `input` in both modes, which the test expects to read to equal
/// documents with no diagnostic but SACM-XMI-010 (an `xsi:type`), and
/// returns the strict one.
fn read_typed(row: &str, what: &str, input: &str) -> Document {
    let mut documents = Vec::new();
    for mode in MODES {
        match read(mode, input) {
            Ok(output) => {
                assert!(
                    output
                        .diagnostics()
                        .iter()
                        .all(|found| found.code() == Code::XMI_XSI_TYPE),
                    "{row}: {what}: reading in {mode:?} mode reported {:?}",
                    output.diagnostics()
                );
                documents.push(output.into_document());
            }
            Err(error) => {
                panic!("{row}: {what}: a {mode:?} read refused {input:?}: {error:?} ({error})")
            }
        }
    }
    assert_eq!(
        documents[0], documents[1],
        "{row}: {what}: the modes read different documents"
    );
    documents.swap_remove(0)
}

/// Reads `input` in both modes, which the test expects to read with no
/// diagnostic to equal documents, and returns the strict one.
fn read_ok(row: &str, what: &str, input: &str) -> Document {
    for mode in MODES {
        if let Ok(output) = read(mode, input) {
            assert!(
                output.diagnostics().is_empty(),
                "{row}: {what}: reading in {mode:?} mode reported {:?}",
                output.diagnostics()
            );
        }
    }
    read_typed(row, what, input)
}

/// Reads `input` in both modes, which the test expects to be refused the same
/// in both, and returns the refusal.
fn refused(row: &str, what: &str, input: &str) -> ReadError {
    let mut errors = Vec::new();
    for mode in MODES {
        match read(mode, input) {
            Ok(output) => panic!(
                "{row}: {what}: a {mode:?} read returned a document of {} elements for {input:?}",
                output.document().element_count()
            ),
            Err(error) => errors.push(error),
        }
    }
    assert_eq!(
        errors[0], errors[1],
        "{row}: {what}: the modes refuse differently"
    );
    errors.swap_remove(0)
}

fn write_in(row: &str, what: &str, doc: &Document, mode: Mode) -> String {
    match xmi::write(doc, &WriteOptions::default().with_mode(mode)) {
        Ok(text) => text,
        Err(error) => panic!("{row}: {what}: a {mode:?} write refused the document: {error:?}"),
    }
}

/// Writes `doc` in `mode`, reads the output back and writes that again: the
/// read-back must be equal and the second write must give the same bytes.
/// Returns the first output.
fn round_trip(row: &str, what: &str, doc: &Document, mode: Mode) -> String {
    let text = write_in(row, what, doc, mode);
    let back = read_ok(row, &format!("{what}, written"), &text);
    assert_eq!(
        semantic_compare(doc, &back),
        [],
        "{row}: {what}: the output does not read back equal\n{text}"
    );
    assert_eq!(&back, doc, "{row}: {what}: the output reads back !=");
    assert_eq!(
        write_in(row, what, &back, mode),
        text,
        "{row}: {what}: writing what was read back gives other bytes"
    );
    text
}

/// The element `id` of `doc`, which must exist.
fn element<'d>(row: &str, doc: &'d Document, id: &str) -> SacmElement<'d> {
    let Some(found) = doc.get(id) else {
        panic!("{row}: the document has no element {id:?}");
    };
    found
}

/// The sources, target, reasoning and `isCounter` of an element, owned.
type Ends = (Vec<String>, Option<String>, Option<String>, bool);

fn ends_of(found: SacmElement<'_>) -> Ends {
    (
        found
            .sources()
            .iter()
            .map(|id| id.as_str().to_owned())
            .collect(),
        found.target().map(|id| id.as_str().to_owned()),
        found.reasoning().map(|id| id.as_str().to_owned()),
        found.is_counter(),
    )
}

fn expected_ends(
    sources: &[&str],
    target: Option<&str>,
    reasoning: Option<&str>,
    counter: bool,
) -> Ends {
    (
        sources.iter().map(|&id| id.to_owned()).collect(),
        target.map(str::to_owned),
        reasoning.map(str::to_owned),
        counter,
    )
}

/// The line of `text` carrying `xmi:id="<id>"`, trimmed.
fn line_with<'t>(row: &str, text: &'t str, id: &str) -> &'t str {
    let attribute = format!("xmi:id=\"{id}\"");
    let Some(line) = text.lines().find(|line| line.contains(&attribute)) else {
        panic!("{row}: the output has no line with {attribute}:\n{text}");
    };
    line.trim()
}

/// Asserts that `error` is `Unsupported`, with no code, at `at`.
fn assert_unsupported_at(row: &str, what: &str, error: &ReadError, at: SourceLocation) {
    assert!(
        matches!(error, ReadError::Unsupported { .. }) && error.code().is_none(),
        "{row}: {what}: not Unsupported: {error:?}"
    );
    assert_eq!(
        error.location(),
        Some(at),
        "{row}: {what}: not refused at the construct: {error}"
    );
}

// --- Editing -------------------------------------------------------------------------

fn named(content: &str) -> LangString {
    LangString::new(content).with_lang("en")
}

/// What a create operation for a relationship is given.
#[derive(Clone)]
struct Spec<'a> {
    parent: &'a str,
    sources: Vec<&'a str>,
    target: &'a str,
    id: Option<&'a str>,
    reasoning: Option<&'a str>,
    counter: bool,
}

/// A create under `p` with `sources` and `target` and nothing optional.
fn spec<'a>(sources: &[&'a str], target: &'a str) -> Spec<'a> {
    Spec {
        parent: "p",
        sources: sources.to_vec(),
        target,
        id: None,
        reasoning: None,
        counter: false,
    }
}

/// Applies the options of `spec` to a create of any of the three kinds.
macro_rules! with_options {
    ($create:expr, $spec:expr) => {{
        let mut create = $create;
        if let Some(id) = $spec.id {
            create = create.with_id(id);
        }
        if let Some(reasoning) = $spec.reasoning {
            create = create.with_reasoning(reasoning);
        }
        if $spec.counter {
            create = create.with_is_counter(true);
        }
        Operation::from(create)
    }};
}

/// The create operation of `kind` for `spec`, built as a caller would.
fn operation(kind: ElementKind, spec: &Spec<'_>) -> Operation {
    let name = named("Relationship");
    let sources = spec.sources.iter().copied();
    match kind {
        ElementKind::AssertedEvidence => with_options!(
            CreateAssertedEvidence::new(spec.parent, name, sources, spec.target),
            spec
        ),
        ElementKind::AssertedArtifactSupport => with_options!(
            CreateAssertedArtifactSupport::new(spec.parent, name, sources, spec.target),
            spec
        ),
        ElementKind::AssertedArtifactContext => with_options!(
            CreateAssertedArtifactContext::new(spec.parent, name, sources, spec.target),
            spec
        ),
        other => panic!("setup: no create operation for {other}"),
    }
}

/// Applies an operation the test expects to succeed, and returns the one id
/// it created.
fn create(row: &str, doc: &mut Document, operation: impl Into<Operation>) -> ElementId {
    match edit::apply(doc, operation) {
        Ok(mutation) => {
            let created = mutation.created_ids();
            let [only] = created else {
                panic!("{row}: created_ids() is {created:?}, not the one new element");
            };
            only.clone()
        }
        Err(error) => panic!("{row}: the operation was refused: {error} ({error:?})"),
    }
}

/// A document built by edits: the AssuranceCasePackage `case` holding the
/// ArgumentPackage `p`, with the Claims c1 (holding the Note n1), c2 and m1,
/// the `ArgumentReasoning` r1, and the `ArtifactReference`s a1 (naming c2)
/// and a2.
fn built(row: &str) -> Document {
    let mut doc = Document::new();
    create(
        row,
        &mut doc,
        CreateAssuranceCasePackage::root(named("Case")).with_id("case"),
    );
    create(
        row,
        &mut doc,
        CreateArgumentPackage::new("case", named("Arguments")).with_id("p"),
    );
    for (id, name) in [
        ("c1", "The brakes are safe"),
        ("c2", "The stopping distance is short enough"),
        ("m1", "The evidence is relevant"),
    ] {
        create(
            row,
            &mut doc,
            CreateClaim::new("p", named(name)).with_id(id),
        );
    }
    create(
        row,
        &mut doc,
        CreateArgumentReasoning::new("p", named("Over the tests")).with_id("r1"),
    );
    create(
        row,
        &mut doc,
        CreateArtifactReference::new("p", named("The test report"))
            .with_id("a1")
            .with_referenced_artifact_elements(["c2"]),
    );
    create(
        row,
        &mut doc,
        CreateArtifactReference::new("p", named("The test plan")).with_id("a2"),
    );
    create(
        row,
        &mut doc,
        CreateNote::new("c1", named("N")).with_id("n1"),
    );
    doc
}

/// The name of a reference end in an `EditError`, by its `Display`.
fn end_name(end: &dyn Display) -> String {
    end.to_string()
}

/// Applies `refused`, which the test expects to be refused leaving `doc`
/// `==` to before and with the same next generated id of `kind`, and returns
/// why.
fn refuse(
    row: &str,
    what: &str,
    doc: &mut Document,
    kind: ElementKind,
    refused: Operation,
) -> EditError {
    let before = doc.clone();
    let error = match edit::apply(doc, refused) {
        Ok(mutation) => panic!(
            "{row}: {what}: accepted, creating {:?}",
            mutation.created_ids()
        ),
        Err(error) => error,
    };
    assert!(
        *doc == before,
        "{row}: {what}: the refusal changed the document"
    );
    let mut probe = doc.clone();
    assert_eq!(
        create(
            row,
            &mut probe,
            operation(kind, &spec(&["a1"], good_target(kind)))
        )
        .as_str(),
        format!("{}_1", prefix(kind)),
        "{row}: {what}: the refusal moved the next generated id"
    );
    let message = error.to_string();
    assert!(
        !message.contains('\n') && message.starts_with(char::is_lowercase),
        "{row}: {what}: the message {message:?} is not one line starting in lower case"
    );
    error
}

/// Asserts that `error` is `WrongReferenceType` for `end`, `id` of kind
/// `kind`, expecting `expected`, with SACM-REF-002 and a message naming all
/// four.
fn assert_wrong_type(
    row: &str,
    what: &str,
    error: &EditError,
    (end, id, kind): (&str, &str, ElementKind),
    expected: &str,
) {
    let EditError::WrongReferenceType {
        end: found_end,
        id: found_id,
        kind: found_kind,
        expected: found_expected,
        ..
    } = error
    else {
        panic!("{row}: {what}: not WrongReferenceType: {error:?}");
    };
    assert_eq!(
        (
            end_name(found_end),
            found_id.as_str(),
            *found_kind,
            *found_expected,
            error.code()
        ),
        (end.to_owned(), id, kind, expected, Code::REF_WRONG_TYPE),
        "{row}: {what}: not WrongReferenceType {{ {end}, {id}, {kind}, {expected} }} (SACM-REF-002)"
    );
    let message = error.to_string();
    for part in [
        format!("{id:?}"),
        end.to_owned(),
        kind.name().to_owned(),
        expected.to_owned(),
    ] {
        assert!(
            message.contains(&part),
            "{row}: {what}: the message {message:?} does not name {part}"
        );
    }
}

// =====================================================================================
// Item 1: reading, writing and round-tripping
// =====================================================================================

fn reads_every_feature_and_writes_in_order(row: &str, kind: ElementKind) {
    let target = good_target(kind);
    let what = format!("every value on an {kind}");
    let doc = read_ok(
        row,
        &what,
        &x_in_ap(
            kind,
            &format!(
                " target=\"{target}\" source=\"a1 a2\" reasoning=\"r1\" isCounter=\"true\" \
                 metaClaim=\"m1\" assertionDeclaration=\"assumed\""
            ),
            "",
        ),
    );
    let x = element(row, &doc, "x");
    assert_eq!(
        (
            x.kind(),
            x.parent().map(|parent| parent.id().as_str()),
            ends_of(x),
            x.assertion_declaration(),
            x.meta_claims()
                .iter()
                .map(|id| id.as_str().to_owned())
                .collect::<Vec<_>>()
        ),
        (
            kind,
            Some("p"),
            expected_ends(&["a1", "a2"], Some(target), Some("r1"), true),
            Some(AssertionDeclaration::Assumed),
            vec!["m1".to_owned()]
        ),
        "{row}: {what}: not every value is read"
    );
    assert_eq!(
        validation::validate(&doc),
        [],
        "{row}: {what}: setup: the document does not validate cleanly"
    );
    let text = round_trip(row, &what, &doc, Mode::Strict);
    assert_eq!(
        line_with(row, &text, "x"),
        format!(
            "<argumentElement xmi:type=\"sacm:{kind}\" xmi:id=\"x\" \
             assertionDeclaration=\"assumed\" metaClaim=\"m1\" isCounter=\"true\" \
             reasoning=\"r1\" source=\"a1 a2\" target=\"{target}\">"
        ),
        "{row}: {what}: not written in the pinned order\n{text}"
    );
    // The element forms, and xsi:type.
    let what = format!("the element forms on an {kind}, by xsi:type");
    let doc = read_typed(
        row,
        &what,
        &in_ap(&format!(
            "<argumentElement xsi:type=\"sacm:{kind}\" xmi:id=\"x\"><name content=\"X\"/>\
             <source xmi:idref=\"a2\"/><source href=\"#a1\"/><target xmi:idref=\"{target}\"/>\
             <reasoning href=\"#r1\"/><metaClaim xmi:idref=\"m1\"/><isCounter>1</isCounter>\
             <assertionDeclaration>axiomatic</assertionDeclaration></argumentElement>"
        )),
    );
    let x = element(row, &doc, "x");
    assert_eq!(
        (x.kind(), ends_of(x), x.assertion_declaration()),
        (
            kind,
            expected_ends(&["a2", "a1"], Some(target), Some("r1"), true),
            Some(AssertionDeclaration::Axiomatic)
        ),
        "{row}: {what}: not read"
    );
    let text = round_trip(row, &what, &doc, Mode::Strict);
    assert!(
        line_with(row, &text, "x").starts_with(&format!(
            "<argumentElement xmi:type=\"sacm:{kind}\" xmi:id=\"x\""
        )),
        "{row}: {what}: not written by xmi:type under argumentElement\n{text}"
    );
}

fn is_refused_outside_an_argument_package(row: &str, kind: ElementKind) {
    for (what, template) in [
        (
            format!("an {kind} in argumentElement of an AssuranceCasePackage"),
            format!(
                "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\"><name content=\"Case\"/>\
                 {MARK}<argumentElement xmi:type=\"sacm:{kind}\" xmi:id=\"x\"/>\
                 </sacm:AssuranceCasePackage>"
            ),
        ),
        (
            format!("an {kind} in a Claim"),
            in_ap(&format!(
                "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"y\"><name content=\"Y\"/>\
                 {MARK}<argumentElement xmi:type=\"sacm:{kind}\" xmi:id=\"x\"/></argumentElement>"
            )),
        ),
        (
            format!("an {kind} with the attribute of an ArtifactReference"),
            x_in_ap(
                kind,
                &format!(" source=\"a1\" target=\"c1\" {MARK}referencedArtifactElement=\"c1\""),
                "",
            ),
        ),
    ] {
        let (input, marks) = marked(row, &template);
        let error = refused(row, &what, &input);
        assert_unsupported_at(row, &what, &error, marks[0]);
    }
}

#[test]
fn sacm23_arg_009_reads_every_relationship_feature_in_both_modes_and_writes_in_order() {
    reads_every_feature_and_writes_in_order(ARG_009, ElementKind::AssertedEvidence);
}

#[test]
fn sacm23_arg_011_reads_every_relationship_feature_in_both_modes_and_writes_in_order() {
    reads_every_feature_and_writes_in_order(ARG_011, ElementKind::AssertedArtifactSupport);
}

#[test]
fn sacm23_arg_012_reads_every_relationship_feature_in_both_modes_and_writes_in_order() {
    reads_every_feature_and_writes_in_order(ARG_012, ElementKind::AssertedArtifactContext);
}

#[test]
fn sacm23_arg_009_is_unsupported_outside_an_argument_package_and_with_another_kinds_end() {
    is_refused_outside_an_argument_package(ARG_009, ElementKind::AssertedEvidence);
}

#[test]
fn sacm23_arg_011_is_unsupported_outside_an_argument_package_and_with_another_kinds_end() {
    is_refused_outside_an_argument_package(ARG_011, ElementKind::AssertedArtifactSupport);
}

#[test]
fn sacm23_arg_012_is_unsupported_outside_an_argument_package_and_with_another_kinds_end() {
    is_refused_outside_an_argument_package(ARG_012, ElementKind::AssertedArtifactContext);
}

fn the_golden_file_round_trips_to_its_own_bytes(row: &str, kind: ElementKind) {
    let golden = std::fs::read_to_string(GOLDEN)
        .unwrap_or_else(|error| panic!("{row}: cannot read {GOLDEN}: {error}"));
    let doc = read_ok(row, "the golden file", &golden);
    assert!(
        doc.elements().any(|found| found.kind() == kind),
        "{row}: setup: the golden file holds no {kind}"
    );
    assert_eq!(
        validation::validate(&doc),
        [],
        "{row}: the golden file does not validate cleanly"
    );
    for mode in MODES {
        let text = round_trip(row, "the golden file", &doc, mode);
        assert_eq!(
            text, golden,
            "{row}: a {mode:?} write of the golden file is not its own bytes"
        );
    }
}

#[test]
fn sacm23_arg_009_the_golden_file_round_trips_to_its_own_bytes() {
    the_golden_file_round_trips_to_its_own_bytes(ARG_009, ElementKind::AssertedEvidence);
}

#[test]
fn sacm23_arg_011_the_golden_file_round_trips_to_its_own_bytes() {
    the_golden_file_round_trips_to_its_own_bytes(ARG_011, ElementKind::AssertedArtifactSupport);
}

#[test]
fn sacm23_arg_012_the_golden_file_round_trips_to_its_own_bytes() {
    the_golden_file_round_trips_to_its_own_bytes(ARG_012, ElementKind::AssertedArtifactContext);
}

fn the_fixture_reads_and_round_trips(row: &str, kind: ElementKind, expected: &[(&str, Ends)]) {
    let bytes = std::fs::read(Path::new(FIXTURE))
        .unwrap_or_else(|error| panic!("{row}: cannot read {FIXTURE}: {error}"));
    let doc = read_ok(row, "the fixture", &String::from_utf8_lossy(&bytes));
    for (id, ends) in expected {
        let found = element(row, &doc, id);
        assert_eq!(
            (found.kind(), ends_of(found)),
            (kind, ends.clone()),
            "{row}: the fixture's {id}"
        );
    }
    assert_eq!(
        validation::validate(&doc),
        [],
        "{row}: the fixture does not validate cleanly"
    );
    round_trip(row, "the fixture", &doc, Mode::Strict);
}

#[test]
fn sacm23_arg_009_the_fixture_reads_with_its_evidence_and_round_trips() {
    the_fixture_reads_and_round_trips(
        ARG_009,
        ElementKind::AssertedEvidence,
        &[(
            "e1",
            expected_ends(&["report", "plan"], Some("c1"), Some("r1"), false),
        )],
    );
}

#[test]
fn sacm23_arg_011_the_fixture_reads_with_its_artifact_support_and_round_trips() {
    the_fixture_reads_and_round_trips(
        ARG_011,
        ElementKind::AssertedArtifactSupport,
        &[("s1", expected_ends(&["plan"], Some("report"), None, false))],
    );
}

#[test]
fn sacm23_arg_012_the_fixture_reads_with_its_artifact_context_and_round_trips() {
    the_fixture_reads_and_round_trips(
        ARG_012,
        ElementKind::AssertedArtifactContext,
        &[(
            "x1",
            expected_ends(&["standard"], Some("report"), None, true),
        )],
    );
}

// =====================================================================================
// Item 2: the create operation
// =====================================================================================

fn the_create_operation_reports_its_arguments(row: &str, kind: ElementKind) {
    macro_rules! report {
        ($new:expr) => {{
            let create = $new;
            assert_eq!(
                (create.id(), create.reasoning(), create.is_counter()),
                (None, None, false),
                "{row}: the options are set by default"
            );
            let create = create
                .with_id("given")
                .with_reasoning("r1")
                .with_is_counter(true);
            (
                create.parent().as_str().to_owned(),
                create.id().map(|id| id.as_str().to_owned()),
                create.name().clone(),
                create
                    .sources()
                    .iter()
                    .map(|id| id.as_str().to_owned())
                    .collect::<Vec<_>>(),
                create.target().as_str().to_owned(),
                create.reasoning().map(|id| id.as_str().to_owned()),
                create.is_counter(),
                Operation::from(create).operation_name().to_owned(),
            )
        }};
    }
    let name = named("Relationship");
    let reported = match kind {
        ElementKind::AssertedEvidence => {
            report!(CreateAssertedEvidence::new(
                "p",
                name.clone(),
                ["a1", "a2"],
                "c1"
            ))
        }
        ElementKind::AssertedArtifactSupport => report!(CreateAssertedArtifactSupport::new(
            "p",
            name.clone(),
            ["a1", "a2"],
            "c1"
        )),
        ElementKind::AssertedArtifactContext => report!(CreateAssertedArtifactContext::new(
            "p",
            name.clone(),
            ["a1", "a2"],
            "c1"
        )),
        other => panic!("{row}: setup: no create operation for {other}"),
    };
    assert_eq!(
        reported,
        (
            "p".to_owned(),
            Some("given".to_owned()),
            name,
            vec!["a1".to_owned(), "a2".to_owned()],
            "c1".to_owned(),
            Some("r1".to_owned()),
            true,
            format!("Create{kind}")
        ),
        "{row}: the operation does not report what it was given"
    );
}

#[test]
fn sacm23_arg_009_create_asserted_evidence_reports_what_it_was_given() {
    the_create_operation_reports_its_arguments(ARG_009, ElementKind::AssertedEvidence);
}

#[test]
fn sacm23_arg_011_create_asserted_artifact_support_reports_what_it_was_given() {
    the_create_operation_reports_its_arguments(ARG_011, ElementKind::AssertedArtifactSupport);
}

#[test]
fn sacm23_arg_012_create_asserted_artifact_context_reports_what_it_was_given() {
    the_create_operation_reports_its_arguments(ARG_012, ElementKind::AssertedArtifactContext);
}

fn the_create_operation_creates_with_its_ends(row: &str, kind: ElementKind) {
    let mut doc = built(row);
    let target = good_target(kind);
    let generated = format!("{}_1", prefix(kind));
    let id = create(row, &mut doc, operation(kind, &spec(&["a1", "a2"], target)));
    assert_eq!(id.as_str(), generated, "{row}: the generated id");
    let found = element(row, &doc, &generated);
    assert_eq!(
        (
            found.kind(),
            found.parent().map(|parent| parent.id().as_str()),
            found.name(),
            ends_of(found),
            found.assertion_declaration(),
        ),
        (
            kind,
            Some("p"),
            Some(&named("Relationship")),
            expected_ends(&["a1", "a2"], Some(target), None, false),
            Some(AssertionDeclaration::Asserted),
        ),
        "{row}: the created {kind}"
    );
    // With every option.
    let mut full = spec(&["a2"], target);
    full.id = Some("given");
    full.reasoning = Some("r1");
    full.counter = true;
    create(row, &mut doc, operation(kind, &full));
    assert_eq!(
        ends(row, &doc, "given"),
        expected_ends(&["a2"], Some(target), Some("r1"), true),
        "{row}: the options are not set as given"
    );
    let id = create(row, &mut doc, operation(kind, &spec(&["a2"], target)));
    assert_eq!(
        id.as_str(),
        format!("{}_2", prefix(kind)),
        "{row}: the second generated id"
    );
    assert_eq!(
        validation::validate(&doc),
        [],
        "{row}: the built document does not validate cleanly"
    );
    // It writes strictly and reads back equal.
    let text = write_in(row, "the built document", &doc, Mode::Strict);
    let back = read_ok(row, "the built document, written", &text);
    assert!(doc == back, "{row}: built == reloaded is false\n{text}");
}

fn ends(row: &str, doc: &Document, id: &str) -> Ends {
    ends_of(element(row, doc, id))
}

#[test]
fn sacm23_arg_009_create_asserted_evidence_creates_with_its_ends_and_a_generated_id() {
    the_create_operation_creates_with_its_ends(ARG_009, ElementKind::AssertedEvidence);
}

#[test]
fn sacm23_arg_011_create_asserted_artifact_support_creates_with_its_ends_and_a_generated_id() {
    the_create_operation_creates_with_its_ends(ARG_011, ElementKind::AssertedArtifactSupport);
}

#[test]
fn sacm23_arg_012_create_asserted_artifact_context_creates_with_its_ends_and_a_generated_id() {
    the_create_operation_creates_with_its_ends(ARG_012, ElementKind::AssertedArtifactContext);
}

fn the_create_operation_refuses_unchanged(row: &str, kind: ElementKind) {
    let mut doc = built(row);
    let target = good_target(kind);
    // ADR 0019's four checks.
    let mut missing_parent = spec(&[], "nope");
    missing_parent.parent = "nowhere";
    let error = refuse(
        row,
        "a missing parent",
        &mut doc,
        kind,
        operation(kind, &missing_parent),
    );
    assert!(
        matches!(&error, EditError::ParentNotFound { .. }),
        "{row}: a missing parent is not ParentNotFound: {error:?}"
    );
    let mut wrong_parent = spec(&["n1"], "n1");
    wrong_parent.parent = "case";
    let error = refuse(
        row,
        "under the case",
        &mut doc,
        kind,
        operation(kind, &wrong_parent),
    );
    assert!(
        matches!(
            &error,
            EditError::InvalidParent { parent_kind: ElementKind::AssuranceCasePackage, kind: k, .. }
                if *k == kind
        ),
        "{row}: under the case is not InvalidParent: {error:?}"
    );
    // No source.
    let what = "no source";
    let error = refuse(
        row,
        what,
        &mut doc,
        kind,
        operation(kind, &spec(&[], target)),
    );
    assert!(
        matches!(
            &error,
            EditError::TooFewReferences {
                end: "source",
                given: 0,
                required: 1,
                ..
            }
        ) && error.code() == Code::MULTIPLICITY_VIOLATION,
        "{row}: {what}: not TooFewReferences {{ source, 0 of 1 }} (SACM-MULT-001): {error:?}"
    );
    // Unknown ids.
    for (what, given, end) in [
        ("an unknown source", spec(&["a1", "nope"], target), "source"),
        ("an unknown target", spec(&["a1"], "nope"), "target"),
    ] {
        let error = refuse(row, what, &mut doc, kind, operation(kind, &given));
        let EditError::ReferenceNotFound { end: found, id, .. } = &error else {
            panic!("{row}: {what}: not ReferenceNotFound: {error:?}");
        };
        assert_eq!(
            (end_name(found), id.as_str(), error.code()),
            (end.to_owned(), "nope", Code::CMD_TARGET_NOT_FOUND),
            "{row}: {what}: not ReferenceNotFound {{ {end}, nope }} (SACM-CMD-001)"
        );
    }
    let mut unknown_reasoning = spec(&["a1"], target);
    unknown_reasoning.reasoning = Some("nope");
    let error = refuse(
        row,
        "an unknown reasoning",
        &mut doc,
        kind,
        operation(kind, &unknown_reasoning),
    );
    assert!(
        matches!(
            &error,
            EditError::ReferenceNotFound {
                end: "reasoning",
                ..
            }
        ),
        "{row}: an unknown reasoning is not ReferenceNotFound: {error:?}"
    );
}

fn the_create_operation_refuses_a_wrongly_typed_end_unchanged(row: &str, kind: ElementKind) {
    let mut doc = built(row);
    let target = good_target(kind);
    let asset = AbstractClass::ArgumentAsset.name();
    let reasoning = ElementKind::ArgumentReasoning.name();
    let with_reasoning = |mut given: Spec<'static>| {
        given.reasoning = Some("c1");
        given
    };
    // A Note is no ArgumentAsset, a package neither, and a Claim is no
    // ArgumentReasoning. The ends are typed in order: the sources, the
    // target, the reasoning.
    let cases = [
        (
            "a Note source",
            spec(&["a1", "n1"], target),
            ("source", "n1", ElementKind::Note, asset),
        ),
        (
            "a Note target",
            spec(&["a1"], "n1"),
            ("target", "n1", ElementKind::Note, asset),
        ),
        (
            "a package target",
            spec(&["a1"], "p"),
            ("target", "p", ElementKind::ArgumentPackage, asset),
        ),
        (
            "a Claim reasoning",
            with_reasoning(spec(&["a1"], target)),
            ("reasoning", "c1", ElementKind::Claim, reasoning),
        ),
        (
            "a Note source and a Note target",
            spec(&["a1", "n1"], "n1"),
            ("source", "n1", ElementKind::Note, asset),
        ),
        (
            "a Note target and a Claim reasoning",
            with_reasoning(spec(&["a1"], "n1")),
            ("target", "n1", ElementKind::Note, asset),
        ),
    ];
    for (what, given, (end, id, found, expected)) in cases {
        let error = refuse(row, what, &mut doc, kind, operation(kind, &given));
        assert_wrong_type(row, what, &error, (end, id, found), expected);
    }
    // Every id exists before any is typed.
    let what = "a Note source and an unknown target";
    let error = refuse(
        row,
        what,
        &mut doc,
        kind,
        operation(kind, &spec(&["n1"], "nope")),
    );
    assert!(
        matches!(&error, EditError::ReferenceNotFound { end: "target", .. }),
        "{row}: {what}: the unknown target is not reported before the wrong type: {error:?}"
    );
}

#[test]
fn sacm23_arg_009_create_asserted_evidence_refuses_a_bad_parent_no_source_and_missing_ids_unchanged()
 {
    the_create_operation_refuses_unchanged(ARG_009, ElementKind::AssertedEvidence);
}

#[test]
fn sacm23_arg_011_create_asserted_artifact_support_refuses_a_bad_parent_no_source_and_missing_ids_unchanged()
 {
    the_create_operation_refuses_unchanged(ARG_011, ElementKind::AssertedArtifactSupport);
}

#[test]
fn sacm23_arg_012_create_asserted_artifact_context_refuses_a_bad_parent_no_source_and_missing_ids_unchanged()
 {
    the_create_operation_refuses_unchanged(ARG_012, ElementKind::AssertedArtifactContext);
}

#[test]
fn sacm23_arg_009_create_asserted_evidence_refuses_a_wrongly_typed_end_unchanged() {
    the_create_operation_refuses_a_wrongly_typed_end_unchanged(
        ARG_009,
        ElementKind::AssertedEvidence,
    );
}

#[test]
fn sacm23_arg_011_create_asserted_artifact_support_refuses_a_wrongly_typed_end_unchanged() {
    the_create_operation_refuses_a_wrongly_typed_end_unchanged(
        ARG_011,
        ElementKind::AssertedArtifactSupport,
    );
}

#[test]
fn sacm23_arg_012_create_asserted_artifact_context_refuses_a_wrongly_typed_end_unchanged() {
    the_create_operation_refuses_a_wrongly_typed_end_unchanged(
        ARG_012,
        ElementKind::AssertedArtifactContext,
    );
}

fn a_reference_id_holding_white_space_is_refused(row: &str, kind: ElementKind) {
    let mut doc = match read(
        Mode::Tolerant,
        &in_ap(&typed("ArtifactReference", "a b", "")),
    ) {
        Ok(output) => output.into_document(),
        Err(error) => panic!("{row}: setup: a tolerant read refused: {error:?}"),
    };
    assert!(
        doc.contains("a b"),
        "{row}: setup: the tolerant read does not hold \"a b\" as written"
    );
    let target = good_target(kind);
    for (what, given, end) in [
        (
            "a white-space source",
            spec(&["a1", "a b"], target),
            "source",
        ),
        ("a white-space target", spec(&["a1"], "a b"), "target"),
        (
            "a white-space source and an unknown target",
            spec(&["a b"], "nope"),
            "source",
        ),
        (
            "an unknown source and a white-space target",
            spec(&["nope"], "a b"),
            "target",
        ),
        (
            "a Note source and a white-space target",
            spec(&["n1"], "a b"),
            "target",
        ),
    ] {
        let error = refuse(row, what, &mut doc, kind, operation(kind, &given));
        let EditError::InvalidReference { end: found, id, .. } = &error else {
            panic!("{row}: {what}: not InvalidReference: {error:?}");
        };
        assert_eq!(
            (end_name(found), id.as_str(), error.code()),
            (end.to_owned(), "a b", Code::ID_INVALID),
            "{row}: {what}: not InvalidReference {{ {end}, \"a b\" }} (SACM-ID-002)"
        );
    }
    // No source comes first.
    let what = "no source and a white-space target";
    let error = refuse(
        row,
        what,
        &mut doc,
        kind,
        operation(kind, &spec(&[], "a b")),
    );
    assert!(
        matches!(&error, EditError::TooFewReferences { end: "source", .. }),
        "{row}: {what} is not TooFewReferences {{ source }}: {error:?}"
    );
}

#[test]
fn sacm23_arg_009_create_asserted_evidence_refuses_a_reference_id_holding_white_space() {
    a_reference_id_holding_white_space_is_refused(ARG_009, ElementKind::AssertedEvidence);
}

#[test]
fn sacm23_arg_011_create_asserted_artifact_support_refuses_a_reference_id_holding_white_space() {
    a_reference_id_holding_white_space_is_refused(ARG_011, ElementKind::AssertedArtifactSupport);
}

#[test]
fn sacm23_arg_012_create_asserted_artifact_context_refuses_a_reference_id_holding_white_space() {
    a_reference_id_holding_white_space_is_refused(ARG_012, ElementKind::AssertedArtifactContext);
}

/// The SACM23-VAL-016 findings of `validate` on `doc`.
fn kind_rule(doc: &Document) -> Vec<Diagnostic> {
    validation::validate(doc)
        .into_iter()
        .filter(|found| found.requirement_id() == "SACM23-VAL-016")
        .collect()
}

fn a_claim_end_is_accepted_and_then_reported(row: &str, kind: ElementKind) {
    let mut doc = built(row);
    // A Claim source: accepted, then one SACM23-VAL-016 finding.
    let mut given = spec(&["c2"], good_target(kind));
    given.id = Some("by_source");
    create(row, &mut doc, operation(kind, &given));
    let found = kind_rule(&doc);
    let [finding] = found.as_slice() else {
        panic!("{row}: a Claim source: not one SACM23-VAL-016 finding: {found:?}");
    };
    assert_eq!(
        (
            finding.code(),
            finding.severity(),
            finding.clause(),
            finding.location(),
            finding.affected()
        ),
        (
            Code::REF_WRONG_TYPE,
            Severity::Error,
            Some(clause(kind)),
            None,
            [ElementId::from("by_source"), ElementId::from("c2")].as_slice()
        ),
        "{row}: a Claim source: not a SACM-REF-002 Error with clause {} on [by_source, c2]",
        clause(kind)
    );
    // A Claim target: accepted; reported for the two artifact kinds only.
    let mut doc = built(row);
    let mut given = spec(&["a1"], "c1");
    given.id = Some("by_target");
    create(row, &mut doc, operation(kind, &given));
    let found = kind_rule(&doc);
    if kind == ElementKind::AssertedEvidence {
        assert_eq!(
            found,
            [],
            "{row}: an AssertedEvidence's Claim target is reported; §11.15 does not narrow it"
        );
        assert_eq!(
            validation::validate(&doc),
            [],
            "{row}: an AssertedEvidence from an ArtifactReference to a Claim gives a finding"
        );
    } else {
        let [finding] = found.as_slice() else {
            panic!("{row}: a Claim target: not one SACM23-VAL-016 finding: {found:?}");
        };
        assert_eq!(
            (finding.code(), finding.clause(), finding.affected()),
            (
                Code::REF_WRONG_TYPE,
                Some(clause(kind)),
                [ElementId::from("by_target"), ElementId::from("c1")].as_slice()
            ),
            "{row}: a Claim target: not SACM-REF-002 with clause {} on [by_target, c1]",
            clause(kind)
        );
    }
}

#[test]
fn sacm23_arg_009_create_asserted_evidence_accepts_a_claim_end_that_validate_then_reports() {
    a_claim_end_is_accepted_and_then_reported(ARG_009, ElementKind::AssertedEvidence);
}

#[test]
fn sacm23_arg_011_create_asserted_artifact_support_accepts_a_claim_end_that_validate_then_reports()
{
    a_claim_end_is_accepted_and_then_reported(ARG_011, ElementKind::AssertedArtifactSupport);
}

#[test]
fn sacm23_arg_012_create_asserted_artifact_context_accepts_a_claim_end_that_validate_then_reports()
{
    a_claim_end_is_accepted_and_then_reported(ARG_012, ElementKind::AssertedArtifactContext);
}

// =====================================================================================
// Item 3: a missing source or target
// =====================================================================================

fn a_missing_end_gives_val_005(row: &str, kind: ElementKind) {
    let target = good_target(kind);
    for (what, attributes, missing) in [
        ("no source", format!(" target=\"{target}\""), vec!["source"]),
        ("no target", " source=\"a1\"".to_owned(), vec!["target"]),
        ("neither", String::new(), vec!["source", "target"]),
    ] {
        let what = format!("an {kind} with {what}");
        let doc = read_ok(row, &what, &x_in_ap(kind, &attributes, ""));
        let found = validation::validate(&doc);
        assert_eq!(
            found.len(),
            missing.len(),
            "{row}: {what}: not one finding per missing end: {found:?}"
        );
        for end in &missing {
            let matching: Vec<&Diagnostic> = found
                .iter()
                .filter(|finding| finding.message().contains(end))
                .collect();
            let [finding] = matching.as_slice() else {
                panic!("{row}: {what}: not one finding naming {end}: {found:?}");
            };
            assert_eq!(
                (
                    finding.code(),
                    finding.severity(),
                    finding.requirement_id(),
                    finding.clause(),
                    finding.location(),
                    finding.affected()
                ),
                (
                    Code::MULTIPLICITY_VIOLATION,
                    Severity::Error,
                    "SACM23-VAL-005",
                    Some("11.13"),
                    None,
                    [ElementId::from("x")].as_slice()
                ),
                "{row}: {what}: not a SACM-MULT-001 Error of SACM23-VAL-005 with clause 11.13 on [x]"
            );
        }
        let text = round_trip(row, &what, &doc, Mode::Tolerant);
        assert!(
            line_with(row, &text, "x").starts_with(&format!(
                "<argumentElement xmi:type=\"sacm:{kind}\" xmi:id=\"x\""
            )),
            "{row}: {what}: not kept by a tolerant write\n{text}"
        );
    }
}

#[test]
fn sacm23_arg_009_a_missing_source_or_target_gives_val_005_with_clause_11_13() {
    a_missing_end_gives_val_005(ARG_009, ElementKind::AssertedEvidence);
}

#[test]
fn sacm23_arg_011_a_missing_source_or_target_gives_val_005_with_clause_11_13() {
    a_missing_end_gives_val_005(ARG_011, ElementKind::AssertedArtifactSupport);
}

#[test]
fn sacm23_arg_012_a_missing_source_or_target_gives_val_005_with_clause_11_13() {
    a_missing_end_gives_val_005(ARG_012, ElementKind::AssertedArtifactContext);
}
