//! Tests for `ArtifactGroup` and its member end `artifactElement`
//! (SACM23-ART-011; formal/23-05-08 §12.3 `artifactElement:ArtifactElement[0..*]`,
//! no composition, `ArtifactGroups` "serve no structural purpose"; ptc/22-03-13
//! `Artifact::ArtifactGroup` (`artifactElement`, no aggregation); ADR 0026,
//! ADR 0027, ADR 0028, ADR 0033, ADR 0034).
//!
//! An `ArtifactGroup` is read typed in an ArtifactPackage's `artifactElement`.
//! Its own `artifactElement` is a multi-valued reference end, not
//! containment: an IDREFS attribute, or elements carrying `xmi:idref` or
//! `href="#id"`, read as SACM23-ARG-013 reads an `ArgumentGroup`'s
//! `argumentElement`. The parent's kind decides what `artifactElement` is: the
//! member end under an `ArtifactGroup`, the composite role under an
//! ArtifactPackage, and elsewhere an attribute or role the class lacks,
//! preserved (ADR 0033). Members stay children of their own package. The end
//! is validated (SACM-REF-001, SACM-REF-002 with clause 12.3, SACM-MULT-001
//! for a repeat), written as an attribute after `abstractForm`, and handled
//! by the delete policies; it has no lower bound, so a scrub keeps the group.
//!
//! The edits through `ReferenceEnd::ArtifactGroupMember`,
//! `Difference::ArtifactGroupMembers` and `CreateArtifactGroup` are tested in
//! `artifact_edits.rs`, with the other edits of slice 0052.
//!
//! Expected locations are marked in the input with [`MARK`] (U+2038), which
//! the test removes before reading. Inputs end lines with LF only.

use sacm::compare::semantic_compare;
use sacm::diagnostic::{Code, Diagnostic, Severity, SourceLocation};
use sacm::edit::{
    self, CrossPackageReferencePolicy, DeleteElement, EditError, Mutation, Operation,
    ReferenceDeletePolicy, SetReferences,
};
use sacm::model::{Document, ElementId, ElementKind, ReferenceEnd, SacmElement};
use sacm::validation;
use sacm::xmi::{self, Mode, ReadError, ReadOptions, ReadOutput, WriteError, WriteOptions};

mod common;

const ROW: &str = "SACM23-ART-011";

/// Both modes, strict first.
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
const END: &str = "artifactElement";

/// The worked fixture of slice 0052.
const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/sacm23/xmi/artifact-package-valid.sacm.xmi"
);

// --- Inputs ------------------------------------------------------------------------

/// A typed `artifactElement` child `id` of class `class`, named, holding
/// `content`.
fn typed(class: &str, id: &str, content: &str) -> String {
    format!(
        "<artifactElement xmi:type=\"sacm:{class}\" xmi:id=\"{id}\"><name content=\"{id}\"/>\
         {content}</artifactElement>"
    )
}

/// The `ArtifactGroup` `id` with `attributes`, holding `content` after its
/// name.
fn group(id: &str, attributes: &str, content: &str) -> String {
    format!(
        "<artifactElement xmi:type=\"sacm:ArtifactGroup\" xmi:id=\"{id}\"{attributes}>\
         <name content=\"G\"/>{content}</artifactElement>"
    )
}

/// The AssuranceCasePackage root `case` holding the `ArtifactPackage`s `a` and
/// `b`. `a` holds the Technique `t1` (with the Description `d1`), the
/// Participant `pa1` and the Property `pr1`, then `body_a`; `b` holds
/// `body_b`. Each on its own line.
fn in_case(body_a: &str, body_b: &str) -> String {
    format!(
        "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\">\n  <name content=\"Case\"/>\n  \
         <artifactPackage xmi:id=\"a\">\n    <name content=\"A\"/>\n    {}\n    {}\n    {}\n    \
         {body_a}\n  </artifactPackage>\n  <artifactPackage xmi:id=\"b\">\n    \
         <name content=\"B\"/>\n    {body_b}\n  </artifactPackage>\n</sacm:AssuranceCasePackage>",
        typed(
            "Technique",
            "t1",
            "<description xmi:id=\"d1\"><content><value content=\"D\"/></content></description>"
        ),
        typed("Participant", "pa1", ""),
        typed("Property", "pr1", "")
    )
}

/// The `ArtifactGroup` `g` in `a` of [`in_case`].
fn g_in(attributes: &str, content: &str) -> String {
    in_case(&group("g", attributes, content), "")
}

