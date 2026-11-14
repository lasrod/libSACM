//! Tests for `metaClaim`, the first multi-valued reference end
//! (SACM23-ARG-004; formal/23-05-08 §11.10 `metaClaim:Claim[0..*]`;
//! ptc/22-03-13 `Argumentation::Assertion`.metaClaim; XMI 2.5.1 rules 2b, 2h,
//! 2j, §7.8.5 `xsd:IDREFS`; D-ORDER; ADR 0026, ADR 0027).
//!
//! Every element of an Assertion kind (a Claim and the five asserted
//! relationships) holds `metaClaim` as ids in document order, repeats kept
//! (`SacmElement::meta_claims`), empty by default. The attribute keeps every
//! token; each `<metaClaim>` element appends one id; an attribute and
//! elements for the end is `RepeatedValue`. It is written as one attribute,
//! the ids separated by one space, after `assertionDeclaration`, and compared
//! in order (`Difference::MetaClaims`). A missing id is SACM-REF-001 with
//! clause `11.10`. On any other kind it is `Unsupported`.
//!
//! Two tests here kill the mutants that docs/quality/mutants-triage.md
//! excluded for this row:
//! - [`sacm23_arg_004_two_meta_claims_are_written_as_one_attribute_with_one_space_between_the_ids`]
//!   kills `replace > with <` in `push_flags_and_references` (the separator);
//! - [`sacm23_arg_004_two_missing_meta_claims_are_two_dangling_findings_and_a_repeated_one_is_one`]
//!   kills `replace == with !=` in `dangling_references` (the dedupe).
//!
//! No edit sets `metaClaim` yet (LIB-EDIT-009), so documents are read from
//! inline XMI. Expected locations are marked in the input with [`MARK`]
//! (U+2038), which the test removes before reading.

use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::Duration;

use sacm::compare::{Difference, semantic_compare};
use sacm::diagnostic::{Code, Diagnostic, Severity, SourceLocation};
use sacm::edit::{self, CreateClaim};
use sacm::model::{Document, ElementId, ElementKind, LangString, SacmElement};
use sacm::validation;
use sacm::xmi::{self, Mode, ReadError, ReadOptions, ReadOutput, WriteOptions};

const ROW: &str = "SACM23-ARG-004";

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

/// The six Assertion kinds (§11.10): a Claim and the five asserted
/// relationships.
const ASSERTION_KINDS: [ElementKind; 6] = [
    ElementKind::Claim,
    ElementKind::AssertedInference,
    ElementKind::AssertedEvidence,
    ElementKind::AssertedContext,
    ElementKind::AssertedArtifactSupport,
    ElementKind::AssertedArtifactContext,
];

// --- Inputs ------------------------------------------------------------------------

/// A named Claim `id`.
fn claim(id: &str) -> String {
    format!(
        "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"{id}\"><name content=\"C\"/>\
         </argumentElement>"
    )
}

/// An ArgumentPackage root `p` named "P", holding the named Claims c1, c2,
/// m1 and m2, then `body`. Each element is on its own line.
fn in_ap(body: &str) -> String {
    format!(
        "<sacm:ArgumentPackage{NS} xmi:id=\"p\">\n  <name content=\"P\"/>\n  {}\n  {}\n  {}\n  {}\n  \
         {body}\n</sacm:ArgumentPackage>",
        claim("c1"),
        claim("c2"),
        claim("m1"),
        claim("m2")
    )
}

