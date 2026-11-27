//! The operation that deletes an element, under explicit policies
//! (LIB-EDIT-004 to LIB-EDIT-006, LIB-EDIT-019, D-POLICY, ADR 0031, ADR
//! 0039).

use std::collections::{BTreeSet, HashMap};

use super::plan::{Plan, Removal};
use super::{DanglingReference, EditError};
use crate::model::{
    AbstractClass, Document, ElementId, Href, Preserved, ReferenceEnd, SacmElement,
};

/// Deletes an element, everything it contains, and whatever its policies
/// allow, so that no typed reference is left dangling.
///
/// - **Three policies, each refusing by default,** so a delete never cascades unless
///   asked to (D-POLICY). SACM 2.3 says nothing about editing, so they follow the
///   assurance-forge C++ library (ADR 0031).
/// - **Ask first.** [`edit::preview`](super::preview) shows what it would do;
///   [`edit::apply_preview`](super::apply_preview) applies that preview.
/// - **Refusals.** [`apply`](super::apply#deletes) lists the checks, in order.
///
/// | Policy | Decides | Refusal (default) |
/// |---|---|---|
/// | [`with_reference_policy`](Self::with_reference_policy), a [`ReferenceDeletePolicy`] | what happens to a reference to a deleted element, held outside the element's subtree | SACM-CMD-002 |
/// | [`with_package_policy`](Self::with_package_policy), a [`PackageDeletePolicy`] | whether an element that contains other elements is deleted with them; `UtilityElement`s go with their owner and do not count | SACM-CMD-006 |
/// | [`with_cross_package_policy`](Self::with_cross_package_policy), a [`CrossPackageReferencePolicy`] | whether a dangling reference held in another package than the element it names goes to the reference policy | SACM-CMD-007 |
///
/// # What `deleted_ids()` lists
///
/// [`Mutation::deleted_ids`](super::Mutation::deleted_ids) lists every deleted
/// element once, in the document's pre-order before the delete:
///
/// | Policy | Lists |
/// |---|---|
/// | any | the element, and everything it contains at any depth |
/// | [`RejectIfNonEmpty`](PackageDeletePolicy::RejectIfNonEmpty) | of what the element contains, only `UtilityElement`s: with anything else the delete is refused, as is one that would remove a non-empty interface or binding for its bound |
/// | [`DeleteRecursively`](PackageDeletePolicy::DeleteRecursively) | nothing more: it only allows the element to contain other elements |
/// | [`RejectIfReferenced`](ReferenceDeletePolicy::RejectIfReferenced) | nothing more: a reference to a listed element, held outside the element's subtree, refuses the delete |
/// | [`DeleteReferencingRelationships`](ReferenceDeletePolicy::DeleteReferencingRelationships) | also each asserted relationship whose `source`, `target` or `reasoning`, and each `ArtifactAssetRelationship` whose `source` or `target`, names a listed element |
/// | [`ScrubReferences`](ReferenceDeletePolicy::ScrubReferences) | also each relationship of either class that loses the id of a listed element and is left with no `source` or no `target` |
/// | either of those two | also each interface that loses such an id and is left with no `implements`, and each binding left with fewer than two distinct participants (LIB-EDIT-019) |
/// | a [`CrossPackageReferencePolicy`] | nothing of its own: it only decides whether a crossing reference refuses the delete |
///
/// - **With its contents, to a fixpoint.** Each relationship, interface or binding listed
///   for a reference is listed with everything it contains, and counts as a listed
///   element in turn: a relationship can be the source or target of another.
/// - **Not listed: an element that only loses a reference.** It survives, and is in
///   [`changed_ids`](super::Mutation::changed_ids), with each element whose preserved
///   child moved to a lower index because a child before it was deleted (ADR 0033).
/// - **Not listed: the ids of preserved content,** which identify no element.
/// - **Nothing is created,** and the parent is not changed otherwise.
///
/// # What happens to references
///
/// - **A surviving end** loses the deleted ids and keeps the rest, in order.
/// - **A relationship** is deleted rather than left without a `source` or `target`
///   (§11.13, §12.14). An `ArgumentGroup` or `ArtifactGroup` that loses its members stays.
/// - **A reference that named no element** before the delete is not touched.
/// - **Preserved content** (ADR 0033) goes with the element that holds it, and its ids
///   are freed. A reference to one of them, held outside the deleted subtree, is a
///   dangling reference like one to a deleted element.
/// - **Its package,** for the cross-package policy, is that of the element that held it.
///
/// # Ids afterwards
///
/// - **The counter is not rewound.** The next generated id is still the smallest unused
///   number above the per-kind counter, so generation stays deterministic.
/// - **A freed id may be used again.** A caller may give it with `with_id`, and a freed
///   id the caller gave may be generated later. An application that needs ids unique
///   across its history gives its own.
///
/// # Examples
///
/// ```
/// use sacm::diagnostic::Code;
/// use sacm::edit::{self, CreateArgumentPackage, CreateAssertedInference, CreateClaim, DeleteElement};
/// use sacm::edit::{Operation, ReferenceDeletePolicy};
/// use sacm::model::{Document, ElementId, LangString, ReferenceEnd};
///
/// let mut doc = Document::new();
/// let name = LangString::new;
/// edit::apply(&mut doc, CreateArgumentPackage::root(name("Arguments")).with_id("p"))?;
/// edit::apply(&mut doc, CreateClaim::new("p", name("Top")).with_id("top"))?;
/// edit::apply(&mut doc, CreateClaim::new("p", name("Sub")).with_id("sub"))?;
/// edit::apply(&mut doc, CreateAssertedInference::new("p", name("I"), ["sub"], "top").with_id("i"))?;
///
/// // `i` names `sub` as its source, so the default refuses.
/// let refused = edit::apply(&mut doc, DeleteElement::new("sub")).unwrap_err();
/// assert_eq!(refused.code(), Code::CMD_DELETE_REFERENCED);
///
/// // Deleting the referencing relationships deletes `i` too. Preview it first:
/// // the preview lists the reference `i` holds to `sub`, and changes nothing.
/// let delete = Operation::from(
///     DeleteElement::new("sub")
///         .with_reference_policy(ReferenceDeletePolicy::DeleteReferencingRelationships),
/// );
/// let preview = edit::preview(&doc, &delete)?;
/// let dangling = &preview.dangling_references()[0];
/// assert_eq!(dangling.holder().as_str(), "i");
/// assert_eq!(dangling.end(), ReferenceEnd::Source);
/// assert_eq!(dangling.id().as_str(), "sub");
/// assert!(doc.contains("sub"));
///
/// // Apply what was previewed.
/// let mutation = edit::apply_preview(&mut doc, &preview)?;
/// assert_eq!(mutation.deleted_ids(), [ElementId::from("sub"), ElementId::from("i")]);
/// assert!(!doc.contains("i"));
/// # Ok::<(), sacm::edit::EditError>(())
/// ```
///
/// What `deleted_ids()` and `changed_ids()` list under each policy:
///
/// ```
/// use sacm::edit::{self, CreateArgumentPackage, CreateAssertedInference, CreateClaim};
/// use sacm::edit::{CreateDescription, DeleteElement, EditError, Mutation};
/// use sacm::edit::{PackageDeletePolicy, ReferenceDeletePolicy};
/// use sacm::model::{Document, ElementId, LangString};
///
/// let mut doc = Document::new();
/// let name = LangString::new;
/// edit::apply(&mut doc, CreateArgumentPackage::root(name("Arguments")).with_id("p"))?;
/// edit::apply(&mut doc, CreateClaim::new("p", name("Top")).with_id("top"))?;
/// edit::apply(&mut doc, CreateClaim::new("p", name("Sub")).with_id("sub"))?;
/// edit::apply(&mut doc, CreateDescription::new("sub", name("About Sub")).with_id("d"))?;
/// edit::apply(&mut doc, CreateClaim::new("p", name("Other")).with_id("other"))?;
/// // `one` has `sub` as its only source; `two` has another source too.
/// edit::apply(&mut doc, CreateAssertedInference::new("p", name("1"), ["sub"], "top").with_id("one"))?;
/// let two = CreateAssertedInference::new("p", name("2"), ["sub", "other"], "top").with_id("two");
/// edit::apply(&mut doc, two)?;
///
/// // What a delete would do, without changing the document.
/// let would = |delete: DeleteElement| -> Result<Mutation, EditError> {
///     Ok(edit::preview(&doc, &delete.into())?.mutation().clone())
/// };
/// let ids = |ids: &[ElementId]| ids.iter().map(ElementId::as_str).collect::<Vec<_>>().join(" ");
///
/// // RejectIfReferenced, the default: `one` and `two` name `sub`, so it refuses.
/// assert!(matches!(would(DeleteElement::new("sub")), Err(EditError::Referenced { .. })));
///
/// // DeleteReferencingRelationships: `sub`, its description, and both inferences.
/// let delete = DeleteElement::new("sub")
///     .with_reference_policy(ReferenceDeletePolicy::DeleteReferencingRelationships);
/// let mutation = would(delete)?;
/// assert_eq!(ids(mutation.deleted_ids()), "sub d one two");
/// assert_eq!(ids(mutation.changed_ids()), "");
///
/// // ScrubReferences: `one` is left with no source, so it goes; `two` only loses `sub`.
/// let delete = DeleteElement::new("sub").with_reference_policy(ReferenceDeletePolicy::ScrubReferences);
/// let mutation = would(delete)?;
/// assert_eq!(ids(mutation.deleted_ids()), "sub d one");
/// assert_eq!(ids(mutation.changed_ids()), "two");
///
/// // RejectIfNonEmpty, the default: `p` contains claims, so it refuses.
/// assert!(matches!(would(DeleteElement::new("p")), Err(EditError::NotEmpty { .. })));
///
/// // DeleteRecursively: `p` and everything it contains, in pre-order.
/// let delete = DeleteElement::new("p").with_package_policy(PackageDeletePolicy::DeleteRecursively);
/// assert_eq!(ids(would(delete)?.deleted_ids()), "p top sub d other one two");
/// # Ok::<(), EditError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
pub struct DeleteElement {
    element: ElementId,
    #[cfg_attr(feature = "serde", serde(default))]
    reference_policy: ReferenceDeletePolicy,
    #[cfg_attr(feature = "serde", serde(default))]
    package_policy: PackageDeletePolicy,
    #[cfg_attr(feature = "serde", serde(default))]
    cross_package_policy: CrossPackageReferencePolicy,
}

