//! Tests for the `name` of every `ModelElement` (SACM23-BASE-003,
//! formal/23-05-08 §8.6, ADR 0012, ADR 0013).
//!
//! The 34 `ModelElement` kinds carry a `name : LangString [1]`. A document can
//! still hold one without a name (`name()` is `None`), and nothing invents one.
//! The four `UtilityElement` kinds have no name, and the `test-support` seam
//! `Document::insert` refuses one for them with `InsertError::NameNotAllowed`.
//!
//! Documents are built through `Document::insert`, the `test-support` seam. The
//! crate dev-depends on itself with that feature, so these tests always see it.

use std::error::Error;

use sacm::model::{
    AbstractClass, Document, ElementId, ElementKind, InsertError, LangString, SacmElement,
};

/// One mebibyte.
const MIB: usize = 1 << 20;

/// The four `UtilityElement` kinds (§8.7), which have no name.
const UTILITY_KINDS: [ElementKind; 4] = [
    ElementKind::Description,
    ElementKind::ImplementationConstraint,
    ElementKind::Note,
    ElementKind::TaggedValue,
];

/// The kinds that carry a name: those that are a `ModelElement`.
fn model_element_kinds() -> Vec<ElementKind> {
    ElementKind::ALL
        .iter()
        .copied()
        .filter(|kind| kind.is_a(AbstractClass::ModelElement))
        .collect()
}

/// The three states of one field: absent, empty and non-empty.
const STATES: [Option<&str>; 3] = [None, Some(""), Some("x")];

/// The nine absent/empty/non-empty `lang` × `content` names. The first is
/// `LangString::default()`.
fn nine_names() -> Vec<LangString> {
    let mut names = Vec::new();
    for lang in STATES {
        for content in STATES {
            let mut name = LangString::default();
            name.set_lang(lang.map(str::to_owned));
            name.set_content(content.map(str::to_owned));
            names.push(name);
        }
    }
    names
}

/// Inserts an element, failing the test if the seam refuses it.
fn add(
    doc: &mut Document,
    parent: Option<&str>,
    kind: ElementKind,
    id: &str,
    name: Option<LangString>,
) {
    let parent = parent.map(ElementId::from);
    let result = doc.insert(parent.as_ref(), kind, ElementId::from(id), name);
    assert_eq!(
        result,
        Ok(()),
        "SACM23-BASE-003: setup could not insert the {kind} {id:?}"
    );
}

/// The name of the element `id`, failing the test if the document lacks it.
fn name_of<'d>(doc: &'d Document, id: &str) -> Option<&'d LangString> {
    let Some(element) = doc.get(id) else {
        panic!("SACM23-BASE-003: the document has no element {id:?}");
    };
    element.name()
}

/// Every element's id and name, in document order.
fn snapshot(doc: &Document) -> Vec<(String, Option<LangString>)> {
    doc.elements()
        .map(|element| (element.id().to_string(), element.name().cloned()))
        .collect()
}

fn title(content: &str) -> LangString {
    LangString::new(content).with_lang("en")
}

// --- 1. ModelElement kinds keep the name given -------------------------------

#[test]
fn sacm23_base_003_every_model_element_kind_keeps_each_of_the_nine_names() {
    for kind in model_element_kinds() {
        let mut doc = Document::new();
        for (index, name) in nine_names().into_iter().enumerate() {
            let id = format!("{kind}_{index}");
            add(&mut doc, None, kind, &id, Some(name.clone()));
            assert_eq!(
                name_of(&doc, &id),
                Some(&name),
                "SACM23-BASE-003: a {kind} inserted with name {name:?} reads back differently"
            );
        }
    }
}

#[test]
fn sacm23_base_003_a_default_name_is_a_name_not_a_missing_one() {
    for kind in model_element_kinds() {
        let mut doc = Document::new();
        add(&mut doc, None, kind, "named", Some(LangString::default()));
        assert_eq!(
            name_of(&doc, "named"),
            Some(&LangString::default()),
            "SACM23-BASE-003: a {kind} named LangString::default() has no name"
        );
    }
}

// --- 2. A missing name stays missing ---------------------------------------------

#[test]
fn sacm23_base_003_model_element_inserted_without_a_name_has_none() {
    for kind in model_element_kinds() {
        let mut doc = Document::new();
        add(&mut doc, None, kind, "unnamed", None);
        let name = name_of(&doc, "unnamed");
        assert_eq!(
            name, None,
            "SACM23-BASE-003: a {kind} inserted without a name has one: {name:?}"
        );
        assert_ne!(
            name,
            Some(&LangString::default()),
            "SACM23-BASE-003: a missing {kind} name equals LangString::default()"
        );
        assert_ne!(
            name,
            Some(&LangString::new("")),
            "SACM23-BASE-003: a missing {kind} name equals LangString::new(\"\")"
        );
    }
}

