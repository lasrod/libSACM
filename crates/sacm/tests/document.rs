//! Tests for the document model: `Document`, `ElementId` and the read-only
//! `SacmElement` views (LIB-API-002, ADR 0010).
//!
//! Documents are built through `Document::insert`, the `test-support` seam. The
//! crate dev-depends on itself with that feature, so these tests always see it.
//!
//! Acceptance item 10 (without `test-support`, no public item exposes storage or
//! mutation) is not checked here. These tests are always compiled with the
//! feature on, and a test cannot prove that a field, an index or a mutating
//! method is absent. The verifier checks it in the rustdoc item list built
//! without the feature.

use std::borrow::Borrow;
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeSet, HashSet};
use std::fmt::Debug;
use std::hash::{Hash, Hasher};

use sacm::model::{Document, ElementId, ElementKind, InsertError, SacmElement};

/// The sample document, as `(parent, kind, id)` in insertion order.
///
/// Children are inserted interleaved across parents, and no sibling list is in
/// alphabetical order. So neither insertion order over the whole document nor
/// id order is the pre-order:
///
/// ```text
/// safety_case          AssuranceCasePackage   (root)
///   artifacts          ArtifactPackage
///     test_report      Artifact
///   arguments          ArgumentPackage
///     claim_top        Claim
///     reasoning        ArgumentReasoning
///     claim_sub        Claim
///   terms              TerminologyPackage
/// evidence_library     ArtifactPackage        (root)
/// ```
const SAMPLE: [(Option<&str>, ElementKind, &str); 9] = [
    (None, ElementKind::AssuranceCasePackage, "safety_case"),
    (
        Some("safety_case"),
        ElementKind::ArtifactPackage,
        "artifacts",
    ),
    (
        Some("safety_case"),
        ElementKind::ArgumentPackage,
        "arguments",
    ),
    (Some("arguments"), ElementKind::Claim, "claim_top"),
    (Some("artifacts"), ElementKind::Artifact, "test_report"),
    (
        Some("arguments"),
        ElementKind::ArgumentReasoning,
        "reasoning",
    ),
    (Some("arguments"), ElementKind::Claim, "claim_sub"),
    (
        Some("safety_case"),
        ElementKind::TerminologyPackage,
        "terms",
    ),
    (None, ElementKind::ArtifactPackage, "evidence_library"),
];

/// The sample document's elements in pre-order document order.
const PRE_ORDER: [&str; 9] = [
    "safety_case",
    "artifacts",
    "test_report",
    "arguments",
    "claim_top",
    "reasoning",
    "claim_sub",
    "terms",
    "evidence_library",
];

/// Inserts an element, failing the test if the seam refuses it.
fn add(doc: &mut Document, parent: Option<&str>, kind: ElementKind, id: &str) {
    let parent = parent.map(ElementId::from);
    assert!(
        doc.insert(parent.as_ref(), kind, ElementId::from(id), None)
            .is_ok(),
        "LIB-API-002: setup could not insert {id:?}"
    );
}

/// Attempts an insert and reports whether it was refused.
fn refused(doc: &mut Document, parent: Option<&str>, kind: ElementKind, id: &str) -> bool {
    let parent = parent.map(ElementId::from);
    doc.insert(parent.as_ref(), kind, ElementId::from(id), None)
        .is_err()
}

fn sample() -> Document {
    let mut doc = Document::new();
    for (parent, kind, id) in SAMPLE {
        add(&mut doc, parent, kind, id);
    }
    doc
}

/// Looks up an element that the test expects to exist.
fn view<'d>(doc: &'d Document, id: &str) -> SacmElement<'d> {
    let Some(element) = doc.get(id) else {
        panic!("LIB-API-002: {id:?} is missing from the document");
    };
    element
}

fn ids<'a>(views: impl Iterator<Item = SacmElement<'a>>) -> Vec<String> {
    views.map(|element| element.id().to_string()).collect()
}

/// Everything navigation can observe about a document.
#[derive(Debug, PartialEq)]
struct Snapshot {
    element_count: usize,
    roots: Vec<String>,
    elements: Vec<Node>,
}

#[derive(Debug, PartialEq)]
struct Node {
    id: String,
    kind: ElementKind,
    parent: Option<String>,
    children: Vec<String>,
}

