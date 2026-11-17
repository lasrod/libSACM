//! Tests for the referrers of an id (LIB-API-014; ADR 0036 D3).
//!
//! `Document::referrers(id)` lists every typed reference that names `id`,
//! whether or not an element has that id: the holder, the end and the id's
//! position in the end. They are ordered by holder in document pre-order,
//! then by end in `ReferenceEnd::ALL` order, then by position; an id held
//! twice in one end gives two entries. References preserved content may
//! hold are not typed and are not listed.
//!
//! The oracle, [`full_scan`], walks every element in pre-order and reads
//! every end through `SacmElement::references` (LIB-API-015).
//!
//! Elsewhere: `edit_atomic.rs` checks that the referrers equal the full scan
//! after every successful operation of LIB-EDIT-002's set.

use std::collections::BTreeSet;
use std::fmt::Debug;
use std::path::Path;

use sacm::edit::{
    self, AddReference, CopyElements, CrossPackageReferencePolicy, DeleteElement, MoveElement,
    Operation, PackageDeletePolicy, ReferenceDeletePolicy, RemoveReference, ReorderElements,
    SetReferences,
};
use sacm::model::{Document, ElementId, ReferenceEnd, Referrer, SacmElement};
use sacm::xmi::{self, ReadOptions};

const ROW: &str = "LIB-API-014";

/// The Claim `top` is the target of the asserted inferences `i1` and `i2`,
/// the source of `i3`, and twice the `metaClaim` of the Claim `b`.
const TOP_START: &str = r#"<sacm:AssuranceCasePackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301" xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="case">
  <name content="Case"/>
  <argumentPackage xmi:id="p">
    <name content="P"/>
    <argumentElement xmi:type="sacm:Claim" xmi:id="top"><name content="Top"/></argumentElement>
    <argumentElement xmi:type="sacm:Claim" xmi:id="a"><name content="A"/></argumentElement>
    <argumentElement xmi:type="sacm:Claim" xmi:id="b" metaClaim="top top"><name content="B"/></argumentElement>
    <argumentElement xmi:type="sacm:AssertedInference" xmi:id="i1" source="a" target="top"><name content="I1"/></argumentElement>
    <argumentElement xmi:type="sacm:AssertedInference" xmi:id="i2" source="b" target="top"><name content="I2"/></argumentElement>
    <argumentElement xmi:type="sacm:AssertedInference" xmi:id="i3" source="top" target="a"><name content="I3"/></argumentElement>
  </argumentPackage>
</sacm:AssuranceCasePackage>
"#;

/// Holders at three depths in two packages: in `p`, the nested package `pn`
/// with the Claim `h3`, then `top` and `h1`; in `q`, `h2`. Each `h` names
/// `top` as its `metaClaim`.
const ORDER_START: &str = r#"<sacm:AssuranceCasePackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301" xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="case">
  <name content="Case"/>
  <argumentPackage xmi:id="p">
    <name content="P"/>
    <argumentElement xmi:type="sacm:ArgumentPackage" xmi:id="pn">
      <name content="Pn"/>
      <argumentElement xmi:type="sacm:Claim" xmi:id="h3" metaClaim="top"><name content="H3"/></argumentElement>
    </argumentElement>
    <argumentElement xmi:type="sacm:Claim" xmi:id="top"><name content="Top"/></argumentElement>
    <argumentElement xmi:type="sacm:Claim" xmi:id="h1" metaClaim="top"><name content="H1"/></argumentElement>
  </argumentPackage>
  <argumentPackage xmi:id="q">
    <name content="Q"/>
    <argumentElement xmi:type="sacm:Claim" xmi:id="h2" metaClaim="top"><name content="H2"/></argumentElement>
  </argumentPackage>
</sacm:AssuranceCasePackage>
"#;

/// One holder with four ends naming `top`, written in another order than
/// `ReferenceEnd::ALL`: the `AssertedInference` `i` with the target `top`, the
/// sources `a`, `top` and `top`, the `metaClaim` `top` and the
/// `citedElement` `top`.
const ENDS_START: &str = r#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301" xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="p">
  <name content="P"/>
  <argumentElement xmi:type="sacm:Claim" xmi:id="top"><name content="Top"/></argumentElement>
  <argumentElement xmi:type="sacm:Claim" xmi:id="a"><name content="A"/></argumentElement>
  <argumentElement xmi:type="sacm:AssertedInference" xmi:id="i" target="top" source="a top top" metaClaim="top" isCitation="true" citedElement="top"><name content="I"/></argumentElement>
