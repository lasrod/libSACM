//! The create operations (LIB-EDIT-001, ADR 0019).

use super::{EditError, Mutation};
use crate::model::{
    Document, ElementId, ElementKind, InsertError, LangString, SacmElement, admits, is_ncname,
};

/// Creates an `AssuranceCasePackage` (SACM 2.3 §9.2): a root, or a child of
/// an element whose class can hold one, such as another
/// `AssuranceCasePackage`.
///
/// Apply it with [`edit::apply`](super::apply). Without
/// [`with_id`](Self::with_id), the id is generated:
/// `assurance_case_package_<n>`.
///
/// ```
/// use sacm::edit::{self, CreateAssuranceCasePackage};
/// use sacm::model::{Document, LangString};
///
/// let mut doc = Document::new();
/// let name = |text: &str| LangString::new(text).with_lang("en");
/// edit::apply(&mut doc, CreateAssuranceCasePackage::root(name("Safety case")).with_id("case"))?;
/// edit::apply(&mut doc, CreateAssuranceCasePackage::new("case", name("Sub-case")))?;
/// let sub = doc.get("assurance_case_package_1").expect("just created");
/// assert_eq!(sub.parent().map(|parent| parent.id().as_str()), Some("case"));
/// # Ok::<(), sacm::edit::EditError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreateAssuranceCasePackage {
    parent: Option<ElementId>,
    id: Option<ElementId>,
    name: LangString,
}

impl CreateAssuranceCasePackage {
    /// Creates a root `AssuranceCasePackage` named `name`, after the
    /// document's last root.
    #[must_use]
    pub fn root(name: LangString) -> Self {
        Self {
            parent: None,
            id: None,
            name,
        }
    }

    /// Creates an `AssuranceCasePackage` named `name` as the last child of
    /// `parent`.
    #[must_use]
    pub fn new(parent: impl Into<ElementId>, name: LangString) -> Self {
        Self {
            parent: Some(parent.into()),
            id: None,
            name,
        }
    }

    /// Gives the new element the id `id` instead of a generated one. It must
    /// be an XML `NCName` and not already in the document.
    #[must_use]
    pub fn with_id(self, id: impl Into<ElementId>) -> Self {
        Self {
            id: Some(id.into()),
            ..self
        }
    }

    /// The parent's id, or `None` for a root.
    #[must_use]
    pub fn parent(&self) -> Option<&ElementId> {
        self.parent.as_ref()
    }

    /// The id given with [`with_id`](Self::with_id), or `None` when the id
    /// is to be generated.
    #[must_use]
    pub fn id(&self) -> Option<&ElementId> {
        self.id.as_ref()
    }

    /// The new element's name.
    #[must_use]
    pub fn name(&self) -> &LangString {
        &self.name
    }

    /// Checks and applies the operation.
    pub(super) fn apply(self, document: &mut Document) -> Result<Mutation, EditError> {
        create(
            document,
            ElementKind::AssuranceCasePackage,
            self.parent.as_ref(),
            self.id,
            self.name,
        )
    }
}

/// Creates an `ArgumentPackage` (SACM 2.3 §11.4): a root, or a child of an
/// element whose class can hold one, such as an `AssuranceCasePackage` or
/// another `ArgumentPackage`.
///
/// Apply it with [`edit::apply`](super::apply). Without
/// [`with_id`](Self::with_id), the id is generated: `argument_package_<n>`.
///
/// ```
/// use sacm::edit::{self, CreateArgumentPackage};
/// use sacm::model::{Document, ElementKind, LangString};
///
/// let mut doc = Document::new();
/// edit::apply(&mut doc, CreateArgumentPackage::root(LangString::new("Arguments")))?;
/// let package = doc.get("argument_package_1").expect("just created");
/// assert_eq!(package.kind(), ElementKind::ArgumentPackage);
/// # Ok::<(), sacm::edit::EditError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreateArgumentPackage {
    parent: Option<ElementId>,
    id: Option<ElementId>,
    name: LangString,
}

impl CreateArgumentPackage {
    /// Creates a root `ArgumentPackage` named `name`, after the document's
    /// last root.
    #[must_use]
    pub fn root(name: LangString) -> Self {
        Self {
            parent: None,
            id: None,
            name,
        }
    }

    /// Creates an `ArgumentPackage` named `name` as the last child of
    /// `parent`.
    #[must_use]
    pub fn new(parent: impl Into<ElementId>, name: LangString) -> Self {
        Self {
            parent: Some(parent.into()),
            id: None,
            name,
        }
    }

    /// Gives the new element the id `id` instead of a generated one. It must
    /// be an XML `NCName` and not already in the document.
    #[must_use]
    pub fn with_id(self, id: impl Into<ElementId>) -> Self {
        Self {
            id: Some(id.into()),
            ..self
        }
    }

    /// The parent's id, or `None` for a root.
    #[must_use]
    pub fn parent(&self) -> Option<&ElementId> {
        self.parent.as_ref()
    }

    /// The id given with [`with_id`](Self::with_id), or `None` when the id
    /// is to be generated.
    #[must_use]
    pub fn id(&self) -> Option<&ElementId> {
        self.id.as_ref()
    }