fn snapshot(doc: &Document) -> Snapshot {
    Snapshot {
        element_count: doc.element_count(),
        roots: ids(doc.roots()),
        elements: doc
            .elements()
            .map(|element| Node {
                id: element.id().to_string(),
                kind: element.kind(),
                parent: element.parent().map(|parent| parent.id().to_string()),
                children: ids(element.children()),
            })
            .collect(),
    }
}

// --- 1. An empty document -------------------------------------------------

#[test]
fn lib_api_002_new_and_default_documents_are_empty() {
    for (how, doc) in [("new", Document::new()), ("default", Document::default())] {
        assert_eq!(
            doc.roots().count(),
            0,
            "LIB-API-002: Document::{how}() has roots"
        );
        assert_eq!(
            doc.element_count(),
            0,
            "LIB-API-002: Document::{how}() counts elements"
        );
        assert_eq!(
            doc.elements().count(),
            0,
            "LIB-API-002: Document::{how}() yields elements"
        );
    }
}

// --- 2. Look-up by id -----------------------------------------------------

#[test]
fn lib_api_002_get_returns_the_inserted_id_and_kind() {
    let doc = sample();
    for (_, kind, id) in SAMPLE {
        let element = view(&doc, id);
        assert_eq!(
            element.id().as_str(),
            id,
            "LIB-API-002: get({id:?}) returned another element"
        );
        assert_eq!(
            element.kind(),
            kind,
            "LIB-API-002: get({id:?}) has the wrong kind"
        );
    }
}

/// Ids that are not in the sample but are near one that is. Ids are compared
/// exactly: no case folding, trimming, prefix or extension match, and no
/// dropping of non-ASCII characters.
const NEAR_MISSES: [&str; 8] = [
    "",
    "missing",
    "Claim_top",
    "claim_top ",
    " claim_top",
    "claim",
    "claim_top_",
    "claim_topé",
];

#[test]
fn lib_api_002_get_of_an_unknown_id_is_none() {
    let doc = sample();
    for id in NEAR_MISSES {
        assert!(
            doc.get(id).is_none(),
            "LIB-API-002: get({id:?}) found an element that was never inserted"
        );
    }
}

#[test]
fn lib_api_002_contains_agrees_with_get() {
    let doc = sample();
    for id in PRE_ORDER.into_iter().chain(["", "missing", "SAFETY_CASE"]) {
        assert_eq!(
            doc.contains(id),
            doc.get(id).is_some(),
            "LIB-API-002: contains({id:?}) disagrees with get"
        );
    }
    assert!(
        PRE_ORDER.into_iter().all(|id| doc.contains(id)),
        "LIB-API-002: contains misses an inserted element"
    );
    for id in NEAR_MISSES {
        assert!(
            !doc.contains(id),
            "LIB-API-002: contains({id:?}) is true for an id that was never inserted"
        );
    }
}

#[test]
fn lib_api_002_lookup_accepts_str_string_and_element_id() {
    let doc = sample();
    let string = String::from("claim_top");
    let element_id = ElementId::from("claim_top");

    let claim = Some(ElementKind::Claim);
    assert_eq!(
        doc.get("claim_top").map(SacmElement::kind),
        claim,
        "LIB-API-002: get(&str)"
    );
    assert_eq!(
        doc.get(&string).map(SacmElement::kind),
        claim,
        "LIB-API-002: get(&String)"
    );
    assert_eq!(
        doc.get(&element_id).map(SacmElement::kind),
        claim,
        "LIB-API-002: get(&ElementId)"
    );

    assert!(doc.contains("claim_top"), "LIB-API-002: contains(&str)");
    assert!(doc.contains(&string), "LIB-API-002: contains(&String)");
    assert!(
        doc.contains(&element_id),
        "LIB-API-002: contains(&ElementId)"
    );
}

/// Ids that a trimming, case-folding or ASCII-only document would change.
const EXACT_IDS: [&str; 5] = [
    "UPPER",
    "MixedCase",
    "Ünïcödé-ид-標識",
    "_acp.1",
    "  padded  ",
];

