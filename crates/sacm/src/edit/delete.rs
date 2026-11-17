//! The operation that deletes an element, under explicit policies
//! (LIB-EDIT-004 to LIB-EDIT-006, D-POLICY, ADR 0031).

use std::collections::{BTreeSet, HashMap};

use super::plan::{Plan, Removal};
use super::{DanglingReference, EditError};
use crate::model::{AbstractClass, Document, ElementId, Preserved, ReferenceEnd, SacmElement};

/// Deletes an element, everything it contains, and whatever its policies
/// allow, so that no reference is left dangling.
///
/// SACM 2.3 says nothing about editing, so the policies follow the
/// assurance-forge C++ library (ADR 0031). Each defaults to refusing, so a
/// delete never cascades unless asked to (D-POLICY):
///
/// - [`with_reference_policy`](Self::with_reference_policy): what happens to
///   a reference to a deleted element held outside the element's subtree
///   ([`ReferenceDeletePolicy`]): refused (SACM-CMD-002), handled by deleting
///   each asserted relationship whose `source`, `target` or `reasoning`, and
///   each `ArtifactAssetRelationship` whose `source` or `target`, names a
///   deleted element, or scrubbed out of its end.
/// - [`with_package_policy`](Self::with_package_policy): whether an element
///   that contains other elements is refused (SACM-CMD-006) or deleted with
///   them ([`PackageDeletePolicy`]). Descriptions, implementation
///   constraints, notes and tagged values go with their owner and do not
///   count.
/// - [`with_cross_package_policy`](Self::with_cross_package_policy):
///   whether a dangling reference held in another package than the element
///   it names is refused (SACM-CMD-007) or handled by the reference policy
///   ([`CrossPackageReferencePolicy`]).
///
/// **Preserved content** (ADR 0033) goes with the element that holds it, and
/// its ids are freed. A reference to one of them, held outside the deleted
/// subtree, is a dangling reference like one to a deleted element, handled
/// by the policies; its package is that of the element that held it.
///
/// **What is deleted** is the element and everything it contains, plus each
/// relationship the reference policy deletes, an asserted relationship or an
/// `ArtifactAssetRelationship`, with its contents, to a fixpoint: a
/// relationship can be the source or target of another. Every surviving end
/// loses the deleted ids and keeps the rest, in order; a relationship is
/// deleted rather than left without a `source` or `target` (§11.13, §12.14),
/// and an `ArgumentGroup` or `ArtifactGroup` that loses its members stays. A
/// reference that named no element before the delete is not touched.
///
/// **The checks**, in order: the element is in the document
/// ([`EditError::ElementNotFound`]); it contains nothing but
/// `UtilityElement`s, and no element of its subtree holds a preserved child
/// element, unless [`PackageDeletePolicy::DeleteRecursively`]
/// ([`EditError::NotEmpty`]); then each dangling reference, in the order of
/// [`Preview::dangling_references`](super::Preview::dangling_references):
/// the first a policy refuses is the answer, as
/// [`EditError::CrossPackageReference`] if it crosses a package boundary and
/// the cross-package policy refuses it, and otherwise as
/// [`EditError::Referenced`].
///
/// The [`Mutation`](super::Mutation) lists every deleted element, and no
/// preserved id, in pre-order before the delete, and, in pre-order after,
/// each surviving element that lost ids and each whose preserved child moved
/// to a lower index among its children because a child before it was
/// deleted (ADR 0033). Nothing is created, and the parent is not changed
/// otherwise.
///
/// A delete does not rewind the per-kind counter, so generation stays
/// deterministic: the next generated id is still the smallest unused number
/// above the counter. A freed id may be used again: a caller may give it
/// with `with_id`, and a freed id the caller gave may be generated later. An
/// application that needs ids unique across its history gives its own.
///
/// Apply it with [`edit::apply`](super::apply), or see what it would do
/// first with [`edit::preview`](super::preview) and apply that preview with
/// [`edit::apply_preview`](super::apply_preview).
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
    /// Delete each asserted relationship whose `source`, `target` or
    /// `reasoning` names a deleted element, and each
    /// `ArtifactAssetRelationship` whose `source` or `target` does, with its
    /// contents, to a fixpoint; remove every other such id from its end
    /// (`citedElement`, `abstractForm`, `metaClaim`, `structure`,
    /// `referencedArtifactElement` and a group's `argumentElement` or
    /// `artifactElement`, none of which has a lower bound).
    DeleteReferencingRelationships,
    /// Remove every such id from its end. A relationship the delete leaves
    /// with no `source`, or with no `target`, is deleted, with its contents,
    /// since SACM requires both: §11.13 for an asserted relationship
    /// (`source:ArgumentAsset[1..*]`, `target:ArgumentAsset[1]`) and §12.14
    /// for an `ArtifactAssetRelationship` (`source:ArtifactAsset[1..*]`,
    /// `target:ArtifactAsset[1..*]`). One that keeps an id in each end stays.
    ScrubReferences,
}

