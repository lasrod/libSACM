//! Tests for `ArtifactReference` and its `referencedArtifactElement`
//! (SACM23-ARG-006; formal/23-05-08 §11.9 "Superclass `ArgumentAsset`",
//! `referencedArtifactElement:Base::ArtifactElement[0..*]`; §11.1; ptc/22-03-13
//! `Argumentation::ArtifactReference`; XMI 2.5.1 rule 4i; ADR 0026, ADR 0027,
//! ADR 0028).
//!
//! An `ArtifactReference` is read under `argumentElement` of an ArgumentPackage.
//! Its `referencedArtifactElement` is a multi-valued reference end
//! (`SacmElement::referenced_artifact_elements`), held in document order with
//! repeats, written after `abstractForm` when not empty, and compared as
//! `Difference::ReferencedArtifactElements`. It is no Assertion, so the
//! Assertion and relationship features are `Unsupported` on it, as is C++'s
//! `referencedArtifact` alias. Every `ModelElement` is an `ArtifactElement`, so a
//! Claim or a package is a well-typed target; only the four `UtilityElement`s
//! are not. `CreateArtifactReference::new(parent, name)` creates one, with
//! `with_id` and `with_referenced_artifact_elements`, and refuses a reference
//! that holds white space, names no element, or names an element that is no
//! `ArtifactElement` (`EditError::WrongReferenceType`, SACM-REF-002).
//!
//! Expected locations are marked in the input with [`MARK`] (U+2038), which
//! the test removes before reading.

use std::fmt::Display;

use sacm::compare::{Difference, semantic_compare};
use sacm::diagnostic::{Code, Diagnostic, Severity, SourceLocation};
use sacm::edit::{
    self, CreateArgumentPackage, CreateArtifactReference, CreateAssuranceCasePackage, CreateClaim,
    CreateDescription, CreateImplementationConstraint, CreateNote, CreateTaggedValue, EditError,
    Operation,
};
use sacm::model::{AbstractClass, Document, ElementId, ElementKind, LangString, SacmElement};
use sacm::validation;
use sacm::xmi::{self, Mode, ReadError, ReadOptions, ReadOutput, WriteOptions};

const ROW: &str = "SACM23-ARG-006";

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

/// The end's name, as in ptc/22-03-13.
const END: &str = "referencedArtifactElement";

// --- Inputs ------------------------------------------------------------------------

/// A named Claim `id`.
fn claim(id: &str) -> String {
    format!(
        "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"{id}\"><name content=\"C\"/>\
         </argumentElement>"
    )
}

/// An `ArtifactReference` `id` named "A" with `attributes`, holding `content`
/// after its name.
fn reference(id: &str, attributes: &str, content: &str) -> String {
    format!(
        "<argumentElement xmi:type=\"sacm:ArtifactReference\" xmi:id=\"{id}\"{attributes}>\
         <name content=\"A\"/>{content}</argumentElement>"
    )
}

/// An ArgumentPackage root `p` named "P", holding the named Claims c1 and c2
/// and then `body`, each on its own line.
fn in_ap(body: &str) -> String {
    format!(
        "<sacm:ArgumentPackage{NS} xmi:id=\"p\">\n  <name content=\"P\"/>\n  {}\n  {}\n  {body}\n\
         </sacm:ArgumentPackage>",
        claim("c1"),
        claim("c2")
    )
}

/// The `ArtifactReference` `x` in [`in_ap`].
fn x_in_ap(attributes: &str, content: &str) -> String {
    in_ap(&reference("x", attributes, content))
}

