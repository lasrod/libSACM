//! Editing: operations as values, applied to a document with [`apply`].
//!
//! An operation is a plain value that says what to change. [`apply`] checks
//! it completely against the document and then either makes the change and
//! returns a [`Mutation`] (the operation, and the ids it created, changed and
//! deleted), or refuses with an [`EditError`]. **A refused operation changes
//! nothing**: not the elements, not the written XMI, and not the ids
//! generated next.
//!
//! # Operations
//!
//! | Operation | Creates | Root | Under a parent |
//! |---|---|---|---|
//! | [`CreateAssuranceCasePackage`] | an `AssuranceCasePackage` (§9.2) | [`root`](CreateAssuranceCasePackage::root) | [`new`](CreateAssuranceCasePackage::new) |
//! | [`CreateArgumentPackage`] | an `ArgumentPackage` (§11.4) | [`root`](CreateArgumentPackage::root) | [`new`](CreateArgumentPackage::new) |
//! | [`CreateClaim`] | a `Claim` (§11.11) | no | [`new`](CreateClaim::new) |
//! | [`CreateArgumentReasoning`] | an `ArgumentReasoning` (§11.12), with an optional `structure` | no | [`new`](CreateArgumentReasoning::new) |
//! | [`CreateAssertedInference`] | an `AssertedInference` (§11.14), with its sources and target, and an optional `reasoning` and `isCounter` | no | [`new`](CreateAssertedInference::new) |
//! | [`CreateAssertedContext`] | an `AssertedContext` (§11.16), with its sources and target, and an optional `reasoning` and `isCounter` | no | [`new`](CreateAssertedContext::new) |
//! | [`CreateDescription`] | a `Description` (§8.9) | no | [`new`](CreateDescription::new) |
//! | [`CreateImplementationConstraint`] | an `ImplementationConstraint` (§8.8) | no | [`new`](CreateImplementationConstraint::new) |
//! | [`CreateNote`] | a `Note` (§8.11) | no | [`new`](CreateNote::new) |
//! | [`CreateTaggedValue`] | a `TaggedValue` (§8.12) | no | [`new`](CreateTaggedValue::new) |
//!
//! A root is added after the document's last root; a child after its parent's
//! last child. A parent is legal exactly when SACM lets its class contain the
//! new element (it has a composite role for that kind), the same rules
//! [`xmi::read`](crate::xmi::read) uses. So a Claim goes in an
//! ArgumentPackage, not directly in an AssuranceCasePackage. An
//! ArtifactPackage, a package interface or a package binding is also a legal
//! parent, though this release cannot create or read those yet.
//!
//! **The name is required**, because SACM gives every `ModelElement` exactly
//! one name (§8.6 `name:LangString[1]`). Every operation that creates one
//! takes it as a [`LangString`](crate::model::LangString), stored exactly as
//! given. Any value is accepted, `LangString::default()` included; an empty
//! name is still a name.
//!
//! The four `UtilityElement`s (descriptions, implementation constraints,
//! notes and tagged values) have no name. They go in any `ModelElement`, in
//! the role of their kind, and take their `content` (and a `TaggedValue` its
//! `key`) as a [`MultiLangString`](crate::model::MultiLangString), stored
//! exactly as given; a `LangString` converts into one with one value. A
//! `UtilityElement` contains no elements, so it is never a parent.
//!
//! An `ArgumentReasoning` and the asserted relationships reference other
//! elements (ADR 0027). An asserted relationship takes its sources (at least
//! one, in order, repeats kept) and its target when it is created, as SACM
//! requires (§11.13 `source:ArgumentAsset[1..*]`, `target:ArgumentAsset[1]`).
//! Every id they reference must name an element of the document, so an edit
//! never makes a dangling reference; the id of a name, `content`, `key` or
//! `value` names no element. Whether each referenced element has the type
//! SACM declares (an `ArgumentAsset` for a source or the target, an
//! `ArgumentReasoning` for the reasoning, an `ArgumentPackage` for the
//! structure) is not checked yet, by edits or by validation, so a strict
//! save does not refuse a wrong one (SACM23-VAL-004).
//! Setting the references of an existing element, and the `metaClaim` of an
//! Assertion, come later (LIB-EDIT-009).
//!
//! # Ids
//!
//! - **Given by the caller** with `with_id`: it must be an XML `NCName` (an
//!   `xmi:id` is an `xsd:ID`, XMI 2.5.1 §7.6.1), such as `claim_top` or
//!   `_c.1`, and not already in the document.
//! - **Generated** otherwise: `<snake_case kind>_<n>`, such as
//!   `assurance_case_package_1`, `argument_package_1` or `claim_1`. `n` is the
//!   smallest number above the document's counter for the kind whose id is not
//!   already in use, whether a caller gave it or it was read from a file, and
//!   that no reference names, so that a reference naming no element never
//!   comes to name the new one by accident (ADR 0026). The
//!   counter advances only when an operation succeeds with a generated id; a
//!   caller id does not move it.
//! - **Deterministic:** the same operations, applied to documents built or read
//!   the same way, give the same ids; a clone generates the same ids as its
//!   original.
//!
//! Ids read from a file are kept exactly as read; only new ids are checked.
//!
//! # Refusals
//!
//! The checks run in this order, and the first that fails is the result. A
//! root operation has no parent, so it skips the first two.
//!
//! | Check | Refusal | Code | Fix |
//! |---|---|---|---|
//! | 1. the parent is in the document | [`EditError::ParentNotFound`] | SACM-CMD-001 | Create the parent first, or correct its id. Ids are compared exactly, case included. |
//! | 2. the parent can contain the new kind | [`EditError::InvalidParent`] | SACM-CMD-005 | Choose a parent whose class can hold the new kind. For example, a Claim goes in an ArgumentPackage, and an ArgumentPackage in an AssuranceCasePackage or another ArgumentPackage. |
//! | 3. a caller id is an `NCName` | [`EditError::InvalidId`] | SACM-ID-002 | An `NCName` starts with a letter or `_` and continues with letters, digits, `-`, `.` or `_`; it has no spaces and no `:`. Or leave out `with_id` to have an id generated. |
//! | 4. a caller id is not in use | [`EditError::DuplicateId`] | SACM-CMD-004 | Ids are unique across the whole document, not per parent. Choose another id, or leave out `with_id` to have one generated. |
//! | 5. an asserted relationship has a source | [`EditError::TooFewReferences`] | SACM-MULT-001 | Give at least one source: SACM requires one (`source:ArgumentAsset[1..*]`). |
//! | 6. no referenced id holds XML white space (space, TAB, LF, CR): each source in order, then the target, the `reasoning` and the `structure` | [`EditError::InvalidReference`] | SACM-ID-002 | An id such as `a b`, read from a file as written, is not an `NCName` and would be written as two ids. Give the referenced element an `NCName` id in the file. |
//! | 7. every referenced id names an element, in the same order | [`EditError::ReferenceNotFound`] | SACM-CMD-001 | Create the referenced element first, or correct its id. Ids are compared exactly, case included. |
//!
//! # Writing and reading back
//!
//! A document built with [`apply`] validates with
//! [`validation::validate`](crate::validation::validate) and writes with
//! [`xmi::write`](crate::xmi::write), and what is written reads back with
//! [`xmi::read`](crate::xmi::read) to the same ids, kinds, names, parents and
//! order.
//!
//! # Examples
//!
//! ```
//! use sacm::diagnostic::Code;
//! use sacm::edit::{self, CreateArgumentPackage, CreateAssuranceCasePackage, CreateClaim};
//! use sacm::model::{Document, LangString};
//! use sacm::validation;
//! use sacm::xmi::{self, WriteOptions};
//!
//! let name = |text: &str| LangString::new(text).with_lang("en");
//! let mut doc = Document::new();
//!
//! edit::apply(&mut doc, CreateAssuranceCasePackage::root(name("Safety case")).with_id("case"))?;
//! // No with_id: the id is generated. Use it as the next parent.
//! let created = edit::apply(&mut doc, CreateArgumentPackage::new("case", name("Arguments")))?;
//! let arguments = created.created_ids()[0].clone();
//! assert_eq!(arguments.as_str(), "argument_package_1");
//! edit::apply(&mut doc, CreateClaim::new(&arguments, name("The system is safe")).with_id("c_top"))?;
//!
//! // A Claim cannot sit directly in an AssuranceCasePackage.
//! let refused = edit::apply(&mut doc, CreateClaim::new("case", name("Misplaced"))).unwrap_err();
//! assert_eq!(refused.code(), Code::CMD_INVALID_PARENT);
//! assert_eq!(doc.element_count(), 3);
//!
//! // Every element has a name, so validation finds nothing.
//! assert!(validation::validate(&doc).is_empty());
//! let text = xmi::write(&doc, &WriteOptions::default())?;
//! assert!(text.contains(r#"xmi:id="c_top""#));
//!
//! // And it reads back.
//! let back = xmi::read(text.as_bytes(), &xmi::ReadOptions::default())?.into_document();
//! let parent = back.get("c_top").and_then(|claim| claim.parent());
//! assert_eq!(parent.map(|parent| parent.id().as_str()), Some("argument_package_1"));
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod argumentation;
mod create;
mod utility;

