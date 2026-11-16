//! The create operations of the Artifact package: `ArtifactPackage`,
//! `Property`, `Technique`, `Participant` and `ArtifactGroup`
//! (SACM23-ART-001, SACM23-ART-003, SACM23-ART-008, SACM23-ART-009,
//! SACM23-ART-011; LIB-EDIT-001, ADR 0019, ADR 0028, ADR 0034).
//!
//! There is one create per class (ADR 0034). Each runs ADR 0019's four
//! checks through the containment table, so a parent is legal exactly when a
//! composite role of its class admits the new kind; the group's members then
//! get ADR 0028's reference checks.

use super::EditError;
use super::argumentation::plan_referencing;
use super::create::plan_create;
use super::plan::{Features, Plan};
use crate::model::{Document, ElementId, ElementKind, LangString, ReferenceEnd};

/// Creates an `ArtifactPackage` (SACM 2.3 §12.2): a root, or a child of an
/// element whose class can hold one, an `AssuranceCasePackage` (its
/// `artifactPackage` role) or another `ArtifactPackage` (its
/// `artifactElement` role).
///
/// An `ArtifactPackage` holds the artifacts of an assurance case and what
/// describes them: `Technique`s, `Participant`s, `Property`s, `ArtifactGroup`s
/// and nested `ArtifactPackage`s. Apply it with [`edit::apply`](super::apply).
/// Without [`with_id`](Self::with_id), the id is generated:
/// `artifact_package_<n>`.
///
/// # Examples
///
/// ```
/// use sacm::edit::{self, CreateArtifactPackage, CreateAssuranceCasePackage};
/// use sacm::model::{Document, ElementKind, LangString};
///
/// let mut doc = Document::new();
/// let name = |text: &str| LangString::new(text).with_lang("en");
/// edit::apply(&mut doc, CreateArtifactPackage::root(name("Shared evidence")).with_id("shared"))?;
/// edit::apply(&mut doc, CreateAssuranceCasePackage::root(name("Brake case")).with_id("case"))?;
/// edit::apply(&mut doc, CreateArtifactPackage::new("case", name("Brake evidence")))?;
///
/// let evidence = doc.get("artifact_package_1").expect("just created");
/// assert_eq!(evidence.kind(), ElementKind::ArtifactPackage);
/// assert_eq!(evidence.parent().map(|parent| parent.id().as_str()), Some("case"));
/// # Ok::<(), sacm::edit::EditError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
pub struct CreateArtifactPackage {
    #[cfg_attr(
        feature = "serde",
        serde(deserialize_with = "serde::Deserialize::deserialize")
    )]
    parent: Option<ElementId>,
    #[cfg_attr(feature = "serde", serde(default))]
    id: Option<ElementId>,
    name: LangString,
}

impl CreateArtifactPackage {
    /// Creates a root `ArtifactPackage` named `name`, after the document's
    /// last root.
    #[must_use]
    pub fn root(name: LangString) -> Self {
        Self {
            parent: None,
            id: None,
            name,
        }
    }

    /// Creates an `ArtifactPackage` named `name` as the last child of
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
            ElementKind::ArtifactPackage,
            self.parent.as_ref(),
            self.id.as_ref(),
            Features::Name(&self.name),
        )
    }
}

/// Defines the create operation of one artifact kind that is given only a
/// parent and a name: the struct `$name`, its methods, and its conversion
/// into an [`Operation`](super::Operation).
macro_rules! named_create {
    (
        $(#[$meta:meta])*
        $name:ident, $kind:ident
    ) => {
        $(#[$meta])*
        #[derive(Clone, Debug, PartialEq, Eq)]
        #[cfg_attr(
            feature = "serde",
            derive(serde::Serialize, serde::Deserialize),
            serde(deny_unknown_fields)
        )]
        pub struct $name {
            parent: ElementId,
            #[cfg_attr(feature = "serde", serde(default))]
            id: Option<ElementId>,
            name: LangString,
        }

        impl $name {
            #[doc = concat!(
                "Creates a `", stringify!($kind), "` named `name` as the last child of `parent`."
            )]
            #[must_use]
            pub fn new(parent: impl Into<ElementId>, name: LangString) -> Self {
                Self {
                    parent: parent.into(),
                    id: None,
                    name,
                }
            }

            /// Gives the new element the id `id` instead of a generated one.
            /// It must be an XML `NCName` and not already in the document.
            /// Otherwise [`apply`](super::apply) refuses with
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

            /// The id given with [`with_id`](Self::with_id), or `None` when
            /// the id is to be generated.
            #[must_use]
            pub fn id(&self) -> Option<&ElementId> {
                self.id.as_ref()
            }

            /// The new element's name.
            #[must_use]
            pub fn name(&self) -> &LangString {
                &self.name
            }

            /// Runs every check of the operation, and returns what it
            /// creates.
            pub(super) fn plan<'o>(
                &'o self,
                document: &Document,
            ) -> Result<Plan<'o>, EditError> {
                plan_create(
                    document,
                    ElementKind::$kind,
                    Some(&self.parent),
                    self.id.as_ref(),
                    Features::Name(&self.name),
                )
            }
        }

        impl From<$name> for super::Operation {
            fn from(operation: $name) -> Self {
                Self::$name(operation)
            }
        }
    };
}