/// An AssuranceCasePackage root `case` named "Case" holding the
/// ArgumentPackage `p`, which holds the Claims c1 and c2 and then `body`, and
/// after it the empty ArgumentPackage `q`. Every element is named.
fn in_case(body: &str) -> String {
    format!(
        "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\">\n  <name content=\"Case\"/>\n  \
         <argumentPackage xmi:id=\"p\">\n    <name content=\"P\"/>\n    {}\n    {}\n    {body}\n  \
         </argumentPackage>\n  <argumentPackage xmi:id=\"q\"><name content=\"Q\"/></argumentPackage>\n\
         </sacm:AssuranceCasePackage>",
        claim("c1"),
        claim("c2")
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

/// Reads `input` in tolerant mode, which the test expects to read.
fn read_tolerant(what: &str, input: &str) -> Document {
    match read(Mode::Tolerant, input) {
        Ok(output) => output.into_document(),
        Err(error) => panic!("{ROW}: {what}: a tolerant read refused {input:?}: {error:?}"),
    }
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

/// The `referencedArtifactElement` ids of the element `id`, as strings.
fn referenced(doc: &Document, id: &str) -> Vec<String> {
    element(doc, id)
        .referenced_artifact_elements()
        .iter()
        .map(|target| target.as_str().to_owned())
        .collect()
}

fn ids(values: &[&str]) -> Vec<String> {
    values.iter().map(|&value| value.to_owned()).collect()
}

fn list(values: &[&str]) -> Vec<ElementId> {
    values.iter().copied().map(ElementId::from).collect()
}

/// The line of `text` carrying `xmi:id="<id>"`, trimmed.
fn line_with<'t>(text: &'t str, id: &str) -> &'t str {
    let attribute = format!("xmi:id=\"{id}\"");
    let Some(line) = text.lines().find(|line| line.contains(&attribute)) else {
        panic!("{ROW}: the output has no line with {attribute}:\n{text}");
    };
    line.trim()
}

/// The findings of `validate` on `doc` with `code`.
fn with_code(doc: &Document, code: Code) -> Vec<Diagnostic> {
    validation::validate(doc)
        .into_iter()
        .filter(|found| found.code() == code)
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
/// `ArgumentPackage`s `p` and `q`; `p` holds the Claims c1 and c2, and c1 holds
/// the Description d1, the `ImplementationConstraint` i1, the Note n1 and the
/// `TaggedValue` t1.
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
    create(
        &mut doc,
        CreateArgumentPackage::new("case", named("Q")).with_id("q"),
    );
    create(&mut doc, CreateClaim::new("p", named("C1")).with_id("c1"));
    create(&mut doc, CreateClaim::new("p", named("C2")).with_id("c2"));
    create(
        &mut doc,
        CreateDescription::new("c1", named("D")).with_id("d1"),
    );
    create(
        &mut doc,
        CreateImplementationConstraint::new("c1", named("I")).with_id("i1"),
    );
    create(&mut doc, CreateNote::new("c1", named("N")).with_id("n1"));
    create(
        &mut doc,
        CreateTaggedValue::new("c1", named("K"), named("V")).with_id("t1"),
    );
    doc
}

/// The create of an `ArtifactReference` under `parent` with `ids`.
fn reference_op(parent: &str, ids: &[&str]) -> CreateArtifactReference {
    CreateArtifactReference::new(parent, named("Reference"))
        .with_referenced_artifact_elements(ids.iter().copied())
}

/// The name of a reference end in an `EditError`, by its `Display`.
fn end_name(end: &dyn Display) -> String {
    end.to_string()
}

/// Applies `operation`, which the test expects to be refused leaving `doc`
/// `==` to before and with the same next generated id, and returns why.
fn refuse(doc: &mut Document, what: &str, operation: CreateArtifactReference) -> EditError {
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
            CreateArtifactReference::new("p", named("Probe"))
        )
        .as_str(),
        "artifact_reference_1",
        "{ROW}: {what}: the refusal moved the next generated id"
    );
    let message = error.to_string();
    assert!(
        !message.contains('\n') && message.starts_with(char::is_lowercase),
        "{ROW}: {what}: the message {message:?} is not one line starting in lower case"
    );
    error
}

/// Asserts that `error` is `WrongReferenceType` for the end
/// `referencedArtifactElement`, the id `id` of kind `kind`, expecting an
/// `ArtifactElement`, with SACM-REF-002 and a message naming all four.
fn assert_wrong_type(what: &str, error: &EditError, id: &str, kind: ElementKind) {
    let EditError::WrongReferenceType {
        end,
        id: found_id,
        kind: found_kind,
        expected,
        ..
    } = error
    else {
        panic!("{ROW}: {what}: not WrongReferenceType: {error:?}");
    };
    let expected_type = AbstractClass::ArtifactElement.name();
    assert_eq!(
        (
            end_name(end),
            found_id.as_str(),
            *found_kind,
            *expected,
            error.code()
        ),
        (
            END.to_owned(),
            id,
            kind,
            expected_type,
            Code::REF_WRONG_TYPE
        ),
        "{ROW}: {what}: not WrongReferenceType {{ {END}, {id}, {kind}, {expected_type} }} \
         (SACM-REF-002)"
    );
    let message = error.to_string();
    for part in [
        format!("{id:?}"),
        END.to_owned(),
        kind.name().to_owned(),
        expected_type.to_owned(),
    ] {
        assert!(
            message.contains(&part),
            "{ROW}: {what}: the message {message:?} does not name {part}"
        );
    }
}

// =====================================================================================
// Item 1: reading the end in both forms
// =====================================================================================

#[test]
fn sacm23_arg_006_the_attribute_reads_every_id_in_order_in_both_modes() {
    for (value, expected) in [
        ("c1 c2", vec!["c1", "c2"]),
        (" c2&#9;c1&#10; ", vec!["c2", "c1"]),
        ("c1", vec!["c1"]),
    ] {
        let what = format!("referencedArtifactElement={value:?}");
        let doc = read_ok(
            &what,
            &x_in_ap(&format!(" referencedArtifactElement=\"{value}\""), ""),
        );
        let x = element(&doc, "x");
        assert_eq!(
            (
                x.kind(),
                x.parent().map(|parent| parent.id().as_str()),
                referenced(&doc, "x")
            ),
            (ElementKind::ArtifactReference, Some("p"), ids(&expected)),
            "{ROW}: {what}: not read as an ArtifactReference in p with {expected:?}"
        );
    }
    // The acceptance item's own form, with no name and no content.
    let what = "the bare element";
    let doc = read_ok(
        what,
        &in_ap(
            "<argumentElement xmi:type=\"sacm:ArtifactReference\" xmi:id=\"r1\" \
             referencedArtifactElement=\"c1 c2\"/>",
        ),
    );
    assert_eq!(
        referenced(&doc, "r1"),
        ids(&["c1", "c2"]),
        "{ROW}: {what}: not read with [c1, c2]"
    );
    // By xsi:type, as every typed child.
    let what = "an ArtifactReference by xsi:type";
    match read(
        Mode::Tolerant,
        &in_ap(
            "<argumentElement xsi:type=\"sacm:ArtifactReference\" xmi:id=\"r1\" \
             referencedArtifactElement=\"c1\"><name content=\"A\"/></argumentElement>",
        ),
    ) {
        Ok(output) => assert_eq!(
            referenced(output.document(), "r1"),
            ids(&["c1"]),
            "{ROW}: {what}: not read"
        ),
        Err(error) => panic!("{ROW}: {what}: refused: {error:?}"),
    }
}

