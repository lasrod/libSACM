//! Tests for moving an element (LIB-EDIT-017; ADR 0036 D1).
//!
//! `edit::MoveElement` moves an element, with everything it contains and its
//! preserved content, under another parent (`new`) or to the roots (`root`),
//! after the last child or root, or at a position (`with_position`): its
//! index among the new parent's children (or the roots) after the move, from
//! 0 to the number of the others.
//!
//! - Ids, references and the element's features do not change; its parent,
//!   its role (as `xmi::write` writes it) and the order do.
//! - The refusals, in order: `ElementNotFound`, `ParentNotFound`,
//!   `InvalidParent` or `InvalidRoot`, `ParentInSubtree`,
//!   `PositionOutOfRange`. A refused move leaves the document `==`, with the
//!   same revision and the same next generated ids.
//! - A preserved child of either parent keeps its place after the typed
//!   child it followed: on removal, a preserved index above the old index
//!   drops by one; on insertion, one above the position rises by one.
//! - `changed_ids` follow LIB-EDIT-003: `Difference::Parent` on the element,
//!   and a parent whose child order or preserved placement changes, in
//!   pre-order after. A move to the place held changes nothing.
//!
//! Every document is read from XMI written here: [`MOVE_START`] and its
//! preserved variants.
//!
//! Elsewhere: moves join LIB-EDIT-002's set in `edit_atomic.rs`, a batch
//! that moves away and back is in `batch.rs`, and the JSON shape in
//! `serde.rs`.

use std::fmt::Debug;

use sacm::compare::{Difference, semantic_compare};
use sacm::diagnostic::{Code, Diagnostic};
use sacm::edit::{
    self, CreateArgumentPackage, CreateArtifactPackage, CreateAssuranceCasePackage, CreateClaim,
    CreateDescription, EditError, MoveElement, Mutation, Operation, ReorderElements,
};
use sacm::model::{Document, ElementId, ElementKind, LangString, SacmElement};
use sacm::validation;
use sacm::xmi::{self, Mode, ReadOptions, WriteOptions};

const ROW: &str = "LIB-EDIT-017";

/// The worked document of slice 0056: the AssuranceCasePackage root `case`
/// holding the ArgumentPackage `p` (the Claims `c1`, `c2` with the
/// `metaClaim` `c1`, and `c3`), the ArgumentPackage `q` (the Claim `d1`) and
/// the ArtifactPackage `e`, empty.
const MOVE_START: &str = r#"<sacm:AssuranceCasePackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301" xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="case">
  <name content="Case"/>
  <argumentPackage xmi:id="p">
    <name content="P"/>
    <argumentElement xmi:type="sacm:Claim" xmi:id="c1"><name content="C1"/></argumentElement>
    <argumentElement xmi:type="sacm:Claim" xmi:id="c2" metaClaim="c1"><name content="C2"/></argumentElement>
    <argumentElement xmi:type="sacm:Claim" xmi:id="c3"><name content="C3"/></argumentElement>
  </argumentPackage>
  <argumentPackage xmi:id="q">
    <name content="Q"/>
    <argumentElement xmi:type="sacm:Claim" xmi:id="d1"><name content="D1"/></argumentElement>
  </argumentPackage>
  <artifactPackage xmi:id="e"><name content="E"/></artifactPackage>
</sacm:AssuranceCasePackage>
"#;

/// [`MOVE_START`] with a preserved `<v:mark>` in `p` between `c1` and `c2`.
const MARK_IN_P: &str = r#"<sacm:AssuranceCasePackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301" xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="case">
  <name content="Case"/>
  <argumentPackage xmi:id="p">
    <name content="P"/>
    <argumentElement xmi:type="sacm:Claim" xmi:id="c1"><name content="C1"/></argumentElement>
    <v:mark xmlns:v="urn:v"/>
    <argumentElement xmi:type="sacm:Claim" xmi:id="c2" metaClaim="c1"><name content="C2"/></argumentElement>
    <argumentElement xmi:type="sacm:Claim" xmi:id="c3"><name content="C3"/></argumentElement>
  </argumentPackage>
  <argumentPackage xmi:id="q">
    <name content="Q"/>
    <argumentElement xmi:type="sacm:Claim" xmi:id="d1"><name content="D1"/></argumentElement>
  </argumentPackage>
  <artifactPackage xmi:id="e"><name content="E"/></artifactPackage>
</sacm:AssuranceCasePackage>
"#;

/// [`MOVE_START`] with a preserved `<v:mark>` in `q` after `d1`.
const MARK_IN_Q: &str = r#"<sacm:AssuranceCasePackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301" xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="case">
  <name content="Case"/>
  <argumentPackage xmi:id="p">
    <name content="P"/>
    <argumentElement xmi:type="sacm:Claim" xmi:id="c1"><name content="C1"/></argumentElement>
    <argumentElement xmi:type="sacm:Claim" xmi:id="c2" metaClaim="c1"><name content="C2"/></argumentElement>
    <argumentElement xmi:type="sacm:Claim" xmi:id="c3"><name content="C3"/></argumentElement>
  </argumentPackage>
  <argumentPackage xmi:id="q">
    <name content="Q"/>
    <argumentElement xmi:type="sacm:Claim" xmi:id="d1"><name content="D1"/></argumentElement>
    <v:mark xmlns:v="urn:v"/>
  </argumentPackage>
  <artifactPackage xmi:id="e"><name content="E"/></artifactPackage>
</sacm:AssuranceCasePackage>
"#;

