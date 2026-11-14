//! Tests for `ArgumentGroup` and its membership end `argumentElement`
//! (SACM23-ARG-013; formal/23-05-08 §11.2 "Superclass `ArgumentationElement`",
//! `argumentationElement:ArgumentationElement[0..*]`; ptc/22-03-13
//! `Argumentation::ArgumentGroup` (`argumentElement`, no aggregation);
//! D-TEXT-MODEL; ADR 0026, ADR 0027, ADR 0028).
//!
//! An `ArgumentGroup` is read under `argumentElement` of an ArgumentPackage.
//! Its own `argumentElement` is a multi-valued reference end, not
//! containment (`SacmElement::group_members`), held in document order with
//! repeats, written after `abstractForm` when not empty, and compared as
//! `Difference::GroupMembers`. The parent's kind decides how
//! `argumentElement` reads: a reference under a group, the composite role
//! under a package. A nested element under a group's `argumentElement` is
//! `Unsupported`, never dropped, and the text's spelling
//! `argumentationElement` stays `Unsupported` (SACM23-XMI-009).
//! `CreateArgumentGroup::new(parent, name)` creates one, with `with_id` and
//! `with_group_members`.
//!
//! The last test checks, through the reader, that no readable class has an
//! element-typed composite role and a reference end of the same name. The
//! roles and ends of each readable kind are written here from
//! `docs/sacm/metamodel-inventory.md` and ptc/22-03-13, not taken from the
//! crate.
//!
//! Expected locations are marked in the input with [`MARK`] (U+2038), which
//! the test removes before reading.

use std::fmt::Display;

use sacm::compare::{Difference, semantic_compare};
use sacm::diagnostic::{Code, Diagnostic, Severity, SourceLocation};
use sacm::edit::{
    self, CreateArgumentGroup, CreateArgumentPackage, CreateArgumentReasoning,
    CreateArtifactReference, CreateAssertedInference, CreateAssuranceCasePackage, CreateClaim,
    CreateNote, EditError, Operation,
};
use sacm::model::{AbstractClass, Document, ElementId, ElementKind, LangString, SacmElement};
use sacm::validation;
use sacm::xmi::{self, Mode, ReadError, ReadOptions, ReadOutput, WriteOptions};

const ROW: &str = "SACM23-ARG-013";

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
const END: &str = "argumentElement";

// --- Inputs ------------------------------------------------------------------------

/// A named Claim `id`.
fn claim(id: &str) -> String {
    format!(
        "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"{id}\"><name content=\"C\"/>\
         </argumentElement>"
    )
}

