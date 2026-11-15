//! Tests for the attribute edits (LIB-EDIT-008; formal/23-05-08 §8.2, §8.6,
//! §11.10, §11.13; D-VAL-LAYER, D-ATOMIC, D-RESULT, D-PRESERVE; ADR 0019
//! amendments 1 and 2, ADR 0029).
//!
//! - `SetName` sets the `name` of any `ModelElement` to a full `LangString`,
//!   held exactly; it keeps the name's `xmi:id`.
//! - `SetGid` sets (`new`) or removes (`remove`) the `gid` of any element,
//!   held exactly.
//! - `SetFlag` sets `isCitation` or `isAbstract` of any element, and
//!   `isCounter` of an asserted relationship, independently of the
//!   `citedElement` and `abstractForm` ends.
//! - `SetAssertionDeclaration` sets the declaration of an Assertion.
//!
//! Each refuses, in this order, an unknown element (`ElementNotFound`,
//! SACM-CMD-001) and a kind without the feature (`FeatureNotApplicable`,
//! SACM-CMD-008), and nothing else: content findings are left to
//! `validate`. A refusal leaves the document `==` to before, with the same
//! next generated ids. `changed_ids` is `[element]` exactly when the value
//! differs.
//!
//! Item 4 (no successful operation of the LIB-EDIT-002 set removes a name)
//! and the last part of item 12 (the operations join the set) are in
//! `edit_atomic.rs`, which holds the set.

use sacm::compare::{Difference, semantic_compare};
use sacm::diagnostic::{Code, Diagnostic, Severity};
use sacm::edit::{
    self, CreateArgumentPackage, CreateAssuranceCasePackage, CreateClaim, EditError, Mutation,
    Operation, SetAssertionDeclaration, SetFlag, SetGid, SetName,
};
use sacm::model::{
    AbstractClass, AssertionDeclaration, Document, ElementId, ElementKind, Flag, LangString,
};
use sacm::validation;
use sacm::xmi::{self, Mode, ReadOptions, WriteAttribute, WriteError, WriteOptions};

const ROW: &str = "LIB-EDIT-008";

/// The namespace declarations of a bare root: `sacm` and `xmi`.
const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
);

/// The five asserted relationship kinds (§11.13-§11.18).
const RELATIONSHIPS: [ElementKind; 5] = [
    ElementKind::AssertedInference,
    ElementKind::AssertedEvidence,
    ElementKind::AssertedContext,
    ElementKind::AssertedArtifactSupport,
    ElementKind::AssertedArtifactContext,
];

/// The four `UtilityElement` kinds (§8.7), which have no name.
const UTILITY_ELEMENTS: [ElementKind; 4] = [
    ElementKind::Description,
    ElementKind::ImplementationConstraint,
    ElementKind::Note,
    ElementKind::TaggedValue,
];

// --- Helpers -------------------------------------------------------------------------

fn id(value: &str) -> ElementId {
    ElementId::from(value)
}

fn named(content: &str) -> LangString {
    LangString::new(content).with_lang("en")
}

/// A `LangString` with exactly the given `lang` and `content`, each absent
/// when `None`.
fn lang_string(lang: Option<&str>, content: Option<&str>) -> LangString {
    let mut value = LangString::default();
    value.set_lang(lang.map(str::to_owned));
    value.set_content(content.map(str::to_owned));
    value
}

/// The nine names: each of absent, empty and non-empty `lang` with each of
/// absent, empty and non-empty `content`.
fn nine_names() -> Vec<LangString> {
    let mut names = Vec::new();
    for lang in [None, Some(""), Some("en")] {
        for content in [None, Some(""), Some("x")] {
            names.push(lang_string(lang, content));
        }
    }
    names
}

/// Applies `operation`, which the test expects to succeed.
fn apply(doc: &mut Document, operation: impl Into<Operation>) -> Mutation {
    let operation = operation.into();
    match edit::apply(doc, operation.clone()) {
        Ok(mutation) => mutation,
        Err(error) => panic!("{ROW}: {operation:?} was refused: {error} ({error:?})"),
    }
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
        semantic_compare(&before, doc),
        [],
        "{ROW}: refusing {operation:?} changed the document"
    );
    assert_eq!(
        next_generated_ids(doc),
        next,
        "{ROW}: refusing {operation:?} changed the next generated ids"
    );
    error
}

/// The ids the next generated AssuranceCasePackage, ArgumentPackage and Claim
/// would get, observed on a clone.
fn next_generated_ids(doc: &Document) -> [ElementId; 3] {
    let mut probe = doc.clone();
    let mut created = |operation: Operation| -> ElementId {
        let mutation = apply(&mut probe, operation);
        mutation.created_ids()[0].clone()
    };
    let case = created(CreateAssuranceCasePackage::root(named("probe")).into());
    let arguments = created(CreateArgumentPackage::root(named("probe")).into());
    let claim = created(CreateClaim::new(&arguments, named("probe")).into());
    [case, arguments, claim]
}

/// Inserts one element through the `test-support` seam.
fn insert(doc: &mut Document, parent: Option<&str>, kind: ElementKind, value: &str) {
    let parent = parent.map(ElementId::from);
    let name = kind.is_a(AbstractClass::ModelElement).then(|| named(value));
    if let Err(error) = doc.insert(parent.as_ref(), kind, id(value), name) {
        panic!("{ROW}: setup could not insert the {kind} {value:?}: {error}");
    }
}

/// The id this file gives the element of `kind` in [`every_kind`].
fn id_of(kind: ElementKind) -> String {
    format!("k_{}", kind.name())
}

