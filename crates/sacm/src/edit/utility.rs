//! The create operations of the `UtilityElement`s (SACM23-BASE-004,
//! SACM23-BASE-005; LIB-EDIT-001, ADR 0019, ADR 0025).

use std::borrow::Cow;

use super::EditError;
use super::create::plan_create;
use super::plan::{Features, Plan};
use crate::model::{Document, ElementId, ElementKind, MultiLangString};

/// Creates a `Description` (SACM 2.3 §8.9) as the last child of a
/// `ModelElement`, in its `description` role, with a `content`.
///
/// - **Content.** A [`MultiLangString`], kept exactly as given: a
///   [`LangString`](crate::model::LangString) converts into one with one value.
/// - **Parent.** Any `ModelElement`. A `UtilityElement` contains no elements, so one as
///   the parent is [`EditError::InvalidParent`].
/// - **Several descriptions are kept** (D-DESC-MULT), and
///   [`validation::validate`](crate::validation::validate) reports more than one as a
///   SACM-MULT-001 Warning (SACM23-VAL-011).
/// - **Id.** Without [`with_id`](Self::with_id), it is generated: `description_<n>`.
/// - **Apply it** with [`edit::apply`](super::apply).
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
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
pub struct CreateDescription {
    parent: ElementId,
    #[cfg_attr(feature = "serde", serde(default))]
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

    /// Gives the new element the id `id` instead of a generated one.
    ///
    /// It must be an XML `NCName` and not already in the document. Otherwise
    /// [`apply`](super::apply) refuses with [`EditError::InvalidId`](super::EditError::InvalidId)
    /// or [`EditError::DuplicateId`](super::EditError::DuplicateId).
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

    /// Runs every check of the operation, and returns what it creates.
    pub(super) fn plan<'o>(&'o self, document: &Document) -> Result<Plan<'o>, EditError> {
        plan_utility(
            document,
            ElementKind::Description,
            &self.parent,
            self.id.as_ref(),
            &self.content,
            None,
        )
    }
}

/// Creates an `ImplementationConstraint` (SACM 2.3 §8.8) as the last child of a
/// `ModelElement`, in its `implementationConstraint` role, with a `content`.
///
/// - **What it is.** A condition an abstract element must meet before it can be made
///   concrete.
/// - **Abstract elements only, by §8.6,** which says an element should have
///   implementation constraints only if it is abstract; [`SetFlag`](super::SetFlag) sets
///   that.
/// - **Not checked here.** A create does not refuse the combination;
///   [`validation::validate`](crate::validation::validate) reports it as a SACM-ABS-001
///   Error (SACM23-VAL-021).
/// - **Rules.** Those of [`CreateDescription`].
/// - **Id.** Without [`with_id`](Self::with_id), it is generated:
///   `implementation_constraint_<n>`.
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
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
pub struct CreateImplementationConstraint {
    parent: ElementId,
    #[cfg_attr(feature = "serde", serde(default))]
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

    /// Gives the new element the id `id` instead of a generated one.
    ///
    /// It must be an XML `NCName` and not already in the document. Otherwise
    /// [`apply`](super::apply) refuses with [`EditError::InvalidId`](super::EditError::InvalidId)
    /// or [`EditError::DuplicateId`](super::EditError::DuplicateId).
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

    /// Runs every check of the operation, and returns what it creates.
    pub(super) fn plan<'o>(&'o self, document: &Document) -> Result<Plan<'o>, EditError> {
        plan_utility(
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
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
pub struct CreateNote {
    parent: ElementId,
    #[cfg_attr(feature = "serde", serde(default))]
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

    /// Gives the new element the id `id` instead of a generated one.
    ///
    /// It must be an XML `NCName` and not already in the document. Otherwise
    /// [`apply`](super::apply) refuses with [`EditError::InvalidId`](super::EditError::InvalidId)
    /// or [`EditError::DuplicateId`](super::EditError::DuplicateId).
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

    /// Runs every check of the operation, and returns what it creates.
    pub(super) fn plan<'o>(&'o self, document: &Document) -> Result<Plan<'o>, EditError> {
        plan_utility(
            document,
            ElementKind::Note,
            &self.parent,
            self.id.as_ref(),
            &self.content,
            None,
        )
    }
}

/// Creates a `TaggedValue` (SACM 2.3 §8.12), a key/value pair, as the last child of a
/// `ModelElement`, in its `taggedValue` role, with a `key` and a `content`.
///
/// - **`key` and `content`** are [`MultiLangString`]s, kept exactly as given. A
///   [`LangString`](crate::model::LangString) converts into one with one value, and a
///   `&str` or `String` converts too.
/// - **Its value is its `content`** (§8.7). In XMI, a `<value>` element is one
///   `LangString` inside `key` or `content` (§8.5), not the tagged value's value.
/// - **Equal keys.** `TaggedValue`s with equal keys are all kept. To set a tagged value
///   by its key, creating it only when there is none, use
///   [`SetTaggedValue`](super::SetTaggedValue).
/// - **Rules.** Otherwise those of [`CreateDescription`].
/// - **Id.** Without [`with_id`](Self::with_id), it is generated: `tagged_value_<n>`.
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
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
pub struct CreateTaggedValue {
    parent: ElementId,
    #[cfg_attr(feature = "serde", serde(default))]
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

    /// Gives the new element the id `id` instead of a generated one.
    ///
    /// It must be an XML `NCName` and not already in the document. Otherwise
    /// [`apply`](super::apply) refuses with [`EditError::InvalidId`](super::EditError::InvalidId)
    /// or [`EditError::DuplicateId`](super::EditError::DuplicateId).
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

    /// Runs every check of the operation, and returns what it creates.
    pub(super) fn plan<'o>(&'o self, document: &Document) -> Result<Plan<'o>, EditError> {
        plan_utility(
            document,
            ElementKind::TaggedValue,
            &self.parent,
            self.id.as_ref(),
            &self.content,
            Some(&self.key),
        )
    }
}

/// The plan of a `UtilityElement` of `kind` under `parent` with `content`,
/// and `key` for a `TaggedValue`, under the checks every create runs.
fn plan_utility<'o>(
    document: &Document,
    kind: ElementKind,
    parent: &'o ElementId,
    id: Option<&ElementId>,
    content: &'o MultiLangString,
    key: Option<&'o MultiLangString>,
) -> Result<Plan<'o>, EditError> {
    plan_create(
        document,
        kind,
        Some(parent),
        id,
        Features::Texts {
            content,
            key: key.map(Cow::Borrowed),
        },
    )
}
