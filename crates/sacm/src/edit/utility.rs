//! The create operations of the `UtilityElement`s (SACM23-BASE-004,
//! SACM23-BASE-005; LIB-EDIT-001, ADR 0019, ADR 0025).

use super::EditError;
use super::create::{Features, create};
use crate::model::{Document, ElementId, ElementKind, MultiLangString};

/// Creates a `Description` (SACM 2.3 §8.9) as the last child of a
/// `ModelElement`, in its `description` role, with a `content`.
///
/// The content is a [`MultiLangString`], kept exactly as given: a
/// [`LangString`](crate::model::LangString) converts into one with one value.
/// The parent can be any `ModelElement`. A `UtilityElement` contains no
/// elements, so it is never a parent: one as the parent is
/// [`EditError::InvalidParent`]. Several descriptions
/// are kept (D-DESC-MULT). Apply it with [`edit::apply`](super::apply).
/// Without [`with_id`](Self::with_id), the id is generated:
/// `description_<n>`.
///
/// # Examples
///
/// ```
/// use sacm::edit::{self, CreateArgumentPackage, CreateDescription};
/// use sacm::model::{Document, LangString, MultiLangString};
///
/// let mut doc = Document::new();
/// let text = |content: &str, lang: &str| LangString::new(content).with_lang(lang);
/// edit::apply(&mut doc, CreateArgumentPackage::root(text("Arguments", "en")).with_id("arguments"))?;
///
/// let content: MultiLangString =
///     [text("Why the brakes are safe", "en"), text("Varför bromsarna är säkra", "sv")]
///         .into_iter()
///         .collect();
/// let created = edit::apply(&mut doc, CreateDescription::new("arguments", content.clone()))?;
/// assert_eq!(created.created_ids()[0].as_str(), "description_1");
///
/// let arguments = doc.get("arguments").expect("just created");
/// let description = arguments.descriptions().next().expect("just created");
/// assert_eq!(description.content(), Some(&content));
/// # Ok::<(), sacm::edit::EditError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreateDescription {
    parent: ElementId,
    id: Option<ElementId>,
    content: MultiLangString,
}

impl CreateDescription {
    /// Creates a `Description` with `content` as the last child of `parent`.
    #[must_use]
    pub fn new(parent: impl Into<ElementId>, content: impl Into<MultiLangString>) -> Self {
        Self {
            parent: parent.into(),
            id: None,
            content: content.into(),
        }
    }

    /// Gives the new element the id `id` instead of a generated one. It must
    /// be an XML `NCName` and not already in the document. Otherwise
    /// [`apply`](super::apply) refuses with
    /// [`EditError::InvalidId`](super::EditError::InvalidId) or
    /// [`EditError::DuplicateId`](super::EditError::DuplicateId).
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

    /// The new element's `content`.
    #[must_use]
    pub fn content(&self) -> &MultiLangString {
        &self.content
    }

    /// Checks and applies the operation, and returns the ids it created.
    pub(super) fn apply(&self, document: &mut Document) -> Result<Vec<ElementId>, EditError> {
        create_utility(
            document,
            ElementKind::Description,
            &self.parent,
            self.id.as_ref(),
            &self.content,
            None,
        )
    }
}