/// A document with one element of every kind: the ArtifactPackage root
/// `art`, which holds one of every other `ModelElement` kind (`artifactElement`
/// admits every one, §12.2, ADR 0014) and one of each `UtilityElement` kind,
/// each named after its kind by [`id_of`]. The root itself is the
/// ArtifactPackage.
fn every_kind() -> Document {
    let mut doc = Document::new();
    insert(&mut doc, None, ElementKind::ArtifactPackage, "art");
    for &kind in ElementKind::ALL {
        if kind != ElementKind::ArtifactPackage {
            insert(&mut doc, Some("art"), kind, &id_of(kind));
        }
    }
    doc
}

/// The id of the element of `kind` in [`every_kind`].
fn every_kind_id(kind: ElementKind) -> String {
    if kind == ElementKind::ArtifactPackage {
        "art".to_owned()
    } else {
        id_of(kind)
    }
}

/// Reads `input` in `mode`.
fn read_in(mode: Mode, input: &str) -> Document {
    match xmi::read(input.as_bytes(), &ReadOptions::default().with_mode(mode)) {
        Ok(output) => output.into_document(),
        Err(error) => panic!("{ROW}: setup: the input does not read in {mode:?}: {error}"),
    }
}

fn read(input: &str) -> Document {
    read_in(Mode::Tolerant, input)
}

/// Writes `doc` tolerantly, so that content findings do not stop the write.
fn write(doc: &Document) -> String {
    match xmi::write(doc, &WriteOptions::default().with_mode(Mode::Tolerant)) {
        Ok(text) => text,
        Err(error) => panic!("{ROW}: the document does not write: {error}"),
    }
}

/// Writes `doc` and reads it back.
fn round_trip(doc: &Document) -> Document {
    read(&write(doc))
}

/// A named element `id` of `kind` under `argumentElement`, with `attributes`
/// and holding `content` after its name.
fn typed(kind: &str, id: &str, attributes: &str, content: &str) -> String {
    format!(
        "<argumentElement xmi:type=\"sacm:{kind}\" xmi:id=\"{id}\"{attributes}>\
         <name content=\"N\"/>{content}</argumentElement>"
    )
}

/// An ArgumentPackage root `p` named "P" holding `body`.
fn in_ap(body: &str) -> String {
    format!(
        "<sacm:ArgumentPackage{NS} xmi:id=\"p\"><name content=\"P\"/>{body}</sacm:ArgumentPackage>"
    )
}

/// An AssuranceCasePackage root `case` holding the ArgumentPackage `p`, which
/// holds every readable kind: the Claims c1 and c2 (c1 with the Description
/// d1, the `ImplementationConstraint` ic1, the Note n1 and the `TaggedValue`
/// t1), the `ArgumentReasoning` `why`, the `ArtifactReference` r1, the
/// `ArgumentGroup` g1 and one of each asserted relationship, each from c2 to
/// c1 (the artifact kinds from r1 to r1); then `body`.
fn every_readable_kind(body: &str) -> String {
    let mut elements = vec![
        typed(
            "Claim",
            "c1",
            "",
            "<description xmi:id=\"d1\"><content><value content=\"D\"/></content></description>\
             <implementationConstraint xmi:id=\"ic1\"/><note xmi:id=\"n1\"/>\
             <taggedValue xmi:id=\"t1\"><key><value content=\"K\"/></key></taggedValue>",
        ),
        typed("Claim", "c2", "", ""),
        typed("ArgumentReasoning", "why", "", ""),
        typed("ArtifactReference", "r1", "", ""),
        typed("ArgumentGroup", "g1", "", ""),
    ];
    for (kind, relationship, ends) in [
        ("AssertedInference", "i1", " source=\"c2\" target=\"c1\""),
        ("AssertedEvidence", "e1", " source=\"r1\" target=\"c1\""),
        ("AssertedContext", "x1", " source=\"r1\" target=\"c1\""),
        (
            "AssertedArtifactSupport",
            "s1",
            " source=\"r1\" target=\"r1\"",
        ),
        (
            "AssertedArtifactContext",
            "ac1",
            " source=\"r1\" target=\"r1\"",
        ),
    ] {
        elements.push(typed(kind, relationship, ends, ""));
    }
    format!(
        "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\"><name content=\"Case\"/>\
         <argumentPackage xmi:id=\"p\"><name content=\"P\"/>{}{body}</argumentPackage>\
         </sacm:AssuranceCasePackage>",
        elements.concat()
    )
}

/// The id of the relationship of `kind` in [`every_readable_kind`].
fn relationship_id(kind: ElementKind) -> &'static str {
    match kind {
        ElementKind::AssertedInference => "i1",
        ElementKind::AssertedEvidence => "e1",
        ElementKind::AssertedContext => "x1",
        ElementKind::AssertedArtifactSupport => "s1",
        ElementKind::AssertedArtifactContext => "ac1",
        other => panic!("{ROW}: {other} is not a relationship"),
    }
}

/// The ids of the readable elements of [`every_readable_kind`] that are
/// `ModelElement`s, with their kinds.
const READABLE_MODEL_ELEMENTS: [(&str, ElementKind); 12] = [
    ("case", ElementKind::AssuranceCasePackage),
    ("p", ElementKind::ArgumentPackage),
    ("c1", ElementKind::Claim),
    ("why", ElementKind::ArgumentReasoning),
    ("r1", ElementKind::ArtifactReference),
    ("g1", ElementKind::ArgumentGroup),
    ("i1", ElementKind::AssertedInference),
    ("e1", ElementKind::AssertedEvidence),
    ("x1", ElementKind::AssertedContext),
    ("s1", ElementKind::AssertedArtifactSupport),
    ("ac1", ElementKind::AssertedArtifactContext),
    ("c2", ElementKind::Claim),
];

