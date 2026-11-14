//! The create operations of the argumentation kinds that reference other
//! elements: `ArgumentReasoning`, `AssertedInference` and `AssertedContext`
//! (SACM23-ARG-005, SACM23-ARG-008, SACM23-ARG-010; LIB-EDIT-001, ADR 0019,
//! ADR 0027).

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
/// [`EditError::ReferenceNotFound`]. Whether the structure is an
/// `ArgumentPackage` is not checked yet, here or by validation
/// (SACM23-VAL-004). Without
/// [`with_id`](Self::with_id), the id is generated: `argument_reasoning_<n>`.
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
    /// `ArgumentPackage` that details the reasoning. It must name an element
    /// of the document; otherwise [`apply`](super::apply) refuses with
    /// [`EditError::ReferenceNotFound`](super::EditError::ReferenceNotFound).
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

/// What the create operations of the asserted relationships are given
/// (SACM 2.3 §11.13), shared by both.
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
    /// reasoning holding no white space, then each naming an element.
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
            /// it. It must name an element of the document; otherwise
            /// [`apply`](super::apply) refuses with
            /// [`EditError::ReferenceNotFound`](super::EditError::ReferenceNotFound).
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
    /// or `value` names no element. A repeated source is kept (ADR 0027),
    /// and whether each end has the type SACM declares is not checked yet,
    /// here or by validation (SACM23-VAL-004). Without [`with_id`](Self::with_id), the id is
    /// generated: `asserted_inference_<n>`.
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

/// Creates an element of `kind` named `name` under `parent`, with `flags`
/// set and each reference end of `references` holding its ids, and returns
/// the ids created: the new element's. Each end comes with the fewest ids it
/// needs.
///
/// The checks, in order: [`checked_place`]'s four (ADR 0019); every end
/// holds at least the ids it needs ([`EditError::TooFewReferences`]); every
/// id, end by end and in order, holds no XML white space
/// ([`EditError::InvalidReference`]); every id, in the same order, names an
/// element of the document ([`EditError::ReferenceNotFound`]). On a refusal the document, its
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