/// An `ArgumentGroup` `id` named "G" with `attributes`, holding `content`
/// after its name.
fn group(id: &str, attributes: &str, content: &str) -> String {
    format!(
        "<argumentElement xmi:type=\"sacm:ArgumentGroup\" xmi:id=\"{id}\"{attributes}>\
         <name content=\"G\"/>{content}</argumentElement>"
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

/// The `ArgumentGroup` `g` in [`in_ap`].
fn g_in_ap(attributes: &str, content: &str) -> String {
    in_ap(&group("g", attributes, content))
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

/// The members of the group `id`, as strings.
fn members(doc: &Document, id: &str) -> Vec<String> {
    element(doc, id)
        .group_members()
        .iter()
        .map(|member| member.as_str().to_owned())
        .collect()
}

/// The ids of the children of `id`, in order.
fn children(doc: &Document, id: &str) -> Vec<String> {
    element(doc, id)
        .children()
        .map(|child| child.id().as_str().to_owned())
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
/// `ArgumentPackage`s `p` and `q`. `p` holds the Claims c1 and c2, the
/// `ArgumentReasoning` r1, the `ArtifactReference` a1 and the
/// `AssertedInference` i1 (c2 to c1); c1 holds the Note n1.
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
        CreateArgumentReasoning::new("p", named("R1")).with_id("r1"),
    );
    create(
        &mut doc,
        CreateArtifactReference::new("p", named("A1")).with_id("a1"),
    );
    create(
        &mut doc,
        CreateAssertedInference::new("p", named("I1"), ["c2"], "c1").with_id("i1"),
    );
    create(&mut doc, CreateNote::new("c1", named("N")).with_id("n1"));
    doc
}

/// The create of an `ArgumentGroup` under `parent` with `members`.
fn group_op(parent: &str, members: &[&str]) -> CreateArgumentGroup {
    CreateArgumentGroup::new(parent, named("Group")).with_group_members(members.iter().copied())
}

/// The name of a reference end in an `EditError`, by its `Display`.
fn end_name(end: &dyn Display) -> String {
    end.to_string()
}

/// Applies `operation`, which the test expects to be refused leaving `doc`
/// `==` to before and with the same next generated id, and returns why.
fn refuse(doc: &mut Document, what: &str, operation: CreateArgumentGroup) -> EditError {
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
        create(&mut probe, CreateArgumentGroup::new("p", named("Probe"))).as_str(),
        "argument_group_1",
        "{ROW}: {what}: the refusal moved the next generated id"
    );
    let message = error.to_string();
    assert!(
        !message.contains('\n') && message.starts_with(char::is_lowercase),
        "{ROW}: {what}: the message {message:?} is not one line starting in lower case"
    );
    error
}

// =====================================================================================
// Item 1: members are references, and stay where they are contained
// =====================================================================================

#[test]
fn sacm23_arg_013_the_attribute_reads_its_members_in_both_modes_and_they_stay_in_their_package() {
    // The acceptance item's own form, with no name.
    let what = "the bare group";
    let doc = read_ok(
        what,
        &in_ap(
            "<argumentElement xmi:type=\"sacm:ArgumentGroup\" xmi:id=\"g1\" \
             argumentElement=\"c1 c2\"/>",
        ),
    );
    let g1 = element(&doc, "g1");
    assert_eq!(
        (
            g1.kind(),
            g1.parent().map(|parent| parent.id().as_str()),
            members(&doc, "g1"),
            children(&doc, "g1"),
        ),
        (
            ElementKind::ArgumentGroup,
            Some("p"),
            ids(&["c1", "c2"]),
            Vec::new()
        ),
        "{ROW}: {what}: not an ArgumentGroup in p with the members [c1, c2] and no children"
    );
    assert_eq!(
        children(&doc, "p"),
        ids(&["c1", "c2", "g1"]),
        "{ROW}: {what}: c1 and c2 are not children of their package"
    );
    for member in ["c1", "c2"] {
        assert_eq!(
            element(&doc, member)
                .parent()
                .map(|parent| parent.id().as_str()),
            Some("p"),
            "{ROW}: {what}: {member} moved into the group"
        );
    }
    // White space separates the ids, in order.
    let what = "argumentElement=\" c2&#9;c1 \"";
    let doc = read_ok(what, &g_in_ap(" argumentElement=\" c2&#9;c1 \"", ""));
    assert_eq!(
        members(&doc, "g"),
        ids(&["c2", "c1"]),
        "{ROW}: {what}: not read in order"
    );
}

// =====================================================================================
// Item 2: the element form, and a nested element
// =====================================================================================

#[test]
fn sacm23_arg_013_member_elements_by_idref_or_href_read_equal_to_the_attribute() {
    let attribute = read_ok(
        "the attribute form",
        &g_in_ap(" argumentElement=\"c1 c2\"", ""),
    );
    for (what, content) in [
        (
            "two xmi:idref elements",
            "<argumentElement xmi:idref=\"c1\"/><argumentElement xmi:idref=\"c2\"/>",
        ),
        (
            "two href elements",
            "<argumentElement href=\"#c1\"/><argumentElement href=\"#c2\"/>",
        ),
        (
            "an idref and an href",
            "<argumentElement xmi:idref=\"c1\"/><argumentElement href=\"#c2\"/>",
        ),
    ] {
        let doc = read_ok(what, &g_in_ap("", content));
        assert_eq!(
            (members(&doc, "g"), children(&doc, "g")),
            (ids(&["c1", "c2"]), Vec::new()),
            "{ROW}: {what}: not read as the members [c1, c2]"
        );
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
}

/// A group's `argumentElement` is a reference end, so an `argumentElement`
/// element in a group is a reference element. A nested element there is
/// refused as any reference element with an attribute that no reference
/// element has: at the first such attribute (SACM23-XMI-010's "an xmi:type
/// on a reference element" and "an xmi:id on a reference element"), or at
/// its start tag when it has no link (ADR 0026).
#[test]
fn sacm23_arg_013_a_nested_element_under_a_group_is_unsupported_at_that_construct() {
    for (what, nested) in [
        (
            "a typed Claim",
            format!(
                "<argumentElement {MARK}xmi:type=\"sacm:Claim\" xmi:id=\"c9\"><name content=\"C9\"/>\
                 </argumentElement>"
            ),
        ),
        (
            "a typed Claim with no content",
            format!("<argumentElement {MARK}xmi:type=\"sacm:Claim\" xmi:id=\"c9\"/>"),
        ),
        (
            "a typed ArgumentGroup",
            format!("<argumentElement {MARK}xmi:type=\"sacm:ArgumentGroup\" xmi:id=\"g9\"/>"),
        ),
        (
            "an element by xsi:type",
            format!("<argumentElement {MARK}xsi:type=\"sacm:Claim\" xmi:id=\"c9\"/>"),
        ),
        (
            "an untyped element with an id",
            format!("<argumentElement {MARK}xmi:id=\"c9\"/>"),
        ),
        (
            "an element with only a name",
            format!("{MARK}<argumentElement><name content=\"C9\"/></argumentElement>"),
        ),
    ] {
        let what = format!("{what} inside a group");
        let (input, marks) = marked(&g_in_ap("", &nested));
        let error = refused(&what, &input);
        assert_unsupported_at(&what, &error, marks[0]);
    }
    // After a member element too: still refused, never dropped.
    let what = "a typed Claim after a member element";
    let (input, marks) = marked(&g_in_ap(
        "",
        &format!(
            "<argumentElement xmi:idref=\"c1\"/>\
             <argumentElement {MARK}xmi:type=\"sacm:Claim\" xmi:id=\"c9\"/>"
        ),
    ));
    let error = refused(what, &input);
    assert_unsupported_at(what, &error, marks[0]);
}

#[test]
fn sacm23_arg_013_an_attribute_and_member_elements_are_repeated_value_and_an_empty_one_is_unsupported()
 {
    let what = "the attribute and a member element";
    let (input, marks) = marked(&g_in_ap(
        &format!(" {MARK}argumentElement=\"c1\""),
        &format!("{MARK}<argumentElement xmi:idref=\"c2\"/>"),
    ));
    let error = refused(what, &input);
    assert!(
        matches!(
            &error,
            ReadError::RepeatedValue { element, property, .. }
                if element.as_str() == "g" && property == END
        ),
        "{ROW}: {what}: not RepeatedValue {{ g, {END} }}: {error:?}"
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
    for (what, attribute) in [
        ("an empty attribute", format!(" {MARK}argumentElement=\"\"")),
        (
            "sacm:argumentElement",
            format!(" {MARK}sacm:argumentElement=\"c1\""),
        ),
    ] {
        let (input, marks) = marked(&g_in_ap(&attribute, ""));
        let error = refused(what, &input);
        assert_unsupported_at(what, &error, marks[0]);
    }
}

// =====================================================================================
// Item 3: the text's spelling, and the attribute on a package
// =====================================================================================

#[test]
fn sacm23_arg_013_argumentation_element_on_a_group_and_argument_element_on_a_package_are_unsupported()
 {
    let cases = [
        (
            "an argumentationElement attribute on a group",
            g_in_ap(&format!(" {MARK}argumentationElement=\"c1\""), ""),
        ),
        (
            "an argumentationElement reference element in a group",
            g_in_ap(
                "",
                &format!("{MARK}<argumentationElement xmi:idref=\"c1\"/>"),
            ),
        ),
        (
            "an argumentationElement nested element in a group",
            g_in_ap(
                "",
                &format!("{MARK}<argumentationElement xmi:type=\"sacm:Claim\" xmi:id=\"c9\"/>"),
            ),
        ),
        (
            "an argumentElement attribute on an ArgumentPackage root",
            format!(
                "<sacm:ArgumentPackage{NS} xmi:id=\"p\" {MARK}argumentElement=\"c1\">\
                 <name content=\"P\"/>{}</sacm:ArgumentPackage>",
                claim("c1")
            ),
        ),
        (
            "an argumentElement attribute on an ArgumentPackage child",
            format!(
                "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\"><name content=\"Case\"/>\
                 <argumentPackage xmi:id=\"p\" {MARK}argumentElement=\"c1\"><name content=\"P\"/>\
                 {}</argumentPackage></sacm:AssuranceCasePackage>",
                claim("c1")
            ),
        ),
        (
            "an argumentElement reference element in an ArgumentPackage",
            format!(
                "<sacm:ArgumentPackage{NS} xmi:id=\"p\"><name content=\"P\"/>{}\
                 {MARK}<argumentElement xmi:idref=\"c1\"/></sacm:ArgumentPackage>",
                claim("c1")
            ),
        ),
        (
            "an argumentElement attribute on a Claim",
            in_ap(&format!(
                "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"x\" {MARK}argumentElement=\"c1\">\
                 <name content=\"X\"/></argumentElement>"
            )),
        ),
    ];
    for (what, template) in &cases {
        let (input, marks) = marked(template);
        assert_eq!(marks.len(), 1, "{ROW}: setup: {what} has not one mark");
        let error = refused(what, &input);
        assert_unsupported_at(what, &error, marks[0]);
    }
    // The composite role still reads under a package.
    let what = "a Claim in argumentElement of an ArgumentPackage";
    let doc = read_ok(what, &in_ap(""));
    assert_eq!(
        (
            children(&doc, "p"),
            element(&doc, "p").group_members().len()
        ),
        (ids(&["c1", "c2"]), 0),
        "{ROW}: {what}: not read as children, or read as members"
    );
}

// =====================================================================================
// Item 4: write, compare and round trip
// =====================================================================================

#[test]
fn sacm23_arg_013_members_are_written_after_abstract_form_kept_when_repeated_and_absent_when_empty()
{
    let what = "every SACMElement feature and the end";
    let doc = read_ok(
        what,
        &in_ap(&format!(
            "{}\n  {}",
            group(
                "g",
                " argumentElement=\"c2 h c1\" abstractForm=\"h\" citedElement=\"h\" gid=\"group-g\" \
                 isCitation=\"true\"",
                ""
            ),
            group("h", " isAbstract=\"true\"", "")
        )),
    );
    assert_eq!(
        validation::validate(&doc),
        [],
        "{ROW}: {what}: setup: the document does not validate cleanly"
    );
    let text = round_trip(what, &doc, Mode::Strict);
    assert_eq!(
        line_with(&text, "g"),
        "<argumentElement xmi:type=\"sacm:ArgumentGroup\" xmi:id=\"g\" gid=\"group-g\" \
         isCitation=\"true\" citedElement=\"h\" abstractForm=\"h\" argumentElement=\"c2 h c1\">",
        "{ROW}: {what}: not written in the order ... abstractForm argumentElement\n{text}"
    );
    // A repeat is kept.
    let what = "argumentElement=\"c1 c1\"";
    let doc = read_ok(what, &g_in_ap(" argumentElement=\"c1 c1\"", ""));
    assert_eq!(
        members(&doc, "g"),
        ids(&["c1", "c1"]),
        "{ROW}: {what}: the repeat is not held"
    );
    for mode in MODES {
        let text = round_trip(what, &doc, mode);
        assert_eq!(
            line_with(&text, "g"),
            "<argumentElement xmi:type=\"sacm:ArgumentGroup\" xmi:id=\"g\" \
             argumentElement=\"c1 c1\">",
            "{ROW}: {what}: not written back unchanged in {mode:?} mode\n{text}"
        );
    }
    // The member elements are written as the attribute.
    let what = "member elements";
    let doc = read_ok(
        what,
        &g_in_ap(
            "",
            "<argumentElement xmi:idref=\"c2\"/><argumentElement href=\"#c1\"/>",
        ),
    );
    let text = round_trip(what, &doc, Mode::Strict);
    assert_eq!(
        line_with(&text, "g"),
        "<argumentElement xmi:type=\"sacm:ArgumentGroup\" xmi:id=\"g\" argumentElement=\"c2 c1\">",
        "{ROW}: {what}: not written as one attribute\n{text}"
    );
    // No members: no attribute.
    let what = "a group with no members";
    let doc = read_ok(what, &g_in_ap("", ""));
    let text = round_trip(what, &doc, Mode::Strict);
    assert_eq!(
        line_with(&text, "g"),
        "<argumentElement xmi:type=\"sacm:ArgumentGroup\" xmi:id=\"g\">",
        "{ROW}: {what}: the empty end is written\n{text}"
    );
}

#[test]
fn sacm23_arg_013_semantic_compare_reports_one_group_members_difference() {
    let doc = |attributes: &str| read_ok(attributes, &g_in_ap(attributes, ""));
    for (left, right, left_ids, right_ids) in [
        (
            " argumentElement=\"c1 c2\"",
            " argumentElement=\"c2 c1\"",
            vec!["c1", "c2"],
            vec!["c2", "c1"],
        ),
        (
            " argumentElement=\"c1\"",
            " argumentElement=\"c1 c1\"",
            vec!["c1"],
            vec!["c1", "c1"],
        ),
        (" argumentElement=\"c1\"", "", vec!["c1"], Vec::new()),
    ] {
        let found = semantic_compare(&doc(left), &doc(right));
        assert_eq!(
            found,
            [Difference::GroupMembers {
                id: ElementId::from("g"),
                left: list(&left_ids),
                right: list(&right_ids),
            }],
            "{ROW}: {left:?} vs {right:?}: not one GroupMembers difference"
        );
        assert_eq!(
            found[0].element_id(),
            Some(&ElementId::from("g")),
            "{ROW}: {left:?} vs {right:?}: element_id() does not name the group"
        );
    }
    // After AbstractForm.
    let found = semantic_compare(&doc(" abstractForm=\"g\" argumentElement=\"c1\""), &doc(""));
    assert!(
        matches!(
            found.as_slice(),
            [
                Difference::AbstractForm { .. },
                Difference::GroupMembers { .. }
            ]
        ),
        "{ROW}: AbstractForm then GroupMembers is not the order: {found:?}"
    );
}

#[test]
fn sacm23_arg_013_a_group_members_difference_displays_its_end_and_both_lists() {
    let doc = |attributes: &str| read_ok(attributes, &g_in_ap(attributes, ""));
    let found = semantic_compare(
        &doc(" argumentElement=\"c1\""),
        &doc(" argumentElement=\"c2 c1\""),
    );
    let [difference] = found.as_slice() else {
        panic!("{ROW}: not one difference: {found:?}");
    };
    let line = difference.to_string();
    assert_eq!(
        line,
        "element \"g\" differs in argumentElement: \
         [\"c1\"] on the left, [\"c2\", \"c1\"] on the right",
        "{ROW}: the line does not name the end argumentElement and both lists"
    );
    assert!(
        !line.contains("citedElement"),
        "{ROW}: the line names citedElement, not argumentElement: {line:?}"
    );
}

#[test]
fn sacm23_arg_013_a_group_holding_itself_another_group_or_its_package_gives_no_finding() {
    for (what, value) in [
        ("itself", "g"),
        ("another group", "h"),
        ("its own package", "p"),
        ("another package", "q"),
        ("an asserted relationship", "i"),
        ("a reasoning and an artifact reference", "r a"),
        ("all of them", "g h p q i r a c1 c2"),
    ] {
        let what = format!("members naming {what}");
        let doc = read_ok(
            &what,
            &in_case(&format!(
                "{}\n    {}\n    {}\n    {}\n    {}",
                group("g", &format!(" argumentElement=\"{value}\""), ""),
                group("h", "", ""),
                "<argumentElement xmi:type=\"sacm:AssertedInference\" xmi:id=\"i\" source=\"c2\" \
                 target=\"c1\"><name content=\"I\"/></argumentElement>",
                "<argumentElement xmi:type=\"sacm:ArgumentReasoning\" xmi:id=\"r\">\
                 <name content=\"R\"/></argumentElement>",
                "<argumentElement xmi:type=\"sacm:ArtifactReference\" xmi:id=\"a\">\
                 <name content=\"A\"/></argumentElement>",
            )),
        );
        assert_eq!(
            validation::validate(&doc),
            [],
            "{ROW}: {what}: validate reports a finding"
        );
        round_trip(&what, &doc, Mode::Strict);
    }
    // A dangling member is SACM-REF-001 with clause 11.2.
    let what = "argumentElement=\"c1 nope\"";
    let doc = read_ok(what, &g_in_ap(" argumentElement=\"c1 nope\"", ""));
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
            Some("11.2"),
            None,
            [ElementId::from("g"), ElementId::from("nope")].as_slice()
        ),
        "{ROW}: {what}: not an Error of SACM23-VAL-003 with clause 11.2 on [g, nope]"
    );
    let text = round_trip(what, &doc, Mode::Tolerant);
    assert!(
        line_with(&text, "g").ends_with(" argumentElement=\"c1 nope\">"),
        "{ROW}: {what}: the dangling id is not written back\n{text}"
    );
}

// =====================================================================================
// Item 5: the create operation
// =====================================================================================

#[test]
fn sacm23_arg_013_create_argument_group_reports_what_it_was_given() {
    let create = CreateArgumentGroup::new("p", named("Group"));
    assert_eq!(
        (
            create.parent().as_str(),
            create.id(),
            create.name(),
            create.group_members()
        ),
        ("p", None, &named("Group"), [].as_slice()),
        "{ROW}: the defaults"
    );
    let create = create
        .with_id("given")
        .with_group_members(["c2", "c1", "c2"]);
    assert_eq!(
        (
            create.id().map(ElementId::as_str),
            create
                .group_members()
                .iter()
                .map(ElementId::as_str)
                .collect::<Vec<_>>()
        ),
        (Some("given"), vec!["c2", "c1", "c2"]),
        "{ROW}: with_id and with_group_members are not reported as given"
    );
    assert_eq!(
        Operation::from(create).operation_name(),
        "CreateArgumentGroup",
        "{ROW}: operation_name()"
    );
}

#[test]
fn sacm23_arg_013_create_argument_group_creates_with_a_generated_id_and_its_members() {
    let mut doc = built();
    let id = create(&mut doc, CreateArgumentGroup::new("p", named("Empty")));
    assert_eq!(id.as_str(), "argument_group_1", "{ROW}: the generated id");
    let found = element(&doc, "argument_group_1");
    assert_eq!(
        (
            found.kind(),
            found.parent().map(|parent| parent.id().as_str()),
            found.name(),
            found.group_members().len(),
            found.assertion_declaration()
        ),
        (
            ElementKind::ArgumentGroup,
            Some("p"),
            Some(&named("Empty")),
            0,
            None
        ),
        "{ROW}: the created ArgumentGroup"
    );
    // Every ArgumentationElement kind it can name, and a caller id.
    let all = ["c1", "r1", "a1", "i1", "argument_group_1", "p", "q"];
    let id = create(&mut doc, group_op("p", &all).with_id("given"));
    assert_eq!(id.as_str(), "given", "{ROW}: with_id is not used");
    assert_eq!(
        members(&doc, "given"),
        ids(&all),
        "{ROW}: the members are not set as given"
    );
    assert_eq!(
        children(&doc, "p"),
        ids(&["c1", "c2", "r1", "a1", "i1", "argument_group_1", "given"]),
        "{ROW}: a member moved, or the group is not the last child"
    );
    let id = create(&mut doc, group_op("q", &["given"]));
    assert_eq!(
        id.as_str(),
        "argument_group_2",
        "{ROW}: the second generated id"
    );
    assert_eq!(
        validation::validate(&doc),
        [],
        "{ROW}: the built document does not validate cleanly"
    );
    // Repeats are kept.
    let id = create(&mut doc, group_op("p", &["c1", "c1"]));
    assert_eq!(
        members(&doc, id.as_str()),
        ids(&["c1", "c1"]),
        "{ROW}: a repeated member is not held as given"
    );
    let text = write_in("the built document", &doc, Mode::Strict);
    let back = read_ok("the built document, written", &text);
    assert!(doc == back, "{ROW}: built == reloaded is false\n{text}");
}

#[test]
fn sacm23_arg_013_create_argument_group_runs_the_adr_0027_refusals_unchanged() {
    let mut doc = built();
    // ADR 0019's four checks.
    let error = refuse(&mut doc, "a missing parent", group_op("nowhere", &["case"]));
    assert!(
        matches!(&error, EditError::ParentNotFound { parent, .. } if parent.as_str() == "nowhere"),
        "{ROW}: a missing parent is not ParentNotFound: {error:?}"
    );
    for (parent, parent_kind) in [
        ("case", ElementKind::AssuranceCasePackage),
        ("c1", ElementKind::Claim),
        ("argument_group_x", ElementKind::ArgumentGroup),
    ] {
        if parent == "argument_group_x" {
            create(
                &mut doc,
                CreateArgumentGroup::new("p", named("X")).with_id(parent),
            );
        }
        let what = format!("under the {parent_kind} {parent:?}");
        let error = refuse(&mut doc, &what, group_op(parent, &["case"]));
        assert!(
            matches!(
                &error,
                EditError::InvalidParent { parent: p, parent_kind: k, kind, .. }
                    if p.as_str() == parent && *k == parent_kind
                        && *kind == ElementKind::ArgumentGroup
            ),
            "{ROW}: {what}: not InvalidParent: {error:?}"
        );
    }
    let error = refuse(
        &mut doc,
        "an invalid id",
        group_op("p", &["case"]).with_id("1g"),
    );
    assert!(
        matches!(&error, EditError::InvalidId { id, .. } if id.as_str() == "1g"),
        "{ROW}: an invalid id is not InvalidId: {error:?}"
    );
    let error = refuse(
        &mut doc,
        "a duplicate id",
        group_op("p", &["case"]).with_id("c1"),
    );
    assert!(
        matches!(&error, EditError::DuplicateId { id, .. } if id.as_str() == "c1"),
        "{ROW}: a duplicate id is not DuplicateId: {error:?}"
    );
    // A missing member.
    let what = "an unknown member";
    let error = refuse(&mut doc, what, group_op("p", &["c1", "nope"]));
    let EditError::ReferenceNotFound { end, id, .. } = &error else {
        panic!("{ROW}: {what}: not ReferenceNotFound: {error:?}");
    };
    assert_eq!(
        (end_name(end), id.as_str(), error.code()),
        (END.to_owned(), "nope", Code::CMD_TARGET_NOT_FOUND),
        "{ROW}: {what}: not ReferenceNotFound {{ {END}, nope }} (SACM-CMD-001)"
    );
}

#[test]
fn sacm23_arg_013_create_argument_group_refuses_a_case_or_a_note_member_unchanged() {
    let mut doc = built();
    // A member that is no ArgumentationElement: the AssuranceCasePackage,
    // and a Note.
    for (id, kind) in [
        ("case", ElementKind::AssuranceCasePackage),
        ("n1", ElementKind::Note),
    ] {
        let what = format!("the {kind} {id} as a member");
        let error = refuse(&mut doc, &what, group_op("p", &["c1", id]));
        let EditError::WrongReferenceType {
            end,
            id: found_id,
            kind: found_kind,
            expected,
            ..
        } = &error
        else {
            panic!("{ROW}: {what}: not WrongReferenceType: {error:?}");
        };
        let expected_type = AbstractClass::ArgumentationElement.name();
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
            "{ROW}: {what}: not WrongReferenceType {{ {END}, {id}, {kind}, {expected_type} }}"
        );
        let message = error.to_string();
        assert!(
            message.contains(&format!("{id:?}"))
                && message.contains(END)
                && message.contains(kind.name())
                && message.contains(expected_type),
            "{ROW}: {what}: the message {message:?} does not name the id, end, kind and type"
        );
    }
    // Existence before type.
    let what = "the case, then an unknown member";
    let error = refuse(&mut doc, what, group_op("p", &["case", "nope"]));
    assert!(
        matches!(&error, EditError::ReferenceNotFound { id, .. } if id.as_str() == "nope"),
        "{ROW}: {what}: the missing member is not reported before the wrong type: {error:?}"
    );
}

