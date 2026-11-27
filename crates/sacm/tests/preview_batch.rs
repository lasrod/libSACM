//! A preview of a batch (LIB-EDIT-024; `lasrod/libSACM#113` A6; ADR 0032,
//! which left it for later; slice 0082).
//!
//! - `edit::preview_batch(&doc, f)` is the `BatchMutation` that
//!   `edit::batch(&mut doc, f)` would return, run on a copy: the generated
//!   ids, every accepted mutation and the net ids, the whole delete cascade
//!   included. The document is unchanged: `==`, same revision, same next
//!   generated id, and a `Preview` taken before still applies.
//! - The closure's error is returned unchanged; a refusal the closure
//!   ignores is not in the preview, and counts in the `BatchError` index.
//! - `edit::preview_all` is `apply_all` on a copy.
//! - A preview holds only while the document stays as it was: after another
//!   edit, the batch may generate other ids.

use sacm::edit::{
    self, AddReference, Batch, BatchError, BatchMutation, CreateClaim, CrossPackageReferencePolicy,
    DeleteElement, EditError, Operation, PackageDeletePolicy, ReferenceDeletePolicy,
    RemoveReference, SetName,
};
use sacm::model::{Document, ElementId, LangString, ReferenceEnd};
use sacm::xmi::{self, ReadOptions};

const ROW: &str = "LIB-EDIT-024";

/// In the AssuranceCasePackage `case`:
/// - the ArgumentPackage `p`, first: the Claims `top`, `c1` and `c2`
///   (`c2`'s `metaClaim` `c1`), the `AssertedInference` `i1` from `ci1` (in
///   `inner`) to `top`, and the Claim `holder`, whose `metaClaim` is `ci2`
///   (in `inner`);
/// - the ArgumentPackage `inner`: the Claims `ci1` and `ci2`, and the Note
///   `ni` on `ci1`.
const START: &str = r#"<sacm:AssuranceCasePackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301" xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="case">
  <name content="Case"/>
  <argumentPackage xmi:id="p">
    <name content="P"/>
    <argumentElement xmi:type="sacm:Claim" xmi:id="top"><name content="Top"/></argumentElement>
    <argumentElement xmi:type="sacm:Claim" xmi:id="c1"><name content="C1"/></argumentElement>
    <argumentElement xmi:type="sacm:Claim" xmi:id="c2" metaClaim="c1"><name content="C2"/></argumentElement>
    <argumentElement xmi:type="sacm:AssertedInference" xmi:id="i1" source="ci1" target="top"><name content="I1"/></argumentElement>
    <argumentElement xmi:type="sacm:Claim" xmi:id="holder" metaClaim="ci2"><name content="Holder"/></argumentElement>
  </argumentPackage>
  <argumentPackage xmi:id="inner">
    <name content="Inner"/>
    <argumentElement xmi:type="sacm:Claim" xmi:id="ci1"><name content="CI1"/><note xmi:id="ni"><content><value content="N"/></content></note></argumentElement>
    <argumentElement xmi:type="sacm:Claim" xmi:id="ci2"><name content="CI2"/></argumentElement>
  </argumentPackage>
</sacm:AssuranceCasePackage>"#;

fn start() -> Document {
    match xmi::read(START.as_bytes(), &ReadOptions::default()) {
        Ok(output) => output.into_document(),
        Err(error) => panic!("{ROW}: setup: START does not read: {error}"),
    }
}

fn ids(list: &[ElementId]) -> Vec<&str> {
    list.iter().map(ElementId::as_str).collect()
}

/// The ids the next generated Claim of `doc` gets.
fn next_claim(doc: &Document) -> Vec<ElementId> {
    let mut copy = doc.clone();
    match edit::apply(&mut copy, CreateClaim::new("p", LangString::new("Next"))) {
        Ok(mutation) => mutation.created_ids().to_vec(),
        Err(error) => panic!("{ROW}: setup: the next CreateClaim is refused: {error}"),
    }
}

