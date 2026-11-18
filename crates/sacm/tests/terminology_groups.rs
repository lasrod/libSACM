//! Tests for `TerminologyGroup` and its member end `terminologyElement`
//! (SACM23-TERM-006; formal/23-05-08 §10.3 "terminologyElement[0..*] - an
//! optional collection of `TerminologyElements` that are organised within the
//! `TerminologyGroup`.", "`TerminologyGroups` serve no structural purpose in the
//! formation of the argument network, nor are they meant as a structural
//! packaging mechanism (this should be done using `TerminologyPackages`).";
//! ptc/22-03-13 `Terminology::TerminologyGroup` (`terminologyElement`, no
//! aggregation); ADR 0028, ADR 0034 decision 5 and amendment 3).
//!
//! A `TerminologyGroup` is read typed in a TerminologyPackage's
//! `terminologyElement` and in an ArtifactPackage's `artifactElement`. Its own
//! `terminologyElement` is a multi-valued reference end, not containment: an
//! IDREFS attribute, or elements carrying `xmi:idref` or `href="#id"`, read as
//! SACM23-ARG-013 reads an `ArgumentGroup`'s `argumentElement`. The parent's
//! kind decides what `terminologyElement` is: the member end under a
//! `TerminologyGroup`, the composite role under a TerminologyPackage. Members
//! stay children of their own package. The end is validated (SACM-REF-001,
//! SACM-REF-002 with clause 10.3, SACM-MULT-001 for a repeat), written as an
//! attribute after `abstractForm`, and handled by the delete policies; it has
//! no lower bound, so a scrub keeps the group.
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

use sacm::compare::Difference;
use sacm::edit::{AddReference, CreateTerminologyGroup, RemoveReference};
use sacm::model::LangString;

mod common;

const ROW: &str = "SACM23-TERM-006";

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
const END: &str = "terminologyElement";

/// The worked fixture of slice 0062.
const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/sacm23/xmi/terminology-valid.sacm.xmi"
);

// --- Inputs ------------------------------------------------------------------------

/// The `TerminologyGroup` `id` with `attributes`, holding `content` after its
/// name.
fn group(id: &str, attributes: &str, content: &str) -> String {
    format!(
        "<terminologyElement xmi:type=\"sacm:TerminologyGroup\" xmi:id=\"{id}\"{attributes}>\
         <name content=\"G\"/>{content}</terminologyElement>"
    )
}

/// The AssuranceCasePackage root `case`, one element per line:
/// - the TerminologyPackage `terms`: the Category `hazards`, the Term `t1`,
///   the Expression `e0` and the TerminologyPackage `iec` holding the Term
///   `appliance`, then `body`;
/// - the TerminologyPackage `other`, holding `other`;
/// - the ArgumentPackage `p`: the Claim `c1` (with the Note `n1`) and the
///   `ArgumentGroup` `ag` (member `c1`);
/// - the ArtifactPackage `evidence`: the Artifact `manual` and the
///   `ArtifactGroup` `ag1` (member `manual`).
fn case(body: &str, other: &str) -> String {
    format!(
        r#"<sacm:AssuranceCasePackage{NS} xmi:id="case">
  <name content="Case"/>
  <terminologyPackage xmi:id="terms"><name content="Terms"/>
    <terminologyElement xmi:type="sacm:Category" xmi:id="hazards"><name content="Hazards"/></terminologyElement>
    <terminologyElement xmi:type="sacm:Term" xmi:id="t1" value="t1"><name content="T1"/></terminologyElement>
    <terminologyElement xmi:type="sacm:Expression" xmi:id="e0" value="e0"><name content="E0"/></terminologyElement>
    <terminologyElement xmi:type="sacm:TerminologyPackage" xmi:id="iec"><name content="IEC"/><terminologyElement xmi:type="sacm:Term" xmi:id="appliance" value="appliance"><name content="Appliance"/></terminologyElement></terminologyElement>
    {body}
  </terminologyPackage>
  <terminologyPackage xmi:id="other"><name content="Other"/>
    {other}
  </terminologyPackage>
  <argumentPackage xmi:id="p"><name content="P"/>
    <argumentElement xmi:type="sacm:Claim" xmi:id="c1"><name content="C1"/><note xmi:id="n1"><content><value content="N"/></content></note></argumentElement>
    <argumentElement xmi:type="sacm:ArgumentGroup" xmi:id="ag" argumentElement="c1"><name content="AG"/></argumentElement>
  </argumentPackage>
  <artifactPackage xmi:id="evidence"><name content="Evidence"/>
    <artifactElement xmi:type="sacm:Artifact" xmi:id="manual"><name content="Manual"/></artifactElement>
    <artifactElement xmi:type="sacm:ArtifactGroup" xmi:id="ag1" artifactElement="manual"><name content="AG1"/></artifactElement>
  </artifactPackage>
</sacm:AssuranceCasePackage>"#
    )
}

