//! Tests for copying elements from another document (LIB-EDIT-018; ADR 0036
//! D2; ADR 0030 amendment 2; GitHub issue #61).
//!
//! `edit::CopyElements::new(&source, elements, parent)` captures the listed
//! elements of `source`, each with everything it contains, when it is built
//! (`ElementNotFound` for an id the source lacks); `apply` adds the copies
//! under `parent` of the target, or as roots with `root`, after the last
//! child or root, in the order given. An element listed twice, or inside
//! another listed element, is copied once.
//!
//! - **Ids.** An element keeps its id when it is an `NCName` free in the
//!   target: no element, part (name, `content`, `key`, `location`, `value`)
//!   or preserved content holds it, and no reference names it. Otherwise it
//!   gets a generated `<kind>_<n>`, in pre-order, that avoids every id the
//!   copy keeps or names. A part keeps its `xmi:id` when it is an `NCName`
//!   free in the target, and is copied without one otherwise. Preserved content is copied verbatim, so
//!   its ids must be free and its references must not name an id the copy
//!   changes (`DuplicateId`).
//! - **References.** One to a copied element follows its copy; any other is
//!   kept as written. The copy is then checked as a create: the parent, the
//!   tops' kinds, the preserved ids, the required ends and every reference
//!   against the target and the copies, in that order.
//! - **The Mutation** lists the created ids in pre-order, the i-th being the
//!   copy of `source_ids()[i]`, and changes and deletes nothing.
//!
//! Elsewhere: copies join LIB-EDIT-002's set in `edit_atomic.rs`, a batch of
//! a copy and an edit of it is in `batch.rs`, and that serde refuses a copy
//! is in `serde.rs`.

use std::fmt::Debug;

use sacm::compare::{Difference, semantic_compare};
use sacm::diagnostic::Code;
use sacm::edit::{
    self, CopyElements, CreateArgumentPackage, CreateArgumentReasoning, CreateArtifactPackage,
    CreateAssertedInference, CreateAssuranceCasePackage, CreateClaim, CreateDescription,
    DeleteElement, EditError, MoveElement, Mutation, Operation, ReferenceDeletePolicy, SetName,
    SetReferences,
};
use sacm::model::{Document, ElementId, ElementKind, LangString, ReferenceEnd, SacmElement};
use sacm::validation;
use sacm::xmi::{self, Mode, ReadOptions, WriteOptions};

const ROW: &str = "LIB-EDIT-018";

/// The target of most copies: the AssuranceCasePackage root `case` holding
/// the ArgumentPackage `p` (the Claims `c1`, `c2` with the `metaClaim` `c1`,
/// and `c3`), the ArgumentPackage `q` (the Claim `d1`) and the
/// ArtifactPackage `e`, empty. The `MOVE_START` of `move_element.rs`.
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

/// [`MOVE_START`] with a preserved `<v:ref xmi:idref="c1"/>` in `c2`: a
/// reference to `c1` that is not typed.
const PRESERVED_REF_START: &str = r#"<sacm:AssuranceCasePackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301" xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="case">
  <name content="Case"/>
  <argumentPackage xmi:id="p">
    <name content="P"/>
    <argumentElement xmi:type="sacm:Claim" xmi:id="c1"><name content="C1"/></argumentElement>
    <argumentElement xmi:type="sacm:Claim" xmi:id="c2"><name content="C2"/><v:ref xmlns:v="urn:v" xmi:idref="c1"/></argumentElement>
    <argumentElement xmi:type="sacm:Claim" xmi:id="c3"><name content="C3"/></argumentElement>
  </argumentPackage>
  <argumentPackage xmi:id="q">
    <name content="Q"/>
    <argumentElement xmi:type="sacm:Claim" xmi:id="d1"><name content="D1"/></argumentElement>
  </argumentPackage>
</sacm:AssuranceCasePackage>
"#;

/// A source for the refusals, read as written: in the ArgumentPackage `src`,
/// the Claim `top2`; a Claim with the id `q` (an ArgumentPackage in
/// [`MOVE_START`]); `wq`, whose `metaClaim` is `q`; `dang`, whose `metaClaim`
/// is the dangling `ghost`; `lean`, whose `metaClaim` is `top2`; the
/// `AssertedInference` `nosrc` with no source and `notgt` with no target; `inf`
/// from `top2` to the dangling `e` (the ArtifactPackage of [`MOVE_START`]);
/// the `ArgumentReasoning` `ar` whose `structure` is `q`; the Claim `c9`
/// holding a preserved `<v:extra xmi:id="x1"/>`; the nested ArgumentPackage
/// `pk`; and the Claim with the id `a b`, which is no `NCName`, cited by
/// nothing.
const SOURCE: &str = r#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301" xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="src">
  <name content="Src"/>
  <argumentElement xmi:type="sacm:Claim" xmi:id="top2"><name content="Top"/></argumentElement>
  <argumentElement xmi:type="sacm:Claim" xmi:id="q"><name content="A Claim named q"/></argumentElement>
  <argumentElement xmi:type="sacm:Claim" xmi:id="wq" metaClaim="q"><name content="Wq"/></argumentElement>
  <argumentElement xmi:type="sacm:Claim" xmi:id="dang" metaClaim="ghost"><name content="Dang"/></argumentElement>
  <argumentElement xmi:type="sacm:Claim" xmi:id="lean" metaClaim="top2"><name content="Lean"/></argumentElement>
  <argumentElement xmi:type="sacm:AssertedInference" xmi:id="nosrc" target="top2"><name content="No source"/></argumentElement>
  <argumentElement xmi:type="sacm:AssertedInference" xmi:id="notgt" source="top2"><name content="No target"/></argumentElement>
  <argumentElement xmi:type="sacm:AssertedInference" xmi:id="inf" source="top2" target="e"><name content="Inf"/></argumentElement>
  <argumentElement xmi:type="sacm:ArgumentReasoning" xmi:id="ar" structure="q"><name content="Ar"/></argumentElement>
  <argumentElement xmi:type="sacm:Claim" xmi:id="c9"><name content="C9"/><v:extra xmlns:v="urn:v" xmi:id="x1"/></argumentElement>
  <argumentElement xmi:type="sacm:ArgumentPackage" xmi:id="pk"><name content="Pk"/></argumentElement>
  <argumentElement xmi:type="sacm:Claim" xmi:id="a b"><name content="Spaced"/></argumentElement>
</sacm:ArgumentPackage>
"#;

/// A source with an id on every part, and every other feature set: in the
/// ArgumentPackage `args`, the Claim `cl` with a gid, `isAbstract`, the
/// declaration `assumed`, a name with an `xmi:id` and a Description `cl_d`
/// whose `content` and value have ids; and the Claim `nameless`, which has no
/// name. In the ArtifactPackage `arts`, the Resource `res`, whose name and
/// `location` and its value have ids; the Artifact `art` with a `version` and
/// a `date`; and the Activity `act` with a `startTime` and an `endTime`.
const PARTS_SOURCE: &str = r#"<xmi:XMI xmlns:sacm="http://www.omg.org/spec/SACM/20220301" xmlns:xmi="http://www.omg.org/spec/XMI/20131001">
  <sacm:ArgumentPackage xmi:id="args">
    <name content="Args"/>
    <argumentElement xmi:type="sacm:Claim" xmi:id="cl" gid="G-7" isAbstract="true" assertionDeclaration="assumed">
      <name xmi:id="cl_name" lang="en" content="Cl"/>
      <description xmi:id="cl_d"><content xmi:id="cl_d_content"><value xmi:id="cl_d_v" lang="en" content="Why"/></content></description>
    </argumentElement>
    <argumentElement xmi:type="sacm:Claim" xmi:id="nameless"/>
  </sacm:ArgumentPackage>
  <sacm:ArtifactPackage xmi:id="arts">
    <name content="Arts"/>
    <artifactElement xmi:type="sacm:Resource" xmi:id="res">
      <name xmi:id="res_name" content="R"/>
      <location xmi:id="res_loc"><value xmi:id="res_loc_v" lang="en" content="https://example.org/r.pdf"/></location>
    </artifactElement>
    <artifactElement xmi:type="sacm:Artifact" xmi:id="art" version="1.2" date="2024-03-01"><name content="Art"/></artifactElement>
    <artifactElement xmi:type="sacm:Activity" xmi:id="act" startTime="2024-02-12" endTime="2024-02-29T17:30:00Z"><name content="Act"/></artifactElement>
  </sacm:ArtifactPackage>
