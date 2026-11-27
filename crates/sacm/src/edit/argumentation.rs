//! The create operations of the argumentation kinds that reference other elements.
//!
//! - **Kinds.** `ArgumentReasoning`, `ArtifactReference`, `ArgumentGroup` and the five
//!   asserted relationships.
//! - **Rows.** SACM23-ARG-005, SACM23-ARG-006, SACM23-ARG-008 to SACM23-ARG-013,
//!   SACM23-VAL-004; LIB-EDIT-001.
//! - **Design.** ADR 0019, ADR 0027, ADR 0028.

use std::slice;

use super::EditError;
use super::create::checked_place;
use super::plan::{Features, NewElement, Plan};
#[cfg(feature = "serde")]
use super::references::Link;
use super::references::{check_bounds, check_targets, expression_ids};
use crate::model::{Document, ElementId, ElementKind, Flag, Href, LangString, ReferenceEnd};

/// Creates an `ArgumentReasoning` (SACM 2.3 §11.12) as the last child of an element
/// whose class can hold one, such as an `ArgumentPackage`.
///
/// - **What it is.** A description of the reasoning behind an asserted relationship,
///   which its [`reasoning`](crate::model::SacmElement::reasoning) names.
/// - **Structure.** With [`with_structure`](Self::with_structure) it names an
///   `ArgumentPackage` that gives the reasoning's detailed structure
///   (`structure:ArgumentPackage[0..1]`).
/// - **Id.** Without [`with_id`](Self::with_id), it is generated:
///   `argument_reasoning_<n>`.
/// - **Apply it** with [`edit::apply`](super::apply).
///
/// After [ADR 0019]'s four checks, the structure is checked in this order:
///
/// | The structure's id | Refusal |
/// |---|---|
/// | holds XML white space, such as the `a b` of an element read from a file | [`EditError::InvalidReference`] |
/// | names no element of the document | [`EditError::ReferenceNotFound`] |
/// | names an element that is not an `ArgumentPackage` | [`EditError::WrongReferenceType`] (SACM-REF-002, ADR 0028) |
///
/// [ADR 0019]: https://github.com/lasrod/libSACM/blob/main/docs/adr/0019-edit-operations.md
///
/// # Examples
///
/// ```
/// use sacm::edit::{self, CreateArgumentPackage, CreateArgumentReasoning, EditError};
/// use sacm::model::{Document, LangString};
///
/// let mut doc = Document::new();
/// let name = |text: &str| LangString::new(text).with_lang("en");
/// edit::apply(&mut doc, CreateArgumentPackage::root(name("Arguments")).with_id("p"))?;
/// edit::apply(&mut doc, CreateArgumentPackage::root(name("Hazard arguments")).with_id("hazards"))?;
///
/// let reasoning = CreateArgumentReasoning::new("p", name("Argument over each hazard"));
/// let created = edit::apply(&mut doc, reasoning.with_structure("hazards"))?;
/// assert_eq!(created.created_ids()[0].as_str(), "argument_reasoning_1");
///
/// // A structure must name an element of the document.
/// let missing = CreateArgumentReasoning::new("p", name("Over nothing")).with_structure("nope");
/// let error = edit::apply(&mut doc, missing).unwrap_err();
/// assert!(matches!(error, EditError::ReferenceNotFound { .. }));
/// assert_eq!(
///     error.to_string(),
///     "the document has no element \"nope\" for the structure to reference"
/// );
/// # Ok::<(), EditError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
pub struct CreateArgumentReasoning {
    parent: ElementId,
    #[cfg_attr(feature = "serde", serde(default))]
    id: Option<ElementId>,
    name: LangString,
    #[cfg_attr(feature = "serde", serde(default))]
    structure: Option<ElementId>,
}