/// Creates an `ImplementationConstraint` (SACM 2.3 §8.8) as the last child
/// of a `ModelElement`, in its `implementationConstraint` role, with a
/// `content`.
///
/// An implementation constraint is a condition an abstract element must meet
/// before it can be made concrete. §8.6 says an element should have
/// implementation constraints only if it is abstract; this release cannot set
/// `isAbstract` yet, and a later validation rule (SACM23-VAL-021) will report
/// the combination.
///
/// It follows the rules of [`CreateDescription`]. Without
/// [`with_id`](Self::with_id), the id is generated:
/// `implementation_constraint_<n>`.
///
/// # Examples
///
/// ```
/// use sacm::edit::{self, CreateArgumentPackage, CreateClaim, CreateImplementationConstraint};
/// use sacm::model::{Document, LangString};
///
/// let mut doc = Document::new();
/// edit::apply(&mut doc, CreateArgumentPackage::root(LangString::new("Arguments")).with_id("arguments"))?;
/// edit::apply(&mut doc, CreateClaim::new("arguments", LangString::new("C1")).with_id("c1"))?;
/// let constraint = CreateImplementationConstraint::new("c1", LangString::new("Name the brake under test"));
/// let created = edit::apply(&mut doc, constraint)?;
/// assert_eq!(created.created_ids()[0].as_str(), "implementation_constraint_1");
/// # Ok::<(), sacm::edit::EditError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreateImplementationConstraint {
    parent: ElementId,
    id: Option<ElementId>,
    content: MultiLangString,
}

impl CreateImplementationConstraint {
    /// Creates an `ImplementationConstraint` with `content` as the last child
    /// of `parent`.
    #[must_use]
    pub fn new(parent: impl Into<ElementId>, content: impl Into<MultiLangString>) -> Self {
        Self {
            parent: parent.into(),
            id: None,
            content: content.into(),
        }
    }

    /// Gives the new element the id `id` instead of a generated one. It must
    /// be an XML `NCName` and not already in the document. Otherwise
    /// [`apply`](super::apply) refuses with
    /// [`EditError::InvalidId`](super::EditError::InvalidId) or
    /// [`EditError::DuplicateId`](super::EditError::DuplicateId).
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

    /// The new element's `content`.
    #[must_use]
    pub fn content(&self) -> &MultiLangString {
        &self.content
    }

    /// Checks and applies the operation, and returns the ids it created.
    pub(super) fn apply(&self, document: &mut Document) -> Result<Vec<ElementId>, EditError> {
        create_utility(
            document,
            ElementKind::ImplementationConstraint,
            &self.parent,
            self.id.as_ref(),
            &self.content,
            None,
        )
    }
}

/// Creates a `Note` (SACM 2.3 §8.11) as the last child of a `ModelElement`,
/// in its `note` role, with a `content`.
///
/// It follows the rules of [`CreateDescription`]. Without
/// [`with_id`](Self::with_id), the id is generated: `note_<n>`.
///
/// # Examples
///
/// ```
/// use sacm::edit::{self, CreateArgumentPackage, CreateNote};
/// use sacm::model::{Document, LangString};
///
/// let mut doc = Document::new();
/// edit::apply(&mut doc, CreateArgumentPackage::root(LangString::new("Arguments")).with_id("arguments"))?;
/// let note = CreateNote::new("arguments", LangString::new("Reviewed").with_lang("en"));
/// edit::apply(&mut doc, note.with_id("n1"))?;
/// let arguments = doc.get("arguments").expect("just created");
/// assert_eq!(arguments.notes().map(|n| n.id().as_str()).collect::<Vec<_>>(), ["n1"]);
/// # Ok::<(), sacm::edit::EditError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreateNote {
    parent: ElementId,
    id: Option<ElementId>,
    content: MultiLangString,
}

impl CreateNote {
    /// Creates a `Note` with `content` as the last child of `parent`.
    #[must_use]
    pub fn new(parent: impl Into<ElementId>, content: impl Into<MultiLangString>) -> Self {
        Self {
            parent: parent.into(),
            id: None,
            content: content.into(),
        }
    }

    /// Gives the new element the id `id` instead of a generated one. It must
    /// be an XML `NCName` and not already in the document. Otherwise
    /// [`apply`](super::apply) refuses with
    /// [`EditError::InvalidId`](super::EditError::InvalidId) or
    /// [`EditError::DuplicateId`](super::EditError::DuplicateId).
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

    /// The new element's `content`.
    #[must_use]
    pub fn content(&self) -> &MultiLangString {
        &self.content
    }