/// Creates two Claims with generated ids, renames `c1`, adds the first new
/// Claim to `c2`'s `metaClaim` and removes `c1` from it.
fn edits(tx: &mut Batch<'_>) -> Result<(), BatchError> {
    let first = tx.apply(CreateClaim::new("p", LangString::new("New 1")))?;
    let first = first.created_ids()[0].clone();
    tx.apply(CreateClaim::new("p", LangString::new("New 2")))?;
    tx.apply(SetName::new("c1", LangString::new("Renamed")))?;
    tx.apply(AddReference::new("c2", ReferenceEnd::MetaClaim, first))?;
    tx.apply(RemoveReference::new("c2", ReferenceEnd::MetaClaim, "c1"))?;
    Ok(())
}

/// The delete of `inner` with everything in it and the relationships that
/// reference it, across packages.
fn cascade() -> DeleteElement {
    DeleteElement::new("inner")
        .with_package_policy(PackageDeletePolicy::DeleteRecursively)
        .with_reference_policy(ReferenceDeletePolicy::DeleteReferencingRelationships)
        .with_cross_package_policy(CrossPackageReferencePolicy::ApplyReferencePolicy)
}

#[test]
fn lib_edit_024_a_preview_is_the_batch_mutation_the_batch_would_give() {
    let doc = start();
    let preview = edit::preview_batch(&doc, edits)
        .unwrap_or_else(|error| panic!("{ROW}: the preview is refused: {error}"));
    let mut batched = doc.clone();
    let batch = edit::batch(&mut batched, edits)
        .unwrap_or_else(|error| panic!("{ROW}: the batch is refused: {error}"));
    assert_eq!(
        preview, batch,
        "{ROW}: the preview is not the batch's mutation"
    );
    assert_eq!(
        preview
            .mutations()
            .iter()
            .map(|m| m.operation().operation_name())
            .collect::<Vec<_>>(),
        [
            "CreateClaim",
            "CreateClaim",
            "SetName",
            "AddReference",
            "RemoveReference"
        ],
        "{ROW}: the mutations of the preview"
    );
    assert_eq!(
        ids(preview.created_ids()),
        ["claim_1", "claim_2"],
        "{ROW}: the generated ids of the preview"
    );
    assert_eq!(
        ids(preview.changed_ids()),
        ["c1", "c2"],
        "{ROW}: the net changed ids of the preview"
    );
    assert!(
        preview.deleted_ids().is_empty(),
        "{ROW}: the preview deletes"
    );
}

#[test]
fn lib_edit_024_a_preview_includes_the_whole_delete_cascade() {
    let doc = start();
    let f = |tx: &mut Batch<'_>| -> Result<(), BatchError> {
        tx.apply(cascade())?;
        Ok(())
    };
    let preview = edit::preview_batch(&doc, f)
        .unwrap_or_else(|error| panic!("{ROW}: the preview is refused: {error}"));
    let gone = ["i1", "inner", "ci1", "ni", "ci2"];
    let in_pre_order_before: Vec<&str> = doc
        .elements()
        .map(|e| e.id().as_str())
        .filter(|id| gone.contains(id))
        .collect();
    assert_eq!(
        in_pre_order_before, gone,
        "{ROW}: setup: the pre-order before"
    );
    assert_eq!(
        ids(preview.deleted_ids()),
        gone,
        "{ROW}: the preview does not delete the package, its contents and the relationship \
         that references it, in pre-order before"
    );
    assert_eq!(
        ids(preview.changed_ids()),
        ["holder"],
        "{ROW}: the preview's changed ids are not the scrubbed holder"
    );
    assert!(
        preview.created_ids().is_empty(),
        "{ROW}: the cascade creates"
    );
    let mut deleted = doc.clone();
    let batch = edit::batch(&mut deleted, f)
        .unwrap_or_else(|error| panic!("{ROW}: the batch is refused: {error}"));
    assert_eq!(
        preview, batch,
        "{ROW}: the preview is not what the batch returns"
    );
    assert!(
        gone.iter().all(|id| !deleted.contains(id)) && doc.contains("inner"),
        "{ROW}: setup: the batch does not delete the cascade, or the preview did"
    );
}