</xmi:XMI>
"#;

/// A target in which the part ids of [`PARTS_SOURCE`] clash: the Claim
/// `res_name` (an element id); a Claim whose name has the `xmi:id`
/// `cl_d_content`; a Description whose value has the `xmi:id` `res_loc`; and
/// the Claim `holder`, whose `metaClaim` names the dangling `cl_d_v`.
const PARTS_CLASH: &str = r#"<xmi:XMI xmlns:sacm="http://www.omg.org/spec/SACM/20220301" xmlns:xmi="http://www.omg.org/spec/XMI/20131001">
  <sacm:ArgumentPackage xmi:id="targs">
    <name content="Target args"/>
    <argumentElement xmi:type="sacm:Claim" xmi:id="res_name"><name content="Taken"/></argumentElement>
    <argumentElement xmi:type="sacm:Claim" xmi:id="named"><name xmi:id="cl_d_content" content="Named"/></argumentElement>
    <argumentElement xmi:type="sacm:Claim" xmi:id="described">
      <name content="Described"/>
      <description xmi:id="td"><content><value xmi:id="res_loc" content="Taken"/></content></description>
    </argumentElement>
    <argumentElement xmi:type="sacm:Claim" xmi:id="holder" metaClaim="cl_d_v"><name content="Holder"/></argumentElement>
  </sacm:ArgumentPackage>
  <sacm:ArtifactPackage xmi:id="tarts"><name content="Target arts"/></sacm:ArtifactPackage>
</xmi:XMI>
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

fn en(content: &str) -> LangString {
    LangString::new(content).with_lang("en")
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

fn source() -> Document {
    read("SOURCE", SOURCE)
}

/// Applies `operation`, which must be accepted, to `doc`, and asserts that
/// `check` and `preview` agree.
fn apply(what: &str, doc: &mut Document, operation: impl Into<Operation>) -> Mutation {
    let operation = operation.into();
    assert_eq!(
        edit::check(doc, &operation),
        Ok(()),
        "{ROW}: {what}: check refuses {operation:?}"
    );
    let preview = match edit::preview(doc, &operation) {
        Ok(preview) => preview,
        Err(error) => panic!("{ROW}: {what}: the preview of {operation:?} was refused: {error}"),
    };
    match edit::apply(doc, operation.clone()) {
        Ok(mutation) => {
            assert_eq!(
                &mutation,
                preview.mutation(),
                "{ROW}: {what}: preview and apply agree"
            );
            mutation
        }
        Err(error) => panic!("{ROW}: {what}: {operation:?} was refused: {error} ({error:?})"),
    }
}

/// `CopyElements::new`, which must succeed.
fn copy_of(what: &str, source: &Document, elements: &[&str], parent: &str) -> CopyElements {
    match CopyElements::new(source, elements.iter().copied(), parent) {
        Ok(copy) => copy,
        Err(error) => panic!("{ROW}: {what}: CopyElements::new refused {elements:?}: {error}"),
    }
}

/// `CopyElements::root`, which must succeed.
fn root_copy_of(what: &str, source: &Document, elements: &[&str]) -> CopyElements {
    match CopyElements::root(source, elements.iter().copied()) {
        Ok(copy) => copy,
        Err(error) => panic!("{ROW}: {what}: CopyElements::root refused {elements:?}: {error}"),
    }
}

/// The element `value` of `doc`, which must hold it.
fn get<'d>(doc: &'d Document, value: &str) -> SacmElement<'d> {
    match doc.get(value) {
        Some(found) => found,
        None => panic!("{ROW}: the document has no {value:?}"),
    }
}

