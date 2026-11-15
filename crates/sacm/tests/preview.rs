//! Tests for previews and revisions (LIB-EDIT-004, LIB-EDIT-007; ADR 0031;
//! D-PREVIEW).
//!
//! - LIB-EDIT-004: `edit::preview` answers as `edit::check` and
//!   `edit::apply` do without changing the document, holds the `Mutation`
//!   `apply` would return, and lists the references a delete would leave
//!   dangling, in document pre-order of the holder, then end, then position.
//! - LIB-EDIT-007: `Document::revision` identifies a state of a document in
//!   the process, and `edit::apply_preview` refuses a preview taken at
//!   another revision with `EditError::PreviewExpired` (SACM-CMD-003).
//!
//! Revisions are compared only for equality, never for their values (ADR
//! 0031 Consequences). The preview of every operation of the LIB-EDIT-002
//! set is tested in `edit_atomic.rs`.

use std::fmt::Debug;

use sacm::diagnostic::Code;
use sacm::edit::CrossPackageReferencePolicy::ApplyReferencePolicy;
use sacm::edit::PackageDeletePolicy::DeleteRecursively;
use sacm::edit::ReferenceDeletePolicy::{
    DeleteReferencingRelationships, RejectIfReferenced, ScrubReferences,
};
use sacm::edit::{
    self, AddReference, BatchError, CreateArgumentPackage, CreateAssuranceCasePackage, CreateClaim,
    DanglingReference, DeleteElement, EditError, Mutation, Operation, Preview, RemoveReference,
    ReorderElements, SetFlag, SetGid, SetName,
};
use sacm::model::{
    Document, ElementId, ElementKind, Flag, LangString, ReferenceEnd, Revision, SacmElement,
};
use sacm::xmi::{self, ReadOptions};

const EDIT_004: &str = "LIB-EDIT-004";
const EDIT_007: &str = "LIB-EDIT-007";

/// The namespace declarations of every document.
const NAMESPACES: &str = r#"xmlns:sacm="http://www.omg.org/spec/SACM/20220301" xmlns:xmi="http://www.omg.org/spec/XMI/20131001""#;

// ---------------------------------------------------------------------------
// Documents
// ---------------------------------------------------------------------------

/// An `argumentElement` of the kind `kind` with the id `id` and the
/// attributes `attributes`, named after its id, holding `children`.
fn element(kind: &str, id: &str, attributes: &str, children: &str) -> String {
    format!(
        r#"<argumentElement xmi:type="sacm:{kind}" xmi:id="{id}" {attributes}><name content="{id}"/>{children}</argumentElement>"#
    )
}

/// A Claim `id` with the attributes `attributes`.
fn claim(id: &str, attributes: &str) -> String {
    element("Claim", id, attributes, "")
}

/// An ArgumentPackage `id` in an AssuranceCasePackage, with the attributes
/// `attributes`, holding `body`.
fn package(id: &str, attributes: &str, body: &str) -> String {
    format!(
        r#"<argumentPackage xmi:id="{id}" {attributes}><name content="{id}"/>{body}</argumentPackage>"#
    )
}

/// The root AssuranceCasePackage `case`, with the attributes `attributes`,
/// holding `body`.
fn case(attributes: &str, body: &str) -> String {
    format!(
        r#"<sacm:AssuranceCasePackage {NAMESPACES} xmi:id="case" {attributes}><name content="case"/>{body}</sacm:AssuranceCasePackage>"#
    )
}

/// Reads `text` with the default options.
fn read(row: &str, text: &str) -> Document {
    match xmi::read(text.as_bytes(), &ReadOptions::default()) {
        Ok(output) => output.into_document(),
        Err(error) => panic!("{row}: setup: the document does not read: {error}\n{text}"),
    }
}

fn named(content: &str) -> LangString {
    LangString::new(content).with_lang("en")
}