impl CreateArgumentReasoning {
    /// Creates an `ArgumentReasoning` named `name` as the last child of
    /// `parent`, with no structure.
    #[must_use]
    pub fn new(parent: impl Into<ElementId>, name: impl Into<LangString>) -> Self {
        Self {
            parent: parent.into(),
            id: None,
            name: name.into(),
            structure: None,
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

    /// Gives the new element the `structure` `structure`: the id of the
    /// `ArgumentPackage` that details the reasoning.
    ///
    /// It must name an `ArgumentPackage` of the document; otherwise [`apply`](super::apply)
    /// refuses with [`EditError::ReferenceNotFound`](super::EditError::ReferenceNotFound)
    /// or [`EditError::WrongReferenceType`](super::EditError::WrongReferenceType).
    #[must_use]
    pub fn with_structure(self, structure: impl Into<ElementId>) -> Self {
        Self {
            structure: Some(structure.into()),
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

    /// The structure given with [`with_structure`](Self::with_structure), or
    /// `None`.
    #[must_use]
    pub fn structure(&self) -> Option<&ElementId> {
        self.structure.as_ref()
    }

    /// Runs every check of the operation, and returns what it creates.
    pub(super) fn plan<'o>(&'o self, document: &Document) -> Result<Plan<'o>, EditError> {
        plan_referencing(
            document,
            ElementKind::ArgumentReasoning,
            Some(&self.parent),
            self.id.as_ref(),
            &self.name,
            &[],
            &[(ReferenceEnd::Structure, self.structure.as_slice())],
        )
    }
}

/// Creates an `ArtifactReference` (SACM 2.3 §11.9) as the last child of an element whose
/// class can hold one, such as an `ArgumentPackage`.
///
/// - **What it is.** An argument's reference to artifacts, or to other elements, through
///   its
///   [`referenced_artifact_elements`](crate::model::SacmElement::referenced_artifact_elements)
///   (`referencedArtifactElement:Base::ArtifactElement[0..*]`).
/// - **An `ArgumentAsset`,** so it can be the source of an `AssertedEvidence` (§11.15),
///   or either end of an `AssertedArtifactSupport` or `AssertedArtifactContext` (§11.17,
///   §11.18).
/// - **No Assertion:** it has no `assertionDeclaration` and no `metaClaim`.
/// - **Its ids.** With
///   [`with_referenced_artifact_elements`](Self::with_referenced_artifact_elements) it
///   names the elements it refers to. None is required.
/// - **Order and repeats.** The ids are kept in the order given, repeats included;
///   validation warns about a repeat (SACM23-VAL-022).
/// - **Any kind but a `UtilityElement`** (`Description`, `ImplementationConstraint`,
///   `Note` and `TaggedValue`) conforms to `ArtifactElement`. So a Claim or a package is
///   a well-typed target, as when an argument cites a Claim of another argument package
///   (§11.1).
/// - **Id.** Without [`with_id`](Self::with_id), it is generated:
///   `artifact_reference_<n>`.
/// - **Apply it** with [`edit::apply`](super::apply).
///
/// After [ADR 0019]'s four checks, the ids are checked in this order:
///
/// | An id | Refusal |
/// |---|---|
/// | holds XML white space | [`EditError::InvalidReference`] |
/// | names no element | [`EditError::ReferenceNotFound`] |
/// | names a `UtilityElement`, which is no `ArtifactElement` | [`EditError::WrongReferenceType`] (SACM-REF-002, ADR 0028) |
///
/// [ADR 0019]: https://github.com/lasrod/libSACM/blob/main/docs/adr/0019-edit-operations.md
///
/// # Examples
///
/// ```
/// use sacm::edit::{self, CreateArgumentPackage, CreateArtifactReference, CreateClaim, CreateNote, EditError};
/// use sacm::model::{Document, LangString};
///
/// let mut doc = Document::new();
/// let name = |text: &str| LangString::new(text).with_lang("en");
/// edit::apply(&mut doc, CreateArgumentPackage::root(name("Arguments")).with_id("p"))?;
/// edit::apply(&mut doc, CreateArgumentPackage::root(name("Supplier arguments")).with_id("s"))?;
/// edit::apply(&mut doc, CreateClaim::new("s", name("The supplier's brakes meet the spec")).with_id("c1"))?;
///
/// // The argument in p cites the supplier's Claim.
/// let reference = CreateArtifactReference::new("p", name("The supplier's brake claim"))
///     .with_referenced_artifact_elements(["c1"]);
/// let created = edit::apply(&mut doc, reference)?;
/// assert_eq!(created.created_ids()[0].as_str(), "artifact_reference_1");
///
/// // A Note is a UtilityElement, which is no ArtifactElement.
/// edit::apply(&mut doc, CreateNote::new("c1", name("Reviewed")).with_id("n1"))?;
/// let wrong = CreateArtifactReference::new("p", name("?")).with_referenced_artifact_elements(["n1"]);
/// let error = edit::apply(&mut doc, wrong).unwrap_err();
/// assert!(matches!(error, EditError::WrongReferenceType { .. }));
/// # Ok::<(), EditError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
pub struct CreateArtifactReference {
    parent: ElementId,
    #[cfg_attr(feature = "serde", serde(default))]
    id: Option<ElementId>,
    name: LangString,
    #[cfg_attr(feature = "serde", serde(default))]
    referenced_artifact_elements: Vec<ElementId>,
}

impl CreateArtifactReference {
    /// Creates an `ArtifactReference` named `name` as the last child of
    /// `parent`, referring to nothing.
    #[must_use]
    pub fn new(parent: impl Into<ElementId>, name: impl Into<LangString>) -> Self {
        Self {
            parent: parent.into(),
            id: None,
            name: name.into(),
            referenced_artifact_elements: Vec::new(),
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

    /// Gives the new element the `referencedArtifactElement` ids `ids`, in the order
    /// given, repeats included, replacing any given before.
    ///
    /// Each must name an `ArtifactElement` of the document (any element but a
    /// `UtilityElement`); otherwise [`apply`](super::apply) refuses with
    /// [`EditError::ReferenceNotFound`](super::EditError::ReferenceNotFound) or
    /// [`EditError::WrongReferenceType`](super::EditError::WrongReferenceType).
    #[must_use]
    pub fn with_referenced_artifact_elements(
        self,
        ids: impl IntoIterator<Item = impl Into<ElementId>>,
    ) -> Self {
        Self {
            referenced_artifact_elements: ids.into_iter().map(Into::into).collect(),
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

    /// The `referencedArtifactElement` ids, in the order given: empty unless given.
    ///
    /// They are given with
    /// [`with_referenced_artifact_elements`](Self::with_referenced_artifact_elements).
    #[must_use]
    pub fn referenced_artifact_elements(&self) -> &[ElementId] {
        &self.referenced_artifact_elements
    }

    /// Runs every check of the operation, and returns what it creates.
    pub(super) fn plan<'o>(&'o self, document: &Document) -> Result<Plan<'o>, EditError> {
        plan_referencing(
            document,
            ElementKind::ArtifactReference,
            Some(&self.parent),
            self.id.as_ref(),
            &self.name,
            &[],
            &[(
                ReferenceEnd::ReferencedArtifactElement,
                &self.referenced_artifact_elements,
            )],
        )
    }
}

/// Creates an `ArgumentGroup` (SACM 2.3 §11.2) as the last child of an element whose
/// class can hold one, such as an `ArgumentPackage`.
///
/// - **What it is.** A named grouping of argumentation elements, its
///   [`group_members`](crate::model::SacmElement::group_members)
///   (`argumentElement:ArgumentationElement[0..*]`).
/// - **Members are references, not children.** A member stays where it is contained, and
///   `ArgumentGroups` "serve no structural purpose in the formation of the argument
///   network" (§11.2).
/// - **Its members.** With [`with_group_members`](Self::with_group_members) it names
///   them. None is required, and the ids are kept in the order given, repeats included.
/// - **Id.** Without [`with_id`](Self::with_id), it is generated: `argument_group_<n>`.
/// - **Apply it** with [`edit::apply`](super::apply).
///
/// After [ADR 0019]'s four checks, the ids are checked in this order:
///
/// | An id | Refusal |
/// |---|---|
/// | holds XML white space | [`EditError::InvalidReference`] |
/// | names no element | [`EditError::ReferenceNotFound`] |
/// | names an element that is no `ArgumentationElement`, such as an `AssuranceCasePackage` or a Note | [`EditError::WrongReferenceType`] (SACM-REF-002, ADR 0028) |
///
/// [ADR 0019]: https://github.com/lasrod/libSACM/blob/main/docs/adr/0019-edit-operations.md
///
/// # Examples
///
/// ```
/// use sacm::edit::{self, CreateArgumentGroup, CreateArgumentPackage, CreateClaim, EditError};
/// use sacm::model::{Document, LangString};
///
/// let mut doc = Document::new();
/// let name = |text: &str| LangString::new(text).with_lang("en");
/// edit::apply(&mut doc, CreateArgumentPackage::root(name("Arguments")).with_id("p"))?;
/// edit::apply(&mut doc, CreateClaim::new("p", name("The brakes are safe")).with_id("c1"))?;
/// edit::apply(&mut doc, CreateClaim::new("p", name("The pump is safe")).with_id("c2"))?;
///
/// let group = CreateArgumentGroup::new("p", name("Hardware")).with_group_members(["c1", "c2"]);
/// let created = edit::apply(&mut doc, group)?;
/// let g = doc.get(&created.created_ids()[0]).expect("just created");
/// assert_eq!(g.id().as_str(), "argument_group_1");
/// assert_eq!(g.group_members().len(), 2);
/// # Ok::<(), EditError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
pub struct CreateArgumentGroup {
    parent: ElementId,
    #[cfg_attr(feature = "serde", serde(default))]
    id: Option<ElementId>,
    name: LangString,
    #[cfg_attr(feature = "serde", serde(default))]
    group_members: Vec<ElementId>,
}

impl CreateArgumentGroup {
    /// Creates an `ArgumentGroup` named `name` as the last child of
    /// `parent`, with no members.
    #[must_use]
    pub fn new(parent: impl Into<ElementId>, name: impl Into<LangString>) -> Self {
        Self {
            parent: parent.into(),
            id: None,
            name: name.into(),
            group_members: Vec::new(),
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

    /// Gives the new group the members `ids`, its `argumentElement` ids, in the order
    /// given, repeats included, replacing any given before.
    ///
    /// Each must name an `ArgumentationElement` of the document; otherwise
    /// [`apply`](super::apply) refuses with
    /// [`EditError::ReferenceNotFound`](super::EditError::ReferenceNotFound) or
    /// [`EditError::WrongReferenceType`](super::EditError::WrongReferenceType).
    #[doc(alias = "argumentElement")]
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
    #[doc(alias = "argumentElement")]
    #[must_use]
    pub fn group_members(&self) -> &[ElementId] {
        &self.group_members
    }

    /// Runs every check of the operation, and returns what it creates.
    pub(super) fn plan<'o>(&'o self, document: &Document) -> Result<Plan<'o>, EditError> {
        plan_referencing(
            document,
            ElementKind::ArgumentGroup,
            Some(&self.parent),
            self.id.as_ref(),
            &self.name,
            &[],
            &[(ReferenceEnd::GroupMember, &self.group_members)],
        )
    }
}

/// What the create operations of the asserted relationships are given
/// (SACM 2.3 §11.13), shared by all five.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
struct Relationship {
    parent: ElementId,
    #[cfg_attr(feature = "serde", serde(default))]
    id: Option<ElementId>,
    name: LangString,
    sources: Vec<ElementId>,
    target: ElementId,
    #[cfg_attr(feature = "serde", serde(default))]
    reasoning: Option<ElementId>,
    #[cfg_attr(feature = "serde", serde(default))]
    is_counter: bool,
}

impl Relationship {
    /// What [`new`](CreateAssertedInference::new) is given, with no options.
    fn new(
        parent: impl Into<ElementId>,
        name: LangString,
        sources: impl IntoIterator<Item = impl Into<ElementId>>,
        target: impl Into<ElementId>,
    ) -> Self {
        Self {
            parent: parent.into(),
            id: None,
            name,
            sources: sources.into_iter().map(Into::into).collect(),
            target: target.into(),
            reasoning: None,
            is_counter: false,
        }
    }

    /// Runs every check of the create of a relationship of `kind`, and
    /// returns what it creates: after [ADR 0019]'s four checks, at least
    /// one source, then every source, in order, the target and the
    /// reasoning holding no white space, then each naming an element, then
    /// each naming an element of its end's declared type.
    ///
    /// [ADR 0019]: https://github.com/lasrod/libSACM/blob/main/docs/adr/0019-edit-operations.md
    fn plan<'o>(&'o self, document: &Document, kind: ElementKind) -> Result<Plan<'o>, EditError> {
        plan_referencing(
            document,
            kind,
            Some(&self.parent),
            self.id.as_ref(),
            &self.name,
            &[(Flag::IsCounter, self.is_counter)],
            &[
                (ReferenceEnd::Source, &self.sources),
                (ReferenceEnd::Target, slice::from_ref(&self.target)),
                (ReferenceEnd::Reasoning, self.reasoning.as_slice()),
            ],
        )
    }
}

/// Defines the create operation of one package interface kind (ADR 0039 D4):
/// the struct `$name`, given a parent or none, a name and the package the
/// interface `implements`, and its methods.
macro_rules! interface_operation {
    (
        $(#[$meta:meta])*
        $name:ident, $kind:ident, $package:literal
    ) => {
        $(#[$meta])*
        #[derive(Clone, Debug, PartialEq, Eq)]
        #[cfg_attr(
            feature = "serde",
            derive(serde::Serialize, serde::Deserialize),
            serde(
                try_from = "crate::edit::argumentation::InterfaceRecord",
                into = "crate::edit::argumentation::InterfaceRecord"
            )
        )]
        pub struct $name {
            parent: Option<ElementId>,
            id: Option<ElementId>,
            name: LangString,
            implements: $crate::edit::references::Link,
        }

        #[cfg(feature = "serde")]
        impl TryFrom<$crate::edit::argumentation::InterfaceRecord> for $name {
            type Error = String;

            fn try_from(
                record: $crate::edit::argumentation::InterfaceRecord,
            ) -> Result<Self, String> {
                let (parent, id, name, implements) = record.into_parts()?;
                Ok(Self {
                    parent,
                    id,
                    name,
                    implements,
                })
            }
        }

        #[cfg(feature = "serde")]
        impl From<$name> for $crate::edit::argumentation::InterfaceRecord {
            fn from(operation: $name) -> Self {
                Self::new(
                    operation.parent,
                    operation.id,
                    operation.name,
                    operation.implements,
                )
            }
        }

        impl $name {
            #[doc = concat!(
                "Creates a root `", stringify!($kind), "` named `name`, after the document's ",
                "last root, declaring the `", $package, "` `implements` names."
            )]
            #[must_use]
            pub fn root(name: impl Into<LangString>, implements: impl Into<ElementId>) -> Self {
                Self {
                    parent: None,
                    id: None,
                    name: name.into(),
                    implements: $crate::edit::references::Link::Id(implements.into()),
                }
            }

            #[doc = concat!(
                "Creates a root `", stringify!($kind), "` named `name`, after the document's ",
                "last root, declaring the `", $package, "` of another document that the ",
                "href `implements` names (LIB-EDIT-020; ADR 0040 amendment 1).\n\n",
                "The href is never checked: the package need not exist."
            )]
            #[must_use]
            pub fn root_href(name: impl Into<LangString>, implements: $crate::model::Href) -> Self {
                Self {
                    parent: None,
                    id: None,
                    name: name.into(),
                    implements: $crate::edit::references::Link::Href(implements),
                }
            }