/// The forms a normalising document might store `id` as, other than `id`.
fn normalised_forms(id: &str) -> Vec<String> {
    let forms = [
        id.to_lowercase(),
        id.to_uppercase(),
        id.trim().to_owned(),
        id.chars().filter(char::is_ascii).collect(),
    ];
    forms.into_iter().filter(|form| form != id).collect()
}

#[test]
fn lib_api_002_document_keeps_inserted_ids_exactly() {
    let mut as_roots = Document::new();
    let mut as_children = Document::new();
    add(
        &mut as_children,
        None,
        ElementKind::ArgumentPackage,
        "arguments",
    );
    for id in EXACT_IDS {
        add(&mut as_roots, None, ElementKind::ArgumentPackage, id);
        add(&mut as_children, Some("arguments"), ElementKind::Claim, id);
    }
    // String == &str compares bytes.
    assert_eq!(
        ids(as_roots.roots()),
        EXACT_IDS,
        "LIB-API-002: roots() changed an id"
    );
    assert_eq!(
        ids(as_roots.elements()),
        EXACT_IDS,
        "LIB-API-002: elements() changed a root id"
    );
    assert_eq!(
        ids(view(&as_children, "arguments").children()),
        EXACT_IDS,
        "LIB-API-002: children() changed an id"
    );
    assert_eq!(
        ids(as_children.elements().skip(1)),
        EXACT_IDS,
        "LIB-API-002: elements() changed a child id"
    );

    for (place, doc) in [("root", &as_roots), ("child", &as_children)] {
        for id in EXACT_IDS {
            assert_eq!(
                view(doc, id).id().as_str().as_bytes(),
                id.as_bytes(),
                "LIB-API-002: get({id:?}) as a {place} returned other bytes"
            );
            assert!(
                doc.contains(id),
                "LIB-API-002: contains({id:?}) as a {place} is false"
            );
            for form in normalised_forms(id) {
                assert!(
                    doc.get(&form).is_none(),
                    "LIB-API-002: {id:?} as a {place} is also found as {form:?}"
                );
                assert!(
                    !doc.contains(&form),
                    "LIB-API-002: {id:?} as a {place} is also contained as {form:?}"
                );
            }
        }
    }
}

// --- 3. Roots -------------------------------------------------------------

#[test]
fn lib_api_002_roots_are_in_insertion_order() {
    assert_eq!(
        ids(sample().roots()),
        ["safety_case", "evidence_library"],
        "LIB-API-002: roots() lost the insertion order or included a non-root"
    );
}

#[test]
fn lib_api_002_roots_have_no_parent() {
    let doc = sample();
    for root in doc.roots() {
        assert!(
            root.parent().is_none(),
            "LIB-API-002: root {} reports a parent",
            root.id()
        );
    }
}

// --- 4. Children and parent -----------------------------------------------

#[test]
fn lib_api_002_children_are_in_insertion_order_across_kinds() {
    let doc = sample();
    let case = view(&doc, "safety_case");
    assert_eq!(
        ids(case.children()),
        ["artifacts", "arguments", "terms"],
        "LIB-API-002: children of mixed package kinds are out of order"
    );

    let arguments = view(&doc, "arguments");
    assert_eq!(
        ids(arguments.children()),
        ["claim_top", "reasoning", "claim_sub"],
        "LIB-API-002: children of an ArgumentPackage are out of order"
    );
    let kinds: Vec<ElementKind> = arguments.children().map(SacmElement::kind).collect();
    assert_eq!(
        kinds,
        [
            ElementKind::Claim,
            ElementKind::ArgumentReasoning,
            ElementKind::Claim
        ],
        "LIB-API-002: children report the wrong kinds"
    );
}

#[test]
fn lib_api_002_leaf_has_no_children() {
    let doc = sample();
    for leaf in [
        "test_report",
        "claim_top",
        "reasoning",
        "claim_sub",
        "terms",
        "evidence_library",
    ] {
        assert_eq!(
            view(&doc, leaf).children().count(),
            0,
            "LIB-API-002: {leaf} has children it was never given"
        );
    }
}

#[test]
fn lib_api_002_each_child_has_the_element_as_parent() {
    let doc = sample();
    for element in doc.elements() {
        for child in element.children() {
            let parent = child.parent().map(|parent| parent.id().clone());
            assert_eq!(
                parent.as_ref(),
                Some(element.id()),
                "LIB-API-002: child {} of {} reports another parent",
                child.id(),
                element.id()
            );
        }
    }
}