#[test]
fn sacm23_arg_006_reference_elements_by_idref_or_href_read_equal_to_the_attribute() {
    let attribute = read_ok(
        "the attribute form",
        &x_in_ap(" referencedArtifactElement=\"c1 c2\"", ""),
    );
    for (what, content) in [
        (
            "two xmi:idref elements",
            "<referencedArtifactElement xmi:idref=\"c1\"/>\
             <referencedArtifactElement xmi:idref=\"c2\"/>",
        ),
        (
            "two href elements",
            "<referencedArtifactElement href=\"#c1\"/><referencedArtifactElement href=\"#c2\"/>",
        ),
        (
            "an idref and an href around a note",
            "<referencedArtifactElement xmi:idref=\" c1 \"/><note xmi:id=\"n\"/>\
             <referencedArtifactElement href=\"#c2\"/>",
        ),
    ] {
        let doc = read_ok(what, &x_in_ap("", content));
        assert_eq!(
            referenced(&doc, "x"),
            ids(&["c1", "c2"]),
            "{ROW}: {what}: not read in document order"
        );
        if !what.contains("note") {
            assert_eq!(
                semantic_compare(&doc, &attribute),
                [],
                "{ROW}: {what}: does not read equal to the attribute form"
            );
            assert!(
                doc == attribute,
                "{ROW}: {what}: reads != to the attribute form"
            );
        }
        let text = round_trip(what, &doc, Mode::Strict);
        assert!(
            !text.contains("<referencedArtifactElement"),
            "{ROW}: {what}: written as elements\n{text}"
        );
    }
    // An element before the name reads too.
    let what = "an element before the name";
    let doc = read_ok(
        what,
        &in_ap(
            "<argumentElement xmi:type=\"sacm:ArtifactReference\" xmi:id=\"x\">\
             <referencedArtifactElement xmi:idref=\"c2\"/><name content=\"A\"/>\
             <referencedArtifactElement xmi:idref=\"c1\"/></argumentElement>",
        ),
    );
    assert_eq!(
        referenced(&doc, "x"),
        ids(&["c2", "c1"]),
        "{ROW}: {what}: not read in document order"
    );
}

#[test]
fn sacm23_arg_006_an_attribute_and_elements_are_repeated_value_and_an_empty_attribute_is_unsupported()
 {
    // The attribute and an element.
    let what = "the attribute and an element";
    let (input, marks) = marked(&x_in_ap(
        &format!(" {MARK}referencedArtifactElement=\"c1\""),
        &format!("{MARK}<referencedArtifactElement xmi:idref=\"c2\"/>"),
    ));
    let error = refused(what, &input);
    assert!(
        matches!(
            &error,
            ReadError::RepeatedValue { element, property, .. }
                if element.as_str() == "x" && property == END
        ),
        "{ROW}: {what}: not RepeatedValue {{ x, {END} }}: {error:?}"
    );
    assert_eq!(
        (error.code(), error.locations(), error.location()),
        (
            Some(Code::MULTIPLICITY_VIOLATION),
            marks.as_slice(),
            marks.get(1).copied()
        ),
        "{ROW}: {what}: not SACM-MULT-001 at both occurrences"
    );
    // An attribute of two ids and two elements: refused, naming both forms.
    let what = "an attribute of two ids and two elements";
    let (input, marks) = marked(&x_in_ap(
        &format!(" {MARK}referencedArtifactElement=\"c1 c2\""),
        &format!(
            "{MARK}<referencedArtifactElement href=\"#c1\"/>\
             <referencedArtifactElement href=\"#c2\"/>"
        ),
    ));
    let error = refused(what, &input);
    assert!(
        matches!(
            &error,
            ReadError::RepeatedValue { element, property, .. }
                if element.as_str() == "x" && property == END
        ) && marks.iter().all(|at| error.locations().contains(at)),
        "{ROW}: {what}: not RepeatedValue {{ x, {END} }} naming both forms: {error:?}"
    );
    // An empty value, white space only, and the end in the SACM namespace.
    for (what, attribute) in [
        (
            "an empty attribute",
            format!(" {MARK}referencedArtifactElement=\"\""),
        ),
        (
            "an attribute of white space",
            format!(" {MARK}referencedArtifactElement=\" &#9; \""),
        ),
        (
            "sacm:referencedArtifactElement",
            format!(" {MARK}sacm:referencedArtifactElement=\"c1\""),
        ),
    ] {
        let (input, marks) = marked(&x_in_ap(&attribute, ""));
        let error = refused(what, &input);
        assert_unsupported_at(what, &error, marks[0]);
    }
    let what = "a reference element in the SACM namespace";
    let (input, marks) = marked(&x_in_ap(
        "",
        &format!("{MARK}<sacm:referencedArtifactElement xmi:idref=\"c1\"/>"),
    ));
    let error = refused(what, &input);
    assert_unsupported_at(what, &error, marks[0]);
}