/// The ids of the children of `parent` in `doc`, in order.
fn children(doc: &Document, parent: &str) -> Vec<String> {
    get(doc, parent)
        .children()
        .map(|child| child.id().as_str().to_owned())
        .collect()
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

/// Refuses `copy` on `doc` with `apply`, `check` and `preview`, which must
/// agree, and asserts that the document is `==` to before, with the same
/// revision and the same next generated ids. Returns the refusal.
fn refused(what: &str, doc: &Document, copy: CopyElements) -> EditError {
    let operation = Operation::from(copy);
    let mut after = doc.clone();
    let revision = after.revision();
    let error = match edit::apply(&mut after, operation.clone()) {
        Ok(mutation) => panic!("{ROW}: {what}: {operation:?} was accepted: {mutation:?}"),
        Err(error) => error,
    };
    assert!(
        after == *doc,
        "{ROW}: {what}: after the refusal {error:?} the target is not == to before"
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

/// The ids every end of `element` holds, through the named accessors.
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

/// The ids `semantic_compare(before, after)` reports right-only, in order,
/// and whether it reports anything else.
fn right_only(before: &Document, after: &Document) -> (Vec<ElementId>, Vec<Difference>) {
    let mut created = Vec::new();
    let mut other = Vec::new();
    for difference in semantic_compare(before, after) {
        match difference {
            Difference::RightOnly { id, .. } => created.push(id),
            difference => other.push(difference),
        }
    }
    (created, other)
}

/// The position of `needle` in `text`, which must hold it.
fn position_of(what: &str, text: &str, needle: &str) -> usize {
    match text.find(needle) {
        Some(found) => found,
        None => panic!("{ROW}: {what}: {needle:?} is not written:\n{text}"),
    }
}

// ---------------------------------------------------------------------------
// Acceptance 1: a draft from a scratch document
// ---------------------------------------------------------------------------

/// The scratch document of acceptance 1: the ArgumentPackage `draft`
/// holding the `ArgumentReasoning` `r1`, the Claims `top2` and `sub2`, and the
/// `AssertedInference` `i1` from `sub2` to `top2` with the reasoning `r1`.
fn draft() -> Document {
    let mut doc = Document::new();
    let what = "the draft";
    apply(
        what,
        &mut doc,
        CreateArgumentPackage::root(en("Draft")).with_id("draft"),
    );
    apply(
        what,
        &mut doc,
        CreateArgumentReasoning::new("draft", en("Argue over each hazard")).with_id("r1"),
    );
    apply(
        what,
        &mut doc,
        CreateClaim::new("draft", en("The brake is safe")).with_id("top2"),
    );
    apply(
        what,
        &mut doc,
        CreateClaim::new("draft", en("Hazard H1 is mitigated")).with_id("sub2"),
    );
    apply(
        what,
        &mut doc,
        CreateAssertedInference::new("draft", en("Inference"), ["sub2"], "top2")
            .with_reasoning("r1")
            .with_id("i1"),
    );
    doc
}

#[test]
fn lib_edit_018_a_draft_of_four_elements_is_copied_under_a_package_with_its_references() {
    let draft = draft();
    let before = move_start();
    let copy = copy_of("the draft", &draft, &["r1", "top2", "sub2", "i1"], "p");
    assert_eq!(
        copy.source_ids(),
        ids(&["r1", "top2", "sub2", "i1"]),
        "{ROW}: source_ids()"
    );
    assert_eq!(copy.parent(), Some(&id("p")), "{ROW}: parent()");
    let mut after = before.clone();
    let mutation = apply("the draft under p", &mut after, copy);

    assert_eq!(
        mutation.created_ids(),
        ids(&["r1", "top2", "sub2", "i1"]),
        "{ROW}: the four free ids are kept, in order"
    );
    assert!(
        mutation.changed_ids().is_empty() && mutation.deleted_ids().is_empty(),
        "{ROW}: a copy changes and deletes nothing: {mutation:?}"
    );
    assert_eq!(
        children(&after, "p"),
        ["c1", "c2", "c3", "r1", "top2", "sub2", "i1"],
        "{ROW}: the four are appended to p in order"
    );
    for value in ["r1", "top2", "sub2", "i1"] {
        let (copied, original) = (get(&after, value), get(&draft, value));
        assert_eq!(
            (copied.kind(), copied.name()),
            (original.kind(), original.name()),
            "{ROW}: the kind and name of {value:?}"
        );
    }
    let inference = get(&after, "i1");
    assert_eq!(
        (
            inference.sources(),
            inference.target(),
            inference.reasoning()
        ),
        (
            ids(&["sub2"]).as_slice(),
            Some(&id("top2")),
            Some(&id("r1"))
        ),
        "{ROW}: the inference's references are intact"
    );
    let (created, other) = right_only(&before, &after);
    assert_eq!(
        created,
        ids(&["r1", "top2", "sub2", "i1"]),
        "{ROW}: the comparison"
    );
    assert_eq!(other, [], "{ROW}: the copy changed an existing element");
    let findings = validation::validate(&before);
    let new: Vec<_> = validation::validate(&after)
        .into_iter()
        .filter(|finding| !findings.contains(finding))
        .collect();
    assert_eq!(
        new,
        [],
        "{ROW}: validate reports something new after the copy"
    );
}

// ---------------------------------------------------------------------------
// Acceptance 2: clashing ids, the mapping, references
// ---------------------------------------------------------------------------

#[test]
fn lib_edit_018_clashing_ids_get_generated_ids_and_internal_references_follow_them() {
    // The target generated claim_1; the source generated claim_1, claim_2
    // and asserted_inference_1, from claim_2 to claim_1.
    let mut target = move_start();
    apply(
        "a generated Claim in the target",
        &mut target,
        CreateClaim::new("p", en("Generated in the target")),
    );
    assert!(target.contains("claim_1"), "{ROW}: setup: claim_1");
    let mut source = Document::new();
    let what = "the source";
    apply(
        what,
        &mut source,
        CreateArgumentPackage::root(en("Draft")).with_id("draft"),
    );
    apply(what, &mut source, CreateClaim::new("draft", en("A")));
    apply(what, &mut source, CreateClaim::new("draft", en("B")));
    apply(
        what,
        &mut source,
        CreateAssertedInference::new("draft", en("I"), ["claim_2"], "claim_1"),
    );
    let copy = copy_of(
        "three generated ids",
        &source,
        &["claim_1", "claim_2", "asserted_inference_1"],
        "q",
    );
    let mut after = target.clone();
    let mutation = apply("the copy under q", &mut after, copy.clone());
    let mapping: Vec<(&str, &str)> = copy
        .source_ids()
        .iter()
        .zip(mutation.created_ids())
        .map(|(from, to)| (from.as_str(), to.as_str()))
        .collect();
    // claim_1 clashes; claim_2 is free and kept, so the generated id avoids
    // it.
    assert_eq!(
        mapping,
        [
            ("claim_1", "claim_3"),
            ("claim_2", "claim_2"),
            ("asserted_inference_1", "asserted_inference_1"),
        ],
        "{ROW}: source_ids() zipped with created_ids() is not the mapping"
    );
    assert_eq!(
        get(&after, "claim_3").name(),
        Some(&en("A")),
        "{ROW}: claim_3 is the copy of the source's claim_1"
    );
    assert_eq!(
        get(&after, "claim_1").name(),
        Some(&en("Generated in the target")),
        "{ROW}: the target's own claim_1 is untouched"
    );
    let inference = get(&after, "asserted_inference_1");
    assert_eq!(
        (inference.sources(), inference.target()),
        (ids(&["claim_2"]).as_slice(), Some(&id("claim_3"))),
        "{ROW}: the inference's references follow the new ids"
    );
    assert_eq!(
        children(&after, "q"),
        ["d1", "claim_3", "claim_2", "asserted_inference_1"],
        "{ROW}: the copies are appended to q in pre-order"
    );

    // The generated id moved the Claim counter: with claim_3 deleted, the
    // next Claim is claim_4, not claim_3.
    let mut freed = after.clone();
    apply(
        "delete claim_3",
        &mut freed,
        DeleteElement::new("claim_3")
            .with_reference_policy(ReferenceDeletePolicy::DeleteReferencingRelationships),
    );
    let next = apply(
        "a generated Claim",
        &mut freed,
        CreateClaim::new("q", en("Next")),
    );
    assert_eq!(
        next.created_ids(),
        ids(&["claim_4"]),
        "{ROW}: the generated id did not advance the counter as a create's does"
    );
    // A kept id does not move its counter: with asserted_inference_1 deleted,
    // the next AssertedInference is asserted_inference_1 again.
    let mut freed = after.clone();
    apply(
        "delete asserted_inference_1",
        &mut freed,
        DeleteElement::new("asserted_inference_1"),
    );
    let next = apply(
        "a generated AssertedInference",
        &mut freed,
        CreateAssertedInference::new("q", en("Next"), ["claim_2"], "claim_2"),
    );
    assert_eq!(
        next.created_ids(),
        ids(&["asserted_inference_1"]),
        "{ROW}: a kept id moved the counter"
    );
}

#[test]
fn lib_edit_018_a_copy_within_one_document_duplicates_the_elements() {
    let before = move_start();
    let copy = copy_of("p within MOVE_START", &before, &["p"], "case");
    let mut after = before.clone();
    let mutation = apply("p copied into case", &mut after, copy.clone());
    assert_eq!(
        copy.source_ids(),
        ids(&["p", "c1", "c2", "c3"]),
        "{ROW}: source_ids() is the subtree in pre-order"
    );
    assert_eq!(
        mutation.created_ids(),
        ids(&["argument_package_1", "claim_1", "claim_2", "claim_3"]),
        "{ROW}: every id clashes, so every copy gets a generated id, in pre-order"
    );
    assert_eq!(
        children(&after, "case"),
        ["p", "q", "e", "argument_package_1"],
        "{ROW}: the copy is appended to case"
    );
    assert_eq!(
        children(&after, "argument_package_1"),
        ["claim_1", "claim_2", "claim_3"],
        "{ROW}: the copy holds the copies of c1, c2 and c3"
    );
    for (original, copied) in [
        ("p", "argument_package_1"),
        ("c1", "claim_1"),
        ("c2", "claim_2"),
        ("c3", "claim_3"),
    ] {
        assert_eq!(
            (get(&after, copied).kind(), get(&after, copied).name()),
            (get(&before, original).kind(), get(&before, original).name()),
            "{ROW}: {copied:?} is a copy of {original:?}"
        );
    }
    assert_eq!(
        get(&after, "claim_2").meta_claims(),
        ids(&["claim_1"]),
        "{ROW}: the copy of c2 names the copy of c1"
    );
    assert_eq!(
        get(&after, "c2").meta_claims(),
        ids(&["c1"]),
        "{ROW}: the original c2 still names c1"
    );
    let (created, other) = right_only(&before, &after);
    assert_eq!(created, mutation.created_ids(), "{ROW}: the comparison");
    assert_eq!(other, [], "{ROW}: the copy changed an existing element");
}

#[test]
fn lib_edit_018_references_leaving_the_copy_are_kept_as_written() {
    // Within one document: c2 alone, whose metaClaim c1 is not copied.
    let before = move_start();
    let mut after = before.clone();
    let mutation = apply(
        "c2 within MOVE_START",
        &mut after,
        copy_of("c2", &before, &["c2"], "q"),
    );
    assert_eq!(
        mutation.created_ids(),
        ids(&["claim_1"]),
        "{ROW}: c2 clashes"
    );
    assert_eq!(
        get(&after, "claim_1").meta_claims(),
        ids(&["c1"]),
        "{ROW}: a reference leaving the copy is kept as written"
    );

    // From a scratch document: x names its c1, which the target also has.
    let mut scratch = Document::new();
    let what = "the scratch document";
    apply(
        what,
        &mut scratch,
        CreateArgumentPackage::root(en("S")).with_id("s"),
    );
    apply(
        what,
        &mut scratch,
        CreateClaim::new("s", en("Its c1")).with_id("c1"),
    );
    apply(
        what,
        &mut scratch,
        CreateClaim::new("s", en("X")).with_id("x"),
    );
    apply(
        what,
        &mut scratch,
        SetReferences::new("x", ReferenceEnd::MetaClaim, ["c1"]),
    );
    let mut alone = before.clone();
    let mutation = apply("x alone", &mut alone, copy_of("x", &scratch, &["x"], "q"));
    assert_eq!(mutation.created_ids(), ids(&["x"]), "{ROW}: x is free");
    assert_eq!(
        get(&alone, "x").meta_claims(),
        ids(&["c1"]),
        "{ROW}: x's metaClaim leaves the copy, so it names the target's c1"
    );
    // Control: with its c1 copied too, x names the copy of its c1.
    let mut both = before.clone();
    let mutation = apply(
        "c1 and x",
        &mut both,
        copy_of("c1 and x", &scratch, &["c1", "x"], "q"),
    );
    assert_eq!(
        mutation.created_ids(),
        ids(&["claim_1", "x"]),
        "{ROW}: c1 clashes, x is free"
    );
    assert_eq!(
        get(&both, "x").meta_claims(),
        ids(&["claim_1"]),
        "{ROW}: with c1 copied, x names its copy"
    );
}

#[test]
fn lib_edit_018_source_ids_pair_with_created_ids_in_pre_order() {
    let before = move_start();
    // Listed against document order: q after p in MOVE_START.
    let copy = copy_of("q then p", &before, &["q", "p"], "case");
    assert_eq!(
        copy.source_ids(),
        ids(&["q", "d1", "p", "c1", "c2", "c3"]),
        "{ROW}: source_ids() is the tops in list order, each subtree in pre-order"
    );
    let mut after = before.clone();
    let mutation = apply("q then p into case", &mut after, copy.clone());
    assert_eq!(
        mutation.created_ids(),
        ids(&[
            "argument_package_1",
            "claim_1",
            "argument_package_2",
            "claim_2",
            "claim_3",
            "claim_4"
        ]),
        "{ROW}: the generated ids, in pre-order"
    );
    assert_eq!(
        copy.source_ids().len(),
        mutation.created_ids().len(),
        "{ROW}: one created id per source id"
    );
    let created_in_document_order: Vec<ElementId> = after
        .elements()
        .map(|element| element.id().clone())
        .filter(|element| mutation.created_ids().contains(element))
        .collect();
    assert_eq!(
        created_in_document_order,
        mutation.created_ids(),
        "{ROW}: created_ids() is in the target's pre-order"
    );
    let mapped = |source: &ElementId| -> ElementId {
        match copy.source_ids().iter().position(|value| value == source) {
            Some(index) => mutation.created_ids()[index].clone(),
            None => source.clone(),
        }
    };
    for (source_id, created_id) in copy.source_ids().iter().zip(mutation.created_ids()) {
        let (original, copied) = (
            get(&before, source_id.as_str()),
            get(&after, created_id.as_str()),
        );
        assert_eq!(
            (copied.kind(), copied.name()),
            (original.kind(), original.name()),
            "{ROW}: {created_id:?} is the copy of {source_id:?}"
        );
        let expected_parent = match original.parent() {
            Some(parent) if copy.source_ids().contains(parent.id()) => mapped(parent.id()),
            _ => id("case"),
        };
        assert_eq!(
            copied.parent().map(SacmElement::id),
            Some(&expected_parent),
            "{ROW}: the parent of {created_id:?}"
        );
        let expected_references: Vec<Vec<ElementId>> = held_ids(original)
            .iter()
            .map(|end| end.iter().map(mapped).collect())
            .collect();
        assert_eq!(
            held_ids(copied),
            expected_references,
            "{ROW}: the references of {created_id:?}"
        );
    }

    // As roots, after the last root.
    let mut rooted = before.clone();
    let mutation = apply(
        "q as a root",
        &mut rooted,
        root_copy_of("q", &before, &["q"]),
    );
    assert_eq!(
        mutation.created_ids(),
        ids(&["argument_package_1", "claim_1"]),
        "{ROW}: a root copy"
    );
    let roots: Vec<&str> = rooted.roots().map(|root| root.id().as_str()).collect();
    assert_eq!(
        roots,
        ["case", "argument_package_1"],
        "{ROW}: appended as the last root"
    );
}

#[test]
fn lib_edit_018_an_element_listed_twice_or_inside_another_is_copied_once() {
    let before = move_start();
    let copy = copy_of("c1, p, c1", &before, &["c1", "p", "c1"], "case");
    assert_eq!(
        copy.source_ids(),
        ids(&["p", "c1", "c2", "c3"]),
        "{ROW}: c1 is inside p and listed twice: copied once, as part of p"
    );
    let mut after = before.clone();
    let mutation = apply("c1, p, c1 into case", &mut after, copy);
    assert_eq!(
        mutation.created_ids().len(),
        4,
        "{ROW}: four elements created: {:?}",
        mutation.created_ids()
    );
    assert_eq!(
        after.element_count(),
        before.element_count() + 4,
        "{ROW}: the element count"
    );

    let twice = copy_of("c1 twice", &before, &["c1", "c1"], "q");
    assert_eq!(twice.source_ids(), ids(&["c1"]), "{ROW}: c1 listed twice");
    let mut after = before.clone();
    let mutation = apply("c1 twice into q", &mut after, twice);
    assert_eq!(mutation.created_ids(), ids(&["claim_1"]), "{ROW}: one copy");

    let order = copy_of("c2, c1", &before, &["c2", "c1"], "q");
    assert_eq!(
        order.source_ids(),
        ids(&["c2", "c1"]),
        "{ROW}: siblings are copied in the order given"
    );

    // An empty list copies nothing.
    let empty = copy_of("nothing", &before, &[], "q");
    assert_eq!(
        empty.source_ids(),
        [] as [ElementId; 0],
        "{ROW}: no source ids"
    );
    let mut after = before.clone();
    let mutation = apply("nothing into q", &mut after, empty);
    assert!(
        mutation.created_ids().is_empty()
            && mutation.changed_ids().is_empty()
            && mutation.deleted_ids().is_empty(),
        "{ROW}: an empty copy reports ids: {mutation:?}"
    );
    assert!(after == before, "{ROW}: an empty copy changed the document");
}

// ---------------------------------------------------------------------------
// Acceptance 3: part ids and preserved content
// ---------------------------------------------------------------------------

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the free and the clashing target, and the features, read best as one list"
)]
fn lib_edit_018_part_ids_are_kept_when_free_and_dropped_when_not() {
    let source = read("PARTS_SOURCE", PARTS_SOURCE);
    let copy_all = |what: &str, target: &Document, args: &str, arts: &str| -> Document {
        let mut after = target.clone();
        apply(
            what,
            &mut after,
            copy_of(what, &source, &["cl", "nameless"], args),
        );
        apply(
            what,
            &mut after,
            copy_of(what, &source, &["res", "art", "act"], arts),
        );
        after
    };

    // Into a target where every id is free: every part id is kept.
    let mut free = Document::new();
    apply(
        "the free target",
        &mut free,
        CreateArgumentPackage::root(en("A")).with_id("targs"),
    );
    apply(
        "the free target",
        &mut free,
        CreateArtifactPackage::root(en("B")).with_id("tarts"),
    );
    let after = copy_all("into the free target", &free, "targs", "tarts");
    let (cl, d, res) = (get(&after, "cl"), get(&after, "cl_d"), get(&after, "res"));
    assert_eq!(
        cl.name_id(),
        Some(&id("cl_name")),
        "{ROW}: a free name id is kept"
    );
    assert_eq!(
        (d.content_id(), d.content_value_ids()),
        (Some(&id("cl_d_content")), [Some(id("cl_d_v"))].as_slice()),
        "{ROW}: free content and value ids are kept"
    );
    assert_eq!(
        res.name_id(),
        Some(&id("res_name")),
        "{ROW}: a free name id is kept"
    );
    assert_eq!(
        (res.location_id(), res.location_value_ids()),
        (Some(&id("res_loc")), [Some(id("res_loc_v"))].as_slice()),
        "{ROW}: free location and value ids are kept"
    );

    // Into a target where res_name, cl_d_content, res_loc and cl_d_v are
    // taken: those parts are copied without an id, the others keep theirs.
    let clash = read("PARTS_CLASH", PARTS_CLASH);
    let after = copy_all("into the clashing target", &clash, "targs", "tarts");
    let (cl, d, res) = (get(&after, "cl"), get(&after, "cl_d"), get(&after, "res"));
    assert_eq!(
        cl.name_id(),
        Some(&id("cl_name")),
        "{ROW}: a free name id is kept"
    );
    assert_eq!(
        (d.content_id(), d.content_value_ids()),
        (None, [None].as_slice()),
        "{ROW}: a content id held by a name and a value id a reference names are dropped"
    );
    assert_eq!(
        res.name_id(),
        None,
        "{ROW}: a name id that is an element's id is dropped"
    );
    assert_eq!(
        (res.location_id(), res.location_value_ids()),
        (None, [Some(id("res_loc_v"))].as_slice()),
        "{ROW}: a location id held by a value is dropped, a free value id kept"
    );
    // The target's own holders are untouched.
    assert_eq!(
        get(&after, "named").name_id(),
        Some(&id("cl_d_content")),
        "{ROW}: the target's name id"
    );
    assert_eq!(
        get(&after, "holder").meta_claims(),
        ids(&["cl_d_v"]),
        "{ROW}: the target's reference"
    );

    // Everything else is copied exactly, into either target.
    for (what, target) in [("free", &free), ("clashing", &clash)] {
        let after = copy_all(what, target, "targs", "tarts");
        for value in ["cl", "cl_d", "nameless", "res", "art", "act"] {
            let (copied, original) = (get(&after, value), get(&source, value));
            assert_eq!(
                (
                    copied.kind(),
                    copied.name(),
                    copied.gid(),
                    copied.is_citation(),
                    copied.is_abstract(),
                    copied.assertion_declaration(),
                ),
                (
                    original.kind(),
                    original.name(),
                    original.gid(),
                    original.is_citation(),
                    original.is_abstract(),
                    original.assertion_declaration(),
                ),
                "{ROW}: {what}: the features of {value:?}"
            );
            assert_eq!(
                (copied.content(), copied.location()),
                (original.content(), original.location()),
                "{ROW}: {what}: the content and location of {value:?}"
            );
            assert_eq!(
                (
                    copied.version(),
                    copied.date(),
                    copied.start_time(),
                    copied.end_time()
                ),
                (
                    original.version(),
                    original.date(),
                    original.start_time(),
                    original.end_time()
                ),
                "{ROW}: {what}: the strings of {value:?}"
            );
        }
        assert_eq!(
            get(&after, "nameless").name(),
            None,
            "{ROW}: {what}: a missing name stays missing"
        );
        // Every xmi:id is still unique: the write reads back equal. (Tolerant:
        // the clashing target holds a dangling reference, which a strict
        // write refuses.)
        let text = match xmi::write(&after, &WriteOptions::default().with_mode(Mode::Tolerant)) {
            Ok(text) => text,
            Err(error) => panic!("{ROW}: {what}: the write was refused: {error}"),
        };
        assert!(
            read(what, &text) == after,
            "{ROW}: {what}: the copy does not read back =="
        );
    }
}

#[test]
fn lib_edit_018_preserved_content_is_copied_exactly_or_refused_as_duplicate_id() {
    // Into another document: copied, written back, and its id joins the
    // target.
    let source = source();
    let before = move_start();
    let mut after = before.clone();
    let mutation = apply(
        "c9 into MOVE_START",
        &mut after,
        copy_of("c9", &source, &["c9"], "p"),
    );
    assert_eq!(mutation.created_ids(), ids(&["c9"]), "{ROW}: c9 is free");
    assert!(
        get(&after, "c9").has_preserved_content(),
        "{ROW}: the copy of c9 lost its preserved content"
    );
    let text = match xmi::write(&after, &WriteOptions::default().with_mode(Mode::Tolerant)) {
        Ok(text) => text,
        Err(error) => panic!("{ROW}: the tolerant write was refused: {error}"),
    };
    // The copy's preserved content is written as the source's is.
    let fragment = |what: &str, text: &str| -> String {
        let start = position_of(what, text, "xmi:id=\"c9\"");
        let from = start + position_of(what, &text[start..], "<v:extra");
        let to = from + position_of(what, &text[from..], "/>") + 2;
        text[from..to].to_owned()
    };
    let written_source =
        match xmi::write(&source, &WriteOptions::default().with_mode(Mode::Tolerant)) {
            Ok(text) => text,
            Err(error) => panic!("{ROW}: setup: the source's tolerant write was refused: {error}"),
        };
    let copied = fragment("the copy", &text);
    assert_eq!(
        copied,
        fragment("the source", &written_source),
        "{ROW}: the preserved content is not written in the copy of c9 as in the source"
    );
    assert!(
        copied.contains("xmi:id=\"x1\""),
        "{ROW}: the preserved id is not copied verbatim: {copied}"
    );
    assert!(
        read("the tolerant write", &text) == after,
        "{ROW}: the copy with preserved content does not read back =="
    );
    assert!(
        xmi::write(&before, &WriteOptions::default()).is_ok(),
        "{ROW}: setup: a strict write refuses MOVE_START"
    );
    assert!(
        xmi::write(&after, &WriteOptions::default()).is_err(),
        "{ROW}: a strict write accepts preserved content copied in"
    );
    let taken = edit::apply(
        &mut after.clone(),
        CreateClaim::new("p", en("Taken")).with_id("x1"),
    );
    assert!(
        matches!(&taken, Err(EditError::DuplicateId { id, .. }) if id.as_str() == "x1"),
        "{ROW}: the preserved id x1 did not join the target: {taken:?}"
    );

    // Within one document: x1 is in use.
    let error = refused(
        "c9 within its own document",
        &source,
        copy_of("c9", &source, &["c9"], "src"),
    );
    assert!(
        matches!(&error, EditError::DuplicateId { id, .. } if id.as_str() == "x1"),
        "{ROW}: a preserved id in use is refused as {error:?}, not DuplicateId of x1"
    );
    assert_eq!(error.code(), Code::CMD_DUPLICATE_ID, "{ROW}: the code");

    // A preserved reference that names an id the copy changes: c2's
    // <v:ref xmi:idref="c1"/>, with c1 copied within one document.
    let doc = read("PRESERVED_REF_START", PRESERVED_REF_START);
    let error = refused(
        "c1 and c2 within one document",
        &doc,
        copy_of("c1 and c2", &doc, &["c1", "c2"], "q"),
    );
    assert!(
        matches!(&error, EditError::DuplicateId { id, .. } if id.as_str() == "c1"),
        "{ROW}: a preserved reference to a changed id is refused as {error:?}, not DuplicateId of c1"
    );
    // Control: c2 alone; the c1 it names is not copied, so not changed.
    let mut alone = doc.clone();
    let mutation = apply("c2 alone", &mut alone, copy_of("c2", &doc, &["c2"], "q"));
    assert_eq!(
        mutation.created_ids(),
        ids(&["claim_1"]),
        "{ROW}: c2 clashes"
    );
    assert!(
        get(&alone, "claim_1").has_preserved_content(),
        "{ROW}: the copy of c2 holds its preserved reference"
    );
}

// ---------------------------------------------------------------------------
// Acceptance 4: refusals, in order
// ---------------------------------------------------------------------------

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one case per check and pair of checks reads best as one list"
)]
fn lib_edit_018_refusals_come_in_order_and_change_nothing() {
    let source = source();
    let doc = move_start();

    // At construction: the first id, in order, that the source lacks. A
    // name's id is no element.
    for (elements, missing) in [
        (vec!["top2", "nowhere", "elsewhere"], "nowhere"),
        (vec!["elsewhere", "nowhere"], "elsewhere"),
    ] {
        for built in [
            CopyElements::new(&source, elements.clone(), "p"),
            CopyElements::root(&source, elements.clone()),
        ] {
            assert!(
                matches!(&built, Err(EditError::ElementNotFound { element, .. }) if element.as_str() == missing),
                "{ROW}: CopyElements of {elements:?} is {built:?}, not ElementNotFound of {missing:?}"
            );
        }
    }
    let named = read(
        "a document with a name id",
        MOVE_START
            .replace(
                "<name content=\"C1\"/>",
                "<name xmi:id=\"c1_name\" content=\"C1\"/>",
            )
            .as_str(),
    );
    let built = CopyElements::new(&named, ["c1_name"], "q");
    assert!(
        matches!(&built, Err(EditError::ElementNotFound { element, .. }) if element.as_str() == "c1_name"),
        "{ROW}: a name's id is copied: {built:?}"
    );

    // 1. The parent is in the target; first even before a missing source.
    for elements in [vec!["top2"], vec!["nosrc"]] {
        let error = refused(
            "an unknown parent",
            &doc,
            copy_of("unknown parent", &source, &elements, "nowhere"),
        );
        assert!(
            matches!(&error, EditError::ParentNotFound { parent, .. } if parent.as_str() == "nowhere"),
            "{ROW}: {elements:?} under nowhere is refused as {error:?}, not ParentNotFound"
        );
    }
    // The parent is checked before an empty list changes nothing.
    let error = refused(
        "an empty copy under an unknown parent",
        &doc,
        copy_of("nothing", &source, &[], "nowhere"),
    );
    assert!(
        matches!(&error, EditError::ParentNotFound { parent, .. } if parent.as_str() == "nowhere"),
        "{ROW}: an empty copy under nowhere is refused as {error:?}, not ParentNotFound"
    );

    // 2. Each top, in list order, can be held by the parent, or is a package
    //    for the roots; first even before a preserved id in use.
    let mut with_x1 = doc.clone();
    apply(
        "x1 in the target",
        &mut with_x1,
        CreateClaim::new("q", en("X1")).with_id("x1"),
    );
    for (target, elements) in [
        (&doc, vec!["top2"]),
        (&doc, vec!["pk", "top2"]),
        (&with_x1, vec!["c9"]),
    ] {
        let error = refused(
            "an illegal parent",
            target,
            copy_of("illegal parent", &source, &elements, "case"),
        );
        assert!(
            matches!(
                &error,
                EditError::InvalidParent { parent, parent_kind: ElementKind::AssuranceCasePackage, kind: ElementKind::Claim, .. }
                    if parent.as_str() == "case"
            ),
            "{ROW}: {elements:?} under case is refused as {error:?}, not InvalidParent of a Claim"
        );
        assert_eq!(error.code(), Code::CMD_INVALID_PARENT, "{ROW}: the code");
    }
    for elements in [vec!["top2"], vec!["pk", "lean"]] {
        let error = refused(
            "a root that is no package",
            &doc,
            root_copy_of("illegal root", &source, &elements),
        );
        let expected = elements[elements.len() - 1];
        assert!(
            matches!(
                &error,
                EditError::InvalidRoot { element, kind: ElementKind::Claim, .. } if element.as_str() == expected
            ),
            "{ROW}: {elements:?} as roots is refused as {error:?}, not InvalidRoot of the source id {expected:?}"
        );
        assert_eq!(error.code(), Code::CMD_INVALID_PARENT, "{ROW}: the code");
    }

    // 3. Preserved ids are free; first even before a missing source, which
    //    comes earlier in the list.
    for elements in [vec!["c9"], vec!["nosrc", "c9"]] {
        let error = refused(
            "a preserved id in use",
            &with_x1,
            copy_of("x1 in use", &source, &elements, "q"),
        );
        assert!(
            matches!(&error, EditError::DuplicateId { id, .. } if id.as_str() == "x1"),
            "{ROW}: {elements:?} is refused as {error:?}, not DuplicateId of x1"
        );
        assert_eq!(error.code(), Code::CMD_DUPLICATE_ID, "{ROW}: the code");
    }

    // 4. Required ends: a source and a target; first even before a
    //    reference naming nothing.
    for (elements, end) in [
        (vec!["nosrc", "top2"], "source"),
        (vec!["notgt", "top2"], "target"),
        (vec!["nosrc"], "source"),
        (vec!["dang", "nosrc"], "source"),
    ] {
        let error = refused(
            "a relationship without an end",
            &doc,
            copy_of("missing end", &source, &elements, "q"),
        );
        assert!(
            matches!(
                &error,
                EditError::TooFewReferences { end: e, given: 0, required: 1, .. } if *e == end
            ),
            "{ROW}: {elements:?} is refused as {error:?}, not TooFewReferences of the {end}"
        );
        assert_eq!(
            error.code(),
            Code::MULTIPLICITY_VIOLATION,
            "{ROW}: the code"
        );
    }

    // 6. A reference naming nothing in the target or the copies; in its own
    //    pass, so before a wrongly typed id listed earlier.
    for (elements, end, missing) in [
        (vec!["dang"], "metaClaim", "ghost"),
        (vec!["lean"], "metaClaim", "top2"),
        (vec!["wq", "dang"], "metaClaim", "ghost"),
    ] {
        let error = refused(
            "a reference naming nothing",
            &doc,
            copy_of("naming nothing", &source, &elements, "q"),
        );
        assert!(
            matches!(
                &error,
                EditError::ReferenceNotFound { end: e, id, .. } if *e == end && id.as_str() == missing
            ),
            "{ROW}: {elements:?} is refused as {error:?}, not ReferenceNotFound of {missing:?}"
        );
        assert_eq!(error.code(), Code::CMD_TARGET_NOT_FOUND, "{ROW}: the code");
    }

    // 7. A reference naming an element of a kind its end does not admit, in
    //    the target: q is an ArgumentPackage there, not a Claim.
    let error = refused(
        "a wrongly typed reference",
        &doc,
        copy_of("wrong type", &source, &["wq"], "q"),
    );
    assert!(
        matches!(
            &error,
            EditError::WrongReferenceType { end: "metaClaim", id, kind: ElementKind::ArgumentPackage, expected: "Claim", .. }
                if id.as_str() == "q"
        ),
        "{ROW}: wq is refused as {error:?}, not WrongReferenceType of q"
    );
    assert_eq!(error.code(), Code::REF_WRONG_TYPE, "{ROW}: the code");

    // Control: lean with top2 copied too names top2's copy, and is accepted.
    let mut accepted = doc.clone();
    let mutation = apply(
        "lean with top2",
        &mut accepted,
        copy_of("lean and top2", &source, &["lean", "top2"], "q"),
    );
    assert_eq!(
        mutation.created_ids(),
        ids(&["lean", "top2"]),
        "{ROW}: both free"
    );
}