            #[doc = concat!(
                "Creates a `", stringify!($kind), "` named `name` as the last child of ",
                "`parent`, declaring the `", $package, "` of another document that the ",
                "href `implements` names (LIB-EDIT-020; ADR 0040 amendment 1).\n\n",
                "The href is never checked: the package need not exist."
            )]
            #[must_use]
            pub fn new_href(
                parent: impl Into<ElementId>,
                name: impl Into<LangString>,
                implements: $crate::model::Href,
            ) -> Self {
                Self {
                    parent: Some(parent.into()),
                    id: None,
                    name: name.into(),
                    implements: $crate::edit::references::Link::Href(implements),
                }
            }

            #[doc = concat!(
                "Creates a `", stringify!($kind), "` named `name` as the last child of ",
                "`parent`, declaring the `", $package, "` `implements` names."
            )]
            #[must_use]
            pub fn new(
                parent: impl Into<ElementId>,
                name: impl Into<LangString>,
                implements: impl Into<ElementId>,
            ) -> Self {
                Self {
                    parent: Some(parent.into()),
                    id: None,
                    name: name.into(),
                    implements: $crate::edit::references::Link::Id(implements.into()),
                }
            }

            /// Gives the new element the id `id` instead of a generated one.
            ///
            /// It must be an XML `NCName` and not already in the document. Otherwise
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

            /// The id of the package the interface declares, its `implements`, or
            /// `None` when an href names it ([`implements_href`](Self::implements_href)).
            #[must_use]
            pub fn implements(&self) -> Option<&ElementId> {
                self.implements.id()
            }

            /// The href that names the package the interface declares, in another
            /// document, or `None` when an id names it ([`implements`](Self::implements)).
            #[must_use]
            pub fn implements_href(&self) -> Option<&$crate::model::Href> {
                self.implements.href()
            }

            /// Runs every check of the operation, and returns what it
            /// creates.
            pub(super) fn plan<'o>(
                &'o self,
                document: &Document,
            ) -> Result<Plan<'o>, EditError> {
                let end = ReferenceEnd::Implements;
                let (ids, hrefs) = match &self.implements {
                    $crate::edit::references::Link::Id(id) => (std::slice::from_ref(id), None),
                    $crate::edit::references::Link::Href(href) => {
                        (&[][..], Some((end, std::slice::from_ref(href))))
                    }
                };
                $crate::edit::argumentation::plan_linking(
                    document,
                    ElementKind::$kind,
                    (self.parent.as_ref(), self.id.as_ref()),
                    &self.name,
                    &[],
                    &[(end, ids)],
                    hrefs,
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

/// The JSON of an interface create: `parent` (required, `null` for a root),
/// `id`, `name`, and exactly one of `implements`, an id, and
/// `implements_href` (ADR 0030; ADR 0040 amendment 1, A7).
#[cfg(feature = "serde")]
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct InterfaceRecord {
    #[serde(deserialize_with = "serde::Deserialize::deserialize")]
    parent: Option<ElementId>,
    #[serde(default)]
    id: Option<ElementId>,
    name: LangString,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "super::references::given"
    )]
    implements: Option<ElementId>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "super::references::given"
    )]
    implements_href: Option<Href>,
}