/// A document for what a move keeps. In `p`: a preserved `<v:mark>` before
/// every typed child, so that it stays first; the Claim `c1` with a gid,
/// `isCitation`, the `citedElement` `d1` (out of its subtree), a name with an
/// `xmi:id`, a Description `c1_d` whose `content` and value have ids, and a
/// preserved `<v:extra xmi:id="x1"/>`; the Claim `c2` with the `metaClaim`
/// `c1`; and the Claim `c3` whose `metaClaim`s are the Description `c1_d`
/// (wrongly typed, SACM-REF-002), the preserved id `x1` (SACM-REF-003) and
/// the dangling `ghost` (SACM-REF-001). In `q`: the Claim `d1`.
const KEEP_START: &str = r#"<sacm:AssuranceCasePackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301" xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="case">
  <name content="Case"/>
  <argumentPackage xmi:id="p">
    <name content="P"/>
    <v:mark xmlns:v="urn:v"/>
    <argumentElement xmi:type="sacm:Claim" xmi:id="c1" gid="G-1" isCitation="true" citedElement="d1">
      <name xmi:id="c1_name" lang="en" content="C1"/>
      <description xmi:id="c1_d"><content xmi:id="c1_d_content"><value xmi:id="c1_d_value" lang="en" content="Why"/></content></description>
      <v:extra xmlns:v="urn:v" xmi:id="x1"/>
    </argumentElement>
    <argumentElement xmi:type="sacm:Claim" xmi:id="c2" metaClaim="c1"><name content="C2"/></argumentElement>
    <argumentElement xmi:type="sacm:Claim" xmi:id="c3" metaClaim="c1_d x1 ghost"><name content="C3"/></argumentElement>
  </argumentPackage>
  <argumentPackage xmi:id="q">
    <name content="Q"/>
    <argumentElement xmi:type="sacm:Claim" xmi:id="d1"><name content="D1"/></argumentElement>
  </argumentPackage>
</sacm:AssuranceCasePackage>
"#;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn id(value: &str) -> ElementId {
    ElementId::from(value)
}

fn ids(values: &[&str]) -> Vec<ElementId> {
    values.iter().copied().map(ElementId::from).collect()
}

/// Reads `input` with the default (tolerant) options.
fn read(what: &str, input: &str) -> Document {
    match xmi::read(input.as_bytes(), &ReadOptions::default()) {
        Ok(output) => output.into_document(),
        Err(error) => panic!("{ROW}: setup: {what} does not read: {error}"),
    }
}

fn move_start() -> Document {
    read("MOVE_START", MOVE_START)
}

/// The write of `doc` in `mode`, which must succeed.
fn write_in(what: &str, doc: &Document, mode: Mode) -> String {
    match xmi::write(doc, &WriteOptions::default().with_mode(mode)) {
        Ok(text) => text,
        Err(error) => panic!("{ROW}: {what}: the {mode:?} write was refused: {error}"),
    }
}

/// The tolerant write of `doc`, which keeps preserved content.
fn write(what: &str, doc: &Document) -> String {
    write_in(what, doc, Mode::Tolerant)
}

/// Applies `operation`, which must be accepted, to `doc`.
fn apply(what: &str, doc: &mut Document, operation: impl Into<Operation>) -> Mutation {
    let operation = operation.into();
    match edit::apply(doc, operation.clone()) {
        Ok(mutation) => mutation,
        Err(error) => panic!("{ROW}: {what}: {operation:?} was refused: {error} ({error:?})"),
    }
}

/// The ids of the children of `parent` in `doc`, in order.
fn children(doc: &Document, parent: &str) -> Vec<String> {
    match doc.get(parent) {
        Some(found) => found
            .children()
            .map(|child| child.id().as_str().to_owned())
            .collect(),
        None => panic!("{ROW}: the document has no {parent:?}"),
    }
}

/// The ids of the roots of `doc`, in order.
fn roots(doc: &Document) -> Vec<String> {
    doc.roots()
        .map(|root| root.id().as_str().to_owned())
        .collect()
}

/// The parent id of `element` in `doc`, or `None` for a root.
fn parent_of(doc: &Document, element: &str) -> Option<String> {
    match doc.get(element) {
        Some(found) => found.parent().map(|parent| parent.id().as_str().to_owned()),
        None => panic!("{ROW}: the document has no {element:?}"),
    }
}

/// `element` and everything it contains, in pre-order.
fn subtree_of(element: SacmElement<'_>) -> Vec<ElementId> {
    let mut found = vec![element.id().clone()];
    for child in element.children() {
        found.extend(subtree_of(child));
    }
    found
}

/// The next generated id of an AssuranceCasePackage, an ArgumentPackage, a
/// Claim, a Description and an ArtifactPackage, observed on a clone.
fn next_ids(doc: &Document) -> Vec<ElementId> {
    let mut probe = doc.clone();
    let mut created = |operation: Operation| -> ElementId {
        match edit::apply(&mut probe, operation.clone()) {
            Ok(mutation) => match mutation.created_ids() {
                [only] => only.clone(),
                other => panic!("{ROW}: setup: the probe {operation:?} created {other:?}"),
            },
            Err(error) => panic!("{ROW}: setup: the probe {operation:?} was refused: {error}"),
        }
    };
    let name = || LangString::new("probe");
    let case = created(CreateAssuranceCasePackage::root(name()).into());
    let arguments = created(CreateArgumentPackage::root(name()).into());
    let claim = created(CreateClaim::new(&arguments, name()).into());
    let description = created(CreateDescription::new(&claim, name()).into());
    let artifacts = created(CreateArtifactPackage::root(name()).into());
    vec![case, arguments, claim, description, artifacts]
}

/// Refuses `operation` on `doc` with `apply`, `check` and `preview`, which
/// must agree, and asserts that the document is `==` to before, with the
/// same revision and the same next generated ids. Returns the refusal.
fn refused(what: &str, doc: &Document, operation: impl Into<Operation>) -> EditError {
    let operation = operation.into();
    let mut after = doc.clone();
    let revision = after.revision();
    let error = match edit::apply(&mut after, operation.clone()) {
        Ok(mutation) => panic!("{ROW}: {what}: {operation:?} was accepted: {mutation:?}"),
        Err(error) => error,
    };
    assert!(
        after == *doc,
        "{ROW}: {what}: after the refusal {error:?} the document is not == to before"
    );
    assert!(
        after.revision() == revision,
        "{ROW}: {what}: the refusal {error:?} changed the revision"
    );
    assert_eq!(
        next_ids(&after),
        next_ids(doc),
        "{ROW}: {what}: the refusal {error:?} changed the next generated ids"
    );
    assert_eq!(
        edit::check(doc, &operation),
        Err(error.clone()),
        "{ROW}: {what}: check answers otherwise than apply"
    );
    assert_eq!(
        edit::preview(doc, &operation).map(|preview| preview.mutation().clone()),
        Err(error.clone()),
        "{ROW}: {what}: preview answers otherwise than apply"
    );
    error
}

