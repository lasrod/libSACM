//! The create operations (LIB-EDIT-001, ADR 0019).

use super::EditError;
use super::argumentation::{binding_operation, interface_operation};
use super::plan::{Features, NewElement, Plan};
use crate::model::{
    Document, ElementId, ElementKind, LangString, ReferenceEnd, SacmElement, admits, is_ncname,
};

/// Creates an `AssuranceCasePackage` (SACM 2.3 §9.2): a root, or a child of
/// an element whose class can hold one, such as another
/// `AssuranceCasePackage`.
///
/// Apply it with [`edit::apply`](super::apply). Without
/// [`with_id`](Self::with_id), the id is generated:
/// `assurance_case_package_<n>`.
///
/// # Examples
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
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
pub struct CreateAssuranceCasePackage {
    #[cfg_attr(
        feature = "serde",
        serde(deserialize_with = "serde::Deserialize::deserialize")
    )]
    parent: Option<ElementId>,
    #[cfg_attr(feature = "serde", serde(default))]
    id: Option<ElementId>,
    name: LangString,
}

impl CreateAssuranceCasePackage {
    /// Creates a root `AssuranceCasePackage` named `name`, after the
    /// document's last root.
    #[must_use]
    pub fn root(name: impl Into<LangString>) -> Self {
        Self {
            parent: None,
            id: None,
            name: name.into(),
        }
    }

    /// Creates an `AssuranceCasePackage` named `name` as the last child of
    /// `parent`.
    #[must_use]
    pub fn new(parent: impl Into<ElementId>, name: impl Into<LangString>) -> Self {
        Self {
            parent: Some(parent.into()),
            id: None,
            name: name.into(),
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
            ElementKind::AssuranceCasePackage,
            self.parent.as_ref(),
            self.id.as_ref(),
            Features::Name(&self.name),
        )
    }
}

interface_operation! {
    /// Creates an `AssuranceCasePackageInterface` (SACM 2.3 §9.3) declaring
    /// the AssuranceCasePackage `implements` names: a root, or a child of an
    /// AssuranceCasePackage or an ArtifactPackage (or one of their interfaces
    /// and bindings). The package's `interface` end is not changed (ADR 0039).
    ///
    /// `implements` is required (`implements:AssuranceCasePackage[1]`). After
    /// [ADR 0019]'s four checks, and the name's expression, an id that holds
    /// XML white space is [`EditError::InvalidReference`], one that names no
    /// element is [`EditError::ReferenceNotFound`], and one that names an
    /// element of no AssuranceCasePackage kind is
    /// [`EditError::WrongReferenceType`] (SACM-REF-002). Without
    /// [`with_id`](Self::with_id), the id is generated:
    /// `assurance_case_package_interface_<n>`.
    ///
    /// [ADR 0019]: https://github.com/lasrod/libSACM/blob/main/docs/adr/0019-edit-operations.md
    ///
    /// # Examples
    ///
    /// ```
    /// use sacm::edit::{self, CreateAssuranceCasePackage, CreateAssuranceCasePackageBinding, CreateAssuranceCasePackageInterface};
    /// use sacm::model::{Document, LangString};
    ///
    /// let en = |t: &str| LangString::new(t).with_lang("en");
    /// let mut doc = Document::new();
    /// edit::apply(&mut doc, CreateAssuranceCasePackage::root(en("Vehicle system case")).with_id("system"))?;
    /// for (id, api) in [("braking_case", "braking_case_api"), ("vehicle_case", "vehicle_case_api")] {
    ///     edit::apply(&mut doc, CreateAssuranceCasePackage::new("system", en(id)).with_id(id))?;
    ///     edit::apply(&mut doc, CreateAssuranceCasePackageInterface::new(id, en(api), id).with_id(api))?;
    /// }
    /// let bind = CreateAssuranceCasePackageBinding::new("system", en("Braking supports the vehicle"), ["braking_case_api", "vehicle_case_api"]);
    /// edit::apply(&mut doc, bind.with_id("binding"))?;
    /// assert_eq!(doc.get("braking_case_api").and_then(|i| i.implements()).map(|id| id.as_str()), Some("braking_case"));
    /// # Ok::<(), sacm::edit::EditError>(())
    /// ```
    CreateAssuranceCasePackageInterface, AssuranceCasePackageInterface, "AssuranceCasePackage"
}

binding_operation! {
    /// Creates an `AssuranceCasePackageBinding` (SACM 2.3 §9.4) binding the
    /// packages `participant_packages` names, each of an
    /// `AssuranceCasePackage` kind: a root, or a child of an
    /// AssuranceCasePackage or an ArtifactPackage (or one of their interfaces
    /// and bindings).
    ///
    /// `participantPackage:AssuranceCasePackage[2..*]`: after [ADR 0019]'s
    /// four checks, fewer than two distinct ids is
    /// [`EditError::TooFewReferences`] (ADR 0039 D3); then the name's
    /// expression; then an id that holds XML white space is
    /// [`EditError::InvalidReference`], one that names no element
    /// [`EditError::ReferenceNotFound`], and one that names an element of no
    /// AssuranceCasePackage kind [`EditError::WrongReferenceType`], each over
    /// the ids in order. Without [`with_id`](Self::with_id), the id is
    /// generated: `assurance_case_package_binding_<n>`. See
    /// [`CreateAssuranceCasePackageInterface`] for an example.
    ///
    /// [ADR 0019]: https://github.com/lasrod/libSACM/blob/main/docs/adr/0019-edit-operations.md
    CreateAssuranceCasePackageBinding, AssuranceCasePackageBinding, "AssuranceCasePackage"
}