#[test]
fn sacm23_base_003_missing_default_and_empty_names_are_three_different_answers() {
    let mut doc = Document::new();
    add(
        &mut doc,
        None,
        ElementKind::ArgumentPackage,
        "arguments",
        None,
    );
    add(
        &mut doc,
        Some("arguments"),
        ElementKind::Claim,
        "missing",
        None,
    );
    add(
        &mut doc,
        Some("arguments"),
        ElementKind::Claim,
        "default",
        Some(LangString::default()),
    );
    add(
        &mut doc,
        Some("arguments"),
        ElementKind::Claim,
        "empty",
        Some(LangString::new("")),
    );
    let names = [
        name_of(&doc, "missing"),
        name_of(&doc, "default"),
        name_of(&doc, "empty"),
    ];
    assert_eq!(
        names,
        [
            None,
            Some(&LangString::default()),
            Some(&LangString::new(""))
        ],
        "SACM23-BASE-003: a missing, a default and an empty name read back wrongly"
    );
}

// --- 3. UtilityElement kinds have no name ------------------------------------------

#[test]
fn sacm23_base_003_utility_element_without_a_name_is_accepted_and_has_none() {
    let mut doc = Document::new();
    add(&mut doc, None, ElementKind::Claim, "claim", None);
    for kind in UTILITY_KINDS {
        let id = format!("{kind}_1");
        add(&mut doc, Some("claim"), kind, &id, None);
        assert_eq!(
            name_of(&doc, &id),
            None,
            "SACM23-BASE-003: a {kind} has a name"
        );
    }
}

#[test]
fn sacm23_base_003_utility_element_refuses_any_name_and_leaves_the_document_unchanged() {
    let mut doc = Document::new();
    add(
        &mut doc,
        None,
        ElementKind::Claim,
        "claim",
        Some(title("C1")),
    );
    add(&mut doc, Some("claim"), ElementKind::Note, "note_0", None);
    let before = snapshot(&doc);

    for kind in UTILITY_KINDS {
        for name in nine_names() {
            for parent in [None, Some("claim")] {
                let id = ElementId::from(format!("{kind}_named"));
                let result = doc.insert(
                    parent.map(ElementId::from).as_ref(),
                    kind,
                    id.clone(),
                    Some(name.clone()),
                );
                assert_eq!(
                    result,
                    Err(InsertError::NameNotAllowed(id.clone(), kind)),
                    "SACM23-BASE-003: a {kind} named {name:?} under {parent:?} was not refused with NameNotAllowed"
                );
                assert_eq!(
                    doc.element_count(),
                    2,
                    "SACM23-BASE-003: a refused named {kind} changed element_count"
                );
                assert!(
                    !doc.contains(&id),
                    "SACM23-BASE-003: a refused named {kind} is in the document"
                );
                assert_eq!(
                    snapshot(&doc),
                    before,
                    "SACM23-BASE-003: a refused named {kind} changed elements()"
                );
            }
        }
    }
}

#[test]
fn sacm23_base_003_earlier_refusals_take_precedence_over_name_not_allowed() {
    let mut doc = Document::new();
    add(&mut doc, None, ElementKind::Claim, "claim", None);
    add(&mut doc, Some("claim"), ElementKind::Note, "note_1", None);
    let before = snapshot(&doc);

    for kind in UTILITY_KINDS {
        let named = Some(LangString::default());
        assert_eq!(
            doc.insert(None, kind, ElementId::from(""), named.clone()),
            Err(InsertError::EmptyId),
            "SACM23-BASE-003: a named {kind} with an empty id is not EmptyId"
        );
        for taken in ["claim", "note_1"] {
            assert_eq!(
                doc.insert(None, kind, ElementId::from(taken), named.clone()),
                Err(InsertError::DuplicateId(ElementId::from(taken))),
                "SACM23-BASE-003: a named {kind} with the taken id {taken:?} is not DuplicateId"
            );
        }
        let nowhere = ElementId::from("nowhere");
        assert_eq!(
            doc.insert(Some(&nowhere), kind, ElementId::from("orphan"), named),
            Err(InsertError::UnknownParent(nowhere.clone())),
            "SACM23-BASE-003: a named {kind} under an unknown parent is not UnknownParent"
        );
    }
    assert_eq!(
        snapshot(&doc),
        before,
        "SACM23-BASE-003: refused named inserts changed the document"
    );
}