/// What a delete does with a reference to a deleted element, held by an
/// element outside the deleted element's subtree (ADR 0031).
///
/// # Examples
///
/// ```
/// use sacm::edit::ReferenceDeletePolicy;
///
/// assert_eq!(ReferenceDeletePolicy::default(), ReferenceDeletePolicy::RejectIfReferenced);
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum ReferenceDeletePolicy {
    /// Refuse the delete if any such reference exists
    /// ([`EditError::Referenced`], SACM-CMD-002). The default.
    #[default]
    RejectIfReferenced,
    /// Delete each relationship that names a deleted element, and remove the id from
    /// every other end.
    ///
    /// - **Relationships deleted:** each asserted relationship whose `source`, `target`
    ///   or `reasoning` names a deleted element, and each `ArtifactAssetRelationship`
    ///   whose `source` or `target` does, with its contents, to a fixpoint.
    /// - **Ids removed** from every other end: `citedElement`, `abstractForm`,
    ///   `metaClaim`, `structure`, `referencedArtifactElement`, a group's
    ///   `argumentElement` or `artifactElement` and a package's `interface`. None of
    ///   them has a lower bound.
    /// - **Interfaces and bindings.** An interface left with no `implements`, or a
    ///   binding left with fewer than two distinct `participantPackage` ids, is deleted
    ///   with its contents, as under `ScrubReferences` (LIB-EDIT-019).
    DeleteReferencingRelationships,
    /// Remove every such id from its end, and delete an element left below an end's
    /// lower bound, with its contents.
    ///
    /// | Deleted when left with | SACM |
    /// |---|---|
    /// | a relationship: no `source` or no `target` | §11.13, §12.14 |
    /// | an interface: no `implements` | §9.3, §10.5, §11.6, §12.5 |
    /// | a binding: fewer than two distinct participants | §9.4, §10.6, §11.5, §12.4 |
    ///
    /// - **Under `RejectIfNonEmpty`,** such an interface or binding that contains
    ///   elements refuses the delete as `NotEmpty`.
    /// - **One that loses no id stays,** even below its bound.
    ScrubReferences,
}