#[test]
fn lib_edit_024_a_preview_changes_nothing() {
    let doc = start();
    let before = doc.clone();
    let revision = doc.revision();
    let next = next_claim(&doc);
    let earlier = edit::preview(
        &doc,
        &Operation::from(SetName::new("top", LangString::new("Earlier"))),
    )
    .unwrap_or_else(|error| panic!("{ROW}: setup: the earlier preview: {error}"));
    let previews = [
        edit::preview_batch(&doc, edits),
        edit::preview_batch(&doc, |tx: &mut Batch<'_>| -> Result<(), BatchError> {
            tx.apply(cascade())?;
            Ok(())
        }),
        edit::preview_all(
            &doc,
            [Operation::from(CreateClaim::new("p", LangString::new("X")))],
        ),
    ];
    for (index, preview) in previews.iter().enumerate() {
        assert!(
            preview.is_ok(),
            "{ROW}: setup: preview {index}: {preview:?}"
        );
        assert_eq!(doc, before, "{ROW}: preview {index} changed the document");
        assert_eq!(
            doc.revision(),
            revision,
            "{ROW}: preview {index} changed the revision"
        );
        assert_eq!(
            next_claim(&doc),
            next,
            "{ROW}: preview {index} changed the next generated id"
        );
    }
    let mut doc = doc;
    let applied = edit::apply_preview(&mut doc, &earlier);
    assert!(
        applied.is_ok(),
        "{ROW}: a preview taken before the batch previews no longer applies: {applied:?}"
    );
}

/// The closure's own error, or a refusal passed on with `?`.
#[derive(Debug, PartialEq)]
enum Stop {
    Here,
    Refused(BatchError),
}

impl From<BatchError> for Stop {
    fn from(error: BatchError) -> Self {
        Self::Refused(error)
    }
}

#[test]
fn lib_edit_024_a_preview_returns_the_closure_s_error() {
    let doc = start();
    let before = doc.clone();
    let own = |tx: &mut Batch<'_>| -> Result<(), Stop> {
        tx.apply(SetName::new("c1", LangString::new("Renamed")))?;
        Err(Stop::Here)
    };
    assert_eq!(
        edit::preview_batch(&doc, own),
        Err(Stop::Here),
        "{ROW}: the closure's own error is not returned unchanged"
    );
    let passed_on = |tx: &mut Batch<'_>| -> Result<(), Stop> {
        tx.apply(SetName::new("c1", LangString::new("Renamed")))?;
        tx.apply(SetName::new("nope", LangString::new("X")))?;
        Ok(())
    };
    let previewed = edit::preview_batch(&doc, passed_on);
    let batched = edit::batch(&mut doc.clone(), passed_on);
    assert_eq!(
        previewed, batched,
        "{ROW}: the refusal passed on is not the batch's"
    );
    match &previewed {
        Err(Stop::Refused(error)) => {
            assert_eq!(error.index(), 1, "{ROW}: the refusal's index");
            assert!(
                matches!(error.error(), EditError::ElementNotFound { element, .. } if element.as_str() == "nope"),
                "{ROW}: the refusal passed on: {error:?}"
            );
        }
        other => panic!("{ROW}: the refusal passed on with ? gives {other:?}"),
    }
    assert_eq!(doc, before, "{ROW}: a refused preview changed the document");
}

#[test]
fn lib_edit_024_an_ignored_refusal_is_not_in_the_preview() {
    let doc = start();
    let mut indexes = Vec::new();
    let preview = edit::preview_batch(&doc, |tx| -> Result<(), BatchError> {
        tx.apply(SetName::new("c1", LangString::new("One")))?;
        if let Err(error) = tx.apply(SetName::new("nope", LangString::new("X"))) {
            indexes.push(error.index());
        }
        tx.apply(SetName::new("c2", LangString::new("Two")))?;
        if let Err(error) = tx.apply(CreateClaim::new("nowhere", LangString::new("X"))) {
            indexes.push(error.index());
        }
        Ok(())
    })
    .unwrap_or_else(|error| panic!("{ROW}: the preview is refused: {error}"));
    assert_eq!(
        indexes,
        [1, 3],
        "{ROW}: the indexes of the ignored refusals"
    );
    let names: Vec<&str> = preview
        .mutations()
        .iter()
        .map(|m| m.operation().operation_name())
        .collect();
    assert_eq!(
        names,
        ["SetName", "SetName"],
        "{ROW}: the ignored refusals are in the preview"
    );
    assert_eq!(
        ids(preview.changed_ids()),
        ["c1", "c2"],
        "{ROW}: the net changed ids"
    );
}