/// One Assertion kind that this release reads, as the element `x` of
/// that kind in [`in_ap`]: what, the kind, and the element given its extra
/// start-tag attributes and the content after its name.
type Holder = (&'static str, ElementKind, fn(&str, &str) -> String);

/// The three readable Assertion kinds. Each relationship has the source `c1`
/// and the target `c2`, so that only `metaClaim` differs.
fn holders() -> [Holder; 3] {
    [
        ("a Claim", ElementKind::Claim, |attributes, content| {
            format!(
                "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"x\"{attributes}>\
                 <name content=\"X\"/>{content}</argumentElement>"
            )
        }),
        (
            "an AssertedInference",
            ElementKind::AssertedInference,
            |attributes, content| {
                format!(
                    "<argumentElement xmi:type=\"sacm:AssertedInference\" xmi:id=\"x\" \
                     source=\"c1\" target=\"c2\"{attributes}><name content=\"X\"/>{content}\
                     </argumentElement>"
                )
            },
        ),
        (
            "an AssertedContext",
            ElementKind::AssertedContext,
            |attributes, content| {
                format!(
                    "<argumentElement xmi:type=\"sacm:AssertedContext\" xmi:id=\"x\" \
                     source=\"c1\" target=\"c2\"{attributes}><name content=\"X\"/>{content}\
                     </argumentElement>"
                )
            },
        ),
    ]
}

/// The start tag `xmi::write` gives the element `x` of `kind` with
/// `metaClaim="<meta>"` and nothing else of its own beyond the holder's ends.
fn written_start_tag(kind: ElementKind, meta: &str) -> String {
    let ends = if kind == ElementKind::Claim {
        ""
    } else {
        " source=\"c1\" target=\"c2\""
    };
    format!("<argumentElement xmi:type=\"sacm:{kind}\" xmi:id=\"x\" metaClaim=\"{meta}\"{ends}>")
}

fn loc(line: u32, column: u32) -> SourceLocation {
    let Some(location) = SourceLocation::new(line, column) else {
        panic!("{ROW}: SourceLocation::new({line}, {column}) refused a 1-based position");
    };
    location
}

/// `template` without its marks, and the location of each mark in order:
/// lines end at LF, CRLF or a lone CR, and columns count characters.
fn marked(template: &str) -> (String, Vec<SourceLocation>) {
    let (mut line, mut column, mut after_cr) = (1_u32, 1_u32, false);
    let mut text = String::with_capacity(template.len());
    let mut marks = Vec::new();
    for c in template.chars() {
        if c == MARK {
            marks.push(loc(line, column));
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

/// Reads `input` in both modes, which the test expects to read with no
/// diagnostic to equal documents, and returns the strict one.
fn read_ok(what: &str, input: &str) -> Document {
    let mut documents = Vec::new();
    for mode in MODES {
        match read(mode, input) {
            Ok(output) => {
                assert!(
                    output.diagnostics().is_empty(),
                    "{ROW}: {what}: reading in {mode:?} mode reported {:?}",
                    output.diagnostics()
                );
                documents.push(output.into_document());
            }
            Err(error) => {
                panic!("{ROW}: {what}: a {mode:?} read refused {input:?}: {error:?} ({error})")
            }
        }
    }
    assert_eq!(
        documents[0], documents[1],
        "{ROW}: {what}: the modes read different documents"
    );
    documents.swap_remove(0)
}

/// Reads `input` in both modes, which the test expects to be refused the same
/// in both, and returns the refusal.
fn refused(what: &str, input: &str) -> ReadError {
    let mut errors = Vec::new();
    for mode in MODES {
        match read(mode, input) {
            Ok(output) => panic!(
                "{ROW}: {what}: a {mode:?} read returned a document of {} elements for {input:?}",
                output.document().element_count()
            ),
            Err(error) => errors.push(error),
        }
    }
    assert_eq!(
        errors[0], errors[1],
        "{ROW}: {what}: the modes refuse differently"
    );
    errors.swap_remove(0)
}

/// Writes `doc` tolerantly: some documents here hold a missing id on
/// purpose, and the save mode is not what is tested.
fn write(what: &str, doc: &Document) -> String {
    match xmi::write(doc, &WriteOptions::default().with_mode(Mode::Tolerant)) {
        Ok(text) => text,
        Err(error) => panic!("{ROW}: {what}: the write refused the document: {error:?}"),
    }
}

/// Writes `doc`, reads the output back and writes that again: the read-back
/// must be equal and the second write must give the same bytes. Returns the
/// first output.
fn round_trip(what: &str, doc: &Document) -> String {
    let text = write(what, doc);
    let back = read_ok(&format!("{what}, written"), &text);
    assert_eq!(
        semantic_compare(doc, &back),
        [],
        "{ROW}: {what}: the output does not read back equal\n{text}"
    );
    assert_eq!(&back, doc, "{ROW}: {what}: the output reads back !=");
    assert_eq!(
        write(what, &back),
        text,
        "{ROW}: {what}: writing what was read back gives other bytes"
    );
    text
}

/// The element `id` of `doc`, which must exist.
fn element<'d>(doc: &'d Document, id: &str) -> SacmElement<'d> {
    let Some(found) = doc.get(id) else {
        panic!("{ROW}: the document has no element {id:?}");
    };
    found
}

/// The meta-claims of the element `id`, as strings.
fn meta_claims(doc: &Document, id: &str) -> Vec<String> {
    element(doc, id)
        .meta_claims()
        .iter()
        .map(|target| target.as_str().to_owned())
        .collect()
}

fn ids(values: &[&str]) -> Vec<String> {
    values.iter().map(|&value| value.to_owned()).collect()
}

/// The line of `text` carrying `xmi:id="<id>"`, trimmed.
fn line_with<'t>(text: &'t str, id: &str) -> &'t str {
    let attribute = format!("xmi:id=\"{id}\"");
    let Some(line) = text.lines().find(|line| line.contains(&attribute)) else {
        panic!("{ROW}: the output has no line with {attribute}:\n{text}");
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
fn assert_unsupported_at(what: &str, error: &ReadError, at: SourceLocation) {
    assert!(
        matches!(error, ReadError::Unsupported { .. }) && error.code().is_none(),
        "{ROW}: {what}: not Unsupported: {error:?}"
    );
    assert_eq!(
        error.location(),
        Some(at),
        "{ROW}: {what}: not refused at the construct: {error}"
    );
}

/// Runs `job` on a worker thread and fails if it does not return within
/// `deadline` or panics.
fn within<T: Send + 'static>(
    what: &str,
    deadline: Duration,
    job: impl FnOnce() -> T + Send + 'static,
) -> T {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let _ = sender.send(job());
    });
    match receiver.recv_timeout(deadline) {
        Ok(result) => result,
        Err(RecvTimeoutError::Timeout) => {
            panic!("{ROW}: {what}: did not return within {deadline:?}")
        }
        Err(RecvTimeoutError::Disconnected) => panic!("{ROW}: {what}: panicked"),
    }
}

