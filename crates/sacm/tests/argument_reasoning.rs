//! Tests for `ArgumentReasoning` and its `structure` (SACM23-ARG-005;
//! formal/23-05-08 §11.12 "Superclass `ArgumentAsset`",
//! `structure:ArgumentPackage[0..1]`; ptc/22-03-13
//! `Argumentation::ArgumentReasoning`; ADR 0019, ADR 0026, ADR 0027).
//!
//! An `ArgumentReasoning` is read under `argumentElement` of an ArgumentPackage,
//! written and round-tripped. `structure` is a single-valued reference end
//! (`SacmElement::structure`), written after `abstractForm`. It has no
//! `assertionDeclaration`, `metaClaim`, `isCounter`, `source`, `target` or
//! `reasoning`. `CreateArgumentReasoning::new(parent, name)` creates one under
//! the LIB-EDIT-001 contract; `with_structure` naming no element is
//! `EditError::ReferenceNotFound` (SACM-CMD-001).
//!
//! Expected locations are marked in the input with [`MARK`] (U+2038), which
//! the test removes before reading.

use std::fmt::Display;

use sacm::compare::{Difference, semantic_compare};
use sacm::diagnostic::{Code, Diagnostic, Severity, SourceLocation};
use sacm::edit::{
    self, CreateArgumentPackage, CreateArgumentReasoning, CreateAssuranceCasePackage, CreateClaim,
    EditError, Operation,
};
use sacm::model::{Document, ElementId, ElementKind, LangString, SacmElement};
use sacm::validation;
use sacm::xmi::{self, Mode, ReadError, ReadOptions, ReadOutput, WriteOptions};

const ROW: &str = "SACM23-ARG-005";

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

/// The valid fixture of the slice.
const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/sacm23/xmi/asserted-relationships-valid.sacm.xmi"
);

// --- Inputs ------------------------------------------------------------------------

/// A named Claim `id`.
fn claim(id: &str) -> String {
    format!(
        "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"{id}\"><name content=\"C\"/>\
         </argumentElement>"
    )
}

/// An `ArgumentReasoning` `id` named "R" with `attributes`, holding `content`
/// after its name.
fn reasoning(id: &str, attributes: &str, content: &str) -> String {
    format!(
        "<argumentElement xmi:type=\"sacm:ArgumentReasoning\" xmi:id=\"{id}\"{attributes}>\
         <name content=\"R\"/>{content}</argumentElement>"
    )
}

