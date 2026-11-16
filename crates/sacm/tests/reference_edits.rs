//! Tests for the reference edits (LIB-EDIT-009; formal/23-05-08 §8.2,
//! §11.2, §11.9, §11.10, §11.12, §11.13; D-VAL-LAYER, D-ATOMIC; ADR 0026,
//! ADR 0027, ADR 0028, ADR 0029).
//!
//! `SetReferences` replaces the ids of one end, in order and with repeats
//! (`clear` empties it), `AddReference` appends one id, and
//! `RemoveReference` drops every occurrence of one, each within the end's
//! bounds: `source` `[1..*]`, `target` `[1]`, the other single-valued ends
//! `[0..1]` and the other multi-valued ends `[0..*]`. They refuse, in order:
//! an unknown element (SACM-CMD-001), a kind without the end (SACM-CMD-008),
//! for a remove an id the end does not hold (SACM-CMD-001), the bounds
//! (SACM-MULT-001), then, over the ids the end does not hold already, white
//! space (SACM-ID-002), an id naming no element (SACM-CMD-001) and a
//! wrongly typed id (SACM-REF-002). Content rules are never refused.
//!
//! The declared types and bounds are written here from ptc/22-03-13, not
//! taken from the crate. The last part of item 9 (the operations join the
//! LIB-EDIT-002 set) is in `edit_atomic.rs`.

use sacm::compare::{Difference, semantic_compare};
use sacm::diagnostic::{Code, Severity};
use sacm::edit::{
    self, AddReference, CreateArgumentPackage, CreateAssuranceCasePackage, CreateClaim, EditError,
    Mutation, Operation, RemoveReference, SetReferences,
};
use sacm::model::{Document, ElementId, ElementKind, LangString, ReferenceEnd, SacmElement};
use sacm::validation;
use sacm::xmi::{self, Mode, ReadOptions, WriteOptions};

const ROW: &str = "LIB-EDIT-009";

/// The namespace declarations of a bare root: `sacm` and `xmi`.
const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
);

/// The ten ends, with their XMI names.
const ENDS: [(ReferenceEnd, &str); 10] = [
    (ReferenceEnd::CitedElement, "citedElement"),
    (ReferenceEnd::AbstractForm, "abstractForm"),
    (ReferenceEnd::MetaClaim, "metaClaim"),
    (ReferenceEnd::Reasoning, "reasoning"),
    (ReferenceEnd::Source, "source"),
    (ReferenceEnd::Target, "target"),
    (ReferenceEnd::Structure, "structure"),
    (
        ReferenceEnd::ReferencedArtifactElement,
        "referencedArtifactElement",
    ),
    (ReferenceEnd::GroupMember, "argumentElement"),
    (ReferenceEnd::ArtifactGroupMember, "artifactElement"),
];

/// The asserted relationships of [`in_case`], each with its source and
/// target.
const RELATIONSHIPS: [&str; 5] = ["i1", "e1", "x1", "s1", "ac1"];

// --- Inputs --------------------------------------------------------------------------

/// A named element `id` of `kind` under `argumentElement`, with `attributes`
/// and holding `content` after its name.
fn typed(kind: &str, id: &str, attributes: &str, content: &str) -> String {
    format!(
        "<argumentElement xmi:type=\"sacm:{kind}\" xmi:id=\"{id}\"{attributes}>\
         <name content=\"N\"/>{content}</argumentElement>"
    )
}

/// An AssuranceCasePackage root `case` holding the ArgumentPackage `p` and,
/// after it, the empty ArgumentPackage `q`. `p` holds:
/// - the Claims c1 (its name with the `xmi:id` `c1_name`; holding the
///   Description d1, whose `content` is `d1_content` with the value `d1_v`,
///   the Note n1 and the `TaggedValue` t1, whose `key` is `t1_key`) and c2;
/// - the `ArgumentReasoning`s `why` and `why2`, the `ArtifactReference`s r1 and
///   r2 and the `ArgumentGroup` g1;
/// - the relationships i1 (`AssertedInference`, c2 to c1), e1
///   (`AssertedEvidence`, r1 to c1), x1 (`AssertedContext`, r1 to c2), s1
///   (`AssertedArtifactSupport`, r1 to r2) and ac1 (`AssertedArtifactContext`,
///   r1 to r2);
/// - then `body`.
fn in_case(body: &str) -> String {
    let elements = [
        "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\">\
         <name xmi:id=\"c1_name\" content=\"C1\"/>\
         <description xmi:id=\"d1\"><content xmi:id=\"d1_content\">\
         <value xmi:id=\"d1_v\" content=\"D\"/></content></description>\
         <note xmi:id=\"n1\"/>\
         <taggedValue xmi:id=\"t1\"><key xmi:id=\"t1_key\"><value content=\"K\"/></key>\
         </taggedValue></argumentElement>"
            .to_owned(),
        typed("Claim", "c2", "", ""),
        typed("ArgumentReasoning", "why", "", ""),
        typed("ArgumentReasoning", "why2", "", ""),
        typed("ArtifactReference", "r1", "", ""),
        typed("ArtifactReference", "r2", "", ""),
        typed("ArgumentGroup", "g1", "", ""),
        typed(
            "AssertedInference",
            "i1",
            " source=\"c2\" target=\"c1\"",
            "",
        ),
        typed("AssertedEvidence", "e1", " source=\"r1\" target=\"c1\"", ""),
        typed("AssertedContext", "x1", " source=\"r1\" target=\"c2\"", ""),
        typed(
            "AssertedArtifactSupport",
            "s1",
            " source=\"r1\" target=\"r2\"",
            "",
        ),
        typed(
            "AssertedArtifactContext",
            "ac1",
            " source=\"r1\" target=\"r2\"",
            "",
        ),
    ];
    format!(
        "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\"><name content=\"Case\"/>\
         <argumentPackage xmi:id=\"p\"><name content=\"P\"/>{}{body}</argumentPackage>\
         <argumentPackage xmi:id=\"q\"><name content=\"Q\"/></argumentPackage>\
         </sacm:AssuranceCasePackage>",
        elements.concat()
    )
}

// --- Helpers -------------------------------------------------------------------------

fn id(value: &str) -> ElementId {
    ElementId::from(value)
}

fn ids(values: &[&str]) -> Vec<ElementId> {
    values.iter().map(|value| id(value)).collect()
}

fn named(content: &str) -> LangString {
    LangString::new(content).with_lang("en")
}

fn read(input: &str) -> Document {
    match xmi::read(
        input.as_bytes(),
        &ReadOptions::default().with_mode(Mode::Tolerant),
    ) {
        Ok(output) => output.into_document(),
        Err(error) => panic!("{ROW}: setup: the input does not read: {error}"),
    }
}

fn doc_with(body: &str) -> Document {
    read(&in_case(body))
}

fn write(doc: &Document) -> String {
    match xmi::write(doc, &WriteOptions::default().with_mode(Mode::Tolerant)) {
        Ok(text) => text,
        Err(error) => panic!("{ROW}: the document does not write: {error}"),
    }
}

fn element<'d>(doc: &'d Document, value: &str) -> SacmElement<'d> {
    match doc.get(value) {
        Some(found) => found,
        None => panic!("{ROW}: the document has no element {value:?}"),
    }
}