</sacm:ArgumentPackage>
"#;

/// Ids that no element has: `c1` names `ghost` (dangling), `x1` (an id of
/// `c2`'s preserved content) and `c1_name` (the `xmi:id` of its own name),
/// as its `metaClaim`s. `c3` is referenced by nothing, and `c4` only by the
/// preserved `<v:ref xmi:idref="c4"/>` of `c2`.
const IDS_START: &str = r#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301" xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="p">
  <name content="P"/>
  <argumentElement xmi:type="sacm:Claim" xmi:id="c1" metaClaim="ghost x1 c1_name"><name xmi:id="c1_name" content="C1"/></argumentElement>
  <argumentElement xmi:type="sacm:Claim" xmi:id="c2"><name content="C2"/><v:extra xmlns:v="urn:v" xmi:id="x1"/><v:ref xmlns:v="urn:v" xmi:idref="c4"/></argumentElement>
  <argumentElement xmi:type="sacm:Claim" xmi:id="c3"><name content="C3"/></argumentElement>
  <argumentElement xmi:type="sacm:Claim" xmi:id="c4"><name content="C4"/></argumentElement>
</sacm:ArgumentPackage>
"#;

/// The delete start document of `edit_atomic.rs`: references inside a
/// package, across packages and into a nested one, relationships that name
/// relationships, and the dangling `ghost`.
const DELETE_START: &str = r#"<xmi:XMI xmlns:sacm="http://www.omg.org/spec/SACM/20220301" xmlns:xmi="http://www.omg.org/spec/XMI/20131001">
  <sacm:AssuranceCasePackage xmi:id="dcase">
    <name content="Delete case"/>
    <argumentPackage xmi:id="dp">
      <name content="DP"/>
      <argumentElement xmi:type="sacm:Claim" xmi:id="goal"><name content="Goal"/></argumentElement>
      <argumentElement xmi:type="sacm:Claim" xmi:id="sub_a" abstractForm="goal"><name content="A"/><note xmi:id="sub_a_note"/></argumentElement>
      <argumentElement xmi:type="sacm:Claim" xmi:id="sub_b" metaClaim="goal ghost goal"><name content="B"/></argumentElement>
      <argumentElement xmi:type="sacm:ArgumentReasoning" xmi:id="strategy" structure="dnested"><name content="Strategy"/></argumentElement>
      <argumentElement xmi:type="sacm:AssertedInference" xmi:id="inf_ab" source="sub_a sub_b" target="goal" reasoning="strategy">
        <name content="Iab"/>
        <description xmi:id="inf_ab_d"><content><value content="Both"/></content></description>
      </argumentElement>
      <argumentElement xmi:type="sacm:AssertedInference" xmi:id="inf_a" source="sub_a" target="goal"><name content="Ia"/></argumentElement>
      <argumentElement xmi:type="sacm:AssertedContext" xmi:id="ctx_on_inf" source="evidence_ref" target="inf_a"><name content="Ctx"/></argumentElement>
      <argumentElement xmi:type="sacm:AssertedInference" xmi:id="inf_on_ctx" source="ctx_on_inf" target="sub_b"><name content="Ictx"/></argumentElement>
      <argumentElement xmi:type="sacm:ArtifactReference" xmi:id="evidence_ref" referencedArtifactElement="sub_a far ghost"><name content="E"/></argumentElement>
      <argumentElement xmi:type="sacm:ArgumentGroup" xmi:id="group" argumentElement="sub_a inf_a ghost"><name content="G"/></argumentElement>
      <argumentElement xmi:type="sacm:ArgumentPackage" xmi:id="dnested">
        <name content="Nested"/>
        <argumentElement xmi:type="sacm:Claim" xmi:id="deep" citedElement="goal"><name content="Deep"/></argumentElement>
        <argumentElement xmi:type="sacm:AssertedInference" xmi:id="inf_deep" source="deep" target="sub_b"><name content="Id"/></argumentElement>
      </argumentElement>
    </argumentPackage>
    <argumentPackage xmi:id="dother">
      <name content="Other"/>
      <argumentElement xmi:type="sacm:Claim" xmi:id="far" citedElement="sub_b" metaClaim="goal"><name content="Far"/></argumentElement>
      <argumentElement xmi:type="sacm:AssertedEvidence" xmi:id="far_ev" source="evidence_ref" target="far"><name content="Fe"/></argumentElement>
      <argumentElement xmi:type="sacm:AssertedInference" xmi:id="far_inf" source="far" target="sub_b"><name content="Fi"/></argumentElement>
    </argumentPackage>
  </sacm:AssuranceCasePackage>
  <sacm:ArgumentPackage xmi:id="dloose"><name content="Loose"/><note xmi:id="dloose_note"/></sacm:ArgumentPackage>