// --- Item 1: the default, and which kinds have the end --------------------------------

#[test]
fn sacm23_arg_004_every_kind_starts_with_no_meta_claims() {
    // Every kind, through the seam, under an ArtifactPackage root, which
    // admits every kind (ADR 0014).
    let mut doc = Document::new();
    let pkg = ElementId::from("pkg");
    if let Err(error) = doc.insert(None, ElementKind::ArtifactPackage, pkg.clone(), None) {
        panic!("{ROW}: setup: the root could not be inserted: {error:?}");
    }
    for &kind in ElementKind::ALL {
        let id = ElementId::from(format!("e_{kind}"));
        if let Err(error) = doc.insert(Some(&pkg), kind, id, None) {
            panic!("{ROW}: setup: a {kind} could not be inserted: {error:?}");
        }
    }
    for found in doc.elements() {
        assert!(
            found.meta_claims().is_empty(),
            "{ROW}: a new {} starts with meta-claims {:?}",
            found.kind(),
            found.meta_claims()
        );
    }
    // A Claim created by an edit, and one read without the end.
    if let Err(error) = edit::apply(
        &mut doc,
        CreateClaim::new("pkg", LangString::new("Made")).with_id("made"),
    ) {
        panic!("{ROW}: setup: CreateClaim was refused: {error}");
    }
    assert!(
        element(&doc, "made").meta_claims().is_empty(),
        "{ROW}: a created Claim starts with meta-claims"
    );
    let read = read_ok("a Claim without metaClaim", &in_ap(&claim("x")));
    assert!(
        read.elements().all(|found| found.meta_claims().is_empty()),
        "{ROW}: an element read without metaClaim has meta-claims"
    );
}

#[test]
fn sacm23_arg_004_the_end_is_read_on_exactly_the_assertion_kinds() {
    // The readable Assertion kinds read it.
    for (holder, kind, element_of) in holders() {
        assert!(
            ASSERTION_KINDS.contains(&kind),
            "{ROW}: setup: {kind} is no Assertion"
        );
        let what = format!("metaClaim=\"m1\" on {holder}");
        let doc = read_ok(&what, &in_ap(&element_of(" metaClaim=\"m1\"", "")));
        assert_eq!(
            meta_claims(&doc, "x"),
            ids(&["m1"]),
            "{ROW}: {what}: not read"
        );
    }
    // Every other readable kind refuses it at the attribute.
    let others: [(&str, String); 9] = [
        (
            "an AssuranceCasePackage root",
            format!(
                "<sacm:AssuranceCasePackage{NS} xmi:id=\"x\" {MARK}metaClaim=\"x\">\
                 <name content=\"X\"/></sacm:AssuranceCasePackage>"
            ),
        ),
        (
            "an ArgumentPackage root",
            format!(
                "<sacm:ArgumentPackage{NS} xmi:id=\"x\" {MARK}metaClaim=\"x\">\
                 <name content=\"X\"/></sacm:ArgumentPackage>"
            ),
        ),
        (
            "an ArgumentPackage in an AssuranceCasePackage",
            format!(
                "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\"><name content=\"Case\"/>\
                 <argumentPackage xmi:id=\"x\" {MARK}metaClaim=\"case\"><name content=\"X\"/>\
                 </argumentPackage></sacm:AssuranceCasePackage>"
            ),
        ),
        (
            "an ArgumentReasoning",
            in_ap(&format!(
                "<argumentElement xmi:type=\"sacm:ArgumentReasoning\" xmi:id=\"x\" \
                 {MARK}metaClaim=\"m1\"><name content=\"X\"/></argumentElement>"
            )),
        ),
        (
            "a Description",
            in_ap(&format!(
                "<description xmi:id=\"x\" {MARK}metaClaim=\"m1\"/>"
            )),
        ),
        (
            "an ImplementationConstraint",
            in_ap(&format!(
                "<implementationConstraint xmi:id=\"x\" {MARK}metaClaim=\"m1\"/>"
            )),
        ),
        (
            "a Note",
            in_ap(&format!("<note xmi:id=\"x\" {MARK}metaClaim=\"m1\"/>")),
        ),
        (
            "a TaggedValue",
            in_ap(&format!(
                "<taggedValue xmi:id=\"x\" {MARK}metaClaim=\"m1\"><key><value content=\"k\"/></key>\
                 </taggedValue>"
            )),
        ),
        (
            "a name",
            in_ap(&format!(
                "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"x\">\
                 <name content=\"X\" {MARK}metaClaim=\"m1\"/></argumentElement>"
            )),
        ),
    ];
    for (what, template) in &others {
        let what = format!("metaClaim on {what}");
        let (input, marks) = marked(template);
        let error = refused(&what, &input);
        assert_unsupported_at(&what, &error, marks[0]);
    }
}

