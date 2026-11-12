//! Editing: operations as values, applied to a document with [`apply`].
//!
//! An operation is a plain value that says what to change. [`apply`] checks
//! it completely against the document and then either makes the change and
//! returns a [`Mutation`] listing what it created, or refuses with an
//! [`EditError`]. **A refused operation changes nothing**: not the elements,
//! not the written XMI, and not the ids generated next.
//!
//! # Operations
//!
//! | Operation | Creates | Root | Under a parent |
//! |---|---|---|---|
//! | [`CreateAssuranceCasePackage`] | an `AssuranceCasePackage` (§9.2) | [`root`](CreateAssuranceCasePackage::root) | [`new`](CreateAssuranceCasePackage::new) |
//! | [`CreateArgumentPackage`] | an `ArgumentPackage` (§11.4) | [`root`](CreateArgumentPackage::root) | [`new`](CreateArgumentPackage::new) |
//! | [`CreateClaim`] | a `Claim` (§11.11) | no | [`new`](CreateClaim::new) |
//!
//! A root is added after the document's last root; a child after its parent's
//! last child. A parent is legal exactly when a SACM composite role of its
//! class holds the new kind: the same containment rules the reader and writer
//! use (see [`Document`]). So a Claim goes in an ArgumentPackage, not directly
//! in an AssuranceCasePackage, and an ArtifactPackage, a package interface or
//! a package binding can be a parent too.
//!
//! **The name is required.** Every operation takes the element's `name` as a
//! [`LangString`](crate::model::LangString), stored exactly as given, so an
//! edit never makes a `ModelElement` without a name ([ADR 0013]). Any value is
//! accepted, `LangString::default()` included.
//!
//! [ADR 0013]: https://github.com/lasrod/libSACM/blob/main/docs/adr/0013-missing-name-stays-missing.md
//!
//! # Ids
//!
//! - **Given by the caller** with `with_id`: it must be an XML `NCName` (an
//!   `xmi:id` is an `xsd:ID`, XMI 2.5.1 §7.6.1), such as `claim_top` or
//!   `_c.1`, and not already in the document.
//! - **Generated** otherwise: `<snake_case kind>_<n>`, such as
//!   `assurance_case_package_1`, `argument_package_1` or `claim_1`. `n` is the
//!   smallest number above the document's counter for the kind whose id is not
//!   already in use, whoever gave it. The counter advances only when an
//!   operation succeeds with a generated id; a caller id does not move it.
//! - **Deterministic:** the same operations on equal documents give the same
//!   ids, and a clone of a document generates the same ids as the original.
//!
//! Ids read from a file are kept exactly as read; only new ids are checked.
//!
//! # Refusals
//!
//! The checks run in this order, and the first that fails is the result. A
//! root operation has no parent, so it skips the first two.
//!
//! | Check | Refusal | Code |
//! |---|---|---|
//! | 1. the parent is in the document | [`EditError::ParentNotFound`] | SACM-CMD-001 |
//! | 2. the parent can contain the new kind | [`EditError::InvalidParent`] | SACM-CMD-005 |
//! | 3. a caller id is an `NCName` | [`EditError::InvalidId`] | SACM-ID-002 |
//! | 4. a caller id is not in use | [`EditError::DuplicateId`] | SACM-CMD-004 |
//!
//! # Examples
//!
//! ```
//! use sacm::diagnostic::Code;
//! use sacm::edit::{self, CreateArgumentPackage, CreateAssuranceCasePackage, CreateClaim};
//! use sacm::model::{Document, ElementId, LangString};
//! use sacm::validation;
//! use sacm::xmi::{self, WriteOptions};
//!
//! let name = |text: &str| LangString::new(text).with_lang("en");
//! let mut doc = Document::new();
//!
//! edit::apply(&mut doc, CreateAssuranceCasePackage::root(name("Safety case")).with_id("case"))?;
//! edit::apply(&mut doc, CreateArgumentPackage::new("case", name("Arguments")).with_id("arguments"))?;
//! // No `with_id`: the id is generated.
//! let created = edit::apply(&mut doc, CreateClaim::new("arguments", name("The system is safe")))?;
//! assert_eq!(created.created_ids(), [ElementId::from("claim_1")]);
//!
//! // A Claim cannot sit directly in an AssuranceCasePackage.
//! let refused = edit::apply(&mut doc, CreateClaim::new("case", name("Misplaced"))).unwrap_err();
//! assert_eq!(refused.code(), Code::CMD_INVALID_PARENT);
//! assert_eq!(doc.element_count(), 3);
//!
//! // Every element has a name, so validation finds nothing.
//! assert!(validation::validate(&doc).is_empty());
//! let text = xmi::write(&doc, &WriteOptions::default())?;
//! assert!(text.contains(r#"xmi:id="claim_1""#));
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod create;