#[cfg(feature = "serde")]
impl InterfaceRecord {
    /// The record of an interface create.
    pub(crate) fn new(
        parent: Option<ElementId>,
        id: Option<ElementId>,
        name: LangString,
        implements: Link,
    ) -> Self {
        let (implements, implements_href) = match implements {
            Link::Id(id) => (Some(id), None),
            Link::Href(href) => (None, Some(href)),
        };
        Self {
            parent,
            id,
            name,
            implements,
            implements_href,
        }
    }

    /// The parent, the id, the name and what `implements` names.
    pub(crate) fn into_parts(
        self,
    ) -> Result<(Option<ElementId>, Option<ElementId>, LangString, Link), String> {
        let implements = Link::from_json(
            self.implements,
            self.implements_href,
            ("implements", "implements_href"),
        )?;
        Ok((self.parent, self.id, self.name, implements))
    }
}

/// Defines the create operation of one package binding kind (ADR 0039 D4):
/// the struct `$name`, given a parent or none, a name and the packages the
/// binding binds, and its methods.
macro_rules! binding_operation {
    (
        $(#[$meta:meta])*
        $name:ident, $kind:ident, $package:literal
    ) => {
        $(#[$meta])*
        #[derive(Clone, Debug, PartialEq, Eq)]
        #[cfg_attr(
            feature = "serde",
            derive(serde::Serialize, serde::Deserialize),
            serde(deny_unknown_fields)
        )]
        pub struct $name {
            #[cfg_attr(
                feature = "serde",
                serde(deserialize_with = "serde::Deserialize::deserialize")
            )]
            parent: Option<ElementId>,
            #[cfg_attr(feature = "serde", serde(default))]
            id: Option<ElementId>,
            name: LangString,
            participant_packages: Vec<ElementId>,
            #[cfg_attr(
                feature = "serde",
                serde(default, skip_serializing_if = "Vec::is_empty")
            )]
            participant_package_hrefs: Vec<$crate::model::Href>,
        }

        impl $name {
            #[doc = concat!(
                "Creates a root `", stringify!($kind), "` named `name`, after the document's ",
                "last root, binding the packages `participant_packages` names (each an `", $package, "`), in the ",
                "order given, repeats included."
            )]
            #[must_use]
            pub fn root(
                name: impl Into<LangString>,
                participant_packages: impl IntoIterator<Item = impl Into<ElementId>>,
            ) -> Self {
                Self {
                    parent: None,
                    id: None,
                    name: name.into(),
                    participant_packages: participant_packages.into_iter().map(Into::into).collect(),
                    participant_package_hrefs: Vec::new(),
                }
            }

            #[doc = concat!(
                "Creates a `", stringify!($kind), "` named `name` as the last child of ",
                "`parent`, binding the packages `participant_packages` names (each an `", $package, "`), in the ",
                "order given, repeats included."
            )]
            #[must_use]
            pub fn new(
                parent: impl Into<ElementId>,
                name: impl Into<LangString>,
                participant_packages: impl IntoIterator<Item = impl Into<ElementId>>,
            ) -> Self {
                Self {
                    parent: Some(parent.into()),
                    id: None,
                    name: name.into(),
                    participant_packages: participant_packages.into_iter().map(Into::into).collect(),
                    participant_package_hrefs: Vec::new(),
                }
            }

            /// This binding with `hrefs` as the cross-document references of its
            /// `participantPackage`, beside its ids (LIB-EDIT-020; ADR 0040 amendment 1).
            ///
            /// - **Kept** in order, repeats included, replacing any given before.
            /// - **Never checked.** Each names a package of another document.
            /// - **The bound.** The two distinct participants a binding needs count the
            ///   distinct ids plus the distinct hrefs.
            #[must_use]
            pub fn with_participant_package_hrefs(
                self,
                hrefs: impl IntoIterator<Item = $crate::model::Href>,
            ) -> Self {
                Self {
                    participant_package_hrefs: hrefs.into_iter().collect(),
                    ..self
                }
            }

            /// Gives the new element the id `id` instead of a generated one.
            ///
            /// It must be an XML `NCName` and not already in the document. Otherwise
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

            /// The ids of the packages the binding binds, its
            /// `participantPackage`, in the order given, repeats included.
            #[doc(alias = "participantPackage")]
            #[must_use]
            pub fn participant_packages(&self) -> &[ElementId] {
                &self.participant_packages
            }

            /// The cross-document references of the binding's `participantPackage`, in
            /// the order given.
            ///
            /// Empty unless
            /// [`with_participant_package_hrefs`](Self::with_participant_package_hrefs)
            /// gave some.
            #[doc(alias = "participantPackage")]
            #[must_use]
            pub fn participant_package_hrefs(&self) -> &[$crate::model::Href] {
                &self.participant_package_hrefs
            }

            /// Runs every check of the operation, and returns what it
            /// creates.
            pub(super) fn plan<'o>(
                &'o self,
                document: &Document,
            ) -> Result<Plan<'o>, EditError> {
                let end = ReferenceEnd::ParticipantPackage;
                $crate::edit::argumentation::plan_linking(
                    document,
                    ElementKind::$kind,
                    (self.parent.as_ref(), self.id.as_ref()),
                    &self.name,
                    &[],
                    &[(end, &self.participant_packages)],
                    Some((end, self.participant_package_hrefs.as_slice())),
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

pub(super) use {binding_operation, interface_operation};

interface_operation! {
    /// Creates an `ArgumentPackageInterface` (SACM 2.3 §11.6) declaring the
    /// ArgumentPackage that `implements` names.
    ///
    /// - **Where.** A root, or a child of an AssuranceCasePackage, an ArgumentPackage or
    ///   an ArtifactPackage (or one of their interfaces and bindings).
    /// - **`implements` is required** (`implements:ArgumentPackage[1]`). The package's
    ///   `interface` end is not changed (ADR 0039).
    /// - **Id.** Without [`with_id`](Self::with_id), it is generated:
    ///   `argument_package_interface_<n>`.
    ///
    /// After [ADR 0019]'s four checks and the name's expression, the `implements` id is
    /// checked in this order:
    ///
    /// | The id | Refusal |
    /// |---|---|
    /// | holds XML white space | [`EditError::InvalidReference`] |
    /// | names no element | [`EditError::ReferenceNotFound`] |
    /// | names an element of no ArgumentPackage kind | [`EditError::WrongReferenceType`] (SACM-REF-002) |
    ///
    /// [ADR 0019]: https://github.com/lasrod/libSACM/blob/main/docs/adr/0019-edit-operations.md
    ///
    /// # Examples
    ///
    /// ```
    /// use sacm::edit::{self, AddReference, CreateArgumentPackage, CreateArgumentPackageBinding, CreateArgumentPackageInterface};
    /// use sacm::model::{Document, LangString, ReferenceEnd};
    ///
    /// let en = |t: &str| LangString::new(t).with_lang("en");
    /// let mut doc = Document::new();
    /// for (id, api) in [("braking", "braking_api"), ("vehicle", "vehicle_api")] {
    ///     edit::apply(&mut doc, CreateArgumentPackage::root(en(id)).with_id(id))?;
    ///     edit::apply(&mut doc, CreateArgumentPackageInterface::new(id, en(api), id).with_id(api))?;
    ///     // The package's `interface` end is never synced: declare it yourself.
    ///     edit::apply(&mut doc, AddReference::new(id, ReferenceEnd::Interface, api))?;
    /// }
    /// let bind = CreateArgumentPackageBinding::root(en("Braking supports the vehicle"), ["braking_api", "vehicle_api"]);
    /// edit::apply(&mut doc, bind.with_id("binding"))?;
    /// assert_eq!(doc.get("braking_api").and_then(|i| i.implements()).map(|id| id.as_str()), Some("braking"));
    /// assert_eq!(doc.get("binding").map(|b| b.participant_packages().len()), Some(2));
    /// # Ok::<(), sacm::edit::EditError>(())
    /// ```
    CreateArgumentPackageInterface, ArgumentPackageInterface, "ArgumentPackage"
}

binding_operation! {
    /// Creates an `ArgumentPackageBinding` (SACM 2.3 §11.5) binding the packages that
    /// `participant_packages` names, each of an `ArgumentPackage` kind.
    ///
    /// - **Where.** A root, or a child of an AssuranceCasePackage, an ArgumentPackage or
    ///   an ArtifactPackage (or one of their interfaces and bindings).
    /// - **Id.** Without [`with_id`](Self::with_id), it is generated:
    ///   `argument_package_binding_<n>`.
    /// - **With interfaces.** [`CreateArgumentPackageInterface`] shows a binding of two
    ///   interfaces.
    ///
    /// After [ADR 0019]'s four checks, the participants
    /// (`participantPackage:ArgumentPackage[2..*]`) are checked in this order, each check
    /// over the ids in order:
    ///
    /// | Check | Refusal |
    /// |---|---|
    /// | fewer than two distinct ids | [`EditError::TooFewReferences`] (ADR 0039 D3) |
    /// | then the name's expression | as [`apply`](super::apply#creates) lists |
    /// | an id holds XML white space | [`EditError::InvalidReference`] |
    /// | an id names no element | [`EditError::ReferenceNotFound`] |
    /// | an id names an element of no ArgumentPackage kind | [`EditError::WrongReferenceType`] |
    ///
    /// [ADR 0019]: https://github.com/lasrod/libSACM/blob/main/docs/adr/0019-edit-operations.md
    ///
    /// # Examples
    ///
    /// ```
    /// use sacm::edit::{self, CreateArgumentPackage, CreateArgumentPackageBinding, EditError};
    /// use sacm::model::Document;
    ///
    /// let mut doc = Document::new();
    /// edit::apply(&mut doc, CreateArgumentPackage::root("Braking").with_id("braking"))?;
    /// edit::apply(&mut doc, CreateArgumentPackage::root("Vehicle").with_id("vehicle"))?;
    ///
    /// let binding = CreateArgumentPackageBinding::root("Bound", ["braking", "vehicle"]);
    /// edit::apply(&mut doc, binding.with_id("binding"))?;
    /// assert_eq!(doc.get("binding").map(|b| b.participant_packages().len()), Some(2));
    ///
    /// // A binding needs two distinct participants.
    /// let one = CreateArgumentPackageBinding::root("One", ["braking", "braking"]);
    /// let error = edit::apply(&mut doc, one).unwrap_err();
    /// assert!(matches!(error, EditError::TooFewReferences { given: 1, required: 2, .. }));
    /// # Ok::<(), EditError>(())
    /// ```
    CreateArgumentPackageBinding, ArgumentPackageBinding, "ArgumentPackage"
}

/// Defines the create operation of one asserted-relationship kind: the
/// struct `$name`, which holds a [`Relationship`], its methods, and its
/// conversion into an [`Operation`](super::Operation).
macro_rules! relationship_operation {
    (
        $(#[$meta:meta])*
        $name:ident, $kind:ident, $new:literal
    ) => {
        $(#[$meta])*
        #[derive(Clone, Debug, PartialEq, Eq)]
        #[cfg_attr(
            feature = "serde",
            derive(serde::Serialize, serde::Deserialize),
            serde(transparent)
        )]
        pub struct $name {
            relationship: Relationship,
        }

        impl $name {
            #[doc = concat!(
                "Creates an `", stringify!($kind), "` named `name` as the last child of ",
                "`parent`, ", $new, " The `source` ids are kept in the order given, ",
                "repeats included. It has no reasoning, and `isCounter` is false."
            )]
            #[must_use]
            pub fn new(
                parent: impl Into<ElementId>,
                name: impl Into<LangString>,
                sources: impl IntoIterator<Item = impl Into<ElementId>>,
                target: impl Into<ElementId>,
            ) -> Self {
                Self {
                    relationship: Relationship::new(parent, name.into(), sources, target),
                }
            }

            /// Gives the new element the id `id` instead of a generated one.
            ///
            /// It must be an XML `NCName` and not already in the document. Otherwise
            /// [`apply`](super::apply) refuses with
            /// [`EditError::InvalidId`](super::EditError::InvalidId) or
            /// [`EditError::DuplicateId`](super::EditError::DuplicateId).
            #[must_use]
            pub fn with_id(mut self, id: impl Into<ElementId>) -> Self {
                self.relationship.id = Some(id.into());
                self
            }

            /// Gives the new element the `reasoning` `reasoning`: the id of the
            /// `ArgumentReasoning` that describes the reasoning behind it.
            ///
            /// It must name an `ArgumentReasoning` of the document; otherwise
            /// [`apply`](super::apply) refuses with
            /// [`EditError::ReferenceNotFound`](super::EditError::ReferenceNotFound) or
            /// [`EditError::WrongReferenceType`](super::EditError::WrongReferenceType).
            #[must_use]
            pub fn with_reasoning(mut self, reasoning: impl Into<ElementId>) -> Self {
                self.relationship.reasoning = Some(reasoning.into());
                self
            }

            /// Sets the new element's `isCounter`: whether it counters its
            /// declared purpose (§11.13). `false` by default.
            #[must_use]
            pub fn with_is_counter(mut self, is_counter: bool) -> Self {
                self.relationship.is_counter = is_counter;
                self
            }

            /// The parent's id.
            #[must_use]
            pub fn parent(&self) -> &ElementId {
                &self.relationship.parent
            }

            /// The id given with [`with_id`](Self::with_id), or `None` when
            /// the id is to be generated.
            #[must_use]
            pub fn id(&self) -> Option<&ElementId> {
                self.relationship.id.as_ref()
            }

            /// The new element's name.
            #[must_use]
            pub fn name(&self) -> &LangString {
                &self.relationship.name
            }

            /// The `source` ids, in the order given, repeats included.
            #[must_use]
            pub fn sources(&self) -> &[ElementId] {
                &self.relationship.sources
            }

            /// The `target` id.
            #[must_use]
            pub fn target(&self) -> &ElementId {
                &self.relationship.target
            }

            /// The reasoning given with
            /// [`with_reasoning`](Self::with_reasoning), or `None`.
            #[must_use]
            pub fn reasoning(&self) -> Option<&ElementId> {
                self.relationship.reasoning.as_ref()
            }

            /// The `isCounter` set with
            /// [`with_is_counter`](Self::with_is_counter): `false` unless set.
            #[must_use]
            pub fn is_counter(&self) -> bool {
                self.relationship.is_counter
            }

            /// Runs every check of the operation, and returns what it
            /// creates.
            pub(super) fn plan<'o>(
                &'o self,
                document: &Document,
            ) -> Result<Plan<'o>, EditError> {
                self.relationship.plan(document, ElementKind::$kind)
            }
        }

        impl From<$name> for super::Operation {
            fn from(operation: $name) -> Self {
                Self::$name(operation)
            }
        }
    };
}