/// The findings of `validate` with `code` that affect `element`.
fn findings(doc: &Document, code: Code, element: &str) -> Vec<Diagnostic> {
    validation::validate(doc)
        .into_iter()
        .filter(|diagnostic| {
            diagnostic.code() == code && diagnostic.affected().first() == Some(&id(element))
        })
        .collect()
}

/// The element `value` of `doc`.
fn element<'d>(doc: &'d Document, value: &str) -> sacm::model::SacmElement<'d> {
    match doc.get(value) {
        Some(found) => found,
        None => panic!("{ROW}: the document has no element {value:?}"),
    }
}

// --- Item 1: every name on every ModelElement kind ------------------------------------

#[test]
fn lib_edit_008_each_of_the_nine_names_is_set_exactly_on_every_model_element_kind() {
    let names = nine_names();
    assert_eq!(names.len(), 9, "{ROW}: setup: not nine names");
    let model_elements: Vec<ElementKind> = ElementKind::ALL
        .iter()
        .copied()
        .filter(|kind| kind.is_a(AbstractClass::ModelElement))
        .collect();
    assert_eq!(
        model_elements.len(),
        34,
        "{ROW}: setup: not 34 ModelElement kinds"
    );
    for kind in model_elements {
        let target = every_kind_id(kind);
        for name in &names {
            let mut doc = every_kind();
            apply(&mut doc, SetName::new(target.as_str(), name.clone()));
            assert_eq!(
                element(&doc, &target).name(),
                Some(name),
                "{ROW}: the name of the {kind} after SetName {name:?}"
            );
        }
    }
}

#[test]
fn lib_edit_008_a_set_name_round_trips_with_absent_and_empty_kept_apart() {
    for (target, kind) in READABLE_MODEL_ELEMENTS {
        for name in nine_names() {
            let mut doc = read(&every_readable_kind(""));
            apply(&mut doc, SetName::new(target, name.clone()));
            let back = round_trip(&doc);
            assert_eq!(
                back, doc,
                "{ROW}: the {kind} {target:?} named {name:?} does not read back equal"
            );
            assert_eq!(
                element(&back, target).name(),
                Some(&name),
                "{ROW}: the {kind} {target:?} reads back with another name than {name:?}"
            );
        }
    }
    // Absent and "" stay apart in both fields.
    let mut doc = read(&every_readable_kind(""));
    apply(&mut doc, SetName::new("c1", lang_string(None, None)));
    apply(
        &mut doc,
        SetName::new("c2", lang_string(Some(""), Some(""))),
    );
    let back = round_trip(&doc);
    assert_eq!(
        (
            element(&back, "c1").name().map(|n| (n.lang(), n.content())),
            element(&back, "c2").name().map(|n| (n.lang(), n.content()))
        ),
        (Some((None, None)), Some((Some(""), Some("")))),
        "{ROW}: absent and empty lang and content are not kept apart"
    );
}

// --- Item 2: a missing name, and a name's xmi:id --------------------------------------

#[test]
fn lib_edit_008_setting_a_name_on_an_element_read_without_one_removes_its_missing_name_finding() {
    let input = in_ap(
        "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"/>\
         <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c2\"/>",
    );
    let mut doc = read(&input);
    let missing = |doc: &Document, value: &str| -> Vec<(Severity, String, Vec<ElementId>)> {
        findings(doc, Code::MULTIPLICITY_VIOLATION, value)
            .into_iter()
            .filter(|d| d.requirement_id() == "SACM23-VAL-018")
            .map(|d| {
                (
                    d.severity(),
                    d.requirement_id().to_owned(),
                    d.affected().to_vec(),
                )
            })
            .collect()
    };
    assert_eq!(
        missing(&doc, "c1"),
        [(Severity::Error, "SACM23-VAL-018".to_owned(), vec![id("c1")])],
        "{ROW}: setup: c1 is not read without a name"
    );
    let mutation = apply(&mut doc, SetName::new("c1", LangString::default()));
    assert_eq!(
        mutation.changed_ids(),
        [id("c1")],
        "{ROW}: naming c1 does not change exactly c1"
    );
    assert_eq!(
        missing(&doc, "c1"),
        [],
        "{ROW}: c1 still has its SACM23-VAL-018 finding after SetName"
    );
    assert_eq!(
        missing(&doc, "c2").len(),
        1,
        "{ROW}: naming c1 removed c2's finding too"
    );
    assert_eq!(
        semantic_compare(&read(&input), &doc),
        [Difference::Name {
            id: id("c1"),
            left: None,
            right: Some(LangString::default())
        }],
        "{ROW}: SetName on a nameless element is not one Name difference"
    );
}