#[test]
fn sacm23_arg_006_referenced_artifact_element_on_another_kind_is_unsupported() {
    let attribute = format!(" {MARK}referencedArtifactElement=\"c1\"");
    let element_form = format!("{MARK}<referencedArtifactElement xmi:idref=\"c1\"/>");
    let typed = |kind: &str, attributes: &str, content: &str| {
        in_ap(&format!(
            "<argumentElement xmi:type=\"sacm:{kind}\" xmi:id=\"x\"{attributes}>\
             <name content=\"X\"/>{content}</argumentElement>"
        ))
    };
    let cases = [
        ("a Claim, as an attribute", typed("Claim", &attribute, "")),
        ("a Claim, as an element", typed("Claim", "", &element_form)),
        (
            "an ArgumentReasoning, as an attribute",
            typed("ArgumentReasoning", &attribute, ""),
        ),
        (
            "an ArgumentGroup, as an element",
            typed("ArgumentGroup", "", &element_form),
        ),
        (
            "an AssertedEvidence, as an attribute",
            typed(
                "AssertedEvidence",
                &format!(" source=\"c1\" target=\"c2\"{attribute}"),
                "",
            ),
        ),
        (
            "an ArgumentPackage root, as an attribute",
            format!(
                "<sacm:ArgumentPackage{NS} xmi:id=\"p\"{attribute}><name content=\"P\"/>\
                 </sacm:ArgumentPackage>"
            ),
        ),
        (
            "a Note, as an attribute",
            in_ap(&format!("<note xmi:id=\"n\"{attribute}/>")),
        ),
    ];
    for (holder, template) in &cases {
        let what = format!("{END} on {holder}");
        let (input, marks) = marked(template);
        assert_eq!(marks.len(), 1, "{ROW}: setup: {what} has not one mark");
        let error = refused(&what, &input);
        assert_unsupported_at(&what, &error, marks[0]);
    }
}

// =====================================================================================
// Item 2: repeats, write order and round trip
// =====================================================================================

#[test]
fn sacm23_arg_006_a_repeated_id_is_held_twice_and_written_back_unchanged() {
    let what = "referencedArtifactElement=\"c1 c1\"";
    let doc = read_ok(what, &x_in_ap(" referencedArtifactElement=\"c1 c1\"", ""));
    assert_eq!(
        referenced(&doc, "x"),
        ids(&["c1", "c1"]),
        "{ROW}: {what}: the repeat is not held"
    );
    for mode in MODES {
        let text = round_trip(what, &doc, mode);
        assert_eq!(
            line_with(&text, "x"),
            "<argumentElement xmi:type=\"sacm:ArtifactReference\" xmi:id=\"x\" \
             referencedArtifactElement=\"c1 c1\">",
            "{ROW}: {what}: not written back unchanged in {mode:?} mode\n{text}"
        );
    }
}

#[test]
fn sacm23_arg_006_the_end_is_written_after_abstract_form_and_absent_when_empty() {
    // Scrambled attribute order, the abstract form `y` an abstract
    // ArtifactReference, so that validation finds nothing.
    let what = "every SACMElement feature and the end";
    let doc = read_ok(
        what,
        &in_ap(&format!(
            "{}\n  {}",
            reference(
                "x",
                " referencedArtifactElement=\"c2 y c1\" abstractForm=\"y\" citedElement=\"c1\" \
                 gid=\"ref-x\" isCitation=\"true\"",
                ""
            ),
            reference("y", " isAbstract=\"true\"", "")
        )),
    );
    assert_eq!(
        validation::validate(&doc),
        [],
        "{ROW}: {what}: setup: the document does not validate cleanly"
    );
    let text = round_trip(what, &doc, Mode::Strict);
    assert_eq!(
        line_with(&text, "x"),
        "<argumentElement xmi:type=\"sacm:ArtifactReference\" xmi:id=\"x\" gid=\"ref-x\" \
         isCitation=\"true\" citedElement=\"c1\" abstractForm=\"y\" \
         referencedArtifactElement=\"c2 y c1\">",
        "{ROW}: {what}: not written in the order ... abstractForm referencedArtifactElement\n{text}"
    );
    // No end: no attribute.
    let what = "an ArtifactReference with no end";
    let doc = read_ok(what, &x_in_ap("", ""));
    assert_eq!(
        referenced(&doc, "x"),
        Vec::<String>::new(),
        "{ROW}: {what}: holds ids"
    );
    let text = round_trip(what, &doc, Mode::Strict);
    assert!(
        line_with(&text, "x")
            == "<argumentElement xmi:type=\"sacm:ArtifactReference\" xmi:id=\"x\">"
            && !text.contains(END),
        "{ROW}: {what}: the empty end is written\n{text}"
    );
    // The element form is written as the attribute.
    let what = "the element form";
    let doc = read_ok(
        what,
        &x_in_ap(
            "",
            "<referencedArtifactElement xmi:idref=\"c2\"/><referencedArtifactElement href=\"#c1\"/>",
        ),
    );
    let text = round_trip(what, &doc, Mode::Strict);
    assert_eq!(
        line_with(&text, "x"),
        "<argumentElement xmi:type=\"sacm:ArtifactReference\" xmi:id=\"x\" \
         referencedArtifactElement=\"c2 c1\">",
        "{ROW}: {what}: not written as one attribute\n{text}"
    );
}