use std::fmt;

pub use argumentation::{CreateArgumentReasoning, CreateAssertedContext, CreateAssertedInference};
pub use create::{CreateArgumentPackage, CreateAssuranceCasePackage, CreateClaim};
pub use utility::{
    CreateDescription, CreateImplementationConstraint, CreateNote, CreateTaggedValue,
};

use crate::diagnostic::Code;
use crate::model::{Document, ElementId, ElementKind};

/// Checks `operation` against `document` and applies it, or refuses it and
/// leaves `document` unchanged.
///
/// `apply` is the only public way to change a [`Document`] (outside the
/// unstable `test-support` feature), and it is all or nothing:
/// - **On success** the document stays structurally valid, as every
///   `Document` is: ids are unique, parent and child links agree, every root
///   is a package, every child is in a composite role of its parent's class,
///   and every Assertion has an
///   [`AssertionDeclaration`](crate::model::AssertionDeclaration) (SACM 2.3
///   §11.10). The returned [`Mutation`] holds the operation and the ids it created,
///   changed and deleted.
/// - **On refusal** the document is unchanged: it is `==` to what it was
///   before (see [`compare`](crate::compare)), and it generates the same next
///   ids.
///
/// `apply` neither checks content findings nor refuses an operation because
/// of one; call [`validate`](crate::validation::validate) again after
/// editing.
///
/// Takes any operation struct, such as [`CreateClaim`], or an [`Operation`].
/// See the [module docs](self) for the operations, the ids and the order of
/// the checks.
///
/// # Errors
///
/// Refuses, checked in this order, when:
/// - the parent is not in the document ([`EditError::ParentNotFound`]);
/// - the parent's class cannot contain the new element's kind
///   ([`EditError::InvalidParent`]);
/// - the id given with `with_id` is not an XML `NCName`
///   ([`EditError::InvalidId`]);
/// - the id given with `with_id` is already in the document, as an
///   element's id or as the `xmi:id` of a name, a `content`, a `key` or a
///   `value` ([`EditError::DuplicateId`]);
/// - an asserted relationship is given no source
///   ([`EditError::TooFewReferences`]);
/// - a referenced id holds XML white space, so it cannot be written as one
///   id ([`EditError::InvalidReference`]);
/// - a referenced id names no element of the document
///   ([`EditError::ReferenceNotFound`]).
///
/// # Examples
///
/// ```
/// use sacm::edit::{self, CreateArgumentPackage, EditError};
/// use sacm::model::{Document, ElementId, LangString};
///
/// let mut doc = Document::new();
/// let mutation = edit::apply(&mut doc, CreateArgumentPackage::root(LangString::new("Arguments")))?;
/// assert_eq!(mutation.created_ids(), [ElementId::from("argument_package_1")]);
///
/// let again = CreateArgumentPackage::root(LangString::new("Again")).with_id("argument_package_1");
/// let before = doc.clone();
/// let error = edit::apply(&mut doc, again).unwrap_err();
/// assert!(matches!(error, EditError::DuplicateId { .. }));
/// assert_eq!(doc, before); // unchanged
/// # Ok::<(), EditError>(())
/// ```
pub fn apply(
    document: &mut Document,
    operation: impl Into<Operation>,
) -> Result<Mutation, EditError> {
    let operation = operation.into();
    // Each operation checks everything before it changes the document, so a
    // refusal here leaves it untouched.
    let created = match &operation {
        Operation::CreateAssuranceCasePackage(create) => create.apply(document)?,
        Operation::CreateArgumentPackage(create) => create.apply(document)?,
        Operation::CreateClaim(create) => create.apply(document)?,
        Operation::CreateArgumentReasoning(create) => create.apply(document)?,
        Operation::CreateAssertedInference(create) => create.apply(document)?,
        Operation::CreateAssertedContext(create) => create.apply(document)?,
        Operation::CreateDescription(create) => create.apply(document)?,
        Operation::CreateImplementationConstraint(create) => create.apply(document)?,
        Operation::CreateNote(create) => create.apply(document)?,
        Operation::CreateTaggedValue(create) => create.apply(document)?,
    };
    Ok(Mutation {
        operation,
        created,
        // A create changes no existing element and deletes none (ADR 0019
        // amendment 1).
        changed: Vec::new(),
        deleted: Vec::new(),
    })
}