/// Whether a delete may remove an element that contains other elements
/// (ADR 0031).
///
/// An element is non-empty when it contains, at any depth, an element that
/// is not a `UtilityElement`, or when it or an element it contains holds a
/// preserved child element (ADR 0033; preserved attributes and text do not
/// count). Descriptions, implementation constraints, notes and tagged values
/// go with their owner under either policy.
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
/// A reference crosses one when the holder's nearest package is not the
/// nearest package of the element it names. An element's nearest package is
/// the element itself, if it is a package, an interface or a binding, and
/// otherwise its closest ancestor that is one. So a nested package is
/// another package than its parent.
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
    /// Refuse the delete if a dangling reference crosses a package boundary
    /// ([`EditError::CrossPackageReference`], SACM-CMD-007), whatever the
    /// reference policy. The default.
    #[default]
    RejectIfExternalReferencesExist,
    /// Treat a crossing reference as any other: the reference policy decides.
    ApplyReferencePolicy,
}

impl DeleteElement {
    /// Deletes the element `element`, with every policy at its default:
    /// refusing a referenced element, a non-empty one and a crossing
    /// reference.
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
        if self.package_policy == PackageDeletePolicy::RejectIfNonEmpty {
            // Each preserved child element anywhere in the subtree is content
            // too (ADR 0033); preserved attributes and text are not.
            let preserved: usize = subtree
                .clone()
                .filter_map(|position| tree.elements[position].preserved())
                .map(Preserved::element_count)
                .sum();
            let contained = subtree
                .clone()
                .skip(1)
                .filter(|&position| {
                    !tree.elements[position]
                        .kind()
                        .is_a(AbstractClass::UtilityElement)
                })
                .count()
                .saturating_add(preserved);
            if contained > 0 {
                return Err(EditError::NotEmpty {
                    element: self.element.clone(),
                    contained,
                });
            }
        }

        let deleted = tree.deleted(subtree.clone(), self.reference_policy);
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

/// The document in pre-order, with what a delete needs to know of each
/// element by its position: the end of its subtree and its nearest package.
/// Built in linear time.
struct Tree<'d> {
    /// Every element, in pre-order.
    elements: Vec<SacmElement<'d>>,
    /// The position of each element, by id, and of the element whose
    /// preserved content holds each preserved id (ADR 0033), which goes
    /// with it. Only looked up.
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
                std::iter::once(element.id())
                    .chain(preserved)
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
        let Some(last) = element
            .preserved()
            .and_then(|preserved| preserved.children.iter().map(|placed| placed.index).max())
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
    /// relationship `policy` deletes, with its subtree, to a fixpoint.
    fn deleted(&self, subtree: std::ops::Range<usize>, policy: ReferenceDeletePolicy) -> Vec<bool> {
        let mut deleted = vec![false; self.elements.len()];
        let mut pending: Vec<usize> = Vec::new();
        for position in subtree {
            deleted[position] = true;
            pending.push(position);
        }
        if policy == ReferenceDeletePolicy::RejectIfReferenced {
            return deleted;
        }
        // The ends through which a delete cascades: each occurrence of an id
        // in the `source` or `target` of a relationship, an asserted one
        // (§11.13) or an ArtifactAssetRelationship (§12.14), and under
        // `DeleteReferencingRelationships` in an asserted one's `reasoning`
        // too (ADR 0034 amendment 2). Each end applies to one class, so a
        // relationship cascades through its own. `remaining` counts, for each
        // relationship and each of these ends, the occurrences not yet
        // deleted, so each reference is looked at once and the fixpoint is
        // linear in the number of references; only a scrub reads it, and
        // every end a scrub cascades through has the lower bound 1.
        let mut ends = vec![
            ReferenceEnd::Source,
            ReferenceEnd::Target,
            ReferenceEnd::ArtifactSource,
            ReferenceEnd::ArtifactTarget,
        ];
        if policy == ReferenceDeletePolicy::DeleteReferencingRelationships {
            ends.push(ReferenceEnd::Reasoning);
        }
        let mut triggers: HashMap<&ElementId, Vec<(usize, ReferenceEnd)>> = HashMap::new();
        let mut remaining: HashMap<(usize, ReferenceEnd), usize> = HashMap::new();
        for (position, element) in self.elements.iter().enumerate() {
            for &end in ends.iter().filter(|end| end.applies_to(element.kind())) {
                let held = element.references(end);
                remaining.insert((position, end), held.len());
                for id in held {
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
            let held = std::iter::once(element.id())
                .chain(preserved)
                .filter_map(|id| triggers.get(id))
                .flatten()
                .copied()
                .collect::<Vec<_>>();
            for (relationship, end) in held {
                if deleted[relationship] {
                    continue;
                }
                let goes = match remaining.get_mut(&(relationship, end)) {
                    // A scrub deletes the relationship once an end loses its
                    // last id: each end keeps its own count.
                    Some(left) if policy == ReferenceDeletePolicy::ScrubReferences => {
                        *left = left.saturating_sub(1);
                        *left == 0
                    }
                    // Any cascading end of the relationship names a deleted
                    // element.
                    _ => true,
                };
                if !goes {
                    continue;
                }
                for inner in self.subtree(relationship) {
                    if !deleted[inner] {
                        deleted[inner] = true;
                        pending.push(inner);
                    }
                }
            }
        }
        deleted
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