/// An ArgumentPackage root `p` holding the Claims c1 and c2 and the
/// `ArgumentGroup` `ag` with `attributes` and `content`.
fn argument_group(attributes: &str, content: &str) -> String {
    format!(
        "<sacm:ArgumentPackage{NS} xmi:id=\"p\">\n  <name content=\"P\"/>\n  \
         <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"><name content=\"C1\"/></argumentElement>\n  \
         <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c2\"><name content=\"C2\"/></argumentElement>\n  \
         <argumentElement xmi:type=\"sacm:ArgumentGroup\" xmi:id=\"ag\"{attributes}><name content=\"AG\"/>\
         {content}</argumentElement>\n</sacm:ArgumentPackage>"
    )
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

// --- Reading, writing and validating --------------------------------------------------

fn read(mode: Mode, input: &str) -> Result<ReadOutput, ReadError> {
    xmi::read(input.as_bytes(), &ReadOptions::default().with_mode(mode))
}

/// Reads `input` in both modes, which the test expects to read with no
/// diagnostic to `==` documents, and returns the strict one.
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

/// Reads `input` in strict mode, which the test expects to refuse it, and
/// returns the refusal. A tolerant read keeps what a strict read refuses
/// here (LIB-COMPAT-001, ADR 0035).
fn refused(what: &str, input: &str) -> ReadError {
    match read(Mode::Strict, input) {
        Ok(output) => panic!(
            "{ROW}: {what}: a strict read returned a document of {} elements for {input:?}",
            output.document().element_count()
        ),
        Err(error) => error,
    }
}

/// Asserts that `template`, without its one mark, reads in both modes and
/// keeps one construct as preserved content of `holder`: exactly one
/// SACM-XMI-003 at the mark, affecting `holder`, a Warning in tolerant mode
/// and an Error in strict mode, and the same document in both. Returns the
/// strict document.
fn assert_preserved_at(what: &str, template: &str, holder: &str) -> Document {
    let (input, marks) = marked(template);
    let [at] = marks.as_slice() else {
        panic!("{ROW}: setup: {what} has not one mark");
    };
    let mut documents = Vec::new();
    for (mode, severity) in [
        (Mode::Strict, Severity::Error),
        (Mode::Tolerant, Severity::Warning),
    ] {
        let output = match read(mode, &input) {
            Ok(output) => output,
            Err(error) => panic!(
                "{ROW}: {what}: a {mode:?} read refused it instead of preserving it: {error:?}"
            ),
        };
        let found: Vec<_> = output
            .diagnostics()
            .iter()
            .map(|found| {
                (
                    found.code(),
                    found.severity(),
                    found.location(),
                    found.affected().to_vec(),
                )
            })
            .collect();
        assert_eq!(
            found,
            [(
                Code::XMI_UNKNOWN_CONTENT,
                severity,
                Some(*at),
                vec![ElementId::from(holder)]
            )],
            "{ROW}: {what}: not one SACM-XMI-003 at the construct affecting {holder:?} in \
             {mode:?} mode"
        );
        let doc = output.into_document();
        assert!(
            doc.get(holder)
                .is_some_and(SacmElement::has_preserved_content),
            "{ROW}: {what}: {holder:?} holds no preserved content in {mode:?} mode"
        );
        documents.push(doc);
    }
    assert_eq!(
        documents[0], documents[1],
        "{ROW}: {what}: the modes read different documents"
    );
    documents.swap_remove(0)
}

fn write_in(what: &str, doc: &Document, mode: Mode) -> String {
    match xmi::write(doc, &WriteOptions::default().with_mode(mode)) {
        Ok(text) => text,
        Err(error) => panic!("{ROW}: {what}: a {mode:?} write refused the document: {error:?}"),
    }
}

/// Writes `doc` in `mode`, reads the output back and writes that again: the
/// read-back must be `==` and the second write must give the same bytes.
fn round_trip(what: &str, doc: &Document, mode: Mode) -> String {
    let text = write_in(what, doc, mode);
    let back = read_ok(&format!("{what}, written in {mode:?} mode"), &text);
    assert_eq!(
        semantic_compare(doc, &back),
        [],
        "{ROW}: {what}: the output does not read back equal\n{text}"
    );
    assert!(back == *doc, "{ROW}: {what}: the output reads back !=");
    assert_eq!(
        write_in(what, &back, mode),
        text,
        "{ROW}: {what}: writing what was read back gives other bytes"
    );
    text
}

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

fn line_with<'t>(text: &'t str, id: &str) -> &'t str {
    let attribute = format!("xmi:id=\"{id}\"");
    let Some(line) = text.lines().find(|line| line.contains(&attribute)) else {
        panic!("{ROW}: the output has no line with {attribute}:\n{text}");
    };
    line.trim()
}

fn with_code(doc: &Document, code: Code) -> Vec<Diagnostic> {
    validation::validate(doc)
        .into_iter()
        .filter(|found| found.code() == code)
        .collect()
}

/// Applies `operation`, which the test expects to succeed.
fn applied(doc: &mut Document, operation: impl Into<Operation>) -> Mutation {
    let operation = operation.into();
    match edit::apply(doc, operation.clone()) {
        Ok(mutation) => mutation,
        Err(error) => panic!("{ROW}: {operation:?} was refused: {error} ({error:?})"),
    }
}

/// Applies `operation`, which the test expects to be refused leaving `doc`
/// `==` to before, and returns why.
fn refused_edit(doc: &mut Document, operation: impl Into<Operation>) -> EditError {
    let operation = operation.into();
    let before = doc.clone();
    let error = match edit::apply(doc, operation.clone()) {
        Ok(mutation) => panic!("{ROW}: {operation:?} was accepted: {mutation:?}"),
        Err(error) => error,
    };
    assert!(
        *doc == before,
        "{ROW}: the refused {operation:?} changed the document"
    );
    error
}

// =====================================================================================
// Item 1: the reference forms, RepeatedValue, an empty attribute, a nested element
// =====================================================================================