/// The `TerminologyGroup` `g` in `terms` of [`case`].
fn g_in(attributes: &str, content: &str) -> String {
    case(&group("g", attributes, content), "")
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
/// SACM-XMI-003 at the mark, affecting `holder`, an Error in strict mode and a
/// Warning in tolerant mode. Returns the strict document.
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

fn parent_of(doc: &Document, id: &str) -> Option<String> {
    element(doc, id)
        .parent()
        .map(|parent| parent.id().as_str().to_owned())
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

fn fixture_text() -> String {
    std::fs::read_to_string(FIXTURE)
        .unwrap_or_else(|error| panic!("{ROW}: cannot read {FIXTURE}: {error}"))
}

/// The fixture with `from` replaced by `to`, which must change it.
fn fixture_with(from: &str, to: &str) -> String {
    let text = fixture_text();
    assert!(
        text.contains(from),
        "{ROW}: setup: the fixture has no {from:?}"
    );
    text.replacen(from, to, 1)
}

/// Applies `operation`, which the test expects to succeed, after checking
/// that `edit::check` accepts it and `edit::preview` gives the mutation
/// `apply` gives.
fn applied(doc: &mut Document, operation: impl Into<Operation>) -> Mutation {
    let operation = operation.into();
    let checked = edit::check(doc, &operation);
    let previewed = edit::preview(doc, &operation).map(|preview| preview.mutation().clone());
    let mutation = match edit::apply(doc, operation.clone()) {
        Ok(mutation) => mutation,
        Err(error) => panic!("{ROW}: {operation:?} was refused: {error} ({error:?})"),
    };
    assert_eq!(
        checked,
        Ok(()),
        "{ROW}: check refuses {operation:?}, which apply accepts"
    );
    assert_eq!(
        previewed.as_ref(),
        Ok(&mutation),
        "{ROW}: preview of {operation:?} does not give apply's mutation"
    );
    mutation
}

/// Applies `operation`, which the test expects to be refused leaving `doc`
/// `==` to before, with `check` and `preview` refusing it the same, and
/// returns why.
fn refuse(doc: &mut Document, operation: impl Into<Operation>) -> EditError {
    let operation = operation.into();
    let before = doc.clone();
    let checked = edit::check(doc, &operation);
    let previewed = edit::preview(doc, &operation).map(|preview| preview.mutation().clone());
    let error = match edit::apply(doc, operation.clone()) {
        Ok(mutation) => panic!("{ROW}: {operation:?} was accepted: {mutation:?}"),
        Err(error) => error,
    };
    assert!(
        *doc == before,
        "{ROW}: the refused {operation:?} changed the document"
    );
    assert_eq!(
        (checked, previewed),
        (Err(error.clone()), Err(error.clone())),
        "{ROW}: check or preview answer {operation:?} otherwise than apply"
    );
    error
}

/// Asserts that `operation` is `FeatureNotApplicable` on `target`, of
/// `kind`, naming `feature`, with SACM-CMD-008, and returns its Display.
fn assert_not_applicable(
    doc: &mut Document,
    operation: impl Into<Operation>,
    target: &str,
    kind: ElementKind,
    feature: &str,
) -> String {
    let error = refuse(doc, operation);
    assert!(
        matches!(
            &error,
            EditError::FeatureNotApplicable { element, kind: found, feature: found_feature, .. }
                if element.as_str() == target && *found == kind && *found_feature == feature
        ),
        "{ROW}: {feature} on the {kind} {target}: not FeatureNotApplicable: {error:?}"
    );
    assert_eq!(
        error.code(),
        Code::CMD_FEATURE_NOT_APPLICABLE,
        "{ROW}: {feature} on the {kind} {target}: not SACM-CMD-008"
    );
    error.to_string()
}

// =====================================================================================
// Item 1: the reference forms, RepeatedValue, an empty attribute, a nested element
// =====================================================================================

#[test]
fn sacm23_term_006_the_attribute_and_the_element_forms_read_equal_and_members_stay_children() {
    let what = "terminologyElement=\"t1 e0 appliance\"";
    let attribute = read_ok(what, &g_in(" terminologyElement=\"t1 e0 appliance\"", ""));
    assert_eq!(
        (
            element(&attribute, "g").kind(),
            members(&attribute, "g"),
            children(&attribute, "g"),
            children(&attribute, "terms"),
            parent_of(&attribute, "appliance")
        ),
        (
            ElementKind::TerminologyGroup,
            ids(&["t1", "e0", "appliance"]),
            Vec::new(),
            ids(&["hazards", "t1", "e0", "iec", "g"]),
            Some("iec".to_owned())
        ),
        "{ROW}: {what}: not a group of three members that stay children of their packages"
    );
    let elements = read_ok(
        "the element forms",
        &g_in(
            "",
            "<terminologyElement xmi:idref=\"t1\"/><terminologyElement href=\"#e0\"/>\
             <terminologyElement xmi:idref=\"appliance\"/>",
        ),
    );
    assert!(
        elements == attribute,
        "{ROW}: the element forms do not read == to the attribute"
    );
    // The fixture's group, in the element form of the acceptance.
    let fixture = read_ok("the fixture", &fixture_text());
    let variant = read_ok(
        "the fixture's group as elements",
        &fixture_with(
            " terminologyElement=\"blade lid appliance\">",
            "><terminologyElement xmi:idref=\"blade\"/><terminologyElement href=\"#lid\"/>\
             <terminologyElement xmi:idref=\"appliance\"/>",
        ),
    );
    assert!(
        variant == fixture,
        "{ROW}: the fixture's group as elements does not read == to the fixture"
    );
    assert_eq!(
        (
            members(&fixture, "reviewed"),
            parent_of(&fixture, "appliance"),
            children(&fixture, "reviewed")
        ),
        (
            ids(&["blade", "lid", "appliance"]),
            Some("iec".to_owned()),
            Vec::new()
        ),
        "{ROW}: the fixture's group"
    );
    // In an ArtifactPackage's artifactElement too.
    let doc = read_ok(
        "a TerminologyGroup typed in artifactElement",
        &case("", "").replace(
            "<artifactElement xmi:type=\"sacm:Artifact\" xmi:id=\"manual\"><name content=\"Manual\"/></artifactElement>",
            "<artifactElement xmi:type=\"sacm:Artifact\" xmi:id=\"manual\"><name content=\"Manual\"/></artifactElement>\
             <artifactElement xmi:type=\"sacm:TerminologyGroup\" xmi:id=\"tg\" terminologyElement=\"t1\"><name content=\"TG\"/></artifactElement>",
        ),
    );
    assert_eq!(
        (
            element(&doc, "tg").kind(),
            parent_of(&doc, "tg"),
            members(&doc, "tg")
        ),
        (
            ElementKind::TerminologyGroup,
            Some("evidence".to_owned()),
            ids(&["t1"])
        ),
        "{ROW}: a TerminologyGroup in an ArtifactPackage"
    );
}

#[test]
fn sacm23_term_006_an_attribute_and_member_elements_are_repeated_value_at_both_occurrences() {
    let what = "the attribute and a member element";
    let (input, marks) = marked(&g_in(
        &format!(" {MARK}terminologyElement=\"t1\""),
        &format!("{MARK}<terminologyElement xmi:idref=\"e0\"/>"),
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
    let output = match read(Mode::Tolerant, &input) {
        Ok(output) => output,
        Err(error) => panic!("{ROW}: {what}: a tolerant read refused it: {error:?}"),
    };
    let found: Vec<_> = output
        .diagnostics()
        .iter()
        .map(|found| (found.code(), found.severity(), found.location()))
        .collect();
    assert_eq!(
        found,
        [(
            Code::MULTIPLICITY_VIOLATION,
            Severity::Warning,
            marks.get(1).copied()
        )],
        "{ROW}: {what}: a tolerant read does not give one SACM-MULT-001 Warning at the repeat"
    );
    assert_eq!(
        members(output.document(), "g"),
        ids(&["t1"]),
        "{ROW}: {what}: a tolerant read does not keep the attribute"
    );
}

#[test]
fn sacm23_term_006_an_empty_or_qualified_attribute_is_preserved_by_the_group() {
    for (what, attribute) in [
        (
            "an empty attribute",
            format!(" {MARK}terminologyElement=\"\""),
        ),
        (
            "an attribute of only white space",
            format!(" {MARK}terminologyElement=\" \""),
        ),
        (
            "sacm:terminologyElement",
            format!(" {MARK}sacm:terminologyElement=\"t1\""),
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

#[test]
fn sacm23_term_006_a_nested_element_under_a_group_is_unsupported_or_preserved_at_that_construct() {
    let label = "(a limit of this version; SACM23-XMI-019 reads it)";
    for (what, nested) in [
        (
            "a typed Term",
            format!(
                "<terminologyElement {MARK}xmi:type=\"sacm:Term\" xmi:id=\"t9\" value=\"t9\">\
                 <name content=\"T9\"/></terminologyElement>"
            ),
        ),
        (
            "a typed Category with no content",
            format!("<terminologyElement {MARK}xmi:type=\"sacm:Category\" xmi:id=\"k9\"/>"),
        ),
        (
            "an element by xsi:type",
            format!("<terminologyElement {MARK}xsi:type=\"sacm:Term\" xmi:id=\"t9\"/>"),
        ),
        (
            "an untyped element with an id",
            format!("<terminologyElement {MARK}xmi:id=\"t9\"/>"),
        ),
        (
            "a typed Term after a member element",
            format!(
                "<terminologyElement xmi:idref=\"t1\"/>\
                 <terminologyElement {MARK}xmi:type=\"sacm:Term\" xmi:id=\"t9\"/>"
            ),
        ),
    ] {
        let what = format!("{what} inside a TerminologyGroup");
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
    let what = "an element with only a name inside a TerminologyGroup";
    let doc = assert_preserved_at(
        what,
        &g_in(
            "",
            &format!("{MARK}<terminologyElement><name content=\"X9\"/></terminologyElement>"),
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
fn sacm23_term_006_each_group_kind_keeps_the_other_groups_ends_as_preserved_content() {
    for (what, attribute) in [
        (
            "argumentElement on a TerminologyGroup",
            format!(" {MARK}argumentElement=\"t1\""),
        ),
        (
            "artifactElement on a TerminologyGroup",
            format!(" {MARK}artifactElement=\"t1\""),
        ),
    ] {
        assert_preserved_at(what, &g_in(&attribute, ""), "g");
    }
    let what = "terminologyElement on an ArtifactGroup";
    let template = case("", "").replace(
        "xmi:id=\"ag1\" artifactElement=\"manual\"",
        &format!("xmi:id=\"ag1\" artifactElement=\"manual\" {MARK}terminologyElement=\"t1\""),
    );
    assert_preserved_at(what, &template, "ag1");
}

// =====================================================================================
// Item 2: group_members() for the three group kinds
// =====================================================================================

#[test]
fn sacm23_term_006_group_members_answers_for_the_three_group_kinds_and_is_empty_for_others() {
    let doc = read_ok(
        "three groups",
        &g_in(" terminologyElement=\"t1 iec t1 g\"", ""),
    );
    assert_eq!(
        (
            members(&doc, "g"),
            members(&doc, "ag1"),
            members(&doc, "ag")
        ),
        (
            ids(&["t1", "iec", "t1", "g"]),
            ids(&["manual"]),
            ids(&["c1"])
        ),
        "{ROW}: group_members() of the three group kinds, in order with repeats"
    );
    for id in [
        "case",
        "terms",
        "hazards",
        "t1",
        "e0",
        "iec",
        "appliance",
        "other",
        "p",
        "c1",
        "n1",
        "evidence",
        "manual",
    ] {
        assert!(
            element(&doc, id).group_members().is_empty(),
            "{ROW}: group_members() of {id} is not empty"
        );
    }
    assert_eq!(
        (parent_of(&doc, "t1"), parent_of(&doc, "iec")),
        (Some("terms".to_owned()), Some("terms".to_owned())),
        "{ROW}: a member moved"
    );
}

// =====================================================================================
// Item 3: the reference edits with TerminologyGroupMember
// =====================================================================================

#[test]
fn sacm23_term_006_terminology_group_member_is_the_end_terminology_element() {
    let end = ReferenceEnd::TerminologyGroupMember;
    assert_eq!(end.name(), END, "{ROW}: TerminologyGroupMember.name()");
    for other in [ReferenceEnd::GroupMember, ReferenceEnd::ArtifactGroupMember] {
        assert_ne!(end, other, "{ROW}: two group ends are one variant");
    }
    for &kind in ElementKind::ALL {
        let group = kind == ElementKind::TerminologyGroup;
        assert_eq!(
            (end.applies_to(kind), end.bounds(kind)),
            (group, group.then_some((0, None))),
            "{ROW}: terminologyElement on a {kind}"
        );
    }
}

#[test]
fn sacm23_term_006_set_add_and_remove_reference_edit_a_terminology_group_s_members() {
    let mut doc = read_ok("an empty group", &g_in("", ""));
    let end = ReferenceEnd::TerminologyGroupMember;
    let mutation = applied(&mut doc, SetReferences::new("g", end, ["t1", "iec", "t1"]));
    assert_eq!(
        (mutation.changed_ids(), members(&doc, "g")),
        (list(&["g"]).as_slice(), ids(&["t1", "iec", "t1"])),
        "{ROW}: SetReferences"
    );
    applied(&mut doc, AddReference::new("g", end, "appliance"));
    assert_eq!(
        members(&doc, "g"),
        ids(&["t1", "iec", "t1", "appliance"]),
        "{ROW}: AddReference appends"
    );
    applied(&mut doc, RemoveReference::new("g", end, "t1"));
    assert_eq!(
        members(&doc, "g"),
        ids(&["iec", "appliance"]),
        "{ROW}: RemoveReference removes every occurrence"
    );
    assert_eq!(
        parent_of(&doc, "appliance"),
        Some("iec".to_owned()),
        "{ROW}: a member moved"
    );
    let error = refuse(&mut doc, AddReference::new("g", end, "nope"));
    assert!(
        matches!(&error, EditError::ReferenceNotFound { end, id, .. }
            if *end == END && id.as_str() == "nope"),
        "{ROW}: AddReference of a missing id: {error:?}"
    );
    let error = refuse(&mut doc, SetReferences::new("g", end, ["t1", "c1"]));
    assert!(
        matches!(&error, EditError::WrongReferenceType { end, id, expected, .. }
            if *end == END && id.as_str() == "c1" && *expected == "TerminologyElement"),
        "{ROW}: SetReferences with a Claim: {error:?}"
    );
    let error = refuse(&mut doc, RemoveReference::new("g", end, "c1"));
    assert!(
        matches!(&error, EditError::ReferenceNotHeld { end, id, .. }
            if *end == END && id.as_str() == "c1"),
        "{ROW}: RemoveReference of an id not held: {error:?}"
    );
    applied(&mut doc, SetReferences::clear("g", end));
    let text = write_in("the cleared group", &doc, Mode::Strict);
    assert_eq!(
        line_with(&text, "g"),
        "<terminologyElement xmi:type=\"sacm:TerminologyGroup\" xmi:id=\"g\">",
        "{ROW}: the cleared end is written\n{text}"
    );
}

#[test]
fn sacm23_term_006_terminology_group_member_on_an_artifact_group_is_feature_not_applicable() {
    let mut doc = read_ok("a group", &g_in("", ""));
    let shown = assert_not_applicable(
        &mut doc,
        SetReferences::new("ag1", ReferenceEnd::TerminologyGroupMember, ["t1"]),
        "ag1",
        ElementKind::ArtifactGroup,
        END,
    );
    assert_eq!(
        shown, "the ArtifactGroup \"ag1\" has no terminologyElement",
        "{ROW}: the Display of terminologyElement on an ArtifactGroup"
    );
    for (target, kind) in [
        ("ag", ElementKind::ArgumentGroup),
        ("terms", ElementKind::TerminologyPackage),
        ("t1", ElementKind::Term),
    ] {
        assert_not_applicable(
            &mut doc,
            AddReference::new(target, ReferenceEnd::TerminologyGroupMember, "t1"),
            target,
            kind,
            END,
        );
    }
}

#[test]
fn sacm23_term_006_the_other_group_ends_on_a_terminology_group_are_feature_not_applicable() {
    let mut doc = read_ok("a group", &g_in(" terminologyElement=\"t1\"", ""));
    for (end, feature) in [
        (ReferenceEnd::GroupMember, "argumentElement"),
        (ReferenceEnd::ArtifactGroupMember, "artifactElement"),
    ] {
        let shown = assert_not_applicable(
            &mut doc,
            SetReferences::new("g", end, ["t1"]),
            "g",
            ElementKind::TerminologyGroup,
            feature,
        );
        assert_eq!(
            shown,
            format!("the TerminologyGroup \"g\" has no {feature}"),
            "{ROW}: the Display of {feature} on a TerminologyGroup"
        );
    }
}

// =====================================================================================
// Item 4: validation and the strict write
// =====================================================================================

#[test]
fn sacm23_term_006_every_terminology_element_member_validates_and_a_claim_is_ref_002() {
    let what = "members of every TerminologyElement kind";
    let doc = read_ok(
        what,
        &case(
            &group(
                "g",
                " terminologyElement=\"hazards t1 e0 iec appliance g h terms\"",
                "",
            ),
            &group("h", " terminologyElement=\"t1 other\"", ""),
        ),
    );
    assert_eq!(
        validation::validate(&doc),
        [],
        "{ROW}: {what}: validate reports a finding"
    );
    round_trip(what, &doc, Mode::Strict);
    for (what, attribute, target) in [
        ("a Claim member", " terminologyElement=\"t1 c1\"", "c1"),
        (
            "an Artifact member",
            " terminologyElement=\"manual\"",
            "manual",
        ),
        ("a Note member", " terminologyElement=\"n1\"", "n1"),
        (
            "an AssuranceCasePackage member",
            " terminologyElement=\"case\"",
            "case",
        ),
    ] {
        let doc = read_ok(what, &g_in(attribute, ""));
        let found: Vec<Finding> = validation::validate(&doc).iter().map(finding).collect();
        assert_eq!(
            found,
            [(
                Code::REF_WRONG_TYPE,
                Severity::Error,
                "SACM23-VAL-004".to_owned(),
                Some("10.3".to_owned()),
                None,
                list(&["g", target])
            )],
            "{ROW}: {what}: not one SACM-REF-002 Error with clause 10.3"
        );
        // An IDREF is untyped: a strict write writes it.
        let text = write_in(what, &doc, Mode::Strict);
        assert!(
            line_with(&text, "g").ends_with(&format!("{attribute}>")),
            "{ROW}: {what}: a strict write does not keep the member\n{text}"
        );
    }
}

#[test]
fn sacm23_term_006_an_absent_member_is_ref_001_and_a_repeat_is_one_mult_001_warning() {
    let what = "terminologyElement=\"t1 ghost\"";
    let doc = read_ok(what, &g_in(" terminologyElement=\"t1 ghost\"", ""));
    let found: Vec<Finding> = validation::validate(&doc).iter().map(finding).collect();
    assert_eq!(
        found,
        [(
            Code::REF_DANGLING,
            Severity::Error,
            "SACM23-VAL-003".to_owned(),
            Some("10.3".to_owned()),
            None,
            list(&["g", "ghost"])
        )],
        "{ROW}: {what}: not one SACM-REF-001 Error with clause 10.3"
    );
    let error = match xmi::write(&doc, &WriteOptions::default().with_mode(Mode::Strict)) {
        Ok(text) => panic!("{ROW}: {what}: a strict write accepted it:\n{text}"),
        Err(error) => error,
    };
    assert!(
        matches!(error, WriteError::Invalid { .. }),
        "{ROW}: {what}: a strict write is not Invalid: {error:?}"
    );
    let text = write_in(what, &doc, Mode::Tolerant);
    assert!(
        line_with(&text, "g").ends_with(" terminologyElement=\"t1 ghost\">"),
        "{ROW}: {what}: the dangling id is not written back\n{text}"
    );
    let what = "terminologyElement=\"t1 e0 t1 t1\"";
    let doc = read_ok(what, &g_in(" terminologyElement=\"t1 e0 t1 t1\"", ""));
    let found: Vec<Finding> = validation::validate(&doc).iter().map(finding).collect();
    assert_eq!(
        found,
        [(
            Code::MULTIPLICITY_VIOLATION,
            Severity::Warning,
            "SACM23-VAL-022".to_owned(),
            Some("10.3".to_owned()),
            None,
            list(&["g", "t1"])
        )],
        "{ROW}: {what}: not one SACM-MULT-001 Warning with clause 10.3"
    );
    let text = round_trip(what, &doc, Mode::Strict);
    assert!(
        line_with(&text, "g").ends_with(" terminologyElement=\"t1 e0 t1 t1\">"),
        "{ROW}: {what}: the repeats are not written back\n{text}"
    );
}

// =====================================================================================
// Item 5: writing and comparing the members
// =====================================================================================

#[test]
fn sacm23_term_006_members_are_written_after_abstract_form_kept_when_repeated_and_absent_when_empty()
 {
    let what = "a group with every SACMElement feature and members";
    let doc = read_ok(
        what,
        &g_in(
            " terminologyElement=\"t1 t1\" abstractForm=\"g\" citedElement=\"g\" isAbstract=\"true\" \
             isCitation=\"true\" gid=\"G-1\"",
            "",
        ),
    );
    let text = round_trip(what, &doc, Mode::Strict);
    assert_eq!(
        line_with(&text, "g"),
        "<terminologyElement xmi:type=\"sacm:TerminologyGroup\" xmi:id=\"g\" gid=\"G-1\" \
         isCitation=\"true\" isAbstract=\"true\" citedElement=\"g\" abstractForm=\"g\" \
         terminologyElement=\"t1 t1\">",
        "{ROW}: {what}: not written in order with the repeat kept\n{text}"
    );
    let doc = read_ok(
        "member elements",
        &g_in(
            "",
            "<terminologyElement href=\"#e0\"/><terminologyElement xmi:idref=\"t1\"/>",
        ),
    );
    let text = round_trip("member elements", &doc, Mode::Strict);
    assert_eq!(
        line_with(&text, "g"),
        "<terminologyElement xmi:type=\"sacm:TerminologyGroup\" xmi:id=\"g\" \
         terminologyElement=\"e0 t1\">",
        "{ROW}: the member elements are not written as one attribute\n{text}"
    );
    let doc = read_ok("an empty group", &g_in("", ""));
    let text = round_trip("an empty group", &doc, Mode::Strict);
    assert_eq!(
        line_with(&text, "g"),
        "<terminologyElement xmi:type=\"sacm:TerminologyGroup\" xmi:id=\"g\">",
        "{ROW}: an empty end is written\n{text}"
    );
}

#[test]
fn sacm23_term_006_a_terminology_group_members_difference_displays_its_end_and_both_lists() {
    let left = read_ok(
        "left",
        &fixture_with(
            " terminologyElement=\"blade lid appliance\"",
            " terminologyElement=\"blade\"",
        ),
    );
    let right = read_ok(
        "right",
        &fixture_with(" terminologyElement=\"blade lid appliance\"", ""),
    );
    let found = semantic_compare(&left, &right);
    let [difference] = found.as_slice() else {
        panic!("{ROW}: not one difference: {found:?}");
    };
    assert_eq!(
        (difference.to_string(), difference.element_id()),
        (
            "element \"reviewed\" differs in terminologyElement: [\"blade\"] on the left, [] on \
             the right"
                .to_owned(),
            Some(&ElementId::from("reviewed"))
        ),
        "{ROW}: the Display of the terminologyElement difference"
    );
}

#[test]
fn sacm23_term_006_semantic_compare_reports_one_terminology_group_members_difference() {
    let with = |attributes: &str| read_ok(attributes, &g_in(attributes, ""));
    for (left, right) in [
        (vec!["t1", "e0"], vec!["e0", "t1"]),
        (vec!["t1"], vec!["t1", "t1"]),
        (vec!["t1"], Vec::new()),
    ] {
        let attribute = |values: &[&str]| {
            if values.is_empty() {
                String::new()
            } else {
                format!(" terminologyElement=\"{}\"", values.join(" "))
            }
        };
        assert_eq!(
            semantic_compare(&with(&attribute(&left)), &with(&attribute(&right))),
            [Difference::TerminologyGroupMembers {
                id: ElementId::from("g"),
                left: list(&left),
                right: list(&right),
            }],
            "{ROW}: {left:?} vs {right:?}: not one TerminologyGroupMembers difference"
        );
    }
    // After AbstractForm on the group; apart from an ArgumentGroup's members.
    let left = read_ok(
        "left",
        &g_in(" abstractForm=\"g\" terminologyElement=\"t1\"", "")
            .replace("argumentElement=\"c1\"", "argumentElement=\"c1 c1\""),
    );
    let right = read_ok("right", &g_in("", ""));
    let found = semantic_compare(&left, &right);
    assert!(
        matches!(
            found.as_slice(),
            [
                Difference::AbstractForm { id: g_a, .. },
                Difference::TerminologyGroupMembers { id: g_m, .. },
                Difference::GroupMembers { id: ag, .. },
            ] if g_a.as_str() == "g" && g_m.as_str() == "g" && ag.as_str() == "ag"
        ),
        "{ROW}: not AbstractForm and TerminologyGroupMembers of g, then GroupMembers of ag: \
         {found:?}"
    );
}

// =====================================================================================
// Item 6: CreateTerminologyGroup
// =====================================================================================

fn named(content: &str) -> LangString {
    LangString::new(content).with_lang("en")
}

#[test]
fn sacm23_term_006_create_terminology_group_reports_what_it_was_given_and_creates_it() {
    let operation = CreateTerminologyGroup::new("terms", named("Reviewed"));
    assert_eq!(
        (
            operation.parent().as_str(),
            operation.id(),
            operation.name(),
            operation.group_members()
        ),
        ("terms", None, &named("Reviewed"), [].as_slice()),
        "{ROW}: the defaults"
    );
    let operation = operation
        .with_group_members(["hazards"])
        .with_id("reviewed")
        .with_group_members(["t1", "appliance", "t1"]);
    assert_eq!(
        (
            operation.id().map(ElementId::as_str),
            operation
                .group_members()
                .iter()
                .map(ElementId::as_str)
                .collect::<Vec<_>>()
        ),
        (Some("reviewed"), vec!["t1", "appliance", "t1"]),
        "{ROW}: the builders replace what they were given"
    );
    assert_eq!(
        Operation::from(operation.clone()).operation_name(),
        "CreateTerminologyGroup",
        "{ROW}: operation_name()"
    );
    let mut doc = read_ok("the case", &case("", ""));
    let mutation = applied(&mut doc, operation);
    assert_eq!(
        (
            mutation.created_ids(),
            mutation.changed_ids(),
            members(&doc, "reviewed"),
            parent_of(&doc, "appliance")
        ),
        (
            list(&["reviewed"]).as_slice(),
            [].as_slice(),
            ids(&["t1", "appliance", "t1"]),
            Some("iec".to_owned())
        ),
        "{ROW}: the created group"
    );
    let text = write_in("the created group", &doc, Mode::Strict);
    assert_eq!(
        line_with(&text, "reviewed"),
        "<terminologyElement xmi:type=\"sacm:TerminologyGroup\" xmi:id=\"reviewed\" \
         terminologyElement=\"t1 appliance t1\">",
        "{ROW}: the created group's start tag\n{text}"
    );
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the cases of one acceptance item read best as one list"
)]
fn sacm23_term_006_create_terminology_group_legal_parents_refusals_and_generated_id() {
    let mut doc = read_ok("the case", &case("", ""));
    let first = applied(&mut doc, CreateTerminologyGroup::new("terms", named("N")));
    let second = applied(
        &mut doc,
        CreateTerminologyGroup::new("evidence", named("N")),
    );
    assert_eq!(
        (first.created_ids(), second.created_ids()),
        (
            list(&["terminology_group_1"]).as_slice(),
            list(&["terminology_group_2"]).as_slice()
        ),
        "{ROW}: the generated ids"
    );
    assert_eq!(
        (
            parent_of(&doc, "terminology_group_1"),
            parent_of(&doc, "terminology_group_2")
        ),
        (Some("terms".to_owned()), Some("evidence".to_owned())),
        "{ROW}: the legal parents"
    );
    let text = round_trip("the created groups", &doc, Mode::Strict);
    assert!(
        line_with(&text, "terminology_group_2")
            .starts_with("<artifactElement xmi:type=\"sacm:TerminologyGroup\""),
        "{ROW}: a group in an ArtifactPackage is not written typed in artifactElement\n{text}"
    );
    for (parent, parent_kind) in [
        ("case", ElementKind::AssuranceCasePackage),
        ("p", ElementKind::ArgumentPackage),
        ("t1", ElementKind::Term),
        ("terminology_group_1", ElementKind::TerminologyGroup),
        ("ag1", ElementKind::ArtifactGroup),
    ] {
        let error = refuse(&mut doc, CreateTerminologyGroup::new(parent, named("N")));
        assert!(
            matches!(
                &error,
                EditError::InvalidParent { parent: found, parent_kind: found_kind, kind, .. }
                    if found.as_str() == parent && *found_kind == parent_kind
                        && *kind == ElementKind::TerminologyGroup
            ),
            "{ROW}: a group under the {parent_kind} {parent}: not InvalidParent: {error:?}"
        );
    }
    let group = |given: &[&str]| {
        CreateTerminologyGroup::new("terms", named("G")).with_group_members(given.iter().copied())
    };
    for (what, operation, expected) in [
        (
            "a missing parent",
            CreateTerminologyGroup::new("nowhere", named("G")).with_group_members(["nope"]),
            "ParentNotFound",
        ),
        (
            "an illegal parent",
            CreateTerminologyGroup::new("p", named("G")).with_group_members(["nope"]),
            "InvalidParent",
        ),
        ("an invalid id", group(&["nope"]).with_id("1g"), "InvalidId"),
        (
            "a duplicate id",
            group(&["nope"]).with_id("t1"),
            "DuplicateId",
        ),
    ] {
        let error = refuse(&mut doc, operation);
        let shown = format!("{error:?}");
        assert!(
            shown.starts_with(expected),
            "{ROW}: {what} with an unknown member: not {expected}: {error:?}"
        );
    }
    for (what, given, expected) in [
        (
            "white space in a member before a missing one",
            vec!["nope", "a b"],
            ("InvalidReference", "a b", None),
        ),
        (
            "a missing member",
            vec!["t1", "nope"],
            ("ReferenceNotFound", "nope", None),
        ),
        (
            "a missing member before a wrong type",
            vec!["c1", "nope"],
            ("ReferenceNotFound", "nope", None),
        ),
        (
            "a Claim member",
            vec!["t1", "c1"],
            ("WrongReferenceType", "c1", Some("TerminologyElement")),
        ),
        (
            "a Note member",
            vec!["n1"],
            ("WrongReferenceType", "n1", Some("TerminologyElement")),
        ),
    ] {
        let error = refuse(&mut doc, group(&given));
        let found = match &error {
            EditError::InvalidReference { end, id, .. } => ("InvalidReference", *end, id, None),
            EditError::ReferenceNotFound { end, id, .. } => ("ReferenceNotFound", *end, id, None),
            EditError::WrongReferenceType {
                end, id, expected, ..
            } => ("WrongReferenceType", *end, id, Some(*expected)),
            other => panic!("{ROW}: {what}: not a reference refusal: {other:?}"),
        };
        assert_eq!(
            (found.0, found.1, found.2.as_str(), found.3),
            (expected.0, END, expected.1, expected.2),
            "{ROW}: {what}"
        );
    }
    let next = applied(&mut doc, CreateTerminologyGroup::new("terms", named("N")));
    assert_eq!(
        next.created_ids(),
        list(&["terminology_group_3"]).as_slice(),
        "{ROW}: a refusal moved the next generated id"
    );
}

// =====================================================================================
// Item 7: the delete policies
// =====================================================================================

#[test]
fn sacm23_term_006_deleting_a_member_is_referenced_or_cross_package_reference_by_default() {
    let mut doc = read_ok(
        "a group in terms",
        &g_in(" terminologyElement=\"t1 e0\"", ""),
    );
    let error = refuse(&mut doc, DeleteElement::new("t1"));
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
        "{ROW}: not SACM-CMD-002"
    );
    let mut doc = read_ok(
        "a group in other",
        &case("", &group("h", " terminologyElement=\"t1\"", "")),
    );
    let error = refuse(&mut doc, DeleteElement::new("t1"));
    assert!(
        matches!(
            &error,
            EditError::CrossPackageReference { holder, end, id, .. }
                if holder.as_str() == "h" && *end == END && id.as_str() == "t1"
        ),
        "{ROW}: deleting t1, a member of h in other: not CrossPackageReference: {error:?}"
    );
    assert_eq!(
        error.code(),
        Code::CMD_EXTERNAL_REFERENCES,
        "{ROW}: not SACM-CMD-007"
    );
}

#[test]
fn sacm23_term_006_scrub_and_delete_referencing_relationships_remove_the_member_and_keep_the_group()
{
    for policy in [
        ReferenceDeletePolicy::ScrubReferences,
        ReferenceDeletePolicy::DeleteReferencingRelationships,
    ] {
        let mut doc = read_ok(
            "a group in terms",
            &g_in(" terminologyElement=\"t1 e0 t1\"", ""),
        );
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
                list(&["t1"]).as_slice(),
                list(&["g"]).as_slice(),
                ids(&["e0"])
            ),
            "{ROW}: {policy:?}: deleting t1 does not remove both its ids from g"
        );
        let mutation = applied(
            &mut doc,
            DeleteElement::new("e0").with_reference_policy(policy),
        );
        assert_eq!(
            (
                mutation.changed_ids(),
                members(&doc, "g"),
                children(&doc, "terms")
            ),
            (
                list(&["g"]).as_slice(),
                Vec::new(),
                ids(&["hazards", "iec", "g"])
            ),
            "{ROW}: {policy:?}: the group does not survive losing its last member"
        );
        let text = round_trip("the emptied group", &doc, Mode::Strict);
        assert_eq!(
            line_with(&text, "g"),
            "<terminologyElement xmi:type=\"sacm:TerminologyGroup\" xmi:id=\"g\">",
            "{ROW}: {policy:?}: the emptied group is not written without the end\n{text}"
        );
        let mut doc = read_ok(
            "a group in other",
            &case("", &group("h", " terminologyElement=\"t1\"", "")),
        );
        let mutation = applied(
            &mut doc,
            DeleteElement::new("t1")
                .with_reference_policy(policy)
                .with_cross_package_policy(CrossPackageReferencePolicy::ApplyReferencePolicy),
        );
        assert_eq!(
            (mutation.changed_ids(), members(&doc, "h")),
            (list(&["h"]).as_slice(), Vec::<String>::new()),
            "{ROW}: {policy:?}: deleting t1 across packages does not scrub h"
        );
    }
    let mut doc = read_ok(
        "a group in terms",
        &g_in(" terminologyElement=\"t1 e0\"", ""),
    );
    let mutation = applied(&mut doc, DeleteElement::new("g"));
    assert_eq!(
        (mutation.deleted_ids(), children(&doc, "terms")),
        (
            list(&["g"]).as_slice(),
            ids(&["hazards", "t1", "e0", "iec"])
        ),
        "{ROW}: deleting the group deletes a member"
    );
}