/// The ids `element` holds in `end`, in order, through the public accessors.
fn held(element: SacmElement<'_>, end: ReferenceEnd) -> Vec<ElementId> {
    fn one(id: Option<&ElementId>) -> Vec<ElementId> {
        id.into_iter().cloned().collect()
    }
    match end {
        ReferenceEnd::CitedElement => one(element.cited_element()),
        ReferenceEnd::AbstractForm => one(element.abstract_form()),
        ReferenceEnd::MetaClaim => element.meta_claims().to_vec(),
        ReferenceEnd::Reasoning => one(element.reasoning()),
        ReferenceEnd::Source => element.sources().to_vec(),
        ReferenceEnd::Target => one(element.target()),
        ReferenceEnd::Structure => one(element.structure()),
        ReferenceEnd::ReferencedArtifactElement => element.referenced_artifact_elements().to_vec(),
        // `group_members()` answers for both group kinds (ADR 0034): each end
        // is the members of its own group kind only.
        ReferenceEnd::GroupMember if element.kind() == ElementKind::ArgumentGroup => {
            element.group_members().to_vec()
        }
        ReferenceEnd::ArtifactGroupMember if element.kind() == ElementKind::ArtifactGroup => {
            element.group_members().to_vec()
        }
        ReferenceEnd::GroupMember | ReferenceEnd::ArtifactGroupMember => Vec::new(),
        other => panic!("{ROW}: an end this test does not know: {other:?}"),
    }
}

fn held_by(doc: &Document, owner: &str, end: ReferenceEnd) -> Vec<ElementId> {
    held(element(doc, owner), end)
}

/// The one `Difference` an edit of `end` on `owner` from `left` to `right`
/// gives.
fn end_difference(
    end: ReferenceEnd,
    owner: &str,
    left: Vec<ElementId>,
    right: Vec<ElementId>,
) -> Difference {
    let one = |ids: Vec<ElementId>| ids.into_iter().next();
    let id = id(owner);
    match end {
        ReferenceEnd::CitedElement => Difference::CitedElement {
            id,
            left: one(left),
            right: one(right),
        },
        ReferenceEnd::AbstractForm => Difference::AbstractForm {
            id,
            left: one(left),
            right: one(right),
        },
        ReferenceEnd::MetaClaim => Difference::MetaClaims { id, left, right },
        ReferenceEnd::Reasoning => Difference::Reasoning {
            id,
            left: one(left),
            right: one(right),
        },
        ReferenceEnd::Source => Difference::Sources { id, left, right },
        ReferenceEnd::Target => Difference::Target {
            id,
            left: one(left),
            right: one(right),
        },
        ReferenceEnd::Structure => Difference::Structure {
            id,
            left: one(left),
            right: one(right),
        },
        ReferenceEnd::ReferencedArtifactElement => {
            Difference::ReferencedArtifactElements { id, left, right }
        }
        ReferenceEnd::GroupMember => Difference::GroupMembers { id, left, right },
        ReferenceEnd::ArtifactGroupMember => Difference::ArtifactGroupMembers { id, left, right },
        other => panic!("{ROW}: an end this test does not know: {other:?}"),
    }
}

fn apply(doc: &mut Document, operation: impl Into<Operation>) -> Mutation {
    let operation = operation.into();
    match edit::apply(doc, operation.clone()) {
        Ok(mutation) => mutation,
        Err(error) => panic!("{ROW}: {operation:?} was refused: {error} ({error:?})"),
    }
}

/// The ids the next generated AssuranceCasePackage, ArgumentPackage and Claim
/// would get, observed on a clone.
fn next_generated_ids(doc: &Document) -> [ElementId; 3] {
    let mut probe = doc.clone();
    let case = apply(&mut probe, CreateAssuranceCasePackage::root(named("probe")));
    let arguments = apply(&mut probe, CreateArgumentPackage::root(named("probe")));
    let arguments = arguments.created_ids()[0].clone();
    let claim = apply(&mut probe, CreateClaim::new(&arguments, named("probe")));
    [
        case.created_ids()[0].clone(),
        arguments,
        claim.created_ids()[0].clone(),
    ]
}

/// The id the next generated Claim under `parent` would get.
fn next_claim_id(doc: &Document, parent: &str) -> ElementId {
    let mut probe = doc.clone();
    apply(&mut probe, CreateClaim::new(parent, named("probe"))).created_ids()[0].clone()
}

/// Applies `operation`, which the test expects to be refused, checks that
/// the document is `==` to before and generates the same next ids, and
/// returns the refusal.
fn refuse(doc: &mut Document, operation: impl Into<Operation>) -> EditError {
    let operation = operation.into();
    let before = doc.clone();
    let next = next_generated_ids(&before);
    let error = match edit::apply(doc, operation.clone()) {
        Ok(mutation) => panic!("{ROW}: {operation:?} was accepted: {mutation:?}"),
        Err(error) => error,
    };
    assert!(
        *doc == before,
        "{ROW}: after refusing {operation:?} ({error:?}) the document is not == to before"
    );
    assert_eq!(
        next_generated_ids(doc),
        next,
        "{ROW}: refusing {operation:?} changed the next generated ids"
    );
    error
}

