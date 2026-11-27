//! `SetTaggedValue`: set the content of the tagged value with a plain key,
//! creating it when there is none (LIB-EDIT-023; `lasrod/libSACM#113` A2;
//! SACM 2.3 §8.12; slice 0082).
//!
//! - With no tagged value whose key has the content `key`, it creates one as
//!   the element's last child, with the key `MultiLangString::from(key)` and
//!   a generated id (`tagged_value_<n>`): created, nothing changed.
//! - Otherwise it sets the content of the first such tagged value, the one
//!   `SacmElement::tagged_value(key)` finds: changed, or nothing when the
//!   content is the one held. The ids of the content are kept by position
//!   (ADR 0029); a freed value id leaves the id space.
//! - It refuses, in order: an unknown element (SACM-CMD-001), an element
//!   whose kind holds no tagged value (SACM-CMD-008, `taggedValue`), and the
//!   `ExpressionLangString`s of the content as `SetContent` refuses them;
//!   on the set path, an expression the matched tagged value already holds
//!   is kept, even dangling.
//! - `check`, `preview` and `apply` agree, a refusal changes nothing, the
//!   result writes and reads back, a batch's second set sees the first, and
//!   the JSON is the golden sample of `operations.json`.

use sacm::diagnostic::Code;
use sacm::edit::{
    self, BatchError, CreateArgumentPackage, CreateClaim, EditError, Mutation, Operation,
    SetTaggedValue,
};
use sacm::model::{Document, ElementId, ElementKind, LangString, MultiLangString, SacmElement};
use sacm::xmi::{self, Mode, ReadOptions, WriteOptions};

const ROW: &str = "LIB-EDIT-023";

/// The namespace declarations of a root: `sacm` and `xmi`.
const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
);

const ELS: &str = "xmi:type=\"sacm:ExpressionLangString\"";

// --- Helpers -----------------------------------------------------------------------------

fn read_doc(what: &str, input: &str) -> Document {
    match xmi::read(input.as_bytes(), &ReadOptions::default()) {
        Ok(output) => output.into_document(),
        Err(error) => panic!("{ROW}: setup: {what}: a strict read refuses: {error}"),
    }
}

fn apply(what: &str, doc: &mut Document, operation: impl Into<Operation>) -> Mutation {
    edit::apply(doc, operation).unwrap_or_else(|error| panic!("{ROW}: {what}: refused: {error}"))
}

fn refused(what: &str, doc: &Document, operation: impl Into<Operation>) -> EditError {
    match edit::apply(&mut doc.clone(), operation) {
        Ok(mutation) => panic!("{ROW}: {what}: accepted: {mutation:?}"),
        Err(error) => error,
    }
}

fn element<'d>(doc: &'d Document, id: &str) -> SacmElement<'d> {
    doc.get(id)
        .unwrap_or_else(|| panic!("{ROW}: setup: the document has no element {id:?}"))
}

fn ids(list: &[ElementId]) -> Vec<&str> {
    list.iter().map(ElementId::as_str).collect()
}

fn child_ids(e: SacmElement<'_>) -> Vec<String> {
    e.children().map(|c| c.id().as_str().to_owned()).collect()
}

/// A tagged value's key, content, content id and content value ids.
type Held = (
    Option<MultiLangString>,
    Option<MultiLangString>,
    Option<ElementId>,
    Vec<Option<ElementId>>,
);

fn held(doc: &Document, id: &str) -> Held {
    let tag = element(doc, id);
    (
        tag.key().cloned(),
        tag.content().cloned(),
        tag.content_id().cloned(),
        tag.content_value_ids().to_vec(),
    )
}

