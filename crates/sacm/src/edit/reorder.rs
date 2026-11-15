//! The operation that reorders the children of an element, or the roots
//! (LIB-EDIT-010, D-ORDER, ADR 0029).

use std::collections::BTreeSet;

use super::EditError;
use super::plan::Plan;
use crate::model::{Document, ElementId, SacmElement};

/// Reorders some of the direct children of an element, or some of the
/// document's roots.
///
/// SACM marks no composition as ordered, but a document keeps the order its
/// elements were added or read in (D-ORDER), and [`xmi::write`](crate::xmi::write)
/// writes it. `order` names the elements to move, in their new order: they
/// go back into the positions they occupy now, and every other child keeps
/// its position. So `["d", "b"]` under a parent with the children `a b c d`
/// gives `a d c b`. The children are those of every role: a Note can be
/// moved before a Claim. Nothing else changes: not the parents, roles, ids
/// or the ids generated next.
///
/// [`new`](Self::new) reorders the children of `parent`, and
/// [`roots`](Self::roots) the roots. Apply it with
/// [`edit::apply`](super::apply). It refuses, in order:
/// 1. an unknown parent, as [`EditError::ElementNotFound`];
/// 2. an empty `order`, as [`EditError::InvalidOrder`];
/// 3. then, id by id in list order, an id that is no direct child of the
///    parent (or no root) as [`EditError::ChildNotFound`], and an id named
///    before as [`EditError::InvalidOrder`].
///
/// The parent is in [`changed_ids`](super::Mutation::changed_ids) when the
/// order of its children changes. A root reorder reports no id: the order
/// of the roots belongs to the document, not to an element.
///
/// # Examples
///
/// ```
/// use sacm::edit::{self, CreateArgumentPackage, CreateClaim, ReorderElements};
/// use sacm::model::{Document, ElementId, LangString};
///
/// let mut doc = Document::new();
/// edit::apply(&mut doc, CreateArgumentPackage::root(LangString::new("Arguments")).with_id("p"))?;
/// for id in ["a", "b", "c", "d"] {
///     edit::apply(&mut doc, CreateClaim::new("p", LangString::new(id)).with_id(id))?;
/// }
///
/// let reordered = edit::apply(&mut doc, ReorderElements::new("p", ["d", "b"]))?;
/// assert_eq!(reordered.changed_ids(), [ElementId::from("p")]);
/// let p = doc.get("p").expect("just created");
/// let children: Vec<&str> = p.children().map(|child| child.id().as_str()).collect();
/// assert_eq!(children, ["a", "d", "c", "b"]);
/// # Ok::<(), sacm::edit::EditError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
pub struct ReorderElements {
    #[cfg_attr(
        feature = "serde",
        serde(deserialize_with = "serde::Deserialize::deserialize")
    )]
    parent: Option<ElementId>,
    order: Vec<ElementId>,
}

impl ReorderElements {
    /// Reorders the children of `parent` that `order` names into the order
    /// given.
    #[must_use]
    pub fn new(
        parent: impl Into<ElementId>,
        order: impl IntoIterator<Item = impl Into<ElementId>>,
    ) -> Self {
        Self {
            parent: Some(parent.into()),
            order: order.into_iter().map(Into::into).collect(),
        }
    }

    /// Reorders the roots that `order` names into the order given.
    #[must_use]
    pub fn roots(order: impl IntoIterator<Item = impl Into<ElementId>>) -> Self {
        Self {
            parent: None,
            order: order.into_iter().map(Into::into).collect(),
        }
    }

    /// The id of the parent whose children are reordered, or `None` for the
    /// roots.
    #[must_use]
    pub fn parent(&self) -> Option<&ElementId> {
        self.parent.as_ref()
    }

    /// The ids to move, in their new order.
    #[must_use]
    pub fn order(&self) -> &[ElementId] {
        &self.order
    }

    /// Runs every check of the operation, and returns what it changes.
    pub(super) fn plan<'o>(&'o self, document: &Document) -> Result<Plan<'o>, EditError> {
        let parent = self.parent.as_ref();
        if let Some(parent) = parent {
            if !document.contains(parent) {
                return Err(EditError::ElementNotFound {
                    element: parent.clone(),
                });
            }
        }
        if self.order.is_empty() {
            return Err(EditError::InvalidOrder { repeated: None });
        }
        let mut seen = BTreeSet::new();
        for child in &self.order {
            let held_parent = document
                .get(child)
                .map(|found| found.parent().map(SacmElement::id));
            if held_parent != Some(parent) {
                return Err(EditError::ChildNotFound {
                    parent: parent.cloned(),
                    child: child.clone(),
                });
            }
            if !seen.insert(child) {
                return Err(EditError::InvalidOrder {
                    repeated: Some(child.clone()),
                });
            }
        }
        // The named ids, in the order they are held now: the order changes
        // exactly when that is not `order`.
        let held: Vec<&ElementId> = match parent.and_then(|parent| document.get(parent)) {
            Some(parent) => parent
                .children()
                .map(SacmElement::id)
                .filter(|child| seen.contains(child))
                .collect(),
            None => document
                .roots()
                .map(SacmElement::id)
                .filter(|root| seen.contains(root))
                .collect(),
        };
        let changes = !held.iter().copied().eq(&self.order);
        Ok(Plan::Reorder {
            parent,
            order: &self.order,
            changes,
        })
    }
}
