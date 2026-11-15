//! Tests for the content and key edits of `UtilityElement`s (LIB-EDIT-012;
//! formal/23-05-08 §8.5, §8.7, §8.12; D-VAL-LAYER, D-PRESERVE; ADR 0025,
//! ADR 0029).
//!
//! `SetContent::new` replaces the `content` of a Description,
//! `ImplementationConstraint`, Note or `TaggedValue` with any `MultiLangString`
//! and `SetContent::remove` removes it (§8.7 `[0..1]`); `SetKey::new`
//! replaces the `key` of a `TaggedValue`, which has no remove form (§8.12
//! `[1]`). Values are held exactly; empty values and repeated languages are
//! accepted and reported by `validate` (SACM23-VAL-010). The role element
//! keeps its `xmi:id`, each value keeps the id of the value at its position,
//! and ids beyond the new length are freed. Each refuses an unknown element
//! (SACM-CMD-001) and a kind without the feature (SACM-CMD-008), in that
//! order. `changed_ids` is the `UtilityElement`, not its owner.
//!
//! Not checked here: that `SetKey` has no remove form (item 3) is a fact of
//! the API, checked by review; this file checks that its key is never
//! absent. The last part of item 6 (the operations join the LIB-EDIT-002
//! set) is in `edit_atomic.rs`.

use sacm::compare::{Difference, semantic_compare};
use sacm::diagnostic::{Code, Severity};
use sacm::edit::{
    self, CreateArgumentPackage, CreateAssuranceCasePackage, CreateClaim, EditError, Mutation,
    Operation, SetContent, SetKey,
};
use sacm::model::{Document, ElementId, ElementKind, LangString, MultiLangString, SacmElement};
use sacm::validation;
use sacm::xmi::{self, Mode, ReadOptions, WriteOptions};

const ROW: &str = "LIB-EDIT-012";

/// The namespace declarations of a bare root: `sacm` and `xmi`.
const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
);

/// The four `UtilityElement`s of [`input`], with their kinds.
const UTILITIES: [(&str, ElementKind); 4] = [
    ("d1", ElementKind::Description),
    ("ic1", ElementKind::ImplementationConstraint),
    ("n1", ElementKind::Note),
    ("t1", ElementKind::TaggedValue),
];

// --- Inputs --------------------------------------------------------------------------

/// An ArgumentPackage root `p` holding:
/// - the Claim `cl`, which holds the Description d1, the
///   `ImplementationConstraint` ic1, the Note n1, and the `TaggedValue`s t1 and
///   t2, both with the key "K" and the content "V";
/// - the Note n9, whose `content` is `c1` with the values `v1` and `v2`;
/// - the `TaggedValue` t9, whose `key` is `k1` with the values `kv1` and `kv2`.
fn input() -> String {
    let tagged = |id: &str| {
        format!(
            "<taggedValue xmi:id=\"{id}\"><key><value lang=\"en\" content=\"K\"/></key>\
             <content><value lang=\"en\" content=\"V\"/></content></taggedValue>"
        )
    };
    format!(
        "<sacm:ArgumentPackage{NS} xmi:id=\"p\"><name content=\"P\"/>\
         <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"cl\"><name content=\"Cl\"/>\
         <description xmi:id=\"d1\"><content><value content=\"D\"/></content></description>\
         <implementationConstraint xmi:id=\"ic1\"/><note xmi:id=\"n1\"/>{}{}</argumentElement>\
         <note xmi:id=\"n9\"><content xmi:id=\"c1\">\
         <value xmi:id=\"v1\" lang=\"en\" content=\"One\"/>\
         <value xmi:id=\"v2\" lang=\"sv\" content=\"Ett\"/></content></note>\
         <taggedValue xmi:id=\"t9\"><key xmi:id=\"k1\">\
         <value xmi:id=\"kv1\" lang=\"en\" content=\"Key\"/>\
         <value xmi:id=\"kv2\" lang=\"sv\" content=\"Nyckel\"/></key></taggedValue>\
         </sacm:ArgumentPackage>",
        tagged("t1"),
        tagged("t2"),
    )
}

// --- Helpers -------------------------------------------------------------------------

fn id(value: &str) -> ElementId {
    ElementId::from(value)
}

fn named(content: &str) -> LangString {
    LangString::new(content).with_lang("en")
}

fn text(values: &[(Option<&str>, Option<&str>)]) -> MultiLangString {
    values
        .iter()
        .map(|&(lang, content)| {
            let mut value = LangString::default();
            value.set_lang(lang.map(str::to_owned));
            value.set_content(content.map(str::to_owned));
            value
        })
        .collect()
}