// --- 4. The kinds with a name ---------------------------------------------------------

#[test]
fn sacm23_base_003_the_kinds_with_a_name_are_exactly_the_34_model_elements() {
    let with_name = model_element_kinds();
    let without_name: Vec<ElementKind> = ElementKind::ALL
        .iter()
        .copied()
        .filter(|kind| !kind.is_a(AbstractClass::ModelElement))
        .collect();
    assert_eq!(
        with_name.len(),
        34,
        "SACM23-BASE-003: there are not 34 ModelElement kinds: {with_name:?}"
    );
    assert_eq!(
        without_name, UTILITY_KINDS,
        "SACM23-BASE-003: the kinds without a name are not the four UtilityElement kinds"
    );
    assert!(
        without_name
            .iter()
            .all(|kind| kind.is_a(AbstractClass::UtilityElement)),
        "SACM23-BASE-003: a kind without a name is not a UtilityElement"
    );
    assert!(
        with_name.contains(&ElementKind::ArtifactAssetRelationship),
        "SACM23-BASE-003: ArtifactAssetRelationship is not a ModelElement kind"
    );
}

#[test]
fn sacm23_base_003_insert_accepts_a_name_exactly_for_model_element_kinds() {
    for &kind in ElementKind::ALL {
        let has_name = kind.is_a(AbstractClass::ModelElement);
        let mut doc = Document::new();
        let id = ElementId::from("element");
        let result = doc.insert(None, kind, id.clone(), Some(title("Name")));
        let expected = if has_name {
            Ok(())
        } else {
            Err(InsertError::NameNotAllowed(id, kind))
        };
        assert_eq!(
            result, expected,
            "SACM23-BASE-003: insert of a named {kind} is wrong (ModelElement: {has_name})"
        );
        assert_eq!(
            doc.element_count(),
            usize::from(has_name),
            "SACM23-BASE-003: the document after inserting a named {kind} has the wrong count"
        );
    }
}

#[test]
fn sacm23_base_003_artifact_asset_relationship_carries_a_name() {
    let mut doc = Document::new();
    add(
        &mut doc,
        None,
        ElementKind::ArtifactPackage,
        "artifacts",
        None,
    );
    add(
        &mut doc,
        Some("artifacts"),
        ElementKind::ArtifactAssetRelationship,
        "relationship",
        Some(title("Report of test")),
    );
    assert_eq!(
        name_of(&doc, "relationship"),
        Some(&title("Report of test")),
        "SACM23-BASE-003: an ArtifactAssetRelationship lost its name"
    );
}

// --- 5. Names are not identifiers ---------------------------------------------------

#[test]
fn sacm23_base_003_two_elements_with_the_same_name_are_both_kept() {
    let mut doc = Document::new();
    add(
        &mut doc,
        None,
        ElementKind::ArgumentPackage,
        "arguments",
        None,
    );
    for id in ["claim_a", "claim_b"] {
        add(
            &mut doc,
            Some("arguments"),
            ElementKind::Claim,
            id,
            Some(title("Top claim")),
        );
    }
    assert_eq!(
        doc.element_count(),
        3,
        "SACM23-BASE-003: two elements with the same name collapsed"
    );
    for id in ["claim_a", "claim_b"] {
        assert_eq!(
            name_of(&doc, id),
            Some(&title("Top claim")),
            "SACM23-BASE-003: {id} lost the name it shares"
        );
    }
}

#[test]
fn sacm23_base_003_get_and_contains_look_up_by_id_not_by_name() {
    let mut doc = Document::new();
    add(
        &mut doc,
        None,
        ElementKind::ArgumentPackage,
        "arguments",
        Some(title("Top claim")),
    );
    // A claim named after another element's id.
    add(
        &mut doc,
        Some("arguments"),
        ElementKind::Claim,
        "claim",
        Some(LangString::new("arguments")),
    );
    assert!(
        doc.get("Top claim").is_none() && !doc.contains("Top claim"),
        "SACM23-BASE-003: an element was found by its name"
    );
    assert_eq!(
        doc.get("arguments").map(SacmElement::kind),
        Some(ElementKind::ArgumentPackage),
        "SACM23-BASE-003: get found an element by name instead of by id"
    );
    assert_eq!(
        name_of(&doc, "arguments"),
        Some(&title("Top claim")),
        "SACM23-BASE-003: the package's name changed"
    );
}

// --- 6. Inserting and cloning keep names ----------------------------------------------

