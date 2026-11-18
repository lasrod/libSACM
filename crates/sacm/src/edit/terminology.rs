//! The create operations of the Terminology package: `TerminologyPackage`,
//! `Category`, `Term`, `Expression` and `TerminologyGroup` (SACM23-TERM-001
//! to SACM23-TERM-006; LIB-EDIT-001, ADR 0019, ADR 0028, ADR 0034 amendment
//! 3).
//!
//! There is one create per class. Each runs ADR 0019's four checks through
//! the containment table, so a parent is legal exactly when a composite role
//! of its class admits the new kind: a TerminologyPackage goes in an
//! AssuranceCasePackage, a TerminologyPackage or an ArtifactPackage, or is a
//! root, and the other kinds go in a TerminologyPackage or an
//! ArtifactPackage. The reference ends then get ADR 0028's reference checks,
//! each phase over the ends in [`ReferenceEnd::ALL`] order. A Term's or an
//! Expression's `value` is required (§10.9 `value:String[1]`) and held as
//! given.

use super::EditError;
use super::argumentation::plan_referencing;
use super::create::{checked_place, plan_create};
use super::plan::{Features, NewElement, Plan};
use super::references::{check_targets, expression_ids};
use crate::model::{Document, ElementId, ElementKind, LangString, ReferenceEnd, StringAttribute};

/// Creates a `TerminologyPackage` (SACM 2.3 §10.4): a root, or a child of an
/// element whose class can hold one, an `AssuranceCasePackage` (its
/// `terminologyPackage` role), another `TerminologyPackage` (its
/// `terminologyElement` role, "`TerminologyPackages` can be nested") or an
/// `ArtifactPackage` (its `artifactElement` role).
///
/// "The TerminologyPackage is the container element for SACM terminology
/// assets" (§10.4): Categories, Terms, Expressions, `TerminologyGroups` and
/// nested `TerminologyPackages`. It is the Terminology unit of interchange
/// (§2.5). Apply it with [`edit::apply`](super::apply). Without
/// [`with_id`](Self::with_id), the id is generated: `terminology_package_<n>`.
///
/// # Examples
///
/// ```
/// use sacm::edit::{self, CreateAssuranceCasePackage, CreateTerminologyPackage};
/// use sacm::model::{Document, ElementKind, LangString};
///
/// let mut doc = Document::new();
/// let name = |text: &str| LangString::new(text).with_lang("en");
/// edit::apply(&mut doc, CreateAssuranceCasePackage::root(name("Blender case")).with_id("case"))?;
/// edit::apply(&mut doc, CreateTerminologyPackage::new("case", name("Blender terms")))?;
///
/// let terms = doc.get("terminology_package_1").expect("just created");
/// assert_eq!(terms.kind(), ElementKind::TerminologyPackage);
/// assert_eq!(terms.parent().map(|parent| parent.id().as_str()), Some("case"));
/// # Ok::<(), sacm::edit::EditError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
pub struct CreateTerminologyPackage {
    #[cfg_attr(
        feature = "serde",
        serde(deserialize_with = "serde::Deserialize::deserialize")
    )]
    parent: Option<ElementId>,
    #[cfg_attr(feature = "serde", serde(default))]
    id: Option<ElementId>,
    name: LangString,
}

impl CreateTerminologyPackage {
    /// Creates a root `TerminologyPackage` named `name`, after the
    /// document's last root.
    #[must_use]
    pub fn root(name: LangString) -> Self {
        Self {
            parent: None,
            id: None,
            name,
        }
    }

    /// Creates a `TerminologyPackage` named `name` as the last child of
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

    /// Runs every check of the operation, and returns what it creates.
    pub(super) fn plan<'o>(&'o self, document: &Document) -> Result<Plan<'o>, EditError> {
        plan_create(
            document,
            ElementKind::TerminologyPackage,
            self.parent.as_ref(),
            self.id.as_ref(),
            Features::Name(&self.name),
        )
    }
}