/// The contents item 1 sets: two languages, empty, a repeated language, and
/// values with absent and empty fields.
fn contents() -> [MultiLangString; 4] {
    [
        text(&[
            (Some("en"), Some("Reviewed")),
            (Some("sv"), Some("Granskad")),
        ]),
        MultiLangString::default(),
        text(&[(Some("en"), Some("a")), (Some("en"), Some("b"))]),
        text(&[(None, None), (Some(""), Some("")), (None, Some("x"))]),
    ]
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

fn doc() -> Document {
    read(&input())
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

/// Whether a caller may give a new Claim under `p` the id `value`: whether
/// the id is free in the document's id space.
fn is_free(doc: &Document, value: &str) -> bool {
    let mut probe = doc.clone();
    match edit::apply(
        &mut probe,
        CreateClaim::new("p", named("probe")).with_id(value),
    ) {
        Ok(_) => true,
        Err(EditError::DuplicateId { .. }) => false,
        Err(other) => panic!("{ROW}: probing the id {value:?}: {other:?}"),
    }
}

fn value_ids(ids: &[Option<ElementId>]) -> Vec<Option<&str>> {
    ids.iter()
        .map(|id| id.as_ref().map(ElementId::as_str))
        .collect()
}

// --- Item 1: every content form on every kind --------------------------------------

#[test]
fn lib_edit_012_each_content_form_is_set_exactly_on_each_utility_kind() {
    for (target, kind) in UTILITIES {
        for content in contents() {
            let mut doc = doc();
            apply(&mut doc, SetContent::new(target, content.clone()));
            assert_eq!(
                element(&doc, target).content(),
                Some(&content),
                "{ROW}: the content of the {kind} after SetContent {content:?}"
            );
        }
        let mut doc = doc();
        apply(&mut doc, SetContent::remove(target));
        assert_eq!(
            element(&doc, target).content(),
            None,
            "{ROW}: the {kind} has a content after SetContent::remove"
        );
        assert_eq!(
            element(&doc, target).kind(),
            kind,
            "{ROW}: setup: the kind of {target:?}"
        );
    }
    // A LangString converts into a content of one value.
    let mut doc = doc();
    apply(&mut doc, SetContent::new("n1", LangString::new("One")));
    assert_eq!(
        element(&doc, "n1").content(),
        Some(&MultiLangString::from(LangString::new("One"))),
        "{ROW}: a LangString content"
    );
}

#[test]
fn lib_edit_012_a_set_content_round_trips_with_absent_and_empty_kept_apart() {
    let mut doc = doc();
    apply(&mut doc, SetContent::remove("d1"));
    apply(&mut doc, SetContent::new("ic1", MultiLangString::default()));
    apply(&mut doc, SetContent::new("n1", contents()[0].clone()));
    apply(&mut doc, SetContent::new("t1", contents()[3].clone()));
    apply(&mut doc, SetKey::new("t1", contents()[2].clone()));
    let back = read(&write(&doc));
    assert_eq!(
        back, doc,
        "{ROW}: the edited contents do not read back equal"
    );
    assert_eq!(
        (
            element(&back, "d1").content().map(MultiLangString::len),
            element(&back, "ic1").content().map(MultiLangString::len)
        ),
        (None, Some(0)),
        "{ROW}: absent and empty content are not kept apart"
    );
}

// --- Item 2: content findings are not refusals ------------------------------------

#[test]
fn lib_edit_012_an_empty_content_or_a_repeated_language_is_reported_by_validate_not_refused() {
    let finding = |doc: &Document, target: &str| -> Vec<(Severity, String)> {
        validation::validate(doc)
            .into_iter()
            .filter(|d| {
                d.code() == Code::MULTIPLICITY_VIOLATION
                    && d.affected().first() == Some(&id(target))
            })
            .map(|d| (d.severity(), d.requirement_id().to_owned()))
            .collect()
    };
    for (target, kind) in UTILITIES {
        for content in [contents()[1].clone(), contents()[2].clone()] {
            let mut doc = doc();
            apply(&mut doc, SetContent::new(target, content.clone()));
            assert_eq!(
                finding(&doc, target),
                [(Severity::Error, "SACM23-VAL-010".to_owned())],
                "{ROW}: the {kind} with the content {content:?} is not one SACM23-VAL-010 Error"
            );
        }
    }
    // The same for a key.
    let mut doc = doc();
    apply(&mut doc, SetKey::new("t1", MultiLangString::default()));
    assert_eq!(
        finding(&doc, "t1"),
        [(Severity::Error, "SACM23-VAL-010".to_owned())],
        "{ROW}: an empty key is not one SACM23-VAL-010 Error"
    );
}

// --- Item 3: a kind without the feature ------------------------------------------

#[test]
fn lib_edit_012_content_on_a_model_element_or_a_key_on_another_kind_is_feature_not_applicable() {
    let cases: Vec<(Operation, &str, ElementKind, &str)> = vec![
        (
            SetContent::new("cl", LangString::new("x")).into(),
            "cl",
            ElementKind::Claim,
            "content",
        ),
        (
            SetContent::remove("cl").into(),
            "cl",
            ElementKind::Claim,
            "content",
        ),
        (
            SetContent::new("p", LangString::new("x")).into(),
            "p",
            ElementKind::ArgumentPackage,
            "content",
        ),
        (
            SetKey::new("n1", LangString::new("k")).into(),
            "n1",
            ElementKind::Note,
            "key",
        ),
        (
            SetKey::new("d1", LangString::new("k")).into(),
            "d1",
            ElementKind::Description,
            "key",
        ),
        (
            SetKey::new("ic1", LangString::new("k")).into(),
            "ic1",
            ElementKind::ImplementationConstraint,
            "key",
        ),
        (
            SetKey::new("cl", LangString::new("k")).into(),
            "cl",
            ElementKind::Claim,
            "key",
        ),
    ];
    for (operation, target, kind, feature) in cases {
        let mut doc = doc();
        let error = refuse(&mut doc, operation.clone());
        assert!(
            matches!(
                &error,
                EditError::FeatureNotApplicable { element, kind: held, feature: held_feature, .. }
                    if element.as_str() == target && *held == kind && *held_feature == feature
            ),
            "{ROW}: {operation:?}: {error:?}"
        );
        assert_eq!(
            error.code(),
            Code::CMD_FEATURE_NOT_APPLICABLE,
            "{ROW}: {operation:?}: the code"
        );
    }
    let mut doc = doc();
    let error = refuse(&mut doc, SetContent::new("cl", LangString::new("x")));
    assert_eq!(
        error.to_string(),
        "the Claim \"cl\" has no content",
        "{ROW}: the Display for content"
    );
    let error = refuse(&mut doc, SetKey::new("n1", LangString::new("k")));
    assert_eq!(
        error.to_string(),
        "the Note \"n1\" has no key",
        "{ROW}: the Display for a key"
    );
}

#[test]
fn lib_edit_012_a_key_edit_always_holds_a_key() {
    // `SetKey` holds a key, not an optional one, so it cannot remove one.
    let operation = SetKey::new("t1", LangString::new("K2"));
    let key: &MultiLangString = operation.key();
    assert_eq!(
        key,
        &MultiLangString::from(LangString::new("K2")),
        "{ROW}: SetKey::key"
    );
    // Even an empty key leaves the TaggedValue with a key.
    let mut doc = doc();
    apply(&mut doc, SetKey::new("t1", MultiLangString::default()));
    assert_eq!(
        element(&doc, "t1").key(),
        Some(&MultiLangString::default()),
        "{ROW}: an empty key is not held as a key"
    );
}

// --- Item 4: tagged values with equal keys ---------------------------------------

#[test]
fn lib_edit_012_editing_one_of_two_tagged_values_with_equal_keys_leaves_the_other_unchanged() {
    let mut doc = doc();
    let before = doc.clone();
    assert_eq!(
        element(&doc, "t1").key(),
        element(&doc, "t2").key(),
        "{ROW}: setup: t1 and t2 have different keys"
    );
    apply(
        &mut doc,
        SetKey::new("t1", LangString::new("K2").with_lang("en")),
    );
    apply(
        &mut doc,
        SetContent::new("t1", LangString::new("V2").with_lang("en")),
    );
    assert_eq!(
        (element(&doc, "t2").key(), element(&doc, "t2").content()),
        (
            element(&before, "t2").key(),
            element(&before, "t2").content()
        ),
        "{ROW}: editing t1 changed t2"
    );
    let differences = semantic_compare(&before, &doc);
    assert!(
        differences
            .iter()
            .all(|d| d.element_id() == Some(&id("t1"))),
        "{ROW}: editing t1 changed another element: {differences:?}"
    );
    assert!(
        differences
            .iter()
            .any(|d| matches!(d, Difference::Key { .. }))
            && differences
                .iter()
                .any(|d| matches!(d, Difference::Content { .. })),
        "{ROW}: the edits are not a Key and a Content difference: {differences:?}"
    );
}

// --- Item 5: ids kept by position ------------------------------------------------

#[test]
fn lib_edit_012_a_replaced_content_keeps_its_id_and_the_value_ids_by_position() {
    let mut doc = doc();
    assert!(
        !is_free(&doc, "v2") && !is_free(&doc, "c1"),
        "{ROW}: setup: the content ids are free before the edit"
    );
    let mutation = apply(
        &mut doc,
        SetContent::new("n9", LangString::new("Only").with_lang("en")),
    );
    assert_eq!(mutation.changed_ids(), [id("n9")], "{ROW}: changed_ids");
    let n9 = element(&doc, "n9");
    assert_eq!(
        (
            n9.content_id().map(ElementId::as_str),
            value_ids(n9.content_value_ids())
        ),
        (Some("c1"), vec![Some("v1")]),
        "{ROW}: the replaced content does not keep c1 and v1"
    );
    assert!(is_free(&doc, "v2"), "{ROW}: v2 is still in use");
    assert!(
        !is_free(&doc, "c1") && !is_free(&doc, "v1"),
        "{ROW}: c1 or v1 is free while the content holds it"
    );
    let written = write(&doc);
    assert!(
        written.contains("<content xmi:id=\"c1\"") && written.contains("xmi:id=\"v1\""),
        "{ROW}: the kept ids are not written: {written}"
    );
    assert!(
        !written.contains("\"v2\""),
        "{ROW}: the freed v2 is written: {written}"
    );
    // A longer content keeps the ids it has and adds values without one.
    let mut longer = read(&input());
    apply(
        &mut longer,
        SetContent::new(
            "n9",
            text(&[
                (Some("en"), Some("a")),
                (Some("sv"), Some("b")),
                (Some("de"), Some("c")),
            ]),
        ),
    );
    assert_eq!(
        value_ids(element(&longer, "n9").content_value_ids()),
        [Some("v1"), Some("v2"), None],
        "{ROW}: a longer content does not keep v1 and v2 by position"
    );
}

#[test]
fn lib_edit_012_removing_a_content_frees_every_id_of_it() {
    let mut doc = doc();
    apply(&mut doc, SetContent::remove("n9"));
    let n9 = element(&doc, "n9");
    assert_eq!(
        (n9.content(), n9.content_id(), n9.content_value_ids().len()),
        (None, None, 0),
        "{ROW}: the removed content is still held"
    );
    for value in ["c1", "v1", "v2"] {
        assert!(is_free(&doc, value), "{ROW}: {value:?} is still in use");
    }
}

#[test]
fn lib_edit_012_a_replaced_key_keeps_its_id_and_the_value_ids_by_position() {
    let mut doc = doc();
    apply(
        &mut doc,
        SetKey::new("t9", LangString::new("Key").with_lang("en")),
    );
    let t9 = element(&doc, "t9");
    assert_eq!(
        (
            t9.key_id().map(ElementId::as_str),
            value_ids(t9.key_value_ids())
        ),
        (Some("k1"), vec![Some("kv1")]),
        "{ROW}: the replaced key does not keep k1 and kv1"
    );
    assert!(is_free(&doc, "kv2"), "{ROW}: kv2 is still in use");
    assert!(!is_free(&doc, "k1"), "{ROW}: k1 is free");
}

#[test]
fn lib_edit_012_a_freed_value_id_may_be_generated() {
    // The values of n8 carry the ids claim_1 and claim_2, so generation
    // skips both until the content no longer holds claim_2.
    let input = format!(
        "<sacm:ArgumentPackage{NS} xmi:id=\"p\"><name content=\"P\"/>\
         <note xmi:id=\"n8\"><content>\
         <value xmi:id=\"claim_1\" content=\"a\"/><value xmi:id=\"claim_2\" content=\"b\"/>\
         </content></note></sacm:ArgumentPackage>"
    );
    let mut doc = read(&input);
    assert_eq!(
        next_generated_ids(&doc)[2],
        id("claim_3"),
        "{ROW}: setup: generation does not skip the value ids"
    );
    apply(&mut doc, SetContent::new("n8", LangString::new("a")));
    assert_eq!(
        next_generated_ids(&doc)[2],
        id("claim_2"),
        "{ROW}: the freed value id claim_2 is not generated"
    );
}

#[test]
fn lib_edit_012_a_freed_value_id_at_or_below_the_counter_is_not_generated() {
    // A value of n8 carries the id claim_2, so the first two generated
    // Claims are claim_1 and claim_3 and the Claim counter stands at 3.
    let input = format!(
        "<sacm:ArgumentPackage{NS} xmi:id=\"p\"><name content=\"P\"/>\
         <note xmi:id=\"n8\"><content>\
         <value xmi:id=\"v1\" content=\"a\"/><value xmi:id=\"claim_2\" content=\"b\"/>\
         </content></note></sacm:ArgumentPackage>"
    );
    let mut doc = read(&input);
    let generated: Vec<ElementId> = ["a", "b"]
        .into_iter()
        .map(|name| apply(&mut doc, CreateClaim::new("p", named(name))).created_ids()[0].clone())
        .collect();
    assert_eq!(
        generated,
        [id("claim_1"), id("claim_3")],
        "{ROW}: setup: generation does not skip the value id claim_2"
    );
    apply(&mut doc, SetContent::new("n8", LangString::new("a")));
    assert!(
        is_free(&doc, "claim_2"),
        "{ROW}: setup: shortening the content does not free claim_2"
    );
    let next = apply(&mut doc, CreateClaim::new("p", named("c")));
    assert_eq!(
        next.created_ids(),
        [id("claim_4")],
        "{ROW}: the freed claim_2, below the Claim counter, is generated again"
    );
}

// --- Item 6: changed_ids and refusals ----------------------------------------------

#[test]
fn lib_edit_012_changed_ids_is_the_utility_element_not_its_owner() {
    let cases: Vec<(Operation, Vec<ElementId>)> = vec![
        (
            SetContent::new("d1", LangString::new("New")).into(),
            vec![id("d1")],
        ),
        (
            SetContent::new("d1", LangString::new("D")).into(),
            Vec::new(),
        ),
        (SetContent::remove("ic1").into(), Vec::new()),
        (SetContent::remove("d1").into(), vec![id("d1")]),
        (
            SetContent::new("ic1", MultiLangString::default()).into(),
            vec![id("ic1")],
        ),
        (
            SetKey::new("t1", LangString::new("K").with_lang("en")).into(),
            Vec::new(),
        ),
        (
            SetKey::new("t1", LangString::new("K")).into(),
            vec![id("t1")],
        ),
        (
            SetContent::new("t1", LangString::new("V").with_lang("en")).into(),
            Vec::new(),
        ),
    ];
    for (operation, expected) in cases {
        let mut doc = doc();
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
        assert_eq!(
            doc.element_count(),
            before.element_count(),
            "{ROW}: {operation:?} created or deleted an element"
        );
        if expected.is_empty() {
            assert!(
                doc == before,
                "{ROW}: {operation:?} changed nothing but the document differs"
            );
        }
    }
}

#[test]
fn lib_edit_012_an_unknown_element_is_element_not_found_and_changes_nothing() {
    for operation in [
        Operation::from(SetContent::new("nowhere", LangString::new("x"))),
        SetContent::remove("nowhere").into(),
        SetKey::new("nowhere", LangString::new("k")).into(),
    ] {
        let mut doc = doc();
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
    }
}

#[test]
fn lib_edit_012_operations_hold_what_they_are_given_and_name_their_struct() {
    let content = MultiLangString::from(LangString::new("x"));
    let set = SetContent::new(id("n1"), content.clone());
    assert_eq!(
        (set.element().as_str(), set.content()),
        ("n1", Some(&content)),
        "{ROW}: SetContent::new's accessors"
    );
    let remove = SetContent::remove("n1");
    assert_eq!(
        (remove.element().as_str(), remove.content()),
        ("n1", None),
        "{ROW}: SetContent::remove's accessors"
    );
    assert_ne!(
        SetContent::new("n1", MultiLangString::default()),
        SetContent::remove("n1"),
        "{ROW}: an empty content equals a removal"
    );
    let key = SetKey::new("t1", content.clone());
    assert_eq!(
        (key.element().as_str(), key.key()),
        ("t1", &content),
        "{ROW}: SetKey's accessors"
    );
    for (operation, name) in [
        (Operation::from(set), "SetContent"),
        (remove.into(), "SetContent"),
        (key.into(), "SetKey"),
    ] {
        assert_eq!(operation.operation_name(), name, "{ROW}: operation_name()");
    }
}