#[test]
fn lib_edit_008_a_replaced_name_keeps_its_xmi_id() {
    let input = in_ap(
        "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\">\
         <name xmi:id=\"n1\" lang=\"en\" content=\"Old\"/></argumentElement>",
    );
    let mut doc = read(&input);
    let before = doc.clone();
    let mutation = apply(&mut doc, SetName::new("c1", named("New")));
    let c1 = element(&doc, "c1");
    assert_eq!(
        (c1.name(), c1.name_id().map(ElementId::as_str)),
        (Some(&named("New")), Some("n1")),
        "{ROW}: the replaced name does not keep its xmi:id"
    );
    assert_eq!(
        semantic_compare(&before, &doc),
        [Difference::Name {
            id: id("c1"),
            left: Some(named("Old")),
            right: Some(named("New"))
        }],
        "{ROW}: replacing a name changed more than the name"
    );
    assert_eq!(
        mutation.changed_ids(),
        [id("c1")],
        "{ROW}: the changed ids of a replaced name"
    );
    assert!(
        write(&doc).contains("<name xmi:id=\"n1\""),
        "{ROW}: the replaced name is not written with its xmi:id"
    );
    // The id is still in use, so a new element cannot take it.
    let refused = refuse(&mut doc, CreateClaim::new("p", named("C")).with_id("n1"));
    assert_eq!(
        refused.code(),
        Code::CMD_DUPLICATE_ID,
        "{ROW}: the kept name id is not in use: {refused:?}"
    );
}

// --- Item 3: no name on a UtilityElement ------------------------------------------

#[test]
fn lib_edit_008_a_name_on_a_utility_element_is_feature_not_applicable() {
    for kind in UTILITY_ELEMENTS {
        let mut doc = every_kind();
        let target = id_of(kind);
        let error = refuse(&mut doc, SetName::new(target.as_str(), named("N")));
        assert_eq!(
            error.code(),
            Code::CMD_FEATURE_NOT_APPLICABLE,
            "{ROW}: a name on the {kind}: {error:?}"
        );
        assert_eq!(
            error.code().as_str(),
            "SACM-CMD-008",
            "{ROW}: the code text of FeatureNotApplicable"
        );
        let EditError::FeatureNotApplicable {
            element,
            kind: held_kind,
            feature,
            ..
        } = &error
        else {
            panic!("{ROW}: a name on the {kind} is not FeatureNotApplicable: {error:?}");
        };
        assert_eq!(
            (element.as_str(), *held_kind, *feature),
            (target.as_str(), kind, "name"),
            "{ROW}: the fields of FeatureNotApplicable for a name on the {kind}"
        );
        assert_eq!(
            error.to_string(),
            format!("the {} {target:?} has no name", kind.name()),
            "{ROW}: the Display of FeatureNotApplicable"
        );
    }
}

// --- Items 5 and 6: gid ---------------------------------------------------------------

#[test]
fn lib_edit_008_a_gid_is_set_and_removed_exactly_on_a_model_element_and_a_utility_element() {
    for target in ["c1", "n1"] {
        let mut doc = read(&every_readable_kind(""));
        let gid = |doc: &Document| element(doc, target).gid().map(str::to_owned);
        assert_eq!(gid(&doc), None, "{ROW}: setup: {target} has a gid");
        for value in ["", " x ", "g", "g"] {
            apply(&mut doc, SetGid::new(target, value));
            assert_eq!(
                gid(&doc).as_deref(),
                Some(value),
                "{ROW}: the gid of {target} after SetGid {value:?}"
            );
        }
        // The gid another element holds.
        apply(&mut doc, SetGid::new("c2", "shared"));
        apply(&mut doc, SetGid::new(target, "shared"));
        assert_eq!(
            gid(&doc).as_deref(),
            Some("shared"),
            "{ROW}: the gid of {target} after taking c2's"
        );
        apply(&mut doc, SetGid::remove(target));
        assert_eq!(
            gid(&doc),
            None,
            "{ROW}: {target} has a gid after SetGid::remove"
        );
    }
}

#[test]
fn lib_edit_008_a_duplicate_gid_is_accepted_and_validate_reports_it() {
    let mut doc = read(&every_readable_kind(""));
    apply(&mut doc, SetGid::new("c1", "urn:1"));
    let duplicates = |doc: &Document| -> Vec<(Severity, String, Vec<ElementId>)> {
        validation::validate(doc)
            .into_iter()
            .filter(|d| d.code() == Code::GID_DUPLICATE)
            .map(|d| {
                (
                    d.severity(),
                    d.requirement_id().to_owned(),
                    d.affected().to_vec(),
                )
            })
            .collect()
    };
    assert_eq!(duplicates(&doc), [], "{ROW}: one holder is a duplicate");
    apply(&mut doc, SetGid::new("n1", "urn:1"));
    assert_eq!(
        duplicates(&doc),
        [(
            Severity::Error,
            "SACM23-VAL-002".to_owned(),
            vec![id("c1"), id("n1")]
        )],
        "{ROW}: a duplicate gid set by an edit is not one SACM-ID-003 Error on both holders"
    );
    // An empty gid is not compared.
    let mut doc = read(&every_readable_kind(""));
    apply(&mut doc, SetGid::new("c1", ""));
    apply(&mut doc, SetGid::new("c2", ""));
    assert_eq!(duplicates(&doc), [], "{ROW}: two empty gids are reported");
}

#[test]
fn lib_edit_008_a_set_gid_round_trips_with_absent_empty_and_a_value_apart() {
    let mut doc = read(&every_readable_kind(""));
    apply(&mut doc, SetGid::new("c1", ""));
    apply(&mut doc, SetGid::new("c2", " g "));
    apply(&mut doc, SetGid::new("n1", "g"));
    apply(&mut doc, SetGid::new("t1", "g"));
    apply(&mut doc, SetGid::remove("t1"));
    let back = round_trip(&doc);
    assert_eq!(back, doc, "{ROW}: the gids do not read back equal");
    let gids: Vec<Option<&str>> = ["c1", "c2", "n1", "t1"]
        .into_iter()
        .map(|value| element(&back, value).gid())
        .collect();
    assert_eq!(
        gids,
        [Some(""), Some(" g "), Some("g"), None],
        "{ROW}: absent, empty and the values are not kept apart"
    );
}