</xmi:XMI>
"#;

/// The directory of the XMI fixtures.
const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/sacm23/xmi");

/// The twelve ends, in the order `ReferenceEnd::ALL` documents: write order.
const ENDS: [ReferenceEnd; 12] = [
    ReferenceEnd::CitedElement,
    ReferenceEnd::AbstractForm,
    ReferenceEnd::MetaClaim,
    ReferenceEnd::Reasoning,
    ReferenceEnd::Source,
    ReferenceEnd::Target,
    ReferenceEnd::Structure,
    ReferenceEnd::ReferencedArtifactElement,
    ReferenceEnd::GroupMember,
    ReferenceEnd::ArtifactGroupMember,
    ReferenceEnd::ArtifactSource,
    ReferenceEnd::ArtifactTarget,
];

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// One referrer as plain values: holder id, end and position.
type Entry = (String, ReferenceEnd, usize);

/// A dangling reference with its sort key: the holder's rank in pre-order,
/// the end's rank in `ReferenceEnd::ALL`, the position; then the holder, the
/// end and the id.
type Ranked = (
    Option<usize>,
    Option<usize>,
    usize,
    ElementId,
    ReferenceEnd,
    ElementId,
);

fn entry(holder: &str, end: ReferenceEnd, position: usize) -> Entry {
    (holder.to_owned(), end, position)
}

/// Reads `input` with the default (tolerant) options.
fn read(what: &str, input: &[u8]) -> Document {
    match xmi::read(input, &ReadOptions::default()) {
        Ok(output) => output.into_document(),
        Err(error) => panic!("{ROW}: setup: {what} does not read: {error}"),
    }
}

/// Applies `operation`, which must be accepted, to `doc`.
fn apply(what: &str, doc: &mut Document, operation: impl Into<Operation>) {
    let operation = operation.into();
    if let Err(error) = edit::apply(doc, operation.clone()) {
        panic!("{ROW}: {what}: {operation:?} was refused: {error}");
    }
}

/// The referrers of `id` in `doc`, as plain values.
fn referrers(doc: &Document, id: &str) -> Vec<Entry> {
    doc.referrers(id)
        .into_iter()
        .map(|referrer| {
            (
                referrer.holder().id().as_str().to_owned(),
                referrer.end(),
                referrer.position(),
            )
        })
        .collect()
}

/// Every typed reference of `doc` that names `id`, by a walk of every
/// element in pre-order, every end in [`ENDS`] order and every position.
fn full_scan(doc: &Document, id: &str) -> Vec<Entry> {
    let mut found = Vec::new();
    for holder in doc.elements() {
        for end in ENDS {
            for (position, held) in holder.references(end).iter().enumerate() {
                if held.as_str() == id {
                    found.push(entry(holder.id().as_str(), end, position));
                }
            }
        }
    }
    found
}

/// Every id any end of `doc` holds, and every element id, and an id of
/// nothing.
fn every_id(doc: &Document) -> BTreeSet<String> {
    let mut found: BTreeSet<String> = BTreeSet::new();
    for element in doc.elements() {
        found.insert(element.id().as_str().to_owned());
        for end in ENDS {
            for held in element.references(end) {
                found.insert(held.as_str().to_owned());
            }
        }
    }
    found.insert("nowhere".to_owned());
    found
}

/// `referrers` equals the full scan for every id of [`every_id`]. Returns
/// how many ids had a referrer.
fn assert_referrers_are_the_full_scan(what: &str, doc: &Document) -> usize {
    let mut referenced = 0;
    for value in every_id(doc) {
        let expected = full_scan(doc, &value);
        if !expected.is_empty() {
            referenced += 1;
        }
        assert_eq!(
            referrers(doc, &value),
            expected,
            "{ROW}: {what}: the referrers of {value:?} are not the full scan in pre-order, ALL \
             order and position"
        );
    }
    referenced
}

/// `element` and everything it contains, in pre-order.
fn subtree_of(element: SacmElement<'_>) -> Vec<ElementId> {
    let mut found = vec![element.id().clone()];
    for child in element.children() {
        found.extend(subtree_of(child));
    }
    found
}

