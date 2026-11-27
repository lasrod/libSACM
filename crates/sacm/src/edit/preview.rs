//! Previews of an operation, and the references to the elements a delete
//! removes (LIB-EDIT-004, LIB-EDIT-007, D-PREVIEW, ADR 0031).

use super::{Mutation, Operation};
use crate::model::{ElementId, ReferenceEnd, Revision};

/// What [`apply`](super::apply) would do with an operation, without doing it.
///
/// - **Holds** the [`Mutation`] `apply` would return, the references to deleted
///   elements, which a delete removes, and the document's [`Revision`] when the preview
///   was taken.
/// - **From** [`edit::preview`](super::preview). [`edit::apply_preview`](super::apply_preview)
///   applies it while the document is still at that revision.
/// - **Written with serde, not read back.** It is written as `{"revision", "mutation",
///   "dangling_references"}`; its operation is the mutation's. A preview read back would
///   prove nothing about the current document (ADR 0031).
///
/// # Examples
///
/// ```
/// use sacm::edit::{self, CreateArgumentPackage, CreateClaim, DeleteElement, Operation};
/// use sacm::edit::{ReferenceDeletePolicy, SetReferences};
/// use sacm::model::{Document, ElementId, LangString, ReferenceEnd};
///
/// let mut doc = Document::new();
/// let create = Operation::from(CreateArgumentPackage::root(LangString::new("Arguments")));
/// let preview = edit::preview(&doc, &create)?;
/// assert_eq!(preview.operation(), &create);
/// assert_eq!(preview.revision(), doc.revision());
/// assert_eq!(preview.mutation().created_ids()[0].as_str(), "argument_package_1");
/// assert!(preview.dangling_references().is_empty()); // only a delete has any
/// assert_eq!(doc.element_count(), 0); // nothing was created
///
/// assert_eq!(&edit::apply_preview(&mut doc, &preview)?, preview.mutation());
///
/// // A delete's preview lists the references it removes: `c2` names `c1`
/// // as a metaClaim, so deleting `c1` scrubs the id from `c2`.
/// let name = LangString::new;
/// edit::apply(&mut doc, CreateClaim::new("argument_package_1", name("C1")).with_id("c1"))?;
/// edit::apply(&mut doc, CreateClaim::new("argument_package_1", name("C2")).with_id("c2"))?;
/// edit::apply(&mut doc, SetReferences::new("c2", ReferenceEnd::MetaClaim, ["c1"]))?;
/// let delete = Operation::from(
///     DeleteElement::new("c1").with_reference_policy(ReferenceDeletePolicy::ScrubReferences),
/// );
/// let preview = edit::preview(&doc, &delete)?;
/// let dangling = &preview.dangling_references()[0];
/// assert_eq!((dangling.holder().as_str(), dangling.id().as_str()), ("c2", "c1"));
/// assert_eq!(dangling.end(), ReferenceEnd::MetaClaim);
///
/// let mutation = edit::apply_preview(&mut doc, &preview)?;
/// assert_eq!(mutation.deleted_ids(), [ElementId::from("c1")]);
/// assert!(doc.get("c2").is_some_and(|c2| c2.meta_claims().is_empty()));
/// # Ok::<(), sacm::edit::EditError>(())
/// ```
#[cfg_attr(
    feature = "serde",
    doc = r"
With the `serde` feature, a `Preview` is `Serialize`, and this does not
compile:

```compile_fail
fn readable<T: serde::de::DeserializeOwned>() {}
readable::<sacm::edit::Preview>();
```

while the same check of a `DanglingReference` does:

```
fn readable<T: serde::de::DeserializeOwned>() {}
readable::<sacm::edit::DanglingReference>();
```
"
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Preview {
    revision: Revision,
    mutation: Mutation,
    dangling: Vec<DanglingReference>,
}

impl Preview {
    /// Builds the preview of `mutation`, taken at `revision`.
    pub(super) fn new(
        revision: Revision,
        mutation: Mutation,
        dangling: Vec<DanglingReference>,
    ) -> Self {
        Self {
            revision,
            mutation,
            dangling,
        }
    }

    /// The document's revision when the preview was taken.
    #[must_use]
    pub fn revision(&self) -> Revision {
        self.revision
    }

    /// The operation previewed, exactly as given.
    #[must_use]
    pub fn operation(&self) -> &Operation {
        self.mutation.operation()
    }

    /// The [`Mutation`] [`apply`](super::apply) would return for the
    /// operation, at the preview's revision.
    #[must_use]
    pub fn mutation(&self) -> &Mutation {
        &self.mutation
    }