relationship_operation! {
    /// Creates an `AssertedInference` (SACM 2.3 §11.14): an assertion that its sources,
    /// the premises, support its target, the conclusion.
    ///
    /// - **Where.** The last child of an element whose class can hold one, such as an
    ///   `ArgumentPackage`.
    /// - **Takes** the sources (`source:ArgumentAsset[1..*]`) and the target
    ///   (`target:ArgumentAsset[1]`), and optionally a
    ///   [`reasoning`](Self::with_reasoning) and [`isCounter`](Self::with_is_counter).
    /// - **Declaration.** Its `assertionDeclaration` is `asserted`, the default (§11.10).
    /// - **A repeated source is kept** (ADR 0027), and validation warns about it
    ///   (SACM23-VAL-022).
    /// - **Id.** Without [`with_id`](Self::with_id), it is generated:
    ///   `asserted_inference_<n>`.
    /// - **Apply it** with [`edit::apply`](super::apply).
    ///
    /// After [ADR 0019]'s four checks, it checks in this order, each over the sources
    /// in order, then the target, then the reasoning:
    ///
    /// | Check | Refusal |
    /// |---|---|
    /// | there is a source | [`EditError::TooFewReferences`] |
    /// | no id holds XML white space, since it could not be written as one id | [`EditError::InvalidReference`] |
    /// | every id names an element of the document | [`EditError::ReferenceNotFound`] |
    /// | every id names an element of a kind its end admits: an `ArgumentAsset` for a source or the target, an `ArgumentReasoning` for the reasoning | [`EditError::WrongReferenceType`] (SACM-REF-002, ADR 0028) |
    ///
    /// - **No dangling or wrongly typed reference.** An edit never makes one
    ///   (D-VAL-LAYER).
    /// - **Not an element:** the id of a name, `content`, `key` or `value`.
    ///
    /// [ADR 0019]: https://github.com/lasrod/libSACM/blob/main/docs/adr/0019-edit-operations.md
    ///
    /// # Examples
    ///
    /// ```
    /// use sacm::edit::{self, CreateArgumentPackage, CreateAssertedInference, CreateClaim, EditError};
    /// use sacm::model::{Document, LangString};
    ///
    /// let mut doc = Document::new();
    /// let name = |text: &str| LangString::new(text).with_lang("en");
    /// edit::apply(&mut doc, CreateArgumentPackage::root(name("Arguments")).with_id("p"))?;
    /// for (id, text) in [
    ///     ("c1", "The brakes are safe"),
    ///     ("c2", "Each brake hazard is mitigated"),
    ///     ("c3", "The hazard list is complete"),
    /// ] {
    ///     edit::apply(&mut doc, CreateClaim::new("p", name(text)).with_id(id))?;
    /// }
    ///
    /// let inference = CreateAssertedInference::new("p", name("The hazards support safety"), ["c2", "c3"], "c1");
    /// let created = edit::apply(&mut doc, inference)?;
    /// let i1 = doc.get(&created.created_ids()[0]).expect("just created");
    /// assert_eq!(i1.id().as_str(), "asserted_inference_1");
    /// assert_eq!(i1.sources().len(), 2);
    ///
    /// // At least one source, and every end names an element.
    /// let none: [&str; 0] = [];
    /// let error = edit::apply(&mut doc, CreateAssertedInference::new("p", name("?"), none, "c1")).unwrap_err();
    /// assert!(matches!(error, EditError::TooFewReferences { .. }));
    /// let error = edit::apply(&mut doc, CreateAssertedInference::new("p", name("?"), ["c2"], "c9")).unwrap_err();
    /// assert_eq!(error.to_string(), "the document has no element \"c9\" for the target to reference");
    /// # Ok::<(), EditError>(())
    /// ```
    CreateAssertedInference, AssertedInference,
    "inferring the conclusion `target` from the premises `sources`."
}