// ---------------------------------------------------------------------------
// Acceptance 1: what and in which order
// ---------------------------------------------------------------------------

#[test]
fn lib_api_014_referrers_list_holder_end_and_position_in_order() {
    let mut doc = read("TOP_START", TOP_START.as_bytes());
    let expected = vec![
        entry("b", ReferenceEnd::MetaClaim, 0),
        entry("b", ReferenceEnd::MetaClaim, 1),
        entry("i1", ReferenceEnd::Target, 0),
        entry("i2", ReferenceEnd::Target, 0),
        entry("i3", ReferenceEnd::Source, 0),
    ];
    assert_eq!(
        referrers(&doc, "top"),
        expected,
        "{ROW}: the five referrers of top, in pre-order, end and position"
    );
    let first = doc.referrers("top");
    assert_eq!(first.len(), 5, "{ROW}: five referrers");
    assert_eq!(
        first[2].holder().kind(),
        sacm::model::ElementKind::AssertedInference,
        "{ROW}: holder() is the holding element"
    );
    // An &ElementId and a &String are ids too.
    assert_eq!(
        doc.referrers(&ElementId::from("top")).len(),
        5,
        "{ROW}: referrers of an &ElementId"
    );
    assert_eq!(
        doc.referrers(&"top".to_owned()).len(),
        5,
        "{ROW}: referrers of a &String"
    );

    apply(
        "swap i1 and i2",
        &mut doc,
        ReorderElements::new("p", ["top", "a", "b", "i2", "i1", "i3"]),
    );
    assert_eq!(
        referrers(&doc, "top"),
        vec![
            entry("b", ReferenceEnd::MetaClaim, 0),
            entry("b", ReferenceEnd::MetaClaim, 1),
            entry("i2", ReferenceEnd::Target, 0),
            entry("i1", ReferenceEnd::Target, 0),
            entry("i3", ReferenceEnd::Source, 0),
        ],
        "{ROW}: after the reorder, the entries of i1 and i2 swap"
    );
    assert_referrers_are_the_full_scan("TOP_START reordered", &doc);
}

#[test]
fn lib_api_014_referrers_follow_pre_order_after_a_reorder_and_a_move() {
    let mut doc = read("ORDER_START", ORDER_START.as_bytes());
    let holders = |doc: &Document| -> Vec<String> {
        referrers(doc, "top")
            .into_iter()
            .map(|(holder, _, _)| holder)
            .collect()
    };
    // h3 is deeper than h1, and comes first in pre-order.
    assert_eq!(holders(&doc), ["h3", "h1", "h2"], "{ROW}: the start");

    apply(
        "q before p",
        &mut doc,
        ReorderElements::new("case", ["q", "p"]),
    );
    assert_eq!(
        holders(&doc),
        ["h2", "h3", "h1"],
        "{ROW}: after q moves before p"
    );

    apply(
        "h1 into q first",
        &mut doc,
        MoveElement::new("h1", "q").with_position(0),
    );
    assert_eq!(
        holders(&doc),
        ["h1", "h2", "h3"],
        "{ROW}: after h1 moves to q at 0"
    );

    apply("h1 back to p", &mut doc, MoveElement::new("h1", "p"));
    assert_eq!(
        holders(&doc),
        ["h2", "h3", "h1"],
        "{ROW}: after h1 moves back to p"
    );

    apply("pn into q", &mut doc, MoveElement::new("pn", "q"));
    assert_eq!(
        holders(&doc),
        ["h2", "h3", "h1"],
        "{ROW}: after pn moves to the end of q"
    );

    apply(
        "pn to the roots first",
        &mut doc,
        MoveElement::root("pn").with_position(0),
    );
    assert_eq!(
        holders(&doc),
        ["h3", "h2", "h1"],
        "{ROW}: after pn becomes the first root"
    );

    apply(
        "the roots reversed",
        &mut doc,
        ReorderElements::roots(["case", "pn"]),
    );
    assert_eq!(
        holders(&doc),
        ["h2", "h1", "h3"],
        "{ROW}: after the roots are reversed"
    );
    assert_referrers_are_the_full_scan("ORDER_START edited", &doc);
}