impl From<CreateTerminologyPackage> for super::Operation {
    fn from(operation: CreateTerminologyPackage) -> Self {
        Self::CreateTerminologyPackage(operation)
    }
}

/// Creates a `Category` (SACM 2.3 §10.8) as the last child of an element
/// whose class can hold one: a `TerminologyPackage` or an `ArtifactPackage`.
/// A Category is never a root.
///
/// "The Category class describes categories of `ExpressionElements` (Terms and
/// Expressions) and can be used to group these elements within
/// `TerminologyPackages`" (§10.8). With
/// [`with_categories`](Self::with_categories) it holds the ids of its own
/// `category` end, held as written: formal/23-05-08 does not say which way a
/// Category's `category` points (spec defect D21). Apply it with
/// [`edit::apply`](super::apply). After [ADR 0019]'s four checks, an id that
/// holds XML white space is [`EditError::InvalidReference`], then one that
/// names no element is [`EditError::ReferenceNotFound`], then one that names
/// no Category is [`EditError::WrongReferenceType`]; each check runs over
/// every id in order before the next. Without [`with_id`](Self::with_id), the
/// id is generated: `category_<n>`.
///
/// [ADR 0019]: https://github.com/lasrod/libSACM/blob/main/docs/adr/0019-edit-operations.md
///
/// # Examples
///
/// ```
/// use sacm::edit::{self, CreateCategory, CreateTerminologyPackage};
/// use sacm::model::{Document, ElementKind, LangString};
///
/// let mut doc = Document::new();
/// let en = |text: &str| LangString::new(text).with_lang("en");
/// edit::apply(&mut doc, CreateTerminologyPackage::root(en("Terms")).with_id("terms"))?;
/// let created = edit::apply(&mut doc, CreateCategory::new("terms", en("Hazards")))?;
///
/// let hazards = doc.get(&created.created_ids()[0]).expect("just created");
/// assert_eq!((hazards.id().as_str(), hazards.kind()), ("category_1", ElementKind::Category));
/// # Ok::<(), sacm::edit::EditError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
pub struct CreateCategory {
    parent: ElementId,
    #[cfg_attr(feature = "serde", serde(default))]
    id: Option<ElementId>,
    name: LangString,
    #[cfg_attr(feature = "serde", serde(default))]
    categories: Vec<ElementId>,
}