#[test]
fn lib_api_002_children_filtered_by_kind_keep_their_order() {
    let doc = sample();
    let arguments = view(&doc, "arguments");
    let claims = ids(arguments
        .children()
        .filter(|child| child.kind() == ElementKind::Claim));
    assert_eq!(
        claims,
        ["claim_top", "claim_sub"],
        "LIB-API-002: filtering children by kind changed their order"
    );
}

/// Navigation must not tie its results to the view it started from: a view
/// borrows the document, so a parent, a child or an id can be returned from a
/// function that only borrows the document.
fn first_child<'d>(doc: &'d Document, id: &str) -> Option<SacmElement<'d>> {
    doc.get(id)?.children().next()
}

fn parent_of<'d>(doc: &'d Document, id: &str) -> Option<SacmElement<'d>> {
    doc.get(id)?.parent()
}

fn assert_copy<T: Copy>() {}

#[test]
fn lib_api_002_views_borrow_the_document_not_each_other() {
    assert_copy::<SacmElement<'static>>();
    let doc = sample();
    assert_eq!(
        first_child(&doc, "arguments")
            .map(SacmElement::id)
            .map(ElementId::as_str),
        Some("claim_top"),
        "LIB-API-002: a child view does not outlive the view it came from"
    );
    assert_eq!(
        parent_of(&doc, "claim_top")
            .map(SacmElement::id)
            .map(ElementId::as_str),
        Some("arguments"),
        "LIB-API-002: a parent view does not outlive the view it came from"
    );
}

// --- 5. All elements in pre-order -----------------------------------------

#[test]
fn lib_api_002_elements_are_in_pre_order_document_order() {
    assert_eq!(
        ids(sample().elements()),
        PRE_ORDER,
        "LIB-API-002: elements() is not in pre-order document order"
    );
}

#[test]
fn lib_api_002_elements_visit_each_element_once_and_match_element_count() {
    let doc = sample();
    let visited = ids(doc.elements());
    let unique: HashSet<&String> = visited.iter().collect();
    assert_eq!(
        unique.len(),
        visited.len(),
        "LIB-API-002: elements() visits an element twice: {visited:?}"
    );
    assert_eq!(
        visited.len(),
        doc.element_count(),
        "LIB-API-002: elements() and element_count() disagree"
    );
    assert_eq!(
        doc.element_count(),
        SAMPLE.len(),
        "LIB-API-002: element_count() is not the number of inserted elements"
    );
}

#[test]
fn lib_api_002_elements_handle_deeply_nested_packages() {
    // A file may nest AssuranceCasePackages arbitrarily deep. Traversal,
    // cloning and dropping must not recurse on the nesting depth.
    const DEPTH: usize = 100_000;
    let id_at = |level: usize| format!("acp_{level}");
    let mut doc = Document::new();
    for level in 0..DEPTH {
        let parent = level.checked_sub(1).map(id_at);
        add(
            &mut doc,
            parent.as_deref(),
            ElementKind::AssuranceCasePackage,
            &id_at(level),
        );
    }
    let copy = doc.clone();
    assert_eq!(
        copy.element_count(),
        DEPTH,
        "LIB-API-002: a deep document lost elements"
    );
    assert!(
        copy.elements()
            .map(|element| element.id().to_string())
            .eq((0..DEPTH).map(id_at)),
        "LIB-API-002: elements() of a deep document is not in pre-order"
    );
}

// --- 6. ElementId ---------------------------------------------------------

/// Ids that a normalising implementation would change.
const AWKWARD_IDS: &[&str] = &[
    "",
    "_acp.1",
    "  padded  ",
    "tab\tand\nnewline",
    "Ünïcödé-ид-標識",
    "UPPER",
    "a/b#c?d",
    "0",
];