#[test]
fn sacm23_arg_013_create_argument_group_refuses_a_member_holding_white_space() {
    let mut doc = read_tolerant(
        "a Claim whose id holds white space",
        &format!(
            "<sacm:ArgumentPackage{NS} xmi:id=\"p\"><name content=\"P\"/>{}{}\
             </sacm:ArgumentPackage>",
            claim("c1"),
            claim("a b")
        ),
    );
    for given in [vec!["a b"], vec!["c1", "a b"], vec!["nope", "a b"]] {
        let what = format!("members {given:?}");
        let error = refuse(&mut doc, &what, group_op("p", &given));
        let EditError::InvalidReference { end, id, .. } = &error else {
            panic!("{ROW}: {what}: not InvalidReference: {error:?}");
        };
        assert_eq!(
            (end_name(end), id.as_str(), error.code()),
            (END.to_owned(), "a b", Code::ID_INVALID),
            "{ROW}: {what}: not InvalidReference {{ {END}, \"a b\" }} (SACM-ID-002)"
        );
    }
}

// =====================================================================================
// Item 6: no class has a role and an end of the same name
// =====================================================================================

/// One readable kind, as an element `x` of that kind given its extra
/// start-tag attributes and the content after its name (or value).
type Holder = (ElementKind, fn(&str, &str) -> String);

