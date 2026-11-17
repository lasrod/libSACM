//! Tests for deleting elements under explicit policies (LIB-EDIT-005,
//! LIB-EDIT-006; ADR 0031; D-POLICY, D-ATOMIC, D-VAL-LAYER).
//!
//! - LIB-EDIT-005: `DeleteElement` refuses a referenced element unless the
//!   caller chooses `DeleteReferencingRelationships` or `ScrubReferences`,
//!   and a successful delete never leaves a dangling reference.
//! - LIB-EDIT-006: a non-empty element needs `DeleteRecursively`, and a
//!   reference across a package boundary needs `ApplyReferencePolicy`.
//!
//! Every document here is read from XMI written in this file, or built with
//! the `test-support` `Document::insert` where the reader cannot read the
//! kind yet (an ArtifactPackage, an interface, a binding). Every element is
//! named after its id.
//!
//! Every accepted delete is checked by [`assert_clean_delete`] against
//! `semantic_compare(before, after)`, and every refusal by [`refused`]: the
//! document is `==` to before, with the same revision and the same next
//! generated ids, and `edit::check` gives the same error. Previews are
//! tested in `preview.rs`, and the deletes over the whole LIB-EDIT-002 set in
//! `edit_atomic.rs`.

use std::collections::BTreeSet;

use sacm::compare::{Difference, semantic_compare};
use sacm::diagnostic::Code;
use sacm::edit::CrossPackageReferencePolicy::{
    ApplyReferencePolicy, RejectIfExternalReferencesExist,
};
use sacm::edit::PackageDeletePolicy::{DeleteRecursively, RejectIfNonEmpty};
use sacm::edit::ReferenceDeletePolicy::{
    DeleteReferencingRelationships, RejectIfReferenced, ScrubReferences,
};
use sacm::edit::{
    self, CreateArgumentPackage, CreateAssuranceCasePackage, CreateClaim,
    CrossPackageReferencePolicy, DeleteElement, EditError, Mutation, Operation,
    PackageDeletePolicy, ReferenceDeletePolicy, SetReferences,
};
use sacm::model::{Document, ElementId, ElementKind, LangString, ReferenceEnd, SacmElement};
use sacm::xmi::{self, Mode, ReadOptions, WriteOptions};

const EDIT_005: &str = "LIB-EDIT-005";
const EDIT_006: &str = "LIB-EDIT-006";

/// The namespace declarations of every document.
const NAMESPACES: &str = r#"xmlns:sacm="http://www.omg.org/spec/SACM/20220301" xmlns:xmi="http://www.omg.org/spec/XMI/20131001""#;

/// The twelve reference ends, in the order of `ReferenceEnd`.
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

/// The three reference policies, from the strictest.
const REFERENCE_POLICIES: [ReferenceDeletePolicy; 3] = [
    RejectIfReferenced,
    DeleteReferencingRelationships,
    ScrubReferences,
];

/// The two package policies, from the strictest.
const PACKAGE_POLICIES: [PackageDeletePolicy; 2] = [RejectIfNonEmpty, DeleteRecursively];

/// The two cross-package policies, from the strictest.
const CROSS_PACKAGE_POLICIES: [CrossPackageReferencePolicy; 2] =
    [RejectIfExternalReferencesExist, ApplyReferencePolicy];

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

/// An ArgumentPackage `id` in an AssuranceCasePackage, holding `body`.
fn package(id: &str, body: &str) -> String {
    format!(r#"<argumentPackage xmi:id="{id}"><name content="{id}"/>{body}</argumentPackage>"#)
}

/// An ArgumentPackage `id` nested in an ArgumentPackage, holding `body`.
fn nested(id: &str, body: &str) -> String {
    element("ArgumentPackage", id, "", body)
}

/// A Note `id`.
fn note(id: &str) -> String {
    format!(r#"<note xmi:id="{id}"/>"#)
}

/// The root AssuranceCasePackage `case`, holding `body`.
fn case(body: &str) -> String {
    format!(
        r#"<sacm:AssuranceCasePackage {NAMESPACES} xmi:id="case"><name content="case"/>{body}</sacm:AssuranceCasePackage>"#
    )
}

/// Reads `text` with the default options.
fn read(row: &str, text: &str) -> Document {
    match xmi::read(text.as_bytes(), &ReadOptions::default()) {
        Ok(output) => output.into_document(),
        Err(error) => panic!("{row}: setup: the document does not read: {error}\n{text}"),
    }
}

fn id(value: &str) -> ElementId {
    ElementId::from(value)
}

fn strs(ids: &[ElementId]) -> Vec<&str> {
    ids.iter().map(ElementId::as_str).collect()
}

fn named(content: &str) -> LangString {
    LangString::new(content).with_lang("en")
}

/// The element `value` of `doc`, or a failed test.
fn get<'d>(row: &str, doc: &'d Document, value: &str) -> SacmElement<'d> {
    match doc.get(value) {
        Some(found) => found,
        None => panic!("{row}: the document has no element {value:?}"),
    }
}

/// A delete of `element` under the three policies given.
fn delete(
    element: &str,
    reference: ReferenceDeletePolicy,
    package: PackageDeletePolicy,
    cross: CrossPackageReferencePolicy,
) -> DeleteElement {
    DeleteElement::new(element)
        .with_reference_policy(reference)
        .with_package_policy(package)
        .with_cross_package_policy(cross)
}

/// The 12 deletes of `element`, one per policy combination, from the
/// strictest.
fn every_policy(element: &str) -> Vec<DeleteElement> {
    let mut deletes = Vec::new();
    for reference in REFERENCE_POLICIES {
        for package in PACKAGE_POLICIES {
            for cross in CROSS_PACKAGE_POLICIES {
                deletes.push(delete(element, reference, package, cross));
            }
        }
    }
    deletes
}

/// The ids `element` holds in `end`, in order, read through the public
/// accessors.
fn held(element: SacmElement<'_>, end: ReferenceEnd) -> Vec<ElementId> {
    fn one(id: Option<&ElementId>) -> Vec<ElementId> {
        id.into_iter().cloned().collect()
    }
    match end {
        ReferenceEnd::CitedElement => one(element.cited_element()),
        ReferenceEnd::AbstractForm => one(element.abstract_form()),
        ReferenceEnd::MetaClaim => element.meta_claims().to_vec(),
        ReferenceEnd::Reasoning => one(element.reasoning()),
        // `sources()` and `targets()` answer for both relationship kinds
        // (ADR 0034 amendment 2): each end is its own kind's only.
        ReferenceEnd::Source if element.kind() != ElementKind::ArtifactAssetRelationship => {
            element.sources().to_vec()
        }
        ReferenceEnd::ArtifactSource
            if element.kind() == ElementKind::ArtifactAssetRelationship =>
        {
            element.sources().to_vec()
        }
        ReferenceEnd::ArtifactTarget
            if element.kind() == ElementKind::ArtifactAssetRelationship =>
        {
            element.targets().to_vec()
        }
        ReferenceEnd::Source | ReferenceEnd::ArtifactSource | ReferenceEnd::ArtifactTarget => {
            Vec::new()
        }
        ReferenceEnd::Target => one(element.target()),
        ReferenceEnd::Structure => one(element.structure()),
        ReferenceEnd::ReferencedArtifactElement => element.referenced_artifact_elements().to_vec(),
        // `group_members()` answers for both group kinds (ADR 0034): each end
        // is the members of its own group kind only.
        ReferenceEnd::GroupMember if element.kind() == ElementKind::ArgumentGroup => {
            element.group_members().to_vec()
        }
        ReferenceEnd::ArtifactGroupMember if element.kind() == ElementKind::ArtifactGroup => {
            element.group_members().to_vec()
        }
        ReferenceEnd::GroupMember | ReferenceEnd::ArtifactGroupMember => Vec::new(),
        other => panic!("{EDIT_005}: an end this test does not know: {other:?}"),
    }
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

// ---------------------------------------------------------------------------
// Answers
// ---------------------------------------------------------------------------

/// The XMI name of a reference end, whether an `EditError` holds it as a
/// `ReferenceEnd` or by its name, as the existing variants do.
trait EndName {
    fn end_name(&self) -> &'static str;
}

impl EndName for ReferenceEnd {
    fn end_name(&self) -> &'static str {
        self.name()
    }
}

impl EndName for &'static str {
    fn end_name(&self) -> &'static str {
        self
    }
}

/// The variant, holder, end and id of an `EditError::Referenced` or
/// `EditError::CrossPackageReference`.
fn named_reference(error: &EditError) -> Option<(&'static str, &str, &'static str, &str)> {
    match error {
        EditError::Referenced {
            holder, end, id, ..
        } => Some(("Referenced", holder.as_str(), end.end_name(), id.as_str())),
        EditError::CrossPackageReference {
            holder, end, id, ..
        } => Some((
            "CrossPackageReference",
            holder.as_str(),
            end.end_name(),
            id.as_str(),
        )),
        _ => None,
    }
}