#[test]
fn lib_api_014_an_id_held_twice_in_one_end_gives_two_entries() {
    let doc = read("ENDS_START", ENDS_START.as_bytes());
    let sources: Vec<Entry> = referrers(&doc, "top")
        .into_iter()
        .filter(|(_, end, _)| *end == ReferenceEnd::Source)
        .collect();
    assert_eq!(
        sources,
        [
            entry("i", ReferenceEnd::Source, 1),
            entry("i", ReferenceEnd::Source, 2)
        ],
        "{ROW}: top held twice in i's sources gives two entries, at its positions"
    );
    let doc = read("TOP_START", TOP_START.as_bytes());
    let meta: Vec<Entry> = referrers(&doc, "top")
        .into_iter()
        .filter(|(_, end, _)| *end == ReferenceEnd::MetaClaim)
        .collect();
    assert_eq!(
        meta,
        [
            entry("b", ReferenceEnd::MetaClaim, 0),
            entry("b", ReferenceEnd::MetaClaim, 1)
        ],
        "{ROW}: top held twice in b's metaClaim gives two entries"
    );
}

#[test]
fn lib_api_014_one_holder_with_two_ends_gives_an_entry_per_end_in_all_order() {
    let doc = read("ENDS_START", ENDS_START.as_bytes());
    assert_eq!(
        referrers(&doc, "top"),
        vec![
            entry("i", ReferenceEnd::CitedElement, 0),
            entry("i", ReferenceEnd::MetaClaim, 0),
            entry("i", ReferenceEnd::Source, 1),
            entry("i", ReferenceEnd::Source, 2),
            entry("i", ReferenceEnd::Target, 0),
        ],
        "{ROW}: one holder's ends come in ReferenceEnd::ALL order, not the order written"
    );
    assert_eq!(
        referrers(&doc, "a"),
        vec![entry("i", ReferenceEnd::Source, 0)],
        "{ROW}: the referrers of a"
    );
}

// ---------------------------------------------------------------------------
// Acceptance 2: ids that no element has, and preserved content
// ---------------------------------------------------------------------------

#[test]
fn lib_api_014_an_unreferenced_id_and_one_named_only_by_preserved_content_give_none() {
    let doc = read("IDS_START", IDS_START.as_bytes());
    for value in ["c3", "nowhere", "p"] {
        assert_eq!(
            referrers(&doc, value),
            [] as [Entry; 0],
            "{ROW}: {value:?}, which nothing references, has referrers"
        );
    }
    assert!(
        doc.get("c2")
            .is_some_and(SacmElement::has_preserved_content),
        "{ROW}: setup: c2 holds no preserved content"
    );
    assert_eq!(
        referrers(&doc, "c4"),
        [] as [Entry; 0],
        "{ROW}: c4, named only by preserved content, has referrers"
    );
}

#[test]
fn lib_api_014_a_dangling_id_and_a_preserved_id_give_their_referrers() {
    let doc = read("IDS_START", IDS_START.as_bytes());
    for (value, position) in [("ghost", 0), ("x1", 1), ("c1_name", 2)] {
        assert!(
            !doc.contains(value),
            "{ROW}: setup: {value:?} is an element"
        );
        assert_eq!(
            referrers(&doc, value),
            vec![entry("c1", ReferenceEnd::MetaClaim, position)],
            "{ROW}: the referrers of {value:?}, which no element has"
        );
    }
    let delete = read("DELETE_START", DELETE_START.as_bytes());
    assert_eq!(
        referrers(&delete, "ghost"),
        vec![
            entry("sub_b", ReferenceEnd::MetaClaim, 1),
            entry("evidence_ref", ReferenceEnd::ReferencedArtifactElement, 2),
            entry("group", ReferenceEnd::GroupMember, 2),
        ],
        "{ROW}: the referrers of the dangling ghost"
    );
}

// ---------------------------------------------------------------------------
// Acceptance 3: a read document
// ---------------------------------------------------------------------------

#[test]
fn lib_api_014_referrers_of_a_read_document_equal_a_full_scan() {
    let mut referenced = 0;
    for (what, text) in [
        ("TOP_START", TOP_START),
        ("ORDER_START", ORDER_START),
        ("ENDS_START", ENDS_START),
        ("IDS_START", IDS_START),
        ("DELETE_START", DELETE_START),
    ] {
        referenced += assert_referrers_are_the_full_scan(what, &read(what, text.as_bytes()));
    }
    let entries = match std::fs::read_dir(FIXTURES) {
        Ok(entries) => entries,
        Err(error) => panic!("{ROW}: setup: cannot list {FIXTURES}: {error}"),
    };
    let mut fixtures: Vec<_> = entries
        .filter_map(Result::ok)
        .map(|found| found.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with("-valid.sacm.xmi"))
        })
        .collect();
    fixtures.sort();
    assert!(
        !fixtures.is_empty(),
        "{ROW}: setup: no fixture in {FIXTURES}"
    );
    for path in &fixtures {
        let bytes = std::fs::read(path).unwrap_or_else(|error| {
            panic!("{ROW}: setup: cannot read {}: {error}", path.display())
        });
        let what = Path::new(path).display().to_string();
        referenced += assert_referrers_are_the_full_scan(&what, &read(&what, &bytes));
    }
    assert!(
        referenced > 20,
        "{ROW}: setup: only {referenced} ids had referrers"
    );
}