#[test]
fn lib_edit_024_preview_all_is_apply_all_on_a_copy() {
    let doc = start();
    let before = doc.clone();
    let accepted = [
        Operation::from(CreateClaim::new("p", LangString::new("A"))),
        SetName::new("c1", LangString::new("B")).into(),
        Operation::from(cascade()),
    ];
    let preview = edit::preview_all(&doc, accepted.clone())
        .unwrap_or_else(|error| panic!("{ROW}: preview_all refuses: {error}"));
    let applied = edit::apply_all(&mut doc.clone(), accepted)
        .unwrap_or_else(|error| panic!("{ROW}: apply_all refuses: {error}"));
    assert_eq!(preview, applied, "{ROW}: preview_all is not apply_all");
    assert_eq!(doc, before, "{ROW}: preview_all changed the document");

    let refused_third = [
        Operation::from(CreateClaim::new("p", LangString::new("A"))),
        SetName::new("c1", LangString::new("B")).into(),
        SetName::new("nope", LangString::new("C")).into(),
        SetName::new("c2", LangString::new("D")).into(),
    ];
    let error = match edit::preview_all(&doc, refused_third.clone()) {
        Ok(preview) => panic!("{ROW}: a refused third operation previews as {preview:?}"),
        Err(error) => error,
    };
    assert_eq!(
        error.index(),
        2,
        "{ROW}: the index of the refused third operation"
    );
    let applied = edit::apply_all(&mut doc.clone(), refused_third);
    assert_eq!(
        Err(error),
        applied.map(drop),
        "{ROW}: preview_all refuses otherwise than apply_all"
    );
    assert_eq!(
        doc, before,
        "{ROW}: a refused preview_all changed the document"
    );
}

#[test]
fn lib_edit_024_an_empty_preview_is_empty() {
    let doc = start();
    let preview: BatchMutation = edit::preview_batch(&doc, |_tx| Ok::<(), BatchError>(()))
        .unwrap_or_else(|error| panic!("{ROW}: an empty preview is refused: {error}"));
    assert!(
        preview.mutations().is_empty(),
        "{ROW}: an empty preview has mutations"
    );
    assert!(preview.is_empty(), "{ROW}: an empty preview is not empty");
    let none: [Operation; 0] = [];
    let all = edit::preview_all(&doc, none)
        .unwrap_or_else(|error| panic!("{ROW}: an empty preview_all is refused: {error}"));
    assert!(
        all.is_empty() && all.mutations().is_empty(),
        "{ROW}: an empty preview_all is not empty"
    );
}

#[test]
fn lib_edit_024_a_preview_of_a_changed_document_may_differ() {
    let mut doc = start();
    let preview = edit::preview_batch(&doc, edits)
        .unwrap_or_else(|error| panic!("{ROW}: the preview is refused: {error}"));
    let taken = edit::apply(
        &mut doc,
        CreateClaim::new("p", LangString::new("Meanwhile")),
    )
    .unwrap_or_else(|error| panic!("{ROW}: setup: the create is refused: {error}"));
    assert_eq!(
        ids(taken.created_ids()),
        [preview.created_ids()[0].as_str()],
        "{ROW}: setup: the create does not take the preview's first id"
    );
    let batch = edit::batch(&mut doc, edits)
        .unwrap_or_else(|error| panic!("{ROW}: the batch is refused: {error}"));
    assert_ne!(
        batch.created_ids(),
        preview.created_ids(),
        "{ROW}: the batch on the changed document generates the preview's ids"
    );
    assert_ne!(batch, preview, "{ROW}: the batch equals the stale preview");
}