/// Every kind this release reads, each as the element `x`.
fn readable_holders() -> [Holder; 15] {
    fn typed(kind: &str, attributes: &str, content: &str) -> String {
        in_ap(&format!(
            "<argumentElement xmi:type=\"sacm:{kind}\" xmi:id=\"x\"{attributes}>\
             <name content=\"X\"/>{content}</argumentElement>"
        ))
    }
    [
        (ElementKind::AssuranceCasePackage, |attributes, content| {
            format!(
                "<sacm:AssuranceCasePackage{NS} xmi:id=\"x\"{attributes}><name content=\"X\"/>\
                 {content}</sacm:AssuranceCasePackage>"
            )
        }),
        (ElementKind::ArgumentPackage, |attributes, content| {
            format!(
                "<sacm:ArgumentPackage{NS} xmi:id=\"x\"{attributes}><name content=\"X\"/>\
                 {content}</sacm:ArgumentPackage>"
            )
        }),
        (ElementKind::Claim, |a, c| typed("Claim", a, c)),
        (ElementKind::ArgumentReasoning, |a, c| {
            typed("ArgumentReasoning", a, c)
        }),
        (ElementKind::ArtifactReference, |a, c| {
            typed("ArtifactReference", a, c)
        }),
        (ElementKind::ArgumentGroup, |a, c| {
            typed("ArgumentGroup", a, c)
        }),
        (ElementKind::AssertedInference, |a, c| {
            typed("AssertedInference", a, c)
        }),
        (ElementKind::AssertedEvidence, |a, c| {
            typed("AssertedEvidence", a, c)
        }),
        (ElementKind::AssertedContext, |a, c| {
            typed("AssertedContext", a, c)
        }),
        (ElementKind::AssertedArtifactSupport, |a, c| {
            typed("AssertedArtifactSupport", a, c)
        }),
        (ElementKind::AssertedArtifactContext, |a, c| {
            typed("AssertedArtifactContext", a, c)
        }),
        (ElementKind::Description, |attributes, content| {
            in_ap(&format!(
                "<description xmi:id=\"x\"{attributes}><content><value content=\"D\"/></content>\
                 {content}</description>"
            ))
        }),
        (
            ElementKind::ImplementationConstraint,
            |attributes, content| {
                in_ap(&format!(
                    "<implementationConstraint xmi:id=\"x\"{attributes}>{content}\
                 </implementationConstraint>"
                ))
            },
        ),
        (ElementKind::Note, |attributes, content| {
            in_ap(&format!("<note xmi:id=\"x\"{attributes}>{content}</note>"))
        }),
        (ElementKind::TaggedValue, |attributes, content| {
            in_ap(&format!(
                "<taggedValue xmi:id=\"x\"{attributes}><key><value content=\"K\"/></key>\
                 {content}</taggedValue>"
            ))
        }),
    ]
}

