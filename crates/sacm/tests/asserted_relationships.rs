//! Tests for the asserted relationships: the features every
//! `AssertedRelationship` has (SACM23-ARG-007; formal/23-05-08 §11.13), and
//! `AssertedInference` (SACM23-ARG-008, §11.14) and `AssertedContext`
//! (SACM23-ARG-010, §11.16), the first two concrete kinds (ptc/22-03-13
//! `Argumentation::AssertedRelationship`; XMI 2.5.1 rules 2b, 2h, 2j, §9.4.1;
//! ADR 0019, ADR 0026, ADR 0027).
//!
//! A relationship holds `source` (many, in document order with repeats),
//! `target` and `reasoning` (one each) and `isCounter` (an `xsd:boolean`,
//! false by default), besides `assertionDeclaration` and `metaClaim`
//! (SACM23-ARG-004, `meta_claims.rs`). They are written in the order
//! `... abstractForm assertionDeclaration metaClaim isCounter reasoning
//! source target`, and compared as
//! `Difference::{IsCounter, Reasoning, Sources, Target}`. A relationship read
//! without a source or a target is
//! kept as read; its finding is SACM23-VAL-005 (`relationship_validation.rs`).
//! `CreateAssertedInference` and `CreateAssertedContext` take the sources
//! and the target, and refuse no source (`EditError::TooFewReferences`) and a
//! reference naming no element (`EditError::ReferenceNotFound`).
//!
//! Each SACM23-ARG-008 test has a SACM23-ARG-010 twin that runs the same body
//! on the other kind. Expected locations are marked in the input with
//! [`MARK`] (U+2038), which the test removes before reading.

use std::fmt::Display;
use std::path::Path;

use sacm::compare::{Difference, semantic_compare};
use sacm::diagnostic::{Code, Diagnostic, Severity, SourceLocation};
use sacm::edit::{
    self, CreateArgumentPackage, CreateArgumentReasoning, CreateAssertedContext,
    CreateAssertedInference, CreateAssuranceCasePackage, CreateClaim, EditError, Operation,
};
use sacm::model::{
    AssertionDeclaration, Document, ElementId, ElementKind, LangString, SacmElement,
};
use sacm::validation;
use sacm::xmi::{self, Mode, ReadError, ReadOptions, ReadOutput, WriteOptions};

const ARG_007: &str = "SACM23-ARG-007";
const ARG_008: &str = "SACM23-ARG-008";
const ARG_010: &str = "SACM23-ARG-010";

const MODES: [Mode; 2] = [Mode::Strict, Mode::Tolerant];

/// Marks, in a test input, where a refusal or an occurrence is expected.
/// Removed before reading.
const MARK: char = '\u{2038}';

/// The namespace declarations of a bare root: `sacm`, `xmi` and `xsi`.
const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
    " xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"",
);

/// The two relationship kinds of this slice, each with its row.
const KINDS: [(&str, ElementKind); 2] = [
    (ARG_008, ElementKind::AssertedInference),
    (ARG_010, ElementKind::AssertedContext),
];

/// The five `AssertedRelationship` kinds (§11.13 to §11.18).
const RELATIONSHIP_KINDS: [ElementKind; 5] = [
    ElementKind::AssertedInference,
    ElementKind::AssertedEvidence,
    ElementKind::AssertedContext,
    ElementKind::AssertedArtifactSupport,
    ElementKind::AssertedArtifactContext,
];

/// The valid fixture of the slice.
const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/sacm23/xmi/asserted-relationships-valid.sacm.xmi"
);

/// The golden file of SACM23-ARG-007, -008 and -010.
const GOLDEN: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/sacm23/xmi/golden/asserted-relationships.golden.sacm.xmi"
);

/// The document of [`GOLDEN`] in another form: attributes in other orders,
/// Boolean literals `1`, white space in `source`, and `source`, `target`,
/// `metaClaim` and `isAbstract` as elements.
const GOLDEN_SCRAMBLED: &str = concat!(
    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
    "<sacm:ArgumentPackage xmi:id=\"arguments\" xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\" ",
    "xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\">\n",
    "<name content=\"Arguments\" lang=\"en\"/>\n",
    "<argumentElement xmi:id=\"c1\" xmi:type=\"sacm:Claim\">",
    "<name content=\"The system is acceptably safe\" lang=\"en\"/></argumentElement>\n",
    "<argumentElement xmi:id=\"c2\" xmi:type=\"sacm:Claim\">",
    "<name content=\"Every identified hazard is mitigated\" lang=\"en\"/></argumentElement>\n",
    "<argumentElement xmi:id=\"c3\" xmi:type=\"sacm:Claim\">",
    "<name content=\"The hazard list is complete\" lang=\"en\"/></argumentElement>\n",
    "<argumentElement xmi:id=\"m1\" xmi:type=\"sacm:Claim\">",
    "<name content=\"The inference is sound\" lang=\"en\"/></argumentElement>\n",
    "<argumentElement xmi:id=\"m2\" xmi:type=\"sacm:Claim\">",
    "<name content=\"The context is relevant\" lang=\"en\"/></argumentElement>\n",
    "<argumentElement structure=\"arguments\" gid=\"reasoning-1\" xmi:id=\"r1\" ",
    "xmi:type=\"sacm:ArgumentReasoning\">",
    "<name content=\"Argument over each hazard\" lang=\"en\"/></argumentElement>\n",
    "<argumentElement target=\"c1\" xmi:id=\"i0\" xmi:type=\"sacm:AssertedInference\">",
    "<isAbstract>1</isAbstract><name content=\"{Premises} support {conclusion}\" lang=\"en\"/>",
    "<source xmi:idref=\"c2\"/></argumentElement>\n",
    "<argumentElement target=\"c1\" source=\" c2&#9;c3 \" reasoning=\"r1\" isCounter=\"1\" ",
    "metaClaim=\"m1 m2\" assertionDeclaration=\"assumed\" abstractForm=\"i0\" citedElement=\"i0\" ",
    "isCitation=\"true\" gid=\"inference-1\" xmi:id=\"i1\" xmi:type=\"sacm:AssertedInference\">",
    "<name content=\"The hazards bear on the conclusion\" lang=\"en\"/></argumentElement>\n",
    "<argumentElement xmi:id=\"x0\" xmi:type=\"sacm:AssertedContext\" isAbstract=\"true\">",
    "<target href=\"#i0\"/><name content=\"{Context} of {inference}\" lang=\"en\"/>",
    "<source href=\"#c3\"/></argumentElement>\n",
    "<argumentElement xmi:id=\"x1\" xmi:type=\"sacm:AssertedContext\" gid=\"context-1\" ",
    "isCitation=\"true\" citedElement=\"x0\" abstractForm=\"x0\" assertionDeclaration=\"axiomatic\" ",
    "isCounter=\"true\" reasoning=\"r1\" target=\"i1\"><metaClaim xmi:idref=\"m2\"/>",
    "<name content=\"The hazard list is the context\" lang=\"en\"/>",
    "<source xmi:idref=\"c3\"/><source xmi:idref=\"m2\"/></argumentElement>\n",
    "</sacm:ArgumentPackage>\n",
);

// --- Inputs ------------------------------------------------------------------------

/// A named Claim `id`.
fn claim(id: &str) -> String {
    format!(
        "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"{id}\"><name content=\"C\"/>\
         </argumentElement>"
    )
}

/// An ArgumentPackage root `p` named "P", holding the named Claims c1, c2,
/// c3 and m1 and the named `ArgumentReasoning` r1, then `body`, each on its
/// own line.
fn in_ap(body: &str) -> String {
    format!(
        "<sacm:ArgumentPackage{NS} xmi:id=\"p\">\n  <name content=\"P\"/>\n  {}\n  {}\n  {}\n  {}\n  \
         <argumentElement xmi:type=\"sacm:ArgumentReasoning\" xmi:id=\"r1\"><name content=\"R\"/>\
         </argumentElement>\n  {body}\n</sacm:ArgumentPackage>",
        claim("c1"),
        claim("c2"),
        claim("c3"),
        claim("m1")
    )
}