#[test]
fn sacm23_art_011_the_attribute_and_the_element_forms_read_equal_and_members_stay_children() {
    let what = "artifactElement=\"t1 pa1\"";
    let attribute = read_ok(what, &g_in(" artifactElement=\"t1 pa1\"", ""));
    assert_eq!(
        (
            element(&attribute, "g").kind(),
            members(&attribute, "g"),
            children(&attribute, "g"),
            children(&attribute, "a")
        ),
        (
            ElementKind::ArtifactGroup,
            ids(&["t1", "pa1"]),
            Vec::new(),
            ids(&["t1", "pa1", "pr1", "g"])
        ),
        "{ROW}: {what}: not an ArtifactGroup with the members [t1, pa1] and no children, \
         its members children of a"
    );
    for member in ["t1", "pa1"] {
        assert_eq!(
            element(&attribute, member)
                .parent()
                .map(|parent| parent.id().as_str()),
            Some("a"),
            "{ROW}: {what}: {member} moved into the group"
        );
    }
    for (what, content) in [
        (
            "xmi:idref then href (the fixture's form)",
            "<artifactElement xmi:idref=\"t1\"/><artifactElement href=\"#pa1\"/>",
        ),
        (
            "two xmi:idref elements",
            "<artifactElement xmi:idref=\"t1\"/><artifactElement xmi:idref=\"pa1\"/>",
        ),
        (
            "two href elements",
            "<artifactElement href=\"#t1\"/><artifactElement href=\"#pa1\"/>",
        ),
    ] {
        let doc = read_ok(what, &g_in("", content));
        assert_eq!(
            (members(&doc, "g"), children(&doc, "g")),
            (ids(&["t1", "pa1"]), Vec::new()),
            "{ROW}: {what}: not read as the members [t1, pa1]"
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
    // White space separates the ids, in order, repeats kept.
    let what = "artifactElement=\" pa1&#9;t1  pa1 \"";
    let doc = read_ok(what, &g_in(" artifactElement=\" pa1&#9;t1  pa1 \"", ""));
    assert_eq!(
        members(&doc, "g"),
        ids(&["pa1", "t1", "pa1"]),
        "{ROW}: {what}: not read in order with the repeat"
    );
}

#[test]
fn sacm23_art_011_an_attribute_and_member_elements_are_repeated_value_at_both_occurrences() {
    let what = "the attribute and a member element";
    let (input, marks) = marked(&g_in(
        &format!(" {MARK}artifactElement=\"t1\""),
        &format!("{MARK}<artifactElement xmi:idref=\"pa1\"/>"),
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
            Code::MULTIPLICITY_VIOLATION,
            marks.as_slice(),
            marks.get(1).copied()
        ),
        "{ROW}: {what}: not SACM-MULT-001 at both occurrences"
    );
}

#[test]
fn sacm23_art_011_an_empty_or_qualified_attribute_is_preserved_by_the_group() {
    for (what, attribute) in [
        ("an empty attribute", format!(" {MARK}artifactElement=\"\"")),
        (
            "an attribute of only white space",
            format!(" {MARK}artifactElement=\" \""),
        ),
        (
            "sacm:artifactElement",
            format!(" {MARK}sacm:artifactElement=\"t1\""),
        ),
    ] {
        let doc = assert_preserved_at(what, &g_in(&attribute, ""), "g");
        assert_eq!(
            members(&doc, "g"),
            Vec::<String>::new(),
            "{ROW}: {what}: read as members"
        );
    }
}

/// A group's `artifactElement` is a reference end, so an `artifactElement`
/// element in a group is a reference element. A nested element there is
/// refused at its first attribute that no reference element has (SACM23-XMI-010's
/// "an xmi:type on a reference element", "an xmi:id on a reference
/// element"), conforming XMI this version does not read yet
/// (SACM23-XMI-019). One with no link and only a name is a defect, kept whole
/// as preserved content of the group (ADR 0033). Never read as a child; the
/// same as SACM23-ARG-013 for an `ArgumentGroup`.
#[test]
fn sacm23_art_011_a_nested_element_under_a_group_is_unsupported_or_preserved_at_that_construct() {
    let label = "(a limit of this version; SACM23-XMI-019 reads it)";
    for (what, nested) in [
        (
            "a typed Technique",
            format!(
                "<artifactElement {MARK}xmi:type=\"sacm:Technique\" xmi:id=\"t9\">\
                 <name content=\"T9\"/></artifactElement>"
            ),
        ),
        (
            "a typed Property with no content",
            format!("<artifactElement {MARK}xmi:type=\"sacm:Property\" xmi:id=\"p9\"/>"),
        ),
        (
            "a typed ArtifactGroup",
            format!("<artifactElement {MARK}xmi:type=\"sacm:ArtifactGroup\" xmi:id=\"g9\"/>"),
        ),
        (
            "an element by xsi:type",
            format!("<artifactElement {MARK}xsi:type=\"sacm:Technique\" xmi:id=\"t9\"/>"),
        ),
        (
            "an untyped element with an id",
            format!("<artifactElement {MARK}xmi:id=\"t9\"/>"),
        ),
        (
            "a typed Technique after a member element",
            format!(
                "<artifactElement xmi:idref=\"t1\"/>\
                 <artifactElement {MARK}xmi:type=\"sacm:Technique\" xmi:id=\"t9\"/>"
            ),
        ),
    ] {
        let what = format!("{what} inside an ArtifactGroup");
        let (input, marks) = marked(&g_in("", &nested));
        let error = refused(&what, &input);
        assert!(
            matches!(
                &error,
                ReadError::Unsupported { location, reason, .. }
                    if *location == Some(marks[0]) && reason.ends_with(label)
            ) && error.code().as_str() == crate::common::unsupported_code(&error.to_string()),
            "{ROW}: {what}: not Unsupported at {} naming SACM23-XMI-019: {error:?}",
            marks[0]
        );
    }
    let what = "an element with only a name inside an ArtifactGroup";
    let doc = assert_preserved_at(
        what,
        &g_in(
            "",
            &format!("{MARK}<artifactElement><name content=\"X9\"/></artifactElement>"),
        ),
        "g",
    );
    assert_eq!(
        (members(&doc, "g"), children(&doc, "g")),
        (Vec::<String>::new(), Vec::new()),
        "{ROW}: {what}: read as a member or a child"
    );
}

#[test]
fn sacm23_art_011_each_group_kind_keeps_the_other_group_s_end_as_preserved_content() {
    // `argumentElement` is no end of an ArtifactGroup, and `artifactElement`
    // is none of an ArgumentGroup: attributes or roles the class lacks,
    // preserved (ADR 0033), even in reference form.
    let artifact_cases = [
        (
            "an argumentElement attribute on an ArtifactGroup",
            g_in(&format!(" {MARK}argumentElement=\"t1\""), ""),
        ),
        (
            "an argumentElement reference element in an ArtifactGroup",
            g_in("", &format!("{MARK}<argumentElement xmi:idref=\"t1\"/>")),
        ),
        (
            "an argumentElement href element in an ArtifactGroup",
            g_in("", &format!("{MARK}<argumentElement href=\"#t1\"/>")),
        ),
    ];
    for (what, template) in &artifact_cases {
        let doc = assert_preserved_at(what, template, "g");
        assert_eq!(
            members(&doc, "g"),
            Vec::<String>::new(),
            "{ROW}: {what}: read as members"
        );
    }
    let argument_cases = [
        (
            "an artifactElement attribute on an ArgumentGroup",
            argument_group(&format!(" {MARK}artifactElement=\"c1\""), ""),
        ),
        (
            "an artifactElement reference element in an ArgumentGroup",
            argument_group("", &format!("{MARK}<artifactElement xmi:idref=\"c1\"/>")),
        ),
    ];
    for (what, template) in &argument_cases {
        let doc = assert_preserved_at(what, template, "ag");
        assert_eq!(
            members(&doc, "ag"),
            Vec::<String>::new(),
            "{ROW}: {what}: read as members"
        );
    }
}

// =====================================================================================
// Item 2: group_members() for both group kinds
// =====================================================================================

#[test]
fn sacm23_art_011_group_members_answers_for_both_group_kinds_and_is_empty_for_others() {
    let doc = read_ok(
        "an ArtifactGroup with a repeat",
        &in_case(
            &group("g", " artifactElement=\"pa1 t1 pa1 g a\"", ""),
            &group("h", "", ""),
        ),
    );
    assert_eq!(
        (members(&doc, "g"), members(&doc, "h")),
        (ids(&["pa1", "t1", "pa1", "g", "a"]), Vec::new()),
        "{ROW}: the ArtifactGroup members in order with repeats, and none"
    );
    for other in ["case", "a", "b", "t1", "pa1", "pr1", "d1"] {
        assert_eq!(
            members(&doc, other),
            Vec::<String>::new(),
            "{ROW}: {other} is no group but has members"
        );
    }
    let doc = read_ok(
        "an ArgumentGroup with a repeat",
        &argument_group(" argumentElement=\"c2 c1 c2\"", ""),
    );
    assert_eq!(
        (members(&doc, "ag"), members(&doc, "c1"), members(&doc, "p")),
        (ids(&["c2", "c1", "c2"]), Vec::new(), Vec::new()),
        "{ROW}: the ArgumentGroup members, and none for a Claim or a package"
    );
}

#[test]
fn sacm23_art_011_set_references_with_group_member_on_an_artifact_group_is_feature_not_applicable()
{
    // `GroupMember` is the ArgumentGroup's end (§11.2): an ArtifactGroup does
    // not have it (SACM-CMD-008).
    let mut doc = read_ok("a group", &g_in(" artifactElement=\"t1\"", ""));
    let error = refused_edit(
        &mut doc,
        SetReferences::new("g", ReferenceEnd::GroupMember, ["pa1"]),
    );
    assert!(
        matches!(
            &error,
            EditError::FeatureNotApplicable { element, kind, feature, .. }
                if element.as_str() == "g" && *kind == ElementKind::ArtifactGroup
                    && *feature == "argumentElement"
        ),
        "{ROW}: GroupMember on an ArtifactGroup: {error:?}"
    );
    assert_eq!(
        error.code(),
        Code::CMD_FEATURE_NOT_APPLICABLE,
        "{ROW}: GroupMember on an ArtifactGroup is not SACM-CMD-008"
    );
}

// =====================================================================================
// Item 4: validation
// =====================================================================================

/// `(code, severity, requirement, clause, location, affected)` of `found`.
type Finding = (
    Code,
    Severity,
    String,
    Option<String>,
    Option<SourceLocation>,
    Vec<ElementId>,
);

fn finding(found: &Diagnostic) -> Finding {
    (
        found.code(),
        found.severity(),
        found.requirement_id().to_owned(),
        found.clause().map(str::to_owned),
        found.location(),
        found.affected().to_vec(),
    )
}

#[test]
fn sacm23_art_011_every_artifact_element_member_validates_and_a_utility_element_is_ref_002() {
    // Every kind but the four UtilityElements is an ArtifactElement, so any of
    // them is a member without a finding: the assets, a package, the group
    // itself, another group, and the AssuranceCasePackage.
    let what = "members of every ArtifactElement kind the document holds";
    let doc = read_ok(
        what,
        &in_case(
            &group("g", " artifactElement=\"t1 pa1 pr1 a b g h case\"", ""),
            &group("h", " artifactElement=\"t1\"", ""),
        ),
    );
    assert_eq!(
        validation::validate(&doc),
        [],
        "{ROW}: {what}: validate reports a finding"
    );
    round_trip(what, &doc, Mode::Strict);
    // A Description member: one SACM-REF-002 with clause 12.3.
    let what = "artifactElement=\"t1 d1\"";
    let doc = read_ok(what, &g_in(" artifactElement=\"t1 d1\"", ""));
    let found: Vec<Finding> = validation::validate(&doc).iter().map(finding).collect();
    assert_eq!(
        found,
        [(
            Code::REF_WRONG_TYPE,
            Severity::Error,
            "SACM23-VAL-004".to_owned(),
            Some("12.3".to_owned()),
            None,
            list(&["g", "d1"])
        )],
        "{ROW}: {what}: not one SACM-REF-002 Error with clause 12.3 on [g, d1]"
    );
    // An IDREF is untyped, so the schema check does not refuse it (ADR 0035
    // D4): a strict write writes it as a tolerant one does.
    let text = write_in(what, &doc, Mode::Tolerant);
    assert!(
        line_with(&text, "g").ends_with(" artifactElement=\"t1 d1\">"),
        "{ROW}: {what}: a tolerant write does not keep the member\n{text}"
    );
    assert_eq!(
        write_in(what, &doc, Mode::Strict),
        text,
        "{ROW}: {what}: the strict write is not the tolerant one"
    );
}

#[test]
fn sacm23_art_011_an_absent_member_is_ref_001_and_a_repeat_is_one_mult_001_warning() {
    let what = "artifactElement=\"t1 ghost\"";
    let doc = read_ok(what, &g_in(" artifactElement=\"t1 ghost\"", ""));
    let found: Vec<Finding> = validation::validate(&doc).iter().map(finding).collect();
    assert_eq!(
        found,
        [(
            Code::REF_DANGLING,
            Severity::Error,
            "SACM23-VAL-003".to_owned(),
            Some("12.3".to_owned()),
            None,
            list(&["g", "ghost"])
        )],
        "{ROW}: {what}: not one SACM-REF-001 Error with clause 12.3 on [g, ghost]"
    );
    let error = match xmi::write(&doc, &WriteOptions::default().with_mode(Mode::Strict)) {
        Ok(text) => panic!("{ROW}: {what}: a strict write accepted it:\n{text}"),
        Err(error) => error,
    };
    assert!(
        matches!(error, WriteError::Invalid { .. }),
        "{ROW}: {what}: a strict write is not Invalid: {error:?}"
    );
    let text = round_trip(what, &doc, Mode::Tolerant);
    assert!(
        line_with(&text, "g").ends_with(" artifactElement=\"t1 ghost\">"),
        "{ROW}: {what}: the dangling id is not written back\n{text}"
    );
    // A repeat: one Warning per repeated id, and a strict write succeeds.
    let what = "artifactElement=\"t1 pa1 t1 t1\"";
    let doc = read_ok(what, &g_in(" artifactElement=\"t1 pa1 t1 t1\"", ""));
    let found: Vec<Finding> = with_code(&doc, Code::MULTIPLICITY_VIOLATION)
        .iter()
        .map(finding)
        .collect();
    assert_eq!(
        found,
        [(
            Code::MULTIPLICITY_VIOLATION,
            Severity::Warning,
            "SACM23-VAL-022".to_owned(),
            Some("12.3".to_owned()),
            None,
            list(&["g", "t1"])
        )],
        "{ROW}: {what}: not one SACM-MULT-001 Warning with clause 12.3 on [g, t1]"
    );
    assert_eq!(
        validation::validate(&doc).len(),
        1,
        "{ROW}: {what}: validate reports more than the repeat"
    );
    let text = round_trip(what, &doc, Mode::Strict);
    assert!(
        line_with(&text, "g").ends_with(" artifactElement=\"t1 pa1 t1 t1\">"),
        "{ROW}: {what}: the repeats are not written back\n{text}"
    );
}

/// The fixture with `from` replaced by `to`, which must change it.
fn fixture_with(from: &str, to: &str) -> String {
    let text = std::fs::read_to_string(FIXTURE)
        .unwrap_or_else(|error| panic!("{ROW}: cannot read {FIXTURE}: {error}"));
    assert!(
        text.contains(from),
        "{ROW}: setup: the fixture has no {from:?}"
    );
    text.replacen(from, to, 1)
}

#[test]
fn sacm23_art_011_fixture_variants_d_e_and_f_validate_as_the_brief_gives() {
    let members = "        <artifactElement xmi:idref=\"fta\"/>\n        \
                   <artifactElement href=\"#assessor\"/>\n";
    let analysis = "<artifactElement xmi:type=\"sacm:ArtifactGroup\" xmi:id=\"analysis\">";
    let with_members = |value: &str| {
        let text = fixture_with(members, "");
        assert!(text.contains(analysis), "{ROW}: setup: no analysis");
        text.replacen(
            analysis,
            &format!(
                "<artifactElement xmi:type=\"sacm:ArtifactGroup\" xmi:id=\"analysis\" \
                 artifactElement=\"{value}\">"
            ),
            1,
        )
    };
    // (d) A Description of fta as a member.
    let what = "variant (d): artifactElement=\"fta d1\"";
    let text = with_members("fta d1").replacen(
        "        <name lang=\"en\" content=\"Fault tree analysis\"/>\n",
        "        <name lang=\"en\" content=\"Fault tree analysis\"/>\n        \
         <description xmi:id=\"d1\"><content><value content=\"D\"/></content></description>\n",
        1,
    );
    let doc = read_ok(what, &text);
    let found: Vec<Finding> = validation::validate(&doc).iter().map(finding).collect();
    assert_eq!(
        found,
        [(
            Code::REF_WRONG_TYPE,
            Severity::Error,
            "SACM23-VAL-004".to_owned(),
            Some("12.3".to_owned()),
            None,
            list(&["analysis", "d1"])
        )],
        "{ROW}: {what}"
    );
    // SACM-REF-002 is no schema error (ADR 0035 D4): a strict write writes
    // it as a tolerant one does.
    let text = write_in(what, &doc, Mode::Tolerant);
    assert_eq!(
        write_in(what, &doc, Mode::Strict),
        text,
        "{ROW}: {what}: the strict write is not the tolerant one"
    );
    assert!(
        line_with(&text, "analysis").contains(" artifactElement=\"fta d1\""),
        "{ROW}: {what}: a tolerant write does not keep the attribute"
    );
    // (e) An absent member.
    let what = "variant (e): artifactElement=\"ghost\"";
    let doc = read_ok(what, &with_members("ghost"));
    let found: Vec<Finding> = validation::validate(&doc).iter().map(finding).collect();
    assert_eq!(
        found,
        [(
            Code::REF_DANGLING,
            Severity::Error,
            "SACM23-VAL-003".to_owned(),
            Some("12.3".to_owned()),
            None,
            list(&["analysis", "ghost"])
        )],
        "{ROW}: {what}"
    );
    // (f) A repeat.
    let what = "variant (f): artifactElement=\"fta fta\"";
    let doc = read_ok(what, &with_members("fta fta"));
    let found: Vec<Finding> = validation::validate(&doc).iter().map(finding).collect();
    assert_eq!(
        found,
        [(
            Code::MULTIPLICITY_VIOLATION,
            Severity::Warning,
            "SACM23-VAL-022".to_owned(),
            Some("12.3".to_owned()),
            None,
            list(&["analysis", "fta"])
        )],
        "{ROW}: {what}"
    );
    write_in(what, &doc, Mode::Strict);
}

// =====================================================================================
// Item 5: writing
// =====================================================================================

#[test]
fn sacm23_art_011_members_are_written_after_abstract_form_kept_when_repeated_and_absent_when_empty()
{
    let what = "every SACMElement feature and the end";
    let doc = read_ok(
        what,
        &in_case(
            &format!(
                "{}\n    {}",
                group(
                    "g",
                    " artifactElement=\"pa1 h t1\" abstractForm=\"h\" citedElement=\"h\" \
                     gid=\"group-g\" isCitation=\"true\"",
                    ""
                ),
                group("h", " isAbstract=\"true\"", "")
            ),
            "",
        ),
    );
    assert_eq!(
        validation::validate(&doc),
        [],
        "{ROW}: {what}: setup: the document does not validate cleanly"
    );
    let text = round_trip(what, &doc, Mode::Strict);
    assert_eq!(
        line_with(&text, "g"),
        "<artifactElement xmi:type=\"sacm:ArtifactGroup\" xmi:id=\"g\" gid=\"group-g\" \
         isCitation=\"true\" citedElement=\"h\" abstractForm=\"h\" artifactElement=\"pa1 h t1\">",
        "{ROW}: {what}: not written in the order ... abstractForm artifactElement\n{text}"
    );
    let what = "artifactElement=\"t1 t1\"";
    let doc = read_ok(what, &g_in(" artifactElement=\"t1 t1\"", ""));
    for mode in MODES {
        let text = round_trip(what, &doc, mode);
        assert_eq!(
            line_with(&text, "g"),
            "<artifactElement xmi:type=\"sacm:ArtifactGroup\" xmi:id=\"g\" \
             artifactElement=\"t1 t1\">",
            "{ROW}: {what}: not written back unchanged in {mode:?} mode\n{text}"
        );
    }
    let what = "member elements";
    let doc = read_ok(
        what,
        &g_in(
            "",
            "<artifactElement xmi:idref=\"pa1\"/><artifactElement href=\"#t1\"/>",
        ),
    );
    let text = round_trip(what, &doc, Mode::Strict);
    assert_eq!(
        line_with(&text, "g"),
        "<artifactElement xmi:type=\"sacm:ArtifactGroup\" xmi:id=\"g\" artifactElement=\"pa1 t1\">",
        "{ROW}: {what}: not written as one attribute\n{text}"
    );
    let what = "a group with no members";
    let doc = read_ok(what, &g_in("", ""));
    let text = round_trip(what, &doc, Mode::Strict);
    assert_eq!(
        line_with(&text, "g"),
        "<artifactElement xmi:type=\"sacm:ArtifactGroup\" xmi:id=\"g\">",
        "{ROW}: {what}: the empty end is written\n{text}"
    );
}

// =====================================================================================
// Item 7: the delete policies
// =====================================================================================

#[test]
fn sacm23_art_011_deleting_a_member_is_referenced_or_cross_package_reference_by_default() {
    // Inside the package: Referenced (SACM-CMD-002).
    let mut doc = read_ok("a group in a", &g_in(" artifactElement=\"t1 pa1\"", ""));
    let error = refused_edit(&mut doc, DeleteElement::new("t1"));
    assert!(
        matches!(
            &error,
            EditError::Referenced { holder, end, id, .. }
                if holder.as_str() == "g" && *end == END && id.as_str() == "t1"
        ),
        "{ROW}: deleting the member t1: not Referenced {{ g, {END}, t1 }}: {error:?}"
    );
    assert_eq!(
        error.code(),
        Code::CMD_DELETE_REFERENCED,
        "{ROW}: deleting the member t1: not SACM-CMD-002"
    );
    // From another package: CrossPackageReference (SACM-CMD-007).
    let mut doc = read_ok(
        "a group in b",
        &in_case("", &group("h", " artifactElement=\"t1\"", "")),
    );
    let error = refused_edit(&mut doc, DeleteElement::new("t1"));
    assert!(
        matches!(
            &error,
            EditError::CrossPackageReference { holder, end, id, .. }
                if holder.as_str() == "h" && *end == END && id.as_str() == "t1"
        ),
        "{ROW}: deleting t1, a member of h in b: not CrossPackageReference {{ h, {END}, t1 }}: \
         {error:?}"
    );
    assert_eq!(
        error.code(),
        Code::CMD_EXTERNAL_REFERENCES,
        "{ROW}: deleting t1 across packages: not SACM-CMD-007"
    );
    // The preview lists the reference, and whether it crosses a package.
    for (what, input, holder, crosses) in [
        ("in a", g_in(" artifactElement=\"t1\"", ""), "g", false),
        (
            "in b",
            in_case("", &group("h", " artifactElement=\"t1\"", "")),
            "h",
            true,
        ),
    ] {
        let doc = read_ok(what, &input);
        let delete = Operation::from(
            DeleteElement::new("t1")
                .with_reference_policy(ReferenceDeletePolicy::ScrubReferences)
                .with_cross_package_policy(CrossPackageReferencePolicy::ApplyReferencePolicy),
        );
        let preview = match edit::preview(&doc, &delete) {
            Ok(preview) => preview,
            Err(error) => panic!("{ROW}: {what}: the preview was refused: {error}"),
        };
        let listed: Vec<_> = preview
            .dangling_references()
            .iter()
            .map(|dangling| {
                (
                    dangling.holder().as_str().to_owned(),
                    dangling.end().name(),
                    dangling.id().as_str().to_owned(),
                    dangling.crosses_package(),
                )
            })
            .collect();
        assert_eq!(
            listed,
            [(holder.to_owned(), END, "t1".to_owned(), crosses)],
            "{ROW}: a group {what}: the preview's dangling references"
        );
    }
}

#[test]
fn sacm23_art_011_scrub_and_delete_referencing_relationships_remove_the_member_and_keep_the_group()
{
    for policy in [
        ReferenceDeletePolicy::ScrubReferences,
        ReferenceDeletePolicy::DeleteReferencingRelationships,
    ] {
        let mut doc = read_ok("a group in a", &g_in(" artifactElement=\"t1 pa1 t1\"", ""));
        let mutation = applied(
            &mut doc,
            DeleteElement::new("t1").with_reference_policy(policy),
        );
        assert_eq!(
            (
                mutation.deleted_ids(),
                mutation.changed_ids(),
                members(&doc, "g")
            ),
            (
                list(&["t1", "d1"]).as_slice(),
                list(&["g"]).as_slice(),
                ids(&["pa1"])
            ),
            "{ROW}: {policy:?}: deleting t1 does not remove both its ids from g"
        );
        // The last member: the group survives, empty.
        let mutation = applied(
            &mut doc,
            DeleteElement::new("pa1").with_reference_policy(policy),
        );
        assert_eq!(
            (
                mutation.deleted_ids(),
                mutation.changed_ids(),
                members(&doc, "g"),
                children(&doc, "a")
            ),
            (
                list(&["pa1"]).as_slice(),
                list(&["g"]).as_slice(),
                Vec::new(),
                ids(&["pr1", "g"])
            ),
            "{ROW}: {policy:?}: the group does not survive losing its last member"
        );
        let text = round_trip("the emptied group", &doc, Mode::Strict);
        assert_eq!(
            line_with(&text, "g"),
            "<artifactElement xmi:type=\"sacm:ArtifactGroup\" xmi:id=\"g\">",
            "{ROW}: {policy:?}: the emptied group is not written without the end\n{text}"
        );
        // Across packages, when the cross-package policy applies the policy.
        let mut doc = read_ok(
            "a group in b",
            &in_case("", &group("h", " artifactElement=\"t1\"", "")),
        );
        let mutation = applied(
            &mut doc,
            DeleteElement::new("t1")
                .with_reference_policy(policy)
                .with_cross_package_policy(CrossPackageReferencePolicy::ApplyReferencePolicy),
        );
        assert_eq!(
            (mutation.changed_ids(), members(&doc, "h")),
            (list(&["h"]).as_slice(), Vec::new()),
            "{ROW}: {policy:?}: deleting t1 across packages does not scrub h"
        );
    }
    // Deleting the group leaves its members where they are.
    let mut doc = read_ok("a group in a", &g_in(" artifactElement=\"t1 pa1\"", ""));
    let mutation = applied(&mut doc, DeleteElement::new("g"));
    assert_eq!(
        (mutation.deleted_ids(), children(&doc, "a")),
        (list(&["g"]).as_slice(), ids(&["t1", "pa1", "pr1"])),
        "{ROW}: deleting the group deletes a member"
    );
}

// =====================================================================================
// The role/end collision test, for the classes slice 0052 reads
// =====================================================================================

/// One readable kind, as an element `x` of that kind given its extra
/// start-tag attributes and the content after its name.
type Holder = (ElementKind, fn(&str, &str) -> String);

/// The five kinds slice 0052 reads, each as the element `x`.
fn artifact_holders() -> [Holder; 5] {
    fn in_package(class: &str, attributes: &str, content: &str) -> String {
        in_case(
            &format!(
                "<artifactElement xmi:type=\"sacm:{class}\" xmi:id=\"x\"{attributes}>\
                 <name content=\"X\"/>{content}</artifactElement>"
            ),
            "",
        )
    }
    [
        (ElementKind::ArtifactPackage, |attributes, content| {
            format!(
                "<sacm:ArtifactPackage{NS} xmi:id=\"x\"{attributes}><name content=\"X\"/>\
                 {}{content}</sacm:ArtifactPackage>",
                typed("Technique", "t1", "")
            )
        }),
        (ElementKind::Property, |a, c| in_package("Property", a, c)),
        (ElementKind::Technique, |a, c| in_package("Technique", a, c)),
        (ElementKind::Participant, |a, c| {
            in_package("Participant", a, c)
        }),
        (ElementKind::ArtifactGroup, |a, c| {
            in_package("ArtifactGroup", a, c)
        }),
    ]
}

/// The element-typed composite roles of `kind` (the inventory's "Concrete
/// class -> containment roles" table; `name` is left out).
fn roles_of(kind: ElementKind) -> Vec<&'static str> {
    let utility = [
        "description",
        "implementationConstraint",
        "note",
        "taggedValue",
    ];
    let own: &[&str] = match kind {
        ElementKind::ArtifactPackage => &["artifactElement"],
        ElementKind::Property | ElementKind::Technique | ElementKind::Participant => &["property"],
        _ => &[],
    };
    own.iter().copied().chain(utility).collect()
}

/// The reference ends of `kind` (ptc/22-03-13) this release reads:
/// `citedElement` and `abstractForm` of every `SACMElement`, and
/// `artifactElement` of an `ArtifactGroup`. ArtifactPackage's `interface` is
/// refused until SACM23-ART-012, so it is left out.
fn ends_of(kind: ElementKind) -> Vec<&'static str> {
    let mut ends = vec!["citedElement", "abstractForm"];
    if kind == ElementKind::ArtifactGroup {
        ends.push("artifactElement");
    }
    ends
}