/// The case of most tests: in the AssuranceCasePackage `case`, the
/// TerminologyPackage `terms` with the Term `blade`, and the ArgumentPackage
/// `p` with:
/// - `c1`: no tagged value; the Notes `n1` and `nx` (whose content value is
///   an `ExpressionLangString` with no expression);
/// - `c2`: the Note `n2`, the tagged value `t1` (key `gsn:id`, content
///   `G1`) and the Note `n3`;
/// - `c3`: the tagged value `tv` (key `K`), whose content `tc` has the
///   values `v0` and `v1`;
/// - `c4`: the tagged values `k1` and `k2`, both with the key `K`;
/// - `c5`: the tagged value `tg` (key `K`), whose content value links the
///   expression `ghost`, which names no element.
fn start_input() -> String {
    let tagged = |id: &str, key: &str, content: &str| {
        format!(
            "<taggedValue xmi:id=\"{id}\"><key><value content=\"{key}\"/></key>{content}</taggedValue>"
        )
    };
    let plain = |text: &str| format!("<content><value content=\"{text}\"/></content>");
    let claim = |id: &str, body: &str| {
        format!(
            "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"{id}\"><name content=\"{id}\"/>{body}</argumentElement>"
        )
    };
    let note = |id: &str| format!("<note xmi:id=\"{id}\">{}</note>", plain(id));
    format!(
        "<sacm:AssuranceCasePackage{NS} xmi:id=\"case\"><name content=\"Case\"/>\
         <terminologyPackage xmi:id=\"terms\"><name content=\"Terms\"/>\
         <terminologyElement xmi:type=\"sacm:Term\" xmi:id=\"blade\" value=\"blade\">\
         <name content=\"Blade\"/></terminologyElement></terminologyPackage>\
         <argumentPackage xmi:id=\"p\"><name content=\"P\"/>{}{}{}{}{}</argumentPackage>\
         </sacm:AssuranceCasePackage>",
        claim(
            "c1",
            &format!(
                "{}<note xmi:id=\"nx\"><content><value {ELS} lang=\"en\" content=\"x\"/></content></note>",
                note("n1")
            )
        ),
        claim(
            "c2",
            &[note("n2"), tagged("t1", "gsn:id", &plain("G1")), note("n3")].concat()
        ),
        claim(
            "c3",
            &tagged(
                "tv",
                "K",
                "<content xmi:id=\"tc\"><value xmi:id=\"v0\" content=\"a\"/>\
                 <value xmi:id=\"v1\" content=\"b\"/></content>"
            )
        ),
        claim(
            "c4",
            &[
                tagged("k1", "K", &plain("first")),
                tagged("k2", "K", &plain("second"))
            ]
            .concat()
        ),
        claim(
            "c5",
            &tagged(
                "tg",
                "K",
                &format!("<content><value {ELS} content=\"g\" expression=\"ghost\"/></content>")
            )
        ),
    )
}

fn start() -> Document {
    read_doc("the start document", &start_input())
}

// --- Creating and setting ----------------------------------------------------------------

#[test]
fn lib_edit_023_creates_the_tagged_value_when_none_has_the_key() {
    let mut doc = start();
    let operation = SetTaggedValue::new("c1", "gsn:id", "G1");
    assert_eq!(
        (
            operation.element().as_str(),
            operation.key(),
            operation.content()
        ),
        ("c1", "gsn:id", &MultiLangString::from("G1")),
        "{ROW}: the accessors"
    );
    let made = apply("a create", &mut doc, operation);
    assert_eq!(
        ids(made.created_ids()),
        ["tagged_value_1"],
        "{ROW}: created_ids of a create"
    );
    assert!(
        made.changed_ids().is_empty() && made.deleted_ids().is_empty(),
        "{ROW}: a create changes or deletes: {made:?}"
    );
    assert_eq!(
        made.created_id().map(ElementId::as_str),
        Some("tagged_value_1"),
        "{ROW}: created_id of a create"
    );
    let c1 = element(&doc, "c1");
    assert_eq!(
        child_ids(c1),
        ["n1", "nx", "tagged_value_1"],
        "{ROW}: the new tagged value is not c1's last child"
    );
    let tag = element(&doc, "tagged_value_1");
    assert_eq!(tag.kind(), ElementKind::TaggedValue, "{ROW}: the new kind");
    assert_eq!(
        (tag.key(), tag.content()),
        (
            Some(&MultiLangString::from("gsn:id")),
            Some(&MultiLangString::from("G1"))
        ),
        "{ROW}: the new tagged value's key and content"
    );
    assert_eq!(
        c1.tagged_value("gsn:id").map(|found| found.id().as_str()),
        Some("tagged_value_1"),
        "{ROW}: tagged_value does not find the new tagged value"
    );
    // A create's defaults, as CreateTaggedValue's.
    assert!(
        tag.gid().is_none()
            && !tag.is_citation()
            && tag.cited_element().is_none()
            && !tag.is_abstract()
            && tag.abstract_form().is_none(),
        "{ROW}: the new tagged value has no default gid, flags or references"
    );
}