/// The position of `needle` in `text`, which must hold it.
fn position_of(what: &str, text: &str, needle: &str) -> usize {
    match text.find(needle) {
        Some(found) => found,
        None => panic!("{ROW}: {what}: {needle:?} is not written:\n{text}"),
    }
}

/// `needles` are written in `text` in this order.
fn assert_written_in_order(what: &str, text: &str, needles: &[&str]) {
    let positions: Vec<usize> = needles
        .iter()
        .map(|needle| position_of(what, text, needle))
        .collect();
    assert!(
        positions.windows(2).all(|pair| pair[0] < pair[1]),
        "{ROW}: {what}: {needles:?} are not written in this order:\n{text}"
    );
}

fn id_attribute(value: &str) -> String {
    format!("xmi:id=\"{value}\"")
}

/// Every id `element` holds in its ends, through the named accessors.
fn held_ids(element: SacmElement<'_>) -> Vec<Vec<ElementId>> {
    let one = |held: Option<&ElementId>| held.into_iter().cloned().collect::<Vec<_>>();
    vec![
        one(element.cited_element()),
        one(element.abstract_form()),
        element.meta_claims().to_vec(),
        one(element.reasoning()),
        element.sources().to_vec(),
        one(element.target()),
        one(element.structure()),
        element.referenced_artifact_elements().to_vec(),
        element.group_members().to_vec(),
    ]
}

// ---------------------------------------------------------------------------
// Acceptance 1: between packages and within one
// ---------------------------------------------------------------------------

#[test]
fn lib_edit_017_a_claim_moves_between_packages_and_within_one() {
    let start = move_start();

    // Appended in another ArgumentPackage.
    let mut doc = start.clone();
    let mutation = apply("c1 to q", &mut doc, MoveElement::new("c1", "q"));
    assert_eq!(
        (children(&doc, "p"), children(&doc, "q")),
        (
            vec!["c2".to_owned(), "c3".to_owned()],
            vec!["d1".to_owned(), "c1".to_owned()]
        ),
        "{ROW}: children() of p and q after c1 is appended to q"
    );
    assert_eq!(
        semantic_compare(&start, &doc),
        [Difference::Parent {
            id: id("c1"),
            left: Some(id("p")),
            right: Some(id("q")),
        }],
        "{ROW}: a move between packages is exactly Parent of the Claim"
    );
    assert_eq!(
        mutation.changed_ids(),
        ids(&["c1"]),
        "{ROW}: a move between packages changes the Claim"
    );
    assert!(
        mutation.created_ids().is_empty() && mutation.deleted_ids().is_empty(),
        "{ROW}: a move creates and deletes nothing: {mutation:?}"
    );

    // At position 0 of another ArgumentPackage.
    let mut doc = start.clone();
    let mutation = apply(
        "c1 to q at 0",
        &mut doc,
        MoveElement::new("c1", "q").with_position(0),
    );
    assert_eq!(
        (children(&doc, "p"), children(&doc, "q")),
        (
            vec!["c2".to_owned(), "c3".to_owned()],
            vec!["c1".to_owned(), "d1".to_owned()]
        ),
        "{ROW}: children() of p and q after c1 is moved to q at 0"
    );
    assert_eq!(
        semantic_compare(&start, &doc),
        [Difference::Parent {
            id: id("c1"),
            left: Some(id("p")),
            right: Some(id("q")),
        }],
        "{ROW}: a move to position 0 of another package is exactly Parent of the Claim"
    );
    assert_eq!(mutation.changed_ids(), ids(&["c1"]), "{ROW}: changed_ids");

    // Within one package, from last to first.
    let mut doc = start.clone();
    let mutation = apply(
        "c3 to p at 0",
        &mut doc,
        MoveElement::new("c3", "p").with_position(0),
    );
    assert_eq!(
        children(&doc, "p"),
        ["c3", "c1", "c2"],
        "{ROW}: children() of p after c3 moves from last to first"
    );
    assert_eq!(
        semantic_compare(&start, &doc),
        [Difference::ChildOrder {
            id: id("p"),
            left: ids(&["c1", "c2", "c3"]),
            right: ids(&["c3", "c1", "c2"]),
        }],
        "{ROW}: a move within one package is exactly ChildOrder of the package"
    );
    assert_eq!(
        mutation.changed_ids(),
        ids(&["p"]),
        "{ROW}: a move within one package changes the package"
    );
}

#[test]
fn lib_edit_017_with_position_puts_the_element_at_that_index() {
    let start = move_start();
    // q holds d1, so c1 has one other child there: positions 0 and 1.
    for (position, expected) in [(0, ["c1", "d1"]), (1, ["d1", "c1"])] {
        let mut doc = start.clone();
        apply(
            "c1 to q",
            &mut doc,
            MoveElement::new("c1", "q").with_position(position),
        );
        assert_eq!(
            children(&doc, "q"),
            expected,
            "{ROW}: c1 moved to q at {position}"
        );
    }
    // With c3 moved in first, q holds d1 and c3: positions 0, 1 and 2, and
    // no position is after the last.
    let mut two = start.clone();
    apply("c3 to q", &mut two, MoveElement::new("c3", "q"));
    let cases: [(Option<usize>, [&str; 3]); 4] = [
        (Some(0), ["c1", "d1", "c3"]),
        (Some(1), ["d1", "c1", "c3"]),
        (Some(2), ["d1", "c3", "c1"]),
        (None, ["d1", "c3", "c1"]),
    ];
    for (position, expected) in cases {
        let mut doc = two.clone();
        let operation = match position {
            Some(position) => MoveElement::new("c1", "q").with_position(position),
            None => MoveElement::new("c1", "q"),
        };
        apply("c1 to q", &mut doc, operation);
        assert_eq!(
            children(&doc, "q"),
            expected,
            "{ROW}: c1 moved to q holding d1 and c3 at {position:?}"
        );
    }
    // Within one parent, the position counts the others: c1 among c2, c3.
    for (position, expected) in [
        (0, ["c1", "c2", "c3"]),
        (1, ["c2", "c1", "c3"]),
        (2, ["c2", "c3", "c1"]),
    ] {
        let mut doc = start.clone();
        apply(
            "c1 within p",
            &mut doc,
            MoveElement::new("c1", "p").with_position(position),
        );
        assert_eq!(
            children(&doc, "p"),
            expected,
            "{ROW}: c1 moved within p to {position}"
        );
    }
    // Among the roots: p and q become roots beside case.
    let mut doc = start.clone();
    apply("p to the roots", &mut doc, MoveElement::root("p"));
    assert_eq!(roots(&doc), ["case", "p"], "{ROW}: a root move appends");
    apply(
        "q to the roots at 0",
        &mut doc,
        MoveElement::root("q").with_position(0),
    );
    assert_eq!(roots(&doc), ["q", "case", "p"], "{ROW}: a root move at 0");
    apply(
        "q among the roots at 1",
        &mut doc,
        MoveElement::root("q").with_position(1),
    );
    assert_eq!(
        roots(&doc),
        ["case", "q", "p"],
        "{ROW}: a root moved among the roots to 1"
    );
}