#[test]
fn sacm23_base_003_inserting_siblings_and_children_leaves_every_name_unchanged() {
    let tree: [(Option<&str>, ElementKind, &str, Option<LangString>); 7] = [
        (
            None,
            ElementKind::AssuranceCasePackage,
            "case",
            Some(title("Case")),
        ),
        (
            Some("case"),
            ElementKind::ArgumentPackage,
            "arguments",
            None,
        ),
        (
            Some("arguments"),
            ElementKind::Claim,
            "claim_top",
            Some(title("Top")),
        ),
        (Some("claim_top"), ElementKind::Note, "note", None),
        (
            Some("arguments"),
            ElementKind::Claim,
            "claim_sub",
            Some(LangString::default()),
        ),
        (
            Some("case"),
            ElementKind::ArtifactPackage,
            "artifacts",
            Some(LangString::new("")),
        ),
        (
            None,
            ElementKind::TerminologyPackage,
            "terms",
            Some(title("Terms")),
        ),
    ];
    let mut doc = Document::new();
    for (count, (parent, kind, id, name)) in tree.iter().cloned().enumerate() {
        add(&mut doc, parent, kind, id, name);
        for (earlier_parent, _, earlier, earlier_name) in &tree[..=count] {
            assert_eq!(
                name_of(&doc, earlier),
                earlier_name.as_ref(),
                "SACM23-BASE-003: after inserting {id:?}, the name of {earlier:?} (under {earlier_parent:?}) changed"
            );
        }
    }
}

#[test]
fn sacm23_base_003_a_cloned_document_returns_the_same_name_for_every_element() {
    let mut doc = Document::new();
    add(
        &mut doc,
        None,
        ElementKind::ArgumentPackage,
        "arguments",
        Some(title("Arguments")),
    );
    add(
        &mut doc,
        Some("arguments"),
        ElementKind::Claim,
        "missing",
        None,
    );
    add(
        &mut doc,
        Some("arguments"),
        ElementKind::Claim,
        "default",
        Some(LangString::default()),
    );
    add(
        &mut doc,
        Some("arguments"),
        ElementKind::TaggedValue,
        "tag",
        None,
    );
    let before = snapshot(&doc);

    let copy = doc.clone();
    assert_eq!(
        snapshot(&copy),
        before,
        "SACM23-BASE-003: a cloned document returns different names"
    );
    add(
        &mut doc,
        Some("arguments"),
        ElementKind::Claim,
        "later",
        Some(title("Later")),
    );
    drop(doc);
    assert_eq!(
        snapshot(&copy),
        before,
        "SACM23-BASE-003: a clone's names changed with its original"
    );
}

// --- 7. The borrow and Debug ----------------------------------------------------------

/// Compiles only if `name()` returns `Option<&'d LangString>` for a view of a
/// document borrowed for `'d`: the view is a local that ends here.
fn name_from_a_dropped_view<'d>(doc: &'d Document, id: &str) -> Option<&'d LangString> {
    let view: SacmElement<'d> = doc.get(id)?;
    view.name()
}

#[test]
fn sacm23_base_003_name_borrows_the_document_not_the_view() {
    let mut doc = Document::new();
    add(
        &mut doc,
        None,
        ElementKind::Claim,
        "claim",
        Some(title("C1")),
    );
    let name = name_from_a_dropped_view(&doc, "claim");
    assert_eq!(
        name,
        Some(&title("C1")),
        "SACM23-BASE-003: the name did not outlive its view"
    );
}

#[test]
fn sacm23_base_003_view_debug_shows_kind_and_id_not_the_name() {
    let mut doc = Document::new();
    let name = LangString::new("NAME-CONTENT-MARKER").with_lang("lang-marker");
    add(&mut doc, None, ElementKind::Claim, "claim_top", Some(name));
    let Some(view) = doc.get("claim_top") else {
        panic!("SACM23-BASE-003: the document has no claim_top");
    };
    let debug = format!("{view:?}");
    assert!(
        debug.contains("Claim") && debug.contains("claim_top"),
        "SACM23-BASE-003: view Debug lacks the kind or the id: {debug}"
    );
    assert!(
        !debug.contains("NAME-CONTENT-MARKER") && !debug.contains("lang-marker"),
        "SACM23-BASE-003: view Debug shows the name: {debug}"
    );
}

// --- 8. Hostile names ------------------------------------------------------------------

