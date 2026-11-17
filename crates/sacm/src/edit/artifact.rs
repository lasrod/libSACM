//! The create operations of the Artifact package: `ArtifactPackage`,
//! `Property`, `Technique`, `Participant`, `ArtifactGroup`, `Artifact`,
//! `Event`, `Resource`, `Activity` and `ArtifactAssetRelationship`
//! (SACM23-ART-001, SACM23-ART-003, SACM23-ART-004 to SACM23-ART-011;
//! LIB-EDIT-001, ADR 0019, ADR 0028, ADR 0034 and its amendments 1 and 2).
//!
//! There is one create per class (ADR 0034). Each runs ADR 0019's four
//! checks through the containment table, so a parent is legal exactly when a
//! composite role of its class admits the new kind; the group's members and
//! the relationship's sources and targets then get ADR 0028's reference
//! checks. The values of an Artifact, an Event, a
//! Resource and an Activity are held as given and never refused (D-DATE).

use super::EditError;
use super::argumentation::plan_referencing;
use super::create::plan_create;
use super::plan::{Features, Plan};
use crate::model::{
    Document, ElementId, ElementKind, LangString, MultiLangString, ReferenceEnd, StringAttribute,
};

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

/// Creates an `Artifact` (SACM 2.3 §12.7) as the last child of an element
/// whose class can hold one: an `ArtifactPackage`. An Artifact is never a
/// root.
///
/// An Artifact "represents the distinguishable units of data used in a
/// structured assurance case" (§12.7), such as a test report, a model or a
/// review record. It is an `ArtifactAsset`, so it can hold
/// [`properties`](crate::model::SacmElement::properties), and it has a
/// `version` (`version: String[0..1]`) and a `date` (`date: date[0..1]`,
/// "the date on which the artifact was created"), given with
/// [`with_version`](Self::with_version) and [`with_date`](Self::with_date)
/// and held exactly as given (D-DATE). Apply it with
/// [`edit::apply`](super::apply); it runs [ADR 0019]'s four checks and
/// nothing else. Without [`with_id`](Self::with_id), the id is generated:
/// `artifact_<n>`.
///
/// [ADR 0019]: https://github.com/lasrod/libSACM/blob/main/docs/adr/0019-edit-operations.md
///
/// # Examples
///
/// ```
/// use sacm::edit::{self, CreateArtifact, CreateArtifactPackage, CreateEvent, CreateResource, SetDate};
/// use sacm::model::{Document, LangString};
/// use sacm::xmi::{self, WriteOptions};
///
/// let en = |text: &str| LangString::new(text).with_lang("en");
/// let mut doc = Document::new();
/// edit::apply(&mut doc, CreateArtifactPackage::root(en("Brake evidence")).with_id("evidence"))?;
/// edit::apply(&mut doc, CreateArtifact::new("evidence", en("Brake test report"))
///     .with_id("report").with_version("1.2").with_date("2024-03-01"))?;
/// edit::apply(&mut doc, CreateResource::new("evidence", en("Report PDF"))
///     .with_location(en("https://example.org/brake-1.2.pdf")))?;
/// edit::apply(&mut doc, CreateEvent::new("evidence", en("Report released")).with_id("released"))?;
/// edit::apply(&mut doc, SetDate::new("released", "2024-03-05"))?;
///
/// let report = doc.get("report").expect("just created");
/// assert_eq!((report.version(), report.date()), (Some("1.2"), Some("2024-03-01")));
/// let xml = xmi::write(&doc, &WriteOptions::default())?;
/// assert!(xml.contains(r#"xmi:id="report" version="1.2" date="2024-03-01""#));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
pub struct CreateArtifact {
    parent: ElementId,
    #[cfg_attr(feature = "serde", serde(default))]
    id: Option<ElementId>,
    name: LangString,
    #[cfg_attr(feature = "serde", serde(default))]
    version: Option<String>,
    #[cfg_attr(feature = "serde", serde(default))]
    date: Option<String>,
}