// ---------------------------------------------------------------------------
// Acceptance 2: roles and the roots, written and read back
// ---------------------------------------------------------------------------

/// Writes `doc` strictly, reads it back, and asserts it is `==`. Returns the
/// text.
fn assert_reads_back_equal(what: &str, doc: &Document) -> String {
    let text = write_in(what, doc, Mode::Strict);
    let back = read(what, &text);
    assert!(
        back == *doc,
        "{ROW}: {what}: the write does not read back ==: {:?}\n{text}",
        semantic_compare(doc, &back)
    );
    text
}

#[test]
fn lib_edit_017_a_move_between_package_kinds_writes_the_new_role_and_reads_back_equal() {
    let start = move_start();
    let original = assert_reads_back_equal("MOVE_START", &start);
    let mut doc = start.clone();
    apply(
        "c1 to the ArtifactPackage e",
        &mut doc,
        MoveElement::new("c1", "e"),
    );
    assert_eq!(
        parent_of(&doc, "c1").as_deref(),
        Some("e"),
        "{ROW}: c1's parent"
    );
    let text = assert_reads_back_equal("c1 in e", &doc);
    assert!(
        text.contains(r#"<artifactElement xmi:type="sacm:Claim" xmi:id="c1">"#),
        "{ROW}: c1 in an ArtifactPackage is not written in the artifactElement role:\n{text}"
    );
    assert!(
        !text.contains(r#"<argumentElement xmi:type="sacm:Claim" xmi:id="c1">"#),
        "{ROW}: c1 is still written in the argumentElement role:\n{text}"
    );

    // And back to its place.
    apply(
        "c1 back to p at 0",
        &mut doc,
        MoveElement::new("c1", "p").with_position(0),
    );
    let text = assert_reads_back_equal("c1 back in p", &doc);
    assert!(
        text.contains(r#"<argumentElement xmi:type="sacm:Claim" xmi:id="c1">"#),
        "{ROW}: c1 back in an ArgumentPackage is not written as an argumentElement:\n{text}"
    );
    assert_eq!(
        semantic_compare(&start, &doc),
        [],
        "{ROW}: c1 moved to e and back differs from the start"
    );
    assert_eq!(
        text, original,
        "{ROW}: c1 moved to e and back is not written as the start"
    );
}

#[test]
fn lib_edit_017_a_package_moves_to_the_roots_and_back() {
    let start = move_start();
    let original = assert_reads_back_equal("MOVE_START", &start);

    // An ArgumentPackage from an AssuranceCasePackage into an ArgumentPackage.
    let mut doc = start.clone();
    let mutation = apply("p into q", &mut doc, MoveElement::new("p", "q"));
    assert_eq!(children(&doc, "q"), ["d1", "p"], "{ROW}: q holds p last");
    assert_eq!(children(&doc, "case"), ["q", "e"], "{ROW}: case lost p");
    assert_eq!(mutation.changed_ids(), ids(&["p"]), "{ROW}: changed_ids");
    let text = assert_reads_back_equal("p in q", &doc);
    assert!(
        text.contains(r#"<argumentElement xmi:type="sacm:ArgumentPackage" xmi:id="p">"#),
        "{ROW}: p in an ArgumentPackage is not written as an argumentElement:\n{text}"
    );
    assert_eq!(
        children(&doc, "p"),
        ["c1", "c2", "c3"],
        "{ROW}: p moved with its children"
    );

    // Then to the roots.
    let mutation = apply("p to the roots", &mut doc, MoveElement::root("p"));
    assert_eq!(roots(&doc), ["case", "p"], "{ROW}: p is the last root");
    assert_eq!(parent_of(&doc, "p"), None, "{ROW}: p has no parent");
    assert_eq!(mutation.changed_ids(), ids(&["p"]), "{ROW}: changed_ids");
    let text = assert_reads_back_equal("p as a root", &doc);
    assert!(
        text.contains("<sacm:ArgumentPackage xmi:id=\"p\""),
        "{ROW}: the root p is not written as a sacm:ArgumentPackage element:\n{text}"
    );

    // From the roots back to its place.
    apply(
        "p back into case at 0",
        &mut doc,
        MoveElement::new("p", "case").with_position(0),
    );
    assert_eq!(roots(&doc), ["case"], "{ROW}: p is no root any more");
    let text = assert_reads_back_equal("p back in case", &doc);
    assert!(
        text.contains("<argumentPackage xmi:id=\"p\">"),
        "{ROW}: p back in case is not written in the argumentPackage role:\n{text}"
    );
    assert_eq!(
        semantic_compare(&start, &doc),
        [],
        "{ROW}: p moved away and back differs from the start"
    );
    assert_eq!(
        text, original,
        "{ROW}: p moved back is not written as the start"
    );
}

// ---------------------------------------------------------------------------
// Acceptance 3: refusals, in order
// ---------------------------------------------------------------------------

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one case per check and pair of checks reads best as one list"
)]
fn lib_edit_017_refusals_come_in_order_and_change_nothing() {
    let doc = move_start();

    // 1. The element is in the document.
    for operation in [
        MoveElement::new("nowhere", "q"),
        // Fails 1, 2 and 5: the first.
        MoveElement::new("nowhere", "elsewhere").with_position(9),
        MoveElement::root("nowhere"),
    ] {
        let error = refused("an unknown element", &doc, operation.clone());
        assert!(
            matches!(&error, EditError::ElementNotFound { element, .. } if element.as_str() == "nowhere"),
            "{ROW}: {operation:?} is refused as {error:?}, not ElementNotFound"
        );
        assert_eq!(error.code(), Code::CMD_TARGET_NOT_FOUND, "{ROW}: the code");
    }

    // 2. The parent of `new` is in the document.
    for operation in [
        MoveElement::new("c1", "nowhere"),
        MoveElement::new("c1", "nowhere").with_position(9),
    ] {
        let error = refused("an unknown parent", &doc, operation.clone());
        assert!(
            matches!(&error, EditError::ParentNotFound { parent, .. } if parent.as_str() == "nowhere"),
            "{ROW}: {operation:?} is refused as {error:?}, not ParentNotFound"
        );
        assert_eq!(error.code(), Code::CMD_TARGET_NOT_FOUND, "{ROW}: the code");
    }

    // 3. The parent's class can hold the kind, or a root is a package.
    let invalid_parents = [
        (
            MoveElement::new("c1", "case"),
            "case",
            ElementKind::AssuranceCasePackage,
            ElementKind::Claim,
        ),
        (
            MoveElement::new("c1", "c2"),
            "c2",
            ElementKind::Claim,
            ElementKind::Claim,
        ),
        // Also inside p's subtree (4) and past the end (5): the first.
        (
            MoveElement::new("p", "c1").with_position(9),
            "c1",
            ElementKind::Claim,
            ElementKind::ArgumentPackage,
        ),
        (
            MoveElement::new("e", "p"),
            "p",
            ElementKind::ArgumentPackage,
            ElementKind::ArtifactPackage,
        ),
    ];
    for (operation, parent, parent_kind, kind) in invalid_parents {
        let error = refused("an illegal parent", &doc, operation.clone());
        assert!(
            matches!(
                &error,
                EditError::InvalidParent { parent: p, parent_kind: pk, kind: k, .. }
                    if p.as_str() == parent && *pk == parent_kind && *k == kind
            ),
            "{ROW}: {operation:?} is refused as {error:?}, not InvalidParent of {parent:?}"
        );
        assert_eq!(error.code(), Code::CMD_INVALID_PARENT, "{ROW}: the code");
    }
    for operation in [
        MoveElement::root("c1"),
        // Also past the end of the roots (5): the first.
        MoveElement::root("c1").with_position(9),
    ] {
        let error = refused("a root that is no package", &doc, operation.clone());
        assert!(
            matches!(
                &error,
                EditError::InvalidRoot { element, kind: ElementKind::Claim, .. } if element.as_str() == "c1"
            ),
            "{ROW}: {operation:?} is refused as {error:?}, not InvalidRoot"
        );
        assert_eq!(
            error.code(),
            Code::CMD_INVALID_PARENT,
            "{ROW}: InvalidRoot is SACM-CMD-005"
        );
        assert_eq!(
            error.to_string(),
            "element \"c1\" of kind Claim cannot be a root, which must be a package",
            "{ROW}: the Display of InvalidRoot"
        );
    }

    // 4. The parent is not the element and not in its subtree.
    for (operation, element, parent) in [
        (MoveElement::new("p", "p"), "p", "p"),
        // The ArtifactPackage e may hold an AssuranceCasePackage, but e is
        // inside case.
        (MoveElement::new("case", "e"), "case", "e"),
        // Also past the end (5): the first.
        (MoveElement::new("case", "e").with_position(9), "case", "e"),
    ] {
        let error = refused("a parent in the subtree", &doc, operation.clone());
        assert!(
            matches!(
                &error,
                EditError::ParentInSubtree { element: el, parent: pa, .. }
                    if el.as_str() == element && pa.as_str() == parent
            ),
            "{ROW}: {operation:?} is refused as {error:?}, not ParentInSubtree"
        );
        assert_eq!(
            error.code(),
            Code::CMD_INVALID_PARENT,
            "{ROW}: ParentInSubtree is SACM-CMD-005"
        );
        assert_eq!(
            error.to_string(),
            format!(
                "element {element:?} cannot be moved into {parent:?}, which is the element itself or inside it"
            ),
            "{ROW}: the Display of ParentInSubtree"
        );
    }

    // 5. The position is at most the number of the others.
    let out_of_range = [
        (
            MoveElement::new("c1", "q").with_position(2),
            Some("q"),
            2,
            1,
            "the position 2 is past the end: \"q\" has 1 other child",
        ),
        (
            MoveElement::new("c1", "p").with_position(3),
            Some("p"),
            3,
            2,
            "the position 3 is past the end: \"p\" has 2 other children",
        ),
        (
            MoveElement::new("c1", "e").with_position(1),
            Some("e"),
            1,
            0,
            "the position 1 is past the end: \"e\" has 0 other children",
        ),
        (
            MoveElement::root("p").with_position(2),
            None,
            2,
            1,
            "the position 2 is past the end: the document has 1 other root",
        ),
        (
            MoveElement::root("case").with_position(1),
            None,
            1,
            0,
            "the position 1 is past the end: the document has 0 other roots",
        ),
    ];
    for (operation, parent, position, others, shown) in out_of_range {
        let error = refused("a position past the end", &doc, operation.clone());
        assert!(
            matches!(
                &error,
                EditError::PositionOutOfRange { parent: pa, position: po, children: ch, .. }
                    if pa.as_ref().map(ElementId::as_str) == parent && *po == position && *ch == others
            ),
            "{ROW}: {operation:?} is refused as {error:?}, not PositionOutOfRange"
        );
        assert_eq!(
            error.code(),
            Code::CMD_INVALID_ORDER,
            "{ROW}: PositionOutOfRange is SACM-CMD-009"
        );
        assert_eq!(
            error.to_string(),
            shown,
            "{ROW}: the Display of PositionOutOfRange"
        );
    }

    // The largest position in range is accepted: control.
    for operation in [
        MoveElement::new("c1", "q").with_position(1),
        MoveElement::new("c1", "e").with_position(0),
        MoveElement::root("p").with_position(1),
        MoveElement::root("case").with_position(0),
    ] {
        assert_eq!(
            edit::check(&doc, &Operation::from(operation.clone())),
            Ok(()),
            "{ROW}: setup: {operation:?} is in range and must be accepted"
        );
    }
}

// ---------------------------------------------------------------------------
// Acceptance 4: what a move keeps
// ---------------------------------------------------------------------------

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "each thing a move keeps is one check, in one list"
)]
fn lib_edit_017_a_move_keeps_ids_references_preserved_content_and_findings() {
    let before = read("KEEP_START", KEEP_START);
    let mut after = before.clone();
    let mutation = apply("c1 to q", &mut after, MoveElement::new("c1", "q"));

    // The parent changes, and nothing else.
    assert_eq!(
        semantic_compare(&before, &after),
        [Difference::Parent {
            id: id("c1"),
            left: Some(id("p")),
            right: Some(id("q")),
        }],
        "{ROW}: the move changed more than c1's parent"
    );
    assert_eq!(mutation.changed_ids(), ids(&["c1"]), "{ROW}: changed_ids");

    // Ids: the subtree, the name, the content and the value.
    let (Some(old), Some(new)) = (before.get("c1"), after.get("c1")) else {
        panic!("{ROW}: c1 is gone");
    };
    assert_eq!(subtree_of(new), subtree_of(old), "{ROW}: c1's subtree");
    assert_eq!(
        subtree_of(new),
        ids(&["c1", "c1_d"]),
        "{ROW}: setup: c1's subtree"
    );
    assert_eq!(
        new.name_id(),
        Some(&id("c1_name")),
        "{ROW}: the name's xmi:id"
    );
    let (Some(old_d), Some(new_d)) = (before.get("c1_d"), after.get("c1_d")) else {
        panic!("{ROW}: c1_d is gone");
    };
    assert_eq!(
        (new_d.content_id(), new_d.content_value_ids()),
        (old_d.content_id(), old_d.content_value_ids()),
        "{ROW}: the content's and value's xmi:ids"
    );
    assert_eq!(
        (new.gid(), new.is_citation(), new.name()),
        (old.gid(), old.is_citation(), old.name()),
        "{ROW}: c1's gid, isCitation and name"
    );

    // Every reference, into and out of the moved subtree, is unchanged.
    for element in before.elements() {
        let Some(now) = after.get(element.id()) else {
            panic!("{ROW}: {:?} is gone", element.id());
        };
        assert_eq!(
            held_ids(now),
            held_ids(element),
            "{ROW}: the references of {:?} changed",
            element.id()
        );
    }

    // Preserved content: c1's moves with it; p's stays with p.
    assert!(
        new.has_preserved_content(),
        "{ROW}: c1 lost its preserved content"
    );
    assert!(
        after
            .get("p")
            .is_some_and(SacmElement::has_preserved_content),
        "{ROW}: p lost its preserved content"
    );
    let text = write("after the move", &after);
    assert_written_in_order(
        "c1's preserved content inside c1 in q",
        &text,
        &[
            &id_attribute("q"),
            &id_attribute("d1"),
            &id_attribute("c1"),
            "<v:extra",
        ],
    );
    assert_written_in_order(
        "p's preserved content in p",
        &text,
        &[
            &id_attribute("p"),
            "<v:mark",
            &id_attribute("c2"),
            &id_attribute("q"),
        ],
    );
    let back = read("the tolerant write after the move", &text);
    assert!(
        back == after,
        "{ROW}: the moved document does not read back =="
    );

    // The next generated ids and the findings.
    assert_eq!(
        next_ids(&after),
        next_ids(&before),
        "{ROW}: the next generated ids"
    );
    let findings = validation::validate(&before);
    let codes: Vec<Code> = findings.iter().map(Diagnostic::code).collect();
    for code in [
        Code::REF_WRONG_TYPE,
        Code::REF_PRESERVED_TARGET,
        Code::REF_DANGLING,
    ] {
        assert!(
            codes.contains(&code),
            "{ROW}: setup: KEEP_START has no {code} finding: {findings:?}"
        );
    }
    assert_eq!(
        validation::validate(&after),
        findings,
        "{ROW}: validate gives other findings after the move"
    );
}