// =====================================================================================
// Item 3: comparison
// =====================================================================================

#[test]
fn sacm23_arg_006_semantic_compare_reports_one_referenced_artifact_elements_difference() {
    let doc = |attributes: &str| read_ok(attributes, &x_in_ap(attributes, ""));
    for (left, right, left_ids, right_ids) in [
        (
            " referencedArtifactElement=\"c1 c2\"",
            " referencedArtifactElement=\"c2 c1\"",
            vec!["c1", "c2"],
            vec!["c2", "c1"],
        ),
        (
            " referencedArtifactElement=\"c1\"",
            " referencedArtifactElement=\"c1 c1\"",
            vec!["c1"],
            vec!["c1", "c1"],
        ),
        (
            " referencedArtifactElement=\"c1\"",
            "",
            vec!["c1"],
            Vec::new(),
        ),
    ] {
        let found = semantic_compare(&doc(left), &doc(right));
        assert_eq!(
            found,
            [Difference::ReferencedArtifactElements {
                id: ElementId::from("x"),
                left: list(&left_ids),
                right: list(&right_ids),
            }],
            "{ROW}: {left:?} vs {right:?}: not one ReferencedArtifactElements difference"
        );
        assert_eq!(
            found[0].element_id(),
            Some(&ElementId::from("x")),
            "{ROW}: {left:?} vs {right:?}: element_id() does not name the reference"
        );
        assert_eq!(
            semantic_compare(&doc(left), &doc(left)),
            [],
            "{ROW}: {left:?}: a document differs from itself"
        );
    }
    // Every SACMElement feature at once: the end comes after AbstractForm.
    let left = doc(
        " gid=\"g\" isCitation=\"true\" isAbstract=\"true\" citedElement=\"c1\" \
         abstractForm=\"x\" referencedArtifactElement=\"c2\"",
    );
    let right = doc("");
    let id = || ElementId::from("x");
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
            Difference::CitedElement {
                id: id(),
                left: Some(ElementId::from("c1")),
                right: None,
            },
            Difference::AbstractForm {
                id: id(),
                left: Some(ElementId::from("x")),
                right: None,
            },
            Difference::ReferencedArtifactElements {
                id: id(),
                left: list(&["c2"]),
                right: Vec::new(),
            },
        ],
        "{ROW}: the differences are not in table order, ending with ReferencedArtifactElements"
    );
}

// =====================================================================================
// Item 4: what an ArtifactReference has and has not
// =====================================================================================

#[test]
fn sacm23_arg_006_assertion_and_relationship_features_and_the_referenced_artifact_alias_are_unsupported()
 {
    let attributes = [
        ("assertionDeclaration", "assumed"),
        ("metaClaim", "c1"),
        ("isCounter", "true"),
        ("source", "c1"),
        ("target", "c1"),
        ("reasoning", "c1"),
        ("referencedArtifact", "c1"),
    ];
    for (feature, value) in attributes {
        let what = format!("{feature}=\"{value}\" on an ArtifactReference");
        let (input, marks) = marked(&x_in_ap(&format!(" {MARK}{feature}=\"{value}\""), ""));
        let error = refused(&what, &input);
        assert_unsupported_at(&what, &error, marks[0]);
    }
    let elements = [
        (
            "assertionDeclaration",
            "<assertionDeclaration>assumed</assertionDeclaration>",
        ),
        ("metaClaim", "<metaClaim xmi:idref=\"c1\"/>"),
        ("isCounter", "<isCounter>true</isCounter>"),
        ("source", "<source xmi:idref=\"c1\"/>"),
        ("target", "<target href=\"#c1\"/>"),
        (
            "referencedArtifact",
            "<referencedArtifact xmi:idref=\"c1\"/>",
        ),
    ];
    for (feature, content) in elements {
        let what = format!("a {feature} element in an ArtifactReference");
        let (input, marks) = marked(&x_in_ap("", &format!("{MARK}{content}")));
        let error = refused(&what, &input);
        assert_unsupported_at(&what, &error, marks[0]);
    }
}

#[test]
fn sacm23_arg_006_it_has_no_assertion_declaration_and_its_inherited_features_round_trip() {
    let what = "an ArtifactReference with every inherited feature";
    let doc = read_ok(
        what,
        &in_ap(&format!(
            "{}\n  {}",
            reference(
                "x",
                " gid=\"ref-x\" isCitation=\"true\" citedElement=\"y\" abstractForm=\"y\" \
                 referencedArtifactElement=\"c1\"",
                "<description xmi:id=\"x_d\"><content><value content=\"D\"/></content></description>\
                 <implementationConstraint xmi:id=\"x_i\"/><note xmi:id=\"x_n\"/>\
                 <taggedValue xmi:id=\"x_t\"><key><value content=\"K\"/></key></taggedValue>"
            ),
            reference("y", " isAbstract=\"true\"", "")
        )),
    );
    let x = element(&doc, "x");
    assert_eq!(
        (
            x.gid(),
            x.is_citation(),
            x.is_abstract(),
            x.cited_element().map(ElementId::as_str),
            x.abstract_form().map(ElementId::as_str),
            x.name().and_then(LangString::content),
            x.assertion_declaration(),
            x.meta_claims().len(),
            x.sources().len(),
            x.target(),
        ),
        (
            Some("ref-x"),
            true,
            false,
            Some("y"),
            Some("y"),
            Some("A"),
            None,
            0,
            0,
            None,
        ),
        "{ROW}: {what}: the inherited features are not read, or it holds an Assertion's"
    );
    let kinds: Vec<ElementKind> = x.children().map(SacmElement::kind).collect();
    assert_eq!(
        kinds,
        [
            ElementKind::Description,
            ElementKind::ImplementationConstraint,
            ElementKind::Note,
            ElementKind::TaggedValue
        ],
        "{ROW}: {what}: the utility elements are not its children in order"
    );
    assert_eq!(
        validation::validate(&doc),
        [],
        "{ROW}: {what}: setup: the document does not validate cleanly"
    );
    round_trip(what, &doc, Mode::Strict);
    // An ArtifactReference is an ArgumentAsset and no Assertion.
    assert!(
        ElementKind::ArtifactReference.is_a(AbstractClass::ArgumentAsset)
            && !ElementKind::ArtifactReference.is_a(AbstractClass::Assertion),
        "{ROW}: an ArtifactReference is not an ArgumentAsset, or is an Assertion"
    );
}