    /// The new element's name.
    #[must_use]
    pub fn name(&self) -> &LangString {
        &self.name
    }

    /// Checks and applies the operation.
    pub(super) fn apply(self, document: &mut Document) -> Result<Mutation, EditError> {
        create(
            document,
            ElementKind::ArgumentPackage,
            self.parent.as_ref(),
            self.id,
            self.name,
        )
    }
}

/// Creates a `Claim` (SACM 2.3 §11.11) as a child of an element whose class
/// can hold one, such as an `ArgumentPackage`. A Claim is never a root.
///
/// Its `assertionDeclaration` is `asserted`. Apply it with
/// [`edit::apply`](super::apply). Without [`with_id`](Self::with_id), the id
/// is generated: `claim_<n>`.
///
/// ```
/// use sacm::edit::{self, CreateArgumentPackage, CreateClaim};
/// use sacm::model::{Document, LangString};
///
/// let mut doc = Document::new();
/// let name = |text: &str| LangString::new(text).with_lang("en");
/// edit::apply(&mut doc, CreateArgumentPackage::root(name("Arguments")).with_id("arguments"))?;
/// edit::apply(&mut doc, CreateClaim::new("arguments", name("The system is safe")).with_id("c_top"))?;
/// let claim = doc.get("c_top").expect("just created");
/// assert_eq!(claim.name().and_then(LangString::content), Some("The system is safe"));
/// # Ok::<(), sacm::edit::EditError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreateClaim {
    parent: ElementId,
    id: Option<ElementId>,
    name: LangString,
}

impl CreateClaim {
    /// Creates a `Claim` named `name` as the last child of `parent`.
    #[must_use]
    pub fn new(parent: impl Into<ElementId>, name: LangString) -> Self {
        Self {
            parent: parent.into(),
            id: None,
            name,
        }
    }

    /// Gives the new element the id `id` instead of a generated one. It must
    /// be an XML `NCName` and not already in the document.
    #[must_use]
    pub fn with_id(self, id: impl Into<ElementId>) -> Self {
        Self {
            id: Some(id.into()),
            ..self
        }
    }

    /// The parent's id.
    #[must_use]
    pub fn parent(&self) -> &ElementId {
        &self.parent
    }

    /// The id given with [`with_id`](Self::with_id), or `None` when the id
    /// is to be generated.
    #[must_use]
    pub fn id(&self) -> Option<&ElementId> {
        self.id.as_ref()
    }

    /// The new element's name.
    #[must_use]
    pub fn name(&self) -> &LangString {
        &self.name
    }

    /// Checks and applies the operation.
    pub(super) fn apply(self, document: &mut Document) -> Result<Mutation, EditError> {
        create(
            document,
            ElementKind::Claim,
            Some(&self.parent),
            self.id,
            self.name,
        )
    }
}

/// Creates an element of `kind` under `parent` (a root when `None`), with the
/// id `id` or a generated one, after every check of ADR 0019 in order. On a
/// refusal the document, its counters included, is unchanged.
fn create(
    document: &mut Document,
    kind: ElementKind,
    parent: Option<&ElementId>,
    id: Option<ElementId>,
    name: LangString,
) -> Result<Mutation, EditError> {
    // 1 and 2: the parent exists and its class can hold `kind`.
    if let Some(parent) = parent {
        let Some(parent_kind) = document.get(parent).map(SacmElement::kind) else {
            return Err(EditError::ParentNotFound {
                parent: parent.clone(),
            });
        };
        if !admits(parent_kind, kind) {
            return Err(EditError::InvalidParent {
                parent: parent.clone(),
                parent_kind,
                kind,
            });
        }
    }
    // 3 and 4: a caller id is an NCName and not in use; otherwise generate one.
    let (id, generated) = if let Some(id) = id {
        if !is_ncname(id.as_str()) {
            return Err(EditError::InvalidId { id });
        }
        if document.contains(&id) {
            return Err(EditError::DuplicateId { id });
        }
        (id, None)
    } else {
        let (id, n) = document.next_generated_id(kind);
        (id, Some(n))
    };
    match document.insert_node(parent, kind, id.clone(), Some(name)) {
        Ok(()) => {}
        Err(InsertError::UnknownParent(parent)) => {
            return Err(EditError::ParentNotFound { parent });
        }
        Err(InsertError::ContainmentNotAllowed {
            parent,
            parent_kind,
            kind,
            ..
        }) => {
            return Err(EditError::InvalidParent {
                parent,
                parent_kind,
                kind,
            });
        }
        Err(InsertError::DuplicateId(id)) => return Err(EditError::DuplicateId { id }),
        // Not reached: the id is an NCName, so not empty, and the operations
        // create named ModelElement kinds, as roots only for package kinds.
        Err(
            InsertError::EmptyId
            | InsertError::NameNotAllowed(..)
            | InsertError::RootNotAllowed(..),
        ) => return Err(EditError::InvalidId { id }),
    }
    if let Some(n) = generated {
        document.commit_generated_id(kind, n);
    }
    Ok(Mutation::created(vec![id]))
}