/// A relationship of `kind` with the id `id`, named "I", with `attributes`
/// and holding `content` after its name.
fn relationship(kind: ElementKind, id: &str, attributes: &str, content: &str) -> String {
    format!(
        "<argumentElement xmi:type=\"sacm:{kind}\" xmi:id=\"{id}\"{attributes}>\
         <name content=\"I\"/>{content}</argumentElement>"
    )
}

/// The relationship `x` of `kind` in [`in_ap`].
fn x_in_ap(kind: ElementKind, attributes: &str, content: &str) -> String {
    in_ap(&relationship(kind, "x", attributes, content))
}

fn loc(row: &str, line: u32, column: u32) -> SourceLocation {
    let Some(location) = SourceLocation::new(line, column) else {
        panic!("{row}: SourceLocation::new({line}, {column}) refused a 1-based position");
    };
    location
}

/// `template` without its marks, and the location of each mark in order:
/// lines end at LF, CRLF or a lone CR, and columns count characters.
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

fn ends(row: &str, doc: &Document, id: &str) -> Ends {
    ends_of(element(row, doc, id))
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

/// The SACM-REF-001 findings of `validate` on `doc`.
fn dangling(doc: &Document) -> Vec<Diagnostic> {
    validation::validate(doc)
        .into_iter()
        .filter(|found| found.code() == Code::REF_DANGLING)
        .collect()
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

/// Asserts that `error` is `RepeatedValue` for `end` on `id`, with exactly
/// `locations` and `location()` the second.
fn assert_repeated(
    row: &str,
    what: &str,
    error: &ReadError,
    id: &str,
    end: &str,
    locations: &[SourceLocation],
) {
    assert!(
        matches!(
            error,
            ReadError::RepeatedValue { element, property, .. }
                if element.as_str() == id && property == end
        ),
        "{row}: {what}: not RepeatedValue {{ {id}, {end} }}: {error:?}"
    );
    assert_eq!(
        (error.code(), error.locations(), error.location()),
        (
            Some(Code::MULTIPLICITY_VIOLATION),
            locations,
            locations.get(1).copied()
        ),
        "{row}: {what}: not SACM-MULT-001 at every occurrence"
    );
}

/// The id prefix `edit::apply` generates for `kind`.
fn prefix(kind: ElementKind) -> &'static str {
    match kind {
        ElementKind::AssertedInference => "asserted_inference",
        ElementKind::AssertedContext => "asserted_context",
        other => panic!("setup: no create operation for {other}"),
    }
}

// --- Editing -------------------------------------------------------------------------

fn named(content: &str) -> LangString {
    LangString::new(content).with_lang("en")
}

/// What a create operation for a relationship is given.
#[derive(Clone)]
struct Spec<'a> {
    parent: &'a str,
    name: &'a str,
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
        name: "Relationship",
        sources: sources.to_vec(),
        target,
        id: None,
        reasoning: None,
        counter: false,
    }
}