/// The ArgumentPackage `p` holding the Claims `c1` (gid `g1`) and `c2`, and
/// the inference `i1` from `c2` to `c1`, in the root `case`.
fn small() -> String {
    case(
        "",
        &package(
            "p",
            "",
            &format!(
                "{}{}{}",
                claim("c1", r#"gid="g1""#),
                claim("c2", ""),
                element("AssertedInference", "i1", r#"source="c2" target="c1""#, "")
            ),
        ),
    )
}

/// The ids the next generated root AssuranceCasePackage, root
/// ArgumentPackage and Claim in it would get, observed on a clone.
fn next_ids(row: &str, doc: &Document) -> Vec<ElementId> {
    let mut probe = doc.clone();
    let mut created = Vec::new();
    let case = edit::apply(&mut probe, CreateAssuranceCasePackage::root(named("probe")));
    let arguments = edit::apply(&mut probe, CreateArgumentPackage::root(named("probe")));
    for answer in [case, arguments] {
        match answer {
            Ok(mutation) => created.extend(mutation.created_ids().iter().cloned()),
            Err(error) => panic!("{row}: setup: the probe was refused: {error}"),
        }
    }
    match edit::apply(&mut probe, CreateClaim::new(&created[1], named("probe"))) {
        Ok(mutation) => created.extend(mutation.created_ids().iter().cloned()),
        Err(error) => panic!("{row}: setup: the probe claim was refused: {error}"),
    }
    created
}

/// Applies `operation`, which the test expects to succeed.
fn apply(row: &str, doc: &mut Document, operation: impl Into<Operation>) -> Mutation {
    let operation = operation.into();
    match edit::apply(doc, operation.clone()) {
        Ok(mutation) => mutation,
        Err(error) => panic!("{row}: setup: {operation:?} was refused: {error} ({error:?})"),
    }
}

/// Previews `operation`, which the test expects to be accepted, and asserts
/// that the preview changed nothing: `==`, revision and next generated ids.
fn previewed(row: &str, context: &str, doc: &Document, operation: &Operation) -> Preview {
    let before = doc.clone();
    let revision = doc.revision();
    let next = next_ids(row, doc);
    let preview = match edit::preview(doc, operation) {
        Ok(preview) => preview,
        Err(error) => panic!("{row}: {context}: the preview of {operation:?} was refused: {error}"),
    };
    assert_unchanged(row, context, doc, &before, revision, &next);
    preview
}

/// Asserts that `doc` is `==` to `before`, has the revision `revision` and
/// generates the ids `next`.
fn assert_unchanged(
    row: &str,
    context: &str,
    doc: &Document,
    before: &Document,
    revision: Revision,
    next: &[ElementId],
) {
    assert!(
        doc == before,
        "{row}: {context}: the document is not == to before"
    );
    assert!(
        doc.revision() == revision,
        "{row}: {context}: the revision changed"
    );
    assert_eq!(
        next_ids(row, doc),
        next,
        "{row}: {context}: the next generated ids changed"
    );
}

/// The dangling references of `preview` as (holder, end, id, crosses).
fn entries(preview: &Preview) -> Vec<(&str, ReferenceEnd, &str, bool)> {
    preview
        .dangling_references()
        .iter()
        .map(|dangling| {
            (
                dangling.holder().as_str(),
                dangling.end(),
                dangling.id().as_str(),
                dangling.crosses_package(),
            )
        })
        .collect()
}

fn strs(ids: &[ElementId]) -> Vec<&str> {
    ids.iter().map(ElementId::as_str).collect()
}

/// Asserts that `revision` is none of `seen`, then adds it.
fn assert_fresh(row: &str, context: &str, seen: &mut Vec<Revision>, revision: Revision) {
    assert!(
        !seen.contains(&revision),
        "{row}: {context}: the revision is one seen before"
    );
    seen.push(revision);
}

/// Applies `preview` to `doc`, asserts `PreviewExpired` (SACM-CMD-003)
/// naming both revisions, and that nothing changed.
fn assert_expired(row: &str, context: &str, doc: &mut Document, preview: &Preview) {
    let before = doc.clone();
    let revision = doc.revision();
    let next = next_ids(row, doc);
    let error = match edit::apply_preview(doc, preview) {
        Err(error) => error,
        Ok(mutation) => panic!("{row}: {context}: the stale preview was applied: {mutation:?}"),
    };
    assert!(
        matches!(
            &error,
            EditError::PreviewExpired { previewed, current, .. }
                if *previewed == preview.revision() && *current == revision
        ),
        "{row}: {context}: the answer is not PreviewExpired naming both revisions: {error:?}"
    );
    assert_eq!(
        error.code(),
        Code::CMD_PREVIEW_EXPIRED,
        "{row}: {context}: the code"
    );
    assert_eq!(
        error.code().as_str(),
        "SACM-CMD-003",
        "{row}: {context}: the code's text"
    );
    assert_unchanged(row, context, doc, &before, revision, &next);
}

// ---------------------------------------------------------------------------
// LIB-EDIT-004: previews
// ---------------------------------------------------------------------------

#[test]
fn lib_edit_004_preview_takes_the_document_and_the_operation_by_reference() {
    let preview: fn(&Document, &Operation) -> Result<Preview, EditError> = edit::preview;
    let doc = read(EDIT_004, &small());
    let operation = Operation::from(SetName::new("c1", named("New")));
    let answer = preview(&doc, &operation);
    assert!(
        answer.is_ok(),
        "{EDIT_004}: the preview of a name was refused: {answer:?}"
    );
    // The caller still holds both.
    assert_eq!(
        edit::apply(&mut doc.clone(), operation).map(drop),
        Ok(()),
        "{EDIT_004}: the previewed operation was refused by apply"
    );
}

/// One operation of each family on [`small`]: a create with a generated id,
/// a name, a gid that is already held (a change of nothing), a reference
/// add, a reorder and deletes under three policies.
fn accepted_operations() -> Vec<Operation> {
    vec![
        CreateClaim::new("p", named("New")).into(),
        CreateClaim::new("p", named("New")).with_id("c_new").into(),
        SetName::new("c1", named("Renamed")).into(),
        SetGid::new("c1", "g1").into(),
        SetFlag::new("i1", Flag::IsCounter, true).into(),
        AddReference::new("c2", ReferenceEnd::MetaClaim, "c1").into(),
        ReorderElements::new("p", ["i1", "c2", "c1"]).into(),
        DeleteElement::new("i1").into(),
        DeleteElement::new("c2")
            .with_reference_policy(DeleteReferencingRelationships)
            .into(),
        DeleteElement::new("c1")
            .with_reference_policy(ScrubReferences)
            .into(),
        DeleteElement::new("case")
            .with_package_policy(DeleteRecursively)
            .into(),
    ]
}

#[test]
fn lib_edit_004_a_preview_holds_the_operation_the_revision_and_the_mutation_apply_would_return() {
    let doc = read(EDIT_004, &small());
    for operation in accepted_operations() {
        let context = format!("{operation:?}");
        let preview = previewed(EDIT_004, &context, &doc, &operation);
        assert_eq!(
            preview.operation(),
            &operation,
            "{EDIT_004}: {context}: operation() is not the operation previewed"
        );
        assert!(
            preview.revision() == doc.revision(),
            "{EDIT_004}: {context}: revision() is not the document's"
        );
        let applied = edit::apply(&mut doc.clone(), operation.clone());
        assert_eq!(
            Ok(preview.mutation().clone()),
            applied,
            "{EDIT_004}: {context}: mutation() is not what apply returns"
        );
    }
}

#[test]
fn lib_edit_004_preview_refuses_exactly_as_check_and_apply() {
    let text = case(
        "",
        &format!(
            "{}{}",
            package(
                "p",
                "",
                &format!(
                    "{}{}{}",
                    claim("c1", ""),
                    claim("c2", r#"metaClaim="c1""#),
                    element("AssertedInference", "i1", r#"source="c2" target="c1""#, "")
                )
            ),
            package("q", "", &claim("far", r#"citedElement="c2""#))
        ),
    );
    let doc = read(EDIT_004, &text);
    let refusals: Vec<(Operation, Code)> = vec![
        (
            CreateClaim::new("nowhere", named("N")).into(),
            Code::CMD_TARGET_NOT_FOUND,
        ),
        (
            CreateClaim::new("p", named("N")).with_id("c1").into(),
            Code::CMD_DUPLICATE_ID,
        ),
        (
            SetFlag::new("c1", Flag::IsCounter, true).into(),
            Code::CMD_FEATURE_NOT_APPLICABLE,
        ),
        (
            RemoveReference::new("c1", ReferenceEnd::MetaClaim, "c2").into(),
            Code::CMD_TARGET_NOT_FOUND,
        ),
        (
            DeleteElement::new("nowhere").into(),
            Code::CMD_TARGET_NOT_FOUND,
        ),
        (DeleteElement::new("p").into(), Code::CMD_PACKAGE_NOT_EMPTY),
        (DeleteElement::new("c1").into(), Code::CMD_DELETE_REFERENCED),
        (
            DeleteElement::new("c2")
                .with_reference_policy(DeleteReferencingRelationships)
                .into(),
            Code::CMD_EXTERNAL_REFERENCES,
        ),
        (
            DeleteElement::new("c2")
                .with_cross_package_policy(ApplyReferencePolicy)
                .into(),
            Code::CMD_DELETE_REFERENCED,
        ),
    ];
    for (operation, code) in refusals {
        let context = format!("{operation:?}");
        let before = doc.clone();
        let revision = doc.revision();
        let next = next_ids(EDIT_004, &doc);
        let answer = edit::preview(&doc, &operation);
        assert_unchanged(EDIT_004, &context, &doc, &before, revision, &next);
        let Err(error) = answer else {
            panic!("{EDIT_004}: {context}: the preview was accepted");
        };
        assert_eq!(
            Err(error.clone()),
            edit::check(&doc, &operation),
            "{EDIT_004}: {context}: preview refuses otherwise than check"
        );
        assert_eq!(
            Err(error.clone()),
            edit::apply(&mut doc.clone(), operation.clone()),
            "{EDIT_004}: {context}: preview refuses otherwise than apply"
        );
        assert_eq!(error.code(), code, "{EDIT_004}: {context}: the code");
    }
}

#[test]
fn lib_edit_004_preview_changes_nothing_and_generates_no_id() {
    let mut doc = read(EDIT_004, &small());
    let create = Operation::from(CreateClaim::new("p", named("New")));
    let first = previewed(EDIT_004, "the first preview", &doc, &create);
    let second = previewed(EDIT_004, "the second preview", &doc, &create);
    assert_eq!(
        first.mutation(),
        second.mutation(),
        "{EDIT_004}: two previews of one create give different mutations"
    );
    let delete = Operation::from(DeleteElement::new("i1"));
    previewed(EDIT_004, "a delete", &doc, &delete);
    let refused = Operation::from(DeleteElement::new("c1"));
    let before = doc.clone();
    let revision = doc.revision();
    let next = next_ids(EDIT_004, &doc);
    let answer = edit::preview(&doc, &refused);
    assert!(
        answer.is_err(),
        "{EDIT_004}: setup: the preview of deleting the referenced c1 was accepted"
    );
    assert_unchanged(
        EDIT_004,
        "a refused preview",
        &doc,
        &before,
        revision,
        &next,
    );
    let applied = apply(EDIT_004, &mut doc, create);
    assert_eq!(
        applied.created_ids(),
        first.mutation().created_ids(),
        "{EDIT_004}: after previews the create generates another id than previewed"
    );
}

#[test]
fn lib_edit_004_an_operation_that_deletes_nothing_has_no_dangling_references() {
    let doc = read(EDIT_004, &small());
    for operation in accepted_operations() {
        let context = format!("{operation:?}");
        let preview = previewed(EDIT_004, &context, &doc, &operation);
        let deletes_referenced = matches!(
            &operation,
            Operation::DeleteElement(delete) if delete.element().as_str() != "i1"
        );
        if !deletes_referenced {
            assert_eq!(
                entries(&preview),
                [],
                "{EDIT_004}: {context}: nothing referenced is deleted, and references are listed"
            );
        }
    }
}

/// `doomed` holds t1, t2, m1, why and the inference `inner` between t1 and
/// t2. `keep` holds `k`, the group `g`, the inference `h` (holding a deleted
/// id in six ends) and the `ArtifactReference` `art`. `ghost` names no
/// element.
fn ordering_document() -> String {
    let doomed = format!(
        "{}{}{}{}{}",
        claim("t1", ""),
        claim("t2", ""),
        claim("m1", ""),
        element("ArgumentReasoning", "why", "", ""),
        element(
            "AssertedInference",
            "inner",
            r#"source="t1" target="t2""#,
            ""
        )
    );
    let keep = format!(
        "{}{}{}{}",
        claim("k", ""),
        element(
            "ArgumentGroup",
            "g",
            r#"argumentElement="t2 k t1 t2 ghost""#,
            ""
        ),
        element(
            "AssertedInference",
            "h",
            r#"target="k" source="t2 k t1" reasoning="why" metaClaim="m1 ghost m1" abstractForm="t2" citedElement="t1""#,
            ""
        ),
        element(
            "ArtifactReference",
            "art",
            r#"referencedArtifactElement="t1 ghost t1""#,
            ""
        )
    );
    case(
        "",
        &format!(
            "{}{}",
            package("doomed", "", &doomed),
            package("keep", "", &keep)
        ),
    )
}

#[test]
fn lib_edit_004_dangling_references_are_the_references_held_outside_the_deleted_subtree_in_order() {
    let doc = read(EDIT_004, &ordering_document());
    let operation = Operation::from(
        DeleteElement::new("doomed")
            .with_reference_policy(ScrubReferences)
            .with_package_policy(DeleteRecursively)
            .with_cross_package_policy(ApplyReferencePolicy),
    );
    let preview = previewed(EDIT_004, "doomed", &doc, &operation);
    // Holder in pre-order; then end in the order of `ReferenceEnd`; then
    // position, one entry per distinct id and end. Not listed: `inner`,
    // inside the deleted subtree; `k`, which is not deleted; `ghost`, which
    // names no element.
    assert_eq!(
        entries(&preview),
        [
            ("g", ReferenceEnd::GroupMember, "t2", true),
            ("g", ReferenceEnd::GroupMember, "t1", true),
            ("h", ReferenceEnd::CitedElement, "t1", true),
            ("h", ReferenceEnd::AbstractForm, "t2", true),
            ("h", ReferenceEnd::MetaClaim, "m1", true),
            ("h", ReferenceEnd::Reasoning, "why", true),
            ("h", ReferenceEnd::Source, "t2", true),
            ("h", ReferenceEnd::Source, "t1", true),
            ("art", ReferenceEnd::ReferencedArtifactElement, "t1", true),
        ],
        "{EDIT_004}: dangling_references()"
    );
    assert_eq!(
        (
            strs(preview.mutation().deleted_ids()),
            strs(preview.mutation().changed_ids())
        ),
        (
            vec!["doomed", "t1", "t2", "m1", "why", "inner"],
            vec!["g", "h", "art"]
        ),
        "{EDIT_004}: the mutation of the preview"
    );
}

/// In `p`: `r` from `c1` to `c2`, the group `g` holding `r`, and `r2` from
/// `c2` to `r`.
fn cascade_document() -> String {
    case(
        "",
        &package(
            "p",
            "",
            &format!(
                "{}{}{}{}{}",
                claim("c1", ""),
                claim("c2", ""),
                element("AssertedInference", "r", r#"source="c1" target="c2""#, ""),
                element("ArgumentGroup", "g", r#"argumentElement="r""#, ""),
                element("AssertedContext", "r2", r#"source="c2" target="r""#, "")
            ),
        ),
    )
}

#[test]
fn lib_edit_004_references_held_by_a_deleted_relationship_outside_the_subtree_are_listed() {
    // `r` and `r2` are outside the subtree of `c1` and go with the
    // delete; the references they hold to deleted elements are listed, as
    // ADR 0031 defines a dangling reference ("held by an element outside the
    // deleted subtree").
    let doc = read(EDIT_004, &cascade_document());
    for policy in [DeleteReferencingRelationships, ScrubReferences] {
        let context = format!("c1 under {policy:?}");
        let operation = Operation::from(DeleteElement::new("c1").with_reference_policy(policy));
        let preview = previewed(EDIT_004, &context, &doc, &operation);
        assert_eq!(
            entries(&preview),
            [
                ("r", ReferenceEnd::Source, "c1", false),
                ("g", ReferenceEnd::GroupMember, "r", false),
                ("r2", ReferenceEnd::Target, "r", false),
            ],
            "{EDIT_004}: {context}: dangling_references()"
        );
        assert_eq!(
            (
                strs(preview.mutation().deleted_ids()),
                strs(preview.mutation().changed_ids())
            ),
            (vec!["c1", "r", "r2"], vec!["g"]),
            "{EDIT_004}: {context}: the mutation of the preview"
        );
    }
}

#[test]
fn lib_edit_004_crosses_package_is_true_exactly_when_the_nearest_packages_differ() {
    // `case` and `p` cite c1, each its own nearest package; `same` is in p
    // with c1; `inner` is in the package n nested in p; `far` is in the
    // sibling package q.
    let text = case(
        r#"citedElement="c1""#,
        &format!(
            "{}{}",
            package(
                "p",
                r#"citedElement="c1""#,
                &format!(
                    "{}{}{}",
                    claim("c1", ""),
                    claim("same", r#"metaClaim="c1""#),
                    element(
                        "ArgumentPackage",
                        "n",
                        "",
                        &claim("inner", r#"metaClaim="c1""#)
                    )
                )
            ),
            package("q", "", &claim("far", r#"metaClaim="c1""#))
        ),
    );
    let doc = read(EDIT_004, &text);
    let operation = Operation::from(
        DeleteElement::new("c1")
            .with_reference_policy(ScrubReferences)
            .with_cross_package_policy(ApplyReferencePolicy),
    );
    let preview = previewed(EDIT_004, "c1", &doc, &operation);
    assert_eq!(
        entries(&preview),
        [
            ("case", ReferenceEnd::CitedElement, "c1", true),
            ("p", ReferenceEnd::CitedElement, "c1", false),
            ("same", ReferenceEnd::MetaClaim, "c1", false),
            ("inner", ReferenceEnd::MetaClaim, "c1", true),
            ("far", ReferenceEnd::MetaClaim, "c1", true),
        ],
        "{EDIT_004}: dangling_references()"
    );
}

#[test]
fn lib_edit_004_crosses_package_counts_interfaces_and_bindings_as_packages() {
    // Inserted: the reader does not read interfaces and bindings yet.
    let mut doc = Document::new();
    for (parent, kind, value) in [
        (None, ElementKind::AssuranceCasePackage, "case"),
        (Some("case"), ElementKind::ArgumentPackage, "ap"),
        (Some("ap"), ElementKind::Claim, "c_ap"),
        (Some("case"), ElementKind::ArgumentPackageInterface, "iface"),
        (Some("iface"), ElementKind::Claim, "c_if"),
        (Some("iface"), ElementKind::Claim, "c_if2"),
        (Some("case"), ElementKind::ArgumentPackageBinding, "bind"),
        (Some("bind"), ElementKind::Claim, "c_b"),
    ] {
        let parent = parent.map(ElementId::from);
        if let Err(error) = doc.insert(
            parent.as_ref(),
            kind,
            ElementId::from(value),
            Some(named(value)),
        ) {
            panic!("{EDIT_004}: setup could not insert the {kind} {value:?}: {error}");
        }
    }
    for holder in ["c_ap", "c_if2", "c_b"] {
        apply(
            EDIT_004,
            &mut doc,
            AddReference::new(holder, ReferenceEnd::MetaClaim, "c_if"),
        );
    }
    let operation = Operation::from(
        DeleteElement::new("c_if")
            .with_reference_policy(ScrubReferences)
            .with_cross_package_policy(ApplyReferencePolicy),
    );
    let preview = previewed(EDIT_004, "c_if", &doc, &operation);
    assert_eq!(
        entries(&preview),
        [
            ("c_ap", ReferenceEnd::MetaClaim, "c_if", true),
            ("c_if2", ReferenceEnd::MetaClaim, "c_if", false),
            ("c_b", ReferenceEnd::MetaClaim, "c_if", true),
        ],
        "{EDIT_004}: the interface iface and the binding bind are packages"
    );
}

#[test]
fn lib_edit_004_crosses_package_compares_the_holder_with_the_element_the_reference_names() {
    // `r`, in q, goes with c1, in p; the group `g`, also in q, holds `r`.
    // That reference stays within q, though the delete started in p.
    let text = case(
        "",
        &format!(
            "{}{}",
            package("p", "", &claim("c1", "")),
            package(
                "q",
                "",
                &format!(
                    "{}{}{}",
                    claim("cq", ""),
                    element("AssertedInference", "r", r#"source="c1" target="cq""#, ""),
                    element("ArgumentGroup", "g", r#"argumentElement="r""#, "")
                )
            )
        ),
    );
    let doc = read(EDIT_004, &text);
    let operation = Operation::from(
        DeleteElement::new("c1")
            .with_reference_policy(DeleteReferencingRelationships)
            .with_cross_package_policy(ApplyReferencePolicy),
    );
    let preview = previewed(EDIT_004, "c1", &doc, &operation);
    assert_eq!(
        entries(&preview),
        [
            ("r", ReferenceEnd::Source, "c1", true),
            ("g", ReferenceEnd::GroupMember, "r", false),
        ],
        "{EDIT_004}: dangling_references()"
    );
}

#[test]
fn lib_edit_004_a_delete_is_previewed_under_the_policies_it_holds() {
    let doc = read(EDIT_004, &cascade_document());
    let rejected =
        Operation::from(DeleteElement::new("c1").with_reference_policy(RejectIfReferenced));
    let answer = edit::preview(&doc, &rejected);
    assert!(
        matches!(
            &answer,
            Err(EditError::Referenced { holder, id, .. })
                if holder.as_str() == "r" && id.as_str() == "c1"
        ),
        "{EDIT_004}: under RejectIfReferenced the preview refuses: {answer:?}"
    );
    let scrub_in_package =
        Operation::from(DeleteElement::new("r").with_reference_policy(ScrubReferences));
    let preview = previewed(EDIT_004, "r scrubbed", &doc, &scrub_in_package);
    assert_eq!(
        (
            entries(&preview),
            strs(preview.mutation().deleted_ids()),
            strs(preview.mutation().changed_ids())
        ),
        (
            vec![
                ("g", ReferenceEnd::GroupMember, "r", false),
                ("r2", ReferenceEnd::Target, "r", false)
            ],
            vec!["r", "r2"],
            vec!["g"]
        ),
        "{EDIT_004}: r2 loses its target and goes; g loses a member and stays"
    );
}

fn assert_preview_value<T: Clone + Debug + Send + Sync>() {}

fn assert_plain_value<T: Clone + Debug + PartialEq + Eq + Send + Sync>() {}

#[test]
fn lib_edit_004_preview_and_dangling_reference_are_thread_safe_values() {
    assert_preview_value::<Preview>();
    assert_plain_value::<DanglingReference>();
    assert_plain_value::<DeleteElement>();
    let doc = read(EDIT_004, &cascade_document());
    let operation = Operation::from(
        DeleteElement::new("c1").with_reference_policy(DeleteReferencingRelationships),
    );
    let preview = previewed(EDIT_004, "c1", &doc, &operation);
    let copy = preview.clone();
    assert_eq!(
        copy.dangling_references(),
        preview.dangling_references(),
        "{EDIT_004}: a cloned preview lists other references"
    );
    let shown = format!("{:?}", preview.dangling_references()[0]);
    assert!(
        shown.contains("c1"),
        "{EDIT_004}: the Debug of a DanglingReference does not show its id: {shown}"
    );
}

// ---------------------------------------------------------------------------
// LIB-EDIT-007: revisions and stale previews
// ---------------------------------------------------------------------------

#[test]
fn lib_edit_007_apply_preview_takes_the_document_mutably_and_the_preview_by_reference() {
    let apply_preview: fn(&mut Document, &Preview) -> Result<Mutation, EditError> =
        edit::apply_preview;
    let mut doc = read(EDIT_007, &small());
    let operation = Operation::from(SetName::new("c1", named("New")));
    let preview = previewed(EDIT_007, "a name", &doc, &operation);
    let applied = apply_preview(&mut doc, &preview);
    assert_eq!(
        applied.as_ref(),
        Ok(preview.mutation()),
        "{EDIT_007}: the preview applied at once"
    );
}

#[test]
fn lib_edit_007_new_default_and_read_documents_have_distinct_revisions() {
    let mut seen = Vec::new();
    let documents = [
        Document::new(),
        Document::new(),
        Document::default(),
        Document::default(),
        read(EDIT_007, &small()),
        read(EDIT_007, &small()),
    ];
    for (index, doc) in documents.iter().enumerate() {
        assert_fresh(
            EDIT_007,
            &format!("document {index}"),
            &mut seen,
            doc.revision(),
        );
    }
}

#[test]
fn lib_edit_007_clone_copies_the_revision() {
    let mut doc = read(EDIT_007, &small());
    let copy = doc.clone();
    assert!(
        copy.revision() == doc.revision(),
        "{EDIT_007}: a clone has another revision"
    );
    let before = doc.revision();
    let mut changed = copy.clone();
    apply(EDIT_007, &mut changed, SetName::new("c1", named("New")));
    assert!(
        changed.revision() != before && doc.revision() == before && copy.revision() == before,
        "{EDIT_007}: editing a clone changed the revision of its original, or not its own"
    );
    apply(EDIT_007, &mut doc, SetName::new("c1", named("New")));
    assert!(
        doc.revision() != changed.revision(),
        "{EDIT_007}: the same edit on two copies gave the same revision"
    );
    assert!(
        doc == changed,
        "{EDIT_007}: setup: the two copies are not =="
    );
}

#[test]
fn lib_edit_007_equality_ignores_the_revision() {
    assert!(
        Document::new() == Document::new(),
        "{EDIT_007}: two new documents are not =="
    );
    let first = read(EDIT_007, &small());
    let second = read(EDIT_007, &small());
    assert!(
        first == second && first.revision() != second.revision(),
        "{EDIT_007}: two reads of one file are not ==, or share a revision"
    );
    let mut edited = first.clone();
    apply(EDIT_007, &mut edited, SetGid::new("c1", "g1"));
    assert!(
        edited == first && edited.revision() != first.revision(),
        "{EDIT_007}: an edit that changes nothing made the document unequal, or kept the revision"
    );
}

#[test]
fn lib_edit_007_every_successful_apply_gives_a_new_revision_even_one_that_changes_nothing() {
    let mut doc = read(EDIT_007, &small());
    let mut seen = vec![doc.revision()];
    let operations: Vec<(Operation, &str)> = vec![
        (SetName::new("c1", named("Renamed")).into(), "a name"),
        (SetName::new("c1", named("Renamed")).into(), "the same name"),
        (SetGid::new("c1", "g1").into(), "the gid held"),
        (
            ReorderElements::new("p", ["c1", "c2", "i1"]).into(),
            "the order held",
        ),
        (CreateClaim::new("p", named("New")).into(), "a create"),
        (DeleteElement::new("i1").into(), "a delete"),
    ];
    for (operation, context) in operations {
        let mutation = apply(EDIT_007, &mut doc, operation);
        if context.contains("held") || context.contains("same") {
            assert_eq!(
                mutation.changed_ids(),
                [] as [ElementId; 0],
                "{EDIT_007}: setup: {context} changed something"
            );
        }
        assert_fresh(EDIT_007, context, &mut seen, doc.revision());
    }
}

#[test]
fn lib_edit_007_every_successful_apply_preview_batch_and_apply_all_gives_a_new_revision() {
    let mut doc = read(EDIT_007, &small());
    let mut seen = vec![doc.revision()];

    let preview = previewed(
        EDIT_007,
        "a name",
        &doc,
        &SetName::new("c1", named("New")).into(),
    );
    let applied = edit::apply_preview(&mut doc, &preview);
    assert!(
        applied.is_ok(),
        "{EDIT_007}: setup: the preview was refused: {applied:?}"
    );
    assert_fresh(EDIT_007, "apply_preview", &mut seen, doc.revision());

    let noop = previewed(
        EDIT_007,
        "the gid held",
        &doc,
        &SetGid::new("c1", "g1").into(),
    );
    let applied = edit::apply_preview(&mut doc, &noop);
    assert!(
        applied.is_ok(),
        "{EDIT_007}: setup: the preview was refused: {applied:?}"
    );
    assert_fresh(
        EDIT_007,
        "apply_preview of a change of nothing",
        &mut seen,
        doc.revision(),
    );

    let operation: Operation = SetName::new("c2", named("Batched")).into();
    let batched = edit::batch(&mut doc, |tx| -> Result<(), BatchError> {
        tx.apply(operation)?;
        Ok(())
    });
    assert!(
        batched.is_ok(),
        "{EDIT_007}: setup: the batch failed: {batched:?}"
    );
    assert_fresh(EDIT_007, "batch", &mut seen, doc.revision());

    let empty = edit::batch(&mut doc, |_tx| -> Result<(), BatchError> { Ok(()) });
    assert!(
        empty.is_ok(),
        "{EDIT_007}: setup: the empty batch failed: {empty:?}"
    );
    assert_fresh(EDIT_007, "an empty batch", &mut seen, doc.revision());

    let refused: Operation = DeleteElement::new("nowhere").into();
    let swallowed = edit::batch(&mut doc, |tx| -> Result<(), BatchError> {
        let _ = tx.apply(refused);
        Ok(())
    });
    assert!(
        swallowed.is_ok(),
        "{EDIT_007}: setup: the batch around a swallowed refusal failed: {swallowed:?}"
    );
    assert_fresh(
        EDIT_007,
        "a batch around a swallowed refusal",
        &mut seen,
        doc.revision(),
    );

    let all = edit::apply_all(
        &mut doc,
        vec![
            Operation::from(SetName::new("c1", named("All"))),
            Operation::from(SetGid::new("c2", "g2")),
        ],
    );
    assert!(all.is_ok(), "{EDIT_007}: setup: apply_all failed: {all:?}");
    assert_fresh(EDIT_007, "apply_all", &mut seen, doc.revision());

    let none = edit::apply_all(&mut doc, Vec::<Operation>::new());
    assert!(
        none.is_ok(),
        "{EDIT_007}: setup: apply_all of nothing failed: {none:?}"
    );
    assert_fresh(EDIT_007, "apply_all of nothing", &mut seen, doc.revision());
}

#[test]
fn lib_edit_007_a_refusal_or_a_rolled_back_batch_keeps_the_revision() {
    let mut doc = read(EDIT_007, &small());
    let revision = doc.revision();
    let keep = |doc: &Document, context: &str| {
        assert!(
            doc.revision() == revision,
            "{EDIT_007}: {context} changed the revision"
        );
    };
    let refused = Operation::from(DeleteElement::new("c1"));
    let answer = edit::apply(&mut doc, refused.clone());
    assert!(
        answer.is_err(),
        "{EDIT_007}: setup: the delete was accepted"
    );
    keep(&doc, "a refused apply");
    let _ = edit::check(&doc, &refused);
    let _ = edit::check(&doc, &DeleteElement::new("i1").into());
    keep(&doc, "check");
    let _ = edit::preview(&doc, &refused);
    let _ = edit::preview(&doc, &DeleteElement::new("i1").into());
    keep(&doc, "preview");

    let good: Operation = SetName::new("c2", named("Rolled back")).into();
    let rolled_back = edit::batch(&mut doc, |tx| -> Result<(), &'static str> {
        let _ = tx.apply(good.clone());
        Err("stop")
    });
    assert_eq!(
        rolled_back.map(drop),
        Err("stop"),
        "{EDIT_007}: setup: the batch did not return the closure's error"
    );
    keep(&doc, "a batch whose closure failed");
    let refused_in_batch = edit::batch(&mut doc, |tx| -> Result<(), BatchError> {
        tx.apply(good.clone())?;
        tx.apply(refused.clone())?;
        Ok(())
    });
    assert!(
        refused_in_batch.is_err(),
        "{EDIT_007}: setup: the batch did not fail"
    );
    keep(&doc, "a batch that propagated a refusal");
    let all = edit::apply_all(&mut doc, vec![good.clone(), refused.clone()]);
    assert!(all.is_err(), "{EDIT_007}: setup: apply_all did not fail");
    keep(&doc, "a refused apply_all");
    assert_eq!(
        doc.get("c2")
            .and_then(SacmElement::name)
            .and_then(LangString::content),
        Some("c2"),
        "{EDIT_007}: a rolled-back batch changed the name"
    );
}

/// A successful change of a document, by one of the four public mutations.
type Change = Box<dyn Fn(&mut Document)>;

#[test]
fn lib_edit_007_apply_preview_after_any_successful_change_is_preview_expired_and_changes_nothing() {
    let operation = Operation::from(DeleteElement::new("i1"));
    let changes: Vec<(&str, Change)> = vec![
        (
            "apply",
            Box::new(|doc| {
                apply(EDIT_007, doc, SetName::new("c1", named("New")));
            }),
        ),
        (
            "an apply that changes nothing",
            Box::new(|doc| {
                apply(EDIT_007, doc, SetGid::new("c1", "g1"));
            }),
        ),
        (
            "apply_preview",
            Box::new(|doc| {
                let other = Operation::from(SetName::new("c2", named("New")));
                let preview = previewed(EDIT_007, "another preview", doc, &other);
                if let Err(error) = edit::apply_preview(doc, &preview) {
                    panic!("{EDIT_007}: setup: the other preview was refused: {error}");
                }
            }),
        ),
        (
            "batch",
            Box::new(|doc| {
                let other = Operation::from(SetName::new("c2", named("New")));
                if let Err(error) = edit::batch(doc, |tx| -> Result<(), BatchError> {
                    tx.apply(other)?;
                    Ok(())
                }) {
                    panic!("{EDIT_007}: setup: the batch failed: {error:?}");
                }
            }),
        ),
        (
            "apply_all",
            Box::new(|doc| {
                let other = Operation::from(SetName::new("c2", named("New")));
                if let Err(error) = edit::apply_all(doc, vec![other]) {
                    panic!("{EDIT_007}: setup: apply_all failed: {error:?}");
                }
            }),
        ),
    ];
    for (context, change) in changes {
        let mut doc = read(EDIT_007, &small());
        let preview = previewed(EDIT_007, context, &doc, &operation);
        change(&mut doc);
        assert_expired(EDIT_007, &format!("after {context}"), &mut doc, &preview);
        // Still expired: the refusal changed nothing.
        assert_expired(
            EDIT_007,
            &format!("again after {context}"),
            &mut doc,
            &preview,
        );
    }
    // A preview applied once is stale afterwards.
    let mut doc = read(EDIT_007, &small());
    let preview = previewed(EDIT_007, "the delete", &doc, &operation);
    if let Err(error) = edit::apply_preview(&mut doc, &preview) {
        panic!("{EDIT_007}: setup: the preview was refused: {error}");
    }
    assert_expired(EDIT_007, "applied twice", &mut doc, &preview);
}

#[test]
fn lib_edit_007_a_refusal_leaves_a_preview_applicable() {
    let mut doc = read(EDIT_007, &small());
    let operation = Operation::from(DeleteElement::new("i1"));
    let preview = previewed(EDIT_007, "the delete", &doc, &operation);
    let refused = edit::apply(&mut doc, DeleteElement::new("c1"));
    assert!(
        refused.is_err(),
        "{EDIT_007}: setup: the delete was accepted"
    );
    let all = edit::apply_all(&mut doc, vec![Operation::from(DeleteElement::new("c1"))]);
    assert!(all.is_err(), "{EDIT_007}: setup: apply_all was accepted");
    let applied = edit::apply_preview(&mut doc, &preview);
    assert_eq!(
        applied.as_ref(),
        Ok(preview.mutation()),
        "{EDIT_007}: after refusals, which keep the revision, the preview does not apply"
    );
}

#[test]
fn lib_edit_007_a_preview_taken_on_a_clone_is_refused_by_the_original_once_either_has_changed() {
    let operation = Operation::from(DeleteElement::new("i1"));
    let edit_name = |doc: &mut Document| {
        apply(EDIT_007, doc, SetName::new("c1", named("New")));
    };

    // The original changes, then the preview is taken on the clone.
    let mut doc = read(EDIT_007, &small());
    let copy = doc.clone();
    edit_name(&mut doc);
    let preview = previewed(EDIT_007, "on the clone", &copy, &operation);
    assert_expired(EDIT_007, "the original changed", &mut doc, &preview);

    // The clone changes, then the preview is taken on it.
    let mut doc = read(EDIT_007, &small());
    let mut copy = doc.clone();
    edit_name(&mut copy);
    let preview = previewed(EDIT_007, "on the changed clone", &copy, &operation);
    assert_expired(EDIT_007, "the clone changed", &mut doc, &preview);

    // Both make the same one edit: equal content and as many edits, still
    // different states (ADR 0031 R7: no per-document counter).
    let mut doc = read(EDIT_007, &small());
    let mut copy = doc.clone();
    edit_name(&mut doc);
    edit_name(&mut copy);
    assert!(doc == copy, "{EDIT_007}: setup: the copies are not ==");
    let preview = previewed(EDIT_007, "on the clone, both changed", &copy, &operation);
    assert_expired(EDIT_007, "both changed alike", &mut doc, &preview);

    // Neither has changed since the clone: the clone's state is the
    // original's, because Clone copies the revision, so the preview applies.
    let mut doc = read(EDIT_007, &small());
    let mut copy = doc.clone();
    let preview = previewed(EDIT_007, "on the unchanged clone", &copy, &operation);
    let applied = edit::apply_preview(&mut doc, &preview);
    assert_eq!(
        applied.as_ref(),
        Ok(preview.mutation()),
        "{EDIT_007}: a preview of an unchanged clone does not apply to the original"
    );
    // The original has changed now, so the same preview no longer applies
    // to it, while it still applies to the unchanged clone.
    assert_expired(EDIT_007, "the original applied it", &mut doc, &preview);
    let applied = edit::apply_preview(&mut copy, &preview);
    assert_eq!(
        applied.as_ref(),
        Ok(preview.mutation()),
        "{EDIT_007}: the preview no longer applies to the unchanged clone"
    );
}

#[test]
fn lib_edit_007_a_preview_of_another_document_is_refused() {
    let operation = Operation::from(DeleteElement::new("i1"));
    let first = read(EDIT_007, &small());
    let mut second = read(EDIT_007, &small());
    assert!(first == second, "{EDIT_007}: setup: the reads are not ==");
    let preview = previewed(EDIT_007, "on the first read", &first, &operation);
    assert_expired(EDIT_007, "the second read", &mut second, &preview);

    let create = Operation::from(CreateArgumentPackage::root(named("A")));
    let empty = Document::new();
    let mut other = Document::new();
    let preview = previewed(EDIT_007, "on a new document", &empty, &create);
    assert_expired(EDIT_007, "another new document", &mut other, &preview);
}

#[test]
fn lib_edit_007_a_preview_applied_at_once_returns_its_mutation() {
    let start = read(EDIT_007, &small());
    for operation in accepted_operations() {
        let context = format!("{operation:?}");
        let mut doc = start.clone();
        let preview = previewed(EDIT_007, &context, &doc, &operation);
        let mut applied_directly = start.clone();
        let direct = edit::apply(&mut applied_directly, operation.clone());
        let applied = edit::apply_preview(&mut doc, &preview);
        assert_eq!(
            applied.as_ref(),
            Ok(preview.mutation()),
            "{EDIT_007}: {context}: apply_preview does not return the preview's mutation"
        );
        assert_eq!(
            applied, direct,
            "{EDIT_007}: {context}: apply_preview returns otherwise than apply"
        );
        assert!(
            doc == applied_directly,
            "{EDIT_007}: {context}: apply_preview left another document than apply"
        );
        assert!(
            doc.revision() != preview.revision(),
            "{EDIT_007}: {context}: the applied preview kept the revision"
        );
        assert_eq!(
            next_ids(EDIT_007, &doc),
            next_ids(EDIT_007, &applied_directly),
            "{EDIT_007}: {context}: apply_preview left other next generated ids than apply"
        );
    }
}

fn assert_revision_value<T: Clone + Debug + PartialEq + Eq + Send + Sync>() {}

#[test]
fn lib_edit_007_revision_is_a_comparable_thread_safe_value() {
    assert_revision_value::<Revision>();
    let doc = Document::new();
    let revision = doc.revision();
    assert!(
        doc.revision() == revision,
        "{EDIT_007}: reading the revision twice gives two revisions"
    );
}