named_create! {
    /// Creates a `Property` (SACM 2.3 §12.8) as the last child of an element
    /// whose class can hold one: an `ArtifactAsset`, such as a `Technique`, a
    /// `Participant` or another `Property` (its `property` role, §12.6), or
    /// an `ArtifactPackage` (its `artifactElement` role). A Property is never
    /// a root.
    ///
    /// A Property "enables the specification of the characteristics of an
    /// Artifact" (§12.8), such as a test result or a judgement of a report's
    /// quality; every `ArtifactAsset` can hold some (§12.6). One held by an
    /// asset is among the asset's
    /// [`properties`](crate::model::SacmElement::properties); one held
    /// directly by an `ArtifactPackage` is a child of the package, not a
    /// property of it. Apply it with [`edit::apply`](super::apply). Without
    /// [`with_id`](Self::with_id), the id is generated: `property_<n>`.
    ///
    /// # Examples
    ///
    /// ```
    /// use sacm::edit::{self, CreateArtifactPackage, CreateProperty, CreateTechnique};
    /// use sacm::model::{Document, LangString};
    ///
    /// let mut doc = Document::new();
    /// let name = |text: &str| LangString::new(text).with_lang("en");
    /// edit::apply(&mut doc, CreateArtifactPackage::root(name("Evidence")).with_id("evidence"))?;
    /// edit::apply(&mut doc, CreateTechnique::new("evidence", name("Fault tree analysis")).with_id("fta"))?;
    /// let created = edit::apply(&mut doc, CreateProperty::new("fta", name("Qualified tool")))?;
    /// assert_eq!(created.created_ids()[0].as_str(), "property_1");
    /// assert_eq!(doc.get("fta").expect("just created").properties().count(), 1);
    /// # Ok::<(), sacm::edit::EditError>(())
    /// ```
    CreateProperty, Property
}

named_create! {
    /// Creates a `Technique` (SACM 2.3 §12.12) as the last child of an
    /// element whose class can hold one: an `ArtifactPackage`. A Technique
    /// is never a root.
    ///
    /// A Technique is an `ArtifactAsset` that "represents techniques
    /// associated with Artifacts (e.g., associated with the creation,
    /// inspection, review or analysis of an Artifact)" (§12.12), such as a
    /// fault tree analysis. It has no attributes of its own; its characteristics are its
    /// [`properties`](crate::model::SacmElement::properties). Apply it with
    /// [`edit::apply`](super::apply). Without [`with_id`](Self::with_id), the
    /// id is generated: `technique_<n>`.
    ///
    /// # Examples
    ///
    /// ```
    /// use sacm::edit::{self, CreateArtifactPackage, CreateTechnique};
    /// use sacm::model::{Document, ElementKind, LangString};
    ///
    /// let mut doc = Document::new();
    /// let name = |text: &str| LangString::new(text).with_lang("en");
    /// edit::apply(&mut doc, CreateArtifactPackage::root(name("Evidence")).with_id("evidence"))?;
    /// let created = edit::apply(&mut doc, CreateTechnique::new("evidence", name("Fault tree analysis")))?;
    /// let technique = doc.get(&created.created_ids()[0]).expect("just created");
    /// assert_eq!(technique.id().as_str(), "technique_1");
    /// assert_eq!(technique.kind(), ElementKind::Technique);
    /// # Ok::<(), sacm::edit::EditError>(())
    /// ```
    CreateTechnique, Technique
}

named_create! {
    /// Creates a `Participant` (SACM 2.3 §12.13) as the last child of an
    /// element whose class can hold one: an `ArtifactPackage`. A Participant
    /// is never a root.
    ///
    /// A Participant is an `ArtifactAsset` that "enables the specification of
    /// the parties involved in the management of `ArtifactAssets`" (§12.13),
    /// such as a person, an organization or a tool. It has no attributes of its own; its characteristics are
    /// its [`properties`](crate::model::SacmElement::properties). Apply it
    /// with [`edit::apply`](super::apply). Without [`with_id`](Self::with_id),
    /// the id is generated: `participant_<n>`.
    ///
    /// # Examples
    ///
    /// ```
    /// use sacm::edit::{self, CreateArtifactPackage, CreateParticipant};
    /// use sacm::model::{Document, ElementKind, LangString};
    ///
    /// let mut doc = Document::new();
    /// let name = |text: &str| LangString::new(text).with_lang("en");
    /// edit::apply(&mut doc, CreateArtifactPackage::root(name("Evidence")).with_id("evidence"))?;
    /// edit::apply(&mut doc, CreateParticipant::new("evidence", name("Independent assessor")).with_id("assessor"))?;
    /// let assessor = doc.get("assessor").expect("just created");
    /// assert_eq!(assessor.kind(), ElementKind::Participant);
    /// # Ok::<(), sacm::edit::EditError>(())
    /// ```
    CreateParticipant, Participant
}