/// An edit operation, as a value: any of the operation structs.
///
/// [`apply`] takes an `Operation` or any operation struct, which converts
/// into one with `From`. Later releases add operations, so a `match` on
/// `Operation` needs a wildcard arm.
///
/// # Examples
///
/// ```
/// use sacm::edit::{CreateClaim, Operation};
/// use sacm::model::LangString;
///
/// let operation = Operation::from(CreateClaim::new("arguments", LangString::new("C1")));
/// assert!(matches!(operation, Operation::CreateClaim(_)));
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Operation {
    /// Creates an `AssuranceCasePackage`.
    CreateAssuranceCasePackage(CreateAssuranceCasePackage),
    /// Creates an `ArgumentPackage`.
    CreateArgumentPackage(CreateArgumentPackage),
    /// Creates a `Claim`.
    CreateClaim(CreateClaim),
    /// Creates an `ArgumentReasoning`.
    CreateArgumentReasoning(CreateArgumentReasoning),
    /// Creates an `AssertedInference`.
    CreateAssertedInference(CreateAssertedInference),
    /// Creates an `AssertedContext`.
    CreateAssertedContext(CreateAssertedContext),
    /// Creates a `Description`.
    CreateDescription(CreateDescription),
    /// Creates an `ImplementationConstraint`.
    CreateImplementationConstraint(CreateImplementationConstraint),
    /// Creates a `Note`.
    CreateNote(CreateNote),
    /// Creates a `TaggedValue`.
    CreateTaggedValue(CreateTaggedValue),
}