// ---------------------------------------------------------------------------
// Acceptance 5: preserved children and the place held
// ---------------------------------------------------------------------------

/// Moves with `operation` in the document `input`, and asserts that the
/// tolerant write holds the children of `parent` in the order `written`,
/// that `apply`, `preview` and `semantic_compare` give `changed`, and that
/// the result reads back `==`.
fn assert_preserved_case(
    what: &str,
    input: &str,
    operation: MoveElement,
    parent: &str,
    written: &[&str],
    changed: &[&str],
) {
    let before = read(what, input);
    let operation = Operation::from(operation);
    let preview = match edit::preview(&before, &operation) {
        Ok(preview) => preview,
        Err(error) => panic!("{ROW}: {what}: the preview was refused: {error}"),
    };
    let mut after = before.clone();
    let mutation = apply(what, &mut after, operation);
    assert_eq!(
        mutation.changed_ids(),
        ids(changed),
        "{ROW}: {what}: changed_ids"
    );
    assert_eq!(
        &mutation,
        preview.mutation(),
        "{ROW}: {what}: preview and apply agree"
    );
    let mut compared: Vec<ElementId> = semantic_compare(&before, &after)
        .iter()
        .filter_map(Difference::element_id)
        .cloned()
        .collect();
    compared.dedup();
    let in_pre_order: Vec<ElementId> = after
        .elements()
        .map(|element| element.id().clone())
        .filter(|element| compared.contains(element))
        .collect();
    assert_eq!(
        in_pre_order,
        ids(changed),
        "{ROW}: {what}: semantic_compare reports other elements"
    );
    let text = write(what, &after);
    let start = position_of(what, &text, &id_attribute(parent));
    let needles: Vec<String> = written
        .iter()
        .map(|needle| {
            if needle.starts_with('<') {
                (*needle).to_owned()
            } else {
                id_attribute(needle)
            }
        })
        .collect();
    assert_written_in_order(
        what,
        &text[start..],
        &needles.iter().map(String::as_str).collect::<Vec<_>>(),
    );
    let back = read(what, &text);
    assert!(
        back == after,
        "{ROW}: {what}: the write does not read back ==\n{text}"
    );
}