#[test]
fn lib_edit_018_unresolvable_references_are_refused_as_a_create_would_be() {
    let source = source();
    let doc = move_start();

    // Not found: the target lacks ghost, so a create naming it is refused,
    // and so is the copy.
    let create = edit::apply(
        &mut doc.clone(),
        CreateAssertedInference::new("q", en("I"), ["d1"], "ghost"),
    );
    assert!(
        matches!(&create, Err(EditError::ReferenceNotFound { end: "target", id, .. }) if id.as_str() == "ghost"),
        "{ROW}: setup: a create naming ghost is {create:?}"
    );
    let error = refused("dang", &doc, copy_of("dang", &source, &["dang"], "q"));
    assert!(
        matches!(&error, EditError::ReferenceNotFound { end: "metaClaim", id, .. } if id.as_str() == "ghost"),
        "{ROW}: a copy naming ghost is refused as {error:?}"
    );

    // Wrong type, in the target: inf's target e is an ArtifactPackage
    // there, and a source or target must be an ArgumentAsset.
    let error = refused(
        "inf and top2",
        &doc,
        copy_of("inf", &source, &["inf", "top2"], "q"),
    );
    assert!(
        matches!(
            &error,
            EditError::WrongReferenceType { end: "target", id, kind: ElementKind::ArtifactPackage, expected: "ArgumentAsset", .. }
                if id.as_str() == "e"
        ),
        "{ROW}: inf's target e is refused as {error:?}"
    );
    // A create of the same is refused the same way.
    let create = edit::apply(
        &mut doc.clone(),
        CreateAssertedInference::new("q", en("I"), ["d1"], "e"),
    );
    assert!(
        matches!(
            &create,
            Err(EditError::WrongReferenceType {
                end: "target",
                kind: ElementKind::ArtifactPackage,
                ..
            })
        ),
        "{ROW}: setup: a create naming e is {create:?}"
    );

    // Checked against the target: ar's structure q is a Claim in the source,
    // and an ArgumentPackage in the target, which a structure admits.
    let mut after = doc.clone();
    let mutation = apply("ar", &mut after, copy_of("ar", &source, &["ar"], "q"));
    assert_eq!(mutation.created_ids(), ids(&["ar"]), "{ROW}: ar is free");
    assert_eq!(
        get(&after, "ar").structure(),
        Some(&id("q")),
        "{ROW}: ar's structure"
    );

    // White space: an id such as `a b` cannot be held in an end, so a copy
    // never holds one as written. A copied element whose own id is `a b`
    // gets a generated id, and nothing names `a b`.
    let mut after = doc.clone();
    let mutation = apply("a b", &mut after, copy_of("a b", &source, &["a b"], "q"));
    assert_eq!(
        mutation.created_ids(),
        ids(&["claim_1"]),
        "{ROW}: an id that is no NCName is never kept"
    );
    assert!(!after.contains("a b"), "{ROW}: the id `a b` was kept");
}