#[test]
fn lib_edit_008_a_gid_holding_a_character_xml_cannot_represent_is_accepted_and_the_write_fails() {
    for target in ["c1", "n1"] {
        let mut doc = read(&every_readable_kind(""));
        // c1 holds an implementation constraint, which needs it abstract
        // (§8.6, SACM23-VAL-021); otherwise the strict write refuses it as
        // Invalid before it reaches the gid.
        apply(&mut doc, SetFlag::new("c1", Flag::IsAbstract, true));
        let mutation = apply(&mut doc, SetGid::new(target, "a\u{1}b"));
        assert_eq!(
            mutation.changed_ids(),
            [id(target)],
            "{ROW}: the changed ids of a gid holding U+0001"
        );
        assert_eq!(
            element(&doc, target).gid(),
            Some("a\u{1}b"),
            "{ROW}: the gid holding U+0001 is not held exactly"
        );
        for mode in [Mode::Strict, Mode::Tolerant] {
            let error = match xmi::write(&doc, &WriteOptions::default().with_mode(mode)) {
                Ok(text) => panic!("{ROW}: a gid holding U+0001 was written in {mode:?}: {text}"),
                Err(error) => error,
            };
            assert!(
                matches!(
                    &error,
                    WriteError::UnrepresentableCharacter {
                        element,
                        character: '\u{1}',
                        attribute: WriteAttribute::Gid,
                        ..
                    } if element.as_str() == target
                ),
                "{ROW}: the write in {mode:?} does not fail with WriteAttribute::Gid on {target}: \
                 {error:?}"
            );
            assert_eq!(
                error.element().map(ElementId::as_str),
                Some(target),
                "{ROW}: the refused element"
            );
        }
    }
}

// --- Items 7 and 8: isCitation and isAbstract ---------------------------------------

#[test]
fn lib_edit_008_is_citation_and_is_abstract_are_set_true_and_false_on_every_kind() {
    for &kind in ElementKind::ALL {
        let target = every_kind_id(kind);
        for flag in [Flag::IsCitation, Flag::IsAbstract] {
            let mut doc = every_kind();
            let value = |doc: &Document| {
                let found = element(doc, &target);
                if flag == Flag::IsCitation {
                    found.is_citation()
                } else {
                    found.is_abstract()
                }
            };
            for set in [true, true, false, true, false, false] {
                apply(&mut doc, SetFlag::new(target.as_str(), flag, set));
                assert_eq!(
                    value(&doc),
                    set,
                    "{ROW}: {} of the {kind} after setting it {set}",
                    flag.name()
                );
            }
        }
    }
}

#[test]
fn lib_edit_008_the_flags_are_set_independently_of_cited_element_and_abstract_form() {
    let input = every_readable_kind(&typed(
        "Claim",
        "c9",
        " isCitation=\"true\" citedElement=\"c2\" abstractForm=\"c2\"",
        "",
    ));
    let mut doc = read(&input);
    // isCitation false keeps the citedElement, and validate then reports it.
    apply(&mut doc, SetFlag::new("c9", Flag::IsCitation, false));
    let c9 = element(&doc, "c9");
    assert_eq!(
        (
            c9.is_citation(),
            c9.cited_element().map(ElementId::as_str),
            c9.abstract_form().map(ElementId::as_str)
        ),
        (false, Some("c2"), Some("c2")),
        "{ROW}: clearing isCitation touched an end"
    );
    let citation: Vec<(Severity, Vec<ElementId>)> = findings(&doc, Code::CITATION_INVALID, "c9")
        .into_iter()
        .map(|d| (d.severity(), d.affected().to_vec()))
        .collect();
    assert_eq!(
        citation,
        [(Severity::Error, vec![id("c9")])],
        "{ROW}: a citedElement without isCitation is not one SACM-CITE-001 Error"
    );
    // isCitation true alone is held, and is no finding.
    apply(&mut doc, SetFlag::new("c1", Flag::IsCitation, true));
    let c1 = element(&doc, "c1");
    assert_eq!(
        (c1.is_citation(), c1.cited_element()),
        (true, None),
        "{ROW}: isCitation true alone is not held"
    );
    assert_eq!(
        findings(&doc, Code::CITATION_INVALID, "c1"),
        [],
        "{ROW}: isCitation true without a citedElement is reported"
    );
    // isAbstract leaves the abstractForm alone.
    apply(&mut doc, SetFlag::new("c9", Flag::IsAbstract, true));
    assert_eq!(
        element(&doc, "c9").abstract_form().map(ElementId::as_str),
        Some("c2"),
        "{ROW}: setting isAbstract touched the abstractForm"
    );
    let differences = semantic_compare(&read(&input), &doc);
    assert!(
        differences.iter().all(|d| matches!(
            d,
            Difference::IsCitation { .. } | Difference::IsAbstract { .. }
        )),
        "{ROW}: the flag edits changed more than the flags: {differences:?}"
    );
}