// =====================================================================================
// Item 5: references
// =====================================================================================

#[test]
fn sacm23_arg_006_a_dangling_id_is_ref_001_with_clause_11_9() {
    let what = "referencedArtifactElement=\"c1 nope\"";
    let doc = read_ok(what, &x_in_ap(" referencedArtifactElement=\"c1 nope\"", ""));
    let found = with_code(&doc, Code::REF_DANGLING);
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
            Some("11.9"),
            None,
            [ElementId::from("x"), ElementId::from("nope")].as_slice()
        ),
        "{ROW}: {what}: not an Error of SACM23-VAL-003 with clause 11.9 on [x, nope]"
    );
    assert!(
        finding.message().contains(END) && finding.message().contains("ArtifactReference"),
        "{ROW}: {what}: the message does not name the end and the kind: {finding:?}"
    );
    assert_eq!(
        with_code(&doc, Code::REF_WRONG_TYPE),
        [],
        "{ROW}: {what}: a dangling id is also reported as wrongly typed"
    );
    let text = round_trip(what, &doc, Mode::Tolerant);
    assert!(
        line_with(&text, "x").ends_with(" referencedArtifactElement=\"c1 nope\">"),
        "{ROW}: {what}: the dangling id is not written back\n{text}"
    );
}

#[test]
fn sacm23_arg_006_a_claim_or_a_package_target_gives_no_finding() {
    for (what, value) in [
        ("a Claim", "c1"),
        ("its own ArgumentPackage", "p"),
        ("another ArgumentPackage", "q"),
        ("the AssuranceCasePackage", "case"),
        ("itself", "x"),
        ("all of them", "c1 p q case x c2"),
    ] {
        let what = format!("referencedArtifactElement naming {what}");
        let doc = read_ok(
            &what,
            &in_case(&reference(
                "x",
                &format!(" referencedArtifactElement=\"{value}\""),
                "",
            )),
        );
        assert_eq!(
            validation::validate(&doc),
            [],
            "{ROW}: {what}: validate reports a finding"
        );
        round_trip(&what, &doc, Mode::Strict);
    }
}

// =====================================================================================
// Item 6: the create operation
// =====================================================================================

#[test]
fn sacm23_arg_006_create_artifact_reference_reports_what_it_was_given() {
    let create = CreateArtifactReference::new("p", named("Reference"));
    assert_eq!(
        (
            create.parent().as_str(),
            create.id(),
            create.name(),
            create.referenced_artifact_elements()
        ),
        ("p", None, &named("Reference"), [].as_slice()),
        "{ROW}: the defaults"
    );
    let create = create
        .with_id("given")
        .with_referenced_artifact_elements(["c2", "c1", "c2"]);
    assert_eq!(
        (
            create.id().map(ElementId::as_str),
            create
                .referenced_artifact_elements()
                .iter()
                .map(ElementId::as_str)
                .collect::<Vec<_>>()
        ),
        (Some("given"), vec!["c2", "c1", "c2"]),
        "{ROW}: with_id and with_referenced_artifact_elements are not reported as given"
    );
    assert_eq!(
        Operation::from(create).operation_name(),
        "CreateArtifactReference",
        "{ROW}: operation_name()"
    );
}