use std::fmt;

pub use create::{CreateArgumentPackage, CreateAssuranceCasePackage, CreateClaim};

use crate::diagnostic::Code;
use crate::model::{Document, ElementId, ElementKind};

/// Checks `operation` against `document` and applies it, or refuses it and
/// leaves `document` unchanged.
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
/// - the id given with `with_id` is already in the document
///   ([`EditError::DuplicateId`]).
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
/// let error = edit::apply(&mut doc, again).unwrap_err();
/// assert!(matches!(error, EditError::DuplicateId { .. }));
/// assert_eq!(doc.element_count(), 1);
/// # Ok::<(), EditError>(())
/// ```
pub fn apply(
    document: &mut Document,
    operation: impl Into<Operation>,
) -> Result<Mutation, EditError> {
    match operation.into() {
        Operation::CreateAssuranceCasePackage(operation) => operation.apply(document),
        Operation::CreateArgumentPackage(operation) => operation.apply(document),
        Operation::CreateClaim(operation) => operation.apply(document),
    }
}

/// An edit operation, as a value: any of the operation structs.
///
/// [`apply`] takes an `Operation` or any operation struct, which converts
/// into one with `From`. Later releases add operations, so a `match` on
/// `Operation` needs a wildcard arm.
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

/// What an applied operation changed.
///
/// ```
/// use sacm::edit::{self, CreateAssuranceCasePackage};
/// use sacm::model::{Document, ElementId, LangString};
///
/// let mut doc = Document::new();
/// let operation = CreateAssuranceCasePackage::root(LangString::new("Case")).with_id("case");
/// let mutation = edit::apply(&mut doc, operation)?;
/// assert_eq!(mutation.created_ids(), [ElementId::from("case")]);
/// # Ok::<(), sacm::edit::EditError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mutation {
    created: Vec<ElementId>,
}

impl Mutation {
    /// A mutation that created `created`, in creation order.
    fn created(created: Vec<ElementId>) -> Self {
        Self { created }
    }

    /// The ids of the elements created, in creation order, with the
    /// operation's own element first.
    #[must_use]
    pub fn created_ids(&self) -> &[ElementId] {
        &self.created
    }
}

/// Why [`apply`] refused an operation. The document is unchanged.
///
/// Each refusal has a catalog [`Code`], from [`code`](Self::code). `Display`
/// writes one line that starts in lower case, with ids quoted by `{:?}` so
/// that line breaks in them never break the line:
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
    /// The parent is not in the document (SACM-CMD-001).
    #[non_exhaustive]
    ParentNotFound {
        /// The parent's id, as given.
        parent: ElementId,
    },
    /// No SACM composite role of the parent's class holds an element of the
    /// new kind (SACM-CMD-005): a Claim directly in an AssuranceCasePackage,
    /// say.
    #[non_exhaustive]
    InvalidParent {
        /// The parent's id.
        parent: ElementId,
        /// The parent's kind.
        parent_kind: ElementKind,
        /// The kind of the element that was to be created.
        kind: ElementKind,
    },
    /// The id given with `with_id` is not an XML `NCName` (SACM-ID-002).
    #[non_exhaustive]
    InvalidId {
        /// The id, as given.
        id: ElementId,
    },
    /// The id given with `with_id` is already in the document
    /// (SACM-CMD-004).
    #[non_exhaustive]
    DuplicateId {
        /// The id, as given.
        id: ElementId,
    },
}

impl EditError {
    /// The catalog code of the refusal: SACM-CMD-001 for
    /// [`ParentNotFound`](Self::ParentNotFound), SACM-CMD-005 for
    /// [`InvalidParent`](Self::InvalidParent), SACM-ID-002 for
    /// [`InvalidId`](Self::InvalidId) and SACM-CMD-004 for
    /// [`DuplicateId`](Self::DuplicateId).
    #[must_use]
    pub fn code(&self) -> Code {
        match self {
            Self::ParentNotFound { .. } => Code::CMD_TARGET_NOT_FOUND,
            Self::InvalidParent { .. } => Code::CMD_INVALID_PARENT,
            Self::InvalidId { .. } => Code::ID_INVALID,
            Self::DuplicateId { .. } => Code::CMD_DUPLICATE_ID,
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
        }
    }
}

impl std::error::Error for EditError {}
