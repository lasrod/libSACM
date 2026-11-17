//! Tests for undo and redo by whole-document copies (LIB-EDIT-011; ADR 0037;
//! D-UNDO).
//!
//! Undo and redo are the application's. The crate adds no API for them: an
//! application keeps [`Document`] clones, as [`History`] below does, which is
//! the recipe of the `edit` module docs ("# Undo and redo"):
//! - clone first, then exactly one atomic call (`edit::apply`,
//!   `edit::apply_preview`, `edit::batch` or `edit::apply_all`), then push the
//!   clone only on `Ok`;
//! - undo and redo restore a copy with `std::mem::replace`.
//!
//! The tests pin what a restored copy is:
//! - `==` to the document before the change, writing the same XMI bytes,
//!   preserved content and the `xmi:id`s of names and contents included;
//! - with the revision it had, so a preview taken before an undone change
//!   applies again;
//! - with the generated-id counters it had, so the next create generates the
//!   id an undone create had (D-ID).
//!
//! Not checked here: the doctest of the module-docs section, which runs with
//! the doctests.

use std::collections::VecDeque;

use sacm::diagnostic::Code;
use sacm::edit::PackageDeletePolicy::DeleteRecursively;
use sacm::edit::{
    self, BatchError, CopyElements, CreateArgumentPackage, CreateClaim, DeleteElement, EditError,
    Operation, SetName,
};
use sacm::model::{Document, ElementId, LangString};
use sacm::xmi::{self, Mode, ReadOptions, WriteOptions};

const ROW: &str = "LIB-EDIT-011";

/// Undo and redo as an application keeps them: whole documents.
struct History {
    undo: VecDeque<Document>,
    redo: Vec<Document>,
    limit: usize,
}

impl History {
    /// Records `before`, the document as it was before a change that
    /// succeeded. A limit of 0 records nothing.
    fn push(&mut self, before: Document) {
        if self.limit == 0 {
            return;
        }
        if self.undo.len() == self.limit {
            self.undo.pop_front();
        }
        self.undo.push_back(before);
        self.redo.clear();
    }
    fn undo(&mut self, doc: &mut Document) -> bool {
        let Some(before) = self.undo.pop_back() else {
            return false;
        };
        self.redo.push(std::mem::replace(doc, before));
        true
    }
    fn redo(&mut self, doc: &mut Document) -> bool {
        let Some(after) = self.redo.pop() else {
            return false;
        };
        self.undo.push_back(std::mem::replace(doc, after));
        true
    }
}

/// An empty history of depth 100.
fn history() -> History {
    History {
        undo: VecDeque::new(),
        redo: Vec::new(),
        limit: 100,
    }
}

/// The AssuranceCasePackage `case` holding the ArgumentPackage `p` with the
/// Claim `c1`. No id in it was generated, so the next generated Claim id is
/// `claim_1`.
const START: &str = r#"<sacm:AssuranceCasePackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301" xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="case">
  <name content="Case"/>
  <argumentPackage xmi:id="p">
    <name content="P"/>
    <argumentElement xmi:type="sacm:Claim" xmi:id="c1"><name content="C1"/></argumentElement>
  </argumentPackage>
</sacm:AssuranceCasePackage>
"#;

/// Hand-written for this test. In the AssuranceCasePackage `case`, the
/// ArgumentPackage `p` with a vendor attribute `v:owner`, a vendor child
/// `<v:review xmi:id="r1">` with mixed content, and a name with the `xmi:id`
/// `p_name`; in `p`, the Claim `c1` with a vendor attribute `v:weight`, the
/// name id `c1_name` and a Description `c1_d` whose `content` and value have
/// the ids `c1_d_content` and `c1_d_v`; the Claim `c2` (name id `c2_name`);
/// and the `AssertedInference` `i1` from `c2` to `c1`.
const PRESERVED: &str = r#"<sacm:AssuranceCasePackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301" xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmlns:v="urn:v" xmi:id="case">
  <name xmi:id="case_name" content="Case"/>
  <argumentPackage xmi:id="p" v:owner="team-a">
    <name xmi:id="p_name" lang="en" content="P"/>
    <v:review xmi:id="r1" v:by="ann">Checked <v:em>twice</v:em></v:review>
    <argumentElement xmi:type="sacm:Claim" xmi:id="c1" v:weight="3">
      <name xmi:id="c1_name" lang="en" content="C1"/>
      <description xmi:id="c1_d"><content xmi:id="c1_d_content"><value xmi:id="c1_d_v" lang="en" content="Why"/></content></description>
    </argumentElement>
    <argumentElement xmi:type="sacm:Claim" xmi:id="c2"><name xmi:id="c2_name" content="C2"/></argumentElement>
    <argumentElement xmi:type="sacm:AssertedInference" xmi:id="i1" source="c2" target="c1"><name content="I1"/></argumentElement>
  </argumentPackage>