/// The create operation of `kind` for `spec`, built as a caller would.
fn operation(kind: ElementKind, spec: &Spec<'_>) -> Operation {
    let sources = spec.sources.iter().copied();
    match kind {
        ElementKind::AssertedInference => {
            let mut create =
                CreateAssertedInference::new(spec.parent, named(spec.name), sources, spec.target);
            if let Some(id) = spec.id {
                create = create.with_id(id);
            }
            if let Some(reasoning) = spec.reasoning {
                create = create.with_reasoning(reasoning);
            }
            if spec.counter {
                create = create.with_is_counter(true);
            }
            create.into()
        }
        ElementKind::AssertedContext => {
            let mut create =
                CreateAssertedContext::new(spec.parent, named(spec.name), sources, spec.target);
            if let Some(id) = spec.id {
                create = create.with_id(id);
            }
            if let Some(reasoning) = spec.reasoning {
                create = create.with_reasoning(reasoning);
            }
            if spec.counter {
                create = create.with_is_counter(true);
            }
            create.into()
        }
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
/// ArgumentPackage `p`, with the Claims c1, c2 and c3 and the
/// `ArgumentReasoning` r1, whose structure is `p`.
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
        ("c1", "The system is safe"),
        ("c2", "The hazards are mitigated"),
        ("c3", "The hazard list is complete"),
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
        CreateArgumentReasoning::new("p", named("Over each hazard"))
            .with_id("r1")
            .with_structure("p"),
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
        create(row, &mut probe, operation(kind, &spec(&["c2"], "c1"))).as_str(),
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

/// Asserts that `error` is `ReferenceNotFound` for `end` and `id`.
fn assert_not_found(row: &str, what: &str, error: &EditError, end: &str, id: &str) {
    let EditError::ReferenceNotFound {
        end: found_end,
        id: found_id,
        ..
    } = error
    else {
        panic!("{row}: {what}: not ReferenceNotFound: {error:?}");
    };
    assert_eq!(
        (end_name(found_end), found_id.as_str(), error.code()),
        (end.to_owned(), id, Code::CMD_TARGET_NOT_FOUND),
        "{row}: {what}: not ReferenceNotFound {{ {end}, {id} }} (SACM-CMD-001)"
    );
    let message = error.to_string();
    assert!(
        message.contains(&format!("{id:?}")) && message.contains(end),
        "{row}: {what}: the message {message:?} does not quote the id and name the end"
    );
}

// =====================================================================================
// SACM23-ARG-007: the AssertedRelationship features
// =====================================================================================

// --- Item 1: the defaults ----------------------------------------------------------------

#[test]
fn sacm23_arg_007_a_relationship_inserted_through_the_seam_has_no_ends_and_the_defaults() {
    let row = ARG_007;
    let mut doc = Document::new();
    let pkg = ElementId::from("pkg");
    if let Err(error) = doc.insert(None, ElementKind::ArtifactPackage, pkg.clone(), None) {
        panic!("{row}: setup: the root could not be inserted: {error:?}");
    }
    for &kind in ElementKind::ALL {
        let id = ElementId::from(format!("e_{kind}"));
        if let Err(error) = doc.insert(Some(&pkg), kind, id, None) {
            panic!("{row}: setup: a {kind} could not be inserted: {error:?}");
        }
    }
    for found in doc.elements() {
        let kind = found.kind();
        assert_eq!(
            ends_of(found),
            expected_ends(&[], None, None, false),
            "{row}: a new {kind} has an end or isCounter set"
        );
        assert!(
            found.meta_claims().is_empty() && found.structure().is_none(),
            "{row}: a new {kind} has a meta-claim or a structure"
        );
        if RELATIONSHIP_KINDS.contains(&kind) {
            assert_eq!(
                found.assertion_declaration(),
                Some(AssertionDeclaration::Asserted),
                "{row}: a new {kind} is not asserted"
            );
        }
    }
}

// --- Item 2: every value, and the attribute order --------------------------------------------

#[test]
fn sacm23_arg_007_every_end_and_flag_reads_on_both_kinds_in_both_modes_and_writes_in_order() {
    let row = ARG_007;
    for (_, kind) in KINDS {
        let what = format!("every value on an {kind}");
        let doc = read_ok(
            row,
            &what,
            &x_in_ap(
                kind,
                " source=\"c2 c3\" target=\"c1\" reasoning=\"r1\" isCounter=\"true\" \
                 assertionDeclaration=\"assumed\" metaClaim=\"m1\"",
                "",
            ),
        );
        let x = element(row, &doc, "x");
        assert_eq!(
            (
                x.kind(),
                ends_of(x),
                x.assertion_declaration(),
                x.meta_claims()
                    .iter()
                    .map(|id| id.as_str().to_owned())
                    .collect::<Vec<_>>()
            ),
            (
                kind,
                expected_ends(&["c2", "c3"], Some("c1"), Some("r1"), true),
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
                 reasoning=\"r1\" source=\"c2 c3\" target=\"c1\">"
            ),
            "{row}: {what}: not written in the pinned order\n{text}"
        );
    }
}

#[test]
fn sacm23_arg_007_the_golden_file_pins_the_attribute_order_of_both_kinds() {
    let row = ARG_007;
    let golden = std::fs::read_to_string(GOLDEN)
        .unwrap_or_else(|error| panic!("{row}: cannot read {GOLDEN}: {error}"));
    let doc = read_ok(row, "the golden document in another form", GOLDEN_SCRAMBLED);
    let kinds: Vec<ElementKind> = doc.elements().map(SacmElement::kind).collect();
    for (_, kind) in KINDS {
        assert!(
            kinds.contains(&kind),
            "{row}: setup: the golden document has no {kind}"
        );
    }
    assert_eq!(
        validation::validate(&doc),
        [],
        "{row}: setup: the golden document does not validate cleanly"
    );
    for mode in MODES {
        let text = write_in(row, "the golden document", &doc, mode);
        assert_eq!(
            text, golden,
            "{row}: a {mode:?} write of the golden document is not the golden bytes"
        );
    }
}

// --- Item 3: isCounter ------------------------------------------------------------------------

#[test]
fn sacm23_arg_007_is_counter_reads_xsd_boolean_literals_and_is_written_only_when_true() {
    let row = ARG_007;
    for (_, kind) in KINDS {
        let base = " source=\"c2\" target=\"c1\"";
        let absent = read_ok(row, "no isCounter", &x_in_ap(kind, base, ""));
        for (value, expected) in [
            ("true", true),
            ("1", true),
            (" true ", true),
            ("&#9;1&#10;", true),
            ("false", false),
            ("0", false),
            (" 0 ", false),
        ] {
            let what = format!("isCounter={value:?} on an {kind}");
            let doc = read_ok(
                row,
                &what,
                &x_in_ap(kind, &format!("{base} isCounter=\"{value}\""), ""),
            );
            assert_eq!(
                element(row, &doc, "x").is_counter(),
                expected,
                "{row}: {what}: not read as {expected}"
            );
            let text = round_trip(row, &what, &doc, Mode::Strict);
            let line = line_with(row, &text, "x");
            if expected {
                assert!(
                    line.contains(" isCounter=\"true\" ") && !line.contains("isCounter=\"1\""),
                    "{row}: {what}: not written as isCounter=\"true\"\n{text}"
                );
            } else {
                assert!(
                    !line.contains("isCounter"),
                    "{row}: {what}: a false isCounter is written\n{text}"
                );
                assert_eq!(
                    (semantic_compare(&doc, &absent), &doc),
                    (Vec::new(), &absent),
                    "{row}: {what}: does not compare equal to an absent isCounter"
                );
            }
        }
        assert!(
            !element(row, &absent, "x").is_counter(),
            "{row}: an absent isCounter is not false"
        );
        // The element form.
        let what = format!("<isCounter>1</isCounter> on an {kind}");
        let doc = read_ok(row, &what, &x_in_ap(kind, base, "<isCounter>1</isCounter>"));
        assert!(
            element(row, &doc, "x").is_counter(),
            "{row}: {what}: not read as true"
        );
        let text = round_trip(row, &what, &doc, Mode::Strict);
        assert!(
            line_with(row, &text, "x").contains(" isCounter=\"true\" ")
                && !text.contains("<isCounter"),
            "{row}: {what}: not written as the attribute\n{text}"
        );
    }
}

#[test]
fn sacm23_arg_007_a_value_that_is_no_boolean_literal_is_invalid_literal_naming_is_counter() {
    let row = ARG_007;
    for (_, kind) in KINDS {
        let mut cases: Vec<(String, String, String, &str)> = Vec::new();
        for value in ["TRUE", "True", "yes", "", "t rue", "2", "false true"] {
            cases.push((
                format!("isCounter={value:?}"),
                format!(" source=\"c2\" target=\"c1\" {MARK}isCounter=\"{value}\""),
                String::new(),
                value,
            ));
        }
        for value in ["TRUE", "yes", "t rue"] {
            cases.push((
                format!("<isCounter>{value}</isCounter>"),
                " source=\"c2\" target=\"c1\"".to_owned(),
                format!("{MARK}<isCounter>{value}</isCounter>"),
                value,
            ));
        }
        for (form, attributes, content, value) in &cases {
            let what = format!("{form} on an {kind}");
            let (input, marks) = marked(row, &x_in_ap(kind, attributes, content));
            let error = refused(row, &what, &input);
            assert!(
                matches!(
                    &error,
                    ReadError::InvalidLiteral { element, property, value: read, .. }
                        if element.as_str() == "x" && property == "isCounter" && read == value
                ),
                "{row}: {what}: not InvalidLiteral {{ x, isCounter, {value:?} }}: {error:?}"
            );
            assert_eq!(
                (error.code(), error.location()),
                (Some(Code::ENUM_INVALID_LITERAL), marks.first().copied()),
                "{row}: {what}: not SACM-ENUM-001 at the value"
            );
            let shown = error.to_string();
            assert!(
                shown.contains(&format!("{value:?}"))
                    && shown.contains("isCounter")
                    && !shown.contains('\n'),
                "{row}: {what}: the Display {shown:?} is not one line quoting the value and naming isCounter"
            );
        }
    }
}

#[test]
fn sacm23_arg_007_is_counter_given_twice_is_repeated_value_and_in_the_sacm_namespace_is_unsupported()
 {
    let row = ARG_007;
    for (_, kind) in KINDS {
        for (what, attributes, content) in [
            (
                "the attribute and an element",
                format!(" source=\"c2\" target=\"c1\" {MARK}isCounter=\"true\""),
                format!("{MARK}<isCounter>true</isCounter>"),
            ),
            (
                "two elements",
                " source=\"c2\" target=\"c1\"".to_owned(),
                format!("{MARK}<isCounter>1</isCounter>{MARK}<isCounter>0</isCounter>"),
            ),
        ] {
            let what = format!("isCounter: {what} on an {kind}");
            let (input, marks) = marked(row, &x_in_ap(kind, &attributes, &content));
            let error = refused(row, &what, &input);
            assert_repeated(row, &what, &error, "x", "isCounter", &marks);
        }
        for (what, attributes, content) in [
            (
                "sacm:isCounter as an attribute",
                format!(" source=\"c2\" target=\"c1\" {MARK}sacm:isCounter=\"true\""),
                String::new(),
            ),
            (
                "sacm:isCounter as an element",
                " source=\"c2\" target=\"c1\"".to_owned(),
                format!("{MARK}<sacm:isCounter>true</sacm:isCounter>"),
            ),
        ] {
            let what = format!("{what} on an {kind}");
            let (input, marks) = marked(row, &x_in_ap(kind, &attributes, &content));
            let error = refused(row, &what, &input);
            assert_unsupported_at(row, &what, &error, marks[0]);
        }
    }
}

// --- Item 4: on other kinds -------------------------------------------------------------------

#[test]
fn sacm23_arg_007_is_counter_source_target_and_reasoning_on_a_claim_a_reasoning_or_a_package_are_unsupported()
 {
    let row = ARG_007;
    let features = [
        ("isCounter", "true", "<isCounter>true</isCounter>"),
        ("source", "c1", "<source xmi:idref=\"c1\"/>"),
        ("target", "c1", "<target href=\"#c1\"/>"),
        ("reasoning", "r1", "<reasoning xmi:idref=\"r1\"/>"),
    ];
    for (feature, value, element_form) in features {
        let attribute = format!(" {MARK}{feature}=\"{value}\"");
        let element_form = format!("{MARK}{element_form}");
        let cases = [
            (
                "a Claim, as an attribute",
                in_ap(&format!(
                    "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"x\"{attribute}>\
                     <name content=\"X\"/></argumentElement>"
                )),
            ),
            (
                "a Claim, as an element",
                in_ap(&format!(
                    "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"x\">\
                     <name content=\"X\"/>{element_form}</argumentElement>"
                )),
            ),
            (
                "an ArgumentReasoning, as an attribute",
                in_ap(&format!(
                    "<argumentElement xmi:type=\"sacm:ArgumentReasoning\" xmi:id=\"x\"{attribute}>\
                     <name content=\"X\"/></argumentElement>"
                )),
            ),
            (
                "an ArgumentReasoning, as an element",
                in_ap(&format!(
                    "<argumentElement xmi:type=\"sacm:ArgumentReasoning\" xmi:id=\"x\">\
                     <name content=\"X\"/>{element_form}</argumentElement>"
                )),
            ),
            (
                "an ArgumentPackage root, as an attribute",
                format!(
                    "<sacm:ArgumentPackage{NS} xmi:id=\"p\"{attribute}><name content=\"P\"/>{}\
                     </sacm:ArgumentPackage>",
                    claim("c1")
                ),
            ),
            (
                "an ArgumentPackage child, as an element",
                format!(
                    "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\"><name content=\"Case\"/>\
                     <argumentPackage xmi:id=\"p\"><name content=\"P\"/>{element_form}\
                     </argumentPackage></sacm:AssuranceCasePackage>"
                ),
            ),
        ];
        for (holder, template) in &cases {
            let what = format!("{feature} on {holder}");
            let (input, marks) = marked(row, template);
            assert_eq!(marks.len(), 1, "{row}: setup: {what} has not one mark");
            let error = refused(row, &what, &input);
            assert_unsupported_at(row, &what, &error, marks[0]);
        }
    }
}

// --- Item 5: many sources, one target, one reasoning -------------------------------------------------

#[test]
fn sacm23_arg_007_source_elements_accumulate_in_order_and_a_repeated_source_is_held_twice() {
    let row = ARG_007;
    for (_, kind) in KINDS {
        let what = format!("<source> elements around the name on an {kind}");
        let input = in_ap(&format!(
            "<argumentElement xmi:type=\"sacm:{kind}\" xmi:id=\"x\" target=\"c1\">\
             <source xmi:idref=\"c3\"/><name content=\"I\"/><source href=\"#c2\"/>\
             <note xmi:id=\"n\"/><source xmi:idref=\" c3 \"/></argumentElement>"
        ));
        let doc = read_ok(row, &what, &input);
        assert_eq!(
            ends(row, &doc, "x"),
            expected_ends(&["c3", "c2", "c3"], Some("c1"), None, false),
            "{row}: {what}: not accumulated in document order"
        );
        let text = round_trip(row, &what, &doc, Mode::Strict);
        assert!(
            line_with(row, &text, "x").ends_with(" source=\"c3 c2 c3\" target=\"c1\">")
                && !text.contains("<source"),
            "{row}: {what}: not written as one attribute\n{text}"
        );
        // Target and reasoning in the element form.
        let what = format!("<target> and <reasoning> elements on an {kind}");
        let doc = read_ok(
            row,
            &what,
            &x_in_ap(
                kind,
                " source=\"c2\"",
                "<target xmi:idref=\"c1\"/><reasoning href=\"#r1\"/>",
            ),
        );
        assert_eq!(
            ends(row, &doc, "x"),
            expected_ends(&["c2"], Some("c1"), Some("r1"), false),
            "{row}: {what}: not read"
        );
        // A repeated source.
        let what = format!("source=\"c1 c1\" on an {kind}");
        let doc = read_ok(
            row,
            &what,
            &x_in_ap(kind, " source=\"c1 c1\" target=\"c2\"", ""),
        );
        assert_eq!(
            ends(row, &doc, "x"),
            expected_ends(&["c1", "c1"], Some("c2"), None, false),
            "{row}: {what}: the repeat is not held"
        );
        let text = round_trip(row, &what, &doc, Mode::Tolerant);
        assert!(
            line_with(row, &text, "x").ends_with(" source=\"c1 c1\" target=\"c2\">"),
            "{row}: {what}: not written back unchanged\n{text}"
        );
    }
}

#[test]
fn sacm23_arg_007_source_in_both_forms_and_a_second_target_or_reasoning_are_repeated_value() {
    let row = ARG_007;
    for (_, kind) in KINDS {
        let cases = [
            (
                "source as an attribute and an element",
                "source",
                format!(" {MARK}source=\"c2\" target=\"c1\""),
                format!("{MARK}<source xmi:idref=\"c3\"/>"),
            ),
            (
                "target=\"c1 c2\"",
                "target",
                format!(" source=\"c3\" {MARK}{MARK}target=\"c1 c2\""),
                String::new(),
            ),
            (
                "two target elements",
                "target",
                " source=\"c3\"".to_owned(),
                format!("{MARK}<target xmi:idref=\"c1\"/>{MARK}<target href=\"#c2\"/>"),
            ),
            (
                "target as an attribute and an element",
                "target",
                format!(" source=\"c3\" {MARK}target=\"c1\""),
                format!("{MARK}<target xmi:idref=\"c1\"/>"),
            ),
            (
                "reasoning=\"r1 c1\"",
                "reasoning",
                format!(" source=\"c3\" target=\"c1\" {MARK}{MARK}reasoning=\"r1 c1\""),
                String::new(),
            ),
            (
                "two reasoning elements",
                "reasoning",
                " source=\"c3\" target=\"c1\"".to_owned(),
                format!("{MARK}<reasoning xmi:idref=\"r1\"/>{MARK}<reasoning xmi:idref=\"r1\"/>"),
            ),
        ];
        for (what, end, attributes, content) in &cases {
            let what = format!("{what} on an {kind}");
            let (input, marks) = marked(row, &x_in_ap(kind, attributes, content));
            let error = refused(row, &what, &input);
            assert_repeated(row, &what, &error, "x", end, &marks);
        }
        // An attribute of two ids and an element: refused, at both. ADR 0027
        // does not say whether the attribute counts once or per id, so only
        // that both places are named is asserted.
        let what = format!("source=\"c2 c3\" and a source element on an {kind}");
        let (input, marks) = marked(
            row,
            &x_in_ap(
                kind,
                &format!(" target=\"c1\" {MARK}source=\"c2 c3\""),
                &format!("{MARK}<source href=\"#c1\"/>"),
            ),
        );
        let error = refused(row, &what, &input);
        assert!(
            matches!(
                &error,
                ReadError::RepeatedValue { element, property, .. }
                    if element.as_str() == "x" && property == "source"
            ) && marks.iter().all(|at| error.locations().contains(at)),
            "{row}: {what}: not RepeatedValue {{ x, source }} naming the attribute and the element: {error:?}"
        );
        for (what, attribute) in [
            ("source=\"\"", format!(" {MARK}source=\"\" target=\"c1\"")),
            ("target=\" \"", format!(" source=\"c2\" {MARK}target=\" \"")),
            (
                "reasoning=\"\"",
                format!(" source=\"c2\" target=\"c1\" {MARK}reasoning=\"\""),
            ),
            (
                "sacm:source",
                format!(" {MARK}sacm:source=\"c2\" target=\"c1\""),
            ),
        ] {
            let what = format!("{what} on an {kind}");
            let (input, marks) = marked(row, &x_in_ap(kind, &attribute, ""));
            let error = refused(row, &what, &input);
            assert_unsupported_at(row, &what, &error, marks[0]);
        }
    }
}

// --- Item 6: missing ends are kept -----------------------------------------------------------------

#[test]
fn sacm23_arg_007_a_relationship_missing_its_source_or_target_reads_and_writes_tolerantly_as_read()
{
    let row = ARG_007;
    for (_, kind) in KINDS {
        for (what, attributes, expected, tail) in [
            (
                "no source",
                " target=\"c1\"",
                expected_ends(&[], Some("c1"), None, false),
                " target=\"c1\">",
            ),
            (
                "no target",
                " source=\"c2\"",
                expected_ends(&["c2"], None, None, false),
                " source=\"c2\">",
            ),
            (
                "neither",
                "",
                expected_ends(&[], None, None, false),
                " xmi:id=\"x\">",
            ),
        ] {
            let what = format!("an {kind} with {what}");
            let doc = read_ok(row, &what, &x_in_ap(kind, attributes, ""));
            assert_eq!(
                ends(row, &doc, "x"),
                expected,
                "{row}: {what}: not kept as read"
            );
            let text = round_trip(row, &what, &doc, Mode::Tolerant);
            let line = line_with(row, &text, "x");
            assert!(
                line.ends_with(tail),
                "{row}: {what}: something was added to the start tag: {line}"
            );
        }
    }
}

// --- Item 7: references --------------------------------------------------------------------------

#[test]
fn sacm23_arg_007_a_dangling_source_target_or_reasoning_is_ref_001_with_clause_11_13() {
    let row = ARG_007;
    for (_, kind) in KINDS {
        for (end, attributes) in [
            ("source", " source=\"c2 nope\" target=\"c1\""),
            ("target", " source=\"c2\" target=\"nope\""),
            (
                "reasoning",
                " source=\"c2\" target=\"c1\" reasoning=\"nope\"",
            ),
        ] {
            let what = format!("a dangling {end} on an {kind}");
            let doc = read_ok(row, &what, &x_in_ap(kind, attributes, ""));
            let found = dangling(&doc);
            let [finding] = found.as_slice() else {
                panic!("{row}: {what}: not one SACM-REF-001: {found:?}");
            };
            assert_eq!(
                (
                    finding.severity(),
                    finding.requirement_id(),
                    finding.clause(),
                    finding.location(),
                    finding.affected()
                ),
                (
                    Severity::Error,
                    "SACM23-VAL-003",
                    Some("11.13"),
                    None,
                    [ElementId::from("x"), ElementId::from("nope")].as_slice()
                ),
                "{row}: {what}: not an Error of SACM23-VAL-003 with clause 11.13 on [x, nope]"
            );
            assert!(
                finding.message().contains(end) && finding.message().contains(kind.name()),
                "{row}: {what}: the message does not name the end and the kind: {finding:?}"
            );
            let text = round_trip(row, &what, &doc, Mode::Tolerant);
            assert!(
                line_with(row, &text, "x").contains("nope"),
                "{row}: {what}: the dangling id is not written back\n{text}"
            );
        }
    }
}

#[test]
fn sacm23_arg_007_a_context_of_an_inference_and_a_relationship_that_is_its_own_source_give_no_finding()
 {
    let row = ARG_007;
    let what = "an AssertedContext targeting an AssertedInference";
    let doc = read_ok(
        row,
        what,
        &in_ap(&format!(
            "{}{}",
            relationship(
                ElementKind::AssertedContext,
                "x",
                " source=\"c3\" target=\"i\"",
                ""
            ),
            relationship(
                ElementKind::AssertedInference,
                "i",
                " source=\"c2\" target=\"c1\"",
                ""
            )
        )),
    );
    assert_eq!(
        validation::validate(&doc),
        [],
        "{row}: {what}: validate reports a finding"
    );
    round_trip(row, what, &doc, Mode::Strict);
    for (_, kind) in KINDS {
        let what = format!("an {kind} that is its own source");
        let doc = read_ok(
            row,
            &what,
            &x_in_ap(kind, " source=\"x c2\" target=\"c1\" reasoning=\"r1\"", ""),
        );
        assert_eq!(
            validation::validate(&doc),
            [],
            "{row}: {what}: validate reports a finding"
        );
        round_trip(row, &what, &doc, Mode::Strict);
    }
}

// --- Item 8: assertionDeclaration ----------------------------------------------------------------

#[test]
fn sacm23_arg_007_all_six_declarations_round_trip_on_both_kinds_and_a_bad_literal_is_invalid_literal()
 {
    let row = ARG_007;
    for (_, kind) in KINDS {
        for &declaration in AssertionDeclaration::ALL {
            let literal = declaration.as_str();
            let what = format!("assertionDeclaration=\"{literal}\" on an {kind}");
            let doc = read_ok(
                row,
                &what,
                &x_in_ap(
                    kind,
                    &format!(" assertionDeclaration=\"{literal}\" source=\"c2\" target=\"c1\""),
                    "",
                ),
            );
            assert_eq!(
                element(row, &doc, "x").assertion_declaration(),
                Some(declaration),
                "{row}: {what}: not read"
            );
            let text = round_trip(row, &what, &doc, Mode::Strict);
            let line = line_with(row, &text, "x");
            let attribute = format!(" assertionDeclaration=\"{literal}\" source=\"c2\"");
            if declaration == AssertionDeclaration::Asserted {
                assert!(
                    !line.contains("assertionDeclaration"),
                    "{row}: {what}: the default is written: {line}"
                );
            } else {
                assert!(
                    line.contains(&attribute),
                    "{row}: {what}: not written before the ends: {line}"
                );
            }
        }
        let what = format!("assertionDeclaration=\"Assumed\" on an {kind}");
        let (input, marks) = marked(
            row,
            &x_in_ap(
                kind,
                &format!(" source=\"c2\" target=\"c1\" {MARK}assertionDeclaration=\"Assumed\""),
                "",
            ),
        );
        let error = refused(row, &what, &input);
        assert!(
            matches!(
                &error,
                ReadError::InvalidLiteral { element, property, value, .. }
                    if element.as_str() == "x" && property == "assertionDeclaration" && value == "Assumed"
            ),
            "{row}: {what}: not InvalidLiteral: {error:?}"
        );
        assert_eq!(
            (error.code(), error.location()),
            (Some(Code::ENUM_INVALID_LITERAL), marks.first().copied()),
            "{row}: {what}: not SACM-ENUM-001 at the attribute"
        );
    }
}

// --- Item 9: comparison ----------------------------------------------------------------------------

#[test]
fn sacm23_arg_007_semantic_compare_reports_is_counter_reasoning_sources_and_target() {
    let row = ARG_007;
    for (_, kind) in KINDS {
        let doc = |attributes: &str| read_ok(row, attributes, &x_in_ap(kind, attributes, ""));
        let id = || ElementId::from("x");
        let some = |value: &str| Some(ElementId::from(value));
        let list = |values: &[&str]| -> Vec<ElementId> {
            values.iter().copied().map(ElementId::from).collect()
        };
        for (left, right, expected) in [
            (
                " source=\"c2\" target=\"c1\" isCounter=\"true\"",
                " source=\"c2\" target=\"c1\"",
                Difference::IsCounter {
                    id: id(),
                    left: true,
                    right: false,
                },
            ),
            (
                " source=\"c2\" target=\"c1\" reasoning=\"r1\"",
                " source=\"c2\" target=\"c1\"",
                Difference::Reasoning {
                    id: id(),
                    left: some("r1"),
                    right: None,
                },
            ),
            (
                " source=\"c2 c3\" target=\"c1\"",
                " source=\"c3 c2\" target=\"c1\"",
                Difference::Sources {
                    id: id(),
                    left: list(&["c2", "c3"]),
                    right: list(&["c3", "c2"]),
                },
            ),
            (
                " source=\"c2\" target=\"c1\"",
                " source=\"c2 c2\" target=\"c1\"",
                Difference::Sources {
                    id: id(),
                    left: list(&["c2"]),
                    right: list(&["c2", "c2"]),
                },
            ),
            (
                " source=\"c2\" target=\"c1\"",
                " source=\"c2\" target=\"c3\"",
                Difference::Target {
                    id: id(),
                    left: some("c1"),
                    right: some("c3"),
                },
            ),
            (
                " source=\"c2\" target=\"c1\"",
                " source=\"c2\"",
                Difference::Target {
                    id: id(),
                    left: some("c1"),
                    right: None,
                },
            ),
        ] {
            assert_eq!(
                semantic_compare(&doc(left), &doc(right)),
                [expected],
                "{row}: {kind}: {left:?} vs {right:?}: not the one difference"
            );
        }
    }
}

#[test]
fn sacm23_arg_007_semantic_compare_reports_every_relationship_feature_after_gid_in_table_order() {
    let row = ARG_007;
    for (_, kind) in KINDS {
        let doc = |attributes: &str| read_ok(row, attributes, &x_in_ap(kind, attributes, ""));
        let id = || ElementId::from("x");
        let some = |value: &str| Some(ElementId::from(value));
        let list = |values: &[&str]| -> Vec<ElementId> {
            values.iter().copied().map(ElementId::from).collect()
        };
        // Every feature at once: in the order after Gid.
        let left = doc(
            " gid=\"g\" isCitation=\"true\" isAbstract=\"true\" citedElement=\"c1\" abstractForm=\"x\" \
             metaClaim=\"m1\" isCounter=\"true\" reasoning=\"r1\" source=\"c2\" target=\"c1\"",
        );
        let right = doc("");
        assert_eq!(
            semantic_compare(&left, &right),
            [
                Difference::Gid {
                    id: id(),
                    left: Some("g".to_owned()),
                    right: None,
                },
                Difference::IsCitation {
                    id: id(),
                    left: true,
                    right: false,
                },
                Difference::IsAbstract {
                    id: id(),
                    left: true,
                    right: false,
                },
                Difference::IsCounter {
                    id: id(),
                    left: true,
                    right: false,
                },
                Difference::CitedElement {
                    id: id(),
                    left: some("c1"),
                    right: None,
                },
                Difference::AbstractForm {
                    id: id(),
                    left: some("x"),
                    right: None,
                },
                Difference::MetaClaims {
                    id: id(),
                    left: list(&["m1"]),
                    right: Vec::new(),
                },
                Difference::Reasoning {
                    id: id(),
                    left: some("r1"),
                    right: None,
                },
                Difference::Sources {
                    id: id(),
                    left: list(&["c2"]),
                    right: Vec::new(),
                },
                Difference::Target {
                    id: id(),
                    left: some("c1"),
                    right: None,
                },
            ],
            "{row}: {kind}: the differences are not in the order after Gid"
        );
        let found = semantic_compare(&left, &right);
        assert!(
            found
                .iter()
                .all(|difference| difference.element_id() == Some(&id())),
            "{row}: {kind}: element_id() does not name the relationship"
        );
    }
}

// =====================================================================================
// SACM23-ARG-008 and SACM23-ARG-010: the two kinds
// =====================================================================================

// --- Item 1: reading ---------------------------------------------------------------------------

fn reads_by_either_type_and_is_refused_untyped_or_in_a_case_package(row: &str, kind: ElementKind) {
    for type_attribute in [
        format!(" xmi:type=\"sacm:{kind}\""),
        format!(" xsi:type=\"sacm:{kind}\""),
        format!(" xsi:type=\"sacm:{kind}\" xmi:type=\"sacm:{kind}\""),
    ] {
        let what = format!("an {kind} by{type_attribute}");
        let doc = read_typed(
            row,
            &what,
            &in_ap(&format!(
                "<argumentElement{type_attribute} xmi:id=\"x\" source=\"c2 c3\" target=\"c1\" \
                 reasoning=\"r1\"><name content=\"I\"/></argumentElement>"
            )),
        );
        let x = element(row, &doc, "x");
        assert_eq!(
            (
                x.kind(),
                x.parent().map(|parent| parent.id().as_str()),
                ends_of(x)
            ),
            (
                kind,
                Some("p"),
                expected_ends(&["c2", "c3"], Some("c1"), Some("r1"), false)
            ),
            "{row}: {what}: not read with its ends"
        );
        let text = round_trip(row, &what, &doc, Mode::Strict);
        assert!(
            line_with(row, &text, "x").starts_with(&format!(
                "<argumentElement xmi:type=\"sacm:{kind}\" xmi:id=\"x\""
            )),
            "{row}: {what}: not written by xmi:type under argumentElement\n{text}"
        );
    }
    for (what, template) in [
        (
            "an untyped argumentElement with ends".to_owned(),
            in_ap(&format!(
                "{MARK}<argumentElement xmi:id=\"x\" source=\"c2\" target=\"c1\"><name content=\"I\"/>\
                 </argumentElement>"
            )),
        ),
        (
            format!("an {kind} in argumentPackage of an AssuranceCasePackage"),
            format!(
                "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\"><name content=\"Case\"/>\
                 {MARK}<argumentPackage xmi:type=\"sacm:{kind}\" xmi:id=\"x\"/></sacm:AssuranceCasePackage>"
            ),
        ),
        (
            format!("an {kind} in argumentElement of an AssuranceCasePackage"),
            format!(
                "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\"><name content=\"Case\"/>\
                 {MARK}<argumentElement xmi:type=\"sacm:{kind}\" xmi:id=\"x\"/></sacm:AssuranceCasePackage>"
            ),
        ),
        (
            format!("an {kind} in a Claim"),
            in_ap(&format!(
                "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"y\"><name content=\"Y\"/>\
                 {MARK}<argumentElement xmi:type=\"sacm:{kind}\" xmi:id=\"x\"/></argumentElement>"
            )),
        ),
    ] {
        let (input, marks) = marked(row, &template);
        let error = refused(row, &what, &input);
        assert_unsupported_at(row, &what, &error, marks[0]);
    }
}

#[test]
fn sacm23_arg_008_reads_with_its_ends_by_xmi_type_or_xsi_type_and_is_refused_untyped_or_in_a_case_package()
 {
    reads_by_either_type_and_is_refused_untyped_or_in_a_case_package(
        ARG_008,
        ElementKind::AssertedInference,
    );
}

#[test]
fn sacm23_arg_010_reads_with_its_ends_by_xmi_type_or_xsi_type_and_is_refused_untyped_or_in_a_case_package()
 {
    reads_by_either_type_and_is_refused_untyped_or_in_a_case_package(
        ARG_010,
        ElementKind::AssertedContext,
    );
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
fn sacm23_arg_008_the_fixture_reads_with_its_inferences_and_round_trips() {
    the_fixture_reads_and_round_trips(
        ARG_008,
        ElementKind::AssertedInference,
        &[
            (
                "i1",
                expected_ends(&["c2", "c3"], Some("c1"), Some("r1"), false),
            ),
            ("i2", expected_ends(&["c4"], Some("i1"), None, true)),
        ],
    );
}

#[test]
fn sacm23_arg_010_the_fixture_reads_with_its_context_and_round_trips() {
    the_fixture_reads_and_round_trips(
        ARG_010,
        ElementKind::AssertedContext,
        &[("x1", expected_ends(&["c3"], Some("i1"), None, false))],
    );
}

// --- Item 2: the golden file -------------------------------------------------------------------

fn the_golden_file_round_trips_to_its_own_bytes(row: &str, kind: ElementKind) {
    let golden = std::fs::read_to_string(GOLDEN)
        .unwrap_or_else(|error| panic!("{row}: cannot read {GOLDEN}: {error}"));
    let doc = read_ok(row, "the golden file", &golden);
    assert!(
        doc.elements().filter(|found| found.kind() == kind).count() >= 2,
        "{row}: setup: the golden file does not hold two {kind}s"
    );
    let text = round_trip(row, "the golden file", &doc, Mode::Strict);
    assert_eq!(
        text, golden,
        "{row}: the golden file does not write back to its own bytes"
    );
}

#[test]
fn sacm23_arg_008_the_golden_file_round_trips_to_its_own_bytes() {
    the_golden_file_round_trips_to_its_own_bytes(ARG_008, ElementKind::AssertedInference);
}

#[test]
fn sacm23_arg_010_the_golden_file_round_trips_to_its_own_bytes() {
    the_golden_file_round_trips_to_its_own_bytes(ARG_010, ElementKind::AssertedContext);
}

// --- Item 3: the create operation --------------------------------------------------------------------

fn the_create_operation_reports_its_arguments(row: &str, kind: ElementKind) {
    let name = named("Relationship");
    // Without the options, then with each.
    let (parent, id, given_name, sources, target, reasoning, counter, operation_name) = match kind {
        ElementKind::AssertedInference => {
            let create = CreateAssertedInference::new("p", name.clone(), ["c2", "c3"], "c1");
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
                create.parent().clone(),
                create.id().cloned(),
                create.name().clone(),
                create.sources().to_vec(),
                create.target().clone(),
                create.reasoning().cloned(),
                create.is_counter(),
                Operation::from(create).operation_name(),
            )
        }
        ElementKind::AssertedContext => {
            let create = CreateAssertedContext::new("p", name.clone(), ["c2", "c3"], "c1");
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
                create.parent().clone(),
                create.id().cloned(),
                create.name().clone(),
                create.sources().to_vec(),
                create.target().clone(),
                create.reasoning().cloned(),
                create.is_counter(),
                Operation::from(create).operation_name(),
            )
        }
        other => panic!("{row}: setup: no create operation for {other}"),
    };
    assert_eq!(
        (
            parent.as_str(),
            id.as_ref().map(ElementId::as_str),
            given_name,
            sources.iter().map(ElementId::as_str).collect::<Vec<_>>(),
            target.as_str(),
            reasoning.as_ref().map(ElementId::as_str),
            counter,
            operation_name
        ),
        (
            "p",
            Some("given"),
            name,
            vec!["c2", "c3"],
            "c1",
            Some("r1"),
            true,
            format!("Create{kind}").as_str()
        ),
        "{row}: the operation does not report what it was given"
    );
}

#[test]
fn sacm23_arg_008_create_asserted_inference_reports_what_it_was_given() {
    the_create_operation_reports_its_arguments(ARG_008, ElementKind::AssertedInference);
}

#[test]
fn sacm23_arg_010_create_asserted_context_reports_what_it_was_given() {
    the_create_operation_reports_its_arguments(ARG_010, ElementKind::AssertedContext);
}

fn the_create_operation_creates_with_its_ends(row: &str, kind: ElementKind) {
    let mut doc = built(row);
    let generated = format!("{}_1", prefix(kind));
    let id = create(row, &mut doc, operation(kind, &spec(&["c2", "c3"], "c1")));
    assert_eq!(id.as_str(), generated, "{row}: the generated id");
    let found = element(row, &doc, &generated);
    assert_eq!(
        (
            found.kind(),
            found.parent().map(|parent| parent.id().as_str()),
            found.name(),
            ends_of(found),
            found.assertion_declaration(),
            found.meta_claims().len()
        ),
        (
            kind,
            Some("p"),
            Some(&named("Relationship")),
            expected_ends(&["c2", "c3"], Some("c1"), None, false),
            Some(AssertionDeclaration::Asserted),
            0
        ),
        "{row}: the created {kind}"
    );
    assert_eq!(
        element(row, &doc, "p")
            .children()
            .last()
            .map(|child| child.id().as_str().to_owned()),
        Some(generated.clone()),
        "{row}: not created as the last child"
    );
    // With every option, a repeated source and a relationship as target.
    let mut full = spec(&["c3", "r1"], generated.as_str());
    full.id = Some("given");
    full.reasoning = Some("r1");
    full.counter = true;
    let id = create(row, &mut doc, operation(kind, &full));
    assert_eq!(id.as_str(), "given", "{row}: with_id is not used");
    assert_eq!(
        ends(row, &doc, "given"),
        expected_ends(&["c3", "r1"], Some(generated.as_str()), Some("r1"), true),
        "{row}: the ends given are not set as given"
    );
    let id = create(row, &mut doc, operation(kind, &spec(&["c1"], "c2")));
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
    // A repeated source is not refused: it is a content finding
    // (SACM23-VAL-022), not a create check (ADR 0027).
    let id = create(
        row,
        &mut doc,
        operation(kind, &spec(&["c1", "c2", "c1"], "c3")),
    );
    assert_eq!(
        ends(row, &doc, id.as_str()),
        expected_ends(&["c1", "c2", "c1"], Some("c3"), None, false),
        "{row}: a repeated source is not held as given"
    );
}

#[test]
fn sacm23_arg_008_create_asserted_inference_creates_with_its_ends_and_a_generated_id() {
    the_create_operation_creates_with_its_ends(ARG_008, ElementKind::AssertedInference);
}

#[test]
fn sacm23_arg_010_create_asserted_context_creates_with_its_ends_and_a_generated_id() {
    the_create_operation_creates_with_its_ends(ARG_010, ElementKind::AssertedContext);
}

fn the_create_operation_refuses_unchanged(row: &str, kind: ElementKind) {
    let mut doc = built(row);
    // No source.
    let what = "no source";
    let error = refuse(row, what, &mut doc, kind, operation(kind, &spec(&[], "c1")));
    let EditError::TooFewReferences {
        end,
        given,
        required,
        ..
    } = &error
    else {
        panic!("{row}: {what}: not TooFewReferences: {error:?}");
    };
    assert_eq!(
        (end_name(end), *given == 0, *required == 1, error.code()),
        (
            "source".to_owned(),
            true,
            true,
            Code::MULTIPLICITY_VIOLATION
        ),
        "{row}: {what}: not TooFewReferences {{ source, 0 of 1 }} (SACM-MULT-001)"
    );
    let message = error.to_string();
    assert!(
        message.contains("source"),
        "{row}: {what}: the message {message:?} does not name the end"
    );
    // No source comes before a missing target.
    let error = refuse(
        row,
        "no source and a missing target",
        &mut doc,
        kind,
        operation(kind, &spec(&[], "nope")),
    );
    assert!(
        matches!(&error, EditError::TooFewReferences { .. }),
        "{row}: no source and a missing target is not TooFewReferences: {error:?}"
    );
    // A missing source, target or reasoning.
    let error = refuse(
        row,
        "a missing source",
        &mut doc,
        kind,
        operation(kind, &spec(&["c2", "nope"], "c1")),
    );
    assert_not_found(row, "a missing source", &error, "source", "nope");
    let error = refuse(
        row,
        "a missing target",
        &mut doc,
        kind,
        operation(kind, &spec(&["c2"], "nope")),
    );
    assert_not_found(row, "a missing target", &error, "target", "nope");
    let mut missing_reasoning = spec(&["c2"], "c1");
    missing_reasoning.reasoning = Some("nope");
    let error = refuse(
        row,
        "a missing reasoning",
        &mut doc,
        kind,
        operation(kind, &missing_reasoning),
    );
    assert_not_found(row, "a missing reasoning", &error, "reasoning", "nope");
    // Ids are compared exactly.
    let error = refuse(
        row,
        "a target differing in case",
        &mut doc,
        kind,
        operation(kind, &spec(&["c2"], "C1")),
    );
    assert_not_found(row, "a target differing in case", &error, "target", "C1");
}

fn the_create_operation_runs_the_four_create_checks_first(row: &str, kind: ElementKind) {
    let mut doc = built(row);
    // ADR 0019's four checks come first.
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
        matches!(&error, EditError::ParentNotFound { parent, .. } if parent.as_str() == "nowhere"),
        "{row}: a missing parent with no source is not ParentNotFound: {error:?}"
    );
    for (parent, parent_kind) in [
        ("case", ElementKind::AssuranceCasePackage),
        ("c1", ElementKind::Claim),
        ("r1", ElementKind::ArgumentReasoning),
    ] {
        let mut wrong_parent = spec(&[], "nope");
        wrong_parent.parent = parent;
        let what = format!("under the {parent_kind} {parent:?}");
        let error = refuse(row, &what, &mut doc, kind, operation(kind, &wrong_parent));
        assert!(
            matches!(
                &error,
                EditError::InvalidParent { parent: p, parent_kind: k, kind: created, .. }
                    if p.as_str() == parent && *k == parent_kind && *created == kind
            ),
            "{row}: {what}: not InvalidParent: {error:?}"
        );
    }
    let mut invalid_id = spec(&[], "nope");
    invalid_id.id = Some("1x");
    let error = refuse(
        row,
        "an invalid id",
        &mut doc,
        kind,
        operation(kind, &invalid_id),
    );
    assert!(
        matches!(&error, EditError::InvalidId { id, .. } if id.as_str() == "1x"),
        "{row}: an invalid id with no source is not InvalidId: {error:?}"
    );
    let mut duplicate_id = spec(&["c2"], "nope");
    duplicate_id.id = Some("c1");
    let error = refuse(
        row,
        "a duplicate id",
        &mut doc,
        kind,
        operation(kind, &duplicate_id),
    );
    assert!(
        matches!(&error, EditError::DuplicateId { id, .. } if id.as_str() == "c1"),
        "{row}: a duplicate id with a missing target is not DuplicateId: {error:?}"
    );
}