#[test]
fn lib_edit_023_sets_the_content_of_the_tagged_value_with_the_key() {
    let mut doc = start();
    let before = child_ids(element(&doc, "c2"));
    let set = apply("a set", &mut doc, SetTaggedValue::new("c2", "gsn:id", "G2"));
    assert_eq!(
        ids(set.changed_ids()),
        ["t1"],
        "{ROW}: changed_ids of a set"
    );
    assert!(
        set.created_ids().is_empty() && set.deleted_ids().is_empty(),
        "{ROW}: a set creates or deletes: {set:?}"
    );
    assert_eq!(set.created_id(), None, "{ROW}: created_id of a set");
    let t1 = element(&doc, "t1");
    assert_eq!(
        (t1.key(), t1.content()),
        (
            Some(&MultiLangString::from("gsn:id")),
            Some(&MultiLangString::from("G2"))
        ),
        "{ROW}: the key and content after a set"
    );
    assert_eq!(
        child_ids(element(&doc, "c2")),
        before,
        "{ROW}: a set moves the tagged value"
    );
}

#[test]
fn lib_edit_023_setting_the_content_held_changes_nothing() {
    let mut doc = start();
    let tolerant = WriteOptions::default().with_mode(Mode::Tolerant);
    let written = |doc: &Document| {
        xmi::write(doc, &tolerant).unwrap_or_else(|error| panic!("{ROW}: setup: write: {error}"))
    };
    let before = written(&doc);
    let same = apply(
        "the content held",
        &mut doc,
        SetTaggedValue::new("c2", "gsn:id", "G1"),
    );
    assert!(
        same.created_ids().is_empty()
            && same.changed_ids().is_empty()
            && same.deleted_ids().is_empty(),
        "{ROW}: setting the content held reports ids: {same:?}"
    );
    assert_eq!(
        written(&doc),
        before,
        "{ROW}: setting the content held changes the written bytes"
    );
}

#[test]
fn lib_edit_023_replaces_the_whole_content_keeping_ids_by_position() {
    let mut doc = start();
    assert!(
        doc.is_id_in_use("v1"),
        "{ROW}: setup: v1 is not in the id space"
    );
    let set = apply(
        "a set of one value",
        &mut doc,
        SetTaggedValue::new("c3", "K", "z"),
    );
    assert_eq!(ids(set.changed_ids()), ["tv"], "{ROW}: changed_ids");
    let tv = element(&doc, "tv");
    assert_eq!(
        tv.content(),
        Some(&MultiLangString::from("z")),
        "{ROW}: the whole content is not replaced"
    );
    assert_eq!(
        tv.content_id().map(ElementId::as_str),
        Some("tc"),
        "{ROW}: the content's id is not kept"
    );
    assert_eq!(
        tv.content_value_ids(),
        [Some(ElementId::from("v0"))],
        "{ROW}: value 0 does not keep its id"
    );
    assert!(
        !doc.is_id_in_use("v1"),
        "{ROW}: the id of the value no longer held is still in use"
    );
    apply(
        "a create given the freed id",
        &mut doc,
        CreateClaim::new("p", LangString::new("C")).with_id("v1"),
    );
}

#[test]
fn lib_edit_023_sets_only_the_first_tagged_value_with_the_key() {
    let mut doc = start();
    let second = held(&doc, "k2");
    let set = apply(
        "a set of K",
        &mut doc,
        SetTaggedValue::new("c4", "K", "third"),
    );
    assert_eq!(ids(set.changed_ids()), ["k1"], "{ROW}: changed_ids");
    assert_eq!(
        element(&doc, "k1").content(),
        Some(&MultiLangString::from("third")),
        "{ROW}: the first tagged value with the key is not set"
    );
    assert_eq!(
        held(&doc, "k2"),
        second,
        "{ROW}: the second tagged value with the key changed"
    );
}