// --- Item 2: the attribute, in order -----------------------------------------------------

#[test]
fn sacm23_arg_004_the_attribute_reads_every_token_in_document_order_and_round_trips() {
    for (holder, kind, element_of) in holders() {
        for (value, expected) in [
            ("m1", &["m1"][..]),
            ("m1 m2", &["m1", "m2"][..]),
            ("m2 m1", &["m2", "m1"][..]),
        ] {
            let what = format!("metaClaim={value:?} on {holder}");
            let doc = read_ok(
                &what,
                &in_ap(&element_of(&format!(" metaClaim=\"{value}\""), "")),
            );
            assert_eq!(
                meta_claims(&doc, "x"),
                ids(expected),
                "{ROW}: {what}: not read in document order"
            );
            assert_eq!(
                dangling(&doc),
                [],
                "{ROW}: {what}: a resolved meta-claim is reported as dangling"
            );
            let text = round_trip(&what, &doc);
            assert_eq!(
                line_with(&text, "x"),
                written_start_tag(kind, value),
                "{ROW}: {what}: not written as one attribute after assertionDeclaration\n{text}"
            );
        }
    }
}

/// Kills the mutant `replace > with <` in `push_flags_and_references`
/// (docs/quality/mutants-triage.md): with it, no space is written between the
/// ids, and `metaClaim="m1m2"` names one other id.
#[test]
fn sacm23_arg_004_two_meta_claims_are_written_as_one_attribute_with_one_space_between_the_ids() {
    for (holder, kind, element_of) in holders() {
        let what = format!("metaClaim=\"m1 m2\" on {holder}");
        let doc = read_ok(&what, &in_ap(&element_of(" metaClaim=\"m1   m2\"", "")));
        let text = write(&what, &doc);
        assert!(
            text.contains(" metaClaim=\"m1 m2\""),
            "{ROW}: {what}: the ids are not written separated by exactly one space\n{text}"
        );
        assert_eq!(
            line_with(&text, "x"),
            written_start_tag(kind, "m1 m2"),
            "{ROW}: {what}: the start tag is not exact\n{text}"
        );
        assert_eq!(
            text.matches("metaClaim").count(),
            1,
            "{ROW}: {what}: not written as one attribute\n{text}"
        );
    }
}

// --- Item 3: the element form accumulates -------------------------------------------------

#[test]
fn sacm23_arg_004_meta_claim_elements_append_in_order_around_other_content_and_write_as_the_attribute()
 {
    for (holder, kind, element_of) in holders() {
        let what =
            format!("<metaClaim xmi:idref=\"m1\"/>, a name, <metaClaim href=\"#m2\"/> on {holder}");
        // The holder writes the name first, so the first link goes before
        // the start of the content by an element of its own.
        let input = in_ap(&element_of("", "<metaClaim href=\"#m2\"/>").replacen(
            '>',
            "><metaClaim xmi:idref=\"m1\"/>",
            1,
        ));
        assert!(
            input.contains(
                "<metaClaim xmi:idref=\"m1\"/><name content=\"X\"/><metaClaim href=\"#m2\"/>"
            ),
            "{ROW}: setup: the input is not link, name, link: {input}"
        );
        let doc = read_ok(&what, &input);
        assert_eq!(
            meta_claims(&doc, "x"),
            ids(&["m1", "m2"]),
            "{ROW}: {what}: not read as [m1, m2]"
        );
        let attribute = read_ok(
            "the attribute form",
            &in_ap(&element_of(" metaClaim=\"m1 m2\"", "")),
        );
        assert_eq!(
            semantic_compare(&doc, &attribute),
            [],
            "{ROW}: {what}: does not read equal to metaClaim=\"m1 m2\""
        );
        let text = round_trip(&what, &doc);
        assert_eq!(
            line_with(&text, "x"),
            written_start_tag(kind, "m1 m2"),
            "{ROW}: {what}: not written in the attribute form\n{text}"
        );
        assert!(
            !text.contains("<metaClaim"),
            "{ROW}: {what}: an element form is written\n{text}"
        );
        // Three elements, one of them a repeat.
        let doc = read_ok(
            "three metaClaim elements",
            &in_ap(&element_of(
                "",
                "<metaClaim xmi:idref=\"m2\"/><metaClaim xmi:idref=\"m1\"/><metaClaim href=\"#m2\"/>",
            )),
        );
        assert_eq!(
            meta_claims(&doc, "x"),
            ids(&["m2", "m1", "m2"]),
            "{ROW}: three metaClaim elements on {holder} are not read in order with the repeat"
        );
    }
}

