//! Tests for `compare::changes` and `compare::Changes` (LIB-RT-005; ADR 0020
//! amendment 1, ADR 0030 amendment 8): the ids created, changed and deleted
//! between two documents, and whether the document itself changed.
//!
//! - Created: only `after` holds the id, in `after`'s pre-order.
//! - Changed: both hold the id and `semantic_compare(before, after)` reports a
//!   difference whose `element_id()` is that id, in `after`'s pre-order. A
//!   move changes the element, a reorder its parent, a change in its subtree
//!   not the element.
//! - Deleted: only `before` holds the id, in `before`'s pre-order. A new id
//!   is a deletion and a creation.
//! - `document_changed()`: `RootOrder`, `DocumentMetadata` or
//!   `DocumentPreservedContent` is reported; `is_empty()` is exactly
//!   `before == after`.
//!
//! The oracle is `semantic_compare` (LIB-RT-001): every valid fixture against
//! itself after one accepted edit of each kind, and consecutive fixtures.
//!
//! Not checked here:
//! - The net ids of random batches: `batch.rs`.
//! - The refusals of `changes.json` with an unknown key: `serde.rs`.

use std::collections::BTreeSet;
use std::fmt::Debug;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use sacm::compare::{Changes, Difference, changes, semantic_compare};
use sacm::edit::{
    self, AddReference, CopyElements, CreateArgumentPackage, CreateClaim, CreateTaggedValue,
    DeleteElement, MoveElement, Mutation, Operation, PackageDeletePolicy, ReferenceDeletePolicy,
    ReorderElements, SetContent, SetName,
};
use sacm::model::{Document, ElementId, LangString, MultiLangString, ReferenceEnd};
use sacm::xmi::{self, Mode, ReadOptions};

const ROW: &str = "LIB-RT-005";

/// The SACM fixtures.
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/sacm23");

/// The namespaces of a hand-written root.
const NS: &str = concat!(
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\"",
    " xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
);

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn id(value: &str) -> ElementId {
    ElementId::from(value)
}

fn ids(values: &[&str]) -> Vec<ElementId> {
    values.iter().copied().map(ElementId::from).collect()
}

fn en(content: &str) -> LangString {
    LangString::new(content).with_lang("en")
}

fn text(content: &str) -> MultiLangString {
    MultiLangString::from(en(content))
}

/// `input` read in `mode`, which must succeed.
fn read_in(what: &str, input: &str, mode: Mode) -> Document {
    match xmi::read(input.as_bytes(), &ReadOptions::default().with_mode(mode)) {
        Ok(output) => output.into_document(),
        Err(error) => panic!("{ROW}: setup: {what} does not read: {error}"),
    }
}

/// `input` read strictly.
fn read(what: &str, input: &str) -> Document {
    read_in(what, input, Mode::Strict)
}

/// Applies `operation`, which must be accepted, to `doc`.
fn apply(doc: &mut Document, operation: impl Into<Operation>) -> Mutation {
    let operation = operation.into();
    match edit::apply(doc, operation.clone()) {
        Ok(mutation) => mutation,
        Err(error) => panic!("{ROW}: setup: {operation:?} was refused: {error}"),
    }
}

/// A clone of `before` with `operations` applied in order, each accepted.
fn edited(before: &Document, operations: Vec<Operation>) -> Document {
    let mut after = before.clone();
    for operation in operations {
        apply(&mut after, operation);
    }
    after
}

/// The one id `mutation` created.
fn only_created(mutation: &Mutation) -> ElementId {
    match mutation.created_ids() {
        [only] => only.clone(),
        other => panic!("{ROW}: setup: {mutation:?} created {other:?}, not one element"),
    }
}

/// `found` lists exactly `created`, `changed` and `deleted`, in order, and
/// says `document_changed`; `is_empty()` agrees.
fn assert_changes(
    what: &str,
    found: &Changes,
    created: &[ElementId],
    changed: &[ElementId],
    deleted: &[ElementId],
    document_changed: bool,
) {
    assert_eq!(found.created_ids(), created, "{ROW}: {what}: created_ids");
    assert_eq!(found.changed_ids(), changed, "{ROW}: {what}: changed_ids");
    assert_eq!(found.deleted_ids(), deleted, "{ROW}: {what}: deleted_ids");
    assert_eq!(
        found.document_changed(),
        document_changed,
        "{ROW}: {what}: document_changed"
    );
    let empty = created.is_empty() && changed.is_empty() && deleted.is_empty() && !document_changed;
    assert_eq!(found.is_empty(), empty, "{ROW}: {what}: is_empty");
}