/// One Claim per key case of the plain-key rule (LIB-API-013, acceptances
/// 20 to 22).
fn key_cases_input() -> String {
    let tagged = |id: &str, key: Option<&str>| {
        let key = key.map_or_else(String::new, |values| format!("<key>{values}</key>"));
        format!(
            "<taggedValue xmi:id=\"{id}\">{key}<content><value content=\"V\"/></content></taggedValue>"
        )
    };
    let claim = |id: &str, body: &str| {
        format!(
            "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"{id}\"><name content=\"{id}\"/>{body}</argumentElement>"
        )
    };
    let value = |content: &str| format!("<value content=\"{content}\"/>");
    let body = [
        claim(
            "c_abb",
            &[
                tagged("ta", Some(&value("A"))),
                tagged("tb1", Some(&value("B"))),
                tagged("tb2", Some(&value("B"))),
            ]
            .concat(),
        ),
        claim(
            "c_multi",
            &tagged(
                "tm",
                Some(
                    "<value lang=\"en\" content=\"Identifier\"/>\
                     <value lang=\"sv\" content=\"Identifierare\"/>",
                ),
            ),
        ),
        claim("c_exact", &tagged("tx", Some(&value("ASIL")))),
        claim(
            "c_lang",
            &tagged("tl", Some("<value lang=\"en\" content=\"L\"/>")),
        ),
        claim(
            "c_absent",
            &tagged("t_absent", Some("<value lang=\"en\"/>")),
        ),
        claim("c_empty", &tagged("t_empty", Some(&value("")))),
        claim(
            "c_expr",
            &tagged(
                "t_expr",
                Some(&format!(
                    "<value {ELS} lang=\"en\" content=\"E\" expression=\"ghost\"/>"
                )),
            ),
        ),
        claim(
            "c_nokey",
            &[tagged("t_nokey", None), tagged("t_emptykey", Some(""))].concat(),
        ),
    ]
    .concat();
    format!(
        "<sacm:ArgumentPackage{NS} xmi:id=\"p\"><name content=\"P\"/>{body}</sacm:ArgumentPackage>"
    )
}

#[test]
fn lib_edit_023_finds_the_tagged_value_that_tagged_value_finds() {
    let doc = read_doc("the key cases", &key_cases_input());
    let probes = [
        ("c_abb", "A"),
        ("c_abb", "B"),
        ("c_abb", "C"),
        ("c_multi", "Identifier"),
        ("c_multi", "Identifierare"),
        ("c_multi", "en"),
        ("c_exact", "ASIL"),
        ("c_exact", "asil"),
        ("c_exact", " ASIL"),
        ("c_lang", "L"),
        ("c_lang", "en"),
        ("c_absent", ""),
        ("c_empty", ""),
        ("c_expr", "E"),
        ("c_nokey", ""),
        ("c_nokey", "V"),
    ];
    let mut set_paths = 0_usize;
    let mut create_paths = 0_usize;
    for (holder, key) in probes {
        let what = format!("SetTaggedValue({holder:?}, {key:?})");
        let expected = element(&doc, holder)
            .tagged_value(key)
            .map(|found| found.id().clone());
        let mut edited = doc.clone();
        let mutation = apply(&what, &mut edited, SetTaggedValue::new(holder, key, "set"));
        if let Some(id) = &expected {
            set_paths += 1;
            assert_eq!(
                (mutation.created_ids(), mutation.changed_ids()),
                (&[] as &[ElementId], std::slice::from_ref(id)),
                "{ROW}: {what} does not set exactly the tagged value tagged_value finds"
            );
            assert_eq!(
                element(&edited, id.as_str()).content(),
                Some(&MultiLangString::from("set")),
                "{ROW}: {what}: the content is not set"
            );
        } else {
            create_paths += 1;
            let [created] = mutation.created_ids() else {
                panic!("{ROW}: {what} creates no one tagged value: {mutation:?}");
            };
            assert!(
                mutation.changed_ids().is_empty(),
                "{ROW}: {what}: a create changes {:?}",
                mutation.changed_ids()
            );
            assert_eq!(
                element(&edited, created.as_str()).key(),
                Some(&MultiLangString::from(key)),
                "{ROW}: {what}: the key created"
            );
            assert_eq!(
                element(&edited, holder)
                    .tagged_value(key)
                    .map(|found| found.id().clone()),
                Some(created.clone()),
                "{ROW}: {what}: tagged_value does not find the created tagged value"
            );
        }
    }
    assert!(
        set_paths >= 7 && create_paths >= 7,
        "{ROW}: setup: {set_paths} set and {create_paths} create paths"
    );
}

// --- Refusals ------------------------------------------------------------------------------