/// A refusal, in a form the tests compare.
#[derive(Debug, PartialEq)]
enum Refusal {
    ElementNotFound(ElementId),
    FeatureNotApplicable(ElementId, ElementKind, &'static str),
    ReferenceNotHeld(&'static str, ElementId),
    TooFew(&'static str, usize, usize),
    TooMany(&'static str, usize, usize),
    Invalid(&'static str, ElementId),
    NotFound(&'static str, ElementId),
    WrongType(&'static str, ElementId, ElementKind, &'static str),
}

fn refusal_of(error: &EditError) -> Refusal {
    match error {
        EditError::ElementNotFound { element, .. } => Refusal::ElementNotFound(element.clone()),
        EditError::FeatureNotApplicable {
            element,
            kind,
            feature,
            ..
        } => Refusal::FeatureNotApplicable(element.clone(), *kind, feature),
        EditError::ReferenceNotHeld { end, id, .. } => Refusal::ReferenceNotHeld(end, id.clone()),
        EditError::TooFewReferences {
            end,
            given,
            required,
            ..
        } => Refusal::TooFew(end, *given, *required),
        EditError::TooManyReferences {
            end,
            given,
            allowed,
            ..
        } => Refusal::TooMany(end, *given, *allowed),
        EditError::InvalidReference { end, id, .. } => Refusal::Invalid(end, id.clone()),
        EditError::ReferenceNotFound { end, id, .. } => Refusal::NotFound(end, id.clone()),
        EditError::WrongReferenceType {
            end,
            id,
            kind,
            expected,
            ..
        } => Refusal::WrongType(end, id.clone(), *kind, expected),
        other => panic!("{ROW}: an EditError this row does not expect: {other:?}"),
    }
}

/// Refuses `operation` on a fresh [`doc_with`]`(body)` and gives the refusal.
fn refusal(body: &str, operation: impl Into<Operation>) -> Refusal {
    let mut doc = doc_with(body);
    refusal_of(&refuse(&mut doc, operation))
}

/// The `(code, severity, requirement, affected)` of every finding of
/// `validate` with `code`.
fn findings(doc: &Document, code: Code) -> Vec<(Severity, String, Option<String>, Vec<ElementId>)> {
    validation::validate(doc)
        .into_iter()
        .filter(|d| d.code() == code)
        .map(|d| {
            (
                d.severity(),
                d.requirement_id().to_owned(),
                d.clause().map(str::to_owned),
                d.affected().to_vec(),
            )
        })
        .collect()
}

// --- Item 1: every end on every kind that owns it -------------------------------------

/// For each end: the owners in [`in_case`] and the ids to set.
fn owners_and_ids() -> Vec<(ReferenceEnd, Vec<&'static str>, Vec<ElementId>)> {
    let relationships = RELATIONSHIPS.to_vec();
    let mut assertions = vec!["c1", "c2"];
    assertions.extend(RELATIONSHIPS);
    let everything = vec![
        "case", "p", "c1", "why", "r1", "g1", "i1", "e1", "d1", "n1", "t1",
    ];
    vec![
        (ReferenceEnd::CitedElement, everything.clone(), ids(&["c2"])),
        (ReferenceEnd::AbstractForm, everything, ids(&["c2"])),
        (
            ReferenceEnd::MetaClaim,
            assertions,
            ids(&["c2", "c1", "c2"]),
        ),
        (
            ReferenceEnd::Reasoning,
            relationships.clone(),
            ids(&["why2"]),
        ),
        (
            ReferenceEnd::Source,
            relationships.clone(),
            ids(&["c2", "r2", "c2"]),
        ),
        (ReferenceEnd::Target, relationships, ids(&["why"])),
        (ReferenceEnd::Structure, vec!["why"], ids(&["q"])),
        (
            ReferenceEnd::ReferencedArtifactElement,
            vec!["r1"],
            ids(&["c1", "case", "c1"]),
        ),
        (
            ReferenceEnd::GroupMember,
            vec!["g1"],
            ids(&["c1", "why", "c1"]),
        ),
    ]
}

#[test]
fn lib_edit_009_setting_each_end_on_each_owner_kind_holds_exactly_the_ids_given() {
    for (end, owners, given) in owners_and_ids() {
        for owner in owners {
            let mut doc = doc_with("");
            let before = doc.clone();
            let old = held_by(&before, owner, end);
            let mutation = apply(&mut doc, SetReferences::new(owner, end, given.clone()));
            assert_eq!(
                held_by(&doc, owner, end),
                given,
                "{ROW}: {} of {owner:?} after SetReferences",
                end.name()
            );
            assert_eq!(
                semantic_compare(&before, &doc),
                [end_difference(end, owner, old, given.clone())],
                "{ROW}: SetReferences {} on {owner:?} changed more than that end",
                end.name()
            );
            assert_eq!(
                mutation.changed_ids(),
                [id(owner)],
                "{ROW}: the changed ids of SetReferences {} on {owner:?}",
                end.name()
            );
            let back = read(&write(&doc));
            assert_eq!(
                back,
                doc,
                "{ROW}: {} of {owner:?} does not read back equal",
                end.name()
            );
        }
    }
}

#[test]
fn lib_edit_009_clear_empties_an_end_and_holds_nothing() {
    let mut doc = doc_with(&typed(
        "Claim",
        "c9",
        " citedElement=\"c2\" abstractForm=\"c2\" metaClaim=\"c1 c2\"",
        "",
    ));
    for end in [
        ReferenceEnd::CitedElement,
        ReferenceEnd::AbstractForm,
        ReferenceEnd::MetaClaim,
    ] {
        let mutation = apply(&mut doc, SetReferences::clear("c9", end));
        assert_eq!(
            (held_by(&doc, "c9", end), mutation.changed_ids()),
            (Vec::new(), [id("c9")].as_slice()),
            "{ROW}: clearing {} of c9",
            end.name()
        );
    }
    for (end, value) in [
        (ReferenceEnd::Reasoning, "why"),
        (ReferenceEnd::Structure, "p"),
        (ReferenceEnd::ReferencedArtifactElement, "c1"),
        (ReferenceEnd::GroupMember, "c1"),
    ] {
        let owner = match end {
            ReferenceEnd::Reasoning => "i1",
            ReferenceEnd::Structure => "why",
            ReferenceEnd::ReferencedArtifactElement => "r1",
            _ => "g1",
        };
        apply(&mut doc, SetReferences::new(owner, end, [value]));
        apply(&mut doc, SetReferences::clear(owner, end));
        assert_eq!(
            held_by(&doc, owner, end),
            [] as [ElementId; 0],
            "{ROW}: clearing {} of {owner}",
            end.name()
        );
    }
    let written = write(&doc);
    assert!(
        !written.contains("citedElement=") && !written.contains("metaClaim="),
        "{ROW}: a cleared end is still written: {written}"
    );
}

// --- Item 2: add and remove ------------------------------------------------------------

#[test]
fn lib_edit_009_add_appends_and_an_id_already_held_is_kept_with_a_repeat_warning() {
    let mut doc = doc_with("");
    apply(
        &mut doc,
        AddReference::new("c1", ReferenceEnd::MetaClaim, "c2"),
    );
    apply(
        &mut doc,
        AddReference::new("c1", ReferenceEnd::MetaClaim, "c1"),
    );
    assert_eq!(
        held_by(&doc, "c1", ReferenceEnd::MetaClaim),
        ids(&["c2", "c1"]),
        "{ROW}: AddReference does not append"
    );
    assert_eq!(
        findings(&doc, Code::MULTIPLICITY_VIOLATION)
            .into_iter()
            .filter(|f| f.1 == "SACM23-VAL-022")
            .count(),
        0,
        "{ROW}: setup: a repeat before one is added"
    );
    let mutation = apply(
        &mut doc,
        AddReference::new("c1", ReferenceEnd::MetaClaim, "c2"),
    );
    assert_eq!(mutation.changed_ids(), [id("c1")], "{ROW}: the changed ids");
    assert_eq!(
        held_by(&doc, "c1", ReferenceEnd::MetaClaim),
        ids(&["c2", "c1", "c2"]),
        "{ROW}: an id already held is not appended again"
    );
    let repeats: Vec<_> = findings(&doc, Code::MULTIPLICITY_VIOLATION)
        .into_iter()
        .filter(|f| f.1 == "SACM23-VAL-022")
        .collect();
    assert_eq!(
        repeats,
        [(
            Severity::Warning,
            "SACM23-VAL-022".to_owned(),
            Some("11.10".to_owned()),
            ids(&["c1", "c2"])
        )],
        "{ROW}: the repeat is not one SACM23-VAL-022 Warning"
    );
    // Sources append too.
    apply(
        &mut doc,
        AddReference::new("i1", ReferenceEnd::Source, "r1"),
    );
    assert_eq!(
        held_by(&doc, "i1", ReferenceEnd::Source),
        ids(&["c2", "r1"]),
        "{ROW}: AddReference does not append a source"
    );
    // A single-valued end that holds nothing takes one id.
    apply(
        &mut doc,
        AddReference::new("c2", ReferenceEnd::CitedElement, "c1"),
    );
    assert_eq!(
        held_by(&doc, "c2", ReferenceEnd::CitedElement),
        ids(&["c1"]),
        "{ROW}: AddReference to an empty single-valued end"
    );
}

#[test]
fn lib_edit_009_remove_drops_every_occurrence() {
    let mut doc = doc_with(&typed("Claim", "c9", " metaClaim=\"c1 c2 c1 c1\"", ""));
    let mutation = apply(
        &mut doc,
        RemoveReference::new("c9", ReferenceEnd::MetaClaim, "c1"),
    );
    assert_eq!(
        held_by(&doc, "c9", ReferenceEnd::MetaClaim),
        ids(&["c2"]),
        "{ROW}: RemoveReference does not drop every occurrence"
    );
    assert_eq!(mutation.changed_ids(), [id("c9")], "{ROW}: the changed ids");
    apply(
        &mut doc,
        RemoveReference::new("c9", ReferenceEnd::MetaClaim, "c2"),
    );
    assert_eq!(
        held_by(&doc, "c9", ReferenceEnd::MetaClaim),
        [] as [ElementId; 0],
        "{ROW}: removing the last id"
    );
    // A source among others.
    apply(
        &mut doc,
        SetReferences::new("i1", ReferenceEnd::Source, ["c2", "r1", "c2"]),
    );
    apply(
        &mut doc,
        RemoveReference::new("i1", ReferenceEnd::Source, "c2"),
    );
    assert_eq!(
        held_by(&doc, "i1", ReferenceEnd::Source),
        ids(&["r1"]),
        "{ROW}: removing a repeated source"
    );
}

#[test]
fn lib_edit_009_removing_an_id_the_end_does_not_hold_is_reference_not_held() {
    for (owner, end, end_name, value) in [
        ("c1", ReferenceEnd::MetaClaim, "metaClaim", "c2"),
        ("i1", ReferenceEnd::Source, "source", "r1"),
        ("i1", ReferenceEnd::Target, "target", "c2"),
        ("i1", ReferenceEnd::Reasoning, "reasoning", "why"),
        ("c1", ReferenceEnd::CitedElement, "citedElement", "nowhere"),
        ("g1", ReferenceEnd::GroupMember, "argumentElement", "c1"),
    ] {
        let mut doc = doc_with("");
        let error = refuse(&mut doc, RemoveReference::new(owner, end, value));
        assert_eq!(
            refusal_of(&error),
            Refusal::ReferenceNotHeld(end_name, id(value)),
            "{ROW}: removing {value:?} from {end_name} of {owner:?}"
        );
        assert_eq!(error.code(), Code::CMD_TARGET_NOT_FOUND, "{ROW}: the code");
    }
    let mut doc = doc_with("");
    let error = refuse(
        &mut doc,
        RemoveReference::new("i1", ReferenceEnd::Source, "x"),
    );
    assert_eq!(
        error.to_string(),
        "the source holds no id \"x\" to remove",
        "{ROW}: the Display of ReferenceNotHeld"
    );
}

// --- Item 3: the bounds ------------------------------------------------------------------

#[test]
fn lib_edit_009_no_sources_or_removing_the_last_source_is_too_few_references() {
    for relationship in RELATIONSHIPS {
        assert_eq!(
            refusal("", SetReferences::clear(relationship, ReferenceEnd::Source)),
            Refusal::TooFew("source", 0, 1),
            "{ROW}: clearing the sources of {relationship:?}"
        );
        assert_eq!(
            refusal(
                "",
                SetReferences::new(relationship, ReferenceEnd::Source, Vec::<ElementId>::new())
            ),
            Refusal::TooFew("source", 0, 1),
            "{ROW}: setting no sources on {relationship:?}"
        );
        let only = held_by(&doc_with(""), relationship, ReferenceEnd::Source);
        assert_eq!(
            only.len(),
            1,
            "{ROW}: setup: {relationship:?} has one source"
        );
        assert_eq!(
            refusal(
                "",
                RemoveReference::new(relationship, ReferenceEnd::Source, only[0].clone())
            ),
            Refusal::TooFew("source", 0, 1),
            "{ROW}: removing the last source of {relationship:?}"
        );
    }
    // A source held twice is still the last source.
    let mut doc = doc_with("");
    apply(
        &mut doc,
        SetReferences::new("i1", ReferenceEnd::Source, ["c2", "c2"]),
    );
    let error = refuse(
        &mut doc,
        RemoveReference::new("i1", ReferenceEnd::Source, "c2"),
    );
    assert_eq!(
        refusal_of(&error),
        Refusal::TooFew("source", 0, 1),
        "{ROW}: removing a source held twice and nothing else"
    );
    assert_eq!(
        error.code(),
        Code::MULTIPLICITY_VIOLATION,
        "{ROW}: the code"
    );
}

#[test]
fn lib_edit_009_the_target_cannot_be_cleared() {
    for relationship in RELATIONSHIPS {
        assert_eq!(
            refusal("", SetReferences::clear(relationship, ReferenceEnd::Target)),
            Refusal::TooFew("target", 0, 1),
            "{ROW}: clearing the target of {relationship:?}"
        );
        let target = held_by(&doc_with(""), relationship, ReferenceEnd::Target);
        assert_eq!(
            refusal(
                "",
                RemoveReference::new(relationship, ReferenceEnd::Target, target[0].clone())
            ),
            Refusal::TooFew("target", 0, 1),
            "{ROW}: removing the target of {relationship:?}"
        );
    }
}

#[test]
fn lib_edit_009_two_ids_on_a_single_valued_end_or_adding_to_a_held_target_is_too_many_references() {
    for (owner, end, name) in [
        ("i1", ReferenceEnd::Target, "target"),
        ("i1", ReferenceEnd::Reasoning, "reasoning"),
        ("why", ReferenceEnd::Structure, "structure"),
        ("c1", ReferenceEnd::CitedElement, "citedElement"),
        ("n1", ReferenceEnd::AbstractForm, "abstractForm"),
    ] {
        let two = match end {
            ReferenceEnd::Reasoning => ["why", "why2"],
            ReferenceEnd::Structure => ["p", "q"],
            _ => ["c1", "c2"],
        };
        assert_eq!(
            refusal("", SetReferences::new(owner, end, two)),
            Refusal::TooMany(name, 2, 1),
            "{ROW}: two ids on {name} of {owner:?}"
        );
        // The same id twice is two ids.
        assert_eq!(
            refusal("", SetReferences::new(owner, end, [two[0], two[0]])),
            Refusal::TooMany(name, 2, 1),
            "{ROW}: one id twice on {name} of {owner:?}"
        );
    }
    let mut doc = doc_with("");
    let error = refuse(
        &mut doc,
        AddReference::new("i1", ReferenceEnd::Target, "c2"),
    );
    assert_eq!(
        refusal_of(&error),
        Refusal::TooMany("target", 2, 1),
        "{ROW}: adding to a held target"
    );
    assert_eq!(
        error.code(),
        Code::MULTIPLICITY_VIOLATION,
        "{ROW}: the code"
    );
    assert_eq!(
        error.to_string(),
        "the target is given 2 ids, and SACM allows at most 1",
        "{ROW}: the Display of TooManyReferences"
    );
    // Adding the id already held is still a second id.
    assert_eq!(
        refusal("", AddReference::new("i1", ReferenceEnd::Target, "c1")),
        Refusal::TooMany("target", 2, 1),
        "{ROW}: adding the held target again"
    );
    apply(
        &mut doc,
        SetReferences::new("i1", ReferenceEnd::Reasoning, ["why"]),
    );
    assert_eq!(
        refusal_of(&refuse(
            &mut doc,
            AddReference::new("i1", ReferenceEnd::Reasoning, "why2")
        )),
        Refusal::TooMany("reasoning", 2, 1),
        "{ROW}: adding to a held reasoning"
    );
}

#[test]
fn lib_edit_009_the_multi_valued_ends_take_any_number_of_ids() {
    let mut doc = doc_with("");
    let many: Vec<ElementId> = ids(&["c1", "c2", "c1", "c2", "c1"]);
    for (owner, end) in [
        ("c1", ReferenceEnd::MetaClaim),
        ("i1", ReferenceEnd::Source),
        ("r1", ReferenceEnd::ReferencedArtifactElement),
        ("g1", ReferenceEnd::GroupMember),
    ] {
        apply(&mut doc, SetReferences::new(owner, end, many.clone()));
        assert_eq!(
            held_by(&doc, owner, end),
            many,
            "{ROW}: {} of {owner}",
            end.name()
        );
    }
}

// --- Item 4: the ids given ---------------------------------------------------------------

#[test]
fn lib_edit_009_an_absent_id_or_the_id_of_a_name_content_key_or_value_is_reference_not_found() {
    for missing in ["nowhere", "c1_name", "d1_content", "d1_v", "t1_key", "C1"] {
        for (owner, end, name) in [
            ("c2", ReferenceEnd::MetaClaim, "metaClaim"),
            ("i1", ReferenceEnd::Source, "source"),
            ("i1", ReferenceEnd::Target, "target"),
            ("c2", ReferenceEnd::CitedElement, "citedElement"),
            ("g1", ReferenceEnd::GroupMember, "argumentElement"),
        ] {
            assert_eq!(
                refusal("", SetReferences::new(owner, end, [missing])),
                Refusal::NotFound(name, id(missing)),
                "{ROW}: setting {missing:?} as {name} of {owner:?}"
            );
            if end != ReferenceEnd::Target {
                assert_eq!(
                    refusal("", AddReference::new(owner, end, missing)),
                    Refusal::NotFound(name, id(missing)),
                    "{ROW}: adding {missing:?} to {name} of {owner:?}"
                );
            }
        }
    }
    let mut doc = doc_with("");
    let error = refuse(
        &mut doc,
        SetReferences::new("c2", ReferenceEnd::MetaClaim, ["nowhere"]),
    );
    assert_eq!(error.code(), Code::CMD_TARGET_NOT_FOUND, "{ROW}: the code");
}

#[test]
fn lib_edit_009_an_id_holding_white_space_is_invalid_reference() {
    let body = typed("Claim", "a b", "", "");
    for (owner, end, name) in [
        ("c2", ReferenceEnd::MetaClaim, "metaClaim"),
        ("i1", ReferenceEnd::Source, "source"),
        ("c2", ReferenceEnd::CitedElement, "citedElement"),
    ] {
        assert_eq!(
            refusal(&body, SetReferences::new(owner, end, ["a b"])),
            Refusal::Invalid(name, id("a b")),
            "{ROW}: setting \"a b\" as {name} of {owner:?}"
        );
        assert_eq!(
            refusal(&body, AddReference::new(owner, end, "a b")),
            Refusal::Invalid(name, id("a b")),
            "{ROW}: adding \"a b\" to {name} of {owner:?}"
        );
    }
    for spaced in ["a\tb", "a\nb", "a\rb", " c1"] {
        assert_eq!(
            refusal(
                "",
                SetReferences::new("c2", ReferenceEnd::MetaClaim, [spaced])
            ),
            Refusal::Invalid("metaClaim", id(spaced)),
            "{ROW}: setting {spaced:?} as a metaClaim"
        );
    }
    let mut doc = doc_with(&body);
    let error = refuse(
        &mut doc,
        AddReference::new("i1", ReferenceEnd::Source, "a b"),
    );
    assert_eq!(error.code(), Code::ID_INVALID, "{ROW}: the code");
}

#[test]
fn lib_edit_009_a_wrongly_typed_id_is_wrong_reference_type() {
    for (owner, end, name, value, kind, expected) in [
        (
            "c2",
            ReferenceEnd::MetaClaim,
            "metaClaim",
            "why",
            ElementKind::ArgumentReasoning,
            "Claim",
        ),
        (
            "i1",
            ReferenceEnd::Source,
            "source",
            "p",
            ElementKind::ArgumentPackage,
            "ArgumentAsset",
        ),
        (
            "i1",
            ReferenceEnd::Target,
            "target",
            "g1",
            ElementKind::ArgumentGroup,
            "ArgumentAsset",
        ),
        (
            "i1",
            ReferenceEnd::Reasoning,
            "reasoning",
            "c1",
            ElementKind::Claim,
            "ArgumentReasoning",
        ),
        (
            "why",
            ReferenceEnd::Structure,
            "structure",
            "c1",
            ElementKind::Claim,
            "ArgumentPackage",
        ),
        (
            "r1",
            ReferenceEnd::ReferencedArtifactElement,
            "referencedArtifactElement",
            "d1",
            ElementKind::Description,
            "ArtifactElement",
        ),
        (
            "g1",
            ReferenceEnd::GroupMember,
            "argumentElement",
            "n1",
            ElementKind::Note,
            "ArgumentationElement",
        ),
    ] {
        let wrong = Refusal::WrongType(name, id(value), kind, expected);
        assert_eq!(
            refusal("", SetReferences::new(owner, end, [value])),
            wrong,
            "{ROW}: setting the {kind} {value:?} as {name} of {owner:?}"
        );
        if end != ReferenceEnd::Target {
            assert_eq!(
                refusal("", AddReference::new(owner, end, value)),
                wrong,
                "{ROW}: adding the {kind} {value:?} to {name} of {owner:?}"
            );
        }
    }
    let mut doc = doc_with("");
    let error = refuse(
        &mut doc,
        SetReferences::new("why", ReferenceEnd::Structure, ["c1"]),
    );
    assert_eq!(error.code(), Code::REF_WRONG_TYPE, "{ROW}: the code");
}

#[test]
fn lib_edit_009_a_cited_element_or_abstract_form_naming_a_utility_element_is_accepted() {
    for value in ["d1", "n1", "t1"] {
        for end in [ReferenceEnd::CitedElement, ReferenceEnd::AbstractForm] {
            let mut doc = doc_with("");
            apply(&mut doc, SetReferences::new("c2", end, [value]));
            assert_eq!(
                held_by(&doc, "c2", end),
                ids(&[value]),
                "{ROW}: {} naming {value:?}",
                end.name()
            );
        }
    }
}

// --- Item 5: a kind without the end, and the order of the checks ----------------------

#[test]
fn lib_edit_009_an_end_on_a_kind_that_lacks_it_is_feature_not_applicable() {
    for (owner, kind, end, name) in [
        ("c1", ElementKind::Claim, ReferenceEnd::Source, "source"),
        ("c1", ElementKind::Claim, ReferenceEnd::Target, "target"),
        (
            "c1",
            ElementKind::Claim,
            ReferenceEnd::Reasoning,
            "reasoning",
        ),
        (
            "c1",
            ElementKind::Claim,
            ReferenceEnd::Structure,
            "structure",
        ),
        (
            "c1",
            ElementKind::Claim,
            ReferenceEnd::ReferencedArtifactElement,
            "referencedArtifactElement",
        ),
        (
            "c1",
            ElementKind::Claim,
            ReferenceEnd::GroupMember,
            "argumentElement",
        ),
        (
            "why",
            ElementKind::ArgumentReasoning,
            ReferenceEnd::MetaClaim,
            "metaClaim",
        ),
        (
            "p",
            ElementKind::ArgumentPackage,
            ReferenceEnd::GroupMember,
            "argumentElement",
        ),
        (
            "n1",
            ElementKind::Note,
            ReferenceEnd::MetaClaim,
            "metaClaim",
        ),
        (
            "i1",
            ElementKind::AssertedInference,
            ReferenceEnd::Structure,
            "structure",
        ),
    ] {
        let expected = Refusal::FeatureNotApplicable(id(owner), kind, name);
        assert_eq!(
            refusal("", SetReferences::new(owner, end, ["c2"])),
            expected,
            "{ROW}: setting {name} on the {kind}"
        );
        assert_eq!(
            refusal("", AddReference::new(owner, end, "c2")),
            expected,
            "{ROW}: adding to {name} on the {kind}"
        );
        assert_eq!(
            refusal("", RemoveReference::new(owner, end, "c2")),
            expected,
            "{ROW}: removing from {name} on the {kind}"
        );
    }
    let mut doc = doc_with("");
    let error = refuse(
        &mut doc,
        SetReferences::new("c1", ReferenceEnd::Source, ["c2"]),
    );
    assert_eq!(
        error.code(),
        Code::CMD_FEATURE_NOT_APPLICABLE,
        "{ROW}: the code"
    );
    assert_eq!(
        error.to_string(),
        "the Claim \"c1\" has no source",
        "{ROW}: the Display of FeatureNotApplicable"
    );
}

#[test]
fn lib_edit_009_an_unknown_element_is_element_not_found() {
    for operation in [
        Operation::from(SetReferences::new("nowhere", ReferenceEnd::Source, ["c2"])),
        SetReferences::clear("nowhere", ReferenceEnd::Target).into(),
        AddReference::new("nowhere", ReferenceEnd::MetaClaim, "c2").into(),
        RemoveReference::new("nowhere", ReferenceEnd::CitedElement, "c2").into(),
    ] {
        assert_eq!(
            refusal("", operation.clone()),
            Refusal::ElementNotFound(id("nowhere")),
            "{ROW}: {operation:?}"
        );
    }
}

#[test]
fn lib_edit_009_an_operation_failing_two_checks_gets_the_first() {
    let spaced = typed("Claim", "a b", "", "");
    let cases: Vec<(&str, Operation, Refusal)> = vec![
        (
            "an unknown element before the end",
            SetReferences::clear("nowhere", ReferenceEnd::Source).into(),
            Refusal::ElementNotFound(id("nowhere")),
        ),
        (
            "an unknown element before a bad id",
            AddReference::new("nowhere", ReferenceEnd::Source, "a b").into(),
            Refusal::ElementNotFound(id("nowhere")),
        ),
        (
            "a kind without the end before the bounds",
            SetReferences::clear("c1", ReferenceEnd::Source).into(),
            Refusal::FeatureNotApplicable(id("c1"), ElementKind::Claim, "source"),
        ),
        (
            "a kind without the end before an id not held",
            RemoveReference::new("c1", ReferenceEnd::Target, "nowhere").into(),
            Refusal::FeatureNotApplicable(id("c1"), ElementKind::Claim, "target"),
        ),
        (
            "an id not held before the bounds",
            RemoveReference::new("i1", ReferenceEnd::Target, "c2").into(),
            Refusal::ReferenceNotHeld("target", id("c2")),
        ),
        (
            "too many before white space",
            SetReferences::new("i1", ReferenceEnd::Target, ["a b", "nowhere"]).into(),
            Refusal::TooMany("target", 2, 1),
        ),
        (
            "too many before white space, adding",
            AddReference::new("i1", ReferenceEnd::Target, "a b").into(),
            Refusal::TooMany("target", 2, 1),
        ),
        (
            "white space before an id naming no element",
            SetReferences::new("i1", ReferenceEnd::Source, ["nowhere", "a b"]).into(),
            Refusal::Invalid("source", id("a b")),
        ),
        (
            "an id naming no element before a wrong type",
            SetReferences::new("i1", ReferenceEnd::Source, ["p", "nowhere"]).into(),
            Refusal::NotFound("source", id("nowhere")),
        ),
        (
            "the first wrong type in list order",
            SetReferences::new("i1", ReferenceEnd::Source, ["c1", "q", "p"]).into(),
            Refusal::WrongType(
                "source",
                id("q"),
                ElementKind::ArgumentPackage,
                "ArgumentAsset",
            ),
        ),
    ];
    for (what, operation, expected) in cases {
        assert_eq!(
            refusal(&spaced, operation.clone()),
            expected,
            "{ROW}: {what}: {operation:?}"
        );
    }
}

// --- Item 6: the repair exception, per end ------------------------------------------

#[test]
fn lib_edit_009_a_held_dangling_id_is_kept_and_a_new_one_refused() {
    let body = typed("AssertedInference", "i9", " source=\"x\" target=\"c1\"", "");
    let mut doc = doc_with(&body);
    let dangling = |doc: &Document| -> Vec<Vec<ElementId>> {
        findings(doc, Code::REF_DANGLING)
            .into_iter()
            .map(|finding| finding.3)
            .filter(|affected| affected.first() == Some(&id("i9")))
            .collect()
    };
    assert_eq!(
        dangling(&doc),
        [ids(&["i9", "x"])],
        "{ROW}: setup: i9's source x is not dangling"
    );
    apply(
        &mut doc,
        SetReferences::new("i9", ReferenceEnd::Source, ["x", "c2"]),
    );
    assert_eq!(
        held_by(&doc, "i9", ReferenceEnd::Source),
        ids(&["x", "c2"]),
        "{ROW}: the held dangling source is not kept"
    );
    // Adding the held id again is accepted too.
    let mut added = doc.clone();
    apply(
        &mut added,
        AddReference::new("i9", ReferenceEnd::Source, "x"),
    );
    assert_eq!(
        held_by(&added, "i9", ReferenceEnd::Source),
        ids(&["x", "c2", "x"]),
        "{ROW}: adding a held dangling id"
    );
    assert_eq!(
        refusal_of(&refuse(
            &mut doc,
            SetReferences::new("i9", ReferenceEnd::Source, ["y"])
        )),
        Refusal::NotFound("source", id("y")),
        "{ROW}: a new dangling source"
    );
    // The exception is per end: the target does not hold x.
    assert_eq!(
        refusal_of(&refuse(
            &mut doc,
            SetReferences::new("i9", ReferenceEnd::Target, ["x"])
        )),
        Refusal::NotFound("target", id("x")),
        "{ROW}: the source's dangling id as the target"
    );
    assert_eq!(
        refusal_of(&refuse(
            &mut doc,
            SetReferences::new("i1", ReferenceEnd::Source, ["x"])
        )),
        Refusal::NotFound("source", id("x")),
        "{ROW}: i9's dangling id on another element"
    );
    apply(
        &mut doc,
        RemoveReference::new("i9", ReferenceEnd::Source, "x"),
    );
    assert_eq!(
        held_by(&doc, "i9", ReferenceEnd::Source),
        ids(&["c2"]),
        "{ROW}: removing the dangling source"
    );
    assert_eq!(
        dangling(&doc),
        [] as [Vec<ElementId>; 0],
        "{ROW}: its SACM-REF-001 stays"
    );
}

#[test]
fn lib_edit_009_a_held_wrongly_typed_id_is_kept_and_a_new_one_refused() {
    let body = typed("AssertedInference", "i9", " source=\"p\" target=\"c1\"", "");
    let mut doc = doc_with(&body);
    let wrong = |doc: &Document| -> Vec<Vec<ElementId>> {
        findings(doc, Code::REF_WRONG_TYPE)
            .into_iter()
            .filter(|finding| finding.1 == "SACM23-VAL-004")
            .map(|finding| finding.3)
            .filter(|affected| affected.first() == Some(&id("i9")))
            .collect()
    };
    assert_eq!(
        wrong(&doc),
        [ids(&["i9", "p"])],
        "{ROW}: setup: i9's source p is not wrongly typed"
    );
    apply(
        &mut doc,
        SetReferences::new("i9", ReferenceEnd::Source, ["p", "c2"]),
    );
    assert_eq!(
        held_by(&doc, "i9", ReferenceEnd::Source),
        ids(&["p", "c2"]),
        "{ROW}: the held wrongly typed source is not kept"
    );
    assert_eq!(
        refusal_of(&refuse(
            &mut doc,
            SetReferences::new("i9", ReferenceEnd::Source, ["p", "q"])
        )),
        Refusal::WrongType(
            "source",
            id("q"),
            ElementKind::ArgumentPackage,
            "ArgumentAsset"
        ),
        "{ROW}: a new wrongly typed source"
    );
    assert_eq!(
        refusal_of(&refuse(
            &mut doc,
            SetReferences::new("i9", ReferenceEnd::Target, ["p"])
        )),
        Refusal::WrongType(
            "target",
            id("p"),
            ElementKind::ArgumentPackage,
            "ArgumentAsset"
        ),
        "{ROW}: the source's wrongly typed id as the target"
    );
    apply(
        &mut doc,
        RemoveReference::new("i9", ReferenceEnd::Source, "p"),
    );
    assert_eq!(
        wrong(&doc),
        [] as [Vec<ElementId>; 0],
        "{ROW}: its SACM-REF-002 stays"
    );
}

// --- Item 7: content rules are not refused -----------------------------------------

#[test]
fn lib_edit_009_one_end_is_edited_while_another_dangles() {
    let body = typed("AssertedInference", "i9", " source=\"c2\" target=\"x\"", "");
    let mut doc = doc_with(&body);
    apply(
        &mut doc,
        SetReferences::new("i9", ReferenceEnd::Reasoning, ["why"]),
    );
    apply(
        &mut doc,
        AddReference::new("i9", ReferenceEnd::Source, "c1"),
    );
    apply(
        &mut doc,
        AddReference::new("i9", ReferenceEnd::MetaClaim, "c1"),
    );
    let i9 = element(&doc, "i9");
    assert_eq!(
        (
            i9.reasoning().map(ElementId::as_str),
            i9.sources().len(),
            i9.target().map(ElementId::as_str)
        ),
        (Some("why"), 2, Some("x")),
        "{ROW}: editing ends beside a dangling target"
    );
}

#[test]
fn lib_edit_009_an_asserted_evidence_source_set_to_a_claim_is_accepted_and_validate_reports_it() {
    let mut doc = doc_with("");
    apply(
        &mut doc,
        SetReferences::new("e1", ReferenceEnd::Source, ["c2"]),
    );
    let narrowed: Vec<_> = findings(&doc, Code::REF_WRONG_TYPE)
        .into_iter()
        .filter(|finding| finding.1 == "SACM23-VAL-016")
        .collect();
    assert_eq!(
        narrowed,
        [(
            Severity::Error,
            "SACM23-VAL-016".to_owned(),
            Some("11.15".to_owned()),
            ids(&["e1", "c2"])
        )],
        "{ROW}: a Claim source of an AssertedEvidence is not one §11.15 finding"
    );
}

#[test]
fn lib_edit_009_self_references_are_accepted() {
    let mut doc = doc_with("");
    for (owner, end) in [
        ("c1", ReferenceEnd::CitedElement),
        ("c1", ReferenceEnd::AbstractForm),
        ("c1", ReferenceEnd::MetaClaim),
        ("i1", ReferenceEnd::Source),
        ("i1", ReferenceEnd::Target),
        ("r1", ReferenceEnd::ReferencedArtifactElement),
        ("g1", ReferenceEnd::GroupMember),
    ] {
        apply(&mut doc, SetReferences::new(owner, end, [owner]));
        assert_eq!(
            held_by(&doc, owner, end),
            ids(&[owner]),
            "{ROW}: {owner:?} naming itself as {}",
            end.name()
        );
    }
}

// --- Item 8: ids freed for generation ----------------------------------------------

/// An ArgumentPackage root `p` holding the Claim `claim_1` and `body`.
fn generation_input(body: &str) -> String {
    format!(
        "<sacm:ArgumentPackage{NS} xmi:id=\"p\"><name content=\"P\"/>{}{body}</sacm:ArgumentPackage>",
        typed("Claim", "claim_1", "", "")
    )
}

#[test]
fn lib_edit_009_a_dangling_id_is_skipped_until_its_last_reference_goes() {
    let input = generation_input(&typed("Claim", "c1", " citedElement=\"claim_2\"", ""));
    let doc = read(&input);
    assert_eq!(
        next_claim_id(&doc, "p"),
        id("claim_3"),
        "{ROW}: generation does not skip the referenced claim_2"
    );
    // Cleared, replaced or removed: each frees it.
    let freeing: [Operation; 3] = [
        SetReferences::clear("c1", ReferenceEnd::CitedElement).into(),
        SetReferences::new("c1", ReferenceEnd::CitedElement, ["claim_1"]).into(),
        RemoveReference::new("c1", ReferenceEnd::CitedElement, "claim_2").into(),
    ];
    for operation in freeing {
        let mut edited = doc.clone();
        apply(&mut edited, operation.clone());
        assert_eq!(
            next_claim_id(&edited, "p"),
            id("claim_2"),
            "{ROW}: after {operation:?} the freed claim_2 is not generated"
        );
        // Deterministic: a clone generates the same.
        assert_eq!(
            next_claim_id(&edited.clone(), "p"),
            id("claim_2"),
            "{ROW}: a clone generates another id"
        );
    }
}

#[test]
fn lib_edit_009_a_freed_dangling_id_at_or_below_the_counter_is_not_generated() {
    // c1 references the absent claim_2, so the first two generated Claims
    // are claim_1 and claim_3 and the Claim counter stands at 3.
    let input = format!(
        "<sacm:ArgumentPackage{NS} xmi:id=\"p\"><name content=\"P\"/>{}</sacm:ArgumentPackage>",
        typed("Claim", "c1", " citedElement=\"claim_2\"", "")
    );
    let mut doc = read(&input);
    let generated: Vec<ElementId> = ["a", "b"]
        .into_iter()
        .map(|name| apply(&mut doc, CreateClaim::new("p", named(name))).created_ids()[0].clone())
        .collect();
    assert_eq!(
        generated,
        ids(&["claim_1", "claim_3"]),
        "{ROW}: setup: generation does not skip the referenced claim_2"
    );
    apply(
        &mut doc,
        RemoveReference::new("c1", ReferenceEnd::CitedElement, "claim_2"),
    );
    assert_eq!(
        held_by(&doc, "c1", ReferenceEnd::CitedElement),
        Vec::<ElementId>::new(),
        "{ROW}: setup: c1 still references claim_2"
    );
    let next = apply(&mut doc, CreateClaim::new("p", named("c")));
    assert_eq!(
        next.created_ids(),
        ids(&["claim_4"]),
        "{ROW}: the freed claim_2, below the Claim counter, is generated again"
    );
}

#[test]
fn lib_edit_009_a_dangling_id_held_by_two_references_is_freed_only_by_the_last() {
    let input = generation_input(&format!(
        "{}{}",
        typed("Claim", "c1", " metaClaim=\"claim_2 claim_2\"", ""),
        typed("Claim", "c2", " citedElement=\"claim_2\"", "")
    ));
    let mut doc = read(&input);
    apply(
        &mut doc,
        RemoveReference::new("c1", ReferenceEnd::MetaClaim, "claim_2"),
    );
    assert_eq!(
        next_claim_id(&doc, "p"),
        id("claim_3"),
        "{ROW}: claim_2 is freed while c2 still references it"
    );
    apply(
        &mut doc,
        SetReferences::clear("c2", ReferenceEnd::CitedElement),
    );
    assert_eq!(
        next_claim_id(&doc, "p"),
        id("claim_2"),
        "{ROW}: claim_2 is not freed once its last reference goes"
    );
}

#[test]
fn lib_edit_009_removing_the_last_reference_to_a_name_id_keeps_it_in_use() {
    // The name of c9 carries the xmi:id claim_2, and c1 references it.
    let input = generation_input(&format!(
        "{}<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c9\">\
         <name xmi:id=\"claim_2\" content=\"N\"/></argumentElement>",
        typed("Claim", "c1", " citedElement=\"claim_2\"", "")
    ));
    let mut doc = read(&input);
    apply(
        &mut doc,
        SetReferences::clear("c1", ReferenceEnd::CitedElement),
    );
    assert_eq!(
        next_claim_id(&doc, "p"),
        id("claim_3"),
        "{ROW}: a name's id is generated once no reference holds it"
    );
    let error = refuse(
        &mut doc,
        CreateClaim::new("p", named("C")).with_id("claim_2"),
    );
    assert_eq!(
        error.code(),
        Code::CMD_DUPLICATE_ID,
        "{ROW}: the name's id is free for a caller: {error:?}"
    );
}

#[test]
fn lib_edit_009_a_new_reference_to_an_element_id_does_not_block_generation_of_others() {
    let mut doc = read(&generation_input(&typed("Claim", "c1", "", "")));
    apply(
        &mut doc,
        SetReferences::new("c1", ReferenceEnd::CitedElement, ["claim_1"]),
    );
    assert_eq!(
        next_claim_id(&doc, "p"),
        id("claim_2"),
        "{ROW}: a reference to claim_1 moved generation past claim_2"
    );
}

// --- Item 9: changed_ids and refusals ----------------------------------------------

#[test]
fn lib_edit_009_changed_ids_is_the_owner_when_the_end_changes_and_empty_when_not() {
    let input = in_case("");
    let cases: Vec<(Operation, Vec<ElementId>)> = vec![
        (
            SetReferences::new("i1", ReferenceEnd::Source, ["c2"]).into(),
            Vec::new(),
        ),
        (
            SetReferences::new("i1", ReferenceEnd::Target, ["c1"]).into(),
            Vec::new(),
        ),
        (
            SetReferences::clear("c1", ReferenceEnd::MetaClaim).into(),
            Vec::new(),
        ),
        (
            SetReferences::new("i1", ReferenceEnd::Source, ["c2", "c2"]).into(),
            ids(&["i1"]),
        ),
        (
            SetReferences::new("i1", ReferenceEnd::Target, ["c2"]).into(),
            ids(&["i1"]),
        ),
        (
            AddReference::new("c1", ReferenceEnd::MetaClaim, "c2").into(),
            ids(&["c1"]),
        ),
        (
            RemoveReference::new("i1", ReferenceEnd::Source, "c2").into(),
            ids(&["i1"]),
        ),
    ];
    for (operation, expected) in cases {
        let mut doc = read(&input);
        if matches!(operation, Operation::RemoveReference(_)) {
            apply(
                &mut doc,
                AddReference::new("i1", ReferenceEnd::Source, "r1"),
            );
        }
        let before = doc.clone();
        let mutation = apply(&mut doc, operation.clone());
        assert_eq!(
            mutation.changed_ids(),
            expected,
            "{ROW}: {operation:?}: changed_ids"
        );
        assert_eq!(
            (mutation.created_ids(), mutation.deleted_ids()),
            ([].as_slice(), [].as_slice()),
            "{ROW}: {operation:?}: created_ids and deleted_ids"
        );
        if expected.is_empty() {
            assert!(
                doc == before,
                "{ROW}: {operation:?} changed nothing but the document differs"
            );
        }
    }
    // A referenced element is never reported.
    let mut doc = read(&input);
    let mutation = apply(
        &mut doc,
        SetReferences::new("i1", ReferenceEnd::Source, ["r2"]),
    );
    assert_eq!(
        mutation.changed_ids(),
        [id("i1")],
        "{ROW}: the old or new source is reported as changed"
    );
}

#[test]
fn lib_edit_009_errors_have_their_codes_and_one_line_displays() {
    let spaced = typed("Claim", "a b", "", "");
    let mut doc = doc_with(&spaced);
    let errors = [
        (
            refuse(
                &mut doc,
                AddReference::new("i1", ReferenceEnd::Target, "c2"),
            ),
            Code::MULTIPLICITY_VIOLATION,
            "the target is given 2 ids, and SACM allows at most 1",
        ),
        (
            refuse(
                &mut doc,
                RemoveReference::new("i1", ReferenceEnd::Source, "x"),
            ),
            Code::CMD_TARGET_NOT_FOUND,
            "the source holds no id \"x\" to remove",
        ),
        (
            refuse(&mut doc, SetReferences::clear("i1", ReferenceEnd::Target)),
            Code::MULTIPLICITY_VIOLATION,
            "the target is given 0 ids, and SACM requires at least 1",
        ),
        (
            refuse(
                &mut doc,
                SetReferences::new("nowhere", ReferenceEnd::Source, ["c1"]),
            ),
            Code::CMD_TARGET_NOT_FOUND,
            "the document has no element \"nowhere\" to edit",
        ),
        (
            refuse(
                &mut doc,
                SetReferences::new("c1", ReferenceEnd::Source, ["c2"]),
            ),
            Code::CMD_FEATURE_NOT_APPLICABLE,
            "the Claim \"c1\" has no source",
        ),
        (
            refuse(
                &mut doc,
                SetReferences::new("c1", ReferenceEnd::MetaClaim, ["a b"]),
            ),
            Code::ID_INVALID,
            "the metaClaim \"a b\" holds white space, so it cannot be written as one id",
        ),
    ];
    for (error, code, text) in errors {
        assert_eq!(error.code(), code, "{ROW}: the code of {error:?}");
        assert_eq!(error.to_string(), text, "{ROW}: the Display of {error:?}");
    }
    assert_eq!(
        Code::CMD_FEATURE_NOT_APPLICABLE.as_str(),
        "SACM-CMD-008",
        "{ROW}: the code text"
    );
}

#[test]
fn lib_edit_009_operations_hold_what_they_are_given_and_name_their_struct() {
    let set = SetReferences::new("i1", ReferenceEnd::Source, ["c2", "c1", "c2"]);
    assert_eq!(
        (set.element().as_str(), set.end(), set.ids()),
        (
            "i1",
            ReferenceEnd::Source,
            ids(&["c2", "c1", "c2"]).as_slice()
        ),
        "{ROW}: SetReferences::new's accessors"
    );
    let clear = SetReferences::clear("i1", ReferenceEnd::Reasoning);
    assert_eq!(
        (clear.element().as_str(), clear.end(), clear.ids()),
        ("i1", ReferenceEnd::Reasoning, [].as_slice()),
        "{ROW}: SetReferences::clear's accessors"
    );
    assert_eq!(
        clear,
        SetReferences::new("i1", ReferenceEnd::Reasoning, Vec::<ElementId>::new()),
        "{ROW}: clear is not an empty set"
    );
    let add = AddReference::new("c1", ReferenceEnd::MetaClaim, id("c2"));
    assert_eq!(
        (add.element().as_str(), add.end(), add.id().as_str()),
        ("c1", ReferenceEnd::MetaClaim, "c2"),
        "{ROW}: AddReference's accessors"
    );
    let remove = RemoveReference::new("c1", ReferenceEnd::MetaClaim, "c2");
    assert_eq!(
        (
            remove.element().as_str(),
            remove.end(),
            remove.id().as_str()
        ),
        ("c1", ReferenceEnd::MetaClaim, "c2"),
        "{ROW}: RemoveReference's accessors"
    );
    for (operation, name) in [
        (Operation::from(set), "SetReferences"),
        (clear.into(), "SetReferences"),
        (add.into(), "AddReference"),
        (remove.into(), "RemoveReference"),
    ] {
        assert_eq!(operation.operation_name(), name, "{ROW}: operation_name()");
    }
    assert_eq!(
        ENDS.map(|(end, _)| end.name()),
        ENDS.map(|(_, name)| name),
        "{ROW}: ReferenceEnd::name is not the XMI name"
    );
}