/// Creates an `ArgumentPackage` (SACM 2.3 §11.4): a root, or a child of an
/// element whose class can hold one, such as an `AssuranceCasePackage` or
/// another `ArgumentPackage`.
///
/// Apply it with [`edit::apply`](super::apply). Without
/// [`with_id`](Self::with_id), the id is generated: `argument_package_<n>`.
///
/// # Examples
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
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
pub struct CreateArgumentPackage {
    #[cfg_attr(
        feature = "serde",
        serde(deserialize_with = "serde::Deserialize::deserialize")
    )]
    parent: Option<ElementId>,
    #[cfg_attr(feature = "serde", serde(default))]
    id: Option<ElementId>,
    name: LangString,
}

impl CreateArgumentPackage {
    /// Creates a root `ArgumentPackage` named `name`, after the document's
    /// last root.
    #[must_use]
    pub fn root(name: impl Into<LangString>) -> Self {
        Self {
            parent: None,
            id: None,
            name: name.into(),
        }
    }

    /// Creates an `ArgumentPackage` named `name` as the last child of
    /// `parent`.
    #[must_use]
    pub fn new(parent: impl Into<ElementId>, name: impl Into<LangString>) -> Self {
        Self {
            parent: Some(parent.into()),
            id: None,
            name: name.into(),
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
            ElementKind::ArgumentPackage,
            self.parent.as_ref(),
            self.id.as_ref(),
            Features::Name(&self.name),
        )
    }
}

/// Creates a `Claim` (SACM 2.3 §11.11) as a child of an element whose class
/// can hold one, such as an `ArgumentPackage`. A Claim is never a root.
///
/// Its `assertionDeclaration` is
/// [`Asserted`](crate::model::AssertionDeclaration::Asserted), the SACM
/// default (§11.10); [`SetAssertionDeclaration`](super::SetAssertionDeclaration)
/// changes it. Apply it with
/// [`edit::apply`](super::apply). Without [`with_id`](Self::with_id), the id
/// is generated: `claim_<n>`.
///
/// # Examples
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
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
pub struct CreateClaim {
    parent: ElementId,
    #[cfg_attr(feature = "serde", serde(default))]
    id: Option<ElementId>,
    name: LangString,
}

impl CreateClaim {
    /// Creates a `Claim` named `name` as the last child of `parent`.
    #[must_use]
    pub fn new(parent: impl Into<ElementId>, name: impl Into<LangString>) -> Self {
        Self {
            parent: parent.into(),
            id: None,
            name: name.into(),
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

    /// The new element's name.
    #[must_use]
    pub fn name(&self) -> &LangString {
        &self.name
    }

    /// Runs every check of the operation, and returns what it creates.
    pub(super) fn plan<'o>(&'o self, document: &Document) -> Result<Plan<'o>, EditError> {
        plan_create(
            document,
            ElementKind::Claim,
            Some(&self.parent),
            self.id.as_ref(),
            Features::Name(&self.name),
        )
    }
}

/// The plan of a new element of `kind` under `parent` (a root when
/// `None`), with the id `id` or a generated one and with `features`, after
/// [`checked_place`]'s checks. Changes nothing: the commit adds the element
/// and then advances the counter for a generated id.
pub(super) fn plan_create<'o>(
    document: &Document,
    kind: ElementKind,
    parent: Option<&'o ElementId>,
    id: Option<&ElementId>,
    features: Features<'o>,
) -> Result<Plan<'o>, EditError> {
    let (id, generated) = checked_place(document, kind, parent, id)?;
    Ok(Plan::Create(NewElement {
        kind,
        parent,
        id,
        generated,
        features,
    }))
}

/// The id of a new element of `kind` under `parent` (a root when `None`):
/// `id`, or the next generated one with its `n`, after ADR 0019's four
/// checks, in order. Changes nothing: the commit advances the counter for a
/// generated id once the element is added.
pub(super) fn checked_place(
    document: &Document,
    kind: ElementKind,
    parent: Option<&ElementId>,
    id: Option<&ElementId>,
) -> Result<(ElementId, Option<u64>), EditError> {
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
    if let Some(id) = id {
        if !is_ncname(id.as_str()) {
            return Err(EditError::InvalidId { id: id.clone() });
        }
        if document.is_id_in_use(id.as_str()) {
            return Err(EditError::DuplicateId { id: id.clone() });
        }
        Ok((id.clone(), None))
    } else {
        let (id, n) = document.next_generated_id(kind);
        Ok((id, Some(n)))
    }
}