/// Whether a delete may remove an element that contains other elements
/// (ADR 0031).
///
/// - **Non-empty** is an element that contains, at any depth, an element that is not a
///   `UtilityElement`, or that holds, itself or in an element it contains, a preserved
///   child element (ADR 0033; preserved attributes and text do not count).
/// - **A proxy counts too:** a child held in another document
///   ([`Proxy`](crate::model::Proxy); ADR 0040 amendment 1).
/// - **`UtilityElement`s go with their owner** under either policy: descriptions,
///   implementation constraints, notes and tagged values.
///
/// # Examples
///
/// ```
/// use sacm::edit::PackageDeletePolicy;
///
/// assert_eq!(PackageDeletePolicy::default(), PackageDeletePolicy::RejectIfNonEmpty);
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum PackageDeletePolicy {
    /// Refuse to delete a non-empty element ([`EditError::NotEmpty`],
    /// SACM-CMD-006). The default.
    #[default]
    RejectIfNonEmpty,
    /// Delete the element and everything it contains.
    DeleteRecursively,
}

/// What a delete does with a dangling reference that crosses a package
/// boundary (ADR 0031).
///
/// - **Crossing.** A reference crosses one when the holder's nearest package is not the
///   nearest package of the element it names.
/// - **Nearest package.** The element itself, if it is a package, an interface or a
///   binding, and otherwise its closest ancestor that is one. So a nested package is
///   another package than its parent.
///
/// # Examples
///
/// ```
/// use sacm::edit::CrossPackageReferencePolicy;
///
/// assert_eq!(
///     CrossPackageReferencePolicy::default(),
///     CrossPackageReferencePolicy::RejectIfExternalReferencesExist
/// );
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum CrossPackageReferencePolicy {
    /// Refuse the delete if a dangling reference crosses a package boundary, whatever the
    /// reference policy ([`EditError::CrossPackageReference`], SACM-CMD-007). The default.
    #[default]
    RejectIfExternalReferencesExist,
    /// Treat a crossing reference as any other: the reference policy decides.
    ApplyReferencePolicy,
}