#[test]
fn sacm23_arg_008_create_asserted_inference_refuses_no_source_and_missing_references_unchanged() {
    the_create_operation_refuses_unchanged(ARG_008, ElementKind::AssertedInference);
}

#[test]
fn sacm23_arg_010_create_asserted_context_refuses_no_source_and_missing_references_unchanged() {
    the_create_operation_refuses_unchanged(ARG_010, ElementKind::AssertedContext);
}

#[test]
fn sacm23_arg_008_create_asserted_inference_runs_the_four_create_checks_before_the_reference_checks()
 {
    the_create_operation_runs_the_four_create_checks_first(ARG_008, ElementKind::AssertedInference);
}

#[test]
fn sacm23_arg_010_create_asserted_context_runs_the_four_create_checks_before_the_reference_checks()
{
    the_create_operation_runs_the_four_create_checks_first(ARG_010, ElementKind::AssertedContext);
}

fn a_reference_to_an_id_that_names_no_element_is_refused(row: &str, kind: ElementKind) {
    // The xmi:id of a name, read from a file, identifies no element.
    let mut doc = read_ok(
        row,
        "a Claim whose name has an id",
        &format!(
            "<sacm:ArgumentPackage{NS} xmi:id=\"p\"><name content=\"P\"/>\
             <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\">\
             <name xmi:id=\"c1_name\" content=\"C\"/></argumentElement>{}</sacm:ArgumentPackage>",
            claim("c2")
        ),
    );
    for (end, sources, target) in [
        ("source", vec!["c1_name"], "c2"),
        ("target", vec!["c2"], "c1_name"),
    ] {
        let what = format!("a {end} naming a name's id");
        let before = doc.clone();
        let error = match edit::apply(&mut doc, operation(kind, &spec(&sources, target))) {
            Ok(mutation) => panic!("{row}: {what}: accepted: {:?}", mutation.created_ids()),
            Err(error) => error,
        };
        assert_not_found(row, &what, &error, end, "c1_name");
        assert!(
            doc == before,
            "{row}: {what}: the refusal changed the document"
        );
    }
}