// --- Item 4: refusals and white space -------------------------------------------------------

#[test]
fn sacm23_arg_004_an_attribute_and_an_element_are_repeated_value_with_every_location() {
    for (holder, _, element_of) in holders() {
        for (what, attributes, content) in [
            (
                "metaClaim=\"m1\" with <metaClaim xmi:idref=\"m2\"/>",
                format!(" {MARK}metaClaim=\"m1\""),
                format!("{MARK}<metaClaim xmi:idref=\"m2\"/>"),
            ),
            (
                "metaClaim=\"m1\" with <metaClaim href=\"#m1\"/>",
                format!(" {MARK}metaClaim=\"m1\""),
                format!("{MARK}<metaClaim href=\"#m1\"/>"),
            ),
        ] {
            let what = format!("{what} on {holder}");
            let (input, marks) = marked(&in_ap(&element_of(&attributes, &content)));
            let error = refused(&what, &input);
            assert!(
                matches!(
                    &error,
                    ReadError::RepeatedValue { element, property, .. }
                        if element.as_str() == "x" && property == "metaClaim"
                ),
                "{ROW}: {what}: not RepeatedValue {{ x, metaClaim }}: {error:?}"
            );
            assert_eq!(
                (error.code(), error.locations(), error.location()),
                (
                    Some(Code::MULTIPLICITY_VIOLATION),
                    marks.as_slice(),
                    marks.get(1).copied()
                ),
                "{ROW}: {what}: not SACM-MULT-001 at the attribute and the element"
            );
        }
    }
}

#[test]
fn sacm23_arg_004_white_space_separates_tokens_and_an_empty_or_qualified_attribute_is_unsupported()
{
    for (holder, _, element_of) in holders() {
        let what = format!("metaClaim=\" m1&#9;m2&#10;\" on {holder}");
        let doc = read_ok(
            &what,
            &in_ap(&element_of(" metaClaim=\" m1&#9;m2&#10;\"", "")),
        );
        assert_eq!(
            meta_claims(&doc, "x"),
            ids(&["m1", "m2"]),
            "{ROW}: {what}: not read as [m1, m2]"
        );
        for (form, attribute) in [
            ("metaClaim=\"\"", format!(" {MARK}metaClaim=\"\"")),
            ("metaClaim=\"  \"", format!(" {MARK}metaClaim=\"  \"")),
            ("sacm:metaClaim", format!(" {MARK}sacm:metaClaim=\"m1\"")),
        ] {
            let what = format!("{form} on {holder}");
            let (input, marks) = marked(&in_ap(&element_of(&attribute, "")));
            let error = refused(&what, &input);
            assert_unsupported_at(&what, &error, marks[0]);
        }
        let what = format!("<sacm:metaClaim> on {holder}");
        let (input, marks) = marked(&in_ap(&element_of(
            "",
            &format!("{MARK}<sacm:metaClaim xmi:idref=\"m1\"/>"),
        )));
        let error = refused(&what, &input);
        assert_unsupported_at(&what, &error, marks[0]);
    }
}

// --- Item 5: a repeated id is kept -------------------------------------------------------------

#[test]
fn sacm23_arg_004_a_repeated_id_is_held_twice_and_written_back_unchanged() {
    for (holder, kind, element_of) in holders() {
        let what = format!("metaClaim=\"m1 m1\" on {holder}");
        let doc = read_ok(&what, &in_ap(&element_of(" metaClaim=\"m1 m1\"", "")));
        assert_eq!(
            meta_claims(&doc, "x"),
            ids(&["m1", "m1"]),
            "{ROW}: {what}: the repeat is not held"
        );
        let text = round_trip(&what, &doc);
        assert_eq!(
            line_with(&text, "x"),
            written_start_tag(kind, "m1 m1"),
            "{ROW}: {what}: not written back unchanged\n{text}"
        );
    }
}

// --- Item 6: dangling ids ------------------------------------------------------------------------