#[test]
fn lib_edit_017_preserved_children_keep_their_place_after_the_typed_child_they_followed() {
    // The mark followed c1 in p. c1 leaves, so the mark is first in p, and
    // p's preserved placement changes.
    assert_preserved_case(
        "c1 leaves p, whose mark followed it",
        MARK_IN_P,
        MoveElement::new("c1", "q"),
        "p",
        &["<v:mark", "c2", "c3", "q"],
        &["p", "c1"],
    );
    // c1 moves within p to the end: the mark still has no typed child
    // before it.
    assert_preserved_case(
        "c1 moves within p to the end",
        MARK_IN_P,
        MoveElement::new("c1", "p").with_position(2),
        "p",
        &["<v:mark", "c2", "c3", "c1", "q"],
        &["p"],
    );
    // The mark follows d1 in q. c1 goes before d1, so the mark still follows
    // d1, and q's preserved placement changes.
    assert_preserved_case(
        "c1 enters q before d1, which the mark follows",
        MARK_IN_Q,
        MoveElement::new("c1", "q").with_position(0),
        "q",
        &["c1", "d1", "<v:mark", "e"],
        &["q", "c1"],
    );
    // Appended, c1 goes after everything, the mark included: q's preserved
    // placement does not change.
    assert_preserved_case(
        "c1 is appended to q after the mark",
        MARK_IN_Q,
        MoveElement::new("c1", "q"),
        "q",
        &["d1", "<v:mark", "c1", "e"],
        &["c1"],
    );
}