impl DeleteElement {
    /// Deletes the element `element`, with every policy at its default: refusing a
    /// referenced element, a non-empty one and a crossing reference.
    #[must_use]
    pub fn new(element: impl Into<ElementId>) -> Self {
        Self {
            element: element.into(),
            reference_policy: ReferenceDeletePolicy::default(),
            package_policy: PackageDeletePolicy::default(),
            cross_package_policy: CrossPackageReferencePolicy::default(),
        }
    }

    /// Sets what happens to a reference to a deleted element.
    #[must_use]
    pub fn with_reference_policy(mut self, policy: ReferenceDeletePolicy) -> Self {
        self.reference_policy = policy;
        self
    }

    /// Sets whether a non-empty element may be deleted.
    #[must_use]
    pub fn with_package_policy(mut self, policy: PackageDeletePolicy) -> Self {
        self.package_policy = policy;
        self
    }

    /// Sets what happens to a dangling reference across a package boundary.
    #[must_use]
    pub fn with_cross_package_policy(mut self, policy: CrossPackageReferencePolicy) -> Self {
        self.cross_package_policy = policy;
        self
    }

    /// The id of the element to delete.
    #[must_use]
    pub fn element(&self) -> &ElementId {
        &self.element
    }

    /// What happens to a reference to a deleted element.
    #[must_use]
    pub fn reference_policy(&self) -> ReferenceDeletePolicy {
        self.reference_policy
    }

    /// Whether a non-empty element may be deleted.
    #[must_use]
    pub fn package_policy(&self) -> PackageDeletePolicy {
        self.package_policy
    }

    /// What happens to a dangling reference across a package boundary.
    #[must_use]
    pub fn cross_package_policy(&self) -> CrossPackageReferencePolicy {
        self.cross_package_policy
    }