/// Kills the mutant `replace == with !=` in `dangling_references`
/// (docs/quality/mutants-triage.md): with it, `"x y"` gives one finding and
/// `"x x"` gives two.
#[test]
fn sacm23_arg_004_two_missing_meta_claims_are_two_dangling_findings_and_a_repeated_one_is_one() {
    for (holder, kind, element_of) in holders() {
        let what = format!("metaClaim=\"x9 y9\" on {holder}");
        let doc = read_ok(&what, &in_ap(&element_of(" metaClaim=\"x9 y9\"", "")));
        let found = dangling(&doc);
        assert_eq!(
            found.len(),
            2,
            "{ROW}: {what}: not two SACM-REF-001: {found:?}"
        );
        let affected: Vec<Vec<&str>> = found
            .iter()
            .map(|finding| finding.affected().iter().map(ElementId::as_str).collect())
            .collect();
        assert_eq!(
            affected,
            [["x", "x9"], ["x", "y9"]],
            "{ROW}: {what}: not [x, x9] then [x, y9]"
        );
        for finding in &found {
            assert_eq!(
                (
                    finding.severity(),
                    finding.requirement_id(),
                    finding.clause(),
                    finding.location()
                ),
                (Severity::Error, "SACM23-VAL-003", Some("11.10"), None),
                "{ROW}: {what}: not an Error of SACM23-VAL-003 with clause 11.10: {finding:?}"
            );
            assert!(
                finding.message().contains("metaClaim") && finding.message().contains(kind.name()),
                "{ROW}: {what}: the message does not name the end and the kind: {finding:?}"
            );
        }

        let what = format!("metaClaim=\"x9 x9\" on {holder}");
        let doc = read_ok(&what, &in_ap(&element_of(" metaClaim=\"x9 x9\"", "")));
        let found = dangling(&doc);
        let affected: Vec<Vec<&str>> = found
            .iter()
            .map(|finding| finding.affected().iter().map(ElementId::as_str).collect())
            .collect();
        assert_eq!(
            affected,
            [["x", "x9"]],
            "{ROW}: {what}: not exactly one SACM-REF-001 for the repeated id: {found:?}"
        );
        // A missing id between two resolved ones, and one resolved id
        // repeated: one finding.
        let what = format!("metaClaim=\"m1 x9 m1 x9 m2\" on {holder}");
        let doc = read_ok(
            &what,
            &in_ap(&element_of(" metaClaim=\"m1 x9 m1 x9 m2\"", "")),
        );
        assert_eq!(
            dangling(&doc).len(),
            1,
            "{ROW}: {what}: not one SACM-REF-001"
        );
    }
}

// --- Item 7: other kinds --------------------------------------------------------------------------

#[test]
fn sacm23_arg_004_meta_claim_on_a_package_a_reasoning_a_note_or_a_name_is_unsupported_at_that_construct()
 {
    let cases = [
        (
            "the attribute on an ArgumentPackage root",
            format!(
                "<sacm:ArgumentPackage{NS} xmi:id=\"p\" {MARK}metaClaim=\"p\"><name content=\"P\"/>\
                 </sacm:ArgumentPackage>"
            ),
        ),
        (
            "an element in an ArgumentPackage root",
            format!(
                "<sacm:ArgumentPackage{NS} xmi:id=\"p\"><name content=\"P\"/>\
                 {MARK}<metaClaim xmi:idref=\"p\"/></sacm:ArgumentPackage>"
            ),
        ),
        (
            "the attribute on an ArgumentReasoning",
            in_ap(&format!(
                "<argumentElement xmi:type=\"sacm:ArgumentReasoning\" xmi:id=\"r\" \
                 {MARK}metaClaim=\"m1\"><name content=\"R\"/></argumentElement>"
            )),
        ),
        (
            "an element in an ArgumentReasoning",
            in_ap(&format!(
                "<argumentElement xmi:type=\"sacm:ArgumentReasoning\" xmi:id=\"r\">\
                 <name content=\"R\"/>{MARK}<metaClaim href=\"#m1\"/></argumentElement>"
            )),
        ),
        (
            "the attribute on a Note",
            in_ap(&format!(
                "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"x\"><name content=\"X\"/>\
                 <note xmi:id=\"n\" {MARK}metaClaim=\"m1\"/></argumentElement>"
            )),
        ),
        (
            "an element in a Note",
            in_ap(&format!(
                "<note xmi:id=\"n\">{MARK}<metaClaim xmi:idref=\"m1\"/></note>"
            )),
        ),
        (
            "the attribute on a name",
            in_ap(&format!(
                "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"x\">\
                 <name content=\"X\" {MARK}metaClaim=\"m1\"/></argumentElement>"
            )),
        ),
        (
            "an element in a name",
            in_ap(&format!(
                "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"x\">\
                 <name content=\"X\">{MARK}<metaClaim xmi:idref=\"m1\"/></name></argumentElement>"
            )),
        ),
    ];
    for (what, template) in &cases {
        let what = format!("metaClaim: {what}");
        let (input, marks) = marked(template);
        assert_eq!(marks.len(), 1, "{ROW}: setup: {what} has not one mark");
        let error = refused(&what, &input);
        assert_unsupported_at(&what, &error, marks[0]);
    }
}