#[test]
fn lib_api_002_element_id_keeps_its_value_byte_for_byte() {
    for &value in AWKWARD_IDS {
        let constructed = [
            ("new(&str)", ElementId::new(value)),
            ("new(String)", ElementId::new(value.to_owned())),
            ("From<&str>", ElementId::from(value)),
            ("From<String>", ElementId::from(value.to_owned())),
        ];
        for (how, id) in constructed {
            let as_ref: &str = id.as_ref();
            let borrowed: &str = id.borrow();
            assert_eq!(
                id.as_str().as_bytes(),
                value.as_bytes(),
                "LIB-API-002: {how} as_str() changed {value:?}"
            );
            assert_eq!(
                id.to_string(),
                value,
                "LIB-API-002: {how} Display changed {value:?}"
            );
            assert_eq!(
                as_ref, value,
                "LIB-API-002: {how} AsRef<str> changed {value:?}"
            );
            assert_eq!(
                borrowed, value,
                "LIB-API-002: {how} Borrow<str> changed {value:?}"
            );
            assert_eq!(
                id.clone(),
                id,
                "LIB-API-002: {how} clone differs for {value:?}"
            );
        }
    }
}

fn hash_of<T: Hash + ?Sized>(value: &T) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

#[test]
fn lib_api_002_element_id_eq_ord_and_hash_follow_the_string() {
    let values = ["b", "a", "B", "a ", "ä", "aa", ""];
    for x in values {
        for y in values {
            let (left, right) = (ElementId::from(x), ElementId::from(y));
            assert_eq!(left == right, x == y, "LIB-API-002: Eq of {x:?} and {y:?}");
            assert_eq!(
                left.cmp(&right),
                x.cmp(y),
                "LIB-API-002: Ord of {x:?} and {y:?}"
            );
        }
        // Borrow<str> requires the hash of an id to be the hash of its string.
        assert_eq!(
            hash_of(&ElementId::from(x)),
            hash_of(x),
            "LIB-API-002: Hash of {x:?} differs from the string's"
        );
    }
}

#[test]
fn lib_api_002_element_id_collections_are_searchable_by_str() {
    let values = ["b", "a", "B", "ä", "aa"];
    let hashed: HashSet<ElementId> = values.into_iter().map(ElementId::from).collect();
    let ordered: BTreeSet<ElementId> = values.into_iter().map(ElementId::from).collect();
    for value in values {
        assert!(
            hashed.contains(value),
            "LIB-API-002: HashSet<ElementId> misses {value:?}"
        );
        assert!(
            ordered.contains(value),
            "LIB-API-002: BTreeSet<ElementId> misses {value:?}"
        );
    }
    let mut sorted = values;
    sorted.sort_unstable();
    assert!(
        ordered.iter().map(ElementId::as_str).eq(sorted),
        "LIB-API-002: ElementId does not sort as its string"
    );
}

// --- 7. The test-support seam refuses bad inserts -------------------------

#[test]
fn lib_api_002_insert_refuses_a_duplicate_id() {
    let mut doc = sample();
    let before = snapshot(&doc);
    // A duplicate of a root and of a nested element, under the same parent,
    // another parent and no parent, with the same kind and another kind.
    let duplicates = [
        (None, ElementKind::AssuranceCasePackage, "safety_case"),
        (Some("arguments"), ElementKind::Claim, "claim_top"),
        (Some("terms"), ElementKind::Term, "claim_top"),
        (None, ElementKind::ArtifactPackage, "test_report"),
        (
            Some("safety_case"),
            ElementKind::ArgumentPackage,
            "evidence_library",
        ),
    ];
    for (parent, kind, id) in duplicates {
        assert!(
            refused(&mut doc, parent, kind, id),
            "LIB-API-002: insert accepted a second {id:?}"
        );
        assert_eq!(
            snapshot(&doc),
            before,
            "LIB-API-002: a refused duplicate {id:?} changed the document"
        );
    }
    assert_eq!(
        view(&doc, "claim_top").kind(),
        ElementKind::Claim,
        "LIB-API-002: a refused duplicate replaced the original"
    );
}

#[test]
fn lib_api_002_insert_treats_ids_differing_in_case_or_space_as_distinct() {
    let mut doc = sample();
    add(
        &mut doc,
        Some("arguments"),
        ElementKind::ArgumentReasoning,
        "Claim_top",
    );
    add(
        &mut doc,
        Some("arguments"),
        ElementKind::ArtifactReference,
        "claim_top ",
    );
    assert_eq!(
        doc.element_count(),
        SAMPLE.len() + 2,
        "LIB-API-002: ids differing in case or space were not both inserted"
    );
    let expected = [
        ("claim_top", ElementKind::Claim),
        ("Claim_top", ElementKind::ArgumentReasoning),
        ("claim_top ", ElementKind::ArtifactReference),
    ];
    for (id, kind) in expected {
        assert_eq!(
            view(&doc, id).kind(),
            kind,
            "LIB-API-002: {id:?} is found as another element"
        );
    }
}