#[test]
fn lib_api_014_referrers_equal_a_full_scan_after_each_edit_of_a_sequence() {
    // Each edit keeps the index: references set, added and removed, holders
    // reordered, moved and copied, and holders and targets deleted.
    let mut doc = read("DELETE_START", DELETE_START.as_bytes());
    let source = doc.clone();
    let copy = |elements: &[&str], parent: &str| -> Operation {
        match CopyElements::new(&source, elements.iter().copied(), parent) {
            Ok(copy) => copy.into(),
            Err(error) => panic!("{ROW}: setup: the copy of {elements:?} was refused: {error}"),
        }
    };
    let steps: Vec<(&str, Operation)> = vec![
        (
            "far's metaClaim set to sub_a twice",
            SetReferences::new("far", ReferenceEnd::MetaClaim, ["sub_a", "sub_a"]).into(),
        ),
        (
            "deep's metaClaim gains sub_a",
            AddReference::new("deep", ReferenceEnd::MetaClaim, "sub_a").into(),
        ),
        (
            "inf_ab loses sub_a",
            RemoveReference::new("inf_ab", ReferenceEnd::Source, "sub_a").into(),
        ),
        (
            "dother before dp",
            ReorderElements::new("dcase", ["dother", "dp"]).into(),
        ),
        (
            "group, which names sub_a, to the start of dother",
            MoveElement::new("group", "dother").with_position(0).into(),
        ),
        (
            "dnested, with its holders, to the roots",
            MoveElement::root("dnested").with_position(0).into(),
        ),
        (
            "inf_a, a holder and a target, copied into dother",
            copy(&["inf_a", "ctx_on_inf"], "dother"),
        ),
        (
            "the holder far deleted",
            DeleteElement::new("far")
                .with_reference_policy(ReferenceDeletePolicy::ScrubReferences)
                .with_cross_package_policy(CrossPackageReferencePolicy::ApplyReferencePolicy)
                .into(),
        ),
        (
            "the target sub_a deleted, its referrers scrubbed",
            DeleteElement::new("sub_a")
                .with_reference_policy(ReferenceDeletePolicy::ScrubReferences)
                .with_package_policy(PackageDeletePolicy::DeleteRecursively)
                .with_cross_package_policy(CrossPackageReferencePolicy::ApplyReferencePolicy)
                .into(),
        ),
        (
            "dp deleted with what references it",
            DeleteElement::new("dp")
                .with_reference_policy(ReferenceDeletePolicy::DeleteReferencingRelationships)
                .with_package_policy(PackageDeletePolicy::DeleteRecursively)
                .with_cross_package_policy(CrossPackageReferencePolicy::ApplyReferencePolicy)
                .into(),
        ),
    ];
    let mut checked = 0;
    for (what, operation) in steps {
        apply(what, &mut doc, operation);
        checked += assert_referrers_are_the_full_scan(what, &doc);
        // Every id a step left unreferenced has no referrer, even one that
        // was held before.
        for value in ["sub_a", "far", "ghost", "goal", "sub_b"] {
            assert_eq!(
                referrers(&doc, value),
                full_scan(&doc, value),
                "{ROW}: {what}: the referrers of {value:?}"
            );
        }
    }
    assert!(
        checked > 10,
        "{ROW}: setup: only {checked} ids had referrers"
    );
}