</sacm:AssuranceCasePackage>
"#;

/// A draft to accept: the ArgumentPackage `draft` with the Claims `top2` and
/// `sub2` and the `AssertedInference` `di` from `sub2` to `top2`.
const DRAFT: &str = r#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301" xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="draft">
  <name content="Draft"/>
  <argumentElement xmi:type="sacm:Claim" xmi:id="top2"><name content="The brake is safe"/></argumentElement>
  <argumentElement xmi:type="sacm:Claim" xmi:id="sub2"><name content="Hazard H1 is mitigated"/></argumentElement>
  <argumentElement xmi:type="sacm:AssertedInference" xmi:id="di" source="sub2" target="top2"><name content="Inference"/></argumentElement>
</sacm:ArgumentPackage>
"#;

fn en(content: &str) -> LangString {
    LangString::new(content).with_lang("en")
}

fn id(value: &str) -> ElementId {
    ElementId::from(value)
}

/// Reads `input` with the default (tolerant) options.
fn read(what: &str, input: &str) -> Document {
    match xmi::read(input.as_bytes(), &ReadOptions::default()) {
        Ok(output) => output.into_document(),
        Err(error) => panic!("{ROW}: setup: {what} does not read: {error}"),
    }
}

/// Writes `doc` in `mode`, which must succeed.
fn write(what: &str, doc: &Document, mode: Mode) -> String {
    match xmi::write(doc, &WriteOptions::default().with_mode(mode)) {
        Ok(text) => text,
        Err(error) => panic!("{ROW}: {what}: the write failed: {error}"),
    }
}

/// The id of the Claim `edit::apply` generates under `parent` in `doc`.
fn created_claim(what: &str, doc: &mut Document, parent: &str) -> ElementId {
    match edit::apply(doc, CreateClaim::new(parent, en("Next"))) {
        Ok(mutation) => match mutation.created_ids() {
            [only] => only.clone(),
            other => panic!("{ROW}: {what}: the create made {other:?}"),
        },
        Err(error) => panic!("{ROW}: {what}: the create was refused: {error}"),
    }
}

#[test]
fn lib_edit_011_undoing_a_batch_restores_an_equal_document_that_writes_the_same_xmi_and_generates_the_same_next_id()
 {
    let mut doc = read("START", START);
    let original = doc.clone();
    let xmi_before = write("before the batch", &doc, Mode::Strict);
    let mut history = history();

    // The recipe: clone first, then one atomic call, then push on Ok.
    let before = doc.clone();
    let batch = edit::batch(&mut doc, |tx| {
        tx.apply(CreateClaim::new("p", en("Generated one")))?;
        tx.apply(CreateClaim::new("p", en("Generated two")))?;
        tx.apply(SetName::new("c1", en("Renamed")))?;
        Ok::<(), BatchError>(())
    });
    let batch = match batch {
        Ok(batch) => batch,
        Err(error) => panic!("{ROW}: the batch was refused: {error}"),
    };
    history.push(before);
    assert_eq!(
        batch.created_ids(),
        [id("claim_1"), id("claim_2")],
        "{ROW}: the batch generates claim_1 and claim_2"
    );
    assert!(doc != original, "{ROW}: the batch changed the document");
    assert_ne!(
        write("after the batch", &doc, Mode::Strict),
        xmi_before,
        "{ROW}: the batch changed the written XMI"
    );

    assert!(history.undo(&mut doc), "{ROW}: there is a step to undo");
    assert!(
        doc == original,
        "{ROW}: after the undo the document is == to before the batch"
    );
    assert_eq!(
        write("after the undo", &doc, Mode::Strict),
        xmi_before,
        "{ROW}: after the undo xmi::write gives the bytes it gave before the batch"
    );
    assert_eq!(
        created_claim("after the undo", &mut doc, "p"),
        id("claim_1"),
        "{ROW}: after the undo the next generated Claim id is claim_1 again (D-ID)"
    );
}