    /// The references to deleted elements, which a delete removes. Empty for every other
    /// operation.
    ///
    /// - **Which.** For a [`DeleteElement`](super::DeleteElement), each reference held by
    ///   an element outside the deleted element's subtree that names a deleted element
    ///   (see [`DanglingReference`]).
    /// - **Order.** By holder in document pre-order, then by end in the order of
    ///   [`ReferenceEnd`]'s variants, then by position, with one entry per distinct id and
    ///   end.
    /// - **A relationship the delete also removes** has its references listed, since it
    ///   is outside the subtree.
    /// - **Not listed:** a reference that named no element before.
    /// - **The holder.** One in `mutation().deleted_ids()` is deleted. Every other holder
    ///   survives and loses the id, and is in `changed_ids()`.
    #[must_use]
    pub fn dangling_references(&self) -> &[DanglingReference] {
        &self.dangling
    }
}

/// One dangling reference of a delete: the element holding it, the end and
/// the id, and whether it crosses a package boundary.
///
/// - **Dangling.** The reference names an element the delete removes, and is held by an
///   element outside the deleted subtree.
/// - **Never left in the document.** The delete's policies remove it, by deleting the
///   holder or scrubbing the id, or the delete is refused.
/// - **Crossing.** It crosses a package boundary when the holder's nearest package is not
///   that of the element the id names; see
///   [`CrossPackageReferencePolicy`](super::CrossPackageReferencePolicy).
///
/// # Examples
///
/// ```
/// use sacm::edit::{self, CreateArgumentPackage, CreateClaim, DeleteElement, Operation, SetReferences};
/// use sacm::model::{Document, LangString, ReferenceEnd};
///
/// let mut doc = Document::new();
/// let name = LangString::new;
/// edit::apply(&mut doc, CreateArgumentPackage::root(name("P")).with_id("p"))?;
/// edit::apply(&mut doc, CreateArgumentPackage::root(name("Q")).with_id("q"))?;
/// edit::apply(&mut doc, CreateClaim::new("p", name("C1")).with_id("c1"))?;
/// edit::apply(&mut doc, CreateClaim::new("q", name("Far")).with_id("far"))?;
/// edit::apply(&mut doc, SetReferences::new("far", ReferenceEnd::CitedElement, ["c1"]))?;
///
/// let preview = edit::preview(&doc, &Operation::from(DeleteElement::new("c1")));
/// // The default policies refuse, so preview with the policies wanted.
/// assert!(preview.is_err());
/// let scrub = DeleteElement::new("c1")
///     .with_reference_policy(edit::ReferenceDeletePolicy::ScrubReferences)
///     .with_cross_package_policy(edit::CrossPackageReferencePolicy::ApplyReferencePolicy);
/// let preview = edit::preview(&doc, &Operation::from(scrub))?;
/// let [dangling] = preview.dangling_references() else { panic!("one reference") };
/// assert_eq!(dangling.holder().as_str(), "far");
/// assert_eq!(dangling.end(), ReferenceEnd::CitedElement);
/// assert_eq!(dangling.id().as_str(), "c1");
/// assert!(dangling.crosses_package()); // far is in q, c1 in p
/// # Ok::<(), sacm::edit::EditError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
pub struct DanglingReference {
    pub(super) holder: ElementId,
    pub(super) end: ReferenceEnd,
    pub(super) id: ElementId,
    pub(super) crosses_package: bool,
}

impl DanglingReference {
    /// The id of the element that holds the reference.
    #[must_use]
    pub fn holder(&self) -> &ElementId {
        &self.holder
    }

    /// The end that holds the reference, such as `source` or `metaClaim`.
    #[must_use]
    pub fn end(&self) -> ReferenceEnd {
        self.end
    }

    /// The id the reference names: an element the delete would remove.
    #[must_use]
    pub fn id(&self) -> &ElementId {
        &self.id
    }

    /// Whether the holder's nearest package is not that of the element the
    /// id names.
    #[must_use]
    pub fn crosses_package(&self) -> bool {
        self.crosses_package
    }
}

/// Serializes as ADR 0031 shapes it: the revision as a JSON number, the
/// mutation, and the dangling references.
#[cfg(feature = "serde")]
impl serde::Serialize for Preview {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        /// A preview as written.
        #[derive(serde::Serialize)]
        struct Written<'a> {
            revision: u64,
            mutation: &'a Mutation,
            dangling_references: &'a [DanglingReference],
        }
        Written {
            revision: self.revision.number(),
            mutation: &self.mutation,
            dangling_references: &self.dangling,
        }
        .serialize(serializer)
    }
}