relationship_operation! {
    /// Creates an `AssertedContext` (SACM 2.3 §11.16): an assertion that its sources
    /// give the context of its target.
    ///
    /// - **Where.** The last child of an element whose class can hold one, such as an
    ///   `ArgumentPackage`.
    /// - **Rules.** Those of [`CreateAssertedInference`].
    /// - **Id.** Without [`with_id`](Self::with_id), it is generated:
    ///   `asserted_context_<n>`.
    ///
    /// # Examples
    ///
    /// ```
    /// use sacm::edit::{self, CreateArgumentPackage, CreateAssertedContext, CreateClaim};
    /// use sacm::model::{Document, LangString};
    ///
    /// let mut doc = Document::new();
    /// let name = |text: &str| LangString::new(text).with_lang("en");
    /// edit::apply(&mut doc, CreateArgumentPackage::root(name("Arguments")).with_id("p"))?;
    /// edit::apply(&mut doc, CreateClaim::new("p", name("The brakes are safe")).with_id("c1"))?;
    /// edit::apply(&mut doc, CreateClaim::new("p", name("In urban traffic")).with_id("c2"))?;
    ///
    /// let context = CreateAssertedContext::new("p", name("Urban traffic is the context"), ["c2"], "c1");
    /// edit::apply(&mut doc, context.with_id("x1"))?;
    /// let x1 = doc.get("x1").expect("just created");
    /// assert_eq!(x1.target().map(|id| id.as_str()), Some("c1"));
    /// # Ok::<(), sacm::edit::EditError>(())
    /// ```
    CreateAssertedContext, AssertedContext,
    "giving the `sources` as the context of `target`."
}