#[test]
fn lib_edit_008_is_abstract_on_an_element_with_an_abstract_form_is_accepted_and_reported() {
    let input = every_readable_kind(&typed("Claim", "c9", " abstractForm=\"c2\"", ""));
    let mut doc = read(&input);
    apply(&mut doc, SetFlag::new("c2", Flag::IsAbstract, true));
    assert!(
        findings(&doc, Code::ABSTRACT_FORM_INVALID, "c9")
            .iter()
            .all(|d| d.severity() != Severity::Error),
        "{ROW}: setup: c9 is reported as abstract before the edit"
    );
    let mutation = apply(&mut doc, SetFlag::new("c9", Flag::IsAbstract, true));
    assert_eq!(mutation.changed_ids(), [id("c9")], "{ROW}: the changed ids");
    let errors: Vec<(Severity, String, Vec<ElementId>)> =
        findings(&doc, Code::ABSTRACT_FORM_INVALID, "c9")
            .into_iter()
            .filter(|d| d.severity() == Severity::Error)
            .map(|d| {
                (
                    d.severity(),
                    d.requirement_id().to_owned(),
                    d.affected().to_vec(),
                )
            })
            .collect();
    assert_eq!(
        errors,
        [(Severity::Error, "SACM23-VAL-008".to_owned(), vec![id("c9")])],
        "{ROW}: isAbstract on an element with an abstractForm is not one SACM-ABS-001 Error"
    );
}

#[test]
fn lib_edit_008_is_abstract_false_on_an_element_with_an_implementation_constraint_is_accepted() {
    let input = every_readable_kind(&typed(
        "Claim",
        "c9",
        " isAbstract=\"true\"",
        "<implementationConstraint xmi:id=\"ic9\"/>",
    ));
    let mut doc = read(&input);
    let mutation = apply(&mut doc, SetFlag::new("c9", Flag::IsAbstract, false));
    assert_eq!(mutation.changed_ids(), [id("c9")], "{ROW}: the changed ids");
    assert!(
        !element(&doc, "c9").is_abstract(),
        "{ROW}: isAbstract is still true"
    );
    assert_eq!(
        element(&doc, "c9")
            .implementation_constraints()
            .map(|ic| ic.id().as_str().to_owned())
            .collect::<Vec<_>>(),
        ["ic9"],
        "{ROW}: the ImplementationConstraint is not kept"
    );
}

// --- Item 9: isCounter -------------------------------------------------------------

#[test]
fn lib_edit_008_is_counter_is_set_true_and_false_on_each_relationship_kind() {
    for kind in RELATIONSHIPS {
        let target = relationship_id(kind);
        let mut doc = read(&every_readable_kind(""));
        for set in [true, false, true] {
            apply(&mut doc, SetFlag::new(target, Flag::IsCounter, set));
            assert_eq!(
                element(&doc, target).is_counter(),
                set,
                "{ROW}: isCounter of the {kind} after setting it {set}"
            );
        }
        let back = round_trip(&doc);
        assert_eq!(
            back, doc,
            "{ROW}: isCounter of the {kind} does not read back"
        );
    }
}

#[test]
fn lib_edit_008_is_counter_on_a_claim_a_reasoning_or_a_package_is_feature_not_applicable() {
    for (target, kind) in [
        ("c1", ElementKind::Claim),
        ("why", ElementKind::ArgumentReasoning),
        ("p", ElementKind::ArgumentPackage),
        ("case", ElementKind::AssuranceCasePackage),
        ("r1", ElementKind::ArtifactReference),
        ("n1", ElementKind::Note),
    ] {
        for value in [true, false] {
            let mut doc = read(&every_readable_kind(""));
            let error = refuse(&mut doc, SetFlag::new(target, Flag::IsCounter, value));
            assert!(
                matches!(
                    &error,
                    EditError::FeatureNotApplicable { element, kind: held, feature: "isCounter", .. }
                        if element.as_str() == target && *held == kind
                ),
                "{ROW}: isCounter={value} on the {kind} {target:?}: {error:?}"
            );
            assert_eq!(
                error.code(),
                Code::CMD_FEATURE_NOT_APPLICABLE,
                "{ROW}: the code"
            );
        }
    }
    let mut doc = read(&every_readable_kind(""));
    let error = refuse(&mut doc, SetFlag::new("c1", Flag::IsCounter, true));
    assert_eq!(
        error.to_string(),
        "the Claim \"c1\" has no isCounter",
        "{ROW}: the Display of FeatureNotApplicable"
    );
}

// --- Item 10: assertionDeclaration ---------------------------------------------------

#[test]
fn lib_edit_008_each_declaration_is_set_on_a_claim_and_on_each_relationship_kind() {
    assert_eq!(
        AssertionDeclaration::ALL.len(),
        6,
        "{ROW}: setup: the literals changed"
    );
    let targets = ["c1"].into_iter().chain(RELATIONSHIPS.map(relationship_id));
    for target in targets {
        for &declaration in AssertionDeclaration::ALL {
            let mut doc = read(&every_readable_kind(""));
            apply(&mut doc, SetAssertionDeclaration::new(target, declaration));
            assert_eq!(
                element(&doc, target).assertion_declaration(),
                Some(declaration),
                "{ROW}: the declaration of {target:?} after setting {declaration:?}"
            );
            assert_eq!(
                round_trip(&doc),
                doc,
                "{ROW}: {declaration:?} on {target:?} does not read back"
            );
        }
    }
}

#[test]
fn lib_edit_008_as_cited_without_a_citation_is_accepted() {
    let mut doc = read(&every_readable_kind(""));
    let mutation = apply(
        &mut doc,
        SetAssertionDeclaration::new("c1", AssertionDeclaration::AsCited),
    );
    assert_eq!(mutation.changed_ids(), [id("c1")], "{ROW}: the changed ids");
    let c1 = element(&doc, "c1");
    assert_eq!(
        (
            c1.assertion_declaration(),
            c1.is_citation(),
            c1.cited_element()
        ),
        (Some(AssertionDeclaration::AsCited), false, None),
        "{ROW}: asCited without a citation is not held as given"
    );
}

