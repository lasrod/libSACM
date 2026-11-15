//! Tests for reordering contained elements and roots (LIB-EDIT-010; D-ORDER,
//! D-ATOMIC, D-RT; ADR 0010 D1/D2, ADR 0017 T4, ADR 0029).
//!
//! `ReorderElements::new(parent, order)` reorders the direct children of any
//! element, and `ReorderElements::roots(order)` the document's roots. The
//! order names a subset in its new order; the subset goes back into the
//! positions it occupies now, and every other child keeps its position.
//! Roles, parents, ids and counters do not change. It refuses, in order: an
//! unknown parent (SACM-CMD-001), an empty list (SACM-CMD-009), then id by
//! id, an id that is not a direct child or root (`ChildNotFound`,
//! SACM-CMD-001) and an id named before (`InvalidOrder`, SACM-CMD-009).
//! `changed_ids` is `[parent]` exactly when the order changes, and empty for
//! the roots.
//!
//! The last part of item 5 (the operations join the LIB-EDIT-002 set) is in
//! `edit_atomic.rs`.

use sacm::compare::{Difference, semantic_compare};
use sacm::diagnostic::Code;
use sacm::edit::{
    self, CreateArgumentPackage, CreateAssuranceCasePackage, CreateClaim, EditError, Mutation,
    Operation, ReorderElements,
};
use sacm::model::{Document, ElementId, LangString, SacmElement};
use sacm::validation;
use sacm::xmi::{self, Mode, ReadOptions, WriteOptions};

const ROW: &str = "LIB-EDIT-010";

/// The opening of an `xmi:XMI` wrapper.
const WRAPPER_OPEN: &str = concat!(
    "<xmi:XMI xmlns:xmi=\"http://www.omg.org/spec/XMI/20131001\"",
    " xmlns:sacm=\"http://www.omg.org/spec/SACM/20220301\">",
);

// --- Inputs --------------------------------------------------------------------------

/// A named Claim `id`, holding `content` after its name.
fn claim(id: &str, content: &str) -> String {
    format!(
        "<argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"{id}\"><name content=\"{id}\"/>\
         {content}</argumentElement>"
    )
}