/// Two root `ArgumentPackage`s, built with `edit::apply`: `p` holding the
/// Claims `c1` to `c4`, and `q`, empty.
fn base() -> Document {
    let mut doc = Document::new();
    apply(
        &mut doc,
        CreateArgumentPackage::root(en("Arguments")).with_id("p"),
    );
    for claim in ["c1", "c2", "c3", "c4"] {
        apply(
            &mut doc,
            CreateClaim::new("p", en(&claim.to_uppercase())).with_id(claim),
        );
    }
    apply(
        &mut doc,
        CreateArgumentPackage::root(en("Other")).with_id("q"),
    );
    doc
}

/// Every file under `dir` whose name holds `-valid` and ends with `.xmi`,
/// sorted.
fn valid_fixtures(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(next) = pending.pop() {
        let entries = std::fs::read_dir(&next)
            .unwrap_or_else(|error| panic!("{ROW}: cannot list {}: {error}", next.display()));
        for entry in entries {
            let entry = entry
                .unwrap_or_else(|error| panic!("{ROW}: cannot list {}: {error}", next.display()));
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if path.is_dir() {
                pending.push(path);
            } else if name.contains("-valid")
                && path.extension().is_some_and(|extension| extension == "xmi")
            {
                found.push(path);
            }
        }
    }
    found.sort();
    assert!(
        !found.is_empty(),
        "{ROW}: setup: no valid fixture under {}",
        dir.display()
    );
    found
}