/// Preserved `<v:mark>`s after typed children: `p` holds the empty nested
/// ArgumentPackage `pn`, the Claim `c1`, a mark and the Claim `c2`; `q` holds
/// the Claim `d1`, a mark and the nested ArgumentPackage `qn`, which holds the
/// Claim `n1` and a mark.
const MARKS_EVERYWHERE: &str = r#"<sacm:AssuranceCasePackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301" xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="case">
  <name content="Case"/>
  <argumentPackage xmi:id="p">
    <name content="P"/>
    <argumentElement xmi:type="sacm:ArgumentPackage" xmi:id="pn"><name content="Pn"/></argumentElement>
    <argumentElement xmi:type="sacm:Claim" xmi:id="c1"><name content="C1"/></argumentElement>
    <v:mark xmlns:v="urn:v"/>
    <argumentElement xmi:type="sacm:Claim" xmi:id="c2"><name content="C2"/></argumentElement>
  </argumentPackage>
  <argumentPackage xmi:id="q">
    <name content="Q"/>
    <argumentElement xmi:type="sacm:Claim" xmi:id="d1"><name content="D1"/></argumentElement>
    <v:mark xmlns:v="urn:v"/>
    <argumentElement xmi:type="sacm:ArgumentPackage" xmi:id="qn">
      <name content="Qn"/>
      <argumentElement xmi:type="sacm:Claim" xmi:id="n1"><name content="N1"/></argumentElement>
      <v:mark xmlns:v="urn:v"/>
    </argumentElement>
  </argumentPackage>
</sacm:AssuranceCasePackage>
"#;

/// `q` holding the nested ArgumentPackage `qn` (the Claim `n1`, then a
/// preserved `<v:mark>`), the Claim `d1`, then a preserved `<v:mark>`.
const NESTED_FIRST: &str = r#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301" xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="q">
  <name content="Q"/>
  <argumentElement xmi:type="sacm:ArgumentPackage" xmi:id="qn">
    <name content="Qn"/>
    <argumentElement xmi:type="sacm:Claim" xmi:id="n1"><name content="N1"/></argumentElement>
    <v:mark xmlns:v="urn:v"/>
  </argumentElement>
  <argumentElement xmi:type="sacm:Claim" xmi:id="d1"><name content="D1"/></argumentElement>
  <v:mark xmlns:v="urn:v"/>
</sacm:ArgumentPackage>
"#;

/// `q` holding the Claim `d1`, then the nested ArgumentPackage `qn` (the
/// Claim `n1`, then a preserved `<v:mark>`).
const NESTED_SECOND: &str = r#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301" xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="q">
  <name content="Q"/>
  <argumentElement xmi:type="sacm:Claim" xmi:id="d1"><name content="D1"/></argumentElement>
  <argumentElement xmi:type="sacm:ArgumentPackage" xmi:id="qn">
    <name content="Qn"/>
    <argumentElement xmi:type="sacm:Claim" xmi:id="n1"><name content="N1"/></argumentElement>
    <v:mark xmlns:v="urn:v"/>
  </argumentElement>
</sacm:ArgumentPackage>
"#;