/// Creates an `ArtifactGroup` (SACM 2.3 §12.3) as the last child of an
/// element whose class can hold one: an `ArtifactPackage`. An `ArtifactGroup`
/// is never a root.
///
/// An `ArtifactGroup` associates "a number of `ArtifactElements` to a common
/// group" (§12.3), its
/// [`group_members`](crate::model::SacmElement::group_members)
/// (`artifactElement:ArtifactElement[0..*]`). The members are references,
/// not children: a member stays where it is contained, and `ArtifactGroups`
/// "serve no structural purpose" (§12.3). In XMI the end is named
/// `artifactElement`, like the composite role of an `ArtifactPackage`; it is
/// [`ReferenceEnd::ArtifactGroupMember`](crate::model::ReferenceEnd::ArtifactGroupMember).
///
/// With [`with_group_members`](Self::with_group_members) it names its
/// members: none is required, and the ids are kept in the order given,
/// repeats included. Apply it with [`edit::apply`](super::apply). After
/// [ADR 0019]'s four checks, an id that holds XML white space is
/// [`EditError::InvalidReference`], then one that names no element is
/// [`EditError::ReferenceNotFound`], then one that names a `UtilityElement`,
/// which is no `ArtifactElement`, is [`EditError::WrongReferenceType`]
/// (SACM-REF-002, ADR 0028); each check runs over every member in order
/// before the next. Without [`with_id`](Self::with_id), the id is generated:
/// `artifact_group_<n>`.
///
/// [ADR 0019]: https://github.com/lasrod/libSACM/blob/main/docs/adr/0019-edit-operations.md
///
/// # Examples
///
/// ```
/// use sacm::edit::{self, CreateArtifactGroup, CreateArtifactPackage, CreateParticipant, CreateProperty, CreateTechnique};
/// use sacm::model::{Document, LangString};
/// use sacm::xmi::{self, WriteOptions};
///
/// let mut doc = Document::new();
/// let en = |text: &str| LangString::new(text).with_lang("en");
/// edit::apply(&mut doc, CreateArtifactPackage::root(en("Evidence")).with_id("evidence"))?;
/// edit::apply(&mut doc, CreateTechnique::new("evidence", en("Fault tree analysis")).with_id("fta"))?;
/// edit::apply(&mut doc, CreateProperty::new("fta", en("Qualified tool")))?;
/// edit::apply(&mut doc, CreateParticipant::new("evidence", en("Independent assessor")).with_id("assessor"))?;
/// let group = CreateArtifactGroup::new("evidence", en("Analysis")).with_group_members(["fta", "assessor"]);
/// let created = edit::apply(&mut doc, group)?;
///
/// let analysis = doc.get(&created.created_ids()[0]).expect("just created");
/// assert_eq!(analysis.group_members().len(), 2);
/// assert_eq!(doc.get("fta").expect("created").properties().count(), 1);
/// let xml = xmi::write(&doc, &WriteOptions::default())?;
/// assert!(xml.contains(r#"artifactElement="fta assessor""#));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
pub struct CreateArtifactGroup {
    parent: ElementId,
    #[cfg_attr(feature = "serde", serde(default))]
    id: Option<ElementId>,
    name: LangString,
    #[cfg_attr(feature = "serde", serde(default))]
    group_members: Vec<ElementId>,
}

impl CreateArtifactGroup {
    /// Creates an `ArtifactGroup` named `name` as the last child of
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

    /// Gives the new group the members `ids`, its `artifactElement` ids, in
    /// the order given, repeats included, replacing any given before. Each
    /// must name an `ArtifactElement` of the document (any element but a
    /// `UtilityElement`); otherwise [`apply`](super::apply) refuses with
    /// [`EditError::ReferenceNotFound`](super::EditError::ReferenceNotFound)
    /// or
    /// [`EditError::WrongReferenceType`](super::EditError::WrongReferenceType).
    #[doc(alias = "artifactElement")]
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
    #[doc(alias = "artifactElement")]
    #[must_use]
    pub fn group_members(&self) -> &[ElementId] {
        &self.group_members
    }

    /// Runs every check of the operation, and returns what it creates.
    pub(super) fn plan<'o>(&'o self, document: &Document) -> Result<Plan<'o>, EditError> {
        plan_referencing(
            document,
            ElementKind::ArtifactGroup,
            &self.parent,
            self.id.as_ref(),
            &self.name,
            &[],
            &[(ReferenceEnd::ArtifactGroupMember, &self.group_members)],
        )
    }
}

impl From<CreateArtifactPackage> for super::Operation {
    fn from(operation: CreateArtifactPackage) -> Self {
        Self::CreateArtifactPackage(operation)
    }
}

impl From<CreateArtifactGroup> for super::Operation {
    fn from(operation: CreateArtifactGroup) -> Self {
        Self::CreateArtifactGroup(operation)
    }
}