relationship_operation! {
    /// Creates an `AssertedEvidence` (SACM 2.3 §11.15): an assertion that its sources,
    /// the evidence, support its target, such as a Claim.
    ///
    /// - **Where.** The last child of an element whose class can hold one, such as an
    ///   `ArgumentPackage`.
    /// - **Counter-evidence.** With [`isCounter`](Self::with_is_counter) true (§11.13).
    /// - **Rules.** Those of [`CreateAssertedInference`].
    /// - **Sources are content.** §11.15 requires each source to be an
    ///   `ArtifactReference`. A create does not refuse another `ArgumentAsset`, such as a
    ///   Claim; [`validation::validate`](crate::validation::validate) reports it as
    ///   SACM-REF-002 (SACM23-VAL-016, D-VAL-LAYER).
    /// - **Validate before a strict save,** since the §11.15 rule is not refused here.
    ///   Its target is not narrowed.
    /// - **Id.** Without [`with_id`](Self::with_id), it is generated:
    ///   `asserted_evidence_<n>`.
    ///
    /// # Examples
    ///
    /// ```
    /// use sacm::edit::{self, CreateArgumentPackage, CreateArtifactReference, CreateAssertedEvidence, CreateClaim};
    /// use sacm::model::{Document, LangString};
    /// use sacm::validation;
    ///
    /// let mut doc = Document::new();
    /// let name = |text: &str| LangString::new(text).with_lang("en");
    /// edit::apply(&mut doc, CreateArgumentPackage::root(name("Arguments")).with_id("p"))?;
    /// edit::apply(&mut doc, CreateClaim::new("p", name("The brakes are safe")).with_id("c1"))?;
    /// edit::apply(&mut doc, CreateArtifactReference::new("p", name("The brake test report")).with_id("a1"))?;
    ///
    /// let evidence = CreateAssertedEvidence::new("p", name("The tests show it"), ["a1"], "c1");
    /// let created = edit::apply(&mut doc, evidence)?;
    /// assert_eq!(created.created_ids()[0].as_str(), "asserted_evidence_1");
    /// assert!(validation::validate(&doc).is_empty());
    /// # Ok::<(), sacm::edit::EditError>(())
    /// ```
    CreateAssertedEvidence, AssertedEvidence,
    "supporting `target`, such as a Claim, with the evidence `sources`."
}