/// The element-typed composite roles of `kind` (the inventory's "Concrete
/// class -> containment roles" table; the value roles `name`, `content`,
/// `key` and `value` are left out).
fn roles_of(kind: ElementKind) -> Vec<&'static str> {
    let utility = [
        "description",
        "implementationConstraint",
        "note",
        "taggedValue",
    ];
    let own: &[&str] = match kind {
        ElementKind::AssuranceCasePackage => &[
            "assuranceCasePackage",
            "argumentPackage",
            "artifactPackage",
            "terminologyPackage",
        ],
        ElementKind::ArgumentPackage => &["argumentElement"],
        ElementKind::Description
        | ElementKind::ImplementationConstraint
        | ElementKind::Note
        | ElementKind::TaggedValue => return Vec::new(),
        _ => &[],
    };
    own.iter().copied().chain(utility).collect()
}

/// The reference ends of `kind` (ptc/22-03-13): `citedElement` and
/// `abstractForm` of every `SACMElement`, `metaClaim` of every Assertion, the
/// three ends of every `AssertedRelationship`, `structure` of an
/// `ArgumentReasoning`, `referencedArtifactElement` of an `ArtifactReference` and
/// `argumentElement` of an `ArgumentGroup`.
fn ends_of(kind: ElementKind) -> Vec<&'static str> {
    let mut ends = vec!["citedElement", "abstractForm"];
    if kind.is_a(AbstractClass::Assertion) {
        ends.push("metaClaim");
    }
    if kind.is_a(AbstractClass::AssertedRelationship) {
        ends.extend(["reasoning", "source", "target"]);
    }
    match kind {
        ElementKind::ArgumentReasoning => ends.push("structure"),
        ElementKind::ArtifactReference => ends.push("referencedArtifactElement"),
        ElementKind::ArgumentGroup => ends.push("argumentElement"),
        _ => {}
    }
    ends
}