#[test]
fn lib_edit_008_a_declaration_on_a_non_assertion_is_feature_not_applicable() {
    for (target, kind) in [
        ("why", ElementKind::ArgumentReasoning),
        ("r1", ElementKind::ArtifactReference),
        ("g1", ElementKind::ArgumentGroup),
        ("p", ElementKind::ArgumentPackage),
        ("case", ElementKind::AssuranceCasePackage),
        ("d1", ElementKind::Description),
    ] {
        let mut doc = read(&every_readable_kind(""));
        let error = refuse(
            &mut doc,
            SetAssertionDeclaration::new(target, AssertionDeclaration::Assumed),
        );
        assert!(
            matches!(
                &error,
                EditError::FeatureNotApplicable { element, kind: held, feature: "assertionDeclaration", .. }
                    if element.as_str() == target && *held == kind
            ),
            "{ROW}: a declaration on the {kind} {target:?}: {error:?}"
        );
    }
}

// --- Item 11: an unknown element, and the order of the checks -------------------------

#[test]
fn lib_edit_008_an_unknown_element_is_element_not_found_and_changes_nothing() {
    let operations: [Operation; 6] = [
        SetName::new("nowhere", named("N")).into(),
        SetGid::new("nowhere", "g").into(),
        SetGid::remove("nowhere").into(),
        SetFlag::new("nowhere", Flag::IsCitation, true).into(),
        SetFlag::new("nowhere", Flag::IsAbstract, false).into(),
        SetAssertionDeclaration::new("nowhere", AssertionDeclaration::Axiomatic).into(),
    ];
    for operation in operations {
        let mut doc = read(&every_readable_kind(""));
        let error = refuse(&mut doc, operation.clone());
        assert!(
            matches!(&error, EditError::ElementNotFound { element, .. } if element.as_str() == "nowhere"),
            "{ROW}: {operation:?}: {error:?}"
        );
        assert_eq!(
            error.code(),
            Code::CMD_TARGET_NOT_FOUND,
            "{ROW}: {operation:?}: the code"
        );
        assert_eq!(
            error.to_string(),
            "the document has no element \"nowhere\" to edit",
            "{ROW}: {operation:?}: the Display"
        );
    }
    // Ids are compared exactly: "C1" is not "c1".
    let mut doc = read(&every_readable_kind(""));
    let error = refuse(&mut doc, SetGid::new("C1", "g"));
    assert_eq!(error.code(), Code::CMD_TARGET_NOT_FOUND, "{ROW}: {error:?}");
}

#[test]
fn lib_edit_008_an_unknown_element_is_checked_before_the_feature() {
    // Each operation would fail the feature check on any element.
    let operations: [Operation; 2] = [
        SetFlag::new("nowhere", Flag::IsCounter, true).into(),
        SetAssertionDeclaration::new("nowhere", AssertionDeclaration::Assumed).into(),
    ];
    for operation in operations {
        let mut doc = read(&every_readable_kind(""));
        let error = refuse(&mut doc, operation.clone());
        assert!(
            matches!(error, EditError::ElementNotFound { .. }),
            "{ROW}: {operation:?} on an unknown element: {error:?}"
        );
    }
}

// --- Item 12: changed_ids, the other lists, names and accessors -----------------------

#[test]
fn lib_edit_008_changed_ids_is_the_element_exactly_when_the_value_differs() {
    let input = every_readable_kind("");
    let cases: [(Operation, bool); 14] = [
        // isCounter=false on a relationship read without it: no change.
        (SetFlag::new("i1", Flag::IsCounter, false).into(), false),
        (SetFlag::new("i1", Flag::IsCounter, true).into(), true),
        (SetFlag::new("c1", Flag::IsCitation, false).into(), false),
        (SetFlag::new("c1", Flag::IsCitation, true).into(), true),
        (SetFlag::new("n1", Flag::IsAbstract, false).into(), false),
        (SetFlag::new("n1", Flag::IsAbstract, true).into(), true),
        (SetName::new("c1", LangString::new("N")).into(), false),
        (
            SetName::new("c1", LangString::new("N").with_lang("")).into(),
            true,
        ),
        (SetGid::remove("c1").into(), false),
        (SetGid::new("c1", "").into(), true),
        (
            SetAssertionDeclaration::new("c1", AssertionDeclaration::Asserted).into(),
            false,
        ),
        (
            SetAssertionDeclaration::new("c1", AssertionDeclaration::Defeated).into(),
            true,
        ),
        (
            SetAssertionDeclaration::new("e1", AssertionDeclaration::Asserted).into(),
            false,
        ),
        (
            SetAssertionDeclaration::new("e1", AssertionDeclaration::NeedsSupport).into(),
            true,
        ),
    ];
    for (operation, changes) in cases {
        let mut doc = read(&input);
        let before = doc.clone();
        let mutation = apply(&mut doc, operation.clone());
        let element = match &operation {
            Operation::SetFlag(set) => set.element().clone(),
            Operation::SetName(set) => set.element().clone(),
            Operation::SetGid(set) => set.element().clone(),
            Operation::SetAssertionDeclaration(set) => set.element().clone(),
            other => panic!("{ROW}: setup: {other:?}"),
        };
        let expected: Vec<ElementId> = if changes { vec![element] } else { Vec::new() };
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
        assert_eq!(
            semantic_compare(&before, &doc).is_empty(),
            !changes,
            "{ROW}: {operation:?}: semantic_compare disagrees with changed_ids"
        );
        assert_eq!(
            mutation.operation(),
            &operation,
            "{ROW}: {operation:?}: operation()"
        );
    }
}