#[test]
fn lib_edit_017_changed_ids_are_in_pre_order_after_the_move_when_preserved_placement_shifts() {
    // Each case: the move, and the changed ids in pre-order after it, which
    // must also be what semantic_compare reports.
    let cases: [(&str, MoveElement, &[&str]); 10] = [
        // Both parents' marks shift; the old parent comes first.
        (
            MARKS_EVERYWHERE,
            MoveElement::new("c1", "q").with_position(0),
            &["p", "q", "c1"],
        ),
        // Both shift; the new parent comes first, the old one after the
        // element.
        (
            MARKS_EVERYWHERE,
            MoveElement::new("d1", "p").with_position(0),
            &["p", "d1", "q"],
        ),
        // The old parent is inside the new one, before the element.
        (
            MARKS_EVERYWHERE,
            MoveElement::new("n1", "q").with_position(2),
            &["qn", "n1"],
        ),
        // The old parent is inside the new one, after the element.
        (
            MARKS_EVERYWHERE,
            MoveElement::new("n1", "q").with_position(0),
            &["q", "n1", "qn"],
        ),
        // The old parent's branch is at the element's new position: after it.
        // q's mark stands in front of the typed child at 1, so it stays.
        (
            MARKS_EVERYWHERE,
            MoveElement::new("n1", "q").with_position(1),
            &["n1", "qn"],
        ),
        // To the roots, first: the old parent comes after the element.
        (
            MARKS_EVERYWHERE,
            MoveElement::root("pn").with_position(0),
            &["pn", "p"],
        ),
        // To the roots, last: the old parent comes first.
        (MARKS_EVERYWHERE, MoveElement::root("pn"), &["p", "pn"]),
        // Within one parent: only the parent.
        (
            MARKS_EVERYWHERE,
            MoveElement::new("c1", "p").with_position(2),
            &["p"],
        ),
        // The old parent is inside the new one, before the element, and the
        // new parent's mark shifts: the new parent first, then the old one.
        (
            NESTED_FIRST,
            MoveElement::new("n1", "q").with_position(1),
            &["q", "qn", "n1"],
        ),
        // The old parent is inside the new one, at the element's new
        // position, where its own parent is not: after the element.
        (
            NESTED_SECOND,
            MoveElement::new("n1", "q").with_position(1),
            &["n1", "qn"],
        ),
    ];
    for (input, operation, expected) in cases {
        let what = format!("{operation:?}");
        let before = read(&what, input);
        let mut after = before.clone();
        let mutation = apply(&what, &mut after, operation);
        let compared: Vec<ElementId> = semantic_compare(&before, &after)
            .iter()
            .filter_map(Difference::element_id)
            .cloned()
            .collect();
        let in_pre_order: Vec<ElementId> = after
            .elements()
            .map(|element| element.id().clone())
            .filter(|element| compared.contains(element))
            .collect();
        assert_eq!(
            in_pre_order,
            ids(expected),
            "{ROW}: {what}: setup: semantic_compare reports other elements"
        );
        assert_eq!(
            mutation.changed_ids(),
            ids(expected),
            "{ROW}: {what}: changed_ids are not what semantic_compare reports, in pre-order after"
        );
        let back = read(&what, &write(&what, &after));
        assert!(
            back == after,
            "{ROW}: {what}: the move does not read back =="
        );
    }
}

#[test]
fn lib_edit_017_a_move_to_the_place_it_holds_changes_nothing() {
    for input in [MOVE_START, MARK_IN_P, MARK_IN_Q] {
        let start = read("the start", input);
        let text = write("the start", &start);
        for operation in [
            MoveElement::new("c2", "p").with_position(1),
            MoveElement::new("c3", "p"),
            MoveElement::new("c3", "p").with_position(2),
            MoveElement::new("d1", "q"),
            MoveElement::root("case"),
            MoveElement::root("case").with_position(0),
        ] {
            let what = format!("{operation:?}");
            let mut doc = start.clone();
            let mutation = apply(&what, &mut doc, operation);
            assert_eq!(
                mutation.changed_ids(),
                [] as [ElementId; 0],
                "{ROW}: {what}: a move to the place held reports a change"
            );
            assert!(
                mutation.created_ids().is_empty() && mutation.deleted_ids().is_empty(),
                "{ROW}: {what}: a move to the place held creates or deletes"
            );
            assert!(doc == start, "{ROW}: {what}: the document is not ==");
            assert_eq!(
                semantic_compare(&start, &doc),
                [],
                "{ROW}: {what}: the comparison reports a difference"
            );
            assert_eq!(
                write(&what, &doc),
                text,
                "{ROW}: {what}: the written text changed"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// The value type and its name
// ---------------------------------------------------------------------------

fn assert_value_type<T: Send + Sync + Clone + Debug + PartialEq + Eq + Into<Operation>>() {}

#[test]
fn lib_edit_017_move_element_is_send_sync_clone_debug_eq_and_converts_into_operation() {
    assert_value_type::<MoveElement>();
    let appended = MoveElement::new("c1", "q");
    assert_eq!(appended.element(), &id("c1"), "{ROW}: element()");
    assert_eq!(appended.parent(), Some(&id("q")), "{ROW}: parent() of new");
    assert_eq!(appended.position(), None, "{ROW}: position() of new");
    let placed = appended.clone().with_position(0);
    assert_eq!(
        placed.position(),
        Some(0),
        "{ROW}: position() of with_position"
    );
    assert_eq!(
        placed.parent(),
        Some(&id("q")),
        "{ROW}: with_position keeps the parent"
    );
    let root = MoveElement::root("p");
    assert_eq!(
        (root.element(), root.parent(), root.position()),
        (&id("p"), None, None),
        "{ROW}: the accessors of root"
    );
    assert_eq!(
        MoveElement::root("p").with_position(1).position(),
        Some(1),
        "{ROW}: position() of a root move"
    );
    assert_ne!(appended, placed, "{ROW}: the position is part of equality");
    assert_ne!(
        MoveElement::new("p", "case"),
        MoveElement::root("p"),
        "{ROW}: the parent is part of equality"
    );
    assert_eq!(appended.clone(), appended, "{ROW}: a clone is equal");
    let shown = format!("{appended:?}");
    assert!(
        shown.contains("MoveElement") && shown.contains("c1") && shown.contains('q'),
        "{ROW}: the Debug of a move: {shown}"
    );
    let operation = Operation::from(placed.clone());
    assert!(
        matches!(&operation, Operation::MoveElement(held) if *held == placed),
        "{ROW}: a MoveElement converts into Operation::MoveElement: {operation:?}"
    );
    // `apply` takes the struct itself.
    let mut doc = move_start();
    apply("the struct", &mut doc, placed);
    assert_eq!(
        children(&doc, "q"),
        ["c1", "d1"],
        "{ROW}: the struct applied"
    );
}

#[test]
fn lib_edit_017_operation_name_is_move_element() {
    for operation in [
        MoveElement::new("c1", "q"),
        MoveElement::new("c1", "q").with_position(0),
        MoveElement::root("p"),
    ] {
        assert_eq!(
            Operation::from(operation.clone()).operation_name(),
            "MoveElement",
            "{ROW}: operation_name() of {operation:?}"
        );
    }
    let mut doc = move_start();
    let mutation = apply("c1 to q", &mut doc, MoveElement::new("c1", "q"));
    assert_eq!(
        mutation.operation().operation_name(),
        "MoveElement",
        "{ROW}: the name of the move a Mutation holds"
    );
    // Control: another operation keeps its own name.
    assert_eq!(
        Operation::from(ReorderElements::new("p", ["c2"])).operation_name(),
        "ReorderElements",
        "{ROW}: setup: ReorderElements' name"
    );
}