impl Operation {
    /// The operation's stable name, for logs: the name of its struct, such as
    /// `"CreateClaim"`.
    ///
    /// The names are stable, like a diagnostic [`Code`]: a name never changes
    /// and is never given to another operation. This release has
    /// `"CreateAssuranceCasePackage"`, `"CreateArgumentPackage"`,
    /// `"CreateClaim"`, `"CreateArgumentReasoning"`,
    /// `"CreateAssertedInference"`, `"CreateAssertedContext"`,
    /// `"CreateDescription"`, `"CreateImplementationConstraint"`,
    /// `"CreateNote"` and `"CreateTaggedValue"`.
    ///
    /// # Examples
    ///
    /// ```
    /// use sacm::edit::{CreateClaim, Operation};
    /// use sacm::model::LangString;
    ///
    /// let operation = Operation::from(CreateClaim::new("arguments", LangString::new("C1")));
    /// assert_eq!(operation.operation_name(), "CreateClaim");
    /// ```
    #[must_use]
    pub fn operation_name(&self) -> &'static str {
        match self {
            Self::CreateAssuranceCasePackage(_) => "CreateAssuranceCasePackage",
            Self::CreateArgumentPackage(_) => "CreateArgumentPackage",
            Self::CreateClaim(_) => "CreateClaim",
            Self::CreateArgumentReasoning(_) => "CreateArgumentReasoning",
            Self::CreateAssertedInference(_) => "CreateAssertedInference",
            Self::CreateAssertedContext(_) => "CreateAssertedContext",
            Self::CreateDescription(_) => "CreateDescription",
            Self::CreateImplementationConstraint(_) => "CreateImplementationConstraint",
            Self::CreateNote(_) => "CreateNote",
            Self::CreateTaggedValue(_) => "CreateTaggedValue",
        }
    }
}