#[test]
fn sacm23_base_003_hostile_names_are_kept_byte_for_byte() {
    let big = "a\u{0}\u{e9}\n".repeat(MIB / 5);
    let hostile = [
        LangString::new(big.as_str()).with_lang("en"),
        LangString::new("\u{0}"),
        LangString::new("a\u{0}b").with_lang("\u{0}"),
        LangString::new("line one\nline two"),
        LangString::new("line one\r\nline two"),
        LangString::new("lone\rCR"),
        LangString::new("tab\there"),
        LangString::new(" padded ").with_lang(" en\t"),
        LangString::new("Caf\u{e9}"),
        LangString::new("Cafe\u{301}"),
    ];
    let mut doc = Document::new();
    add(
        &mut doc,
        None,
        ElementKind::ArgumentPackage,
        "arguments",
        None,
    );
    for (index, name) in hostile.iter().enumerate() {
        add(
            &mut doc,
            Some("arguments"),
            ElementKind::Claim,
            &format!("claim_{index}"),
            Some(name.clone()),
        );
    }
    for (index, name) in hostile.iter().enumerate() {
        let id = format!("claim_{index}");
        let Some(kept) = name_of(&doc, &id) else {
            panic!("SACM23-BASE-003: {id} lost its hostile name");
        };
        assert_eq!(
            kept.content().map(str::as_bytes),
            name.content().map(str::as_bytes),
            "SACM23-BASE-003: the content of {id}'s name changed"
        );
        assert_eq!(
            kept.lang().map(str::as_bytes),
            name.lang().map(str::as_bytes),
            "SACM23-BASE-003: the lang of {id}'s name changed"
        );
        let Some(view) = doc.get(&id) else {
            panic!("SACM23-BASE-003: the document has no {id}");
        };
        assert!(
            !format!("{view:?}").is_empty(),
            "SACM23-BASE-003: Debug of {id} is empty"
        );
    }
    assert_ne!(
        name_of(&doc, "claim_8"),
        name_of(&doc, "claim_9"),
        "SACM23-BASE-003: NFC and NFD names compare equal"
    );
    assert!(
        !format!("{doc:?}").is_empty(),
        "SACM23-BASE-003: Debug of a document with hostile names is empty"
    );
}

// --- 9. NameNotAllowed as an error ------------------------------------------------------

/// The refusal of a named element of `kind` with `id`.
fn name_not_allowed(kind: ElementKind, id: &str) -> InsertError {
    let mut doc = Document::new();
    let Err(error) = doc.insert(None, kind, ElementId::from(id), Some(LangString::default()))
    else {
        panic!("SACM23-BASE-003: insert accepted a named {kind} {id:?}");
    };
    error
}

fn assert_error_type<T: Error + Send + Sync + 'static>() {}

#[test]
fn sacm23_base_003_name_not_allowed_display_is_exact() {
    assert_eq!(
        name_not_allowed(ElementKind::Note, "note_1").to_string(),
        "element \"note_1\" of kind Note cannot have a name",
        "SACM23-BASE-003: the NameNotAllowed message is wrong"
    );
}

#[test]
fn sacm23_base_003_name_not_allowed_names_the_id_and_kind_on_one_line() {
    assert_error_type::<InsertError>();
    for kind in UTILITY_KINDS {
        for id in ["note_1", "Note.", "a\nb", "\r\nleading", "a\u{0}b", "ünï"] {
            let error = name_not_allowed(kind, id);
            assert_eq!(
                error,
                InsertError::NameNotAllowed(ElementId::from(id), kind),
                "SACM23-BASE-003: a named {kind} {id:?} is not NameNotAllowed"
            );
            let message = error.to_string();
            assert_eq!(
                message,
                format!("element {id:?} of kind {kind} cannot have a name"),
                "SACM23-BASE-003: the NameNotAllowed message for {kind} {id:?} is wrong"
            );
            assert!(
                !message.contains(['\n', '\r'])
                    && message.starts_with(char::is_lowercase)
                    && !message.ends_with(['.', '!', ':']),
                "SACM23-BASE-003: the NameNotAllowed message breaks the ADR 0011 rules: {message:?}"
            );
            assert!(
                format!("{error:?}").starts_with("NameNotAllowed"),
                "SACM23-BASE-003: Debug of NameNotAllowed does not name it: {error:?}"
            );
            assert!(
                error.source().is_none(),
                "SACM23-BASE-003: NameNotAllowed wraps no cause but has a source"
            );
            let boxed: Box<dyn Error + Send + Sync + 'static> = Box::new(error.clone());
            assert_eq!(
                boxed.downcast_ref::<InsertError>(),
                Some(&error),
                "SACM23-BASE-003: a boxed NameNotAllowed does not downcast back to itself"
            );
        }
    }
}