#[test]
fn lib_edit_011_undoing_a_delete_restores_preserved_content_and_file_ids() {
    let mut doc = read("PRESERVED", PRESERVED);
    let original = doc.clone();
    let xmi_before = write("as read", &doc, Mode::Tolerant);
    for held in [
        "v:owner=\"team-a\"",
        "v:weight=\"3\"",
        "<v:review",
        "xmi:id=\"r1\"",
        "<v:em>twice</v:em>",
        "xmi:id=\"p_name\"",
        "xmi:id=\"c1_name\"",
        "xmi:id=\"c1_d_content\"",
        "xmi:id=\"c1_d_v\"",
    ] {
        assert!(
            xmi_before.contains(held),
            "{ROW}: setup: the tolerant write of the file read holds {held}:\n{xmi_before}"
        );
    }
    let mut history = history();

    let before = doc.clone();
    match edit::apply(
        &mut doc,
        DeleteElement::new("p").with_package_policy(DeleteRecursively),
    ) {
        Ok(mutation) => assert!(
            mutation.deleted_ids().contains(&id("p")),
            "{ROW}: the delete deletes p: {mutation:?}"
        ),
        Err(error) => panic!("{ROW}: the delete of p was refused: {error}"),
    }
    history.push(before);
    let xmi_deleted = write("after the delete", &doc, Mode::Tolerant);
    for gone in ["v:owner", "<v:review", "c1_name", "c1_d_content"] {
        assert!(
            !xmi_deleted.contains(gone),
            "{ROW}: the delete removed {gone}:\n{xmi_deleted}"
        );
    }
    // The ids the delete freed may be taken by a create now.
    for freed in ["r1", "p_name", "c1_name", "c1_d_content", "c1_d_v"] {
        let mut probe = doc.clone();
        assert!(
            edit::apply(
                &mut probe,
                CreateArgumentPackage::new("case", en("Taker")).with_id(freed)
            )
            .is_ok(),
            "{ROW}: after the delete the id {freed:?} is free"
        );
    }

    assert!(history.undo(&mut doc), "{ROW}: there is a step to undo");
    assert!(
        doc == original,
        "{ROW}: after the undo the document is == to the one read"
    );
    assert_eq!(
        write("after the undo", &doc, Mode::Tolerant),
        xmi_before,
        "{ROW}: after the undo the tolerant write gives the bytes it gave before the delete"
    );
    // The file ids are in use again.
    for held in ["r1", "p_name", "c1_name", "c1_d_content", "c1_d_v"] {
        let mut probe = doc.clone();
        let answer = edit::apply(
            &mut probe,
            CreateArgumentPackage::new("case", en("Taker")).with_id(held),
        );
        assert!(
            matches!(&answer, Err(EditError::DuplicateId { id: taken, .. }) if *taken == id(held)),
            "{ROW}: after the undo the id {held:?} is in use again: {answer:?}"
        );
        if let Err(error) = &answer {
            assert_eq!(
                error.code(),
                Code::CMD_DUPLICATE_ID,
                "{ROW}: the code of the refusal of {held:?}"
            );
        }
    }
}

#[test]
fn lib_edit_011_accepting_a_draft_is_one_step() {
    let draft = read("DRAFT", DRAFT);
    let mut doc = read("START", START);
    let original = doc.clone();
    let mut history = history();

    let copy = match CopyElements::new(&draft, ["top2", "sub2", "di"], "p") {
        Ok(copy) => copy,
        Err(error) => panic!("{ROW}: setup: CopyElements::new refused the draft: {error}"),
    };
    let before = doc.clone();
    let batch = edit::batch(&mut doc, |tx| {
        tx.apply(copy)?;
        tx.apply(SetName::new("top2", en("The brake is safe enough")))?;
        Ok::<(), BatchError>(())
    });
    match batch {
        Ok(batch) => assert_eq!(
            batch.created_ids(),
            [id("top2"), id("sub2"), id("di")],
            "{ROW}: the batch copies the draft with its ids"
        ),
        Err(error) => panic!("{ROW}: the batch accepting the draft was refused: {error}"),
    }
    history.push(before);
    let accepted = doc.clone();
    assert!(
        accepted != original,
        "{ROW}: accepting the draft changed the document"
    );

    assert!(history.undo(&mut doc), "{ROW}: there is a step to undo");
    assert!(
        doc == original,
        "{ROW}: one undo gives the document before the batch"
    );
    assert!(
        !history.undo(&mut doc),
        "{ROW}: accepting the draft was one step: there is no second step to undo"
    );
    assert!(
        doc == original,
        "{ROW}: an undo with nothing to undo leaves the document as it is"
    );

    assert!(history.redo(&mut doc), "{ROW}: there is a step to redo");
    assert!(
        doc == accepted,
        "{ROW}: one redo gives the document after the batch"
    );
    assert!(
        !history.redo(&mut doc),
        "{ROW}: accepting the draft was one step: there is no second step to redo"
    );
}