/// Every valid fixture, read strictly, with its file name.
fn fixture_documents() -> Vec<(String, Document)> {
    valid_fixtures(Path::new(FIXTURES))
        .into_iter()
        .map(|path| {
            let name = path
                .file_name()
                .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
            let bytes = std::fs::read(&path)
                .unwrap_or_else(|error| panic!("{ROW}: cannot read {}: {error}", path.display()));
            match xmi::read(&bytes, &ReadOptions::default()) {
                Ok(output) => (name, output.into_document()),
                Err(error) => panic!("{ROW}: setup: {name} does not read strictly: {error}"),
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// The oracle: semantic_compare(before, after)
// ---------------------------------------------------------------------------

/// What `changes(before, after)` must give, from `semantic_compare`.
#[derive(Debug, PartialEq, Eq)]
struct Expected {
    created: Vec<ElementId>,
    changed: Vec<ElementId>,
    deleted: Vec<ElementId>,
    document_changed: bool,
}

/// The right-only ids in order, the distinct ids of every other per-element
/// difference in `after`'s pre-order, the left-only ids in order, and
/// whether a document-level difference is reported.
fn expected(before: &Document, after: &Document) -> Expected {
    let mut created = Vec::new();
    let mut deleted = Vec::new();
    let mut changed = BTreeSet::new();
    let mut document_changed = false;
    for difference in semantic_compare(before, after) {
        match &difference {
            Difference::RightOnly { id, .. } => created.push(id.clone()),
            Difference::LeftOnly { id, .. } => deleted.push(id.clone()),
            Difference::RootOrder { .. }
            | Difference::DocumentMetadata
            | Difference::DocumentPreservedContent => document_changed = true,
            other => match other.element_id() {
                Some(id) => {
                    changed.insert(id.clone());
                }
                None => {
                    panic!("{ROW}: a document-level difference this test does not know: {other:?}")
                }
            },
        }
    }
    let changed = after
        .elements()
        .map(|element| element.id().clone())
        .filter(|id| changed.contains(id))
        .collect();
    Expected {
        created,
        changed,
        deleted,
        document_changed,
    }
}

/// `changes(before, after)` is what [`expected`] gives, and `is_empty()` is
/// `before == after`.
fn assert_agrees(context: &str, before: &Document, after: &Document) {
    let found = changes(before, after);
    let expected = expected(before, after);
    assert_eq!(
        Expected {
            created: found.created_ids().to_vec(),
            changed: found.changed_ids().to_vec(),
            deleted: found.deleted_ids().to_vec(),
            document_changed: found.document_changed(),
        },
        expected,
        "{ROW}: {context}: changes(before, after) is not the right-only, per-element and \
         left-only ids of semantic_compare, with its document-level differences"
    );
    assert_eq!(
        found.is_empty(),
        before == after,
        "{ROW}: {context}: is_empty() is not before == after"
    );
}

/// The ids of `doc`'s elements, in pre-order.
fn element_ids(doc: &Document) -> Vec<ElementId> {
    doc.elements().map(|element| element.id().clone()).collect()
}

/// The kinds of edit the oracle applies.
const EDIT_KINDS: [&str; 7] = [
    "SetName",
    "DeleteElement",
    "MoveElement",
    "ReorderElements",
    "CopyElements",
    "SetContent",
    "AddReference",
];

/// The first operation of `candidates` that `doc` accepts.
fn first_accepted(
    doc: &Document,
    candidates: impl IntoIterator<Item = Operation>,
) -> Option<Operation> {
    candidates
        .into_iter()
        .find(|operation| edit::check(doc, operation).is_ok())
}

/// Every operation of the [`EDIT_KINDS`] that `doc` accepts on its
/// elements: per element, a rename, a recursive delete that also deletes
/// the relationships that reference it, its first accepted move to another
/// element or to the roots, a reverse of its children, a copy into its own
/// parent, a content, and its first accepted reference; and a reverse of
/// the roots.
fn accepted_edits(doc: &Document) -> Vec<(&'static str, Operation)> {
    let all = element_ids(doc);
    let mut candidates: Vec<(&'static str, Option<Operation>)> = Vec::new();
    for element in doc.elements() {
        let e = element.id();
        let parent = element.parent().map(|parent| parent.id().clone());
        candidates.push(("SetName", Some(SetName::new(e, en("Renamed")).into())));
        candidates.push((
            "DeleteElement",
            Some(
                DeleteElement::new(e)
                    .with_package_policy(PackageDeletePolicy::DeleteRecursively)
                    .with_reference_policy(ReferenceDeletePolicy::DeleteReferencingRelationships)
                    .into(),
            ),
        ));
        let moves = all
            .iter()
            .filter(|target| *target != e && Some(*target) != parent.as_ref())
            .map(|target| Operation::from(MoveElement::new(e, target)))
            .chain(
                parent
                    .is_some()
                    .then(|| Operation::from(MoveElement::root(e))),
            );
        candidates.push(("MoveElement", first_accepted(doc, moves)));
        let mut children: Vec<ElementId> = element.children().map(|c| c.id().clone()).collect();
        if children.len() >= 2 {
            children.reverse();
            candidates.push((
                "ReorderElements",
                Some(ReorderElements::new(e, children).into()),
            ));
        }
        let copy = match &parent {
            Some(parent) => CopyElements::new(doc, [e], parent),
            None => CopyElements::root(doc, [e]),
        };
        candidates.push(("CopyElements", copy.ok().map(Operation::from)));
        candidates.push((
            "SetContent",
            Some(SetContent::new(e, text("Changed")).into()),
        ));
        let references = ReferenceEnd::ALL.iter().flat_map(|&end| {
            all.iter()
                .map(move |target| Operation::from(AddReference::new(e, end, target)))
        });
        candidates.push(("AddReference", first_accepted(doc, references)));
    }
    let mut roots: Vec<ElementId> = doc.roots().map(|root| root.id().clone()).collect();
    if roots.len() >= 2 {
        roots.reverse();
        candidates.push((
            "ReorderElements",
            Some(ReorderElements::roots(roots).into()),
        ));
    }
    candidates
        .into_iter()
        .filter_map(|(kind, operation)| operation.map(|operation| (kind, operation)))
        .filter(|(_, operation)| edit::check(doc, operation).is_ok())
        .collect()
}

/// Every pair the oracle compares, with a label: each valid fixture against
/// itself after each accepted edit of [`accepted_edits`], and each fixture
/// against the next. Fails the test if an edit kind was never accepted.
fn oracle_pairs() -> Vec<(String, Document, Document)> {
    let fixtures = fixture_documents();
    let mut pairs = Vec::new();
    let mut seen: BTreeSet<&'static str> = BTreeSet::new();
    for (name, before) in &fixtures {
        for (kind, operation) in accepted_edits(before) {
            seen.insert(kind);
            let label = format!("{name} after {operation:?}");
            let after = edited(before, vec![operation]);
            pairs.push((label, before.clone(), after));
        }
    }
    for kind in EDIT_KINDS {
        assert!(
            seen.contains(kind),
            "{ROW}: setup: no fixture accepts a {kind}"
        );
    }
    for window in fixtures.windows(2) {
        if let [(left_name, left), (right_name, right)] = window {
            pairs.push((
                format!("{left_name} vs {right_name}"),
                left.clone(),
                right.clone(),
            ));
        }
    }
    pairs
}

// ---------------------------------------------------------------------------
// Equal documents
// ---------------------------------------------------------------------------

#[test]
fn lib_rt_005_changes_of_equal_documents_are_empty() {
    let mut documents = fixture_documents();
    documents.push(("the base document".to_owned(), base()));
    documents.push(("an empty document".to_owned(), Document::new()));
    for (name, doc) in &documents {
        for (what, found) in [
            ("its clone", changes(doc, &doc.clone())),
            ("itself", changes(doc, doc)),
        ] {
            assert_changes(&format!("{name} and {what}"), &found, &[], &[], &[], false);
            assert!(found.is_empty(), "{ROW}: {name} and {what}: is_empty()");
        }
    }
}

// ---------------------------------------------------------------------------
// Created, changed and deleted, in order
// ---------------------------------------------------------------------------

#[test]
fn lib_rt_005_created_changed_and_deleted_follow_the_document_order() {
    let before = base();

    // Created in the order c9, then a TaggedValue of c1: the pre-order of
    // `after` puts the TaggedValue first.
    let mut after = before.clone();
    apply(&mut after, CreateClaim::new("p", en("C9")).with_id("c9"));
    let tagged = only_created(&apply(
        &mut after,
        CreateTaggedValue::new("c1", text("Key"), text("Value")),
    ));
    assert_changes(
        "a Claim, then a TaggedValue of an earlier Claim, created",
        &changes(&before, &after),
        &[tagged, id("c9")],
        &[],
        &[],
        false,
    );

    // Renamed c3, then c1.
    let after = edited(
        &before,
        vec![
            SetName::new("c3", en("C3 renamed")).into(),
            SetName::new("c1", en("C1 renamed")).into(),
        ],
    );
    assert_changes(
        "c3, then c1, renamed",
        &changes(&before, &after),
        &[],
        &ids(&["c1", "c3"]),
        &[],
        false,
    );

    // Renamed c1 and c3, and p's children reversed: `after`'s pre-order,
    // not `before`'s.
    let after = edited(
        &before,
        vec![
            SetName::new("c1", en("C1 renamed")).into(),
            SetName::new("c3", en("C3 renamed")).into(),
            ReorderElements::new("p", ["c4", "c3", "c2", "c1"]).into(),
        ],
    );
    assert_changes(
        "c1 and c3 renamed, and the children of p reversed",
        &changes(&before, &after),
        &[],
        &ids(&["p", "c3", "c1"]),
        &[],
        false,
    );

    // Deleted c4, then c2.
    let after = edited(
        &before,
        vec![
            DeleteElement::new("c4").into(),
            DeleteElement::new("c2").into(),
        ],
    );
    assert_changes(
        "c4, then c2, deleted",
        &changes(&before, &after),
        &[],
        &[],
        &ids(&["c2", "c4"]),
        false,
    );
}

#[test]
fn lib_rt_005_a_move_changes_the_element_and_not_its_parents() {
    let before = base();
    let after = edited(&before, vec![MoveElement::new("c1", "q").into()]);
    assert_changes(
        "c1 moved from p to q",
        &changes(&before, &after),
        &[],
        &ids(&["c1"]),
        &[],
        false,
    );
}

#[test]
fn lib_rt_005_a_reorder_changes_the_parent_and_a_root_reorder_the_document() {
    let before = base();
    let after = edited(
        &before,
        vec![ReorderElements::new("p", ["c3", "c2", "c1"]).into()],
    );
    assert_changes(
        "the children of p reordered",
        &changes(&before, &after),
        &[],
        &ids(&["p"]),
        &[],
        false,
    );

    let after = edited(&before, vec![ReorderElements::roots(["q", "p"]).into()]);
    let found = changes(&before, &after);
    assert_changes("the roots reordered", &found, &[], &[], &[], true);
    assert!(!found.is_empty(), "{ROW}: the roots reordered: is_empty()");

    let after = edited(
        &before,
        vec![CreateArgumentPackage::root(en("Added")).with_id("r").into()],
    );
    assert_changes(
        "a root added",
        &changes(&before, &after),
        &ids(&["r"]),
        &[],
        &[],
        false,
    );
}

/// A root ArgumentPackage `p` holding the Claim `c1`, whose name has the
/// `xmi:id` `name_id` and which carries `vendor` as attributes, with a Note
/// `n1`.
fn claim_with(name_id: &str, vendor: &str) -> String {
    format!(
        "<sacm:ArgumentPackage{NS} xmlns:v=\"urn:vendor\" xmi:id=\"p\"><name content=\"P\"/>\
         <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c1\"{vendor}>\
         <name xmi:id=\"{name_id}\" content=\"C1\"/>\
         <note xmi:id=\"n1\"><content><value content=\"N\"/></content></note>\
         </argumentElement></sacm:ArgumentPackage>"
    )
}

/// [`claim_with`] inside an `xmi:XMI` element with the `xmi:uuid` `uuid`.
fn wrapped(uuid: &str) -> String {
    let root = claim_with("c1_name", "").replacen(NS, "", 1);
    format!("<xmi:XMI{NS} xmi:uuid=\"{uuid}\">{root}</xmi:XMI>")
}

#[test]
fn lib_rt_005_a_change_of_content_or_ids_alone_changes_the_element() {
    let before = read("the Claim with a Note", &claim_with("c1_name", ""));
    let after = edited(&before, vec![SetContent::new("n1", text("Changed")).into()]);
    assert_changes(
        "the content of n1 set",
        &changes(&before, &after),
        &[],
        &ids(&["n1"]),
        &[],
        false,
    );

    let after = read("another name id", &claim_with("c1_label", ""));
    assert_ne!(before, after, "{ROW}: setup: the name ids are compared");
    assert_changes(
        "only the xmi:id of c1's name differs",
        &changes(&before, &after),
        &[],
        &ids(&["c1"]),
        &[],
        false,
    );

    let kept = |value: &str| {
        read_in(
            "a vendor attribute",
            &claim_with("c1_name", &format!(" v:layout=\"{value}\"")),
            Mode::Tolerant,
        )
    };
    assert_changes(
        "only a preserved vendor attribute of c1 differs",
        &changes(&kept("1"), &kept("2")),
        &[],
        &ids(&["c1"]),
        &[],
        false,
    );

    let found = changes(
        &read("xmi:uuid u1", &wrapped("u1")),
        &read("xmi:uuid u2", &wrapped("u2")),
    );
    assert_changes(
        "only the xmi:uuid of xmi:XMI differs",
        &found,
        &[],
        &[],
        &[],
        true,
    );
    assert!(
        !found.is_empty(),
        "{ROW}: only the xmi:uuid differs: is_empty()"
    );

    let wrapper =
        |what: &str, attributes: &str| read_in(what, &vendor_wrapped(attributes), Mode::Tolerant);
    let one = wrapper("v:w=\"1\" on xmi:XMI", " v:w=\"1\"");
    let two = wrapper("v:w=\"2\" on xmi:XMI", " v:w=\"2\"");
    let none = wrapper("no vendor attribute on xmi:XMI", "");
    for (what, left, right) in [
        ("only a vendor attribute of xmi:XMI differs", &one, &two),
        (
            "a vendor attribute of xmi:XMI on the left only",
            &one,
            &none,
        ),
    ] {
        assert_eq!(
            semantic_compare(left, right),
            [Difference::DocumentPreservedContent],
            "{ROW}: setup: {what}: not exactly one DocumentPreservedContent"
        );
        let found = changes(left, right);
        assert_changes(what, &found, &[], &[], &[], true);
        assert!(!found.is_empty(), "{ROW}: {what}: is_empty()");
    }
}

/// [`claim_with`] inside an `xmi:XMI` element that declares the vendor
/// namespace `v` and carries `attributes`.
fn vendor_wrapped(attributes: &str) -> String {
    let root = claim_with("c1_name", "").replacen(NS, "", 1);
    let root = root.replacen(" xmlns:v=\"urn:vendor\"", "", 1);
    format!("<xmi:XMI{NS} xmlns:v=\"urn:vendor\"{attributes}>{root}</xmi:XMI>")
}

/// A root ArgumentPackage `p` holding the element `x` of `kind` and a
/// Claim `c2`.
fn with_x(kind: &str) -> String {
    format!(
        "<sacm:ArgumentPackage{NS} xmi:id=\"p\"><name content=\"P\"/>\
         <argumentElement xmi:type=\"sacm:{kind}\" xmi:id=\"x\"><name content=\"X\"/></argumentElement>\
         <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c2\"><name content=\"C2\"/></argumentElement>\
         </sacm:ArgumentPackage>"
    )
}

#[test]
fn lib_rt_005_an_element_matched_by_id_with_another_kind_is_changed() {
    let before = read("x a Claim", &with_x("Claim"));
    let after = read("x an ArgumentReasoning", &with_x("ArgumentReasoning"));
    assert_changes(
        "x a Claim, then an ArgumentReasoning",
        &changes(&before, &after),
        &[],
        &ids(&["x"]),
        &[],
        false,
    );

    // The kind alone: an ArgumentReasoning and an ArtifactReference with
    // the same name differ in nothing else.
    let before = read("x an ArgumentReasoning", &with_x("ArgumentReasoning"));
    let after = read("x an ArtifactReference", &with_x("ArtifactReference"));
    assert!(
        matches!(
            semantic_compare(&before, &after).as_slice(),
            [Difference::Kind { .. }]
        ),
        "{ROW}: setup: the kind is not the only difference"
    );
    assert_changes(
        "x an ArgumentReasoning, then an ArtifactReference",
        &changes(&before, &after),
        &[],
        &ids(&["x"]),
        &[],
        false,
    );
}

#[test]
fn lib_rt_005_a_new_id_is_a_deletion_and_a_creation() {
    let claim = |claim: &str| {
        format!(
            "<sacm:ArgumentPackage{NS} xmi:id=\"p\"><name content=\"P\"/>\
             <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"{claim}\"><name content=\"C1\"/></argumentElement>\
             </sacm:ArgumentPackage>"
        )
    };
    assert_changes(
        "the Claim c1 written as c1b",
        &changes(&read("c1", &claim("c1")), &read("c1b", &claim("c1b"))),
        &ids(&["c1b"]),
        &[],
        &ids(&["c1"]),
        false,
    );
}

// ---------------------------------------------------------------------------
// The oracle and the swap
// ---------------------------------------------------------------------------

#[test]
fn lib_rt_005_changes_agree_with_semantic_compare() {
    for (context, before, after) in oracle_pairs() {
        assert_agrees(&context, &before, &after);
    }
}

#[test]
fn lib_rt_005_swapping_the_documents_swaps_created_and_deleted() {
    for (context, before, after) in oracle_pairs() {
        let forward = changes(&before, &after);
        let backward = changes(&after, &before);
        assert_eq!(
            backward.created_ids(),
            forward.deleted_ids(),
            "{ROW}: {context}: created after the swap is not deleted before it"
        );
        assert_eq!(
            backward.deleted_ids(),
            forward.created_ids(),
            "{ROW}: {context}: deleted after the swap is not created before it"
        );
        let set = |ids: &[ElementId]| ids.iter().cloned().collect::<BTreeSet<_>>();
        assert_eq!(
            set(backward.changed_ids()),
            set(forward.changed_ids()),
            "{ROW}: {context}: the changed ids differ after the swap"
        );
        assert_eq!(
            backward.document_changed(),
            forward.document_changed(),
            "{ROW}: {context}: document_changed differs after the swap"
        );
    }
}

// ---------------------------------------------------------------------------
// Linear time, no recursion
// ---------------------------------------------------------------------------

/// Runs `job` on its own thread; `None` if it takes longer than `deadline`.
fn within<T: Send + 'static>(
    what: &str,
    deadline: Duration,
    job: impl FnOnce() -> T + Send + 'static,
) -> Option<T> {
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let _ = sender.send(job());
    });
    match receiver.recv_timeout(deadline) {
        Ok(result) => Some(result),
        Err(RecvTimeoutError::Timeout) => None,
        Err(RecvTimeoutError::Disconnected) => panic!("{ROW}: {what}: panicked"),
    }
}

/// One root ArgumentPackage `p` holding the Claims `c0` to `c<count - 1>`:
/// named `C<n>` in order, or, when `changed`, named `D<n>` in reverse order.
fn flat(count: usize, changed: bool) -> Document {
    let mut claims: Vec<String> = (0..count)
        .map(|n| {
            let name = if changed { format!("D{n}") } else { format!("C{n}") };
            format!(
                "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"c{n}\"><name content=\"{name}\"/></argumentElement>"
            )
        })
        .collect();
    if changed {
        claims.reverse();
    }
    read(
        "a flat package",
        &format!(
            "<sacm:ArgumentPackage{NS} xmi:id=\"p\"><name content=\"P\"/>{}</sacm:ArgumentPackage>",
            claims.concat()
        ),
    )
}

/// How long `changes(before, after)` takes.
fn time_changes(before: &Document, after: &Document) -> Duration {
    let start = Instant::now();
    let found = changes(before, after);
    let elapsed = start.elapsed();
    assert!(
        !found.is_empty(),
        "{ROW}: setup: the baseline documents differ"
    );
    elapsed
}

/// A chain of `depth` `ArgumentPackage`s, `p0` the root and each `p<n>` the
/// only child of `p<n - 1>`, built with `edit::apply`.
fn chain(depth: usize) -> Document {
    let mut doc = Document::new();
    apply(&mut doc, CreateArgumentPackage::root(en("P")).with_id("p0"));
    for level in 1..depth {
        apply(
            &mut doc,
            CreateArgumentPackage::new(format!("p{}", level - 1), en("P"))
                .with_id(format!("p{level}")),
        );
    }
    doc
}

#[test]
fn lib_rt_005_twenty_thousand_elements_compare_in_linear_time() {
    // Matching each element of `after` by a scan of `before` is quadratic.
    // The comparison of LARGE claims is timed against a baseline of SMALL
    // claims on this machine, as in content_edits.rs (LIB-EDIT-012): 32
    // times the elements is about 32 times the time when linear and 1,024
    // when quadratic; the limit is RATIO times the baseline, FLOOR keeps a
    // tiny baseline from making it unreasonably short, the baseline is the
    // fastest of BASELINE_RUNS, and the large one has ATTEMPTS tries.
    const SMALL: usize = 625;
    const LARGE: usize = 32 * SMALL;
    const RATIO: u32 = 200;
    const FLOOR: Duration = Duration::from_millis(500);
    const ATTEMPTS: usize = 3;
    const BASELINE_RUNS: usize = 3;
    const DEPTH: usize = 20_000;
    let (small_before, small_after) = (flat(SMALL, false), flat(SMALL, true));
    let (large_before, large_after) = (flat(LARGE, false), flat(LARGE, true));
    let what = format!("changes between two packages of {LARGE} Claims");
    let mut baseline = Duration::MAX;
    let mut limits = Vec::new();
    let mut done = None;
    for _ in 0..ATTEMPTS {
        for _ in 0..BASELINE_RUNS {
            baseline = baseline.min(time_changes(&small_before, &small_after));
        }
        let limit = baseline.saturating_mul(RATIO).max(FLOOR);
        let (before, after) = (large_before.clone(), large_after.clone());
        if let Some(found) = within(&what, limit, move || changes(&before, &after)) {
            done = Some(found);
            break;
        }
        limits.push(limit);
    }
    let Some(found) = done else {
        panic!(
            "{ROW}: {what}: took longer than {RATIO} times the comparison of {SMALL} Claims in \
             each of {ATTEMPTS} tries (limits {limits:?}, the fastest of {SMALL} {baseline:?}): \
             changes is not linear in the number of elements"
        );
    };
    let mut expected = vec![id("p")];
    expected.extend((0..LARGE).rev().map(|n| id(&format!("c{n}"))));
    assert_changes(&what, &found, &[], &expected, &[], false);

    // Deep: a chain 20,000 packages deep, the deepest renamed, compared on
    // this thread, whose stack a recursion of that depth overflows in debug.
    let before = chain(DEPTH);
    let deepest = format!("p{}", DEPTH - 1);
    let after = edited(
        &before,
        vec![SetName::new(deepest.as_str(), en("Deepest")).into()],
    );
    assert_changes(
        "a chain 20,000 deep with the deepest package renamed",
        &changes(&before, &after),
        &[],
        &[id(&deepest)],
        &[],
        false,
    );
    assert!(
        changes(&before, &before.clone()).is_empty(),
        "{ROW}: a chain 20,000 deep and its clone"
    );
}

// ---------------------------------------------------------------------------
// The value type and its JSON
// ---------------------------------------------------------------------------

/// The samples of `changes.json`: root `p` "Arguments" with `c1` to `c3`
/// and root `q` "Evidence", against a clone with `c1` renamed, `c2`
/// deleted, `c4` created in `p` and the roots reordered; then the same
/// document against its clone.
#[cfg(feature = "serde")]
fn golden_samples() -> Vec<Changes> {
    let mut before = Document::new();
    apply(
        &mut before,
        CreateArgumentPackage::root(LangString::new("Arguments")).with_id("p"),
    );
    for (claim, name) in [("c1", "C1"), ("c2", "C2"), ("c3", "C3")] {
        apply(
            &mut before,
            CreateClaim::new("p", LangString::new(name)).with_id(claim),
        );
    }
    apply(
        &mut before,
        CreateArgumentPackage::root(LangString::new("Evidence")).with_id("q"),
    );
    let after = edited(
        &before,
        vec![
            SetName::new("c1", LangString::new("C1 changed")).into(),
            DeleteElement::new("c2").into(),
            CreateClaim::new("p", LangString::new("C4"))
                .with_id("c4")
                .into(),
            ReorderElements::roots(["q", "p"]).into(),
        ],
    );
    vec![changes(&before, &after), changes(&before, &before.clone())]
}

#[cfg(feature = "serde")]
#[test]
fn lib_rt_005_changes_json_matches_the_golden_sample() {
    const GOLDEN: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/data/golden/serde/changes.json"
    );
    let samples = golden_samples();
    assert_changes(
        "the first sample",
        &samples[0],
        &ids(&["c4"]),
        &ids(&["c1"]),
        &ids(&["c2"]),
        true,
    );
    let expected = std::fs::read_to_string(GOLDEN)
        .unwrap_or_else(|error| panic!("{ROW}: cannot read {GOLDEN}: {error}"));
    let written = match serde_json::to_string_pretty(&samples) {
        Ok(json) => json + "\n",
        Err(error) => panic!("{ROW}: the samples do not serialize: {error}"),
    };
    assert_eq!(
        written, expected,
        "{ROW}: serializing the samples does not give the bytes of changes.json"
    );
    let read: Vec<Changes> = match serde_json::from_str(&expected) {
        Ok(read) => read,
        Err(error) => panic!("{ROW}: changes.json does not read back: {error}"),
    };
    assert_eq!(
        read, samples,
        "{ROW}: changes.json reads back to other values"
    );
}

fn assert_value_type<T: Clone + Debug + PartialEq + Eq + Send + Sync>() {}

#[test]
fn lib_rt_005_changes_is_a_send_sync_value_type() {
    assert_value_type::<Changes>();
    let found = changes(&base(), &Document::new());
    let copy = found.clone();
    assert_eq!(copy, found, "{ROW}: a clone of Changes is not == to it");
    assert_ne!(
        copy,
        changes(&base(), &base()),
        "{ROW}: two different Changes are =="
    );
}