#[test]
fn lib_api_014_referrers_follow_pre_order_across_a_copied_root_and_a_later_one() {
    // A copy of p as a root holds claim_2, whose metaClaim is claim_1, the
    // copy of c1; a root created after it holds x, whose metaClaim is claim_1
    // too. Pre-order: the copy's holder first.
    let start = read("TOP_START", TOP_START.as_bytes());
    let mut doc = start.clone();
    let copy = match CopyElements::root(&start, ["p"]) {
        Ok(copy) => copy,
        Err(error) => panic!("{ROW}: setup: the copy was refused: {error}"),
    };
    apply("p copied as a root", &mut doc, copy);
    let holder = doc
        .referrers("claim_1")
        .first()
        .map(|referrer| referrer.holder().id().clone());
    apply(
        "a root created after the copy",
        &mut doc,
        edit::CreateArgumentPackage::root(sacm::model::LangString::new("Later")).with_id("later"),
    );
    apply(
        "a Claim in it",
        &mut doc,
        edit::CreateClaim::new("later", sacm::model::LangString::new("X")).with_id("x"),
    );
    apply(
        "x names claim_1",
        &mut doc,
        SetReferences::new("x", ReferenceEnd::MetaClaim, ["claim_1"]),
    );
    let holders: Vec<String> = referrers(&doc, "claim_1")
        .into_iter()
        .map(|(holder, _, _)| holder)
        .collect();
    assert_eq!(
        holders.last().map(String::as_str),
        Some("x"),
        "{ROW}: the holder in the later root does not come last: {holders:?}"
    );
    assert_eq!(
        holders.first().cloned(),
        holder.map(|id| id.as_str().to_owned()),
        "{ROW}: the holder in the copied root does not come first: {holders:?}"
    );
    assert_referrers_are_the_full_scan("a copied root and a later one", &doc);
}

/// The holders of the referrers of `id` in `doc`, in order.
fn holders_of(doc: &Document, id: &str) -> Vec<String> {
    referrers(doc, id)
        .into_iter()
        .map(|(holder, _, _)| holder)
        .collect()
}

#[test]
fn lib_api_014_referrers_follow_pre_order_after_a_root_delete_and_a_later_create() {
    // Three roots: r1; r2 holding the Claim x; r3 holding the Claims a and
    // b, b naming x. r1 is deleted, then a root r4 is created with a Claim c
    // naming x: c comes after b in pre-order.
    let mut doc = read(
        "three roots",
        br#"<xmi:XMI xmlns:sacm="http://www.omg.org/spec/SACM/20220301" xmlns:xmi="http://www.omg.org/spec/XMI/20131001">
  <sacm:ArgumentPackage xmi:id="r1"><name content="R1"/></sacm:ArgumentPackage>
  <sacm:ArgumentPackage xmi:id="r2"><name content="R2"/><argumentElement xmi:type="sacm:Claim" xmi:id="x"><name content="X"/></argumentElement></sacm:ArgumentPackage>
  <sacm:ArgumentPackage xmi:id="r3"><name content="R3"/><argumentElement xmi:type="sacm:Claim" xmi:id="a"><name content="A"/></argumentElement><argumentElement xmi:type="sacm:Claim" xmi:id="b" metaClaim="x"><name content="B"/></argumentElement></sacm:ArgumentPackage>
</xmi:XMI>
"#,
    );
    apply("r1 deleted", &mut doc, DeleteElement::new("r1"));
    apply(
        "the root r4",
        &mut doc,
        edit::CreateArgumentPackage::root(sacm::model::LangString::new("R4")).with_id("r4"),
    );
    apply(
        "the Claim c in r4",
        &mut doc,
        edit::CreateClaim::new("r4", sacm::model::LangString::new("C")).with_id("c"),
    );
    apply(
        "c names x",
        &mut doc,
        SetReferences::new("c", ReferenceEnd::MetaClaim, ["x"]),
    );
    assert_eq!(
        holders_of(&doc, "x"),
        ["b", "c"],
        "{ROW}: after a root delete and a later create, the referrers of x are not in pre-order"
    );
    assert_referrers_are_the_full_scan("a root deleted and another created", &doc);
}

#[test]
fn lib_api_014_referrers_follow_pre_order_into_a_root_copy() {
    // The root p holds the Claims a and b, b naming a; the source's root s
    // holds the Claim c naming a. s is copied as a root after p, and the
    // copy of c, which keeps its id and its reference to a, comes after b.
    let mut doc = read(
        "the target",
        br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301" xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="p"><name content="P"/><argumentElement xmi:type="sacm:Claim" xmi:id="a"><name content="A"/></argumentElement><argumentElement xmi:type="sacm:Claim" xmi:id="b" metaClaim="a"><name content="B"/></argumentElement></sacm:ArgumentPackage>"#,
    );
    let source = read(
        "the source",
        br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301" xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="s"><name content="S"/><argumentElement xmi:type="sacm:Claim" xmi:id="c" metaClaim="a"><name content="C"/></argumentElement></sacm:ArgumentPackage>"#,
    );
    let copy = match CopyElements::root(&source, ["s"]) {
        Ok(copy) => copy,
        Err(error) => panic!("{ROW}: setup: the copy was refused: {error}"),
    };
    apply("s copied as a root", &mut doc, copy);
    assert_eq!(
        holders_of(&doc, "a"),
        ["b", "c"],
        "{ROW}: after a root copy, the referrers of a are not in pre-order"
    );
    assert_referrers_are_the_full_scan("a root copied in", &doc);
}