impl From<CreateAssuranceCasePackage> for Operation {
    fn from(operation: CreateAssuranceCasePackage) -> Self {
        Self::CreateAssuranceCasePackage(operation)
    }
}

impl From<CreateArgumentPackage> for Operation {
    fn from(operation: CreateArgumentPackage) -> Self {
        Self::CreateArgumentPackage(operation)
    }
}

impl From<CreateClaim> for Operation {
    fn from(operation: CreateClaim) -> Self {
        Self::CreateClaim(operation)
    }
}

impl From<CreateArgumentReasoning> for Operation {
    fn from(operation: CreateArgumentReasoning) -> Self {
        Self::CreateArgumentReasoning(operation)
    }
}

impl From<CreateDescription> for Operation {
    fn from(operation: CreateDescription) -> Self {
        Self::CreateDescription(operation)
    }
}

impl From<CreateImplementationConstraint> for Operation {
    fn from(operation: CreateImplementationConstraint) -> Self {
        Self::CreateImplementationConstraint(operation)
    }
}

impl From<CreateNote> for Operation {
    fn from(operation: CreateNote) -> Self {
        Self::CreateNote(operation)
    }
}

impl From<CreateTaggedValue> for Operation {
    fn from(operation: CreateTaggedValue) -> Self {
        Self::CreateTaggedValue(operation)
    }
}

/// What an applied operation did: the operation, and the ids it created,
/// changed and deleted.
///
/// [`apply`] returns one for every operation it applies. The three lists are
/// what [`semantic_compare`](crate::compare::semantic_compare) of the
/// document before and after would report; `apply` does not run it (ADR 0019
/// amendment 1):
/// - [`created_ids`](Self::created_ids) are in the document after, not
///   before;
/// - [`changed_ids`](Self::changed_ids) are in both, with a difference in the
///   element itself;
/// - [`deleted_ids`](Self::deleted_ids) are in the document before, not
///   after.
///
/// Adding a child does not change its parent. The new element is reported as
/// created, and the parent's name, declaration and the order of its existing
/// children are as before (`semantic_compare` compares the order of the
/// children both documents hold, so a new child is not a reordering). The
/// parent is in [`operation`](Self::operation). So in this release, where
/// every operation is a create, `changed_ids` and `deleted_ids` are always
/// empty.
///
/// A refused operation returns an [`EditError`] instead, and the document is
/// unchanged; log it with [`EditError::code`]. To log the refused operation's
/// name, keep a clone of the operation, since `apply` takes it by value.
///
/// # Examples
///
/// An audit-log line:
///
/// ```
/// use sacm::edit::{self, CreateArgumentPackage, CreateClaim};
/// use sacm::model::{Document, ElementId, LangString};
///
/// let mut doc = Document::new();
/// let arguments = CreateArgumentPackage::root(LangString::new("Arguments")).with_id("arguments");
/// edit::apply(&mut doc, arguments)?;
/// // No with_id: the claim's id is generated.
/// let claim = CreateClaim::new("arguments", LangString::new("The system is safe"));
/// let mutation = edit::apply(&mut doc, claim)?;
///
/// let ids = |ids: &[ElementId]| ids.iter().map(ElementId::as_str).collect::<Vec<_>>().join(",");
/// let line = format!(
///     "{} created=[{}] changed=[{}] deleted=[{}]",
///     mutation.operation().operation_name(),
///     ids(mutation.created_ids()),
///     ids(mutation.changed_ids()),
///     ids(mutation.deleted_ids()),
/// );
/// assert_eq!(line, "CreateClaim created=[claim_1] changed=[] deleted=[]");
/// # Ok::<(), sacm::edit::EditError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mutation {
    operation: Operation,
    created: Vec<ElementId>,
    changed: Vec<ElementId>,
    deleted: Vec<ElementId>,
}