#[test]
fn lib_api_002_insert_refuses_an_empty_id() {
    let mut empty = Document::new();
    assert!(
        refused(&mut empty, None, ElementKind::AssuranceCasePackage, ""),
        "LIB-API-002: insert accepted an empty root id"
    );
    assert_eq!(
        empty.element_count(),
        0,
        "LIB-API-002: a refused empty id was counted"
    );
    assert_eq!(
        empty.roots().count(),
        0,
        "LIB-API-002: a refused empty id became a root"
    );

    let mut doc = sample();
    let before = snapshot(&doc);
    for parent in [None, Some("arguments")] {
        assert!(
            refused(&mut doc, parent, ElementKind::Claim, ""),
            "LIB-API-002: insert accepted an empty id under {parent:?}"
        );
        assert_eq!(
            snapshot(&doc),
            before,
            "LIB-API-002: a refused empty id changed the document"
        );
    }
    assert!(
        !doc.contains(""),
        "LIB-API-002: a refused empty id is findable"
    );
}

#[test]
fn lib_api_002_insert_refuses_an_unknown_parent() {
    let mut doc = sample();
    let before = snapshot(&doc);
    // "orphan" as its own parent is unknown too: it does not exist yet.
    for parent in [
        "missing",
        "Arguments",
        "arguments ",
        " arguments",
        "argumentsé",
        "",
        "orphan",
    ] {
        assert!(
            refused(&mut doc, Some(parent), ElementKind::Claim, "orphan"),
            "LIB-API-002: insert accepted the unknown parent {parent:?}"
        );
        assert_eq!(
            snapshot(&doc),
            before,
            "LIB-API-002: a refused insert under {parent:?} changed the document"
        );
    }
    assert!(
        !doc.contains("orphan"),
        "LIB-API-002: a refused element is findable"
    );

    let mut empty = Document::new();
    assert!(
        refused(&mut empty, Some("missing"), ElementKind::Claim, "orphan"),
        "LIB-API-002: insert into an empty document accepted an unknown parent"
    );
    assert_eq!(
        empty.element_count(),
        0,
        "LIB-API-002: a refused insert was counted"
    );
}

#[test]
fn lib_api_002_refused_inserts_do_not_reserve_their_id() {
    let mut doc = sample();
    assert!(
        refused(&mut doc, Some("missing"), ElementKind::Claim, "orphan"),
        "LIB-API-002: unknown parent accepted"
    );
    add(&mut doc, Some("arguments"), ElementKind::Claim, "orphan");
    assert_eq!(
        ids(view(&doc, "arguments").children()),
        ["claim_top", "reasoning", "claim_sub", "orphan"],
        "LIB-API-002: a valid insert after a refused one is not the last child"
    );
    assert_eq!(
        doc.element_count(),
        SAMPLE.len() + 1,
        "LIB-API-002: element_count after a refusal"
    );
}

/// Attempts an insert that the test expects to be refused, and returns why.
fn insert_error(
    doc: &mut Document,
    parent: Option<&str>,
    kind: ElementKind,
    id: &str,
) -> InsertError {
    let parent = parent.map(ElementId::from);
    let Err(error) = doc.insert(parent.as_ref(), kind, ElementId::from(id), None) else {
        panic!("LIB-API-002: insert accepted {id:?} under {parent:?}");
    };
    error
}

fn assert_error<T: std::error::Error + Send + Sync + 'static>() {}