// --- Item 8: where a meta-claim may point ------------------------------------------------------------

#[test]
fn sacm23_arg_004_self_forward_and_cross_root_meta_claims_resolve_and_a_package_target_round_trips()
{
    let cases = [
        (
            "a self-reference",
            in_ap(
                "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"x\" metaClaim=\"x\">\
                 <name content=\"X\"/></argumentElement>",
            ),
            "x",
            vec!["x"],
        ),
        (
            "a forward reference",
            format!(
                "<sacm:ArgumentPackage{NS} xmi:id=\"p\"><name content=\"P\"/>\
                 <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"x\" metaClaim=\"later\">\
                 <name content=\"X\"/></argumentElement>{}</sacm:ArgumentPackage>",
                claim("later")
            ),
            "x",
            vec!["later"],
        ),
        (
            "a reference into a later root",
            format!(
                "<xmi:XMI{NS}>\
                 <sacm:ArgumentPackage xmi:id=\"r1\"><name content=\"R1\"/>\
                 <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"x\" metaClaim=\"m9 x\">\
                 <name content=\"X\"/></argumentElement></sacm:ArgumentPackage>\
                 <sacm:ArgumentPackage xmi:id=\"r2\"><name content=\"R2\"/>{}</sacm:ArgumentPackage>\
                 </xmi:XMI>",
                claim("m9")
            ),
            "x",
            vec!["m9", "x"],
        ),
        (
            "a reference into an earlier root",
            format!(
                "<xmi:XMI{NS}>\
                 <sacm:ArgumentPackage xmi:id=\"r1\"><name content=\"R1\"/>{}</sacm:ArgumentPackage>\
                 <sacm:ArgumentPackage xmi:id=\"r2\"><name content=\"R2\"/>\
                 <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"x\" metaClaim=\"m9\">\
                 <name content=\"X\"/></argumentElement></sacm:ArgumentPackage>\
                 </xmi:XMI>",
                claim("m9")
            ),
            "x",
            vec!["m9"],
        ),
    ];
    for (what, input, owner, expected) in &cases {
        let doc = read_ok(what, input);
        assert_eq!(
            meta_claims(&doc, owner),
            ids(expected),
            "{ROW}: {what}: not read"
        );
        assert_eq!(
            validation::validate(&doc),
            [],
            "{ROW}: {what}: validate reports a finding"
        );
        round_trip(what, &doc);
    }
    // A target of another kind than Claim is held: its typing is
    // SACM23-VAL-004's. Only SACM-REF-001 is asserted absent here.
    for (holder, kind, element_of) in holders() {
        let what = format!("metaClaim=\"p\", naming the package, on {holder}");
        let doc = read_ok(&what, &in_ap(&element_of(" metaClaim=\"p\"", "")));
        assert_eq!(
            (meta_claims(&doc, "x"), dangling(&doc)),
            (ids(&["p"]), Vec::new()),
            "{ROW}: {what}: not held, or reported as dangling"
        );
        let text = round_trip(&what, &doc);
        assert_eq!(
            line_with(&text, "x"),
            written_start_tag(kind, "p"),
            "{ROW}: {what}: not written back unchanged\n{text}"
        );
    }
}

// --- Item 9: comparison ---------------------------------------------------------------------------------

#[test]
fn sacm23_arg_004_semantic_compare_reports_meta_claims_compared_in_order_with_repeats() {
    for (holder, _, element_of) in holders() {
        for (left, right) in [("m1 m2", "m2 m1"), ("m1", "m1 m1"), ("m1", "")] {
            let what = format!("metaClaim={left:?} vs {right:?} on {holder}");
            let attribute = |value: &str| {
                if value.is_empty() {
                    String::new()
                } else {
                    format!(" metaClaim=\"{value}\"")
                }
            };
            let left_doc = read_ok(&what, &in_ap(&element_of(&attribute(left), "")));
            let right_doc = read_ok(&what, &in_ap(&element_of(&attribute(right), "")));
            let as_ids = |value: &str| -> Vec<ElementId> {
                value.split_whitespace().map(ElementId::from).collect()
            };
            assert_eq!(
                semantic_compare(&left_doc, &right_doc),
                [Difference::MetaClaims {
                    id: ElementId::from("x"),
                    left: as_ids(left),
                    right: as_ids(right),
                }],
                "{ROW}: {what}: not one MetaClaims difference"
            );
            assert_eq!(
                semantic_compare(&left_doc, &left_doc),
                [],
                "{ROW}: {what}: a document differs from itself"
            );
        }
    }
}

// --- Item 10: generated ids and large values -------------------------------------------------------------