impl Mutation {
    /// The operation, exactly as given to [`apply`].
    ///
    /// A generated id is not filled in: for an operation without `with_id`,
    /// its `id()` is still `None`, and the id is in
    /// [`created_ids`](Self::created_ids).
    ///
    /// # Examples
    ///
    /// ```
    /// use sacm::edit::{self, CreateArgumentPackage, Operation};
    /// use sacm::model::{Document, LangString};
    ///
    /// let mut doc = Document::new();
    /// let operation = CreateArgumentPackage::root(LangString::new("Arguments"));
    /// let mutation = edit::apply(&mut doc, operation.clone())?;
    /// assert_eq!(mutation.operation(), &Operation::from(operation));
    /// # Ok::<(), sacm::edit::EditError>(())
    /// ```
    #[must_use]
    pub fn operation(&self) -> &Operation {
        &self.operation
    }

    /// The ids of the elements created, in creation order, with the
    /// operation's own element first.
    ///
    /// They are the ids `semantic_compare(before, after)` reports as only in
    /// after. A create creates exactly its own element; its parent is not
    /// changed.
    #[must_use]
    pub fn created_ids(&self) -> &[ElementId] {
        &self.created
    }

    /// The ids of the existing elements changed, in the document's
    /// [pre-order](crate::model::Document::elements) after the operation.
    ///
    /// They are the ids, in both before and after, for which
    /// `semantic_compare(before, after)` reports a difference of the element
    /// itself, such as its kind, parent, name or child order. A create
    /// does not change its parent, so this is empty for every operation in
    /// this release.
    #[must_use]
    pub fn changed_ids(&self) -> &[ElementId] {
        &self.changed
    }

    /// The ids of the elements deleted, in the document's
    /// [pre-order](crate::model::Document::elements) before the operation.
    ///
    /// They are the ids `semantic_compare(before, after)` reports as only in
    /// before. A create deletes nothing, so this is empty for every
    /// operation in this release.
    #[must_use]
    pub fn deleted_ids(&self) -> &[ElementId] {
        &self.deleted
    }
}