#[test]
fn lib_edit_023_refuses_an_unknown_element() {
    let doc = start();
    let error = refused(
        "an unknown element",
        &doc,
        SetTaggedValue::new("nope", "K", "V"),
    );
    assert!(
        matches!(&error, EditError::ElementNotFound { element, .. } if element.as_str() == "nope"),
        "{ROW}: not ElementNotFound nope: {error:?}"
    );
    assert_eq!(error.code(), Code::CMD_TARGET_NOT_FOUND, "{ROW}: the code");
    assert_eq!(
        error.to_string(),
        "the document has no element \"nope\" to edit",
        "{ROW}: the Display"
    );
    // Before any check of the content.
    let error = refused(
        "an unknown element with a dangling expression",
        &doc,
        SetTaggedValue::new("nope", "K", LangString::new("x").with_expression("none")),
    );
    assert!(
        matches!(&error, EditError::ElementNotFound { .. }),
        "{ROW}: the element is not checked first: {error:?}"
    );
}

#[test]
fn lib_edit_023_refuses_an_element_that_holds_no_tagged_values() {
    let doc = start();
    let note_error = refused("a Note", &doc, SetTaggedValue::new("n1", "K", "V"));
    assert!(
        matches!(
            &note_error,
            EditError::FeatureNotApplicable {
                element,
                kind: ElementKind::Note,
                feature: "taggedValue",
                ..
            } if element.as_str() == "n1"
        ),
        "{ROW}: not FeatureNotApplicable taggedValue on the Note n1: {note_error:?}"
    );
    assert_eq!(
        note_error.code(),
        Code::CMD_FEATURE_NOT_APPLICABLE,
        "{ROW}: the code"
    );
    assert_eq!(
        note_error.to_string(),
        "the Note \"n1\" has no taggedValue",
        "{ROW}: the Display"
    );
    // A tagged value holds none either; and the kind is checked before the
    // content.
    let error = refused(
        "a tagged value, with a dangling expression",
        &doc,
        SetTaggedValue::new("t1", "K", LangString::new("x").with_expression("none")),
    );
    assert!(
        matches!(
            &error,
            EditError::FeatureNotApplicable {
                kind: ElementKind::TaggedValue,
                feature: "taggedValue",
                ..
            }
        ),
        "{ROW}: a tagged value is not refused for its kind first: {error:?}"
    );
    // With serde, the refusal round-trips with its feature name.
    #[cfg(feature = "serde")]
    {
        let json = serde_json::to_string(&note_error)
            .unwrap_or_else(|failure| panic!("{ROW}: the refusal does not serialize: {failure}"));
        assert_eq!(
            json,
            r#"{"type":"FeatureNotApplicable","code":"SACM-CMD-008","element":"n1","kind":"Note","feature":"taggedValue"}"#,
            "{ROW}: the JSON of the refusal"
        );
        let read: EditError = serde_json::from_str(&json)
            .unwrap_or_else(|failure| panic!("{ROW}: {json} does not read back: {failure}"));
        assert_eq!(read, note_error, "{ROW}: the refusal does not read back");
    }
}

/// The `ExpressionLangString` with no expression that `nx` holds.
fn without_expression(doc: &Document) -> LangString {
    element(doc, "nx")
        .content()
        .and_then(|content| content.values().first().cloned())
        .filter(|value| value.is_expression_lang_string() && value.expression().is_none())
        .unwrap_or_else(|| panic!("{ROW}: setup: nx holds no ExpressionLangString without one"))
}

/// A content of a plain value then `value`.
fn after_plain(value: LangString) -> MultiLangString {
    [LangString::new("first").with_lang("en"), value]
        .into_iter()
        .collect()
}

/// Whether `error` is the refusal the case `expected` names.
fn is_expression_refusal(expected: &str, error: &EditError) -> bool {
    match expected {
        "missing" => matches!(
            error,
            EditError::TooFewReferences {
                end: "expression",
                given: 0,
                required: 1,
                ..
            }
        ),
        "a b" => matches!(
            error,
            EditError::InvalidReference { end: "expression", id, .. } if id.as_str() == "a b"
        ),
        "c1" => matches!(
            error,
            EditError::WrongReferenceType {
                end: "expression",
                id,
                kind: ElementKind::Claim,
                ..
            } if id.as_str() == "c1"
        ),
        other => matches!(
            error,
            EditError::ReferenceNotFound { end: "expression", id, .. } if id.as_str() == other
        ),
    }
}