#[test]
fn sacm23_arg_008_create_asserted_inference_refuses_an_end_naming_the_id_of_a_name() {
    a_reference_to_an_id_that_names_no_element_is_refused(ARG_008, ElementKind::AssertedInference);
}

#[test]
fn sacm23_arg_010_create_asserted_context_refuses_an_end_naming_the_id_of_a_name() {
    a_reference_to_an_id_that_names_no_element_is_refused(ARG_010, ElementKind::AssertedContext);
}

// --- Item 4: any ArgumentAsset at either end --------------------------------------------------------

fn any_argument_asset_at_either_end_gives_no_finding(row: &str, kind: ElementKind) {
    let mut base = built(row);
    // Another relationship, of the other kind, to be a source and a target.
    let other = if kind == ElementKind::AssertedInference {
        ElementKind::AssertedContext
    } else {
        ElementKind::AssertedInference
    };
    let mut other_spec = spec(&["c2"], "c1");
    other_spec.id = Some("other");
    create(row, &mut base, operation(other, &other_spec));
    let assets = [
        ("a Claim", "c3"),
        ("an ArgumentReasoning", "r1"),
        ("another relationship", "other"),
    ];
    for (source_what, source) in assets {
        for (target_what, target) in assets {
            let what = format!("{source_what} as the source and {target_what} as the target");
            let mut doc = base.clone();
            let mut given = spec(&[source], target);
            given.id = Some("x");
            create(row, &mut doc, operation(kind, &given));
            assert_eq!(
                ends(row, &doc, "x"),
                expected_ends(&[source], Some(target), None, false),
                "{row}: {what}: the ends"
            );
            assert_eq!(
                validation::validate(&doc),
                [],
                "{row}: {what}: validate reports a finding"
            );
        }
    }
    // The same, read from a file.
    let what = format!("an {kind} read with every ArgumentAsset kind at its ends");
    let doc = read_ok(
        row,
        &what,
        &in_ap(&format!(
            "{}{}",
            relationship(other, "other", " source=\"c2\" target=\"c1\"", ""),
            relationship(kind, "x", " source=\"c3 r1 other\" target=\"other\"", "")
        )),
    );
    assert_eq!(
        validation::validate(&doc),
        [],
        "{row}: {what}: validate reports a finding"
    );
}