#[test]
fn sacm23_arg_004_an_id_less_claim_beside_meta_claim_claim_1_gets_another_id() {
    let idless = "<argumentElement xmi:type=\"sacm:Claim\"><name content=\"C\"/></argumentElement>";
    for (what, body) in [
        (
            "the attribute before the id-less Claim",
            format!(
                "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"x\" metaClaim=\"m1 claim_1\">\
                 <name content=\"X\"/></argumentElement>{idless}"
            ),
        ),
        (
            "an element after the id-less Claim",
            format!(
                "{idless}<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"x\">\
                 <name content=\"X\"/><metaClaim xmi:idref=\"m2\"/>\
                 <metaClaim href=\"#claim_1\"/></argumentElement>"
            ),
        ),
    ] {
        let mut outputs = Vec::new();
        for mode in MODES {
            match read(mode, &in_ap(&body)) {
                Ok(output) => outputs.push(output.into_document()),
                Err(error) => panic!("{ROW}: {what}: a {mode:?} read refused: {error:?}"),
            }
        }
        let doc = &outputs[0];
        assert!(
            !doc.contains("claim_1"),
            "{ROW}: {what}: the id-less Claim got the referenced id claim_1"
        );
        let found = dangling(doc);
        let affected: Vec<Vec<&str>> = found
            .iter()
            .map(|finding| finding.affected().iter().map(ElementId::as_str).collect())
            .collect();
        assert_eq!(
            affected,
            [["x", "claim_1"]],
            "{ROW}: {what}: the reference to claim_1 does not dangle"
        );
    }
    // An edit's generated id skips it too.
    let mut doc = read_ok(
        "metaClaim=\"claim_1\" then CreateClaim",
        &in_ap(
            "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"x\" metaClaim=\"claim_1\">\
             <name content=\"X\"/></argumentElement>",
        ),
    );
    let created = match edit::apply(&mut doc, CreateClaim::new("p", LangString::new("New"))) {
        Ok(mutation) => mutation.created_ids().to_vec(),
        Err(error) => panic!("{ROW}: CreateClaim was refused: {error:?}"),
    };
    assert!(
        created.len() == 1 && created[0].as_str() != "claim_1",
        "{ROW}: CreateClaim generated the referenced id: {created:?}"
    );
}

#[test]
fn sacm23_arg_004_a_one_mib_meta_claim_and_ten_thousand_tokens_do_not_panic() {
    const MIB: usize = 1 << 20;
    const TOKENS: usize = 10_000;
    let deadline = Duration::from_secs(30);

    // One token of 1 MiB: held whole, and dangling.
    let long = "a".repeat(MIB);
    let input = in_ap(&format!(
        "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"x\" metaClaim=\"{long}\">\
         <name content=\"X\"/></argumentElement>"
    ));
    let what = "a 1 MiB metaClaim";
    let held = within(what, deadline, move || {
        let doc = match xmi::read(input.as_bytes(), &ReadOptions::default()) {
            Ok(output) => output.into_document(),
            Err(error) => return Err(format!("refused: {error:?}")),
        };
        let lengths: Vec<usize> = doc.get("x").map_or_else(Vec::new, |x| {
            x.meta_claims().iter().map(|id| id.as_str().len()).collect()
        });
        let findings = validation::validate(&doc).len();
        let written = xmi::write(&doc, &WriteOptions::default().with_mode(Mode::Tolerant))
            .map(|text| text.len())
            .map_err(|error| format!("{error:?}"))?;
        Ok((lengths, findings, written > MIB))
    });
    assert_eq!(
        held,
        Ok((vec![MIB], 1, true)),
        "{ROW}: {what}: not held whole, reported once and written"
    );

    // 10,000 tokens of one resolved id, and 10,000 distinct missing ids.
    for (what, value, findings) in [
        ("10,000 repeats of m1", "m1 ".repeat(TOKENS), 0),
        (
            "10,000 distinct missing ids",
            (0..TOKENS).fold(String::new(), |mut tokens, n| {
                tokens.push('t');
                tokens.push_str(&n.to_string());
                tokens.push(' ');
                tokens
            }),
            TOKENS,
        ),
    ] {
        let input = in_ap(&format!(
            "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"x\" metaClaim=\"{value}\">\
             <name content=\"X\"/></argumentElement>"
        ));
        let result = within(what, deadline, move || {
            let doc = match xmi::read(input.as_bytes(), &ReadOptions::default()) {
                Ok(output) => output.into_document(),
                Err(error) => return Err(format!("refused: {error:?}")),
            };
            let held = doc.get("x").map_or(0, |x| x.meta_claims().len());
            let dangling = validation::validate(&doc)
                .iter()
                .filter(|finding| finding.code() == Code::REF_DANGLING)
                .count();
            xmi::write(&doc, &WriteOptions::default().with_mode(Mode::Tolerant))
                .map_err(|error| format!("{error:?}"))?;
            Ok((held, dangling))
        });
        assert_eq!(
            result,
            Ok((TOKENS, findings)),
            "{ROW}: {what}: not every token held, or the findings are wrong"
        );
    }
}