    /// Checks and applies the operation, and returns the ids it created.
    pub(super) fn apply(&self, document: &mut Document) -> Result<Vec<ElementId>, EditError> {
        create_utility(
            document,
            ElementKind::Note,
            &self.parent,
            self.id.as_ref(),
            &self.content,
            None,
        )
    }
}

/// Creates a `TaggedValue` (SACM 2.3 §8.12), a key/value pair, as the last
/// child of a `ModelElement`, in its `taggedValue` role, with a `key` and a
/// `content`.
///
/// Both are [`MultiLangString`]s, kept exactly as given; a
/// [`LangString`](crate::model::LangString) converts into one with one value.
/// The tagged value's value is its `content` (§8.7). In XMI, a `<value>`
/// element is one `LangString` inside `key` or `content` (§8.5), not the
/// tagged value's value. `TaggedValue`s with equal keys are all kept. It
/// otherwise follows the rules of [`CreateDescription`]. Without
/// [`with_id`](Self::with_id), the id is generated: `tagged_value_<n>`.
///
/// # Examples
///
/// ```
/// use sacm::edit::{self, CreateArgumentPackage, CreateClaim, CreateTaggedValue};
/// use sacm::model::{Document, LangString, MultiLangString};
///
/// let mut doc = Document::new();
/// edit::apply(&mut doc, CreateArgumentPackage::root(LangString::new("Arguments")).with_id("arguments"))?;
/// edit::apply(&mut doc, CreateClaim::new("arguments", LangString::new("C1")).with_id("c1"))?;
/// let created = edit::apply(
///     &mut doc,
///     CreateTaggedValue::new("c1", LangString::new("ASIL"), LangString::new("D")),
/// )?;
/// assert_eq!(created.created_ids()[0].as_str(), "tagged_value_1");
///
/// let tagged = doc.get("tagged_value_1").expect("just created");
/// assert_eq!(tagged.key(), Some(&MultiLangString::from(LangString::new("ASIL"))));
/// # Ok::<(), sacm::edit::EditError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreateTaggedValue {
    parent: ElementId,
    id: Option<ElementId>,
    key: MultiLangString,
    content: MultiLangString,
}

impl CreateTaggedValue {
    /// Creates a `TaggedValue` with `key` and `content` as the last child of
    /// `parent`.
    #[must_use]
    pub fn new(
        parent: impl Into<ElementId>,
        key: impl Into<MultiLangString>,
        content: impl Into<MultiLangString>,
    ) -> Self {
        Self {
            parent: parent.into(),
            id: None,
            key: key.into(),
            content: content.into(),
        }
    }

    /// Gives the new element the id `id` instead of a generated one. It must
    /// be an XML `NCName` and not already in the document. Otherwise
    /// [`apply`](super::apply) refuses with
    /// [`EditError::InvalidId`](super::EditError::InvalidId) or
    /// [`EditError::DuplicateId`](super::EditError::DuplicateId).
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

    /// The new element's `key`.
    #[must_use]
    pub fn key(&self) -> &MultiLangString {
        &self.key
    }

    /// The new element's `content`.
    #[must_use]
    pub fn content(&self) -> &MultiLangString {
        &self.content
    }

    /// Checks and applies the operation, and returns the ids it created.
    pub(super) fn apply(&self, document: &mut Document) -> Result<Vec<ElementId>, EditError> {
        create_utility(
            document,
            ElementKind::TaggedValue,
            &self.parent,
            self.id.as_ref(),
            &self.content,
            Some(&self.key),
        )
    }
}

/// Creates a `UtilityElement` of `kind` under `parent` with `content`, and
/// `key` for a `TaggedValue`, under the checks every create runs.
fn create_utility(
    document: &mut Document,
    kind: ElementKind,
    parent: &ElementId,
    id: Option<&ElementId>,
    content: &MultiLangString,
    key: Option<&MultiLangString>,
) -> Result<Vec<ElementId>, EditError> {
    create(
        document,
        kind,
        Some(parent),
        id,
        Features::Texts { content, key },
    )
}