/// Four roots in an `xmi:XMI` wrapper:
/// - the ArgumentPackage `ra`, holding the Claims a (its name with the
///   `xmi:id` `a_name`), b (holding the Description bd, whose `content` is
///   `bd_content`), c and d;
/// - the ArgumentPackage `rb`;
/// - the AssuranceCasePackage `rc`, holding the ArgumentPackage `sub`, which
///   holds the Claim g;
/// - the ArgumentPackage `rd`, holding the Claims c1 and c2, the Description
///   d1 between them and the Note n1 after them.
fn input() -> String {
    format!(
        "{WRAPPER_OPEN}\
         <sacm:ArgumentPackage xmi:id=\"ra\"><name content=\"RA\"/>\
         <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"a\"><name xmi:id=\"a_name\" content=\"A\"/></argumentElement>\
         {}{}{}</sacm:ArgumentPackage>\
         <sacm:ArgumentPackage xmi:id=\"rb\"><name content=\"RB\"/></sacm:ArgumentPackage>\
         <sacm:AssuranceCasePackage xmi:id=\"rc\"><name content=\"RC\"/>\
         <argumentPackage xmi:id=\"sub\"><name content=\"Sub\"/>{}</argumentPackage>\
         </sacm:AssuranceCasePackage>\
         <sacm:ArgumentPackage xmi:id=\"rd\"><name content=\"RD\"/>{}\
         <description xmi:id=\"d1\"><content><value content=\"D\"/></content></description>\
         {}<note xmi:id=\"n1\"/></sacm:ArgumentPackage>\
         </xmi:XMI>",
        claim(
            "b",
            "<description xmi:id=\"bd\"><content xmi:id=\"bd_content\">\
             <value content=\"BD\"/></content></description>"
        ),
        claim("c", ""),
        claim("d", ""),
        claim("g", ""),
        claim("c1", ""),
        claim("c2", ""),
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

fn read(text: &str) -> Document {
    match xmi::read(
        text.as_bytes(),
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

fn children(doc: &Document, parent: &str) -> Vec<String> {
    element(doc, parent)
        .children()
        .map(|child| child.id().as_str().to_owned())
        .collect()
}

fn roots(doc: &Document) -> Vec<String> {
    doc.roots()
        .map(|root| root.id().as_str().to_owned())
        .collect()
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
    ChildNotFound(Option<ElementId>, ElementId),
    InvalidOrder(Option<ElementId>),
}

fn refusal(operation: impl Into<Operation>) -> Refusal {
    let mut doc = doc();
    match refuse(&mut doc, operation) {
        EditError::ElementNotFound { element, .. } => Refusal::ElementNotFound(element),
        EditError::ChildNotFound { parent, child, .. } => Refusal::ChildNotFound(parent, child),
        EditError::InvalidOrder { repeated, .. } => Refusal::InvalidOrder(repeated),
        other => panic!("{ROW}: an EditError this row does not expect: {other:?}"),
    }
}

/// The findings of `validate`, as sorted lines: a reorder may change the
/// order `validate` reports them in (document order), not the findings.
fn findings(doc: &Document) -> Vec<String> {
    let mut lines: Vec<String> = validation::validate(doc)
        .iter()
        .map(|finding| format!("{finding:?}"))
        .collect();
    lines.sort();
    lines
}

// --- Item 1: the subset goes back into its positions --------------------------------

#[test]
fn lib_edit_010_a_named_subset_goes_back_into_the_positions_it_occupies() {
    let mut doc = doc();
    let before = doc.clone();
    assert_eq!(children(&doc, "ra"), ["a", "b", "c", "d"], "{ROW}: setup");
    let mutation = apply(&mut doc, ReorderElements::new("ra", ["d", "b"]));
    assert_eq!(
        children(&doc, "ra"),
        ["a", "d", "c", "b"],
        "{ROW}: [d, b] does not go back into the positions of b and d"
    );
    assert_eq!(mutation.changed_ids(), [id("ra")], "{ROW}: changed_ids");
    assert_eq!(
        (mutation.created_ids(), mutation.deleted_ids()),
        ([].as_slice(), [].as_slice()),
        "{ROW}: created_ids and deleted_ids"
    );
    assert_eq!(
        semantic_compare(&before, &doc),
        [Difference::ChildOrder {
            id: id("ra"),
            left: ids(&["a", "b", "c", "d"]),
            right: ids(&["a", "d", "c", "b"]),
        }],
        "{ROW}: the reorder is not one ChildOrder difference on its parent"
    );
}

#[test]
fn lib_edit_010_a_full_permutation_reorders_every_child() {
    let mut doc = doc();
    apply(&mut doc, ReorderElements::new("ra", ["c", "a", "d", "b"]));
    assert_eq!(
        children(&doc, "ra"),
        ["c", "a", "d", "b"],
        "{ROW}: a full permutation"
    );
    // One child alone stays where it is.
    let mutation = apply(&mut doc, ReorderElements::new("ra", ["d"]));
    assert_eq!(
        (children(&doc, "ra"), mutation.changed_ids()),
        (
            vec![
                "c".to_owned(),
                "a".to_owned(),
                "d".to_owned(),
                "b".to_owned()
            ],
            [].as_slice()
        ),
        "{ROW}: one child alone"
    );
}

#[test]
fn lib_edit_010_the_roots_reorder_the_same_way() {
    let mut doc = doc();
    let before = doc.clone();
    assert_eq!(roots(&doc), ["ra", "rb", "rc", "rd"], "{ROW}: setup");
    let mutation = apply(&mut doc, ReorderElements::roots(["rd", "rb"]));
    assert_eq!(
        roots(&doc),
        ["ra", "rd", "rc", "rb"],
        "{ROW}: [rd, rb] does not go back into the positions of rb and rd"
    );
    assert_eq!(
        mutation.changed_ids(),
        [] as [ElementId; 0],
        "{ROW}: a root reorder reports an id"
    );
    assert_eq!(
        semantic_compare(&before, &doc),
        [Difference::RootOrder {
            left: ids(&["ra", "rb", "rc", "rd"]),
            right: ids(&["ra", "rd", "rc", "rb"]),
        }],
        "{ROW}: the root reorder is not one RootOrder difference"
    );
    apply(&mut doc, ReorderElements::roots(["rd", "rc", "rb", "ra"]));
    assert_eq!(
        roots(&doc),
        ["rd", "rc", "rb", "ra"],
        "{ROW}: a full permutation of the roots"
    );
    // The pre-order follows the roots.
    let first: Vec<String> = doc
        .elements()
        .take(3)
        .map(|element| element.id().as_str().to_owned())
        .collect();
    assert_eq!(
        first,
        ["rd", "c1", "d1"],
        "{ROW}: elements() after a root reorder"
    );
}

// --- Item 2: across roles ---------------------------------------------------------

#[test]
fn lib_edit_010_a_reorder_across_roles_is_written_and_read_back_in_the_held_order() {
    let mut doc = doc();
    assert_eq!(
        children(&doc, "rd"),
        ["c1", "d1", "c2", "n1"],
        "{ROW}: setup"
    );
    apply(
        &mut doc,
        ReorderElements::new("rd", ["n1", "c2", "d1", "c1"]),
    );
    assert_eq!(
        children(&doc, "rd"),
        ["n1", "c2", "d1", "c1"],
        "{ROW}: the reorder across roles"
    );
    let rd = element(&doc, "rd");
    assert_eq!(
        (
            rd.descriptions()
                .map(|d| d.id().as_str())
                .collect::<Vec<_>>(),
            rd.notes().map(|n| n.id().as_str()).collect::<Vec<_>>()
        ),
        (vec!["d1"], vec!["n1"]),
        "{ROW}: the reorder changed a role"
    );
    let text = write(&doc);
    let position = |value: &str| match text.find(&format!("xmi:id=\"{value}\"")) {
        Some(position) => position,
        None => panic!("{ROW}: {value:?} is not written: {text}"),
    };
    assert!(
        position("n1") < position("c2")
            && position("c2") < position("d1")
            && position("d1") < position("c1"),
        "{ROW}: the children are not written in the held order: {text}"
    );
    let back = read(&text);
    assert_eq!(
        children(&back, "rd"),
        ["n1", "c2", "d1", "c1"],
        "{ROW}: the held order does not read back"
    );
}

// --- Item 3: what is compared, saved and loaded --------------------------------------

#[test]
fn lib_edit_010_after_write_and_read_nothing_more_differs() {
    let mut doc = doc();
    apply(&mut doc, ReorderElements::new("ra", ["d", "c", "b", "a"]));
    apply(&mut doc, ReorderElements::roots(["rc", "ra"]));
    let back = read(&write(&doc));
    assert_eq!(
        semantic_compare(&doc, &back),
        [],
        "{ROW}: the reordered document does not read back equal"
    );
    assert_eq!(
        (children(&back, "ra"), roots(&back)),
        (
            ["d", "c", "b", "a"].map(str::to_owned).to_vec(),
            ["rc", "rb", "ra", "rd"].map(str::to_owned).to_vec()
        ),
        "{ROW}: the orders read back"
    );
}

#[test]
fn lib_edit_010_the_current_order_is_ok_with_empty_changed_ids() {
    let mut doc = doc();
    let before = doc.clone();
    for operation in [
        ReorderElements::new("ra", ["a", "b", "c", "d"]),
        ReorderElements::new("ra", ["b", "d"]),
        ReorderElements::new("rc", ["sub"]),
        ReorderElements::roots(["ra", "rb", "rc", "rd"]),
        ReorderElements::roots(["rc"]),
    ] {
        let mutation = apply(&mut doc, operation.clone());
        assert_eq!(
            mutation.changed_ids(),
            [] as [ElementId; 0],
            "{ROW}: {operation:?} in the current order reports an id"
        );
    }
    assert!(
        doc == before,
        "{ROW}: the current order changed the document"
    );
    assert_eq!(
        next_generated_ids(&doc),
        next_generated_ids(&before),
        "{ROW}: the current order changed the next ids"
    );
}

// --- Item 4: refusals ----------------------------------------------------------------

#[test]
fn lib_edit_010_an_unknown_parent_is_element_not_found() {
    assert_eq!(
        refusal(ReorderElements::new("nowhere", ["a"])),
        Refusal::ElementNotFound(id("nowhere")),
        "{ROW}: an unknown parent"
    );
    let mut doc = doc();
    let error = refuse(&mut doc, ReorderElements::new("nowhere", ["a"]));
    assert_eq!(error.code(), Code::CMD_TARGET_NOT_FOUND, "{ROW}: the code");
    assert_eq!(
        error.to_string(),
        "the document has no element \"nowhere\" to edit",
        "{ROW}: the Display"
    );
}

#[test]
fn lib_edit_010_an_id_that_is_no_direct_child_or_root_is_child_not_found() {
    let cases: [(&str, ReorderElements, Refusal); 8] = [
        (
            "a grandchild",
            ReorderElements::new("rc", ["g"]),
            Refusal::ChildNotFound(Some(id("rc")), id("g")),
        ),
        (
            "another parent's child",
            ReorderElements::new("ra", ["c1"]),
            Refusal::ChildNotFound(Some(id("ra")), id("c1")),
        ),
        (
            "the parent itself",
            ReorderElements::new("ra", ["ra"]),
            Refusal::ChildNotFound(Some(id("ra")), id("ra")),
        ),
        (
            "the id of a name",
            ReorderElements::new("ra", ["a_name"]),
            Refusal::ChildNotFound(Some(id("ra")), id("a_name")),
        ),
        (
            "the id of a content",
            ReorderElements::new("b", ["bd_content"]),
            Refusal::ChildNotFound(Some(id("b")), id("bd_content")),
        ),
        (
            "a missing id",
            ReorderElements::new("ra", ["a", "nowhere"]),
            Refusal::ChildNotFound(Some(id("ra")), id("nowhere")),
        ),
        (
            "a child that is no root",
            ReorderElements::roots(["ra", "a"]),
            Refusal::ChildNotFound(None, id("a")),
        ),
        (
            "the id of a name among the roots",
            ReorderElements::roots(["a_name"]),
            Refusal::ChildNotFound(None, id("a_name")),
        ),
    ];
    for (what, operation, expected) in cases {
        assert_eq!(refusal(operation), expected, "{ROW}: {what}");
    }
    let mut doc = doc();
    let error = refuse(&mut doc, ReorderElements::new("rc", ["g"]));
    assert_eq!(error.code(), Code::CMD_TARGET_NOT_FOUND, "{ROW}: the code");
    assert_eq!(
        error.to_string(),
        "the element \"rc\" has no child \"g\"",
        "{ROW}: the Display for a child"
    );
    let error = refuse(&mut doc, ReorderElements::roots(["a"]));
    assert_eq!(
        error.to_string(),
        "the document has no root \"a\"",
        "{ROW}: the Display for a root"
    );
}

#[test]
fn lib_edit_010_a_repeated_id_or_an_empty_list_is_invalid_order() {
    let cases: [(&str, ReorderElements, Refusal); 5] = [
        (
            "a repeated child",
            ReorderElements::new("ra", ["a", "b", "a"]),
            Refusal::InvalidOrder(Some(id("a"))),
        ),
        (
            "a repeated root",
            ReorderElements::roots(["rb", "rb"]),
            Refusal::InvalidOrder(Some(id("rb"))),
        ),
        (
            "no child",
            ReorderElements::new("ra", Vec::<ElementId>::new()),
            Refusal::InvalidOrder(None),
        ),
        (
            "no root",
            ReorderElements::roots(Vec::<ElementId>::new()),
            Refusal::InvalidOrder(None),
        ),
        (
            "no child of a Note, which has none",
            ReorderElements::new("n1", Vec::<ElementId>::new()),
            Refusal::InvalidOrder(None),
        ),
    ];
    for (what, operation, expected) in cases {
        assert_eq!(refusal(operation), expected, "{ROW}: {what}");
    }
    let mut doc = doc();
    let error = refuse(&mut doc, ReorderElements::new("ra", ["c", "c"]));
    assert_eq!(error.code(), Code::CMD_INVALID_ORDER, "{ROW}: the code");
    assert_eq!(
        error.code().as_str(),
        "SACM-CMD-009",
        "{ROW}: the code text"
    );
    assert_eq!(
        error.to_string(),
        "the new order names \"c\" more than once",
        "{ROW}: the Display of a repeat"
    );
    let error = refuse(&mut doc, ReorderElements::roots(Vec::<ElementId>::new()));
    assert_eq!(
        error.to_string(),
        "the new order names no element",
        "{ROW}: the Display of an empty list"
    );
}

#[test]
fn lib_edit_010_the_checks_run_in_order_and_id_by_id() {
    let cases: [(&str, ReorderElements, Refusal); 5] = [
        (
            "an unknown parent before an empty list",
            ReorderElements::new("nowhere", Vec::<ElementId>::new()),
            Refusal::ElementNotFound(id("nowhere")),
        ),
        (
            "a repeat before a later id that is no child",
            ReorderElements::new("ra", ["a", "a", "nowhere"]),
            Refusal::InvalidOrder(Some(id("a"))),
        ),
        (
            "an id that is no child before a later repeat",
            ReorderElements::new("ra", ["nowhere", "a", "a"]),
            Refusal::ChildNotFound(Some(id("ra")), id("nowhere")),
        ),
        (
            "a child of a Note, which has none",
            ReorderElements::new("n1", ["a"]),
            Refusal::ChildNotFound(Some(id("n1")), id("a")),
        ),
        (
            "a root that is no root before a repeat",
            ReorderElements::roots(["g", "ra", "ra"]),
            Refusal::ChildNotFound(None, id("g")),
        ),
    ];
    for (what, operation, expected) in cases {
        assert_eq!(refusal(operation), expected, "{ROW}: {what}");
    }
}

// --- Item 5: nothing else changes ---------------------------------------------------

#[test]
fn lib_edit_010_element_count_roles_parents_and_findings_do_not_change() {
    // `h` nests the ArgumentPackage h1 beside the Claim hc: a §11.4
    // homogeneity finding (SACM-PKG-001), and hc has no name.
    let text = format!(
        "{WRAPPER_OPEN}\
         <sacm:ArgumentPackage xmi:id=\"h\"><name content=\"H\"/>\
         <argumentElement xmi:type=\"sacm:ArgumentPackage\" xmi:id=\"h1\"><name content=\"H1\"/></argumentElement>\
         <note xmi:id=\"hn\"/>\
         <argumentElement xmi:type=\"sacm:Claim\" xmi:id=\"hc\"/>\
         </sacm:ArgumentPackage>\
         <sacm:ArgumentPackage xmi:id=\"other\"/>\
         </xmi:XMI>"
    );
    let mut doc = read(&text);
    let before = doc.clone();
    let found = findings(&doc);
    assert!(
        validation::validate(&doc)
            .iter()
            .any(|finding| finding.code() == Code::PACKAGE_CONTENT_INVALID),
        "{ROW}: setup: no homogeneity finding"
    );
    apply(&mut doc, ReorderElements::new("h", ["hc", "hn", "h1"]));
    apply(&mut doc, ReorderElements::roots(["other", "h"]));
    assert_eq!(
        children(&doc, "h"),
        ["hc", "hn", "h1"],
        "{ROW}: the reorder"
    );
    assert_eq!(
        doc.element_count(),
        before.element_count(),
        "{ROW}: the element count changed"
    );
    for element in before.elements() {
        let now = element_or_panic(&doc, element.id());
        assert_eq!(
            (now.kind(), now.parent().map(|p| p.id().clone())),
            (element.kind(), element.parent().map(|p| p.id().clone())),
            "{ROW}: the kind or parent of {:?} changed",
            element.id()
        );
        let notes = |e: SacmElement<'_>| e.notes().map(|n| n.id().clone()).collect::<Vec<_>>();
        assert_eq!(
            notes(now),
            notes(element),
            "{ROW}: the notes of {:?} changed",
            element.id()
        );
    }
    assert_eq!(findings(&doc), found, "{ROW}: the findings changed");
    assert_eq!(
        next_generated_ids(&doc),
        next_generated_ids(&before),
        "{ROW}: the counters changed"
    );
}

fn element_or_panic<'d>(doc: &'d Document, value: &ElementId) -> SacmElement<'d> {
    element(doc, value.as_str())
}

// --- Accessors, names and equality ---------------------------------------------------

#[test]
fn lib_edit_010_operations_hold_what_they_are_given_and_name_their_struct() {
    let children = ReorderElements::new(id("ra"), ["d", "b"]);
    assert_eq!(
        (children.parent(), children.order()),
        (Some(&id("ra")), ids(&["d", "b"]).as_slice()),
        "{ROW}: ReorderElements::new's accessors"
    );
    let roots = ReorderElements::roots([id("rb"), id("ra")]);
    assert_eq!(
        (roots.parent(), roots.order()),
        (None, ids(&["rb", "ra"]).as_slice()),
        "{ROW}: ReorderElements::roots's accessors"
    );
    assert_ne!(
        ReorderElements::new("ra", ["a"]),
        ReorderElements::roots(["a"]),
        "{ROW}: a child reorder equals a root reorder"
    );
    assert_eq!(children.clone(), children, "{ROW}: Clone + Eq");
    for operation in [Operation::from(children), roots.into()] {
        assert_eq!(
            operation.operation_name(),
            "ReorderElements",
            "{ROW}: operation_name()"
        );
    }
    // The operation is held as given in the mutation.
    let mut doc = doc();
    let given = ReorderElements::new("ra", ["b", "a"]);
    let mutation = apply(&mut doc, given.clone());
    assert_eq!(
        mutation.operation(),
        &Operation::from(given),
        "{ROW}: operation()"
    );
}
