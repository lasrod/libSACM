//! The create operations of the argumentation kinds that reference other
//! elements: `ArgumentReasoning`, `ArtifactReference`, `ArgumentGroup` and
//! the five asserted relationships (SACM23-ARG-005, SACM23-ARG-006,
//! SACM23-ARG-008 to SACM23-ARG-013, SACM23-VAL-004; LIB-EDIT-001, ADR 0019,
//! ADR 0027, ADR 0028).

use std::slice;

use super::EditError;
use super::create::{checked_place, refusal};
use crate::model::{Document, ElementId, ElementKind, Flag, LangString, ReferenceEnd};

/// Creates an `ArgumentReasoning` (SACM 2.3 §11.12) as the last child of an
/// element whose class can hold one, such as an `ArgumentPackage`: a
/// description of the reasoning behind an asserted relationship, which its
/// [`reasoning`](crate::model::SacmElement::reasoning) names.
///
/// With [`with_structure`](Self::with_structure) it names an
/// `ArgumentPackage` that gives the reasoning's detailed structure
/// (`structure:ArgumentPackage[0..1]`). Apply it with
/// [`edit::apply`](super::apply). After [ADR 0019]'s four checks, a
/// structure whose id holds XML white space, such as the `a b` of an
/// element read from a file, is [`EditError::InvalidReference`], and one
/// that names no element of the document is
/// [`EditError::ReferenceNotFound`]; then one that names an element that is
/// not an `ArgumentPackage` is [`EditError::WrongReferenceType`]
/// (SACM-REF-002, ADR 0028). Without [`with_id`](Self::with_id), the id is
/// generated: `argument_reasoning_<n>`.
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
pub struct CreateArgumentReasoning {
    parent: ElementId,
    id: Option<ElementId>,
    name: LangString,
    structure: Option<ElementId>,
}