/// An AssuranceCasePackage root `case` holding the ArgumentPackage `p`, which
/// holds the Claim `c1` and then `body`, and after it the empty
/// ArgumentPackage `q`. Every element is named.
fn in_case(body: &str) -> String {
    format!(
        "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\">\n  <name content=\"Case\"/>\n  \
         <argumentPackage xmi:id=\"p\">\n    <name content=\"P\"/>\n    {}\n    {body}\n  \
         </argumentPackage>\n  <argumentPackage xmi:id=\"q\"><name content=\"Q\"/></argumentPackage>\n\
         </sacm:AssuranceCasePackage>",
        claim("c1")
    )
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

/// Reads `input` in both modes, which the test expects to read to equal
/// documents with no diagnostic but SACM-XMI-010 (an `xsi:type`), and
/// returns the strict one.
fn read_typed(what: &str, input: &str) -> Document {
    let mut documents = Vec::new();
    for mode in MODES {
        match read(mode, input) {
            Ok(output) => {
                assert!(
                    output
                        .diagnostics()
                        .iter()
                        .all(|found| found.code() == Code::XMI_XSI_TYPE),
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

/// Reads `input` in both modes, which the test expects to read with no
/// diagnostic to equal documents, and returns the strict one.
fn read_ok(what: &str, input: &str) -> Document {
    for mode in MODES {
        if let Ok(output) = read(mode, input) {
            assert!(
                output.diagnostics().is_empty(),
                "{ROW}: {what}: reading in {mode:?} mode reported {:?}",
                output.diagnostics()
            );
        }
    }
    read_typed(what, input)
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

fn write_in(what: &str, doc: &Document, mode: Mode) -> String {
    match xmi::write(doc, &WriteOptions::default().with_mode(mode)) {
        Ok(text) => text,
        Err(error) => panic!("{ROW}: {what}: a {mode:?} write refused the document: {error:?}"),
    }
}

/// Writes `doc` in `mode`, reads the output back and writes that again: the
/// read-back must be equal and the second write must give the same bytes.
/// Returns the first output.
fn round_trip(what: &str, doc: &Document, mode: Mode) -> String {
    let text = write_in(what, doc, mode);
    let back = read_ok(&format!("{what}, written"), &text);
    assert_eq!(
        semantic_compare(doc, &back),
        [],
        "{ROW}: {what}: the output does not read back equal\n{text}"
    );
    assert_eq!(&back, doc, "{ROW}: {what}: the output reads back !=");
    assert_eq!(
        write_in(what, &back, mode),
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

/// The `structure` of the element `id`, as a string.
fn structure(doc: &Document, id: &str) -> Option<String> {
    element(doc, id)
        .structure()
        .map(|target| target.as_str().to_owned())
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

/// Asserts that `error` is `RepeatedValue` for `structure` on `id`, with
/// exactly `locations` and `location()` the second.
fn assert_repeated(what: &str, error: &ReadError, id: &str, locations: &[SourceLocation]) {
    assert!(
        matches!(
            error,
            ReadError::RepeatedValue { element, property, .. }
                if element.as_str() == id && property == "structure"
        ),
        "{ROW}: {what}: not RepeatedValue {{ {id}, structure }}: {error:?}"
    );
    assert_eq!(
        (error.code(), error.locations(), error.location()),
        (
            Some(Code::MULTIPLICITY_VIOLATION),
            locations,
            locations.get(1).copied()
        ),
        "{ROW}: {what}: not SACM-MULT-001 at every occurrence"
    );
}

// --- Editing -------------------------------------------------------------------------

fn named(content: &str) -> LangString {
    LangString::new(content).with_lang("en")
}

/// Applies an operation the test expects to succeed, and returns the one id
/// it created.
fn create(doc: &mut Document, operation: impl Into<Operation>) -> ElementId {
    match edit::apply(doc, operation) {
        Ok(mutation) => {
            let created = mutation.created_ids();
            let [only] = created else {
                panic!("{ROW}: created_ids() is {created:?}, not the one new element");
            };
            only.clone()
        }
        Err(error) => panic!("{ROW}: the operation was refused: {error} ({error:?})"),
    }
}

/// A document built by edits: the AssuranceCasePackage `case` holding the
/// ArgumentPackage `p`, with the Claim `c1`, and the ArgumentPackage `q`.
fn built() -> Document {
    let mut doc = Document::new();
    create(
        &mut doc,
        CreateAssuranceCasePackage::root(named("Case")).with_id("case"),
    );
    create(
        &mut doc,
        CreateArgumentPackage::new("case", named("P")).with_id("p"),
    );
    create(&mut doc, CreateClaim::new("p", named("C1")).with_id("c1"));
    create(
        &mut doc,
        CreateArgumentPackage::new("case", named("Q")).with_id("q"),
    );
    doc
}

/// The name of a reference end in an `EditError`, by its `Display`.
fn end_name(end: &dyn Display) -> String {
    end.to_string()
}

/// Applies `operation`, which the test expects to be refused leaving `doc`
/// `==` to before and with the same next generated id, and returns why.
fn refuse(doc: &mut Document, what: &str, operation: CreateArgumentReasoning) -> EditError {
    let before = doc.clone();
    let error = match edit::apply(doc, operation) {
        Ok(mutation) => panic!(
            "{ROW}: {what}: accepted, creating {:?}",
            mutation.created_ids()
        ),
        Err(error) => error,
    };
    assert!(
        *doc == before,
        "{ROW}: {what}: the refusal changed the document"
    );
    let mut probe = doc.clone();
    assert_eq!(
        create(
            &mut probe,
            CreateArgumentReasoning::new("p", named("Probe"))
        )
        .as_str(),
        "argument_reasoning_1",
        "{ROW}: {what}: the refusal moved the next generated id"
    );
    let message = error.to_string();
    assert!(
        !message.contains('\n') && message.starts_with(char::is_lowercase),
        "{ROW}: {what}: the message {message:?} is not one line starting in lower case"
    );
    error
}

// --- Item 1: reading -----------------------------------------------------------------------

#[test]
fn sacm23_arg_005_it_reads_in_both_modes_by_xmi_type_and_xsi_type_with_its_inherited_features() {
    for type_attribute in [
        " xmi:type=\"sacm:ArgumentReasoning\"",
        " xsi:type=\"sacm:ArgumentReasoning\"",
        " xmi:type=\"sacm:ArgumentReasoning\" xsi:type=\"sacm:ArgumentReasoning\"",
    ] {
        let what = format!("an ArgumentReasoning by{type_attribute}");
        let input = in_case(&format!(
            "<argumentElement{type_attribute} xmi:id=\"r1\" gid=\"g-r1\" isCitation=\"true\" \
             isAbstract=\"true\" citedElement=\"c1\">\
             <name lang=\"en\" content=\"Argument over each hazard\"/>\
             <description xmi:id=\"d1\"><content><value content=\"Why\"/></content></description>\
             <note xmi:id=\"n1\"/><implementationConstraint xmi:id=\"k1\"/>\
             <taggedValue xmi:id=\"t1\"><key><value content=\"k\"/></key></taggedValue>\
             </argumentElement>"
        ));
        let doc = read_typed(&what, &input);
        let r1 = element(&doc, "r1");
        assert_eq!(
            (
                r1.kind(),
                r1.parent().map(|parent| parent.id().as_str()),
                r1.name().and_then(LangString::content),
                r1.gid(),
                r1.is_citation(),
                r1.is_abstract(),
                r1.cited_element().map(ElementId::as_str),
            ),
            (
                ElementKind::ArgumentReasoning,
                Some("p"),
                Some("Argument over each hazard"),
                Some("g-r1"),
                true,
                true,
                Some("c1"),
            ),
            "{ROW}: {what}: not read with its features"
        );
        let children: Vec<&str> = r1.children().map(|child| child.id().as_str()).collect();
        assert_eq!(
            children,
            ["d1", "n1", "k1", "t1"],
            "{ROW}: {what}: the utility children are not read in order"
        );
        assert_eq!(
            (
                r1.structure(),
                r1.assertion_declaration(),
                r1.meta_claims().len(),
                r1.sources().len(),
                r1.target(),
                r1.reasoning(),
                r1.is_counter()
            ),
            (None, None, 0, 0, None, None, false),
            "{ROW}: {what}: an ArgumentReasoning has an end or a flag it lacks"
        );
        let text = round_trip(&what, &doc, Mode::Tolerant);
        assert!(
            line_with(&text, "r1")
                .starts_with("<argumentElement xmi:type=\"sacm:ArgumentReasoning\" xmi:id=\"r1\""),
            "{ROW}: {what}: not written by xmi:type under argumentElement\n{text}"
        );
    }
}

#[test]
fn sacm23_arg_005_the_fixture_reads_its_reasoning_and_structure() {
    let bytes = std::fs::read(FIXTURE)
        .unwrap_or_else(|error| panic!("{ROW}: cannot read {FIXTURE}: {error}"));
    let doc = read_ok("the fixture", &String::from_utf8_lossy(&bytes));
    let r1 = element(&doc, "r1");
    assert_eq!(
        (r1.kind(), structure(&doc, "r1")),
        (
            ElementKind::ArgumentReasoning,
            Some("hazard_arguments".to_owned())
        ),
        "{ROW}: the fixture's reasoning"
    );
    assert_eq!(
        validation::validate(&doc),
        [],
        "{ROW}: the fixture does not validate cleanly"
    );
    round_trip("the fixture", &doc, Mode::Strict);
}

// --- Item 2: structure, both forms --------------------------------------------------------

#[test]
fn sacm23_arg_005_structure_reads_in_both_forms_writes_after_abstract_form_and_round_trips() {
    let attribute = read_ok(
        "structure=\"q\"",
        &in_case(&reasoning("r1", " structure=\"q\"", "")),
    );
    assert_eq!(
        structure(&attribute, "r1"),
        Some("q".to_owned()),
        "{ROW}: structure=\"q\" is not read"
    );
    for link in [
        "<structure xmi:idref=\"q\"/>",
        "<structure href=\"#q\"/>",
        "<structure xmi:idref=\" q \"/>",
    ] {
        let doc = read_ok(link, &in_case(&reasoning("r1", "", link)));
        assert_eq!(
            (semantic_compare(&doc, &attribute), &doc),
            (Vec::new(), &attribute),
            "{ROW}: {link}: does not read equal to structure=\"q\""
        );
    }
    let text = round_trip("structure=\"q\"", &attribute, Mode::Strict);
    assert_eq!(
        line_with(&text, "r1"),
        "<argumentElement xmi:type=\"sacm:ArgumentReasoning\" xmi:id=\"r1\" structure=\"q\">",
        "{ROW}: structure is not written as the attribute\n{text}"
    );
    assert!(
        !text.contains("<structure"),
        "{ROW}: an element form is written\n{text}"
    );

    // After every SACMElement feature, in any input order.
    let what = "structure with every SACMElement feature";
    let doc = read_ok(
        what,
        &in_case(&format!(
            "{}{}",
            reasoning("r0", " isAbstract=\"true\"", ""),
            reasoning(
                "r1",
                " structure=\"q\" abstractForm=\"r0\" citedElement=\"c1\" isCitation=\"1\" \
                 gid=\"g\"",
                ""
            )
        )),
    );
    let text = round_trip(what, &doc, Mode::Tolerant);
    assert_eq!(
        line_with(&text, "r1"),
        "<argumentElement xmi:type=\"sacm:ArgumentReasoning\" xmi:id=\"r1\" gid=\"g\" \
         isCitation=\"true\" citedElement=\"c1\" abstractForm=\"r0\" structure=\"q\">",
        "{ROW}: {what}: structure is not written after abstractForm\n{text}"
    );

    // Absent: None, and nothing written.
    let doc = read_ok("no structure", &in_case(&reasoning("r1", "", "")));
    assert_eq!(
        structure(&doc, "r1"),
        None,
        "{ROW}: an absent structure is not None"
    );
    let text = round_trip("no structure", &doc, Mode::Strict);
    assert!(
        !text.contains("structure"),
        "{ROW}: an absent structure is written\n{text}"
    );
}

// --- Item 3: a second value -------------------------------------------------------------------

#[test]
fn sacm23_arg_005_a_second_structure_is_repeated_value_and_an_empty_one_is_unsupported() {
    for (what, attributes, content) in [
        (
            "structure=\"q p\"",
            format!(" {MARK}{MARK}structure=\"q p\""),
            String::new(),
        ),
        (
            "two structure elements",
            String::new(),
            format!("{MARK}<structure xmi:idref=\"q\"/>{MARK}<structure href=\"#p\"/>"),
        ),
        (
            "the attribute and an element",
            format!(" {MARK}structure=\"q\""),
            format!("{MARK}<structure xmi:idref=\"q\"/>"),
        ),
    ] {
        let (input, marks) = marked(&in_case(&reasoning("r1", &attributes, &content)));
        let error = refused(what, &input);
        assert_repeated(what, &error, "r1", &marks);
    }
    for (what, attribute) in [
        ("structure=\"\"", format!(" {MARK}structure=\"\"")),
        ("structure=\"  \"", format!(" {MARK}structure=\"  \"")),
        ("sacm:structure", format!(" {MARK}sacm:structure=\"q\"")),
    ] {
        let (input, marks) = marked(&in_case(&reasoning("r1", &attribute, "")));
        let error = refused(what, &input);
        assert_unsupported_at(what, &error, marks[0]);
    }
}

// --- Item 4: where structure may point ----------------------------------------------------------

#[test]
fn sacm23_arg_005_structure_naming_its_package_a_later_package_or_another_root_resolves() {
    let cases = [
        (
            "its own containing package",
            in_case(&reasoning("r1", " structure=\"p\"", "")),
            "p",
        ),
        (
            "a later package",
            in_case(&reasoning("r1", " structure=\"q\"", "")),
            "q",
        ),
        (
            "the root",
            in_case(&reasoning("r1", " structure=\"case\"", "")),
            "case",
        ),
        (
            "a package in a later root",
            format!(
                "<xmi:XMI{NS}>\
                 <sacm:ArgumentPackage xmi:id=\"a\"><name content=\"A\"/>{}</sacm:ArgumentPackage>\
                 <sacm:ArgumentPackage xmi:id=\"b\"><name content=\"B\"/></sacm:ArgumentPackage>\
                 </xmi:XMI>",
                reasoning("r1", " structure=\"b\"", "")
            ),
            "b",
        ),
        (
            "an earlier root",
            format!(
                "<xmi:XMI{NS}>\
                 <sacm:ArgumentPackage xmi:id=\"a\"><name content=\"A\"/></sacm:ArgumentPackage>\
                 <sacm:ArgumentPackage xmi:id=\"b\"><name content=\"B\"/>{}</sacm:ArgumentPackage>\
                 </xmi:XMI>",
                reasoning("r1", " structure=\"a\"", "")
            ),
            "a",
        ),
    ];
    for (what, input, target) in &cases {
        let what = format!("structure naming {what}");
        let doc = read_ok(&what, input);
        assert_eq!(
            structure(&doc, "r1"),
            Some((*target).to_owned()),
            "{ROW}: {what}: not read"
        );
        // The case root is an AssuranceCasePackage: its type is
        // SACM23-VAL-004's, so only SACM-REF-001 is asserted absent for it.
        if *target == "case" {
            assert_eq!(dangling(&doc), [], "{ROW}: {what}: reported as dangling");
        } else {
            assert_eq!(
                validation::validate(&doc),
                [],
                "{ROW}: {what}: validate reports a finding"
            );
        }
        round_trip(&what, &doc, Mode::Tolerant);
    }
}

#[test]
fn sacm23_arg_005_structure_naming_a_claim_is_held_and_a_dangling_one_is_ref_001_with_clause_11_12()
{
    let what = "structure naming the Claim c1";
    let doc = read_ok(what, &in_case(&reasoning("r1", " structure=\"c1\"", "")));
    assert_eq!(
        (structure(&doc, "r1"), dangling(&doc)),
        (Some("c1".to_owned()), Vec::new()),
        "{ROW}: {what}: not held, or reported as dangling"
    );
    let text = round_trip(what, &doc, Mode::Tolerant);
    assert!(
        line_with(&text, "r1").ends_with(" structure=\"c1\">"),
        "{ROW}: {what}: not written back\n{text}"
    );

    let what = "structure=\"nope\"";
    let doc = read_ok(what, &in_case(&reasoning("r1", " structure=\"nope\"", "")));
    let found = dangling(&doc);
    let [finding] = found.as_slice() else {
        panic!("{ROW}: {what}: not one SACM-REF-001: {found:?}");
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
            Some("11.12"),
            None,
            [ElementId::from("r1"), ElementId::from("nope")].as_slice()
        ),
        "{ROW}: {what}: not an Error of SACM23-VAL-003 with clause 11.12 on [r1, nope]"
    );
    assert!(
        finding.message().contains("structure") && finding.message().contains("ArgumentReasoning"),
        "{ROW}: {what}: the message does not name the end and the kind: {finding:?}"
    );
    let text = round_trip(what, &doc, Mode::Tolerant);
    assert!(
        line_with(&text, "r1").ends_with(" structure=\"nope\">"),
        "{ROW}: {what}: not written back\n{text}"
    );
}

// --- Item 5: what an ArgumentReasoning does not have ---------------------------------------------

#[test]
fn sacm23_arg_005_the_assertion_and_relationship_features_on_an_argument_reasoning_are_unsupported()
{
    let mut cases: Vec<(String, String, String)> = Vec::new();
    for (name, value) in [
        ("assertionDeclaration", "asserted"),
        ("isCounter", "true"),
        ("metaClaim", "c1"),
        ("source", "c1"),
        ("target", "c1"),
        ("reasoning", "r0"),
    ] {
        cases.push((
            format!("{name} as an attribute"),
            format!(" {MARK}{name}=\"{value}\""),
            String::new(),
        ));
    }
    for content in [
        "<isCounter>true</isCounter>",
        "<metaClaim xmi:idref=\"c1\"/>",
        "<source xmi:idref=\"c1\"/>",
        "<target href=\"#c1\"/>",
        "<reasoning xmi:idref=\"r0\"/>",
        "<assertionDeclaration>asserted</assertionDeclaration>",
    ] {
        cases.push((
            format!("{content} as an element"),
            String::new(),
            format!("{MARK}{content}"),
        ));
    }
    for (what, attributes, content) in &cases {
        let what = format!("{what} on an ArgumentReasoning");
        let (input, marks) = marked(&in_case(&format!(
            "{}{}",
            reasoning("r0", "", ""),
            reasoning("r1", attributes, content)
        )));
        let error = refused(&what, &input);
        assert_unsupported_at(&what, &error, marks[0]);
    }
}

#[test]
fn sacm23_arg_005_an_argument_reasoning_in_an_assurance_case_package_is_unsupported() {
    for (what, child) in [
        (
            "in argumentPackage",
            "<argumentPackage xmi:type=\"sacm:ArgumentReasoning\" xmi:id=\"r1\"/>",
        ),
        (
            "in argumentElement, a role the class lacks",
            "<argumentElement xmi:type=\"sacm:ArgumentReasoning\" xmi:id=\"r1\"/>",
        ),
        (
            "in argumentElement by xsi:type",
            "<argumentElement xsi:type=\"sacm:ArgumentReasoning\" xmi:id=\"r1\"/>",
        ),
    ] {
        let what = format!("an ArgumentReasoning {what} of an AssuranceCasePackage");
        let (input, marks) = marked(&format!(
            "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\">\n  <name content=\"Case\"/>\n  \
             {MARK}{child}\n</sacm:AssuranceCasePackage>"
        ));
        let error = refused(&what, &input);
        assert_unsupported_at(&what, &error, marks[0]);
    }
}

// --- Item 6: CreateArgumentReasoning ----------------------------------------------------------------

#[test]
fn sacm23_arg_005_create_argument_reasoning_creates_under_an_argument_package_with_a_generated_id()
{
    let mut doc = built();
    let operation = CreateArgumentReasoning::new("p", named("Over hazards"));
    assert_eq!(
        (
            operation.parent().as_str(),
            operation.id(),
            operation.name(),
            operation.structure()
        ),
        ("p", None, &named("Over hazards"), None),
        "{ROW}: the operation does not report what it was given"
    );
    assert_eq!(
        Operation::from(operation.clone()).operation_name(),
        "CreateArgumentReasoning",
        "{ROW}: operation_name()"
    );
    let id = create(&mut doc, operation);
    assert_eq!(
        id.as_str(),
        "argument_reasoning_1",
        "{ROW}: the generated id"
    );
    let r1 = element(&doc, "argument_reasoning_1");
    assert_eq!(
        (
            r1.kind(),
            r1.parent().map(|parent| parent.id().as_str()),
            r1.name(),
            r1.structure()
        ),
        (
            ElementKind::ArgumentReasoning,
            Some("p"),
            Some(&named("Over hazards")),
            None
        ),
        "{ROW}: the created ArgumentReasoning"
    );
    assert_eq!(
        doc.get("p").map(|p| p
            .children()
            .last()
            .map(|child| child.id().as_str().to_owned())),
        Some(Some("argument_reasoning_1".to_owned())),
        "{ROW}: not created as the last child"
    );

    // With an id and a structure.
    let operation = CreateArgumentReasoning::new("p", named("Detailed"))
        .with_id("r_detail")
        .with_structure("q");
    assert_eq!(
        (
            operation.id().map(ElementId::as_str),
            operation.structure().map(ElementId::as_str)
        ),
        (Some("r_detail"), Some("q")),
        "{ROW}: with_id and with_structure are not reported"
    );
    create(&mut doc, operation);
    assert_eq!(
        structure(&doc, "r_detail"),
        Some("q".to_owned()),
        "{ROW}: with_structure is not set"
    );
    // A structure naming its own package, and the next generated id.
    create(
        &mut doc,
        CreateArgumentReasoning::new("p", named("Own")).with_structure("p"),
    );
    assert_eq!(
        structure(&doc, "argument_reasoning_2"),
        Some("p".to_owned()),
        "{ROW}: the second generated id, with its structure"
    );
    assert_eq!(
        validation::validate(&doc),
        [],
        "{ROW}: the built document does not validate cleanly"
    );
    round_trip("the built document", &doc, Mode::Strict);
}

#[test]
fn sacm23_arg_005_create_argument_reasoning_refuses_a_wrong_parent_and_a_missing_structure_unchanged()
 {
    let mut doc = built();
    // Under an AssuranceCasePackage or a Claim.
    for (parent, parent_kind) in [
        ("case", ElementKind::AssuranceCasePackage),
        ("c1", ElementKind::Claim),
    ] {
        let what = format!("under the {parent_kind} {parent:?}");
        let error = refuse(
            &mut doc,
            &what,
            CreateArgumentReasoning::new(parent, named("R")),
        );
        assert!(
            matches!(
                &error,
                EditError::InvalidParent { parent: p, parent_kind: k, kind, .. }
                    if p.as_str() == parent && *k == parent_kind
                        && *kind == ElementKind::ArgumentReasoning
            ),
            "{ROW}: {what}: not InvalidParent: {error:?}"
        );
        assert_eq!(
            error.code(),
            Code::CMD_INVALID_PARENT,
            "{ROW}: {what}: code"
        );
    }
    // A structure naming no element.
    let what = "with_structure(\"nope\")";
    let error = refuse(
        &mut doc,
        what,
        CreateArgumentReasoning::new("p", named("R")).with_structure("nope"),
    );
    let EditError::ReferenceNotFound { end, id, .. } = &error else {
        panic!("{ROW}: {what}: not ReferenceNotFound: {error:?}");
    };
    assert_eq!(
        (end_name(end), id.as_str(), error.code()),
        ("structure".to_owned(), "nope", Code::CMD_TARGET_NOT_FOUND),
        "{ROW}: {what}: not ReferenceNotFound {{ structure, nope }} (SACM-CMD-001)"
    );
    let message = error.to_string();
    assert!(
        message.contains("\"nope\"") && message.contains("structure"),
        "{ROW}: {what}: the message {message:?} does not quote the id and name the end"
    );

    // The id of a name identifies no element.
    let mut read = read_ok(
        "a package whose name has an id",
        &format!(
            "<sacm:ArgumentPackage{NS} xmi:id=\"p\"><name xmi:id=\"p_name\" content=\"P\"/>\
             </sacm:ArgumentPackage>"
        ),
    );
    let before = read.clone();
    let answer = edit::apply(
        &mut read,
        CreateArgumentReasoning::new("p", named("R")).with_structure("p_name"),
    );
    assert!(
        matches!(&answer, Err(EditError::ReferenceNotFound { id, .. }) if id.as_str() == "p_name"),
        "{ROW}: with_structure naming a name's id is not ReferenceNotFound: {answer:?}"
    );
    assert!(read == before, "{ROW}: the refusal changed the document");
}

#[test]
fn sacm23_arg_005_create_argument_reasoning_runs_the_four_create_checks_before_the_structure_check()
{
    let mut doc = built();
    // ADR 0019's four checks come first.
    let error = refuse(
        &mut doc,
        "a missing parent and structure",
        CreateArgumentReasoning::new("nowhere", named("R")).with_structure("nope"),
    );
    assert!(
        matches!(&error, EditError::ParentNotFound { parent, .. } if parent.as_str() == "nowhere"),
        "{ROW}: a missing parent and structure is not ParentNotFound: {error:?}"
    );
    let error = refuse(
        &mut doc,
        "a wrong parent and a missing structure",
        CreateArgumentReasoning::new("case", named("R")).with_structure("nope"),
    );
    assert!(
        matches!(&error, EditError::InvalidParent { .. }),
        "{ROW}: a wrong parent and a missing structure is not InvalidParent: {error:?}"
    );
    let error = refuse(
        &mut doc,
        "an invalid id and a missing structure",
        CreateArgumentReasoning::new("p", named("R"))
            .with_id("1r")
            .with_structure("nope"),
    );
    assert!(
        matches!(&error, EditError::InvalidId { id, .. } if id.as_str() == "1r"),
        "{ROW}: an invalid id and a missing structure is not InvalidId: {error:?}"
    );
    let error = refuse(
        &mut doc,
        "a duplicate id and a missing structure",
        CreateArgumentReasoning::new("p", named("R"))
            .with_id("c1")
            .with_structure("nope"),
    );
    assert!(
        matches!(&error, EditError::DuplicateId { id, .. } if id.as_str() == "c1"),
        "{ROW}: a duplicate id and a missing structure is not DuplicateId: {error:?}"
    );
}

// --- Item 7: comparison -----------------------------------------------------------------------------

#[test]
fn sacm23_arg_005_semantic_compare_reports_structure() {
    let with = |attributes: &str| read_ok(attributes, &in_case(&reasoning("r1", attributes, "")));
    let id = || ElementId::from("r1");
    for (left, right, expected) in [
        (
            " structure=\"q\"",
            "",
            Difference::Structure {
                id: id(),
                left: Some(ElementId::from("q")),
                right: None,
            },
        ),
        (
            " structure=\"q\"",
            " structure=\"p\"",
            Difference::Structure {
                id: id(),
                left: Some(ElementId::from("q")),
                right: Some(ElementId::from("p")),
            },
        ),
        (
            " structure=\"nope\"",
            " structure=\"q\"",
            Difference::Structure {
                id: id(),
                left: Some(ElementId::from("nope")),
                right: Some(ElementId::from("q")),
            },
        ),
    ] {
        assert_eq!(
            semantic_compare(&with(left), &with(right)),
            [expected],
            "{ROW}: {left:?} vs {right:?}: not one Structure difference"
        );
    }
    // Structure comes after AbstractForm, the last end before it.
    let left = with(" abstractForm=\"c1\" structure=\"q\"");
    let right = with("");
    let found = semantic_compare(&left, &right);
    assert!(
        matches!(
            found.as_slice(),
            [
                Difference::AbstractForm { .. },
                Difference::Structure { .. }
            ]
        ),
        "{ROW}: AbstractForm then Structure is not the order: {found:?}"
    );
}