#[test]
fn sacm23_art_011_no_artifact_class_has_a_composite_role_and_a_reference_end_of_the_same_name() {
    let mut failures = Vec::new();
    for (kind, element_of) in &artifact_holders() {
        let roles = roles_of(*kind);
        let ends = ends_of(*kind);
        for role in &roles {
            if ends.contains(role) {
                failures.push(format!("the tables give {kind} a role and an end {role}"));
            }
        }
        // A role name, as an attribute, is no reference end of the class:
        // preserved by the element, never read as a reference.
        for role in &roles {
            let (input, marks) = marked(&element_of(&format!(" {MARK}{role}=\"t1\""), ""));
            match read(Mode::Tolerant, &input) {
                Ok(output)
                    if output
                        .diagnostics()
                        .iter()
                        .map(|found| (found.code(), found.location(), found.affected().to_vec()))
                        .eq([(
                            Code::XMI_UNKNOWN_CONTENT,
                            marks.first().copied(),
                            list(&["x"]),
                        )])
                        && output.document().get("x").is_some_and(|x| {
                            x.has_preserved_content() && x.group_members().is_empty()
                        }) => {}
                other => failures.push(format!(
                    "{role}=\"t1\" on a {kind}: not one SACM-XMI-003 at the attribute, \
                     preserved by x: {:?}",
                    other.map(|output| output.diagnostics().to_vec())
                )),
            }
        }
        // An end name, as a nested typed element, is no composite role: a strict
        // read refuses it, never reading it as a child.
        for end in &ends {
            let input = element_of(
                "",
                &format!(
                    "<{end} xmi:type=\"sacm:Technique\" xmi:id=\"z9\"><name content=\"Z\"/></{end}>"
                ),
            );
            match read(Mode::Strict, &input) {
                Err(error @ ReadError::Unsupported { .. }) if error.location().is_some() => {}
                other => failures.push(format!(
                    "a nested Technique in {end} of a {kind}: not Unsupported: {other:?}"
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
    // Both readings of artifactElement in one document.
    let what = "artifactElement as an end on a group and a role on a package";
    let doc = read_ok(what, &g_in(" artifactElement=\"t1\"", ""));
    assert_eq!(
        (members(&doc, "g"), children(&doc, "a"), members(&doc, "a")),
        (ids(&["t1"]), ids(&["t1", "pa1", "pr1", "g"]), Vec::new()),
        "{ROW}: {what}"
    );
}