#[test]
fn sacm23_arg_006_create_artifact_reference_creates_with_a_generated_id_and_its_references() {
    let mut doc = built();
    // No ids: accepted.
    let id = create(&mut doc, CreateArtifactReference::new("p", named("Plain")));
    assert_eq!(
        id.as_str(),
        "artifact_reference_1",
        "{ROW}: the generated id"
    );
    let found = element(&doc, "artifact_reference_1");
    assert_eq!(
        (
            found.kind(),
            found.parent().map(|parent| parent.id().as_str()),
            found.name(),
            found.referenced_artifact_elements().len(),
            found.assertion_declaration()
        ),
        (
            ElementKind::ArtifactReference,
            Some("p"),
            Some(&named("Plain")),
            0,
            None
        ),
        "{ROW}: the created ArtifactReference"
    );
    assert_eq!(
        element(&doc, "p")
            .children()
            .last()
            .map(|child| child.id().as_str().to_owned()),
        Some("artifact_reference_1".to_owned()),
        "{ROW}: not created as the last child"
    );
    // Any ModelElement: a Claim, both packages, the case and another
    // ArtifactReference; and with a caller id.
    let id = create(
        &mut doc,
        reference_op("p", &["c1", "p", "q", "case", "artifact_reference_1"]).with_id("given"),
    );
    assert_eq!(id.as_str(), "given", "{ROW}: with_id is not used");
    assert_eq!(
        referenced(&doc, "given"),
        ids(&["c1", "p", "q", "case", "artifact_reference_1"]),
        "{ROW}: the references are not set as given"
    );
    // The next generated id, under the other package.
    let id = create(&mut doc, reference_op("q", &["given"]));
    assert_eq!(
        id.as_str(),
        "artifact_reference_2",
        "{ROW}: the second generated id"
    );
    assert_eq!(
        validation::validate(&doc),
        [],
        "{ROW}: the built document does not validate cleanly"
    );
    // Repeats are kept: a content finding (SACM23-VAL-022), not a refusal.
    let id = create(&mut doc, reference_op("p", &["c1", "c2", "c1"]));
    assert_eq!(
        referenced(&doc, id.as_str()),
        ids(&["c1", "c2", "c1"]),
        "{ROW}: a repeated id is not held as given"
    );
    // It writes strictly and reads back equal.
    let text = write_in("the built document", &doc, Mode::Strict);
    let back = read_ok("the built document, written", &text);
    assert_eq!(
        semantic_compare(&doc, &back),
        [],
        "{ROW}: the built document does not read back equal\n{text}"
    );
    assert!(doc == back, "{ROW}: built == reloaded is false\n{text}");
}

#[test]
fn sacm23_arg_006_create_artifact_reference_runs_the_four_create_checks_first() {
    let mut doc = built();
    let error = refuse(
        &mut doc,
        "a missing parent",
        reference_op("nowhere", &["n1"]),
    );
    assert!(
        matches!(&error, EditError::ParentNotFound { parent, .. } if parent.as_str() == "nowhere"),
        "{ROW}: a missing parent is not ParentNotFound: {error:?}"
    );
    for (parent, parent_kind) in [
        ("case", ElementKind::AssuranceCasePackage),
        ("c1", ElementKind::Claim),
        ("n1", ElementKind::Note),
    ] {
        let what = format!("under the {parent_kind} {parent:?}");
        let error = refuse(&mut doc, &what, reference_op(parent, &["nope"]));
        assert!(
            matches!(
                &error,
                EditError::InvalidParent { parent: p, parent_kind: k, kind, .. }
                    if p.as_str() == parent && *k == parent_kind
                        && *kind == ElementKind::ArtifactReference
            ),
            "{ROW}: {what}: not InvalidParent: {error:?}"
        );
        assert_eq!(
            error.code(),
            Code::CMD_INVALID_PARENT,
            "{ROW}: {what}: code"
        );
    }
    let error = refuse(
        &mut doc,
        "an invalid id",
        reference_op("p", &["nope"]).with_id("1x"),
    );
    assert!(
        matches!(&error, EditError::InvalidId { id, .. } if id.as_str() == "1x"),
        "{ROW}: an invalid id is not InvalidId: {error:?}"
    );
    let error = refuse(
        &mut doc,
        "a duplicate id",
        reference_op("p", &["n1"]).with_id("c1"),
    );
    assert!(
        matches!(&error, EditError::DuplicateId { id, .. } if id.as_str() == "c1"),
        "{ROW}: a duplicate id is not DuplicateId: {error:?}"
    );
}

#[test]
fn sacm23_arg_006_create_artifact_reference_refuses_a_missing_or_wrongly_typed_id_unchanged() {
    let mut doc = built();
    // An id naming no element, compared exactly.
    for (what, given, missing) in [
        ("an unknown id", vec!["c1", "nope"], "nope"),
        ("an id differing in case", vec!["C1"], "C1"),
    ] {
        let error = refuse(&mut doc, what, reference_op("p", &given));
        let EditError::ReferenceNotFound { end, id, .. } = &error else {
            panic!("{ROW}: {what}: not ReferenceNotFound: {error:?}");
        };
        assert_eq!(
            (end_name(end), id.as_str(), error.code()),
            (END.to_owned(), missing, Code::CMD_TARGET_NOT_FOUND),
            "{ROW}: {what}: not ReferenceNotFound {{ {END}, {missing} }} (SACM-CMD-001)"
        );
    }
    // Each UtilityElement kind is no ArtifactElement.
    for (id, kind) in [
        ("n1", ElementKind::Note),
        ("d1", ElementKind::Description),
        ("i1", ElementKind::ImplementationConstraint),
        ("t1", ElementKind::TaggedValue),
    ] {
        let what = format!("the {kind} {id}");
        let error = refuse(&mut doc, &what, reference_op("p", &["c1", id]));
        assert_wrong_type(&what, &error, id, kind);
    }
    // Every id is checked for existence before any is checked for type.
    let what = "a Note, then an unknown id";
    let error = refuse(&mut doc, what, reference_op("p", &["n1", "nope"]));
    assert!(
        matches!(&error, EditError::ReferenceNotFound { id, .. } if id.as_str() == "nope"),
        "{ROW}: {what}: the missing id is not reported before the wrong type: {error:?}"
    );
    // The first wrongly typed id is reported.
    let what = "a TaggedValue, then a Note";
    let error = refuse(&mut doc, what, reference_op("p", &["c1", "t1", "n1"]));
    assert_wrong_type(what, &error, "t1", ElementKind::TaggedValue);
}