    /// Runs every check of the operation, and returns what it removes.
    pub(super) fn plan<'o>(&'o self, document: &Document) -> Result<Plan<'o>, EditError> {
        let Some(target) = document.get(&self.element) else {
            return Err(EditError::ElementNotFound {
                element: self.element.clone(),
            });
        };
        let tree = Tree::of(document);
        let Some(start) = tree.position(target.id()) else {
            return Err(EditError::ElementNotFound {
                element: self.element.clone(),
            });
        };
        let subtree = tree.subtree(start);
        let rejects_non_empty = self.package_policy == PackageDeletePolicy::RejectIfNonEmpty;
        if rejects_non_empty {
            let contained = tree.contained(start);
            if contained > 0 {
                return Err(EditError::NotEmpty {
                    element: self.element.clone(),
                    contained,
                });
            }
        }

        let (deleted, mut holders) = tree.deleted(subtree.clone(), self.reference_policy);
        let dangling = tree.dangling(&subtree, &deleted);
        for reference in &dangling {
            if reference.crosses_package()
                && self.cross_package_policy
                    == CrossPackageReferencePolicy::RejectIfExternalReferencesExist
            {
                return Err(EditError::CrossPackageReference {
                    holder: reference.holder().clone(),
                    end: reference.end().name(),
                    id: reference.id().clone(),
                });
            }
            if self.reference_policy == ReferenceDeletePolicy::RejectIfReferenced {
                return Err(EditError::Referenced {
                    holder: reference.holder().clone(),
                    end: reference.end().name(),
                    id: reference.id().clone(),
                });
            }
        }
        // Check 4 (LIB-EDIT-019, ADR 0039 D5): the package policy guards each
        // interface or binding the delete removes for its bound: the first in
        // pre-order that contains more than `UtilityElement`s refuses.
        if rejects_non_empty {
            holders.sort_unstable();
            if let Some((holder, contained)) = holders
                .iter()
                .map(|&holder| (holder, tree.contained(holder)))
                .find(|&(_, contained)| contained > 0)
            {
                return Err(EditError::NotEmpty {
                    element: tree.elements[holder].id().clone(),
                    contained,
                });
            }
        }

        // The holders that survive lose the deleted ids, and a survivor whose
        // preserved child comes after a deleted child of it has that child
        // moved to a lower index (ADR 0033): both change, in pre-order.
        let mut is_changed = vec![false; tree.elements.len()];
        for reference in &dangling {
            if let Some(position) = tree.position(reference.holder()) {
                is_changed[position] = true;
            }
        }
        for (position, element) in tree.elements.iter().enumerate() {
            if !deleted[position] && tree.moves_preserved(*element, &deleted) {
                is_changed[position] = true;
            }
        }
        let changed: Vec<ElementId> = tree
            .elements
            .iter()
            .enumerate()
            .filter(|&(position, _)| is_changed[position] && !deleted[position])
            .map(|(_, element)| element.id().clone())
            .collect();
        let deleted = tree
            .elements
            .iter()
            .zip(&deleted)
            .filter(|(_, gone)| **gone)
            .map(|(element, _)| element.id().clone())
            .collect();
        Ok(Plan::Delete(Removal {
            deleted,
            changed,
            dangling,
        }))
    }
}

/// Whether `end` deletes its holder by the bound rule under both non-reject
/// reference policies: an interface's `implements` and a binding's
/// `participantPackage` (LIB-EDIT-019, ADR 0039 D5).
const fn is_bound_end(end: ReferenceEnd) -> bool {
    matches!(
        end,
        ReferenceEnd::Implements | ReferenceEnd::ParticipantPackage
    )
}

/// The document in pre-order, with what a delete needs to know of each
/// element by its position: the end of its subtree and its nearest package.
/// Built in linear time.
struct Tree<'d> {
    /// Every element, in pre-order.
    elements: Vec<SacmElement<'d>>,
    /// The position of each element, by id. Only looked up.
    ///
    /// A preserved id (ADR 0033) has the position of the element whose preserved content
    /// holds it, which goes with it.
    positions: HashMap<&'d ElementId, usize>,
    /// The position after the last element of each element's subtree: a
    /// subtree is a range of the pre-order.
    ends: Vec<usize>,
    /// The position of each element's nearest package: itself or its
    /// closest ancestor of a package kind (every root is one).
    packages: Vec<Option<usize>>,
}