/// Why [`apply`] refused an operation. The document is unchanged.
///
/// Each refusal has a catalog [`Code`], from [`code`](Self::code). `Display`
/// writes one line that starts in lower case, with ids quoted by `{:?}` so
/// that line breaks in them never break the line.
///
/// # Examples
///
/// ```
/// use sacm::edit::{self, CreateClaim};
/// use sacm::model::{Document, LangString};
///
/// let mut doc = Document::new();
/// let error = edit::apply(&mut doc, CreateClaim::new("arguments", LangString::new("C1"))).unwrap_err();
/// assert_eq!(error.to_string(), "the document has no element \"arguments\" to be the parent");
/// assert_eq!(error.code().as_str(), "SACM-CMD-001");
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum EditError {
    /// The parent is not in the document (SACM-CMD-001). Create the parent
    /// first, or correct its id. Ids are compared exactly, case included.
    #[non_exhaustive]
    ParentNotFound {
        /// The parent's id, as given.
        parent: ElementId,
    },
    /// No SACM composite role of the parent's class holds an element of the
    /// new kind (SACM-CMD-005): a Claim directly in an AssuranceCasePackage,
    /// say. Choose a parent whose class can hold the new kind. For example, a
    /// Claim goes in an ArgumentPackage, and an ArgumentPackage in an
    /// AssuranceCasePackage or another ArgumentPackage.
    #[non_exhaustive]
    InvalidParent {
        /// The parent's id.
        parent: ElementId,
        /// The parent's kind.
        parent_kind: ElementKind,
        /// The kind of the element that was to be created.
        kind: ElementKind,
    },
    /// The id given with `with_id` is not an XML `NCName` (SACM-ID-002). An
    /// `NCName` starts with a letter or `_` and continues with letters,
    /// digits, `-`, `.` or `_`; it has no spaces and no `:`. Or leave out
    /// `with_id` to have an id generated.
    #[non_exhaustive]
    InvalidId {
        /// The id, as given.
        id: ElementId,
    },
    /// The id given with `with_id` is already in the document, as an
    /// element's id or as the `xmi:id` of a name, a `content`, a `key` or a
    /// `value`, which identify no element
    /// ([`SacmElement::name_id`](crate::model::SacmElement::name_id),
    /// [`SacmElement::content_id`](crate::model::SacmElement::content_id))
    /// (SACM-CMD-004). Ids are unique across the whole document, not per
    /// parent. Choose another id, or leave out `with_id` to have one
    /// generated.
    #[non_exhaustive]
    DuplicateId {
        /// The id, as given.
        id: ElementId,
    },
    /// A reference end is given fewer ids than SACM requires
    /// (SACM-MULT-001): an asserted relationship with no source
    /// (`source:ArgumentAsset[1..*]`, §11.13). Give at least `required` ids.
    ///
    /// ```
    /// use sacm::edit::{self, CreateArgumentPackage, CreateAssertedInference, CreateClaim, EditError};
    /// use sacm::model::{Document, ElementId, LangString};
    ///
    /// let mut doc = Document::new();
    /// edit::apply(&mut doc, CreateArgumentPackage::root(LangString::new("Arguments")).with_id("p"))?;
    /// edit::apply(&mut doc, CreateClaim::new("p", LangString::new("C1")).with_id("c1"))?;
    /// let no_sources: Vec<ElementId> = Vec::new();
    /// let inference = CreateAssertedInference::new("p", LangString::new("I1"), no_sources, "c1");
    /// let error = edit::apply(&mut doc, inference).unwrap_err();
    /// assert!(matches!(
    ///     error,
    ///     EditError::TooFewReferences { end: "source", given: 0, required: 1, .. }
    /// ));
    /// assert_eq!(error.to_string(), "the source is given 0 ids, and SACM requires at least 1");
    /// # Ok::<(), EditError>(())
    /// ```
    #[non_exhaustive]
    TooFewReferences {
        /// The end, by its XMI name, such as `source`.
        end: &'static str,
        /// How many ids it was given.
        given: usize,
        /// How many it needs at least.
        required: usize,
    },
    /// A referenced id holds XML white space (space, TAB, LF or CR), so it is
    /// not an `NCName` and cannot be written as one id (SACM-ID-002): written
    /// to a reference attribute, `a b` would read back as the two ids `a`
    /// and `b`. A file can give an element such an id, since
    /// [`xmi::read`](crate::xmi::read) keeps an id as written and
    /// [`validation::validate`](crate::validation::validate) reports it
    /// (SACM23-VAL-020). The first such id is reported, in the order of
    /// [`ReferenceNotFound`](Self::ReferenceNotFound). Give the referenced
    /// element an `NCName` id in the file first.
    ///
    /// ```
    /// use sacm::edit::{self, CreateAssertedInference, EditError};
    /// use sacm::xmi::{self, ReadOptions};
    ///
    /// let input = br#"<sacm:ArgumentPackage xmlns:sacm="http://www.omg.org/spec/SACM/20220301"
    ///     xmlns:xmi="http://www.omg.org/spec/XMI/20131001" xmi:id="p">
    ///   <argumentElement xmi:type="sacm:Claim" xmi:id="c1"/>
    ///   <argumentElement xmi:type="sacm:Claim" xmi:id="a b"/>
    /// </sacm:ArgumentPackage>"#;
    /// let mut doc = xmi::read(input, &ReadOptions::default())?.into_document();
    /// let name = sacm::model::LangString::new("I1");
    /// let error = edit::apply(&mut doc, CreateAssertedInference::new("p", name, ["a b"], "c1")).unwrap_err();
    /// assert!(matches!(error, EditError::InvalidReference { end: "source", .. }));
    /// assert_eq!(
    ///     error.to_string(),
    ///     "the source \"a b\" holds white space, so it cannot be written as one id"
    /// );
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[non_exhaustive]
    InvalidReference {
        /// The end, by its XMI name, such as `source`.
        end: &'static str,
        /// The id, as given.
        id: ElementId,
    },
    /// A referenced id names no element of the document (SACM-CMD-001): a
    /// source, the target or the `reasoning` of an asserted relationship, or
    /// the `structure` of an `ArgumentReasoning`. An edit never makes a
    /// dangling reference (D-VAL-LAYER), and the `xmi:id` of a name,
    /// `content`, `key` or `value` names no element. The first such id is
    /// reported: the sources in order, then the target, the reasoning and the
    /// structure. Create the referenced element first, or correct the id;
    /// ids are compared exactly, case included.
    #[non_exhaustive]
    ReferenceNotFound {
        /// The end, by its XMI name, such as `target`.
        end: &'static str,
        /// The id, as given.
        id: ElementId,
    },
}