#[test]
fn sacm23_arg_008_any_argument_asset_as_source_and_target_gives_no_finding() {
    any_argument_asset_at_either_end_gives_no_finding(ARG_008, ElementKind::AssertedInference);
}

#[test]
fn sacm23_arg_010_any_argument_asset_as_source_and_target_gives_no_finding() {
    any_argument_asset_at_either_end_gives_no_finding(ARG_010, ElementKind::AssertedContext);
}

// --- Item 5: a document built only by edits (LIB-RT-003) ---------------------------------------------

fn a_document_built_by_edits_round_trips(row: &str, kind: ElementKind) {
    let mut doc = built(row);
    let mut inference = spec(&["c2", "c3"], "c1");
    inference.id = Some("i1");
    inference.reasoning = Some("r1");
    create(
        row,
        &mut doc,
        operation(ElementKind::AssertedInference, &inference),
    );
    let mut context = spec(&["c3"], "i1");
    context.id = Some("x1");
    context.counter = true;
    create(
        row,
        &mut doc,
        operation(ElementKind::AssertedContext, &context),
    );
    // And one of the row's kind with a generated id.
    let generated = create(row, &mut doc, operation(kind, &spec(&["c1"], "c2")));
    assert_eq!(
        element(row, &doc, generated.as_str()).kind(),
        kind,
        "{row}: setup: the generated element"
    );
    assert_eq!(
        validation::validate(&doc),
        [],
        "{row}: the built document does not validate cleanly"
    );
    let written = match xmi::write(&doc, &WriteOptions::default()) {
        Ok(text) => text,
        Err(error) => panic!("{row}: a strict write refused the built document: {error:?}"),
    };
    let output = match xmi::read(written.as_bytes(), &ReadOptions::default()) {
        Ok(output) => output,
        Err(error) => panic!("{row}: the output does not read back: {error}\n{written}"),
    };
    assert_eq!(
        output.diagnostics(),
        [],
        "{row}: the output reads back with diagnostics\n{written}"
    );
    let reloaded = output.into_document();
    assert_eq!(
        validation::validate(&reloaded),
        [],
        "{row}: the reloaded document does not validate cleanly\n{written}"
    );
    assert_eq!(
        semantic_compare(&doc, &reloaded),
        [],
        "{row}: the reloaded document differs from the built one\n{written}"
    );
    assert!(
        doc == reloaded,
        "{row}: built == reloaded is false\n{written}"
    );
}

#[test]
fn sacm23_arg_008_a_document_built_only_by_edits_writes_strictly_reloads_and_compares_equal() {
    a_document_built_by_edits_round_trips(ARG_008, ElementKind::AssertedInference);
}

#[test]
fn sacm23_arg_010_a_document_built_only_by_edits_writes_strictly_reloads_and_compares_equal() {
    a_document_built_by_edits_round_trips(ARG_010, ElementKind::AssertedContext);
}