/// Applies `operation`, which must be refused, to `doc`, and asserts that
/// the refusal changed nothing (`==`, revision, next generated ids) and that
/// `edit::check` gives the same error. Returns the error.
fn refused(
    row: &str,
    context: &str,
    doc: &mut Document,
    operation: impl Into<Operation>,
) -> EditError {
    let operation = operation.into();
    let before = doc.clone();
    let revision = doc.revision();
    let next = next_ids(row, doc);
    let checked = edit::check(doc, &operation);
    let error = match edit::apply(doc, operation.clone()) {
        Err(error) => error,
        Ok(mutation) => panic!("{row}: {context}: {operation:?} was accepted: {mutation:?}"),
    };
    assert_eq!(
        checked,
        Err(error.clone()),
        "{row}: {context}: check answers otherwise than apply"
    );
    assert!(
        *doc == before,
        "{row}: {context}: the refusal {error:?} changed the document"
    );
    assert!(
        doc.revision() == revision,
        "{row}: {context}: the refusal {error:?} changed the revision"
    );
    assert_eq!(
        next_ids(row, doc),
        next,
        "{row}: {context}: the refusal {error:?} changed the next generated ids"
    );
    error
}

/// Applies `operation`, which must succeed, to `doc`, asserts that the
/// mutation holds the operation and, for a delete, [`assert_clean_delete`].
/// Returns the mutation.
fn accepted(
    row: &str,
    context: &str,
    doc: &mut Document,
    operation: impl Into<Operation>,
) -> Mutation {
    let operation = operation.into();
    let before = doc.clone();
    let checked = edit::check(doc, &operation);
    let mutation = match edit::apply(doc, operation.clone()) {
        Ok(mutation) => mutation,
        Err(error) => panic!("{row}: {context}: {operation:?} was refused: {error} ({error:?})"),
    };
    assert_eq!(
        checked,
        Ok(()),
        "{row}: {context}: check refused what apply accepts"
    );
    assert_eq!(
        mutation.operation(),
        &operation,
        "{row}: {context}: the mutation does not hold the operation given"
    );
    if matches!(operation, Operation::DeleteElement(_)) {
        assert_clean_delete(row, context, &before, doc, &mutation);
    }
    mutation
}