#[test]
fn lib_edit_018_generated_ids_avoid_the_ids_the_copy_keeps_and_names() {
    // The target generated claim_1.
    let mut target = move_start();
    apply(
        "claim_1 in the target",
        &mut target,
        CreateClaim::new("p", en("Target")),
    );
    // The source: claim_1 (clashes), claim_2 (free, kept) and k, whose
    // preserved content names claim_3, which nothing holds.
    let source = read(
        "the source",
        r#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301" xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="src">
  <name content="Src"/>
  <argumentElement xmi:type="sacm:Claim" xmi:id="claim_1"><name content="One"/></argumentElement>
  <argumentElement xmi:type="sacm:Claim" xmi:id="claim_2"><name content="Two"/></argumentElement>
  <argumentElement xmi:type="sacm:Claim" xmi:id="k"><name content="K"/><v:ref xmlns:v="urn:v" xmi:idref="claim_3"/></argumentElement>
</sacm:ArgumentPackage>
"#,
    );
    let copy = copy_of("three", &source, &["claim_1", "claim_2", "k"], "q");
    let mut after = target.clone();
    let mutation = apply("the copy", &mut after, copy);
    assert_eq!(
        mutation.created_ids(),
        ids(&["claim_4", "claim_2", "k"]),
        "{ROW}: claim_1's copy avoids claim_2, which the copy keeps, and claim_3, which it names"
    );
    assert!(!after.contains("claim_3"), "{ROW}: claim_3 was created");
    // The generated id moved the counter to 4, as a create's does.
    let next = apply(
        "the next Claim",
        &mut after,
        CreateClaim::new("q", en("Next")),
    );
    assert_eq!(
        next.created_ids(),
        ids(&["claim_5"]),
        "{ROW}: the next Claim"
    );
}