#[test]
fn lib_api_002_insert_error_display_names_the_offending_id() {
    assert_error::<InsertError>();
    let mut doc = sample();
    // The wording is not pinned; only that the message says which id is at fault.
    let duplicate = insert_error(&mut doc, Some("arguments"), ElementKind::Claim, "claim_top");
    assert!(
        duplicate.to_string().contains("claim_top"),
        "LIB-API-002: a duplicate-id error does not name the id: {duplicate}"
    );
    let unknown = insert_error(
        &mut doc,
        Some("no_such_parent"),
        ElementKind::Claim,
        "orphan",
    );
    assert!(
        unknown.to_string().contains("no_such_parent"),
        "LIB-API-002: an unknown-parent error does not name the parent: {unknown}"
    );
    let empty = insert_error(&mut doc, None, ElementKind::AssuranceCasePackage, "");
    assert!(
        !empty.to_string().is_empty(),
        "LIB-API-002: an empty-id error has an empty message"
    );
}

// --- 8. Clone -------------------------------------------------------------

#[test]
fn lib_api_002_cloned_document_navigates_like_the_original() {
    let original = sample();
    let copy = original.clone();
    assert_eq!(
        snapshot(&copy),
        snapshot(&original),
        "LIB-API-002: a clone navigates differently from its original"
    );
    for (_, kind, id) in SAMPLE {
        assert_eq!(
            copy.get(id).map(SacmElement::kind),
            Some(kind),
            "LIB-API-002: the clone cannot look up {id:?}"
        );
    }
}

#[test]
fn lib_api_002_cloned_document_is_independent_of_the_original() {
    let original = sample();
    let before = snapshot(&original);
    let mut copy = original.clone();
    add(
        &mut copy,
        Some("arguments"),
        ElementKind::Claim,
        "only_in_copy",
    );
    add(
        &mut copy,
        None,
        ElementKind::AssuranceCasePackage,
        "second_case",
    );

    assert_eq!(
        snapshot(&original),
        before,
        "LIB-API-002: inserting into a clone changed the original"
    );
    assert!(
        !original.contains("only_in_copy"),
        "LIB-API-002: the original sees the clone's element"
    );
    assert!(
        copy.contains("only_in_copy"),
        "LIB-API-002: the clone lost its own element"
    );
}

// --- 9. Send, Sync and Debug ----------------------------------------------

fn assert_send_sync_debug<T: Send + Sync + Debug>() {}

#[test]
fn lib_api_002_document_views_and_ids_are_send_sync_and_debug() {
    assert_send_sync_debug::<Document>();
    assert_send_sync_debug::<SacmElement<'static>>();
    assert_send_sync_debug::<ElementId>();
}

#[test]
fn lib_api_002_view_debug_shows_kind_and_id_not_the_document() {
    let doc = sample();
    let debug = format!("{:?}", view(&doc, "claim_top"));
    assert!(
        debug.contains("Claim"),
        "LIB-API-002: view Debug lacks the kind: {debug}"
    );
    assert!(
        debug.contains("claim_top"),
        "LIB-API-002: view Debug lacks the id: {debug}"
    );
    for unrelated in [
        "test_report",
        "evidence_library",
        "reasoning",
        "claim_sub",
        "terms",
    ] {
        assert!(
            !debug.contains(unrelated),
            "LIB-API-002: view Debug prints the document ({unrelated} found): {debug}"
        );
    }
}

#[test]
fn lib_api_002_document_debug_lists_the_count_and_ids_in_pre_order() {
    let doc = sample();
    let debug = format!("{doc:?}");
    // No id or kind name of the sample contains a digit, so this finds the count.
    assert!(
        debug.contains(&doc.element_count().to_string()),
        "LIB-API-002: Document Debug lacks the element count: {debug}"
    );
    let positions: Option<Vec<usize>> = PRE_ORDER.into_iter().map(|id| debug.find(id)).collect();
    let Some(positions) = positions else {
        panic!("LIB-API-002: Document Debug lacks an element id: {debug}");
    };
    assert!(
        positions.windows(2).all(|pair| pair[0] < pair[1]),
        "LIB-API-002: Document Debug does not list the ids in pre-order: {debug}"
    );
    assert_eq!(
        format!("{:?}", doc.clone()),
        debug,
        "LIB-API-002: a clone's Debug differs from its original's"
    );
}

#[test]
fn lib_api_002_element_id_debug_shows_the_value() {
    let debug = format!("{:?}", ElementId::from("claim_top"));
    assert!(
        debug.contains("claim_top"),
        "LIB-API-002: ElementId Debug lacks its value: {debug}"
    );
}