impl CreateArgumentReasoning {
    /// Creates an `ArgumentReasoning` named `name` as the last child of
    /// `parent`, with no structure.
    #[must_use]
    pub fn new(parent: impl Into<ElementId>, name: LangString) -> Self {
        Self {
            parent: parent.into(),
            id: None,
            name,
            structure: None,
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

    /// Gives the new element the `structure` `structure`: the id of the
    /// `ArgumentPackage` that details the reasoning. It must name an
    /// `ArgumentPackage` of the document; otherwise [`apply`](super::apply)
    /// refuses with
    /// [`EditError::ReferenceNotFound`](super::EditError::ReferenceNotFound)
    /// or
    /// [`EditError::WrongReferenceType`](super::EditError::WrongReferenceType).
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

    /// Checks and applies the operation, and returns the ids it created.
    pub(super) fn apply(&self, document: &mut Document) -> Result<Vec<ElementId>, EditError> {
        create_referencing(
            document,
            ElementKind::ArgumentReasoning,
            &self.parent,
            self.id.as_ref(),
            &self.name,
            &[],
            &[(ReferenceEnd::Structure, self.structure.as_slice(), 0)],
        )
    }
}

/// Creates an `ArtifactReference` (SACM 2.3 §11.9) as the last child of an
/// element whose class can hold one, such as an `ArgumentPackage`: an
/// argument's reference to artifacts, or to other elements, through its
/// [`referenced_artifact_elements`](crate::model::SacmElement::referenced_artifact_elements)
/// (`referencedArtifactElement:Base::ArtifactElement[0..*]`).
///
/// An `ArtifactReference` is an `ArgumentAsset`, so it can be the source of
/// an `AssertedEvidence` (§11.15), or either end of an
/// `AssertedArtifactSupport` or `AssertedArtifactContext` (§11.17, §11.18).
/// It is no Assertion: it has no `assertionDeclaration` and no `metaClaim`.
/// Artifacts themselves (§12, the `ArtifactPackage`) are not supported yet,
/// so for now an `ArtifactReference` can name only elements of argument and
/// assurance-case packages, or nothing.
///
/// With
/// [`with_referenced_artifact_elements`](Self::with_referenced_artifact_elements)
/// it names the elements it refers to: none is required, and the ids are
/// kept in the order given, repeats included (validation warns about a
/// repeat, SACM23-VAL-022). Every kind except the `UtilityElement`s
/// (`Description`, `ImplementationConstraint`, `Note` and `TaggedValue`)
/// conforms to `ArtifactElement`, so a Claim or a package is a well-typed
/// target, as when an argument cites a Claim of another argument package
/// (§11.1). Apply it with
/// [`edit::apply`](super::apply). After [ADR 0019]'s four checks, an id that
/// holds XML white space is [`EditError::InvalidReference`], then one that
/// names no element is [`EditError::ReferenceNotFound`], then one that names
/// a `UtilityElement`, which is no `ArtifactElement`, is
/// [`EditError::WrongReferenceType`] (SACM-REF-002, ADR 0028). Without
/// [`with_id`](Self::with_id), the id is generated:
/// `artifact_reference_<n>`.
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
pub struct CreateArtifactReference {
    parent: ElementId,
    id: Option<ElementId>,
    name: LangString,
    referenced_artifact_elements: Vec<ElementId>,
}

impl CreateArtifactReference {
    /// Creates an `ArtifactReference` named `name` as the last child of
    /// `parent`, referring to nothing.
    #[must_use]
    pub fn new(parent: impl Into<ElementId>, name: LangString) -> Self {
        Self {
            parent: parent.into(),
            id: None,
            name,
            referenced_artifact_elements: Vec::new(),
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

    /// Gives the new element the `referencedArtifactElement` ids `ids`, in
    /// the order given, repeats included, replacing any given before. Each
    /// must name an `ArtifactElement` of the document (any element but a
    /// `UtilityElement`); otherwise [`apply`](super::apply) refuses with
    /// [`EditError::ReferenceNotFound`](super::EditError::ReferenceNotFound)
    /// or
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

    /// The `referencedArtifactElement` ids given with
    /// [`with_referenced_artifact_elements`](Self::with_referenced_artifact_elements),
    /// in the order given: empty unless given.
    #[must_use]
    pub fn referenced_artifact_elements(&self) -> &[ElementId] {
        &self.referenced_artifact_elements
    }

    /// Checks and applies the operation, and returns the ids it created.
    pub(super) fn apply(&self, document: &mut Document) -> Result<Vec<ElementId>, EditError> {
        create_referencing(
            document,
            ElementKind::ArtifactReference,
            &self.parent,
            self.id.as_ref(),
            &self.name,
            &[],
            &[(
                ReferenceEnd::ReferencedArtifactElement,
                &self.referenced_artifact_elements,
                0,
            )],
        )
    }
}

/// Creates an `ArgumentGroup` (SACM 2.3 §11.2) as the last child of an
/// element whose class can hold one, such as an `ArgumentPackage`: a named
/// grouping of argumentation elements, its
/// [`group_members`](crate::model::SacmElement::group_members)
/// (`argumentElement:ArgumentationElement[0..*]`).
///
/// The members are references, not children: a member stays where it is
/// contained, and `ArgumentGroups` "serve no structural purpose in the
/// formation of the argument network" (§11.2). With
/// [`with_group_members`](Self::with_group_members) it names its members:
/// none is required, and the ids are kept in the order given, repeats
/// included. Apply it with [`edit::apply`](super::apply). After
/// [ADR 0019]'s four checks, an id that holds XML white space is
/// [`EditError::InvalidReference`], then one that names no element is
/// [`EditError::ReferenceNotFound`], then one that names an element that is
/// no `ArgumentationElement`, such as an `AssuranceCasePackage` or a Note,
/// is [`EditError::WrongReferenceType`] (SACM-REF-002, ADR 0028). Without
/// [`with_id`](Self::with_id), the id is generated: `argument_group_<n>`.
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
pub struct CreateArgumentGroup {
    parent: ElementId,
    id: Option<ElementId>,
    name: LangString,
    group_members: Vec<ElementId>,
}

impl CreateArgumentGroup {
    /// Creates an `ArgumentGroup` named `name` as the last child of
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

    /// Gives the new group the members `ids`, its `argumentElement` ids, in
    /// the order given, repeats included, replacing any given before. Each
    /// must name an `ArgumentationElement` of the document; otherwise
    /// [`apply`](super::apply) refuses with
    /// [`EditError::ReferenceNotFound`](super::EditError::ReferenceNotFound)
    /// or
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

    /// Checks and applies the operation, and returns the ids it created.
    pub(super) fn apply(&self, document: &mut Document) -> Result<Vec<ElementId>, EditError> {
        create_referencing(
            document,
            ElementKind::ArgumentGroup,
            &self.parent,
            self.id.as_ref(),
            &self.name,
            &[],
            &[(ReferenceEnd::GroupMember, &self.group_members, 0)],
        )
    }
}

/// What the create operations of the asserted relationships are given
/// (SACM 2.3 §11.13), shared by all five.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Relationship {
    parent: ElementId,
    id: Option<ElementId>,
    name: LangString,
    sources: Vec<ElementId>,
    target: ElementId,
    reasoning: Option<ElementId>,
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

    /// Checks and applies the create of a relationship of `kind`, and
    /// returns the ids it created: after [ADR 0019]'s four checks, at least
    /// one source, then every source, in order, the target and the
    /// reasoning holding no white space, then each naming an element, then
    /// each naming an element of its end's declared type.
    ///
    /// [ADR 0019]: https://github.com/lasrod/libSACM/blob/main/docs/adr/0019-edit-operations.md
    fn apply(
        &self,
        document: &mut Document,
        kind: ElementKind,
    ) -> Result<Vec<ElementId>, EditError> {
        create_referencing(
            document,
            kind,
            &self.parent,
            self.id.as_ref(),
            &self.name,
            &[(Flag::IsCounter, self.is_counter)],
            &[
                (ReferenceEnd::Source, &self.sources, 1),
                (ReferenceEnd::Target, slice::from_ref(&self.target), 1),
                (ReferenceEnd::Reasoning, self.reasoning.as_slice(), 0),
            ],
        )
    }
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
                name: LangString,
                sources: impl IntoIterator<Item = impl Into<ElementId>>,
                target: impl Into<ElementId>,
            ) -> Self {
                Self {
                    relationship: Relationship::new(parent, name, sources, target),
                }
            }

            /// Gives the new element the id `id` instead of a generated one.
            /// It must be an XML `NCName` and not already in the document.
            /// Otherwise [`apply`](super::apply) refuses with
            /// [`EditError::InvalidId`](super::EditError::InvalidId) or
            /// [`EditError::DuplicateId`](super::EditError::DuplicateId).
            #[must_use]
            pub fn with_id(mut self, id: impl Into<ElementId>) -> Self {
                self.relationship.id = Some(id.into());
                self
            }

            /// Gives the new element the `reasoning` `reasoning`: the id of
            /// the `ArgumentReasoning` that describes the reasoning behind
            /// it. It must name an `ArgumentReasoning` of the document;
            /// otherwise [`apply`](super::apply) refuses with
            /// [`EditError::ReferenceNotFound`](super::EditError::ReferenceNotFound)
            /// or
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

            /// Checks and applies the operation, and returns the ids it
            /// created.
            pub(super) fn apply(
                &self,
                document: &mut Document,
            ) -> Result<Vec<ElementId>, EditError> {
                self.relationship.apply(document, ElementKind::$kind)
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
    /// Creates an `AssertedInference` (SACM 2.3 §11.14) as the last child of
    /// an element whose class can hold one, such as an `ArgumentPackage`: an
    /// assertion that its sources, the premises, support its target, the
    /// conclusion.
    ///
    /// It takes the sources (`source:ArgumentAsset[1..*]`) and the target
    /// (`target:ArgumentAsset[1]`), and optionally a
    /// [`reasoning`](Self::with_reasoning) and
    /// [`isCounter`](Self::with_is_counter). Its `assertionDeclaration` is
    /// `asserted`, the default (§11.10). Apply it with
    /// [`edit::apply`](super::apply). After [ADR 0019]'s four checks it
    /// refuses no source as [`EditError::TooFewReferences`], then a source
    /// (in order), the target or the reasoning whose id holds XML white space
    /// as [`EditError::InvalidReference`], since it could not be written as
    /// one id, then one that names no element of the document as
    /// [`EditError::ReferenceNotFound`], so an edit never makes a
    /// dangling reference (D-VAL-LAYER). The id of a name, `content`, `key`
    /// or `value` names no element. Then, in the same order, an id that
    /// names an element of a kind its end does not admit (an
    /// `ArgumentAsset` for a source or the target, an `ArgumentReasoning`
    /// for the reasoning) is [`EditError::WrongReferenceType`]
    /// (SACM-REF-002, ADR 0028), so an edit never makes a wrongly typed
    /// reference either. A repeated source is kept (ADR 0027), and
    /// validation warns about it (SACM23-VAL-022). Without
    /// [`with_id`](Self::with_id), the id is generated:
    /// `asserted_inference_<n>`.
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
    /// Creates an `AssertedContext` (SACM 2.3 §11.16) as the last child of
    /// an element whose class can hold one, such as an `ArgumentPackage`: an
    /// assertion that its sources give the context of its target.
    ///
    /// It follows the rules of [`CreateAssertedInference`]. Without
    /// [`with_id`](Self::with_id), the id is generated:
    /// `asserted_context_<n>`.
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
    /// Creates an `AssertedEvidence` (SACM 2.3 §11.15) as the last child of
    /// an element whose class can hold one, such as an `ArgumentPackage`: an
    /// assertion that its sources, the evidence, support its target, such as
    /// a Claim. With [`isCounter`](Self::with_is_counter) true it is
    /// counter-evidence (§11.13).
    ///
    /// It follows the rules of [`CreateAssertedInference`]. §11.15 requires
    /// each source to be an `ArtifactReference`; that is content, so a
    /// create does not refuse another `ArgumentAsset`, such as a Claim, and
    /// [`validation::validate`](crate::validation::validate) reports it as
    /// SACM-REF-002 (SACM23-VAL-016, D-VAL-LAYER). Its target is not
    /// narrowed. The §11.15 rule is not refused here, so validate before a
    /// strict save. Without [`with_id`](Self::with_id), the id is generated:
    /// `asserted_evidence_<n>`.
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
    /// Creates an `AssertedArtifactSupport` (SACM 2.3 §11.17) as the last
    /// child of an element whose class can hold one, such as an
    /// `ArgumentPackage`: an assertion that the artifacts its source
    /// `ArtifactReference`s name support the one its target names.
    ///
    /// It follows the rules of [`CreateAssertedInference`]. §11.17 requires
    /// its sources and target to be `ArtifactReference`s; that is content,
    /// so a create does not refuse another `ArgumentAsset`, and
    /// [`validation::validate`](crate::validation::validate) reports it as
    /// SACM-REF-002 (SACM23-VAL-016, D-VAL-LAYER). The §11.17 rule is not
    /// refused here, so validate before a strict save. Without
    /// [`with_id`](Self::with_id), the id is generated:
    /// `asserted_artifact_support_<n>`.
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
    /// Creates an `AssertedArtifactContext` (SACM 2.3 §11.18) as the last
    /// child of an element whose class can hold one, such as an
    /// `ArgumentPackage`: an assertion that the artifacts its source
    /// `ArtifactReference`s name are context for the one its target names.
    ///
    /// It follows the rules of [`CreateAssertedArtifactSupport`], with
    /// §11.18 in place of §11.17: the §11.18 rule is not refused here, so
    /// validate before a strict save. Without [`with_id`](Self::with_id),
    /// the id is generated: `asserted_artifact_context_<n>`.
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

/// Creates an element of `kind` named `name` under `parent`, with `flags`
/// set and each reference end of `references` holding its ids, and returns
/// the ids created: the new element's. Each end comes with the fewest ids it
/// needs.
///
/// The checks, in order: [`checked_place`]'s four (ADR 0019); every end
/// holds at least the ids it needs ([`EditError::TooFewReferences`]); every
/// id, end by end and in order, holds no XML white space
/// ([`EditError::InvalidReference`]); every id, in the same order, names an
/// element of the document ([`EditError::ReferenceNotFound`]); every id, in
/// the same order, names an element of a kind its end's declared type admits
/// ([`EditError::WrongReferenceType`], ADR 0028), so every id is checked for
/// existence before any is checked for type. On a refusal the document, its
/// counters included, is unchanged: every check runs before the one change.
fn create_referencing(
    document: &mut Document,
    kind: ElementKind,
    parent: &ElementId,
    id: Option<&ElementId>,
    name: &LangString,
    flags: &[(Flag, bool)],
    references: &[(ReferenceEnd, &[ElementId], usize)],
) -> Result<Vec<ElementId>, EditError> {
    let (id, generated) = checked_place(document, kind, Some(parent), id)?;
    for &(end, targets, required) in references {
        if targets.len() < required {
            return Err(EditError::TooFewReferences {
                end: end.name(),
                given: targets.len(),
                required,
            });
        }
    }
    // An id with XML white space in it would be written as an IDREFS
    // attribute of several ids, so it cannot be referenced.
    for &(end, targets, _) in references {
        if let Some(spaced) = targets
            .iter()
            .find(|target| target.as_str().contains([' ', '\t', '\n', '\r']))
        {
            return Err(EditError::InvalidReference {
                end: end.name(),
                id: spaced.clone(),
            });
        }
    }
    for &(end, targets, _) in references {
        if let Some(missing) = targets.iter().find(|target| !document.contains(*target)) {
            return Err(EditError::ReferenceNotFound {
                end: end.name(),
                id: missing.clone(),
            });
        }
    }
    for &(end, targets, _) in references {
        let declared = end.target();
        let wrong = targets.iter().find_map(|target| {
            let kind = document.get(target)?.kind();
            (!declared.admits(kind)).then_some((target, kind))
        });
        if let Some((target, kind)) = wrong {
            return Err(EditError::WrongReferenceType {
                end: end.name(),
                id: target.clone(),
                kind,
                expected: declared.name(),
            });
        }
    }
    let held = references
        .iter()
        .filter(|(_, targets, _)| !targets.is_empty())
        .map(|&(end, targets, _)| (end, targets.to_vec()))
        .collect();
    document
        .insert_with_references(
            Some(parent),
            kind,
            id.clone(),
            Some(name.clone()),
            flags,
            held,
        )
        .map_err(|error| refusal(error, &id))?;
    if let Some(n) = generated {
        document.commit_generated_id(kind, n);
    }
    Ok(vec![id])
}