#[test]
fn lib_edit_018_a_generated_id_skips_every_taken_candidate() {
    // Each target holds the Claim `h` in the ArgumentPackage `p`, and the
    // source a Claim `h` too, so `h`'s copy gets a generated id. The
    // candidates claim_1, claim_2, ... are taken in turn by an element, by
    // dangling references of the target, or by ids the copy keeps.
    let package = |body: &str| {
        format!(
            r#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301" xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="p"><name content="P"/>{body}</sacm:ArgumentPackage>"#
        )
    };
    let claim = |id: &str, extra: &str| {
        format!(
            r#"<argumentElement xmi:type="sacm:Claim" xmi:id="{id}"{extra}><name content="{id}"/></argumentElement>"#
        )
    };
    let cases = [
        (
            "an element holds claim_1, and nothing is referenced",
            package(&[claim("claim_1", ""), claim("h", "")].concat()),
            package(&claim("h", "")),
            "claim_2",
        ),
        (
            "dangling references hold claim_1 to claim_4",
            package(&claim("h", r#" metaClaim="claim_1 claim_2 claim_3 claim_4""#)),
            package(&claim("h", "")),
            "claim_5",
        ),
        (
            "the copy keeps the preserved ids claim_1 to claim_3",
            package(&claim("h", "")),
            package(
                &[
                    claim("h", ""),
                    r#"<argumentElement xmi:type="sacm:Claim" xmi:id="k"><name content="K"/><v:a xmlns:v="urn:v" xmi:id="claim_1"/><v:b xmlns:v="urn:v" xmi:id="claim_2"/><v:c xmlns:v="urn:v" xmi:id="claim_3"/></argumentElement>"#.to_owned(),
                ]
                .concat(),
            ),
            "claim_4",
        ),
    ];
    for (what, target, source, expected) in cases {
        let target = read(what, &target);
        let source = read(what, &source);
        let listed: Vec<&str> = source
            .elements()
            .skip(1)
            .map(|element| element.id().as_str())
            .collect();
        let mut after = target.clone();
        let mutation = apply(what, &mut after, copy_of(what, &source, &listed, "p"));
        assert_eq!(
            mutation.created_ids().first(),
            Some(&id(expected)),
            "{ROW}: {what}: the copy of h does not get the first free candidate"
        );
        let text = match xmi::write(&after, &WriteOptions::default().with_mode(Mode::Tolerant)) {
            Ok(text) => text,
            Err(error) => panic!("{ROW}: {what}: the write was refused: {error}"),
        };
        assert!(
            read(what, &text) == after,
            "{ROW}: {what}: the copy does not read back =="
        );
    }
}

#[test]
fn lib_edit_018_a_copy_moved_to_the_place_it_holds_changes_nothing() {
    // A copy is added after the last child or root: a move there is the
    // place it holds, and changes nothing.
    let before = move_start();
    let mut doc = before.clone();
    let copied = apply(
        "c1 copied into q",
        &mut doc,
        copy_of("c1", &before, &["c1"], "q"),
    );
    let copy_of_c1 = copied.created_ids()[0].clone();
    let rooted = apply(
        "p copied as a root",
        &mut doc,
        root_copy_of("p", &before, &["p"]),
    );
    let copy_of_p = rooted.created_ids()[0].clone();
    for (what, operation) in [
        (
            "the copy of c1, last in q",
            MoveElement::new(&copy_of_c1, "q"),
        ),
        (
            "the copy of c1, at 1 in q",
            MoveElement::new(&copy_of_c1, "q").with_position(1),
        ),
        (
            "the copy of p, the last root",
            MoveElement::root(&copy_of_p),
        ),
        (
            "a child of the copy of p, last",
            MoveElement::new("claim_4", &copy_of_p),
        ),
    ] {
        let mut moved = doc.clone();
        let mutation = apply(what, &mut moved, operation);
        assert_eq!(
            mutation.changed_ids(),
            [] as [ElementId; 0],
            "{ROW}: {what}: a move of a copy to the place it holds reports a change"
        );
        assert!(moved == doc, "{ROW}: {what}: the move changed the document");
    }
    // Control: to the first place, it is a change.
    let mut moved = doc.clone();
    let mutation = apply(
        "first in q",
        &mut moved,
        MoveElement::new(&copy_of_c1, "q").with_position(0),
    );
    assert_eq!(
        mutation.changed_ids(),
        ids(&["q"]),
        "{ROW}: a move of the copy within q"
    );
}

#[test]
fn lib_edit_018_a_generated_id_avoids_a_part_id_and_a_preserved_id_the_copy_keeps() {
    // The target generated claim_1, so the source's claim_1 clashes. The
    // copy keeps the source's free part id claim_2 (the `xmi:id` of k's
    // name) and its preserved id claim_3, so claim_1's copy is claim_4.
    let mut target = move_start();
    apply(
        "claim_1 in the target",
        &mut target,
        CreateClaim::new("p", en("Target")),
    );
    let source = read(
        "the source",
        r#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301" xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="src">
  <name content="Src"/>
  <argumentElement xmi:type="sacm:Claim" xmi:id="claim_1"><name content="One"/></argumentElement>
  <argumentElement xmi:type="sacm:Claim" xmi:id="k"><name xmi:id="claim_2" content="K"/><v:extra xmlns:v="urn:v" xmi:id="claim_3"/></argumentElement>
</sacm:ArgumentPackage>
"#,
    );
    let mut after = target.clone();
    let mutation = apply(
        "the copy",
        &mut after,
        copy_of("two", &source, &["claim_1", "k"], "q"),
    );
    assert_eq!(
        mutation.created_ids(),
        ids(&["claim_4", "k"]),
        "{ROW}: claim_1's copy avoids the kept part id claim_2 and the preserved id claim_3"
    );
    assert_eq!(
        get(&after, "k").name_id(),
        Some(&id("claim_2")),
        "{ROW}: setup: the part id claim_2 is kept"
    );
    // Every id stays unique: the tolerant write reads back equal.
    let text = match xmi::write(&after, &WriteOptions::default().with_mode(Mode::Tolerant)) {
        Ok(text) => text,
        Err(error) => panic!("{ROW}: the write was refused: {error}"),
    };
    assert!(
        read("the write", &text) == after,
        "{ROW}: the copy does not read back =="
    );
}

/// An ArgumentPackage `id` holding `body`, in a document of its own.
fn package_of(id: &str, body: &str) -> String {
    format!(
        r#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301" xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="{id}"><name content="{id}"/>{body}</sacm:ArgumentPackage>"#
    )
}

#[test]
fn lib_edit_018_a_preserved_id_a_reference_names_is_refused_as_duplicate_id() {
    // No element of the target has the id x1, but the dangling metaClaim of
    // h names it, so x1 is not free: c9's preserved <v:extra xmi:id="x1"/>
    // cannot be copied verbatim.
    let target = read(
        "the target",
        &package_of(
            "p",
            r#"<argumentElement xmi:type="sacm:Claim" xmi:id="h" metaClaim="x1"><name content="H"/></argumentElement>"#,
        ),
    );
    let source = read(
        "the source",
        &package_of(
            "s",
            r#"<argumentElement xmi:type="sacm:Claim" xmi:id="c9"><name content="C9"/><v:extra xmlns:v="urn:v" xmi:id="x1"/></argumentElement>"#,
        ),
    );
    let error = refused(
        "c9 into a target naming x1",
        &target,
        copy_of("c9", &source, &["c9"], "p"),
    );
    assert!(
        matches!(&error, EditError::DuplicateId { id, .. } if id.as_str() == "x1"),
        "{ROW}: a preserved id a reference of the target names is refused as {error:?}, not \
         DuplicateId of x1"
    );
    assert_eq!(error.code(), Code::CMD_DUPLICATE_ID, "{ROW}: the code");
}

#[test]
fn lib_edit_018_preserved_content_naming_a_dropped_part_id_is_refused_as_duplicate_id() {
    // The target holds the Claim kn, so the copy of k's name drops its
    // xmi:id kn; m's preserved <v:ref xmi:idref="kn"/> would then name the
    // target's kn instead of the copied name.
    let target = read(
        "the target",
        &package_of(
            "p",
            r#"<argumentElement xmi:type="sacm:Claim" xmi:id="kn"><name content="Taken"/></argumentElement>"#,
        ),
    );
    let source = read(
        "the source",
        &package_of(
            "s",
            r#"<argumentElement xmi:type="sacm:Claim" xmi:id="k"><name xmi:id="kn" content="K"/></argumentElement><argumentElement xmi:type="sacm:Claim" xmi:id="m"><name content="M"/><v:ref xmlns:v="urn:v" xmi:idref="kn"/></argumentElement>"#,
        ),
    );
    let error = refused(
        "k and m into a target holding kn",
        &target,
        copy_of("k and m", &source, &["k", "m"], "p"),
    );
    assert!(
        matches!(&error, EditError::DuplicateId { id, .. } if id.as_str() == "kn"),
        "{ROW}: preserved content naming a dropped part id is refused as {error:?}, not \
         DuplicateId of kn"
    );
    assert_eq!(error.code(), Code::CMD_DUPLICATE_ID, "{ROW}: the code");
}

#[test]
fn lib_edit_018_a_copied_preserved_reference_is_avoided_by_later_generated_ids() {
    // k's preserved <v:ref xmi:idref="claim_1"/> names claim_1 in the
    // target once copied, so a later generated Claim id avoids it.
    let mut target = read("the target", &package_of("p", ""));
    let source = read(
        "the source",
        &package_of(
            "s",
            r#"<argumentElement xmi:type="sacm:Claim" xmi:id="k"><name content="K"/><v:ref xmlns:v="urn:v" xmi:idref="claim_1"/></argumentElement>"#,
        ),
    );
    let copied = apply("k", &mut target, copy_of("k", &source, &["k"], "p"));
    assert_eq!(copied.created_ids(), ids(&["k"]), "{ROW}: setup: k is free");
    let created = apply(
        "a Claim after the copy",
        &mut target,
        CreateClaim::new("p", en("Later")),
    );
    assert_eq!(
        created.created_ids(),
        ids(&["claim_2"]),
        "{ROW}: a generated id does not avoid the id a copied preserved reference names"
    );
}

// ---------------------------------------------------------------------------
// The value type
// ---------------------------------------------------------------------------

fn assert_value_type<T: Send + Sync + Clone + Debug + PartialEq + Eq + Into<Operation>>() {}

#[test]
fn lib_edit_018_copy_elements_is_send_sync_clone_debug_eq_and_converts_into_operation() {
    assert_value_type::<CopyElements>();
    let mut draft = draft();
    let copy = copy_of("the draft", &draft, &["r1", "top2"], "p");
    assert_eq!(copy.clone(), copy, "{ROW}: a clone is equal");
    let shown = format!("{copy:?}");
    assert!(
        shown.starts_with("CopyElements {")
            && shown.contains("parent")
            && shown.contains("source_ids")
            && shown.contains("r1")
            && shown.contains("top2")
            && shown.ends_with(".. }"),
        "{ROW}: the Debug of a copy names its parent and source ids, non-exhaustively: {shown}"
    );
    let root = root_copy_of("the draft", &draft, &["draft"]);
    assert_eq!(root.parent(), None, "{ROW}: parent() of root");
    assert_ne!(root, copy, "{ROW}: different copies are unequal");
    let operation = Operation::from(copy.clone());
    assert!(
        matches!(&operation, Operation::CopyElements(held) if *held == copy),
        "{ROW}: a CopyElements converts into Operation::CopyElements: {operation:?}"
    );
    assert_eq!(
        operation.operation_name(),
        "CopyElements",
        "{ROW}: operation_name()"
    );

    // The snapshot is taken when the copy is built: a later change to the
    // source is not copied, and a snapshot of other content is unequal.
    apply(
        "rename top2 in the draft",
        &mut draft,
        SetName::new("top2", en("Renamed later")),
    );
    let later = copy_of("the renamed draft", &draft, &["r1", "top2"], "p");
    assert_ne!(later, copy, "{ROW}: copies of different content are equal");
    let mut target = move_start();
    let mutation = apply("the first copy", &mut target, copy);
    assert_eq!(
        mutation.operation().operation_name(),
        "CopyElements",
        "{ROW}: the name of the copy a Mutation holds"
    );
    assert_eq!(
        get(&target, "top2").name(),
        Some(&en("The brake is safe")),
        "{ROW}: the copy holds the source as it was when the copy was built"
    );
}