impl<'d> Tree<'d> {
    /// The tree of `document`.
    fn of(document: &'d Document) -> Self {
        let elements: Vec<SacmElement<'d>> = document.elements().collect();
        let positions: HashMap<&'d ElementId, usize> = elements
            .iter()
            .enumerate()
            .flat_map(|(position, element)| {
                let preserved = element
                    .preserved()
                    .map_or(&[][..], |held| held.ids.as_slice());
                // The ids of its XMI metadata are its own too (ADR 0042 D5).
                let metadata = element.metadata_parts().flat_map(|held| held.ids.iter());
                std::iter::once(element.id())
                    .chain(preserved)
                    .chain(metadata)
                    .map(move |id| (id, position))
            })
            .collect();
        let parents: Vec<Option<usize>> = elements
            .iter()
            .map(|element| {
                element
                    .parent()
                    .and_then(|parent| positions.get(parent.id()).copied())
            })
            .collect();
        // A parent precedes its children, so each element's package is known
        // when it is reached, and each subtree's end once its last element
        // is: the subtree of a parent ends where its last child's ends.
        let mut packages: Vec<Option<usize>> = Vec::with_capacity(elements.len());
        for (position, element) in elements.iter().enumerate() {
            let package = if element.kind().is_package() {
                Some(position)
            } else {
                parents[position].and_then(|parent| packages.get(parent).copied().flatten())
            };
            packages.push(package);
        }
        let mut ends: Vec<usize> = (1..=elements.len()).collect();
        for position in (0..elements.len()).rev() {
            if let Some(parent) = parents[position] {
                let end = ends[position];
                if let Some(parent_end) = ends.get_mut(parent) {
                    *parent_end = (*parent_end).max(end);
                }
            }
        }
        Self {
            elements,
            positions,
            ends,
            packages,
        }
    }

    /// The position of the element `id`, if the document has it.
    fn position(&self, id: &ElementId) -> Option<usize> {
        self.positions.get(id).copied()
    }

    /// The positions of the subtree of the element at `position`.
    fn subtree(&self, position: usize) -> std::ops::Range<usize> {
        position..self.ends.get(position).copied().unwrap_or(position)
    }

    /// Whether deleting `deleted` moves a preserved child of `element` to a
    /// lower index among its children: whether a child of it before its last
    /// preserved child is deleted (ADR 0033).
    fn moves_preserved(&self, element: SacmElement<'_>, deleted: &[bool]) -> bool {
        // Its XMI metadata children are placed the same way (ADR 0042).
        let Some(last) = element
            .preserved()
            .into_iter()
            .chain(element.metadata())
            .flat_map(|placed| placed.children.iter().map(|placed| placed.index))
            .max()
        else {
            return false;
        };
        element
            .children()
            .take(last)
            .any(|child| self.is_deleted(deleted, child.id()))
    }

    /// Whether `id` names an element of `deleted`.
    fn is_deleted(&self, deleted: &[bool], id: &ElementId) -> bool {
        self.position(id)
            .is_some_and(|position| deleted.get(position).copied().unwrap_or(false))
    }