relationship_operation! {
    /// Creates an `AssertedArtifactSupport` (SACM 2.3 §11.17): an assertion that the
    /// artifacts its source `ArtifactReference`s name support the one its target names.
    ///
    /// - **Where.** The last child of an element whose class can hold one, such as an
    ///   `ArgumentPackage`.
    /// - **Rules.** Those of [`CreateAssertedInference`].
    /// - **Ends are content.** §11.17 requires its sources and target to be
    ///   `ArtifactReference`s. A create does not refuse another `ArgumentAsset`;
    ///   [`validation::validate`](crate::validation::validate) reports it as SACM-REF-002
    ///   (SACM23-VAL-016, D-VAL-LAYER).
    /// - **Validate before a strict save,** since the §11.17 rule is not refused here.
    /// - **Id.** Without [`with_id`](Self::with_id), it is generated:
    ///   `asserted_artifact_support_<n>`.
    ///
    /// # Examples
    ///
    /// ```
    /// use sacm::edit::{self, CreateArgumentPackage, CreateArtifactReference, CreateAssertedArtifactSupport};
    /// use sacm::model::{Document, LangString};
    ///
    /// let mut doc = Document::new();
    /// let name = |text: &str| LangString::new(text).with_lang("en");
    /// edit::apply(&mut doc, CreateArgumentPackage::root(name("Arguments")).with_id("p"))?;
    /// edit::apply(&mut doc, CreateArtifactReference::new("p", name("The test plan")).with_id("plan"))?;
    /// edit::apply(&mut doc, CreateArtifactReference::new("p", name("The test report")).with_id("report"))?;
    ///
    /// let support = CreateAssertedArtifactSupport::new("p", name("The plan supports the report"), ["plan"], "report");
    /// edit::apply(&mut doc, support.with_id("s1"))?;
    /// assert_eq!(doc.get("s1").and_then(|s1| s1.target()).map(|id| id.as_str()), Some("report"));
    /// # Ok::<(), sacm::edit::EditError>(())
    /// ```
    CreateAssertedArtifactSupport, AssertedArtifactSupport,
    "asserting that the artifacts the `ArtifactReference`s `sources` name support the one `target` names."
}

relationship_operation! {
    /// Creates an `AssertedArtifactContext` (SACM 2.3 §11.18): an assertion that the
    /// artifacts its source `ArtifactReference`s name are context for the one its target names.
    ///
    /// - **Where.** The last child of an element whose class can hold one, such as an
    ///   `ArgumentPackage`.
    /// - **Rules.** Those of [`CreateAssertedArtifactSupport`], with §11.18 in place of
    ///   §11.17.
    /// - **Validate before a strict save,** since the §11.18 rule is not refused here.
    /// - **Id.** Without [`with_id`](Self::with_id), it is generated:
    ///   `asserted_artifact_context_<n>`.
    ///
    /// # Examples
    ///
    /// ```
    /// use sacm::edit::{self, CreateArgumentPackage, CreateArtifactReference, CreateAssertedArtifactContext};
    /// use sacm::model::{Document, LangString};
    ///
    /// let mut doc = Document::new();
    /// let name = |text: &str| LangString::new(text).with_lang("en");
    /// edit::apply(&mut doc, CreateArgumentPackage::root(name("Arguments")).with_id("p"))?;
    /// edit::apply(&mut doc, CreateArtifactReference::new("p", name("The standard")).with_id("standard"))?;
    /// edit::apply(&mut doc, CreateArtifactReference::new("p", name("The test plan")).with_id("plan"))?;
    ///
    /// let context = CreateAssertedArtifactContext::new("p", name("The plan follows the standard"), ["standard"], "plan");
    /// let created = edit::apply(&mut doc, context)?;
    /// assert_eq!(created.created_ids()[0].as_str(), "asserted_artifact_context_1");
    /// # Ok::<(), sacm::edit::EditError>(())
    /// ```
    CreateAssertedArtifactContext, AssertedArtifactContext,
    "giving the artifacts the `ArtifactReference`s `sources` name as context for the one `target` names."
}

/// The plan of an element of `kind` named `name` under `parent`, or as a
/// root for `None`, with `flags` set and each reference end of `references`
/// holding its ids.
///
/// The checks, in order: [`checked_place`]'s four (ADR 0019); every end
/// holds at least the distinct ids it needs and at most the ids it may
/// ([`EditError::TooFewReferences`], ADR 0039 D3); then the name's
/// expression (check 5a); then
/// the referenced ids, by [`check_targets`]: white space, then existence,
/// then type, each over every end and its ids in order (ADR 0028), the
/// order a set operation checks them in too (ADR 0029). Changes nothing:
/// the commit adds the element, and then advances the counter for a
/// generated id.
pub(super) fn plan_referencing<'o>(
    document: &Document,
    kind: ElementKind,
    parent: Option<&'o ElementId>,
    id: Option<&ElementId>,
    name: &'o LangString,
    flags: &[(Flag, bool)],
    references: &[(ReferenceEnd, &'o [ElementId])],
) -> Result<Plan<'o>, EditError> {
    plan_linking(document, kind, (parent, id), name, flags, references, None)
}

/// [`plan_referencing`], for an element at `place`, its parent and its id,
/// one of whose ends of `references` also holds the cross-document
/// references `hrefs` (LIB-EDIT-020; ADR 0040 amendment 1, A5). The end's
/// bounds count them beside its ids, a lower bound the distinct values and
/// an upper bound every value (ADR 0040 D5); an `Href` is never checked
/// (D6).
pub(super) fn plan_linking<'o>(
    document: &Document,
    kind: ElementKind,
    place: (Option<&'o ElementId>, Option<&ElementId>),
    name: &'o LangString,
    flags: &[(Flag, bool)],
    references: &[(ReferenceEnd, &'o [ElementId])],
    hrefs: Option<(ReferenceEnd, &'o [Href])>,
) -> Result<Plan<'o>, EditError> {
    let (parent, id) = place;
    let (id, generated) = checked_place(document, kind, parent, id)?;
    for &(end, targets) in references {
        let beside = match hrefs {
            Some((linked, hrefs)) if linked == end => hrefs,
            _ => &[],
        };
        check_bounds(end, targets, beside)?;
    }
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
        parent,
        id,
        generated,
        features: Features::Referencing {
            name,
            flags: flags.to_vec(),
            references: references.to_vec(),
            hrefs,
        },
    }))
}