#[test]
fn sacm23_arg_006_create_artifact_reference_refuses_an_id_holding_white_space_or_a_name_id() {
    let mut doc = read_tolerant(
        "a Claim whose id holds white space, and a name with an id",
        &format!(
            "<sacm:ArgumentPackage{NS} xmi:id=\"p\"><name xmi:id=\"p_name\" content=\"P\"/>\
             {}{}<note xmi:id=\"n1\"/></sacm:ArgumentPackage>",
            claim("c1"),
            claim("a b")
        ),
    );
    assert_eq!(
        element(&doc, "a b").kind(),
        ElementKind::Claim,
        "{ROW}: setup: the tolerant read does not hold the Claim \"a b\" as written"
    );
    let what = "an id holding white space";
    let error = refuse(&mut doc, what, reference_op("p", &["c1", "a b"]));
    let EditError::InvalidReference { end, id, .. } = &error else {
        panic!("{ROW}: {what}: not InvalidReference: {error:?}");
    };
    assert_eq!(
        (end_name(end), id.as_str(), error.code()),
        (END.to_owned(), "a b", Code::ID_INVALID),
        "{ROW}: {what}: not InvalidReference {{ {END}, \"a b\" }} (SACM-ID-002)"
    );
    // White space comes before a missing id and a wrong type.
    for given in [["nope", "a b"], ["n1", "a b"]] {
        let what = format!("{given:?}");
        let error = refuse(&mut doc, &what, reference_op("p", &given));
        assert!(
            matches!(&error, EditError::InvalidReference { id, .. } if id.as_str() == "a b"),
            "{ROW}: {what}: white space is not reported first: {error:?}"
        );
    }
    // The id of a name identifies no element.
    let what = "the id of a name";
    let error = refuse(&mut doc, what, reference_op("p", &["p_name"]));
    assert!(
        matches!(&error, EditError::ReferenceNotFound { id, .. } if id.as_str() == "p_name"),
        "{ROW}: {what}: not ReferenceNotFound: {error:?}"
    );
}

#[test]
fn sacm23_arg_006_create_artifact_reference_accepts_an_artifact_held_through_the_seam() {
    // Artifacts (§12) are not read yet; the test-support seam holds one.
    let mut doc = built();
    for (parent, kind, id, name) in [
        (
            Some("case"),
            ElementKind::ArtifactPackage,
            "arts",
            "Artifacts",
        ),
        (
            Some("arts"),
            ElementKind::Artifact,
            "art",
            "Brake test report",
        ),
    ] {
        let parent = parent.map(ElementId::from);
        if let Err(error) = doc.insert(
            parent.as_ref(),
            kind,
            ElementId::from(id),
            Some(named(name)),
        ) {
            panic!("{ROW}: setup: the {kind} {id:?} could not be inserted: {error}");
        }
    }
    let id = create(&mut doc, reference_op("p", &["art", "arts"]));
    assert_eq!(
        referenced(&doc, id.as_str()),
        ids(&["art", "arts"]),
        "{ROW}: the Artifact and its package are not referenced"
    );
    let wrong: Vec<Diagnostic> = with_code(&doc, Code::REF_WRONG_TYPE)
        .into_iter()
        .chain(with_code(&doc, Code::REF_DANGLING))
        .collect();
    assert_eq!(
        wrong,
        [],
        "{ROW}: an Artifact or an ArtifactPackage target is reported"
    );
}

// =====================================================================================
// The fixture and the golden file
// =====================================================================================

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

#[test]
fn sacm23_arg_006_the_fixture_and_the_golden_file_read_their_references_and_round_trip() {
    let fixture = std::fs::read_to_string(FIXTURE)
        .unwrap_or_else(|error| panic!("{ROW}: cannot read {FIXTURE}: {error}"));
    let doc = read_ok("the fixture", &fixture);
    for (id, expected) in [
        ("report", vec!["h1", "h2"]),
        ("plan", vec!["h1"]),
        ("standard", Vec::new()),
    ] {
        assert_eq!(
            (element(&doc, id).kind(), referenced(&doc, id)),
            (ElementKind::ArtifactReference, ids(&expected)),
            "{ROW}: the fixture's {id}"
        );
    }
    assert_eq!(
        validation::validate(&doc),
        [],
        "{ROW}: the fixture does not validate cleanly"
    );
    round_trip("the fixture", &doc, Mode::Strict);

    let golden = std::fs::read_to_string(GOLDEN)
        .unwrap_or_else(|error| panic!("{ROW}: cannot read {GOLDEN}: {error}"));
    let doc = read_ok("the golden file", &golden);
    assert_eq!(
        (referenced(&doc, "a1"), referenced(&doc, "a2")),
        (ids(&["h1", "h2"]), Vec::new()),
        "{ROW}: the golden file's ArtifactReferences"
    );
    for mode in MODES {
        assert_eq!(
            round_trip("the golden file", &doc, mode),
            golden,
            "{ROW}: a {mode:?} write of the golden file is not its own bytes"
        );
    }
}