impl CreateArtifact {
    /// Creates an `Artifact` named `name` as the last child of `parent`,
    /// with no version and no date.
    #[must_use]
    pub fn new(parent: impl Into<ElementId>, name: LangString) -> Self {
        Self {
            parent: parent.into(),
            id: None,
            name,
            version: None,
            date: None,
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

    /// Gives the new Artifact the `version` `version`, exactly as given,
    /// replacing any given before.
    #[doc(alias = "version")]
    #[must_use]
    pub fn with_version(self, version: impl Into<String>) -> Self {
        Self {
            version: Some(version.into()),
            ..self
        }
    }

    /// Gives the new Artifact the `date` `date`, exactly as given, replacing
    /// any given before: SACM 2.3 gives `date` no lexical form (D-DATE).
    #[must_use]
    pub fn with_date(self, date: impl Into<String>) -> Self {
        Self {
            date: Some(date.into()),
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

    /// The version given with [`with_version`](Self::with_version), or
    /// `None`.
    #[must_use]
    pub fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }

    /// The date given with [`with_date`](Self::with_date), or `None`.
    #[must_use]
    pub fn date(&self) -> Option<&str> {
        self.date.as_deref()
    }

    /// Runs every check of the operation, and returns what it creates.
    pub(super) fn plan<'o>(&'o self, document: &Document) -> Result<Plan<'o>, EditError> {
        plan_create(
            document,
            ElementKind::Artifact,
            Some(&self.parent),
            self.id.as_ref(),
            Features::Asset {
                name: &self.name,
                strings: vec![
                    (StringAttribute::Version, self.version.as_deref()),
                    (StringAttribute::Date, self.date.as_deref()),
                ],
                location: None,
            },
        )
    }
}

/// Creates an `Event` (SACM 2.3 §12.9) as the last child of an element whose
/// class can hold one: an `ArtifactPackage`. An Event is never a root.
///
/// An Event "enables the specification of the events in the lifecycle of an
/// Artifact" (§12.9), such as a release or a review. It is an
/// `ArtifactAsset`, so it can hold
/// [`properties`](crate::model::SacmElement::properties), and it has a
/// `date` (`date: date[0..1]`, "the date on which the Event occurred"),
/// given with [`with_date`](Self::with_date) and held exactly as given
/// (D-DATE). Apply it with [`edit::apply`](super::apply). Without
/// [`with_id`](Self::with_id), the id is generated: `event_<n>`.
///
/// # Examples
///
/// ```
/// use sacm::edit::{self, CreateArtifactPackage, CreateEvent};
/// use sacm::model::{Document, ElementKind, LangString};
///
/// let mut doc = Document::new();
/// let en = |text: &str| LangString::new(text).with_lang("en");
/// edit::apply(&mut doc, CreateArtifactPackage::root(en("Evidence")).with_id("evidence"))?;
/// let released = CreateEvent::new("evidence", en("Report released")).with_date("2024-03-05");
/// let created = edit::apply(&mut doc, released)?;
///
/// let event = doc.get(&created.created_ids()[0]).expect("just created");
/// assert_eq!((event.id().as_str(), event.kind()), ("event_1", ElementKind::Event));
/// assert_eq!(event.date(), Some("2024-03-05"));
/// # Ok::<(), sacm::edit::EditError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
pub struct CreateEvent {
    parent: ElementId,
    #[cfg_attr(feature = "serde", serde(default))]
    id: Option<ElementId>,
    name: LangString,
    #[cfg_attr(feature = "serde", serde(default))]
    date: Option<String>,
}

impl CreateEvent {
    /// Creates an `Event` named `name` as the last child of `parent`, with
    /// no date.
    #[must_use]
    pub fn new(parent: impl Into<ElementId>, name: LangString) -> Self {
        Self {
            parent: parent.into(),
            id: None,
            name,
            date: None,
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

    /// Gives the new Event the `date` `date`, exactly as given, replacing
    /// any given before: SACM 2.3 gives `date` no lexical form (D-DATE).
    #[must_use]
    pub fn with_date(self, date: impl Into<String>) -> Self {
        Self {
            date: Some(date.into()),
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

    /// The date given with [`with_date`](Self::with_date), or `None`.
    #[must_use]
    pub fn date(&self) -> Option<&str> {
        self.date.as_deref()
    }

    /// Runs every check of the operation, and returns what it creates.
    pub(super) fn plan<'o>(&'o self, document: &Document) -> Result<Plan<'o>, EditError> {
        plan_create(
            document,
            ElementKind::Event,
            Some(&self.parent),
            self.id.as_ref(),
            Features::Asset {
                name: &self.name,
                strings: vec![(StringAttribute::Date, self.date.as_deref())],
                location: None,
            },
        )
    }
}

/// Creates a `Resource` (SACM 2.3 §12.10) as the last child of an element
/// whose class can hold one: an `ArtifactPackage`. A Resource is never a
/// root.
///
/// A Resource "corresponds to the tangible objects representing an
/// Artifact" (§12.10), such as a file or a printed copy. It is an
/// `ArtifactAsset`, so it can hold
/// [`properties`](crate::model::SacmElement::properties), and it has a
/// `location` (`location:Base::MultiLangString`, "the path or URL
/// specifying the location of the Resource, can be in multiple languages"),
/// given with [`with_location`](Self::with_location) and held exactly as
/// given. Apply it with [`edit::apply`](super::apply). Without
/// [`with_id`](Self::with_id), the id is generated: `resource_<n>`.
///
/// # Examples
///
/// ```
/// use sacm::edit::{self, CreateArtifactPackage, CreateResource};
/// use sacm::model::{Document, LangString, MultiLangString};
///
/// let mut doc = Document::new();
/// let en = |text: &str| LangString::new(text).with_lang("en");
/// edit::apply(&mut doc, CreateArtifactPackage::root(en("Evidence")).with_id("evidence"))?;
/// let pdf = CreateResource::new("evidence", en("Report PDF"))
///     .with_location(en("https://example.org/brake-1.2.pdf"));
/// let created = edit::apply(&mut doc, pdf)?;
///
/// let resource = doc.get(&created.created_ids()[0]).expect("just created");
/// assert_eq!(resource.id().as_str(), "resource_1");
/// let location = MultiLangString::from(en("https://example.org/brake-1.2.pdf"));
/// assert_eq!(resource.location(), Some(&location));
/// # Ok::<(), sacm::edit::EditError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
pub struct CreateResource {
    parent: ElementId,
    #[cfg_attr(feature = "serde", serde(default))]
    id: Option<ElementId>,
    name: LangString,
    #[cfg_attr(feature = "serde", serde(default))]
    location: Option<MultiLangString>,
}

impl CreateResource {
    /// Creates a `Resource` named `name` as the last child of `parent`, with
    /// no location.
    #[must_use]
    pub fn new(parent: impl Into<ElementId>, name: LangString) -> Self {
        Self {
            parent: parent.into(),
            id: None,
            name,
            location: None,
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

    /// Gives the new Resource the `location` `location`, held exactly,
    /// replacing any given before. A [`LangString`] converts into a location
    /// of one value. An empty one, or one with a repeated language, is
    /// accepted, and [`validation::validate`](crate::validation::validate)
    /// reports it (SACM23-VAL-010).
    #[must_use]
    pub fn with_location(self, location: impl Into<MultiLangString>) -> Self {
        Self {
            location: Some(location.into()),
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

    /// The location given with [`with_location`](Self::with_location), or
    /// `None`.
    #[must_use]
    pub fn location(&self) -> Option<&MultiLangString> {
        self.location.as_ref()
    }

    /// Runs every check of the operation, and returns what it creates.
    pub(super) fn plan<'o>(&'o self, document: &Document) -> Result<Plan<'o>, EditError> {
        plan_create(
            document,
            ElementKind::Resource,
            Some(&self.parent),
            self.id.as_ref(),
            Features::Asset {
                name: &self.name,
                strings: Vec::new(),
                location: self.location.as_ref(),
            },
        )
    }
}

/// Creates an `Activity` (SACM 2.3 §12.11) as the last child of an element
/// whose class can hold one: an `ArtifactPackage`. An Activity is never a
/// root.
///
/// An Activity "represents units of work related to the management of
/// `ArtifactAssets`" (§12.11), such as a test campaign or a review. It is an
/// `ArtifactAsset`, so it can hold
/// [`properties`](crate::model::SacmElement::properties), and it has a
/// `startTime` and an `endTime` (`date[0..1]`, "time when the activity
/// started" and "ended"), given with
/// [`with_start_time`](Self::with_start_time) and
/// [`with_end_time`](Self::with_end_time) and held exactly as given
/// (D-DATE); SACM 2.3 states no order between them, so none is checked.
/// Apply it with [`edit::apply`](super::apply). Without
/// [`with_id`](Self::with_id), the id is generated: `activity_<n>`.
///
/// # Examples
///
/// ```
/// use sacm::edit::{self, CreateActivity, CreateArtifactPackage};
/// use sacm::model::{Document, LangString};
///
/// let mut doc = Document::new();
/// let en = |text: &str| LangString::new(text).with_lang("en");
/// edit::apply(&mut doc, CreateArtifactPackage::root(en("Evidence")).with_id("evidence"))?;
/// let testing = CreateActivity::new("evidence", en("Brake testing"))
///     .with_id("testing")
///     .with_start_time("2024-02-12")
///     .with_end_time("2024-02-29T17:30:00Z");
/// edit::apply(&mut doc, testing)?;
///
/// let testing = doc.get("testing").expect("just created");
/// assert_eq!(testing.start_time(), Some("2024-02-12"));
/// assert_eq!(testing.end_time(), Some("2024-02-29T17:30:00Z"));
/// # Ok::<(), sacm::edit::EditError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
pub struct CreateActivity {
    parent: ElementId,
    #[cfg_attr(feature = "serde", serde(default))]
    id: Option<ElementId>,
    name: LangString,
    #[cfg_attr(feature = "serde", serde(default))]
    start_time: Option<String>,
    #[cfg_attr(feature = "serde", serde(default))]
    end_time: Option<String>,
}

impl CreateActivity {
    /// Creates an `Activity` named `name` as the last child of `parent`,
    /// with no start or end time.
    #[must_use]
    pub fn new(parent: impl Into<ElementId>, name: LangString) -> Self {
        Self {
            parent: parent.into(),
            id: None,
            name,
            start_time: None,
            end_time: None,
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

    /// Gives the new Activity the `startTime` `start_time`, exactly as
    /// given, replacing any given before (D-DATE).
    #[doc(alias = "startTime")]
    #[must_use]
    pub fn with_start_time(self, start_time: impl Into<String>) -> Self {
        Self {
            start_time: Some(start_time.into()),
            ..self
        }
    }

    /// Gives the new Activity the `endTime` `end_time`, exactly as given,
    /// replacing any given before (D-DATE).
    #[doc(alias = "endTime")]
    #[must_use]
    pub fn with_end_time(self, end_time: impl Into<String>) -> Self {
        Self {
            end_time: Some(end_time.into()),
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

    /// The start time given with [`with_start_time`](Self::with_start_time),
    /// or `None`.
    #[doc(alias = "startTime")]
    #[must_use]
    pub fn start_time(&self) -> Option<&str> {
        self.start_time.as_deref()
    }

    /// The end time given with [`with_end_time`](Self::with_end_time), or
    /// `None`.
    #[doc(alias = "endTime")]
    #[must_use]
    pub fn end_time(&self) -> Option<&str> {
        self.end_time.as_deref()
    }

    /// Runs every check of the operation, and returns what it creates.
    pub(super) fn plan<'o>(&'o self, document: &Document) -> Result<Plan<'o>, EditError> {
        plan_create(
            document,
            ElementKind::Activity,
            Some(&self.parent),
            self.id.as_ref(),
            Features::Asset {
                name: &self.name,
                strings: vec![
                    (StringAttribute::StartTime, self.start_time.as_deref()),
                    (StringAttribute::EndTime, self.end_time.as_deref()),
                ],
                location: None,
            },
        )
    }
}

/// Creates an `ArtifactAssetRelationship` (SACM 2.3 §12.14) as the last
/// child of an element whose class can hold one: an `ArtifactPackage`. An
/// `ArtifactAssetRelationship` is never a root.
///
/// An `ArtifactAssetRelationship` "enables the `ArtifactAssets` of a structured
/// assurance case to be linked together" (§12.14): its `source` and `target`
/// (`source:ArtifactAsset[1..*]`, `target:ArtifactAsset[1..*]`) name the
/// linked assets, such as a test report and the activity that produced it.
/// It is an `ArtifactAsset` itself, so it can hold
/// [`properties`](crate::model::SacmElement::properties) and be the source
/// or target of another. The ends are
/// [`ReferenceEnd::ArtifactSource`](crate::model::ReferenceEnd::ArtifactSource)
/// and
/// [`ReferenceEnd::ArtifactTarget`](crate::model::ReferenceEnd::ArtifactTarget),
/// read with [`sources`](crate::model::SacmElement::sources) and
/// [`targets`](crate::model::SacmElement::targets).
///
/// The sources and the targets are kept in the order given, repeats
/// included. Apply it with [`edit::apply`](super::apply). After [ADR 0019]'s
/// four checks, an empty `source`, then an empty `target`, is
/// [`EditError::TooFewReferences`]; then an id that holds XML white space is
/// [`EditError::InvalidReference`], then one that names no element is
/// [`EditError::ReferenceNotFound`], then one that names no `ArtifactAsset`
/// is [`EditError::WrongReferenceType`] (SACM-REF-002); each check runs over
/// the sources, then the targets, in order, before the next. Without
/// [`with_id`](Self::with_id), the id is generated:
/// `artifact_asset_relationship_<n>`.
///
/// [ADR 0019]: https://github.com/lasrod/libSACM/blob/main/docs/adr/0019-edit-operations.md
///
/// # Examples
///
/// ```
/// use sacm::edit::{self, CreateActivity, CreateArtifact, CreateArtifactAssetRelationship, CreateArtifactPackage};
/// use sacm::model::{Document, LangString, ReferenceEnd};
///
/// let mut doc = Document::new();
/// let en = |text: &str| LangString::new(text).with_lang("en");
/// edit::apply(&mut doc, CreateArtifactPackage::root(en("Evidence")).with_id("evidence"))?;
/// edit::apply(&mut doc, CreateArtifact::new("evidence", en("Brake test report")).with_id("report"))?;
/// edit::apply(&mut doc, CreateActivity::new("evidence", en("Brake testing")).with_id("testing"))?;
/// let link = CreateArtifactAssetRelationship::new("evidence", en("Produced by"), ["report"], ["testing"]);
/// let created = edit::apply(&mut doc, link)?;
///
/// let rel = doc.get(&created.created_ids()[0]).expect("just created");
/// assert_eq!(rel.id().as_str(), "artifact_asset_relationship_1");
/// assert_eq!(rel.targets().iter().map(|id| id.as_str()).collect::<Vec<_>>(), ["testing"]);
/// assert_eq!(ReferenceEnd::ArtifactTarget.name(), "target");
/// # Ok::<(), sacm::edit::EditError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
pub struct CreateArtifactAssetRelationship {
    parent: ElementId,
    #[cfg_attr(feature = "serde", serde(default))]
    id: Option<ElementId>,
    name: LangString,
    sources: Vec<ElementId>,
    targets: Vec<ElementId>,
}

impl CreateArtifactAssetRelationship {
    /// Creates an `ArtifactAssetRelationship` named `name` as the last child
    /// of `parent`, from the `ArtifactAsset`s `sources` to the
    /// `ArtifactAsset`s `targets`, each in the order given, repeats
    /// included. Each end needs at least one id
    /// (§12.14 `[1..*]`); otherwise [`apply`](super::apply) refuses with
    /// [`EditError::TooFewReferences`](super::EditError::TooFewReferences).
    #[must_use]
    pub fn new(
        parent: impl Into<ElementId>,
        name: LangString,
        sources: impl IntoIterator<Item = impl Into<ElementId>>,
        targets: impl IntoIterator<Item = impl Into<ElementId>>,
    ) -> Self {
        Self {
            parent: parent.into(),
            id: None,
            name,
            sources: sources.into_iter().map(Into::into).collect(),
            targets: targets.into_iter().map(Into::into).collect(),
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

    /// The source ids given, its `source` end, in the order given.
    #[doc(alias = "source")]
    #[must_use]
    pub fn sources(&self) -> &[ElementId] {
        &self.sources
    }

    /// The target ids given, its `target` end, in the order given.
    #[doc(alias = "target")]
    #[must_use]
    pub fn targets(&self) -> &[ElementId] {
        &self.targets
    }

    /// Runs every check of the operation, and returns what it creates.
    pub(super) fn plan<'o>(&'o self, document: &Document) -> Result<Plan<'o>, EditError> {
        plan_referencing(
            document,
            ElementKind::ArtifactAssetRelationship,
            &self.parent,
            self.id.as_ref(),
            &self.name,
            &[],
            &[
                (ReferenceEnd::ArtifactSource, &self.sources),
                (ReferenceEnd::ArtifactTarget, &self.targets),
            ],
        )
    }
}

impl From<CreateArtifactAssetRelationship> for super::Operation {
    fn from(operation: CreateArtifactAssetRelationship) -> Self {
        Self::CreateArtifactAssetRelationship(operation)
    }
}

impl From<CreateArtifact> for super::Operation {
    fn from(operation: CreateArtifact) -> Self {
        Self::CreateArtifact(operation)
    }
}

impl From<CreateEvent> for super::Operation {
    fn from(operation: CreateEvent) -> Self {
        Self::CreateEvent(operation)
    }
}

impl From<CreateResource> for super::Operation {
    fn from(operation: CreateResource) -> Self {
        Self::CreateResource(operation)
    }
}

impl From<CreateActivity> for super::Operation {
    fn from(operation: CreateActivity) -> Self {
        Self::CreateActivity(operation)
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