#[test]
fn sacm23_arg_013_no_readable_class_has_a_composite_role_and_a_reference_end_of_the_same_name() {
    let holders = readable_holders();
    assert_eq!(
        holders.len(),
        15,
        "{ROW}: setup: not the 15 kinds this release reads"
    );
    let mut failures = Vec::new();
    for (kind, element_of) in &holders {
        let roles = roles_of(*kind);
        let ends = ends_of(*kind);
        // The tables themselves, from the inventory and ptc/22-03-13.
        for role in &roles {
            if ends.contains(role) {
                failures.push(format!("the tables give {kind} a role and an end {role}"));
            }
        }
        // A role name, as an attribute, is no reference end of the class.
        for role in &roles {
            let (input, marks) = marked(&element_of(&format!(" {MARK}{role}=\"c1\""), ""));
            match read(Mode::Tolerant, &input) {
                Err(error @ ReadError::Unsupported { .. })
                    if error.location() == marks.first().copied() => {}
                other => failures.push(format!(
                    "{role}=\"c1\" on a {kind}: not Unsupported at the attribute: {other:?}"
                )),
            }
        }
        // An end name, as a nested typed element, is no composite role: it
        // is refused (where within it is SACM23-XMI-010's), never read as a
        // child.
        for end in &ends {
            let input = element_of(
                "",
                &format!(
                    "<{end} xmi:type=\"sacm:Claim\" xmi:id=\"z9\"><name content=\"Z\"/></{end}>"
                ),
            );
            match read(Mode::Tolerant, &input) {
                Err(error @ ReadError::Unsupported { .. }) if error.location().is_some() => {}
                other => failures.push(format!(
                    "a nested Claim in {end} of a {kind}: not Unsupported: {other:?}"
                )),
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{ROW}: {} names read both as a role and as an end:\n{}",
        failures.len(),
        failures.join("\n")
    );
    // Both directions of argumentElement read.
    let what = "argumentElement as a reference on a group and a role on a package";
    let doc = read_ok(what, &g_in_ap(" argumentElement=\"c1\"", ""));
    assert_eq!(
        (members(&doc, "g"), children(&doc, "p")),
        (ids(&["c1"]), ids(&["c1", "c2", "g"])),
        "{ROW}: {what}"
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
fn sacm23_arg_013_the_fixture_and_the_golden_file_read_their_group_and_round_trip() {
    for (what, path, members_of_g1, package) in [
        ("the fixture", FIXTURE, ["c1", "report", "e1"], "arguments"),
        ("the golden file", GOLDEN, ["c1", "a1", "e1"], "arguments"),
    ] {
        let text = std::fs::read_to_string(path)
            .unwrap_or_else(|error| panic!("{ROW}: cannot read {path}: {error}"));
        let doc = read_ok(what, &text);
        assert_eq!(
            (element(&doc, "g1").kind(), members(&doc, "g1")),
            (ElementKind::ArgumentGroup, ids(&members_of_g1)),
            "{ROW}: {what}: g1"
        );
        for member in members_of_g1 {
            assert_eq!(
                element(&doc, member)
                    .parent()
                    .map(|parent| parent.id().as_str()),
                Some(package),
                "{ROW}: {what}: the member {member} is not a child of {package}"
            );
        }
        assert_eq!(
            validation::validate(&doc),
            [],
            "{ROW}: {what} does not validate cleanly"
        );
        let written = round_trip(what, &doc, Mode::Strict);
        if path == GOLDEN {
            assert_eq!(
                written, text,
                "{ROW}: {what} is not written to its own bytes"
            );
        }
    }
}
