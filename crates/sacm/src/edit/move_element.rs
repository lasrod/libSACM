//! The move operation (LIB-EDIT-017; ADR 0036 D1).

use super::EditError;
use super::plan::Plan;
use crate::model::{Document, ElementId, Preserved, SacmElement, admits};

/// Moves an element, with everything it contains and its preserved content,
/// under another parent ([`new`](Self::new)) or to the roots
/// ([`root`](Self::root)): after the last child or root, or at a position
/// ([`with_position`](Self::with_position)), its index among the new
/// parent's children (or the roots) after the move.
///
/// Apply it with [`edit::apply`](super::apply). Ids, references and the
/// element's features do not change, and neither do the ids generated next;
/// its parent, its role and the order do, and
/// [`xmi::write`](crate::xmi::write) writes the role of the new parent's
/// class (an `argumentElement` of an ArgumentPackage, say, or an
/// `artifactElement` of an ArtifactPackage). A preserved child of either
/// parent keeps its place after the typed child it followed. The refusals
/// and what the [`Mutation`](super::Mutation) reports are in the
/// [module docs](super#moving-and-copying-elements).
///
/// # Examples
///
/// Drag a claim into another package, as its first child:
///
/// ```
/// use sacm::edit::{self, CreateArgumentPackage, CreateClaim, MoveElement};
/// use sacm::model::{Document, ElementId, LangString};
///
/// let mut doc = Document::new();
/// let name = LangString::new;
/// edit::apply(&mut doc, CreateArgumentPackage::root(name("P")).with_id("p"))?;
/// edit::apply(&mut doc, CreateArgumentPackage::root(name("Q")).with_id("q"))?;
/// edit::apply(&mut doc, CreateClaim::new("p", name("C1")).with_id("c1"))?;
/// edit::apply(&mut doc, CreateClaim::new("q", name("D1")).with_id("d1"))?;
///
/// let mutation = edit::apply(&mut doc, MoveElement::new("c1", "q").with_position(0))?;
/// assert_eq!(mutation.changed_ids(), [ElementId::from("c1")]);
/// let q: Vec<&str> = doc.get("q").expect("a root").children().map(|child| child.id().as_str()).collect();
/// assert_eq!(q, ["c1", "d1"]);
///
/// // Moved to the place it holds, it changes nothing.
/// let again = edit::apply(&mut doc, MoveElement::new("c1", "q").with_position(0))?;
/// assert!(again.changed_ids().is_empty());
/// # Ok::<(), sacm::edit::EditError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
pub struct MoveElement {
    element: ElementId,
    #[cfg_attr(
        feature = "serde",
        serde(deserialize_with = "serde::Deserialize::deserialize")
    )]
    parent: Option<ElementId>,
    #[cfg_attr(feature = "serde", serde(default))]
    position: Option<usize>,
}

impl MoveElement {
    /// Moves `element` under `parent`, after its last child.
    #[must_use]
    pub fn new(element: impl Into<ElementId>, parent: impl Into<ElementId>) -> Self {
        Self {
            element: element.into(),
            parent: Some(parent.into()),
            position: None,
        }
    }

    /// Moves `element` to the roots, after the last root. Only a package may
    /// be a root.
    #[must_use]
    pub fn root(element: impl Into<ElementId>) -> Self {
        Self {
            element: element.into(),
            parent: None,
            position: None,
        }
    }

    /// Puts the element at `position` instead of last: its index among the
    /// new parent's children (or the roots) after the move, from 0 to the
    /// number of the others. Past that, [`apply`](super::apply) refuses with
    /// [`EditError::PositionOutOfRange`].
    #[must_use]
    pub fn with_position(self, position: usize) -> Self {
        Self {
            position: Some(position),
            ..self
        }
    }

    /// The id of the element to move.
    #[must_use]
    pub fn element(&self) -> &ElementId {
        &self.element
    }

    /// The new parent's id, or `None` for the roots.
    #[must_use]
    pub fn parent(&self) -> Option<&ElementId> {
        self.parent.as_ref()
    }

    /// The position given with [`with_position`](Self::with_position), or
    /// `None` for after the last.
    #[must_use]
    pub fn position(&self) -> Option<usize> {
        self.position
    }

    /// Runs every check of the operation, in the order of ADR 0036 D1, and
    /// returns what it changes.
    pub(super) fn plan<'o>(&'o self, document: &Document) -> Result<Plan<'o>, EditError> {
        // 1. The element is in the document.
        let Some(element) = document.get(&self.element) else {
            return Err(EditError::ElementNotFound {
                element: self.element.clone(),
            });
        };
        let siblings = if let Some(parent_id) = &self.parent {
            // 2. The parent is in the document.
            let Some(parent) = document.get(parent_id) else {
                return Err(EditError::ParentNotFound {
                    parent: parent_id.clone(),
                });
            };
            // 3. Its class can hold the element's kind.
            if !admits(parent.kind(), element.kind()) {
                return Err(EditError::InvalidParent {
                    parent: parent_id.clone(),
                    parent_kind: parent.kind(),
                    kind: element.kind(),
                });
            }
            // 4. It is not the element, nor inside it.
            let mut up = Some(parent);
            while let Some(ancestor) = up {
                if ancestor.id() == element.id() {
                    return Err(EditError::ParentInSubtree {
                        element: self.element.clone(),
                        parent: parent_id.clone(),
                    });
                }
                up = ancestor.parent();
            }
            // 5. The element keeps no type attribute that only a root can
            // carry (LIB-COMPAT-019, D-ROOT-NAME).
            if let Some(attr) = element.preserved().and_then(Preserved::kept_type) {
                return Err(EditError::KeptRootType {
                    element: self.element.clone(),
                    attribute: attr.name.written(),
                    value: attr.value.clone(),
                });
            }
            parent.children().count()
        } else {
            // 3. A root is a package.
            if !element.kind().is_package() {
                return Err(EditError::InvalidRoot {
                    element: self.element.clone(),
                    kind: element.kind(),
                });
            }
            document.roots().count()
        };
        // 6. The position is at most the number of the others.
        let stays = element.parent().map(SacmElement::id) == self.parent.as_ref();
        let others = siblings.saturating_sub(usize::from(stays));
        let position = self.position.unwrap_or(others);
        if position > others {
            return Err(EditError::PositionOutOfRange {
                parent: self.parent.clone(),
                position,
                children: others,
            });
        }
        Ok(
            match document.move_changes(&self.element, self.parent.as_ref(), position) {
                None => Plan::Unchanged,
                Some(changed) => Plan::Move {
                    element: &self.element,
                    parent: self.parent.as_ref(),
                    position,
                    changed,
                },
            },
        )
    }
}