// ---------------------------------------------------------------------------
// Acceptance 4: the dangling references of a delete
// ---------------------------------------------------------------------------

#[test]
fn lib_api_014_the_dangling_references_of_a_delete_are_referrers_outside_the_subtree() {
    let doc = read("DELETE_START", DELETE_START.as_bytes());
    let pre_order: Vec<ElementId> = doc.elements().map(|element| element.id().clone()).collect();
    let rank = |value: &ElementId| pre_order.iter().position(|found| found == value);
    let end_rank = |end: ReferenceEnd| ENDS.iter().position(|found| *found == end);
    let mut compared = 0;
    for element in doc.elements() {
        for reference_policy in [
            ReferenceDeletePolicy::ScrubReferences,
            ReferenceDeletePolicy::DeleteReferencingRelationships,
        ] {
            let delete = DeleteElement::new(element.id())
                .with_reference_policy(reference_policy)
                .with_package_policy(PackageDeletePolicy::DeleteRecursively)
                .with_cross_package_policy(CrossPackageReferencePolicy::ApplyReferencePolicy);
            let what = format!("{delete:?}");
            let preview = match edit::preview(&doc, &Operation::from(delete)) {
                Ok(preview) => preview,
                Err(error) => panic!("{ROW}: {what}: the preview was refused: {error}"),
            };
            let listed: Vec<(ElementId, ReferenceEnd, ElementId)> = preview
                .dangling_references()
                .iter()
                .map(|dangling| {
                    (
                        dangling.holder().clone(),
                        dangling.end(),
                        dangling.id().clone(),
                    )
                })
                .collect();
            // The referrers of every deleted id, held outside the subtree,
            // one per holder, end and id, in pre-order, ALL order and
            // position.
            let subtree: BTreeSet<ElementId> = subtree_of(element).into_iter().collect();
            let mut found: Vec<Ranked> = Vec::new();
            for deleted in preview.mutation().deleted_ids() {
                for referrer in doc.referrers(deleted) {
                    let holder = referrer.holder().id();
                    if subtree.contains(holder) {
                        continue;
                    }
                    found.push((
                        rank(holder),
                        end_rank(referrer.end()),
                        referrer.position(),
                        holder.clone(),
                        referrer.end(),
                        deleted.clone(),
                    ));
                }
            }
            found.sort_by_key(|(holder, end, position, ..)| (*holder, *end, *position));
            let mut expected: Vec<(ElementId, ReferenceEnd, ElementId)> = Vec::new();
            for (_, _, _, holder, end, deleted) in found {
                let key = (holder, end, deleted);
                if !expected.contains(&key) {
                    expected.push(key);
                }
            }
            if !expected.is_empty() {
                compared += 1;
            }
            assert_eq!(
                listed, expected,
                "{ROW}: {what}: the dangling references are not the referrers of the deleted ids \
                 held outside the subtree, one per id and end"
            );
        }
    }
    assert!(
        compared > 10,
        "{ROW}: setup: only {compared} deletes dangle references"
    );
}

// ---------------------------------------------------------------------------
// The type
// ---------------------------------------------------------------------------

fn assert_traits<T: Send + Sync + Copy + Clone + Debug>() {}

#[test]
fn lib_api_014_referrer_is_send_sync_copy_and_debug() {
    assert_traits::<Referrer<'static>>();
    let doc = read("TOP_START", TOP_START.as_bytes());
    let all = doc.referrers("top");
    let Some(&first) = all.first() else {
        panic!("{ROW}: setup: top has no referrer");
    };
    // Copy: used twice, by value.
    let (holder, again) = (first.holder(), first.holder());
    assert_eq!(holder.id(), again.id(), "{ROW}: a copied referrer");
    assert_eq!(
        (first.end(), first.position()),
        (ReferenceEnd::MetaClaim, 0),
        "{ROW}: the first referrer"
    );
    let shown = format!("{first:?}");
    assert!(
        shown.contains("Referrer") && shown.contains('b') && shown.contains("MetaClaim"),
        "{ROW}: the Debug of a referrer shows its holder and end: {shown}"
    );
}