#[test]
fn lib_edit_008_an_unchanged_value_writes_nothing() {
    // Setting the value held leaves the document == to before, counters
    // included.
    let mut doc = read(&every_readable_kind(""));
    let before = doc.clone();
    apply(&mut doc, SetFlag::new("i1", Flag::IsCounter, false));
    apply(&mut doc, SetGid::remove("c1"));
    apply(&mut doc, SetName::new("c1", LangString::new("N")));
    assert!(
        doc == before,
        "{ROW}: an unchanged value changed the document"
    );
    assert_eq!(
        next_generated_ids(&doc),
        next_generated_ids(&before),
        "{ROW}: an unchanged value changed the next ids"
    );
}

#[test]
fn lib_edit_008_operation_names_are_the_struct_names() {
    let cases: [(Operation, &str); 5] = [
        (SetName::new("c1", named("N")).into(), "SetName"),
        (SetGid::new("c1", "g").into(), "SetGid"),
        (SetGid::remove("c1").into(), "SetGid"),
        (SetFlag::new("c1", Flag::IsAbstract, true).into(), "SetFlag"),
        (
            SetAssertionDeclaration::new("c1", AssertionDeclaration::Assumed).into(),
            "SetAssertionDeclaration",
        ),
    ];
    for (operation, expected) in cases {
        assert_eq!(
            operation.operation_name(),
            expected,
            "{ROW}: operation_name() of {operation:?}"
        );
    }
}

#[test]
fn lib_edit_008_operations_hold_what_they_are_given() {
    let name = LangString::new("Säkerhet").with_lang("sv");
    let set_name = SetName::new("c1", name.clone());
    assert_eq!(
        (set_name.element().as_str(), set_name.name()),
        ("c1", &name),
        "{ROW}: SetName's accessors"
    );
    let set_gid = SetGid::new(id("c1"), String::from(" g "));
    assert_eq!(
        (set_gid.element().as_str(), set_gid.gid()),
        ("c1", Some(" g ")),
        "{ROW}: SetGid::new's accessors"
    );
    assert_eq!(
        SetGid::new("c1", "").gid(),
        Some(""),
        "{ROW}: an empty gid is a value, not a removal"
    );
    let removal = SetGid::remove("c1");
    assert_eq!(
        (removal.element().as_str(), removal.gid()),
        ("c1", None),
        "{ROW}: SetGid::remove's accessors"
    );
    let set_flag = SetFlag::new("i1", Flag::IsCounter, true);
    assert_eq!(
        (
            set_flag.element().as_str(),
            set_flag.flag(),
            set_flag.value()
        ),
        ("i1", Flag::IsCounter, true),
        "{ROW}: SetFlag's accessors"
    );
    for flag in [Flag::IsCounter, Flag::IsCitation] {
        assert!(
            !SetFlag::new("i1", flag, false).value(),
            "{ROW}: SetFlag::value does not hold false for {}",
            flag.name()
        );
    }
    let declaration = SetAssertionDeclaration::new("c1", AssertionDeclaration::Defeated);
    assert_eq!(
        (declaration.element().as_str(), declaration.declaration()),
        ("c1", AssertionDeclaration::Defeated),
        "{ROW}: SetAssertionDeclaration's accessors"
    );
    assert_eq!(
        [Flag::IsCitation, Flag::IsAbstract, Flag::IsCounter].map(Flag::name),
        ["isCitation", "isAbstract", "isCounter"],
        "{ROW}: Flag::name"
    );
    // Clone, Debug, PartialEq and Eq.
    assert_eq!(
        set_name.clone(),
        set_name,
        "{ROW}: SetName is not Clone + Eq"
    );
    assert_ne!(
        SetGid::new("c1", ""),
        SetGid::remove("c1"),
        "{ROW}: an empty gid equals a removal"
    );
    assert!(
        format!("{set_flag:?}").contains("i1"),
        "{ROW}: the Debug of SetFlag does not show its element"
    );
}

#[test]
fn lib_edit_008_feature_checks_ignore_content_findings() {
    // Every content rule an attribute edit could break, broken at once: the
    // edits all succeed.
    let mut doc = read(&every_readable_kind(&typed(
        "Claim",
        "c9",
        " citedElement=\"c2\" abstractForm=\"c2\"",
        "<implementationConstraint xmi:id=\"ic9\"/>",
    )));
    for operation in [
        Operation::from(SetGid::new("c1", "dup")),
        SetGid::new("c2", "dup").into(),
        SetFlag::new("c9", Flag::IsCitation, false).into(),
        SetFlag::new("c9", Flag::IsAbstract, true).into(),
        SetAssertionDeclaration::new("c9", AssertionDeclaration::AsCited).into(),
        SetName::new("c9", LangString::default()).into(),
    ] {
        apply(&mut doc, operation);
    }
    let codes: Vec<&str> = validation::validate(&doc)
        .iter()
        .map(|d| d.code().as_str())
        .collect();
    for code in [
        Code::GID_DUPLICATE,
        Code::CITATION_INVALID,
        Code::ABSTRACT_FORM_INVALID,
    ] {
        assert!(
            codes.contains(&code.as_str()),
            "{ROW}: validate does not report {code:?} after the edits: {codes:?}"
        );
    }
}