impl CreateCategory {
    /// Creates a `Category` named `name` as the last child of `parent`, with
    /// no `category` ids.
    #[must_use]
    pub fn new(parent: impl Into<ElementId>, name: LangString) -> Self {
        Self {
            parent: parent.into(),
            id: None,
            name,
            categories: Vec::new(),
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

    /// Gives the new Category the `category` ids `ids`, in the order given,
    /// repeats included, replacing any given before. Each must name a
    /// Category of the document.
    #[must_use]
    pub fn with_categories(self, ids: impl IntoIterator<Item = impl Into<ElementId>>) -> Self {
        Self {
            categories: ids.into_iter().map(Into::into).collect(),
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

    /// The ids given with [`with_categories`](Self::with_categories), in the
    /// order given: empty unless given.
    #[must_use]
    pub fn categories(&self) -> &[ElementId] {
        &self.categories
    }

    /// Runs every check of the operation, and returns what it creates.
    pub(super) fn plan<'o>(&'o self, document: &Document) -> Result<Plan<'o>, EditError> {
        plan_referencing(
            document,
            ElementKind::Category,
            Some(&self.parent),
            self.id.as_ref(),
            &self.name,
            &[],
            &[(ReferenceEnd::Category, &self.categories)],
        )
    }
}

impl From<CreateCategory> for super::Operation {
    fn from(operation: CreateCategory) -> Self {
        Self::CreateCategory(operation)
    }
}

/// Creates a `Term` (SACM 2.3 §10.11) as the last child of an element whose
/// class can hold one: a `TerminologyPackage` or an `ArtifactPackage`. A Term
/// is never a root.
///
/// "Term is used to model both abstract and concrete terms in SACM"
/// (§10.11). It takes its `value` (§10.9 `value:String[1]`, required) when it
/// is created, held exactly as given. With
/// [`with_categories`](Self::with_categories) it names the Categories it
/// belongs to, with [`with_origin`](Self::with_origin) the `ModelElement` it
/// originates from (`origin:Base::ModelElement[0..1]`), and with
/// [`with_external_reference`](Self::with_external_reference) its
/// `externalReference` (`String[0..1]`, such as a URI). Apply it with
/// [`edit::apply`](super::apply). After [ADR 0019]'s four checks, an id that
/// holds XML white space is [`EditError::InvalidReference`], then one that
/// names no element is [`EditError::ReferenceNotFound`], then one of the
/// wrong kind is [`EditError::WrongReferenceType`]; each check runs over the
/// `category` ids, then the `origin`, before the next. Without
/// [`with_id`](Self::with_id), the id is generated: `term_<n>`.
///
/// [ADR 0019]: https://github.com/lasrod/libSACM/blob/main/docs/adr/0019-edit-operations.md
///
/// # Examples
///
/// ```
/// use sacm::edit::{self, CreateTerm, CreateTerminologyPackage};
/// use sacm::model::{Document, LangString};
/// use sacm::xmi::{self, WriteOptions};
///
/// let mut doc = Document::new();
/// let en = |text: &str| LangString::new(text).with_lang("en");
/// edit::apply(&mut doc, CreateTerminologyPackage::root(en("Terms")).with_id("terms"))?;
/// let blade = CreateTerm::new("terms", en("Blade"), "blade")
///     .with_external_reference("https://example.org/terms/blade");
/// edit::apply(&mut doc, blade.with_id("blade"))?;
///
/// let blade = doc.get("blade").expect("just created");
/// assert_eq!(blade.value(), Some("blade"));
/// let xml = xmi::write(&doc, &WriteOptions::default())?;
/// assert!(xml.contains(r#"value="blade" externalReference="https://example.org/terms/blade""#));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
pub struct CreateTerm {
    parent: ElementId,
    #[cfg_attr(feature = "serde", serde(default))]
    id: Option<ElementId>,
    name: LangString,
    value: String,
    #[cfg_attr(feature = "serde", serde(default))]
    categories: Vec<ElementId>,
    #[cfg_attr(feature = "serde", serde(default))]
    origin: Option<ElementId>,
    #[cfg_attr(feature = "serde", serde(default))]
    external_reference: Option<String>,
}

impl CreateTerm {
    /// Creates a `Term` named `name` with the value `value` as the last child
    /// of `parent`, with no categories, no origin and no external reference.
    #[must_use]
    pub fn new(parent: impl Into<ElementId>, name: LangString, value: impl Into<String>) -> Self {
        Self {
            parent: parent.into(),
            id: None,
            name,
            value: value.into(),
            categories: Vec::new(),
            origin: None,
            external_reference: None,
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

    /// Gives the new Term the Categories `ids` it belongs to, in the order
    /// given, repeats included, replacing any given before. Each must name a
    /// Category of the document.
    #[must_use]
    pub fn with_categories(self, ids: impl IntoIterator<Item = impl Into<ElementId>>) -> Self {
        Self {
            categories: ids.into_iter().map(Into::into).collect(),
            ..self
        }
    }

    /// Gives the new Term the origin `id`, replacing any given before. It
    /// must name a `ModelElement` of the document (any element but a
    /// `UtilityElement`).
    #[must_use]
    pub fn with_origin(self, id: impl Into<ElementId>) -> Self {
        Self {
            origin: Some(id.into()),
            ..self
        }
    }

    /// Gives the new Term the `externalReference` `reference`, exactly as
    /// given, replacing any given before.
    #[must_use]
    pub fn with_external_reference(self, reference: impl Into<String>) -> Self {
        Self {
            external_reference: Some(reference.into()),
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

    /// The new Term's value.
    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }

    /// The ids given with [`with_categories`](Self::with_categories), in the
    /// order given: empty unless given.
    #[must_use]
    pub fn categories(&self) -> &[ElementId] {
        &self.categories
    }

    /// The origin given with [`with_origin`](Self::with_origin), or `None`.
    #[must_use]
    pub fn origin(&self) -> Option<&ElementId> {
        self.origin.as_ref()
    }

    /// The external reference given with
    /// [`with_external_reference`](Self::with_external_reference), or `None`.
    #[must_use]
    pub fn external_reference(&self) -> Option<&str> {
        self.external_reference.as_deref()
    }

    /// Runs every check of the operation, and returns what it creates.
    pub(super) fn plan<'o>(&'o self, document: &Document) -> Result<Plan<'o>, EditError> {
        plan_expression_element(
            document,
            ElementKind::Term,
            &self.parent,
            self.id.as_ref(),
            &self.name,
            vec![
                (StringAttribute::Value, Some(self.value.as_str())),
                (
                    StringAttribute::ExternalReference,
                    self.external_reference.as_deref(),
                ),
            ],
            &[
                (ReferenceEnd::Category, &self.categories),
                (ReferenceEnd::Origin, self.origin.as_slice()),
            ],
        )
    }
}

impl From<CreateTerm> for super::Operation {
    fn from(operation: CreateTerm) -> Self {
        Self::CreateTerm(operation)
    }
}

/// Creates an `Expression` (SACM 2.3 §10.10) as the last child of an
/// element whose class can hold one: a `TerminologyPackage` or an
/// `ArtifactPackage`. An Expression is never a root.
///
/// "The Expression class is used to model both abstract and concrete phrases
/// in SACM" (§10.10). It takes its `value` (§10.9 `value:String[1]`,
/// required) when it is created, held exactly as given. With
/// [`with_expression_elements`](Self::with_expression_elements) it names the
/// `ExpressionElements` it is formed from (`element:ExpressionElement[0..*]`),
/// and with [`with_categories`](Self::with_categories) the Categories it
/// belongs to. Nothing checks that the value names its elements
/// (D-EXPR-NAMES). Apply it with [`edit::apply`](super::apply); the reference
/// checks run as for [`CreateTerm`], over the `category` ids, then the
/// `element` ids. Without [`with_id`](Self::with_id), the id is generated:
/// `expression_<n>`.
///
/// # Examples
///
/// ```
/// use sacm::edit::{self, CreateExpression, CreateTerm, CreateTerminologyPackage};
/// use sacm::model::{Document, LangString};
///
/// let en = |text: &str| LangString::new(text).with_lang("en");
/// let mut doc = Document::new();
/// edit::apply(&mut doc, CreateTerminologyPackage::root(en("Blender terms")).with_id("terms"))?;
/// edit::apply(&mut doc, CreateTerm::new("terms", en("Blade"), "blade").with_id("blade"))?;
/// edit::apply(&mut doc, CreateTerm::new("terms", en("Lid"), "lid").with_id("lid"))?;
/// let guarded = CreateExpression::new("terms", en("Blade guarded"), "$Blade$ is covered by the $Lid$")
///     .with_expression_elements(["blade", "lid"]);
/// edit::apply(&mut doc, guarded.with_id("guarded"))?;
/// let parts: Vec<&str> = doc.get("guarded").expect("created").expression_elements().iter().map(|id| id.as_str()).collect();
/// assert_eq!(parts, ["blade", "lid"]);
/// # Ok::<(), sacm::edit::EditError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
pub struct CreateExpression {
    parent: ElementId,
    #[cfg_attr(feature = "serde", serde(default))]
    id: Option<ElementId>,
    name: LangString,
    value: String,
    #[cfg_attr(feature = "serde", serde(default))]
    categories: Vec<ElementId>,
    #[cfg_attr(feature = "serde", serde(default))]
    expression_elements: Vec<ElementId>,
}

impl CreateExpression {
    /// Creates an `Expression` named `name` with the value `value` as the
    /// last child of `parent`, with no categories and no elements.
    #[must_use]
    pub fn new(parent: impl Into<ElementId>, name: LangString, value: impl Into<String>) -> Self {
        Self {
            parent: parent.into(),
            id: None,
            name,
            value: value.into(),
            categories: Vec::new(),
            expression_elements: Vec::new(),
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

    /// Gives the new Expression the Categories `ids` it belongs to, in the
    /// order given, repeats included, replacing any given before. Each must
    /// name a Category of the document.
    #[must_use]
    pub fn with_categories(self, ids: impl IntoIterator<Item = impl Into<ElementId>>) -> Self {
        Self {
            categories: ids.into_iter().map(Into::into).collect(),
            ..self
        }
    }

    /// Gives the new Expression the `ExpressionElements` `ids` it is formed
    /// from, its `element` ids, in the order given, repeats included,
    /// replacing any given before. Each must name a Term or an Expression of
    /// the document.
    #[doc(alias = "element")]
    #[must_use]
    pub fn with_expression_elements(
        self,
        ids: impl IntoIterator<Item = impl Into<ElementId>>,
    ) -> Self {
        Self {
            expression_elements: ids.into_iter().map(Into::into).collect(),
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

    /// The new Expression's value.
    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }

    /// The ids given with [`with_categories`](Self::with_categories), in the
    /// order given: empty unless given.
    #[must_use]
    pub fn categories(&self) -> &[ElementId] {
        &self.categories
    }

    /// The ids given with
    /// [`with_expression_elements`](Self::with_expression_elements), in the
    /// order given: empty unless given.
    #[doc(alias = "element")]
    #[must_use]
    pub fn expression_elements(&self) -> &[ElementId] {
        &self.expression_elements
    }

    /// Runs every check of the operation, and returns what it creates.
    pub(super) fn plan<'o>(&'o self, document: &Document) -> Result<Plan<'o>, EditError> {
        plan_expression_element(
            document,
            ElementKind::Expression,
            &self.parent,
            self.id.as_ref(),
            &self.name,
            vec![(StringAttribute::Value, Some(self.value.as_str()))],
            &[
                (ReferenceEnd::Category, &self.categories),
                (ReferenceEnd::Element, &self.expression_elements),
            ],
        )
    }
}

impl From<CreateExpression> for super::Operation {
    fn from(operation: CreateExpression) -> Self {
        Self::CreateExpression(operation)
    }
}

/// Creates a `TerminologyGroup` (SACM 2.3 §10.3) as the last child of an
/// element whose class can hold one: a `TerminologyPackage` or an
/// `ArtifactPackage`. A `TerminologyGroup` is never a root.
///
/// A `TerminologyGroup` organises "a collection of `TerminologyElements`"
/// (§10.3), its [`group_members`](crate::model::SacmElement::group_members)
/// (`terminologyElement:TerminologyElement[0..*]`). The members are
/// references, not children: "`TerminologyGroups` serve no structural purpose"
/// (§10.3). In XMI the end is named `terminologyElement`, like the composite
/// role of a `TerminologyPackage`; it is
/// [`ReferenceEnd::TerminologyGroupMember`](crate::model::ReferenceEnd::TerminologyGroupMember).
/// With [`with_group_members`](Self::with_group_members) it names its
/// members; each must name a `TerminologyElement`, checked as for
/// [`CreateCategory`]. Apply it with [`edit::apply`](super::apply). Without
/// [`with_id`](Self::with_id), the id is generated: `terminology_group_<n>`.
///
/// # Examples
///
/// ```
/// use sacm::edit::{self, CreateTerm, CreateTerminologyGroup, CreateTerminologyPackage};
/// use sacm::model::{Document, LangString};
///
/// let mut doc = Document::new();
/// let en = |text: &str| LangString::new(text).with_lang("en");
/// edit::apply(&mut doc, CreateTerminologyPackage::root(en("Terms")).with_id("terms"))?;
/// edit::apply(&mut doc, CreateTerm::new("terms", en("Blade"), "blade").with_id("blade"))?;
/// let reviewed = CreateTerminologyGroup::new("terms", en("Reviewed")).with_group_members(["blade"]);
/// let created = edit::apply(&mut doc, reviewed)?;
///
/// let group = doc.get(&created.created_ids()[0]).expect("just created");
/// assert_eq!(group.group_members().iter().map(|id| id.as_str()).collect::<Vec<_>>(), ["blade"]);
/// # Ok::<(), sacm::edit::EditError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
pub struct CreateTerminologyGroup {
    parent: ElementId,
    #[cfg_attr(feature = "serde", serde(default))]
    id: Option<ElementId>,
    name: LangString,
    #[cfg_attr(feature = "serde", serde(default))]
    group_members: Vec<ElementId>,
}

impl CreateTerminologyGroup {
    /// Creates a `TerminologyGroup` named `name` as the last child of
    /// `parent`, with no members.
    #[must_use]
    pub fn new(parent: impl Into<ElementId>, name: LangString) -> Self {
        Self {
            parent: parent.into(),
            id: None,
            name,
            group_members: Vec::new(),
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

    /// Gives the new group the members `ids`, its `terminologyElement` ids,
    /// in the order given, repeats included, replacing any given before.
    /// Each must name a `TerminologyElement` of the document.
    #[doc(alias = "terminologyElement")]
    #[must_use]
    pub fn with_group_members(self, ids: impl IntoIterator<Item = impl Into<ElementId>>) -> Self {
        Self {
            group_members: ids.into_iter().map(Into::into).collect(),
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

    /// The members given with [`with_group_members`](Self::with_group_members),
    /// in the order given: empty unless given.
    #[doc(alias = "terminologyElement")]
    #[must_use]
    pub fn group_members(&self) -> &[ElementId] {
        &self.group_members
    }

    /// Runs every check of the operation, and returns what it creates.
    pub(super) fn plan<'o>(&'o self, document: &Document) -> Result<Plan<'o>, EditError> {
        plan_referencing(
            document,
            ElementKind::TerminologyGroup,
            Some(&self.parent),
            self.id.as_ref(),
            &self.name,
            &[],
            &[(ReferenceEnd::TerminologyGroupMember, &self.group_members)],
        )
    }
}

impl From<CreateTerminologyGroup> for super::Operation {
    fn from(operation: CreateTerminologyGroup) -> Self {
        Self::CreateTerminologyGroup(operation)
    }
}

/// The plan of creating an `ExpressionElement` of `kind`: ADR 0019's four
/// checks, then ADR 0028's reference checks, each
/// phase over `references` in order; the element then holds `strings` and
/// `references` as given. Changes nothing.
fn plan_expression_element<'o>(
    document: &Document,
    kind: ElementKind,
    parent: &'o ElementId,
    id: Option<&ElementId>,
    name: &'o LangString,
    strings: Vec<(StringAttribute, Option<&'o str>)>,
    references: &[(ReferenceEnd, &'o [ElementId])],
) -> Result<Plan<'o>, EditError> {
    let (id, generated) = checked_place(document, kind, Some(parent), id)?;
    // No bound can be broken: `category` and `element` are `[0..*]`, and a
    // Term's one `origin` is an `Option`, so the bounds need no check.
    debug_assert!(
        references
            .iter()
            .all(|&(end, targets)| end.upper_bound().is_none_or(|upper| targets.len() <= upper)),
        "the ends of an ExpressionElement create are within their bounds"
    );
    // The name's expression comes after the element's own ends, in each
    // check (SACM23-BASE-009).
    let expressions = expression_ids([name])?;
    let mut checked: Vec<(ReferenceEnd, &[ElementId], &[ElementId])> = references
        .iter()
        .map(|&(end, targets)| (end, targets, [].as_slice()))
        .collect();
    checked.push((ReferenceEnd::Expression, &expressions, &[]));
    check_targets(document, kind, &checked)?;
    Ok(Plan::Create(NewElement {
        kind,
        parent: Some(parent),
        id,
        generated,
        features: Features::Terminology {
            name,
            strings,
            references: references.to_vec(),
        },
    }))
}