    /// The elements to delete, by position: the subtree `subtree`, plus each
    /// relationship, interface and binding `policy` deletes, with its
    /// subtree, to a fixpoint; and the positions of the interfaces and
    /// bindings it deletes for their bound, which the package policy guards
    /// (LIB-EDIT-019, ADR 0039 D5).
    fn deleted(
        &self,
        subtree: std::ops::Range<usize>,
        policy: ReferenceDeletePolicy,
    ) -> (Vec<bool>, Vec<usize>) {
        let mut deleted = vec![false; self.elements.len()];
        let mut pending: Vec<usize> = Vec::new();
        let mut holders: Vec<usize> = Vec::new();
        for position in subtree {
            deleted[position] = true;
            pending.push(position);
        }
        if policy == ReferenceDeletePolicy::RejectIfReferenced {
            return (deleted, holders);
        }
        // The ends through which a delete cascades, each with its rule. Under
        // `DeleteReferencingRelationships` a relationship goes on any id of a
        // deleted element in its `source` or `target`, an asserted one's
        // (§11.13) or an ArtifactAssetRelationship's (§12.14), or in an
        // asserted one's `reasoning` (ADR 0034 amendment 2). Under
        // `ScrubReferences` those `source` and `target` ends follow the bound
        // rule, and so, under either, do an interface's `implements` and a
        // binding's `participantPackage` (ADR 0039 D5): a holder goes when it
        // loses an id there and is left below the end's lower bound, counting
        // distinct ids, so one already below it that loses no id stays. Each
        // end applies to its own classes. `remaining` counts, for each holder
        // and each of these ends, the distinct ids not yet deleted, and each
        // distinct id is one trigger, so each reference is looked at once and
        // the fixpoint is linear in the number of references.
        let by_bound = |end: ReferenceEnd| {
            policy == ReferenceDeletePolicy::ScrubReferences || is_bound_end(end)
        };
        let mut ends = vec![
            ReferenceEnd::Source,
            ReferenceEnd::Target,
            ReferenceEnd::ArtifactSource,
            ReferenceEnd::ArtifactTarget,
        ];
        if policy == ReferenceDeletePolicy::DeleteReferencingRelationships {
            ends.push(ReferenceEnd::Reasoning);
        }
        ends.extend([ReferenceEnd::Implements, ReferenceEnd::ParticipantPackage]);
        let mut triggers: HashMap<&ElementId, Vec<(usize, ReferenceEnd)>> = HashMap::new();
        let mut remaining: HashMap<(usize, ReferenceEnd), usize> = HashMap::new();
        for (position, element) in self.elements.iter().enumerate() {
            for &end in ends.iter().filter(|end| end.applies_to(element.kind())) {
                let distinct: BTreeSet<&ElementId> = element.references(end).iter().collect();
                // A cross-document reference counts toward the bound and is
                // never deleted with an element (ADR 0040 D5, D7).
                let hrefs: BTreeSet<&Href> = element.hrefs(end).iter().collect();
                remaining.insert((position, end), distinct.len() + hrefs.len());
                for id in distinct {
                    triggers.entry(id).or_default().push((position, end));
                }
            }
        }
        // Each element enters `pending` once, when it is first deleted; the
        // ids of its preserved content go with it (ADR 0033).
        while let Some(position) = pending.pop() {
            let element = self.elements[position];
            let preserved = element
                .preserved()
                .map_or(&[][..], |held| held.ids.as_slice());
            let metadata = element.metadata_parts().flat_map(|held| held.ids.iter());
            let held = std::iter::once(element.id())
                .chain(preserved)
                .chain(metadata)
                .filter_map(|id| triggers.get(id))
                .flatten()
                .copied()
                .collect::<Vec<_>>();
            for (holder, end) in held {
                if deleted[holder] {
                    continue;
                }
                let goes = match remaining.get_mut(&(holder, end)) {
                    // The bound rule: the holder goes once the end is left
                    // below its lower bound. Each end keeps its own count.
                    Some(left) if by_bound(end) => {
                        *left = left.saturating_sub(1);
                        *left < end.lower_bound()
                    }
                    // Any id of a deleted element in a cascading end.
                    _ => true,
                };
                if !goes {
                    continue;
                }
                if is_bound_end(end) {
                    holders.push(holder);
                }
                for inner in self.subtree(holder) {
                    if !deleted[inner] {
                        deleted[inner] = true;
                        pending.push(inner);
                    }
                }
            }
        }
        (deleted, holders)
    }

    /// How many elements deleting the element at `position` removes that
    /// [`PackageDeletePolicy::RejectIfNonEmpty`] refuses: each element of its
    /// subtree but itself that is no `UtilityElement`, each preserved child
    /// element anywhere in its subtree (ADR 0033; preserved attributes and
    /// text do not count), and each proxy anywhere in its subtree, a child
    /// held in another document (ADR 0040 amendment 1, A4).
    fn contained(&self, position: usize) -> usize {
        let subtree = self.subtree(position);
        let preserved: usize = subtree
            .clone()
            .map(|inner| {
                let element = self.elements[inner];
                element
                    .preserved()
                    .map_or(0, Preserved::element_count)
                    .saturating_add(element.proxies().len())
            })
            .sum();
        subtree
            .skip(1)
            .filter(|&inner| {
                !self.elements[inner]
                    .kind()
                    .is_a(AbstractClass::UtilityElement)
            })
            .count()
            .saturating_add(preserved)
    }

    /// Every reference held by an element outside `subtree` that names an
    /// element of `deleted`: by holder in pre-order, then by end in the
    /// order of [`ReferenceEnd`], then by first position, one per distinct
    /// id and end.
    fn dangling(
        &self,
        subtree: &std::ops::Range<usize>,
        deleted: &[bool],
    ) -> Vec<DanglingReference> {
        let mut dangling = Vec::new();
        for (position, holder) in self.elements.iter().enumerate() {
            if subtree.contains(&position) {
                continue;
            }
            for &end in ReferenceEnd::ALL {
                let mut seen: BTreeSet<&ElementId> = BTreeSet::new();
                for id in holder.references(end) {
                    if !self.is_deleted(deleted, id) || !seen.insert(id) {
                        continue;
                    }
                    let named = self.position(id).and_then(|named| self.packages[named]);
                    dangling.push(DanglingReference {
                        holder: holder.id().clone(),
                        end,
                        id: id.clone(),
                        crosses_package: named != self.packages[position],
                    });
                }
            }
        }
        dangling
    }
}