#[test]
fn lib_edit_011_undo_and_redo_restore_the_revision() {
    let mut doc = read("START", START);
    let mut history = history();
    let revision_before = doc.revision();

    let before = doc.clone();
    if let Err(error) = edit::apply(&mut doc, SetName::new("c1", en("Renamed"))) {
        panic!("{ROW}: the change was refused: {error}");
    }
    history.push(before);
    let revision_after = doc.revision();
    assert_ne!(
        revision_after, revision_before,
        "{ROW}: the change gives a new revision"
    );

    assert!(history.undo(&mut doc), "{ROW}: there is a step to undo");
    assert_eq!(
        doc.revision(),
        revision_before,
        "{ROW}: after the undo the revision is the one before the change"
    );
    assert!(history.redo(&mut doc), "{ROW}: there is a step to redo");
    assert_eq!(
        doc.revision(),
        revision_after,
        "{ROW}: after the redo the revision is the one after the change"
    );
}

#[test]
fn lib_edit_011_a_preview_taken_before_an_undone_change_applies_after_the_undo() {
    let mut doc = read("START", START);
    let mut history = history();
    let preview = match edit::preview(&doc, &Operation::from(CreateClaim::new("p", en("Later")))) {
        Ok(preview) => preview,
        Err(error) => panic!("{ROW}: setup: the preview was refused: {error}"),
    };

    let before = doc.clone();
    if let Err(error) = edit::apply(&mut doc, SetName::new("c1", en("Renamed"))) {
        panic!("{ROW}: the change was refused: {error}");
    }
    history.push(before);

    let changed = doc.clone();
    let expired = edit::apply_preview(&mut doc, &preview);
    assert!(
        matches!(
            &expired,
            Err(EditError::PreviewExpired { previewed, current, .. })
                if *previewed == preview.revision() && *current == changed.revision()
        ),
        "{ROW}: after the change the preview is PreviewExpired: {expired:?}"
    );
    if let Err(error) = &expired {
        assert_eq!(
            error.code(),
            Code::CMD_PREVIEW_EXPIRED,
            "{ROW}: the code of the expired preview"
        );
        assert_eq!(
            error.code().as_str(),
            "SACM-CMD-003",
            "{ROW}: the code's text"
        );
    }
    assert!(
        doc == changed && doc.revision() == changed.revision(),
        "{ROW}: the expired preview changed nothing"
    );

    assert!(history.undo(&mut doc), "{ROW}: there is a step to undo");
    match edit::apply_preview(&mut doc, &preview) {
        Ok(mutation) => assert_eq!(
            &mutation,
            preview.mutation(),
            "{ROW}: after the undo the preview applies as it was previewed"
        ),
        Err(error) => panic!("{ROW}: after the undo the preview was refused: {error:?}"),
    }
}

#[test]
fn lib_edit_011_a_refused_change_records_no_step() {
    let mut doc = read("START", START);
    let original = doc.clone();
    let revision_original = doc.revision();
    let mut history = history();

    // A successful change: one step.
    let before = doc.clone();
    if edit::apply(&mut doc, SetName::new("c1", en("Renamed"))).is_ok() {
        history.push(before);
    } else {
        panic!("{ROW}: the first change was refused");
    }
    let changed = doc.clone();
    let revision_changed = doc.revision();

    // A refused change: deleting the non-empty `p` without DeleteRecursively.
    let before = doc.clone();
    let answer = edit::apply(&mut doc, DeleteElement::new("p"));
    match &answer {
        Err(error) => assert_eq!(
            error.code(),
            Code::CMD_PACKAGE_NOT_EMPTY,
            "{ROW}: the delete is refused as SACM-CMD-006: {error:?}"
        ),
        Ok(mutation) => {
            panic!("{ROW}: the delete of a non-empty package was accepted: {mutation:?}")
        }
    }
    if answer.is_ok() {
        history.push(before);
    }
    assert!(
        doc == changed,
        "{ROW}: after the refusal the document is == to before it"
    );
    assert_eq!(
        doc.revision(),
        revision_changed,
        "{ROW}: the refusal leaves the revision unchanged"
    );

    assert!(history.undo(&mut doc), "{ROW}: there is a step to undo");
    assert!(
        doc == original,
        "{ROW}: the undo restores the state before the last successful change"
    );
    assert_eq!(
        doc.revision(),
        revision_original,
        "{ROW}: the undo restores the revision before the last successful change"
    );
    assert!(
        !history.undo(&mut doc),
        "{ROW}: the refused change recorded no step"
    );
}