/// What every successful delete keeps (D-ATOMIC, D-VAL-LAYER):
/// - `created_ids` is empty, and `deleted_ids` are the left-only ids of
///   `semantic_compare(before, after)`, in pre-order before;
/// - the comparison reports nothing else but reference ends, so the parent
///   and every other feature are unchanged;
/// - the survivors are the elements before minus `deleted_ids`, in order;
/// - each surviving end is its old list minus the deleted ids, in order, so
///   no element holds a deleted id and an id that dangled before still does;
/// - `changed_ids` are the survivors whose ends changed, in pre-order after.
fn assert_clean_delete(
    row: &str,
    context: &str,
    before: &Document,
    after: &Document,
    mutation: &Mutation,
) {
    assert_eq!(
        mutation.created_ids(),
        [] as [ElementId; 0],
        "{row}: {context}: a delete reported created ids"
    );
    let differences = semantic_compare(before, after);
    let left_only: Vec<ElementId> = differences
        .iter()
        .filter_map(|difference| match difference {
            Difference::LeftOnly { id, .. } => Some(id.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(
        mutation.deleted_ids(),
        left_only,
        "{row}: {context}: deleted_ids() is not the left-only ids of semantic_compare"
    );
    for difference in &differences {
        assert!(
            matches!(
                difference,
                Difference::LeftOnly { .. }
                    | Difference::CitedElement { .. }
                    | Difference::AbstractForm { .. }
                    | Difference::MetaClaims { .. }
                    | Difference::Reasoning { .. }
                    | Difference::Sources { .. }
                    | Difference::Target { .. }
                    | Difference::Structure { .. }
                    | Difference::ReferencedArtifactElements { .. }
                    | Difference::GroupMembers { .. }
                    | Difference::ArtifactGroupMembers { .. }
                    | Difference::ArtifactSources { .. }
                    | Difference::ArtifactTargets { .. }
            ),
            "{row}: {context}: the delete changed more than elements and reference ends: {difference}"
        );
    }
    let deleted: BTreeSet<&ElementId> = mutation.deleted_ids().iter().collect();
    let survivors: Vec<&ElementId> = before
        .elements()
        .map(SacmElement::id)
        .filter(|id| !deleted.contains(id))
        .collect();
    let now: Vec<&ElementId> = after.elements().map(SacmElement::id).collect();
    assert_eq!(
        now, survivors,
        "{row}: {context}: the survivors are not the elements before minus deleted_ids"
    );
    let mut changed = Vec::new();
    for element in after.elements() {
        let old = get(row, before, element.id().as_str());
        let mut lost = false;
        for end in ENDS {
            let expected: Vec<ElementId> = held(old, end)
                .into_iter()
                .filter(|id| !deleted.contains(id))
                .collect();
            let held_now = held(element, end);
            assert_eq!(
                held_now,
                expected,
                "{row}: {context}: the {} of {:?} is not its old list minus the deleted ids",
                end.name(),
                element.id()
            );
            lost |= held_now != held(old, end);
        }
        if lost {
            changed.push(element.id().clone());
        }
    }
    assert_eq!(
        mutation.changed_ids(),
        changed,
        "{row}: {context}: changed_ids() is not the surviving holders that lost ids, in pre-order"
    );
}

// ---------------------------------------------------------------------------
// LIB-EDIT-005: the operation and its policies
// ---------------------------------------------------------------------------

#[test]
fn lib_edit_005_delete_element_rejects_by_default_and_each_policy_is_set_by_a_builder() {
    let default = DeleteElement::new("c1");
    assert_eq!(
        default.element().as_str(),
        "c1",
        "{EDIT_005}: element() is not the id given"
    );
    assert_eq!(
        (
            default.reference_policy(),
            default.package_policy(),
            default.cross_package_policy()
        ),
        (
            RejectIfReferenced,
            RejectIfNonEmpty,
            RejectIfExternalReferencesExist
        ),
        "{EDIT_005}: the policies do not default to reject"
    );
    for reference in REFERENCE_POLICIES {
        for package in PACKAGE_POLICIES {
            for cross in CROSS_PACKAGE_POLICIES {
                let set = delete("c1", reference, package, cross);
                assert_eq!(
                    (
                        set.reference_policy(),
                        set.package_policy(),
                        set.cross_package_policy()
                    ),
                    (reference, package, cross),
                    "{EDIT_005}: the builders did not set the policies"
                );
                assert_eq!(
                    set.element().as_str(),
                    "c1",
                    "{EDIT_005}: a builder changed the element"
                );
            }
        }
    }
    // Each builder sets only its own policy.
    let only_package = DeleteElement::new("c1").with_package_policy(DeleteRecursively);
    assert_eq!(
        (
            only_package.reference_policy(),
            only_package.cross_package_policy()
        ),
        (RejectIfReferenced, RejectIfExternalReferencesExist),
        "{EDIT_005}: with_package_policy changed another policy"
    );
}

#[test]
fn lib_edit_005_delete_element_is_an_operation_named_delete_element() {
    let operation =
        Operation::from(DeleteElement::new("c1").with_reference_policy(ScrubReferences));
    assert_eq!(
        operation.operation_name(),
        "DeleteElement",
        "{EDIT_005}: operation_name() of a delete"
    );
    assert!(
        matches!(
            &operation,
            Operation::DeleteElement(inner) if inner.reference_policy() == ScrubReferences
        ),
        "{EDIT_005}: Operation::from does not hold the DeleteElement given"
    );
    assert_ne!(
        Operation::from(DeleteElement::new("c1")),
        operation,
        "{EDIT_005}: deletes under different policies are equal operations"
    );
}

#[test]
fn lib_edit_005_an_id_that_names_no_element_is_element_not_found_under_every_policy() {
    let text = case(&format!(
        r#"<argumentPackage xmi:id="p"><name xmi:id="p_name" content="P"/>{}</argumentPackage>"#,
        element(
            "Claim",
            "c1",
            "",
            r#"<description xmi:id="d1"><content xmi:id="d1_content"><value xmi:id="d1_value" content="D"/></content></description>"#,
        )
    ));
    let mut doc = read(EDIT_005, &text);
    for missing in ["nowhere", "p_name", "d1_content", "d1_value", "", "C1"] {
        for operation in every_policy(missing) {
            let context = format!("{operation:?}");
            let error = refused(EDIT_005, &context, &mut doc, operation);
            assert!(
                matches!(&error, EditError::ElementNotFound { element, .. } if element.as_str() == missing),
                "{EDIT_005}: {context}: the answer {error:?} is not ElementNotFound for {missing:?}"
            );
            assert_eq!(
                error.code(),
                Code::CMD_TARGET_NOT_FOUND,
                "{EDIT_005}: {context}: the code"
            );
        }
    }
}

/// One document per reference end: in the ArgumentPackage `p`, the element
/// `h` holds the id `t` in `end`, and nothing else references `t`.
fn holder_of(end: ReferenceEnd) -> String {
    let a = claim("a", "");
    let t = claim("t", "");
    let body = match end {
        ReferenceEnd::CitedElement => format!("{t}{}", claim("h", r#"citedElement="t""#)),
        ReferenceEnd::AbstractForm => format!("{t}{}", claim("h", r#"abstractForm="t""#)),
        ReferenceEnd::MetaClaim => format!("{t}{}", claim("h", r#"metaClaim="t""#)),
        ReferenceEnd::Reasoning => format!(
            "{a}{}{}",
            element("ArgumentReasoning", "t", "", ""),
            element(
                "AssertedInference",
                "h",
                r#"source="a" target="a" reasoning="t""#,
                ""
            )
        ),
        ReferenceEnd::Source => format!(
            "{a}{t}{}",
            element("AssertedInference", "h", r#"source="t" target="a""#, "")
        ),
        ReferenceEnd::Target => format!(
            "{a}{t}{}",
            element("AssertedInference", "h", r#"source="a" target="t""#, "")
        ),
        ReferenceEnd::Structure => format!(
            "{}{}",
            nested("t", ""),
            element("ArgumentReasoning", "h", r#"structure="t""#, "")
        ),
        ReferenceEnd::ReferencedArtifactElement => format!(
            "{t}{}",
            element(
                "ArtifactReference",
                "h",
                r#"referencedArtifactElement="t""#,
                ""
            )
        ),
        ReferenceEnd::GroupMember => format!(
            "{t}{}",
            element("ArgumentGroup", "h", r#"argumentElement="t""#, "")
        ),
        // An ArtifactGroup lives in an ArtifactPackage `p` (§12.3).
        ReferenceEnd::ArtifactGroupMember => {
            return case(&format!(
                r#"<artifactPackage xmi:id="p"><name content="p"/>{}{}</artifactPackage>"#,
                r#"<artifactElement xmi:type="sacm:Technique" xmi:id="t"><name content="t"/></artifactElement>"#,
                r#"<artifactElement xmi:type="sacm:ArtifactGroup" xmi:id="h" artifactElement="t"><name content="h"/></artifactElement>"#,
            ));
        }
        // An ArtifactAssetRelationship lives in an ArtifactPackage `p`, and
        // links ArtifactAssets (§12.14).
        ReferenceEnd::ArtifactSource | ReferenceEnd::ArtifactTarget => {
            let ends = if end == ReferenceEnd::ArtifactSource {
                r#"source="t" target="a""#
            } else {
                r#"source="a" target="t""#
            };
            return case(&format!(
                r#"<artifactPackage xmi:id="p"><name content="p"/>{}{}<artifactElement xmi:type="sacm:ArtifactAssetRelationship" xmi:id="h" {ends}><name content="h"/></artifactElement></artifactPackage>"#,
                r#"<artifactElement xmi:type="sacm:Technique" xmi:id="t"><name content="t"/></artifactElement>"#,
                r#"<artifactElement xmi:type="sacm:Activity" xmi:id="a"><name content="a"/></artifactElement>"#,
            ));
        }
        other => panic!("{EDIT_005}: an end this test does not know: {other:?}"),
    };
    case(&package("p", &body))
}

#[test]
fn lib_edit_005_reject_if_referenced_refuses_a_reference_through_every_end() {
    for end in ENDS {
        let mut doc = read(EDIT_005, &holder_of(end));
        // Under ApplyReferencePolicy every dangling reference meets the
        // reference policy.
        let context = format!("t referenced through {}, ApplyReferencePolicy", end.name());
        let error = refused(
            EDIT_005,
            &context,
            &mut doc,
            DeleteElement::new("t").with_cross_package_policy(ApplyReferencePolicy),
        );
        assert_eq!(
            named_reference(&error),
            Some(("Referenced", "h", end.name(), "t")),
            "{EDIT_005}: {context}: the answer {error:?} does not name the dangling reference"
        );
        assert_eq!(
            error.code(),
            Code::CMD_DELETE_REFERENCED,
            "{EDIT_005}: {context}: the code"
        );
        assert_eq!(
            error.code().as_str(),
            "SACM-CMD-002",
            "{EDIT_005}: {context}: the code's text"
        );
        // Under the defaults too, except for a `structure`: the
        // ArgumentPackage `t` is its own nearest package, so the reference
        // from `p` crosses a package boundary (LIB-EDIT-006).
        let context = format!("t referenced through {}, the defaults", end.name());
        let error = refused(EDIT_005, &context, &mut doc, DeleteElement::new("t"));
        let variant = if end == ReferenceEnd::Structure {
            "CrossPackageReference"
        } else {
            "Referenced"
        };
        assert_eq!(
            named_reference(&error),
            Some((variant, "h", end.name(), "t")),
            "{EDIT_005}: {context}: the answer {error:?}"
        );
    }
}

#[test]
fn lib_edit_005_the_refusal_names_the_first_dangling_reference_by_holder_then_end() {
    // `doomed` holds t1, t2 and a relationship between them, which goes with
    // them. In pre-order the group `g` holds the first dangling reference;
    // `h` holds a metaClaim, which comes before its sources and target.
    let doomed = format!(
        "{}{}{}",
        claim("t1", ""),
        claim("t2", ""),
        element(
            "AssertedInference",
            "inside",
            r#"source="t1" target="t2""#,
            ""
        )
    );
    let holder = element(
        "AssertedInference",
        "h",
        r#"target="t1" source="t2 t1" metaClaim="t2""#,
        "",
    );
    let group = element("ArgumentGroup", "g", r#"argumentElement="t1""#, "");
    let operation = delete(
        "doomed",
        RejectIfReferenced,
        DeleteRecursively,
        ApplyReferencePolicy,
    );

    let text = case(&format!(
        "{}{}",
        package("keep", &format!("{group}{holder}")),
        package("doomed", &doomed)
    ));
    let mut doc = read(EDIT_005, &text);
    let error = refused(EDIT_005, "g before h", &mut doc, operation.clone());
    assert_eq!(
        named_reference(&error),
        Some(("Referenced", "g", "argumentElement", "t1")),
        "{EDIT_005}: the first holder in pre-order is g: {error:?}"
    );

    let text = case(&format!(
        "{}{}",
        package("keep", &holder),
        package("doomed", &doomed)
    ));
    let mut doc = read(EDIT_005, &text);
    let error = refused(EDIT_005, "h alone", &mut doc, operation);
    assert_eq!(
        named_reference(&error),
        Some(("Referenced", "h", "metaClaim", "t2")),
        "{EDIT_005}: metaClaim comes before source and target: {error:?}"
    );
}

#[test]
fn lib_edit_005_the_references_an_element_holds_do_not_refuse_its_delete() {
    let text = case(&package(
        "p",
        &format!(
            "{}{}{}{}",
            claim("top", ""),
            claim("sub", r#"metaClaim="top""#),
            element("ArgumentReasoning", "why", "", ""),
            element(
                "AssertedInference",
                "inf",
                r#"source="sub" target="top" reasoning="why" metaClaim="top""#,
                &note("inf_note")
            )
        ),
    ));
    let mut doc = read(EDIT_005, &text);
    let mutation = accepted(EDIT_005, "inf", &mut doc, DeleteElement::new("inf"));
    assert_eq!(
        strs(mutation.deleted_ids()),
        ["inf", "inf_note"],
        "{EDIT_005}: deleted_ids()"
    );
    assert_eq!(
        mutation.changed_ids(),
        [] as [ElementId; 0],
        "{EDIT_005}: a delete nothing references changed ids"
    );
}

#[test]
fn lib_edit_005_an_unreferenced_element_is_deleted_and_the_parent_is_not_changed() {
    let text = case(&format!(
        r#"<argumentPackage xmi:id="p" gid="g-p"><name content="p"/>{}{}</argumentPackage>"#,
        claim("c1", ""),
        claim("c2", "")
    ));
    let mut doc = read(EDIT_005, &text);
    let before = doc.clone();
    let mutation = accepted(EDIT_005, "c1", &mut doc, DeleteElement::new("c1"));
    assert_eq!(
        strs(mutation.deleted_ids()),
        ["c1"],
        "{EDIT_005}: deleted_ids()"
    );
    assert_eq!(
        mutation.changed_ids(),
        [] as [ElementId; 0],
        "{EDIT_005}: the parent is reported changed"
    );
    let about_parent: Vec<Difference> = semantic_compare(&before, &doc)
        .into_iter()
        .filter(|difference| difference.element_id().map(ElementId::as_str) == Some("p"))
        .collect();
    assert_eq!(
        about_parent,
        [],
        "{EDIT_005}: the delete changed the parent"
    );
    let children: Vec<&str> = get(EDIT_005, &doc, "p")
        .children()
        .map(|child| child.id().as_str())
        .collect();
    assert_eq!(children, ["c2"], "{EDIT_005}: the parent's children");
    assert!(!doc.contains("c1"), "{EDIT_005}: c1 is still there");
}

#[test]
fn lib_edit_005_a_note_is_deleted_and_its_owner_is_not_changed() {
    let text = case(&package(
        "p",
        &element(
            "Claim",
            "c1",
            "",
            &format!(
                r#"<description xmi:id="d1"><content><value content="D"/></content></description>{}"#,
                note("n1")
            ),
        ),
    ));
    let mut doc = read(EDIT_005, &text);
    let mutation = accepted(EDIT_005, "n1", &mut doc, DeleteElement::new("n1"));
    assert_eq!(
        strs(mutation.deleted_ids()),
        ["n1"],
        "{EDIT_005}: deleted_ids()"
    );
    assert_eq!(
        mutation.changed_ids(),
        [] as [ElementId; 0],
        "{EDIT_005}: the owner is reported changed"
    );
    let left: Vec<&str> = get(EDIT_005, &doc, "c1")
        .children()
        .map(|child| child.id().as_str())
        .collect();
    assert_eq!(left, ["d1"], "{EDIT_005}: the owner's other children");
}

#[test]
fn lib_edit_005_a_root_is_deleted() {
    let text = format!(
        r#"<xmi:XMI {NAMESPACES}>{}<sacm:ArgumentPackage xmi:id="q"><name content="q"/>{}</sacm:ArgumentPackage></xmi:XMI>"#,
        case(&package("p", &claim("c1", ""))),
        note("q_note")
    );
    let mut doc = read(EDIT_005, &text);
    let mutation = accepted(EDIT_005, "q", &mut doc, DeleteElement::new("q"));
    assert_eq!(
        strs(mutation.deleted_ids()),
        ["q", "q_note"],
        "{EDIT_005}: a root holding only a Note is deleted under the defaults"
    );
    let roots: Vec<&str> = doc.roots().map(|root| root.id().as_str()).collect();
    assert_eq!(roots, ["case"], "{EDIT_005}: the roots after the delete");
}

// ---------------------------------------------------------------------------
// LIB-EDIT-005: DeleteReferencingRelationships
// ---------------------------------------------------------------------------

/// `inf` from `sub1` and `sub2` to `top` with the reasoning `why`, and
/// `other` from `sub2` to `top`, in `p`.
fn inference_document() -> String {
    case(&package(
        "p",
        &format!(
            "{}{}{}{}{}{}",
            claim("top", ""),
            element("Claim", "sub1", "", &note("sub1_note")),
            claim("sub2", ""),
            element("ArgumentReasoning", "why", "", ""),
            element(
                "AssertedInference",
                "inf",
                r#"source="sub1 sub2" target="top" reasoning="why""#,
                ""
            ),
            element(
                "AssertedInference",
                "other",
                r#"source="sub2" target="top""#,
                ""
            )
        ),
    ))
}

#[test]
fn lib_edit_005_delete_referencing_relationships_deletes_each_relationship_naming_a_deleted_source_target_or_reasoning()
 {
    for (target, deleted, context) in [
        (
            "sub1",
            vec!["sub1", "sub1_note", "inf"],
            "a source, though inf has another",
        ),
        ("top", vec!["top", "inf", "other"], "a target"),
        ("why", vec!["why", "inf"], "a reasoning"),
    ] {
        let mut doc = read(EDIT_005, &inference_document());
        let context = format!("{target}: {context}");
        let mutation = accepted(
            EDIT_005,
            &context,
            &mut doc,
            DeleteElement::new(target).with_reference_policy(DeleteReferencingRelationships),
        );
        assert_eq!(
            strs(mutation.deleted_ids()),
            deleted,
            "{EDIT_005}: {context}: deleted_ids()"
        );
        assert_eq!(
            mutation.changed_ids(),
            [] as [ElementId; 0],
            "{EDIT_005}: {context}: changed_ids()"
        );
    }
}

/// `r1` from `c1`; `r2` targets `r1`; `r3` has the sources `r2` and `c3`;
/// `r4` has the source `r2` only; the group `g` holds `r3` and `c3`; `keep`
/// names neither.
fn relationship_chain() -> String {
    case(&package(
        "p",
        &format!(
            "{}{}{}{}{}{}{}{}{}",
            claim("c1", ""),
            claim("c2", ""),
            claim("c3", ""),
            element("AssertedInference", "r1", r#"source="c1" target="c2""#, ""),
            element("AssertedContext", "r2", r#"source="c3" target="r1""#, ""),
            element(
                "AssertedInference",
                "r3",
                r#"source="r2 c3" target="c2""#,
                &note("r3_note")
            ),
            element("AssertedInference", "r4", r#"source="r2" target="c2""#, ""),
            element("ArgumentGroup", "g", r#"argumentElement="r3 c3""#, ""),
            element(
                "AssertedInference",
                "keep",
                r#"source="c3" target="c2""#,
                ""
            )
        ),
    ))
}

#[test]
fn lib_edit_005_delete_referencing_relationships_cascades_to_a_fixpoint() {
    let mut doc = read(EDIT_005, &relationship_chain());
    let mutation = accepted(
        EDIT_005,
        "c1",
        &mut doc,
        DeleteElement::new("c1").with_reference_policy(DeleteReferencingRelationships),
    );
    assert_eq!(
        strs(mutation.deleted_ids()),
        ["c1", "r1", "r2", "r3", "r3_note", "r4"],
        "{EDIT_005}: r1 names c1, r2 names r1, r3 and r4 name r2"
    );
    assert_eq!(
        strs(mutation.changed_ids()),
        ["g"],
        "{EDIT_005}: the group lost r3"
    );
    assert_eq!(
        strs(get(EDIT_005, &doc, "g").group_members()),
        ["c3"],
        "{EDIT_005}: the group's members"
    );
}

/// `c1` is held by every end that does not cascade: `holder`'s
/// `citedElement`, `abstractForm` and `metaClaim` (with `m`, repeated), the
/// `ArtifactReference` `art`, the group `g`, and the `metaClaim` of the
/// relationship `rel`, whose source and target are `m`.
fn non_cascading_holders() -> String {
    case(&package(
        "p",
        &format!(
            "{}{}{}{}{}{}",
            claim("c1", ""),
            claim("m", ""),
            claim(
                "holder",
                r#"citedElement="c1" abstractForm="c1" metaClaim="m c1 m c1""#
            ),
            element(
                "ArtifactReference",
                "art",
                r#"referencedArtifactElement="c1 m c1""#,
                ""
            ),
            element("ArgumentGroup", "g", r#"argumentElement="m c1""#, ""),
            element(
                "AssertedInference",
                "rel",
                r#"source="m" target="m" metaClaim="c1""#,
                ""
            )
        ),
    ))
}

#[test]
fn lib_edit_005_the_non_reject_policies_remove_every_other_dangling_id_from_its_end() {
    for policy in [DeleteReferencingRelationships, ScrubReferences] {
        let context = format!("c1 under {policy:?}");
        let mut doc = read(EDIT_005, &non_cascading_holders());
        let mutation = accepted(
            EDIT_005,
            &context,
            &mut doc,
            DeleteElement::new("c1").with_reference_policy(policy),
        );
        assert_eq!(
            strs(mutation.deleted_ids()),
            ["c1"],
            "{EDIT_005}: {context}: a metaClaim does not delete its relationship"
        );
        assert_eq!(
            strs(mutation.changed_ids()),
            ["holder", "art", "g", "rel"],
            "{EDIT_005}: {context}: changed_ids()"
        );
        let holder = get(EDIT_005, &doc, "holder");
        assert_eq!(
            (holder.cited_element(), holder.abstract_form()),
            (None, None),
            "{EDIT_005}: {context}: citedElement and abstractForm"
        );
        assert_eq!(
            strs(holder.meta_claims()),
            ["m", "m"],
            "{EDIT_005}: {context}: the metaClaim keeps its other ids, repeats included"
        );
        assert_eq!(
            strs(get(EDIT_005, &doc, "art").referenced_artifact_elements()),
            ["m"],
            "{EDIT_005}: {context}: referencedArtifactElement"
        );
        let rel = get(EDIT_005, &doc, "rel");
        assert_eq!(
            (rel.meta_claims(), strs(rel.sources()), rel.target()),
            (&[] as &[ElementId], vec!["m"], Some(&id("m"))),
            "{EDIT_005}: {context}: the relationship lost only its metaClaim"
        );
    }
}

#[test]
fn lib_edit_005_the_non_reject_policies_remove_a_deleted_structure() {
    let text = case(&package(
        "p",
        &format!(
            "{}{}",
            nested("sp", ""),
            element("ArgumentReasoning", "why", r#"structure="sp""#, "")
        ),
    ));
    for policy in [DeleteReferencingRelationships, ScrubReferences] {
        let context = format!("sp under {policy:?}");
        let mut doc = read(EDIT_005, &text);
        let mutation = accepted(
            EDIT_005,
            &context,
            &mut doc,
            DeleteElement::new("sp")
                .with_reference_policy(policy)
                .with_cross_package_policy(ApplyReferencePolicy),
        );
        assert_eq!(
            (strs(mutation.deleted_ids()), strs(mutation.changed_ids())),
            (vec!["sp"], vec!["why"]),
            "{EDIT_005}: {context}: the ids"
        );
        assert_eq!(
            get(EDIT_005, &doc, "why").structure(),
            None,
            "{EDIT_005}: {context}: the structure"
        );
    }
}

#[test]
fn lib_edit_005_an_argument_group_that_loses_its_last_member_keeps_existing() {
    let text = case(&package(
        "p",
        &format!(
            "{}{}",
            claim("c1", ""),
            element("ArgumentGroup", "g", r#"argumentElement="c1""#, "")
        ),
    ));
    for policy in [DeleteReferencingRelationships, ScrubReferences] {
        let context = format!("c1 under {policy:?}");
        let mut doc = read(EDIT_005, &text);
        let mutation = accepted(
            EDIT_005,
            &context,
            &mut doc,
            DeleteElement::new("c1").with_reference_policy(policy),
        );
        assert_eq!(
            (strs(mutation.deleted_ids()), strs(mutation.changed_ids())),
            (vec!["c1"], vec!["g"]),
            "{EDIT_005}: {context}: the group is changed, not deleted"
        );
        let group = get(EDIT_005, &doc, "g");
        assert_eq!(
            (group.kind(), group.group_members()),
            (ElementKind::ArgumentGroup, &[] as &[ElementId]),
            "{EDIT_005}: {context}: the group exists with no members"
        );
    }
}

#[test]
fn lib_edit_005_deleted_ids_are_in_pre_order_before_and_changed_ids_in_pre_order_after() {
    // `early` and `g_early` come before `t` in pre-order, `late` and
    // `g_late` after it.
    let text = case(&package(
        "p",
        &format!(
            "{}{}{}{}{}{}",
            element(
                "AssertedInference",
                "early",
                r#"source="t" target="c2""#,
                ""
            ),
            element("ArgumentGroup", "g_early", r#"argumentElement="t""#, ""),
            element("Claim", "t", "", &note("t_note")),
            claim("c2", ""),
            element("AssertedInference", "late", r#"source="c2" target="t""#, ""),
            element("ArgumentGroup", "g_late", r#"argumentElement="t c2""#, "")
        ),
    ));
    let mut doc = read(EDIT_005, &text);
    let mutation = accepted(
        EDIT_005,
        "t",
        &mut doc,
        DeleteElement::new("t").with_reference_policy(DeleteReferencingRelationships),
    );
    assert_eq!(
        strs(mutation.deleted_ids()),
        ["early", "t", "t_note", "late"],
        "{EDIT_005}: deleted_ids() in pre-order before, not the element first"
    );
    assert_eq!(
        strs(mutation.changed_ids()),
        ["g_early", "g_late"],
        "{EDIT_005}: changed_ids() in pre-order after"
    );
}

// ---------------------------------------------------------------------------
// LIB-EDIT-005: ScrubReferences
// ---------------------------------------------------------------------------

#[test]
fn lib_edit_005_scrub_references_keeps_a_relationship_that_still_has_a_source_and_a_target() {
    let mut doc = read(EDIT_005, &inference_document());
    let mutation = accepted(
        EDIT_005,
        "sub1",
        &mut doc,
        DeleteElement::new("sub1").with_reference_policy(ScrubReferences),
    );
    assert_eq!(
        (strs(mutation.deleted_ids()), strs(mutation.changed_ids())),
        (vec!["sub1", "sub1_note"], vec!["inf"]),
        "{EDIT_005}: inf keeps sub2 as its source"
    );
    let inf = get(EDIT_005, &doc, "inf");
    assert_eq!(
        (strs(inf.sources()), inf.target(), inf.reasoning()),
        (vec!["sub2"], Some(&id("top")), Some(&id("why"))),
        "{EDIT_005}: the ends of inf"
    );
}

#[test]
fn lib_edit_005_scrub_references_removes_a_deleted_reasoning_and_keeps_the_relationship() {
    let mut doc = read(EDIT_005, &inference_document());
    let mutation = accepted(
        EDIT_005,
        "why",
        &mut doc,
        DeleteElement::new("why").with_reference_policy(ScrubReferences),
    );
    assert_eq!(
        (strs(mutation.deleted_ids()), strs(mutation.changed_ids())),
        (vec!["why"], vec!["inf"]),
        "{EDIT_005}: the ids"
    );
    let inf = get(EDIT_005, &doc, "inf");
    assert_eq!(
        (strs(inf.sources()), inf.reasoning()),
        (vec!["sub1", "sub2"], None),
        "{EDIT_005}: inf lost its reasoning only"
    );
}

#[test]
fn lib_edit_005_scrub_references_deletes_a_relationship_left_without_a_source() {
    // `lone` keeps its reasoning, and is deleted all the same (§11.13
    // `source:ArgumentAsset[1..*]`; ADR 0031 R1).
    let text = case(&package(
        "p",
        &format!(
            "{}{}{}{}",
            claim("top", ""),
            claim("sub1", ""),
            element("ArgumentReasoning", "why", "", ""),
            element(
                "AssertedInference",
                "lone",
                r#"source="sub1 sub1" target="top" reasoning="why""#,
                &note("lone_note")
            )
        ),
    ));
    let mut doc = read(EDIT_005, &text);
    let mutation = accepted(
        EDIT_005,
        "sub1",
        &mut doc,
        DeleteElement::new("sub1").with_reference_policy(ScrubReferences),
    );
    assert_eq!(
        (strs(mutation.deleted_ids()), strs(mutation.changed_ids())),
        (vec!["sub1", "lone", "lone_note"], Vec::<&str>::new()),
        "{EDIT_005}: lone has no source left"
    );
}

#[test]
fn lib_edit_005_scrub_references_deletes_a_relationship_left_without_a_target() {
    let mut doc = read(EDIT_005, &inference_document());
    let mutation = accepted(
        EDIT_005,
        "top",
        &mut doc,
        DeleteElement::new("top").with_reference_policy(ScrubReferences),
    );
    assert_eq!(
        (strs(mutation.deleted_ids()), strs(mutation.changed_ids())),
        (vec!["top", "inf", "other"], Vec::<&str>::new()),
        "{EDIT_005}: inf and other have no target left (§11.13 `target:ArgumentAsset[1]`)"
    );
}

#[test]
fn lib_edit_005_scrub_references_cascades_to_a_fixpoint() {
    let mut doc = read(EDIT_005, &relationship_chain());
    let mutation = accepted(
        EDIT_005,
        "c1",
        &mut doc,
        DeleteElement::new("c1").with_reference_policy(ScrubReferences),
    );
    assert_eq!(
        strs(mutation.deleted_ids()),
        ["c1", "r1", "r2", "r4"],
        "{EDIT_005}: r1 loses its only source, r2 its target, r4 its only source"
    );
    assert_eq!(
        strs(mutation.changed_ids()),
        ["r3"],
        "{EDIT_005}: r3 keeps c3 as its source"
    );
    assert_eq!(
        strs(get(EDIT_005, &doc, "r3").sources()),
        ["c3"],
        "{EDIT_005}: the sources of r3"
    );
}

#[test]
fn lib_edit_005_scrub_references_keeps_a_relationship_read_without_a_source_when_another_end_loses_an_id()
 {
    // `ac` was read without a source. Scrubbing its metaClaim does not leave
    // it without one: it had none (D-VAL-LAYER: the finding stays).
    let text = case(&package(
        "p",
        &format!(
            "{}{}{}",
            claim("c2", ""),
            claim("m", ""),
            element(
                "AssertedArtifactContext",
                "ac",
                r#"target="c2" metaClaim="m""#,
                ""
            )
        ),
    ));
    for policy in [DeleteReferencingRelationships, ScrubReferences] {
        let context = format!("m under {policy:?}");
        let mut doc = read(EDIT_005, &text);
        let mutation = accepted(
            EDIT_005,
            &context,
            &mut doc,
            DeleteElement::new("m").with_reference_policy(policy),
        );
        assert_eq!(
            (strs(mutation.deleted_ids()), strs(mutation.changed_ids())),
            (vec!["m"], vec!["ac"]),
            "{EDIT_005}: {context}: ac is changed, not deleted"
        );
        let ac = get(EDIT_005, &doc, "ac");
        assert_eq!(
            (ac.sources(), ac.target()),
            (&[] as &[ElementId], Some(&id("c2"))),
            "{EDIT_005}: {context}: the ends of ac"
        );
    }
}

// ---------------------------------------------------------------------------
// LIB-EDIT-005: references that dangled before, and the id counter
// ---------------------------------------------------------------------------

#[test]
fn lib_edit_005_references_that_dangled_before_the_delete_are_unchanged() {
    // `ghost` names no element: read from the file, it dangles before the
    // delete and must after it.
    let text = case(&package(
        "p",
        &format!(
            "{}{}{}{}{}{}",
            claim("c1", ""),
            claim("c2", ""),
            claim("free", ""),
            claim("h", r#"metaClaim="ghost c1 ghost" citedElement="ghost""#),
            element("ArgumentGroup", "g", r#"argumentElement="ghost c1""#, ""),
            element(
                "AssertedInference",
                "r",
                r#"source="ghost c1" target="c2""#,
                ""
            )
        ),
    ));
    for policy in REFERENCE_POLICIES {
        let context = format!("free under {policy:?}");
        let mut doc = read(EDIT_005, &text);
        let mutation = accepted(
            EDIT_005,
            &context,
            &mut doc,
            DeleteElement::new("free").with_reference_policy(policy),
        );
        assert_eq!(
            (strs(mutation.deleted_ids()), strs(mutation.changed_ids())),
            (vec!["free"], Vec::<&str>::new()),
            "{EDIT_005}: {context}: a dangling id is not touched"
        );
    }
    let mut doc = read(EDIT_005, &text);
    let mutation = accepted(
        EDIT_005,
        "c1 scrubbed",
        &mut doc,
        DeleteElement::new("c1").with_reference_policy(ScrubReferences),
    );
    assert_eq!(
        (strs(mutation.deleted_ids()), strs(mutation.changed_ids())),
        (vec!["c1"], vec!["h", "g", "r"]),
        "{EDIT_005}: r keeps the dangling source ghost, so it is not deleted"
    );
    let h = get(EDIT_005, &doc, "h");
    assert_eq!(
        (strs(h.meta_claims()), h.cited_element()),
        (vec!["ghost", "ghost"], Some(&id("ghost"))),
        "{EDIT_005}: the dangling ids of h"
    );
    assert_eq!(
        strs(get(EDIT_005, &doc, "g").group_members()),
        ["ghost"],
        "{EDIT_005}: the dangling member of g"
    );
    assert_eq!(
        strs(get(EDIT_005, &doc, "r").sources()),
        ["ghost"],
        "{EDIT_005}: the dangling source of r"
    );
    let mut doc = read(EDIT_005, &text);
    let mutation = accepted(
        EDIT_005,
        "c1 cascaded",
        &mut doc,
        DeleteElement::new("c1").with_reference_policy(DeleteReferencingRelationships),
    );
    assert_eq!(
        (strs(mutation.deleted_ids()), strs(mutation.changed_ids())),
        (vec!["c1", "r"], vec!["h", "g"]),
        "{EDIT_005}: r names c1 as a source, so it goes"
    );
}

/// The tolerant write of `doc`.
fn tolerant_write(row: &str, context: &str, doc: &Document) -> String {
    match xmi::write(doc, &WriteOptions::default().with_mode(Mode::Tolerant)) {
        Ok(out) => out,
        Err(error) => panic!("{row}: {context}: the tolerant write failed: {error:?}"),
    }
}

#[test]
fn lib_edit_005_an_id_named_only_inside_preserved_content_is_not_refused_listed_or_scrubbed() {
    // Preserved content is opaque (ADR 0033): an `xmi:idref`, an `href` or
    // an attribute of a preserved fragment that names c2 is no typed
    // reference, so deleting c2 is not refused, lists no dangling reference,
    // changes no holder, and the fragment is written back as read.
    const V: &str = r#"xmlns:v="urn:example:vendor""#;
    let idref = format!(r#"<v:link {V} xmi:idref="c2" />"#);
    let href = format!(r##"<v:link {V} href="#c2" />"##);
    let cases = [
        (
            "a preserved <v:link xmi:idref=\"c2\"/>",
            element("Claim", "c1", "", &idref),
            idref.clone(),
        ),
        (
            "a preserved <v:link href=\"#c2\"/>",
            element("Claim", "c1", "", &href),
            href.clone(),
        ),
        (
            "a preserved attribute v:ref=\"c2\"",
            element("Claim", "c1", &format!(r#"{V} v:ref="c2""#), ""),
            r#"v:ref="c2""#.to_owned(),
        ),
    ];
    for (what, holder, fragment) in &cases {
        let text = case(&package("p", &format!("{holder}{}", claim("c2", ""))));
        let read_doc = read(EDIT_005, &text);
        let written = tolerant_write(EDIT_005, what, &read_doc);
        assert!(
            get(EDIT_005, &read_doc, "c1").has_preserved_content()
                && written.matches(fragment.as_str()).count() == 1,
            "{EDIT_005}: setup: {what}: c1 does not hold the fragment, written once as read, \
             {fragment:?}, in\n{written}"
        );
        for policy in REFERENCE_POLICIES {
            let context = format!("{what}: c2 under {policy:?}");
            let mut doc = read_doc.clone();
            let operation = Operation::from(DeleteElement::new("c2").with_reference_policy(policy));
            let preview = match edit::preview(&doc, &operation) {
                Ok(preview) => preview,
                Err(error) => panic!("{EDIT_005}: {context}: the preview was refused: {error}"),
            };
            assert!(
                preview.dangling_references().is_empty(),
                "{EDIT_005}: {context}: the preview lists dangling references: {:?}",
                preview.dangling_references()
            );
            let mutation = accepted(EDIT_005, &context, &mut doc, operation);
            assert_eq!(
                (strs(mutation.deleted_ids()), strs(mutation.changed_ids())),
                (vec!["c2"], Vec::<&str>::new()),
                "{EDIT_005}: {context}: c2 alone is deleted and no holder is changed"
            );
            let out = tolerant_write(EDIT_005, &context, &doc);
            assert!(
                get(EDIT_005, &doc, "c1").has_preserved_content()
                    && out.matches(fragment.as_str()).count() == 1
                    && !out.contains(r#"xmi:id="c2""#),
                "{EDIT_005}: {context}: the tolerant write does not hold the fragment {fragment:?} \
                 as read, once, without c2, in\n{out}"
            );
        }
    }
}

/// A delete does not rewind the per-kind counter of generated ids (ADR
/// 0031): after deleting a generated element, the next create of that kind
/// gets the next number above the counter. Nothing more is promised about
/// the ids a delete frees.
#[test]
fn lib_edit_005_a_delete_does_not_rewind_the_id_counter() {
    let mut doc = Document::new();
    let created = |doc: &mut Document, operation: Operation| -> String {
        let mutation = accepted(EDIT_005, "setup", doc, operation);
        match mutation.created_ids() {
            [only] => only.as_str().to_owned(),
            other => panic!("{EDIT_005}: setup: created {other:?}"),
        }
    };
    let case = created(
        &mut doc,
        CreateAssuranceCasePackage::root(named("Case")).into(),
    );
    let arguments = created(
        &mut doc,
        CreateArgumentPackage::new(case.as_str(), named("Arguments")).into(),
    );
    let first = created(
        &mut doc,
        CreateClaim::new(arguments.as_str(), named("One")).into(),
    );
    let second = created(
        &mut doc,
        CreateClaim::new(arguments.as_str(), named("Two")).into(),
    );
    assert_eq!(
        (
            case.as_str(),
            arguments.as_str(),
            first.as_str(),
            second.as_str()
        ),
        (
            "assurance_case_package_1",
            "argument_package_1",
            "claim_1",
            "claim_2"
        ),
        "{EDIT_005}: setup: the generated ids"
    );

    accepted(EDIT_005, "claim_2", &mut doc, DeleteElement::new("claim_2"));
    let third = created(
        &mut doc,
        CreateClaim::new(arguments.as_str(), named("Three")).into(),
    );
    assert_eq!(
        third, "claim_3",
        "{EDIT_005}: deleting claim_2, the highest generated, rewound the Claim counter"
    );
    accepted(EDIT_005, "claim_1", &mut doc, DeleteElement::new("claim_1"));
    let fourth = created(
        &mut doc,
        CreateClaim::new(arguments.as_str(), named("Four")).into(),
    );
    assert_eq!(
        fourth, "claim_4",
        "{EDIT_005}: deleting claim_1 rewound the Claim counter"
    );
    // A caller may give the id of a deleted element: it is not in use.
    let given = created(
        &mut doc,
        CreateClaim::new(arguments.as_str(), named("Given"))
            .with_id("claim_2")
            .into(),
    );
    assert_eq!(
        given, "claim_2",
        "{EDIT_005}: the id of a deleted element, given by the caller"
    );

    accepted(
        EDIT_005,
        "the root",
        &mut doc,
        DeleteElement::new(case.as_str()).with_package_policy(DeleteRecursively),
    );
    assert_eq!(
        doc.element_count(),
        0,
        "{EDIT_005}: the root's subtree went"
    );
    let case_again = created(
        &mut doc,
        CreateAssuranceCasePackage::root(named("Case")).into(),
    );
    let arguments_again = created(
        &mut doc,
        CreateArgumentPackage::new(case_again.as_str(), named("Arguments")).into(),
    );
    let claim_again = created(
        &mut doc,
        CreateClaim::new(arguments_again.as_str(), named("Again")).into(),
    );
    assert_eq!(
        (
            case_again.as_str(),
            arguments_again.as_str(),
            claim_again.as_str()
        ),
        ("assurance_case_package_2", "argument_package_2", "claim_5"),
        "{EDIT_005}: a recursive delete rewound the counters"
    );
}

// ---------------------------------------------------------------------------
// LIB-EDIT-006: non-empty elements
// ---------------------------------------------------------------------------

/// The package `p` holds `c1` (with a Description and a Note), `c2` and the
/// nested package `n`, which holds `c3` (with a Note); `q` holds `far`, which
/// cites `c1`.
fn non_empty_package() -> String {
    case(&format!(
        "{}{}",
        package(
            "p",
            &format!(
                "{}{}{}",
                element(
                    "Claim",
                    "c1",
                    "",
                    &format!(
                        r#"<description xmi:id="c1_d"><content><value content="D"/></content></description>{}"#,
                        note("c1_note")
                    )
                ),
                claim("c2", ""),
                nested("n", &element("Claim", "c3", "", &note("c3_note")))
            )
        ),
        package("q", &claim("far", r#"citedElement="c1""#))
    ))
}

#[test]
fn lib_edit_006_reject_if_non_empty_refuses_a_package_with_its_count_of_contained_elements() {
    let mut doc = read(EDIT_006, &non_empty_package());
    for reference in REFERENCE_POLICIES {
        for cross in CROSS_PACKAGE_POLICIES {
            let operation = delete("p", reference, RejectIfNonEmpty, cross);
            let context = format!("{operation:?}");
            let error = refused(EDIT_006, &context, &mut doc, operation);
            assert!(
                matches!(
                    &error,
                    EditError::NotEmpty { element, contained, .. }
                        if element.as_str() == "p" && *contained == 4
                ),
                "{EDIT_006}: {context}: c1, c2, n and c3 count, the UtilityElements do not: {error:?}"
            );
            assert_eq!(
                error.code(),
                Code::CMD_PACKAGE_NOT_EMPTY,
                "{EDIT_006}: {context}: the code"
            );
            assert_eq!(
                error.code().as_str(),
                "SACM-CMD-006",
                "{EDIT_006}: {context}: the code's text"
            );
        }
    }
    let error = refused(EDIT_006, "n", &mut doc, DeleteElement::new("n"));
    assert!(
        matches!(&error, EditError::NotEmpty { element, contained, .. } if element.as_str() == "n" && *contained == 1),
        "{EDIT_006}: the nested package holds c3: {error:?}"
    );
}

#[test]
fn lib_edit_006_reject_if_non_empty_refuses_any_element_that_contains_a_model_element() {
    // An Artifact holding a Property (§12.6), inserted: the reader does not
    // read artifacts yet.
    let mut doc = Document::new();
    let insert = |doc: &mut Document, parent: Option<&str>, kind: ElementKind, value: &str| {
        let parent = parent.map(ElementId::from);
        if let Err(error) = doc.insert(parent.as_ref(), kind, id(value), None) {
            panic!("{EDIT_006}: setup could not insert the {kind} {value:?}: {error}");
        }
    };
    insert(&mut doc, None, ElementKind::ArtifactPackage, "arts");
    insert(&mut doc, Some("arts"), ElementKind::Artifact, "a1");
    insert(&mut doc, Some("a1"), ElementKind::Property, "prop");
    insert(&mut doc, Some("a1"), ElementKind::Note, "a1_note");
    let error = refused(EDIT_006, "a1", &mut doc, DeleteElement::new("a1"));
    assert!(
        matches!(&error, EditError::NotEmpty { element, contained, .. } if element.as_str() == "a1" && *contained == 1),
        "{EDIT_006}: the Artifact holds a Property: {error:?}"
    );
    let mutation = accepted(
        EDIT_006,
        "a1 recursively",
        &mut doc,
        DeleteElement::new("a1").with_package_policy(DeleteRecursively),
    );
    let deleted: BTreeSet<&str> = mutation
        .deleted_ids()
        .iter()
        .map(ElementId::as_str)
        .collect();
    assert_eq!(
        (
            mutation.deleted_ids().first().map(ElementId::as_str),
            deleted
        ),
        (Some("a1"), BTreeSet::from(["a1", "prop", "a1_note"])),
        "{EDIT_006}: the Artifact went with its contents"
    );
}

#[test]
fn lib_edit_006_an_element_holding_only_utility_elements_is_not_refused() {
    let utilities = format!(
        r#"<description xmi:id="d"><content><value content="D"/></content></description><implementationConstraint xmi:id="ic"><content><value content="I"/></content></implementationConstraint>{}<taggedValue xmi:id="tv"><key><value content="K"/></key><content><value content="V"/></content></taggedValue>"#,
        note("n")
    );
    let p2 = package("p2", &note("p2_note"));
    let text = case(&format!(
        "{}{p2}",
        package("p", &element("Claim", "c1", "", &utilities))
    ));
    let mut doc = read(EDIT_006, &text);
    let mutation = accepted(EDIT_006, "c1", &mut doc, DeleteElement::new("c1"));
    let deleted: BTreeSet<&str> = mutation
        .deleted_ids()
        .iter()
        .map(ElementId::as_str)
        .collect();
    assert_eq!(
        (
            mutation.deleted_ids().first().map(ElementId::as_str),
            deleted
        ),
        (Some("c1"), BTreeSet::from(["c1", "d", "ic", "n", "tv"])),
        "{EDIT_006}: a Claim holding only UtilityElements is deleted with them under RejectIfNonEmpty"
    );
    let mutation = accepted(EDIT_006, "p2", &mut doc, DeleteElement::new("p2"));
    assert_eq!(
        strs(mutation.deleted_ids()),
        ["p2", "p2_note"],
        "{EDIT_006}: a package holding only a Note is deleted under RejectIfNonEmpty"
    );
    let mutation = accepted(
        EDIT_006,
        "an empty package",
        &mut doc,
        DeleteElement::new("p"),
    );
    assert_eq!(
        strs(mutation.deleted_ids()),
        ["p"],
        "{EDIT_006}: an empty package is deleted under RejectIfNonEmpty"
    );
}

#[test]
fn lib_edit_006_delete_recursively_deletes_the_whole_subtree_and_references_within_it_do_not_refuse()
 {
    let text = case(&package(
        "p",
        &format!(
            "{}{}{}{}",
            element("Claim", "c1", "", &note("c1_note")),
            claim("c2", r#"metaClaim="c1""#),
            element("AssertedInference", "inf", r#"source="c1" target="c2""#, ""),
            nested(
                "n",
                &format!(
                    "{}{}",
                    claim("c3", r#"citedElement="c1""#),
                    element("ArgumentGroup", "g", r#"argumentElement="c2 c3""#, "")
                )
            )
        ),
    ));
    let mut doc = read(EDIT_006, &text);
    let everything: Vec<String> = doc
        .elements()
        .map(|element| element.id().as_str().to_owned())
        .collect();
    let mutation = accepted(
        EDIT_006,
        "the root, every other policy rejecting",
        &mut doc,
        DeleteElement::new("case").with_package_policy(DeleteRecursively),
    );
    assert_eq!(
        mutation
            .deleted_ids()
            .iter()
            .map(ElementId::as_str)
            .collect::<Vec<_>>(),
        everything,
        "{EDIT_006}: the whole subtree, in pre-order; references inside it are not dangling"
    );
    assert_eq!(
        (doc.element_count(), doc.roots().count()),
        (0, 0),
        "{EDIT_006}: the document is empty"
    );

    let mut doc = read(EDIT_006, &text);
    let mutation = accepted(
        EDIT_006,
        "p",
        &mut doc,
        DeleteElement::new("p").with_package_policy(DeleteRecursively),
    );
    assert_eq!(
        strs(mutation.deleted_ids()),
        ["p", "c1", "c1_note", "c2", "inf", "n", "c3", "g"],
        "{EDIT_006}: the package p and everything it contains"
    );
    let roots: Vec<&str> = doc.roots().map(|root| root.id().as_str()).collect();
    assert_eq!(roots, ["case"], "{EDIT_006}: the root stays");
}

// ---------------------------------------------------------------------------
// LIB-EDIT-006: package boundaries
// ---------------------------------------------------------------------------

/// `c1` in `p`, held by the group `g` and the relationship `r` (from `c1` and
/// `cq` to `cq`) in the sibling package `q`.
fn sibling_references() -> String {
    case(&format!(
        "{}{}",
        package("p", &claim("c1", "")),
        package(
            "q",
            &format!(
                "{}{}{}",
                claim("cq", ""),
                element("ArgumentGroup", "g", r#"argumentElement="c1""#, ""),
                element(
                    "AssertedInference",
                    "r",
                    r#"source="c1 cq" target="cq""#,
                    ""
                )
            )
        )
    ))
}

#[test]
fn lib_edit_006_reject_if_external_references_exist_refuses_a_crossing_reference_with_cmd_007() {
    let mut doc = read(EDIT_006, &sibling_references());
    for reference in REFERENCE_POLICIES {
        for package_policy in PACKAGE_POLICIES {
            let operation = delete(
                "c1",
                reference,
                package_policy,
                RejectIfExternalReferencesExist,
            );
            let context = format!("{operation:?}");
            let error = refused(EDIT_006, &context, &mut doc, operation);
            assert_eq!(
                named_reference(&error),
                Some(("CrossPackageReference", "g", "argumentElement", "c1")),
                "{EDIT_006}: {context}: the first crossing reference: {error:?}"
            );
            assert_eq!(
                error.code(),
                Code::CMD_EXTERNAL_REFERENCES,
                "{EDIT_006}: {context}: the code"
            );
            assert_eq!(
                error.code().as_str(),
                "SACM-CMD-007",
                "{EDIT_006}: {context}: the code's text"
            );
        }
    }
}

#[test]
fn lib_edit_006_apply_reference_policy_hands_crossing_references_to_the_reference_policy() {
    let mut doc = read(EDIT_006, &sibling_references());
    let error = refused(
        EDIT_006,
        "RejectIfReferenced",
        &mut doc,
        DeleteElement::new("c1").with_cross_package_policy(ApplyReferencePolicy),
    );
    assert_eq!(
        named_reference(&error),
        Some(("Referenced", "g", "argumentElement", "c1")),
        "{EDIT_006}: RejectIfReferenced refuses the crossing reference as SACM-CMD-002: {error:?}"
    );
    for (policy, deleted, changed) in [
        (DeleteReferencingRelationships, vec!["c1", "r"], vec!["g"]),
        (ScrubReferences, vec!["c1"], vec!["g", "r"]),
    ] {
        let context = format!("{policy:?}");
        let mut doc = read(EDIT_006, &sibling_references());
        let mutation = accepted(
            EDIT_006,
            &context,
            &mut doc,
            DeleteElement::new("c1")
                .with_reference_policy(policy)
                .with_cross_package_policy(ApplyReferencePolicy),
        );
        assert_eq!(
            (strs(mutation.deleted_ids()), strs(mutation.changed_ids())),
            (deleted, changed),
            "{EDIT_006}: {context}: the crossing references are handled by the reference policy"
        );
    }
}

/// `c1` and `same` (whose metaClaim is `c1`) in `p`; `n`, nested in `p`,
/// holds `c_n`, which cites `c1`, and `h_n`, whose metaClaim is `c_n2`, and
/// `c_n2`; `p` also holds `h_p`, whose metaClaim is `c_n`.
fn nested_references() -> String {
    case(&package(
        "p",
        &format!(
            "{}{}{}{}",
            claim("c1", ""),
            claim("same", r#"metaClaim="c1""#),
            claim("h_p", r#"metaClaim="c_n""#),
            nested(
                "n",
                &format!(
                    "{}{}{}",
                    claim("c_n", r#"citedElement="c1""#),
                    claim("h_n", r#"metaClaim="c_n2""#),
                    claim("c_n2", "")
                )
            )
        ),
    ))
}

#[test]
fn lib_edit_006_a_nested_package_is_a_different_package() {
    let mut doc = read(EDIT_006, &nested_references());
    // c1 is held by `same` in p (not crossing) and by `c_n` in n (crossing).
    // `same` comes first, so RejectIfReferenced refuses it as SACM-CMD-002;
    // under DeleteReferencingRelationships only the crossing one is refused.
    let error = refused(EDIT_006, "c1", &mut doc, DeleteElement::new("c1"));
    assert_eq!(
        named_reference(&error),
        Some(("Referenced", "same", "metaClaim", "c1")),
        "{EDIT_006}: same and c1 share the package p: {error:?}"
    );
    let error = refused(
        EDIT_006,
        "c1, cascading",
        &mut doc,
        DeleteElement::new("c1").with_reference_policy(DeleteReferencingRelationships),
    );
    assert_eq!(
        named_reference(&error),
        Some(("CrossPackageReference", "c_n", "citedElement", "c1")),
        "{EDIT_006}: c_n is in the nested package n, c1 in p: {error:?}"
    );
    let error = refused(EDIT_006, "c_n", &mut doc, DeleteElement::new("c_n"));
    assert_eq!(
        named_reference(&error),
        Some(("CrossPackageReference", "h_p", "metaClaim", "c_n")),
        "{EDIT_006}: h_p is in p, c_n in the nested package n: {error:?}"
    );
    let error = refused(EDIT_006, "c_n2", &mut doc, DeleteElement::new("c_n2"));
    assert_eq!(
        named_reference(&error),
        Some(("Referenced", "h_n", "metaClaim", "c_n2")),
        "{EDIT_006}: h_n and c_n2 share the nested package n: {error:?}"
    );
}

/// Inserted (the reader does not read interfaces and bindings yet): the
/// root `case` holds the `ArgumentPackage` `ap` (holding `c_ap`), the
/// `ArgumentPackageInterface` `iface` (holding `c_if` and `c_if2`) and the
/// `ArgumentPackageBinding` `bind` (holding `c_b`). With `edit::apply`:
/// `case` and `ap` cite `c_ap`; `c_ap` and `c_b` have the metaClaim `c_if`;
/// `c_if` has the metaClaim `c_if2`.
fn interface_and_binding() -> Document {
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
        if let Err(error) = doc.insert(parent.as_ref(), kind, id(value), Some(named(value))) {
            panic!("{EDIT_006}: setup could not insert the {kind} {value:?}: {error}");
        }
    }
    for (holder, end, target) in [
        ("case", ReferenceEnd::CitedElement, "c_ap"),
        ("ap", ReferenceEnd::CitedElement, "c_ap"),
        ("c_ap", ReferenceEnd::MetaClaim, "c_if"),
        ("c_b", ReferenceEnd::MetaClaim, "c_if"),
        ("c_if", ReferenceEnd::MetaClaim, "c_if2"),
    ] {
        if let Err(error) = edit::apply(&mut doc, SetReferences::new(holder, end, [target])) {
            panic!(
                "{EDIT_006}: setup could not set the {} of {holder}: {error}",
                end.name()
            );
        }
    }
    doc
}

#[test]
fn lib_edit_006_the_nearest_package_is_the_element_itself_or_its_closest_package_interface_or_binding_ancestor()
 {
    let mut doc = interface_and_binding();
    // c_ap: `case` is its own nearest package, an AssuranceCasePackage, not
    // `ap`, so its citation crosses; `ap` cites from within itself.
    let error = refused(EDIT_006, "c_ap", &mut doc, DeleteElement::new("c_ap"));
    assert_eq!(
        named_reference(&error),
        Some(("CrossPackageReference", "case", "citedElement", "c_ap")),
        "{EDIT_006}: case and ap are different packages: {error:?}"
    );
    let error = refused(
        EDIT_006,
        "c_ap, cascading",
        &mut doc,
        DeleteElement::new("c_ap").with_reference_policy(DeleteReferencingRelationships),
    );
    assert_eq!(
        named_reference(&error),
        Some(("CrossPackageReference", "case", "citedElement", "c_ap")),
        "{EDIT_006}: the crossing citation is refused under any reference policy: {error:?}"
    );
    let error = refused(
        EDIT_006,
        "c_ap, crossing applied",
        &mut doc,
        DeleteElement::new("c_ap").with_cross_package_policy(ApplyReferencePolicy),
    );
    assert_eq!(
        named_reference(&error),
        Some(("Referenced", "case", "citedElement", "c_ap")),
        "{EDIT_006}: {error:?}"
    );
    // c_if is held from the package ap and from the binding bind.
    let error = refused(
        EDIT_006,
        "c_if, cascading",
        &mut doc,
        DeleteElement::new("c_if").with_reference_policy(DeleteReferencingRelationships),
    );
    assert_eq!(
        named_reference(&error),
        Some(("CrossPackageReference", "c_ap", "metaClaim", "c_if")),
        "{EDIT_006}: an interface is a package: {error:?}"
    );
    // c_if2 is held from within the interface.
    let error = refused(EDIT_006, "c_if2", &mut doc, DeleteElement::new("c_if2"));
    assert_eq!(
        named_reference(&error),
        Some(("Referenced", "c_if", "metaClaim", "c_if2")),
        "{EDIT_006}: c_if and c_if2 share the interface iface: {error:?}"
    );
    let mutation = accepted(
        EDIT_006,
        "c_if2, cascading",
        &mut doc,
        DeleteElement::new("c_if2").with_reference_policy(DeleteReferencingRelationships),
    );
    assert_eq!(
        strs(mutation.changed_ids()),
        ["c_if"],
        "{EDIT_006}: c_if lost its metaClaim"
    );
    // With c_if2 gone, c_if is held from ap and bind only.
    let mutation = accepted(
        EDIT_006,
        "c_if, crossing applied",
        &mut doc,
        DeleteElement::new("c_if")
            .with_reference_policy(DeleteReferencingRelationships)
            .with_cross_package_policy(ApplyReferencePolicy),
    );
    assert_eq!(
        strs(mutation.changed_ids()),
        ["c_ap", "c_b"],
        "{EDIT_006}: the holders in the package and the binding lost c_if"
    );
}

#[test]
fn lib_edit_006_the_first_refused_reference_in_list_order_decides_between_cmd_007_and_cmd_002() {
    let same = package(
        "p",
        &format!("{}{}", claim("c1", ""), claim("same", r#"metaClaim="c1""#)),
    );
    let far = package("q", &claim("far", r#"metaClaim="c1""#));

    // `same` comes first and is refused by RejectIfReferenced, though `far`
    // crosses.
    let mut doc = read(EDIT_006, &case(&format!("{same}{far}")));
    let error = refused(EDIT_006, "same first", &mut doc, DeleteElement::new("c1"));
    assert_eq!(
        named_reference(&error),
        Some(("Referenced", "same", "metaClaim", "c1")),
        "{EDIT_006}: the first dangling reference in list order is refused first: {error:?}"
    );
    // Under DeleteReferencingRelationships `same` is not refused.
    let error = refused(
        EDIT_006,
        "same first, cascading",
        &mut doc,
        DeleteElement::new("c1").with_reference_policy(DeleteReferencingRelationships),
    );
    assert_eq!(
        named_reference(&error),
        Some(("CrossPackageReference", "far", "metaClaim", "c1")),
        "{EDIT_006}: the first refused reference is far's: {error:?}"
    );

    // `far` comes first.
    let mut doc = read(EDIT_006, &case(&format!("{far}{same}")));
    let error = refused(EDIT_006, "far first", &mut doc, DeleteElement::new("c1"));
    assert_eq!(
        named_reference(&error),
        Some(("CrossPackageReference", "far", "metaClaim", "c1")),
        "{EDIT_006}: {error:?}"
    );
}

#[test]
fn lib_edit_006_the_check_order_is_element_not_found_then_not_empty_then_references() {
    let mut doc = read(EDIT_006, &non_empty_package());
    // 1. ElementNotFound, under every policy.
    for operation in every_policy("nowhere") {
        let context = format!("{operation:?}");
        let error = refused(EDIT_006, &context, &mut doc, operation);
        assert!(
            matches!(&error, EditError::ElementNotFound { .. }),
            "{EDIT_006}: {context}: {error:?}"
        );
    }
    // 2. NotEmpty before the crossing citation of c1 by far: covered under
    //    every reference and cross-package policy by
    //    lib_edit_006_reject_if_non_empty_refuses_a_package_with_its_count_of_contained_elements.
    let error = refused(EDIT_006, "p, all reject", &mut doc, DeleteElement::new("p"));
    assert!(
        matches!(&error, EditError::NotEmpty { .. }),
        "{EDIT_006}: NotEmpty comes before the references: {error:?}"
    );
    // 3. Then the references: far, in q, cites c1, in p.
    let error = refused(
        EDIT_006,
        "p recursively",
        &mut doc,
        DeleteElement::new("p").with_package_policy(DeleteRecursively),
    );
    assert_eq!(
        named_reference(&error),
        Some(("CrossPackageReference", "far", "citedElement", "c1")),
        "{EDIT_006}: {error:?}"
    );
    let error = refused(
        EDIT_006,
        "p recursively, crossing applied",
        &mut doc,
        DeleteElement::new("p")
            .with_package_policy(DeleteRecursively)
            .with_cross_package_policy(ApplyReferencePolicy),
    );
    assert_eq!(
        named_reference(&error),
        Some(("Referenced", "far", "citedElement", "c1")),
        "{EDIT_006}: {error:?}"
    );
    let mutation = accepted(
        EDIT_006,
        "p recursively, scrubbed",
        &mut doc,
        delete(
            "p",
            ScrubReferences,
            DeleteRecursively,
            ApplyReferencePolicy,
        ),
    );
    assert_eq!(
        (strs(mutation.deleted_ids()), strs(mutation.changed_ids())),
        (
            vec!["p", "c1", "c1_d", "c1_note", "c2", "n", "c3", "c3_note"],
            vec!["far"]
        ),
        "{EDIT_006}: the ids"
    );
}