impl EditError {
    /// The catalog code of the refusal: SACM-CMD-001 for
    /// [`ParentNotFound`](Self::ParentNotFound), SACM-CMD-005 for
    /// [`InvalidParent`](Self::InvalidParent), SACM-ID-002 for
    /// [`InvalidId`](Self::InvalidId), SACM-CMD-004 for
    /// [`DuplicateId`](Self::DuplicateId), SACM-MULT-001 for
    /// [`TooFewReferences`](Self::TooFewReferences), SACM-ID-002 for
    /// [`InvalidReference`](Self::InvalidReference) and SACM-CMD-001 for
    /// [`ReferenceNotFound`](Self::ReferenceNotFound).
    #[must_use]
    pub fn code(&self) -> Code {
        match self {
            Self::ParentNotFound { .. } | Self::ReferenceNotFound { .. } => {
                Code::CMD_TARGET_NOT_FOUND
            }
            Self::InvalidParent { .. } => Code::CMD_INVALID_PARENT,
            Self::InvalidId { .. } | Self::InvalidReference { .. } => Code::ID_INVALID,
            Self::DuplicateId { .. } => Code::CMD_DUPLICATE_ID,
            Self::TooFewReferences { .. } => Code::MULTIPLICITY_VIOLATION,
        }
    }
}

impl fmt::Display for EditError {
    /// Writes the refusal on one line, as described on [`EditError`].
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ParentNotFound { parent } => write!(
                f,
                "the document has no element {:?} to be the parent",
                parent.as_str()
            ),
            Self::InvalidParent {
                parent,
                parent_kind,
                kind,
            } => write!(
                f,
                "element {:?} of kind {parent_kind} cannot contain an element of kind {kind}",
                parent.as_str()
            ),
            Self::InvalidId { id } => {
                write!(f, "the element id {:?} is not an XML NCName", id.as_str())
            }
            Self::DuplicateId { id } => write!(
                f,
                "the document already has an element with id {:?}",
                id.as_str()
            ),
            Self::TooFewReferences {
                end,
                given,
                required,
            } => write!(
                f,
                "the {end} is given {given} ids, and SACM requires at least {required}"
            ),
            Self::InvalidReference { end, id } => write!(
                f,
                "the {end} {:?} holds white space, so it cannot be written as one id",
                id.as_str()
            ),
            Self::ReferenceNotFound { end, id } => write!(
                f,
                "the document has no element {:?} for the {end} to reference",
                id.as_str()
            ),
        }
    }
}

impl std::error::Error for EditError {}