/// The refused expression values: none, white space, dangling, a Claim.
fn refused_values(doc: &Document) -> [(&'static str, LangString); 4] {
    let x = || LangString::new("x").with_lang("sv");
    [
        ("missing", without_expression(doc)),
        ("a b", x().with_expression("a b")),
        ("none", x().with_expression("none")),
        ("c1", x().with_expression("c1")),
    ]
}

#[test]
fn lib_edit_023_checks_expression_values_as_set_content_does() {
    let doc = start();
    // The create path (c1 has no K) and the set path (c5's tg has K).
    for (path, holder) in [("create", "c1"), ("set", "c5")] {
        for (expected, value) in refused_values(&doc) {
            let what = format!("the {path} path, expression {expected:?}");
            let error = refused(
                &what,
                &doc,
                SetTaggedValue::new(holder, "K", after_plain(value)),
            );
            assert!(
                is_expression_refusal(expected, &error),
                "{ROW}: {what}: {error:?}"
            );
            if expected == "missing" {
                assert_eq!(
                    error.code(),
                    Code::MULTIPLICITY_VIOLATION,
                    "{ROW}: {what}: the code"
                );
            }
        }
        // Check 3 runs over every value before 4 to 6: a missing expression
        // after a dangling one is refused first.
        let both: MultiLangString = [
            LangString::new("x").with_expression("none"),
            without_expression(&doc),
        ]
        .into_iter()
        .collect();
        let error = refused(
            &format!("the {path} path, dangling then missing"),
            &doc,
            SetTaggedValue::new(holder, "K", both),
        );
        assert!(
            is_expression_refusal("missing", &error),
            "{ROW}: the {path} path: a missing expression is not checked before a dangling \
             one: {error:?}"
        );
        // An expression naming the Term is accepted.
        let mut edited = doc.clone();
        apply(
            &format!("the {path} path, expression blade"),
            &mut edited,
            SetTaggedValue::new(holder, "K", LangString::new("x").with_expression("blade")),
        );
    }
    // The set path keeps the dangling expression tg already holds, as read.
    let mut edited = doc.clone();
    let kept = apply(
        "the set path, keeping ghost",
        &mut edited,
        SetTaggedValue::new("c5", "K", LangString::new("y").with_expression("ghost")),
    );
    assert_eq!(ids(kept.changed_ids()), ["tg"], "{ROW}: keeping ghost");
    // The create path holds no expression: ghost is refused.
    let error = refused(
        "the create path, ghost",
        &doc,
        SetTaggedValue::new("c1", "K", LangString::new("y").with_expression("ghost")),
    );
    assert!(
        is_expression_refusal("ghost", &error),
        "{ROW}: the create path does not refuse ghost: {error:?}"
    );
}

/// Every case of the tests above, as a document and an operation.
fn all_cases() -> Vec<(String, Document, Operation)> {
    let doc = start();
    let mut cases: Vec<(String, Document, Operation)> = vec![
        (
            "a create".to_owned(),
            doc.clone(),
            SetTaggedValue::new("c1", "gsn:id", "G1").into(),
        ),
        (
            "a set".to_owned(),
            doc.clone(),
            SetTaggedValue::new("c2", "gsn:id", "G2").into(),
        ),
        (
            "the content held".to_owned(),
            doc.clone(),
            SetTaggedValue::new("c2", "gsn:id", "G1").into(),
        ),
        (
            "ids by position".to_owned(),
            doc.clone(),
            SetTaggedValue::new("c3", "K", "z").into(),
        ),
        (
            "the first of two".to_owned(),
            doc.clone(),
            SetTaggedValue::new("c4", "K", "third").into(),
        ),
        (
            "an unknown element".to_owned(),
            doc.clone(),
            SetTaggedValue::new("nope", "K", "V").into(),
        ),
        (
            "a Note".to_owned(),
            doc.clone(),
            SetTaggedValue::new("n1", "K", "V").into(),
        ),
        (
            "keeping ghost".to_owned(),
            doc.clone(),
            SetTaggedValue::new("c5", "K", LangString::new("y").with_expression("ghost")).into(),
        ),
    ];
    for (path, holder) in [("create", "c1"), ("set", "c5")] {
        for (expected, value) in refused_values(&doc) {
            cases.push((
                format!("the {path} path, expression {expected:?}"),
                doc.clone(),
                SetTaggedValue::new(holder, "K", after_plain(value)).into(),
            ));
        }
    }
    cases
}

#[test]
fn lib_edit_023_check_preview_and_apply_agree() {
    let mut refusals = 0_usize;
    for (what, doc, operation) in all_cases() {
        let checked = edit::check(&doc, &operation);
        let previewed = edit::preview(&doc, &operation);
        let mut edited = doc.clone();
        let applied = edit::apply(&mut edited, operation.clone());
        assert_eq!(
            checked,
            applied.clone().map(drop),
            "{ROW}: {what}: check does not answer as apply"
        );
        match (&previewed, &applied) {
            (Ok(preview), Ok(mutation)) => assert_eq!(
                preview.mutation(),
                mutation,
                "{ROW}: {what}: the preview's mutation is not apply's"
            ),
            (Err(previewed), Err(error)) => {
                assert_eq!(previewed, error, "{ROW}: {what}: preview refuses otherwise");
            }
            _ => panic!("{ROW}: {what}: preview {previewed:?} and apply {applied:?} disagree"),
        }
        if applied.is_err() {
            refusals += 1;
            assert_eq!(edited, doc, "{ROW}: {what}: a refusal changed the document");
            assert_eq!(
                edited.revision(),
                doc.revision(),
                "{ROW}: {what}: a refusal changed the revision"
            );
            let next = |d: &Document| {
                let mut copy = d.clone();
                apply(
                    "the next CreateClaim",
                    &mut copy,
                    CreateClaim::new("p", LangString::new("N")),
                )
                .created_ids()
                .to_vec()
            };
            assert_eq!(
                next(&edited),
                next(&doc),
                "{ROW}: {what}: a refusal changed the next generated id"
            );
        }
    }
    assert!(refusals >= 10, "{ROW}: setup: only {refusals} refusals");
}

#[test]
fn lib_edit_023_operation_name_is_set_tagged_value() {
    let operation = Operation::from(SetTaggedValue::new("c1", "ASIL", "D"));
    assert!(
        matches!(&operation, Operation::SetTaggedValue(set) if set.key() == "ASIL"),
        "{ROW}: not the SetTaggedValue variant: {operation:?}"
    );
    assert_eq!(
        operation.operation_name(),
        "SetTaggedValue",
        "{ROW}: the operation name"
    );
}

#[test]
fn lib_edit_023_a_set_tagged_value_writes_and_reads_back() {
    let mut doc = Document::new();
    apply(
        "p",
        &mut doc,
        CreateArgumentPackage::root(LangString::new("P")).with_id("p"),
    );
    apply(
        "c1",
        &mut doc,
        CreateClaim::new("p", LangString::new("C1")).with_id("c1"),
    );
    apply(
        "a create",
        &mut doc,
        SetTaggedValue::new("c1", "gsn:id", "G1"),
    );
    apply(
        "a set",
        &mut doc,
        SetTaggedValue::new(
            "c1",
            "gsn:id",
            [
                LangString::new("G2").with_lang("en"),
                LangString::new("G2").with_lang("sv"),
            ]
            .into_iter()
            .collect::<MultiLangString>(),
        ),
    );
    apply(
        "a second key",
        &mut doc,
        SetTaggedValue::new("c1", "ASIL", String::from("D")),
    );
    let text = xmi::write(&doc, &WriteOptions::default()).unwrap_or_else(|error| {
        panic!("{ROW}: a strict write refuses the edited document: {error}")
    });
    let back = read_doc("the written document", &text);
    assert_eq!(
        back, doc,
        "{ROW}: the document read back is not the edited one"
    );
    assert_eq!(
        element(&back, "c1").tagged_values().count(),
        2,
        "{ROW}: the tagged values read back"
    );
}

#[test]
fn lib_edit_023_in_a_batch_a_second_set_sees_the_first() {
    let mut doc = start();
    let batch = edit::batch(&mut doc, |tx| -> Result<(), BatchError> {
        tx.apply(SetTaggedValue::new("c1", "K", "one"))?;
        tx.apply(SetTaggedValue::new("c1", "K", "two"))?;
        Ok(())
    })
    .unwrap_or_else(|error| panic!("{ROW}: the batch is refused: {error}"));
    let [first, second] = batch.mutations() else {
        panic!("{ROW}: not two mutations: {:?}", batch.mutations());
    };
    let Some(created) = first.created_id() else {
        panic!("{ROW}: the first set creates none: {first:?}");
    };
    assert_eq!(
        (second.created_ids(), second.changed_ids()),
        (&[] as &[ElementId], std::slice::from_ref(created)),
        "{ROW}: the second set does not change what the first created"
    );
    assert_eq!(
        (batch.created_ids(), batch.changed_ids()),
        (std::slice::from_ref(created), &[] as &[ElementId]),
        "{ROW}: the net ids of the batch"
    );
    let c1 = element(&doc, "c1");
    assert_eq!(c1.tagged_values().count(), 1, "{ROW}: one tagged value");
    assert_eq!(
        c1.tagged_value("K").and_then(SacmElement::content),
        Some(&MultiLangString::from("two")),
        "{ROW}: the content after the batch"
    );
}

#[cfg(feature = "serde")]
mod serde_json_shape {
    use std::fmt::Write as _;

    use super::{Operation, ROW, SetTaggedValue};

    /// The sample of `operations.json`, alone and pretty.
    const PRETTY: &str = "{
  \"type\": \"SetTaggedValue\",
  \"element\": \"c1\",
  \"key\": \"ASIL\",
  \"content\": [
    {
      \"lang\": null,
      \"content\": \"D\"
    }
  ]
}";

    /// The sample, compact.
    const COMPACT: &str = r#"{"type":"SetTaggedValue","element":"c1","key":"ASIL","content":[{"lang":null,"content":"D"}]}"#;

    /// The golden operations, as text.
    fn operations_json() -> String {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/data/golden/serde/operations.json"
        );
        std::fs::read_to_string(path)
            .unwrap_or_else(|error| panic!("{ROW}: cannot read {path}: {error}"))
    }

    fn refused(what: &str, json: &str) {
        assert_ne!(json, COMPACT, "{ROW}: setup: {what}: nothing changed");
        match serde_json::from_str::<Operation>(json) {
            Ok(value) => panic!("{ROW}: {what}: {json} was read as {value:?}"),
            Err(error) => assert!(
                error.is_data(),
                "{ROW}: {what}: refused as malformed JSON, not as data: {error}"
            ),
        }
    }

    #[test]
    fn lib_edit_023_json_matches_the_golden_sample() {
        let operation = Operation::from(SetTaggedValue::new("c1", "ASIL", "D"));
        let pretty = serde_json::to_string_pretty(&operation)
            .unwrap_or_else(|error| panic!("{ROW}: the operation does not serialize: {error}"));
        assert_eq!(pretty, PRETTY, "{ROW}: the pretty JSON");
        let indented = PRETTY.lines().fold(String::new(), |mut out, line| {
            let _ = writeln!(out, "  {line}");
            out
        });
        assert!(
            operations_json().contains(indented.trim_end()),
            "{ROW}: operations.json does not hold the sample"
        );
        let compact = serde_json::to_string(&operation)
            .unwrap_or_else(|error| panic!("{ROW}: the operation does not serialize: {error}"));
        assert_eq!(compact, COMPACT, "{ROW}: the compact JSON");
        let read: Operation = serde_json::from_str(&pretty)
            .unwrap_or_else(|error| panic!("{ROW}: {pretty} does not read back: {error}"));
        assert_eq!(read, operation, "{ROW}: the JSON does not read back equal");
        // The bare struct is the operation without its tag.
        let bare = serde_json::to_string(&SetTaggedValue::new("c1", "ASIL", "D"))
            .unwrap_or_else(|error| panic!("{ROW}: the struct does not serialize: {error}"));
        assert_eq!(
            bare,
            COMPACT.replacen(r#""type":"SetTaggedValue","#, "", 1),
            "{ROW}: the bare struct"
        );
        let read: SetTaggedValue = serde_json::from_str(&bare)
            .unwrap_or_else(|error| panic!("{ROW}: {bare} does not read back: {error}"));
        assert_eq!(
            read,
            SetTaggedValue::new("c1", "ASIL", "D"),
            "{ROW}: the bare struct does not read back equal"
        );

        refused(
            "an extra id",
            &COMPACT.replacen(r#""element":"c1","#, r#""element":"c1","id":null,"#, 1),
        );
        refused(
            "a missing key",
            &COMPACT.replacen(r#""key":"ASIL","#, "", 1),
        );
        refused(
            "a key given as a MultiLangString",
            &COMPACT.replacen(r#""key":"ASIL""#, r#""key":[{"content":"K"}]"#, 1),
        );
        refused(
            "a misspelt type",
            &COMPACT.replacen("SetTaggedValue", "SetTaggedvalue", 1),
        );
        refused(
            "a missing element",
            &COMPACT.replacen(r#""element":"c1","#, "", 1),
        );
        refused(
            "a missing content",
            &COMPACT.replacen(r#","content":[{"lang":null,"content":"D"}]"#, "", 1),
        );
    }
}
