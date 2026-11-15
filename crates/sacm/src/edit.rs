//! Editing: operations as values, applied to a document with [`apply`].
//!
//! An operation is a plain value that says what to change. [`apply`] checks
//! it completely against the document and then either makes the change and
//! returns a [`Mutation`] (the operation, and the ids it created, changed and
//! deleted), or refuses with an [`EditError`]. **A refused operation changes
//! nothing**: not the elements, not the written XMI, and not the ids
//! generated next.
//!
//! # Operations that create an element
//!
//! | Operation | Creates | Root | Under a parent |
//! |---|---|---|---|
//! | [`CreateAssuranceCasePackage`] | an `AssuranceCasePackage` (§9.2) | [`root`](CreateAssuranceCasePackage::root) | [`new`](CreateAssuranceCasePackage::new) |
//! | [`CreateArgumentPackage`] | an `ArgumentPackage` (§11.4) | [`root`](CreateArgumentPackage::root) | [`new`](CreateArgumentPackage::new) |
//! | [`CreateClaim`] | a `Claim` (§11.11) | no | [`new`](CreateClaim::new) |
//! | [`CreateArgumentReasoning`] | an `ArgumentReasoning` (§11.12), with an optional `structure` | no | [`new`](CreateArgumentReasoning::new) |
//! | [`CreateAssertedInference`] | an `AssertedInference` (§11.14), with its sources and target, and an optional `reasoning` and `isCounter` | no | [`new`](CreateAssertedInference::new) |
//! | [`CreateAssertedContext`] | an `AssertedContext` (§11.16), with its sources and target, and an optional `reasoning` and `isCounter` | no | [`new`](CreateAssertedContext::new) |
//! | [`CreateArtifactReference`] | an `ArtifactReference` (§11.9), with its optional `referencedArtifactElement` ids | no | [`new`](CreateArtifactReference::new) |
//! | [`CreateArgumentGroup`] | an `ArgumentGroup` (§11.2), with its optional members (`argumentElement`) | no | [`new`](CreateArgumentGroup::new) |
//! | [`CreateAssertedEvidence`] | an `AssertedEvidence` (§11.15), with its sources and target, and an optional `reasoning` and `isCounter` | no | [`new`](CreateAssertedEvidence::new) |
//! | [`CreateAssertedArtifactSupport`] | an `AssertedArtifactSupport` (§11.17), with its sources and target, and an optional `reasoning` and `isCounter` | no | [`new`](CreateAssertedArtifactSupport::new) |
//! | [`CreateAssertedArtifactContext`] | an `AssertedArtifactContext` (§11.18), with its sources and target, and an optional `reasoning` and `isCounter` | no | [`new`](CreateAssertedArtifactContext::new) |
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
//! An `ArgumentReasoning`, an `ArtifactReference`, an `ArgumentGroup` and
//! the asserted relationships reference other elements (ADR 0027, ADR 0028).
//! An asserted relationship takes its sources (at least one, in order,
//! repeats kept) and its target when it is created, as SACM requires
//! (§11.13 `source:ArgumentAsset[1..*]`, `target:ArgumentAsset[1]`). Every
//! id they reference must name an element of the document, so an edit never
//! makes a dangling reference; the id of a name, `content`, `key` or `value`
//! names no element. And each referenced element must have the type SACM
//! declares for its end, so an edit never makes a wrongly typed reference
//! either (D-VAL-LAYER): an `ArgumentAsset` for a source or the target, an
//! `ArgumentReasoning` for the reasoning, an `ArgumentPackage` for the
//! structure, an `ArtifactElement` (any element but a `UtilityElement`) for
//! a `referencedArtifactElement`, and an `ArgumentationElement` for a group
//! member. The narrower rules of §11.15, §11.17 and §11.18, that the ends of
//! an `AssertedEvidence`, `AssertedArtifactSupport` or
//! `AssertedArtifactContext` be `ArtifactReference`s, are content: a create
//! does not refuse them, and
//! [`validation::validate`](crate::validation::validate) reports them
//! (SACM23-VAL-016). The `metaClaim`, `citedElement` and `abstractForm` of
//! an element are set after it is created, with [`SetReferences`].
//!
//! # Operations that change an element
//!
//! | Operation | Changes | Forms |
//! |---|---|---|
//! | [`SetName`] | the `name` of a `ModelElement` (§8.6) | [`new`](SetName::new) |
//! | [`SetGid`] | the `gid` of any element (§8.2) | [`new`](SetGid::new), [`remove`](SetGid::remove) |
//! | [`SetFlag`] | one [`Flag`](crate::model::Flag): `isCitation` or `isAbstract` of any element (§8.2), `isCounter` of an asserted relationship (§11.13) | [`new`](SetFlag::new) |
//! | [`SetAssertionDeclaration`] | the `assertionDeclaration` of an Assertion (§11.10) | [`new`](SetAssertionDeclaration::new) |
//! | [`SetContent`] | the `content` of a `UtilityElement` (§8.7) | [`new`](SetContent::new), [`remove`](SetContent::remove) |
//! | [`SetKey`] | the `key` of a `TaggedValue` (§8.12) | [`new`](SetKey::new) |
//! | [`SetReferences`] | the ids one [`ReferenceEnd`](crate::model::ReferenceEnd) holds, such as `source` or `metaClaim` | [`new`](SetReferences::new), [`clear`](SetReferences::clear) |
//! | [`AddReference`] | one [`ReferenceEnd`](crate::model::ReferenceEnd), such as `source` or `metaClaim`, with one id appended | [`new`](AddReference::new) |
//! | [`RemoveReference`] | one [`ReferenceEnd`](crate::model::ReferenceEnd), such as `source` or `metaClaim`, with every occurrence of one id removed | [`new`](RemoveReference::new) |
//! | [`ReorderElements`] | the order of an element's children, or of the roots | [`new`](ReorderElements::new), [`roots`](ReorderElements::roots) |
//!
//! Each names the element it changes by id, and replaces one feature
//! exactly as given (ADR 0029). The value of a name, gid, flag, declaration,
//! `content` or key is never refused: an empty name, a duplicate gid, an
//! empty `content` or an `asCited` declaration on an element that cites
//! nothing are content, which
//! [`validation::validate`](crate::validation::validate) reports
//! (D-VAL-LAYER). A flag and its reference are set independently, such as
//! `isCitation` and `citedElement`. A reference end stays within its bounds
//! (`source` `[1..*]`, `target` `[1]`, the other single-valued ends
//! `[0..1]`), and every id it newly holds names an element of a kind it
//! admits; an id it already holds is kept, even a dangling or wrongly typed
//! one, so an end read from a file can be repaired one id at a time.
//!
//! Ids are kept by position: a replaced name keeps its `xmi:id`, and a
//! replaced `content` or `key` keeps the id of its element and, for each
//! value, the id of the value at its position. The ids beyond the new
//! length, every id of a removed `content`, and an id no reference holds
//! any more are freed, so a later create may generate them.
//!
//! An operation that sets the value held changes nothing: the document is
//! not written, and [`Mutation::changed_ids`] is empty. Otherwise it is the
//! element, or for [`ReorderElements::new`] the parent; a root reorder
//! changes no element, so it reports none.
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
//! ## Creates
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
//! | 6. no referenced id holds XML white space (space, TAB, LF, CR): each source in order, then the target, the `reasoning`, the `structure`, the `referencedArtifactElement` ids and the group members | [`EditError::InvalidReference`] | SACM-ID-002 | An id such as `a b`, read from a file as written, is not an `NCName` and would be written as two ids. Give the referenced element an `NCName` id in the file. |
//! | 7. every referenced id names an element, in the same order | [`EditError::ReferenceNotFound`] | SACM-CMD-001 | Create the referenced element first, or correct its id. Ids are compared exactly, case included. |
//! | 8. every referenced id names an element of a kind its end admits, in the same order | [`EditError::WrongReferenceType`] | SACM-REF-002 | Reference an element of the end's type: for example, an `ArgumentPackage` as a `structure`, and an `ArgumentAsset`, not a package, as a source. |
//!
//! ## Operations that change an element
//!
//! Every check runs before the one change, and the first that fails is the
//! result. Each operation first checks:
//!
//! | Check | Refusal | Code |
//! |---|---|---|
//! | 1. the element (for a reorder, the parent) is in the document | [`EditError::ElementNotFound`] | SACM-CMD-001 |
//! | 2. its kind has the feature: a `ModelElement` for a name, an `AssertedRelationship` for `isCounter`, an Assertion for a declaration, a `UtilityElement` for a `content`, a `TaggedValue` for a key, the end's class for a reference end | [`EditError::FeatureNotApplicable`] | SACM-CMD-008 |
//!
//! [`SetName`], [`SetGid`], [`SetFlag`], [`SetAssertionDeclaration`],
//! [`SetContent`] and [`SetKey`] check nothing more. Every element has a
//! gid, `isCitation` and `isAbstract`, so those skip the second check too.
//!
//! The reference operations then check, in this order:
//!
//! | Check | Refusal | Code |
//! |---|---|---|
//! | 3. for [`RemoveReference`] only: the end holds the id | [`EditError::ReferenceNotHeld`] | SACM-CMD-001 |
//! | 4. the end would hold at least the ids it needs: one `source`, one `target` | [`EditError::TooFewReferences`] | SACM-MULT-001 |
//! | 5. the end would hold at most the ids it allows: one for a single-valued end | [`EditError::TooManyReferences`] | SACM-MULT-001 |
//! | 6. no new id holds XML white space | [`EditError::InvalidReference`] | SACM-ID-002 |
//! | 7. every new id names an element | [`EditError::ReferenceNotFound`] | SACM-CMD-001 |
//! | 8. every new id names an element of a kind the end admits | [`EditError::WrongReferenceType`] | SACM-REF-002 |
//!
//! A new id is one the end does not hold already; checks 6 to 8 each run
//! over the new ids in list order, as the creates' checks 6 to 8 do.
//!
//! [`ReorderElements`] checks, after the parent (check 1;
//! [`ReorderElements::roots`] has no parent, so it skips check 1):
//!
//! | Check | Refusal | Code |
//! |---|---|---|
//! | 2. the list names at least one element | [`EditError::InvalidOrder`] | SACM-CMD-009 |
//! | 3. id by id, in list order: it is a direct child of the parent (or a root) | [`EditError::ChildNotFound`] | SACM-CMD-001 |
//! | 4. for the same id, right after check 3: it was not named before | [`EditError::InvalidOrder`] | SACM-CMD-009 |
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
mod references;
mod reorder;
mod set;
mod utility;

use std::fmt;

pub use argumentation::{
    CreateArgumentGroup, CreateArgumentReasoning, CreateArtifactReference,
    CreateAssertedArtifactContext, CreateAssertedArtifactSupport, CreateAssertedContext,
    CreateAssertedEvidence, CreateAssertedInference,
};
pub use create::{CreateArgumentPackage, CreateAssuranceCasePackage, CreateClaim};
pub use references::{AddReference, RemoveReference, SetReferences};
pub use reorder::ReorderElements;
pub use set::{SetAssertionDeclaration, SetContent, SetFlag, SetGid, SetKey, SetName};
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
/// Refuses a create, checked in this order, when:
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
///   ([`EditError::ReferenceNotFound`]);
/// - a referenced id names an element of a kind its end does not admit
///   ([`EditError::WrongReferenceType`]).
///
/// Refuses an operation that changes an element, checked in this order,
/// when:
/// - the element, or the parent to reorder, is not in the document
///   ([`EditError::ElementNotFound`]);
/// - its kind has no such feature ([`EditError::FeatureNotApplicable`]);
/// - for [`RemoveReference`], the end does not hold the id
///   ([`EditError::ReferenceNotHeld`]);
/// - for a reference operation, the end would hold fewer or more ids than
///   its bounds allow ([`EditError::TooFewReferences`],
///   [`EditError::TooManyReferences`]), or an id it does not hold already
///   fails the create's last three checks;
/// - for [`ReorderElements`], the list is empty or names an id twice
///   ([`EditError::InvalidOrder`]), or names an id that is no direct child
///   or root ([`EditError::ChildNotFound`]).
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
    // refusal here leaves it untouched. A create changes no existing element
    // (ADR 0019 amendment 1), an edit creates none (ADR 0029), and none
    // deletes one.
    let (created, changed) = match &operation {
        Operation::CreateAssuranceCasePackage(create) => (create.apply(document)?, Vec::new()),
        Operation::CreateArgumentPackage(create) => (create.apply(document)?, Vec::new()),
        Operation::CreateClaim(create) => (create.apply(document)?, Vec::new()),
        Operation::CreateArgumentReasoning(create) => (create.apply(document)?, Vec::new()),
        Operation::CreateAssertedInference(create) => (create.apply(document)?, Vec::new()),
        Operation::CreateAssertedContext(create) => (create.apply(document)?, Vec::new()),
        Operation::CreateArtifactReference(create) => (create.apply(document)?, Vec::new()),
        Operation::CreateArgumentGroup(create) => (create.apply(document)?, Vec::new()),
        Operation::CreateAssertedEvidence(create) => (create.apply(document)?, Vec::new()),
        Operation::CreateAssertedArtifactSupport(create) => (create.apply(document)?, Vec::new()),
        Operation::CreateAssertedArtifactContext(create) => (create.apply(document)?, Vec::new()),
        Operation::CreateDescription(create) => (create.apply(document)?, Vec::new()),
        Operation::CreateImplementationConstraint(create) => (create.apply(document)?, Vec::new()),
        Operation::CreateNote(create) => (create.apply(document)?, Vec::new()),
        Operation::CreateTaggedValue(create) => (create.apply(document)?, Vec::new()),
        Operation::SetName(set) => (Vec::new(), set.apply(document)?),
        Operation::SetGid(set) => (Vec::new(), set.apply(document)?),
        Operation::SetFlag(set) => (Vec::new(), set.apply(document)?),
        Operation::SetAssertionDeclaration(set) => (Vec::new(), set.apply(document)?),
        Operation::SetContent(set) => (Vec::new(), set.apply(document)?),
        Operation::SetKey(set) => (Vec::new(), set.apply(document)?),
        Operation::SetReferences(set) => (Vec::new(), set.apply(document)?),
        Operation::AddReference(add) => (Vec::new(), add.apply(document)?),
        Operation::RemoveReference(remove) => (Vec::new(), remove.apply(document)?),
        Operation::ReorderElements(reorder) => (Vec::new(), reorder.apply(document)?),
    };
    Ok(Mutation {
        operation,
        created,
        changed,
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
    /// Creates an `ArtifactReference`.
    CreateArtifactReference(CreateArtifactReference),
    /// Creates an `ArgumentGroup`.
    CreateArgumentGroup(CreateArgumentGroup),
    /// Creates an `AssertedEvidence`.
    CreateAssertedEvidence(CreateAssertedEvidence),
    /// Creates an `AssertedArtifactSupport`.
    CreateAssertedArtifactSupport(CreateAssertedArtifactSupport),
    /// Creates an `AssertedArtifactContext`.
    CreateAssertedArtifactContext(CreateAssertedArtifactContext),
    /// Creates a `Description`.
    CreateDescription(CreateDescription),
    /// Creates an `ImplementationConstraint`.
    CreateImplementationConstraint(CreateImplementationConstraint),
    /// Creates a `Note`.
    CreateNote(CreateNote),
    /// Creates a `TaggedValue`.
    CreateTaggedValue(CreateTaggedValue),
    /// Sets the name of a `ModelElement`.
    SetName(SetName),
    /// Sets or removes the `gid` of an element.
    SetGid(SetGid),
    /// Sets a flag of an element: `isCitation`, `isAbstract` or `isCounter`.
    SetFlag(SetFlag),
    /// Sets the `assertionDeclaration` of an Assertion.
    SetAssertionDeclaration(SetAssertionDeclaration),
    /// Sets or removes the `content` of a `UtilityElement`.
    SetContent(SetContent),
    /// Sets the `key` of a `TaggedValue`.
    SetKey(SetKey),
    /// Replaces the ids a reference end of an element holds.
    SetReferences(SetReferences),
    /// Appends an id to a reference end of an element.
    AddReference(AddReference),
    /// Removes every occurrence of an id from a reference end of an element.
    RemoveReference(RemoveReference),
    /// Reorders the children of an element, or the roots.
    ReorderElements(ReorderElements),
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
    /// `"CreateArtifactReference"`, `"CreateArgumentGroup"`,
    /// `"CreateAssertedEvidence"`, `"CreateAssertedArtifactSupport"`,
    /// `"CreateAssertedArtifactContext"`, `"CreateDescription"`,
    /// `"CreateImplementationConstraint"`, `"CreateNote"`,
    /// `"CreateTaggedValue"`, `"SetName"`, `"SetGid"`, `"SetFlag"`,
    /// `"SetAssertionDeclaration"`, `"SetContent"`, `"SetKey"`,
    /// `"SetReferences"`, `"AddReference"`, `"RemoveReference"` and
    /// `"ReorderElements"`. The two forms of one struct, such as
    /// [`SetGid::new`] and [`SetGid::remove`], share its name.
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
            Self::CreateArtifactReference(_) => "CreateArtifactReference",
            Self::CreateArgumentGroup(_) => "CreateArgumentGroup",
            Self::CreateAssertedEvidence(_) => "CreateAssertedEvidence",
            Self::CreateAssertedArtifactSupport(_) => "CreateAssertedArtifactSupport",
            Self::CreateAssertedArtifactContext(_) => "CreateAssertedArtifactContext",
            Self::CreateDescription(_) => "CreateDescription",
            Self::CreateImplementationConstraint(_) => "CreateImplementationConstraint",
            Self::CreateNote(_) => "CreateNote",
            Self::CreateTaggedValue(_) => "CreateTaggedValue",
            Self::SetName(_) => "SetName",
            Self::SetGid(_) => "SetGid",
            Self::SetFlag(_) => "SetFlag",
            Self::SetAssertionDeclaration(_) => "SetAssertionDeclaration",
            Self::SetContent(_) => "SetContent",
            Self::SetKey(_) => "SetKey",
            Self::SetReferences(_) => "SetReferences",
            Self::AddReference(_) => "AddReference",
            Self::RemoveReference(_) => "RemoveReference",
            Self::ReorderElements(_) => "ReorderElements",
        }
    }
}

/// Converts each operation struct that is not a create into its
/// [`Operation`] variant of the same name.
macro_rules! operation_from {
    ($($name:ident),* $(,)?) => {
        $(
            impl From<$name> for Operation {
                fn from(operation: $name) -> Self {
                    Self::$name(operation)
                }
            }
        )*
    };
}

operation_from!(
    SetName,
    SetGid,
    SetFlag,
    SetAssertionDeclaration,
    SetContent,
    SetKey,
    SetReferences,
    AddReference,
    RemoveReference,
    ReorderElements,
);

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

impl From<CreateArtifactReference> for Operation {
    fn from(operation: CreateArtifactReference) -> Self {
        Self::CreateArtifactReference(operation)
    }
}

impl From<CreateArgumentGroup> for Operation {
    fn from(operation: CreateArgumentGroup) -> Self {
        Self::CreateArgumentGroup(operation)
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
/// parent is in [`operation`](Self::operation).
///
/// An operation that changes an element, such as [`SetName`] or
/// [`ReorderElements`], creates nothing, and changes its element (for a
/// reorder, the parent) exactly when its value differs from the one held;
/// setting the value held changes nothing, writes nothing, and gives
/// `changed_ids` empty. An element a reference names is not changed by the
/// reference, and a root reorder changes no element. No operation of this
/// release deletes one, so `deleted_ids` is always empty.
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
    /// itself, such as its kind, parent, name or child order. A create does
    /// not change its parent, so it gives none. An operation that changes an
    /// element gives that element, or none when it sets the value held; a
    /// [`ReorderElements::new`] gives the parent, and a
    /// [`ReorderElements::roots`] none.
    ///
    /// # Examples
    ///
    /// ```
    /// use sacm::edit::{self, CreateArgumentPackage, CreateClaim, SetFlag};
    /// use sacm::model::{Document, ElementId, Flag, LangString};
    ///
    /// let mut doc = Document::new();
    /// edit::apply(&mut doc, CreateArgumentPackage::root(LangString::new("Arguments")).with_id("p"))?;
    /// edit::apply(&mut doc, CreateClaim::new("p", LangString::new("C1")).with_id("c1"))?;
    ///
    /// let set = edit::apply(&mut doc, SetFlag::new("c1", Flag::IsAbstract, true))?;
    /// assert_eq!(set.changed_ids(), [ElementId::from("c1")]);
    /// let again = edit::apply(&mut doc, SetFlag::new("c1", Flag::IsAbstract, true))?;
    /// assert!(again.changed_ids().is_empty());
    /// # Ok::<(), sacm::edit::EditError>(())
    /// ```
    #[must_use]
    pub fn changed_ids(&self) -> &[ElementId] {
        &self.changed
    }

    /// The ids of the elements deleted, in the document's
    /// [pre-order](crate::model::Document::elements) before the operation.
    ///
    /// They are the ids `semantic_compare(before, after)` reports as only in
    /// before. No operation of this release deletes an element, so this is
    /// always empty.
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
    /// (`source:ArgumentAsset[1..*]`, §11.13) or no target
    /// (`target:ArgumentAsset[1]`), which [`SetReferences::clear`] of either
    /// end, or [`RemoveReference`] of the last source or of the target, would
    /// leave. `given` is how many ids the end would hold. Give at least
    /// `required` ids.
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
        /// How many ids it would hold.
        given: usize,
        /// How many it needs at least.
        required: usize,
    },
    /// A reference end is given more ids than SACM allows (SACM-MULT-001):
    /// two ids on a single-valued end, such as `target`, `reasoning` or
    /// `citedElement`, or an [`AddReference`] to one that holds an id
    /// already. The same id twice is two ids. Give at most `allowed` ids, or
    /// replace the one held with [`SetReferences`].
    ///
    /// ```
    /// use sacm::edit::{self, CreateArgumentPackage, CreateClaim, EditError, SetReferences};
    /// use sacm::model::{Document, LangString, ReferenceEnd};
    ///
    /// let mut doc = Document::new();
    /// edit::apply(&mut doc, CreateArgumentPackage::root(LangString::new("Arguments")).with_id("p"))?;
    /// edit::apply(&mut doc, CreateClaim::new("p", LangString::new("C1")).with_id("c1"))?;
    /// let two = SetReferences::new("c1", ReferenceEnd::CitedElement, ["c1", "c1"]);
    /// let error = edit::apply(&mut doc, two).unwrap_err();
    /// assert!(matches!(
    ///     error,
    ///     EditError::TooManyReferences { end: "citedElement", given: 2, allowed: 1, .. }
    /// ));
    /// assert_eq!(error.to_string(), "the citedElement is given 2 ids, and SACM allows at most 1");
    /// # Ok::<(), EditError>(())
    /// ```
    #[non_exhaustive]
    TooManyReferences {
        /// The end, by its XMI name, such as `target`.
        end: &'static str,
        /// How many ids it would hold.
        given: usize,
        /// How many it holds at most.
        allowed: usize,
    },
    /// A referenced id holds XML white space (space, TAB, LF or CR), so it is
    /// not an `NCName` and cannot be written as one id (SACM-ID-002): written
    /// to a reference attribute, `a b` would read back as the two ids `a`
    /// and `b`. A file can give an element such an id, since
    /// [`xmi::read`](crate::xmi::read) keeps an id as written and
    /// [`validation::validate`](crate::validation::validate) reports it
    /// (SACM23-VAL-020). The first such id is reported, in the order of
    /// [`ReferenceNotFound`](Self::ReferenceNotFound). [`SetReferences`]
    /// and [`AddReference`] check only the ids the end does not hold
    /// already. Give the referenced element an `NCName` id in the file
    /// first.
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
    /// source, the target or the `reasoning` of an asserted relationship,
    /// the `structure` of an `ArgumentReasoning`, a
    /// `referencedArtifactElement` of an `ArtifactReference` or a member of
    /// an `ArgumentGroup`. An edit never makes a dangling reference
    /// (D-VAL-LAYER), and the `xmi:id` of a name, `content`, `key` or
    /// `value` names no element. The first such id is reported: the sources
    /// in order, then the target, the reasoning, the structure, the
    /// `referencedArtifactElement` ids in order and the group members in
    /// order. [`SetReferences`] and [`AddReference`] check only the ids the
    /// end does not hold already, in list order, so a dangling id read from
    /// a file can be kept while the end is edited. Create the referenced
    /// element first, or correct the id; ids are compared exactly, case
    /// included.
    #[non_exhaustive]
    ReferenceNotFound {
        /// The end, by its XMI name, such as `target`.
        end: &'static str,
        /// The id, as given.
        id: ElementId,
    },
    /// A referenced id names an element of a kind its end's declared type
    /// does not admit (SACM-REF-002): a `structure` that is no
    /// `ArgumentPackage`, a source or target that is no `ArgumentAsset`, a
    /// `reasoning` that is no `ArgumentReasoning`, a
    /// `referencedArtifactElement` that is a `UtilityElement`, or a group
    /// member that is no `ArgumentationElement`. An edit never makes a
    /// wrongly typed reference (D-VAL-LAYER, ADR 0028). It is checked once
    /// every id is known to name an element
    /// ([`ReferenceNotFound`](Self::ReferenceNotFound)), in the same order,
    /// and the first such id is reported. [`SetReferences`] and
    /// [`AddReference`] check only the ids the end does not hold already.
    /// Reference an element of the end's type instead.
    ///
    /// The narrower §11.15, §11.17 and §11.18 rules, that the ends of an
    /// `AssertedEvidence`, `AssertedArtifactSupport` or
    /// `AssertedArtifactContext` be `ArtifactReference`s, are content, not
    /// refusals: [`validation::validate`](crate::validation::validate)
    /// reports them (SACM23-VAL-016).
    ///
    /// ```
    /// use sacm::edit::{self, CreateArgumentPackage, CreateArgumentReasoning, CreateClaim, EditError};
    /// use sacm::model::{Document, ElementKind, LangString};
    ///
    /// let mut doc = Document::new();
    /// edit::apply(&mut doc, CreateArgumentPackage::root(LangString::new("Arguments")).with_id("p"))?;
    /// edit::apply(&mut doc, CreateClaim::new("p", LangString::new("C1")).with_id("c1"))?;
    /// let reasoning = CreateArgumentReasoning::new("p", LangString::new("R1")).with_structure("c1");
    /// let error = edit::apply(&mut doc, reasoning).unwrap_err();
    /// assert!(matches!(
    ///     error,
    ///     EditError::WrongReferenceType {
    ///         end: "structure",
    ///         kind: ElementKind::Claim,
    ///         expected: "ArgumentPackage",
    ///         ..
    ///     }
    /// ));
    /// assert_eq!(
    ///     error.to_string(),
    ///     r#"the structure "c1" names a Claim, and the structure must be an ArgumentPackage"#
    /// );
    /// # Ok::<(), EditError>(())
    /// ```
    #[non_exhaustive]
    WrongReferenceType {
        /// The end, by its XMI name, such as `structure`.
        end: &'static str,
        /// The id, as given.
        id: ElementId,
        /// The kind of the element the id names.
        kind: ElementKind,
        /// The end's declared type, by its SACM class name, such as
        /// `ArgumentPackage` or `ArgumentAsset`.
        expected: &'static str,
    },
    /// The element to edit is not in the document (SACM-CMD-001): the
    /// element of a set or reference operation, or the parent of a
    /// [`ReorderElements::new`]. Ids are compared exactly, case included;
    /// the `xmi:id` of a name, `content`, `key` or `value` names no element.
    /// Correct the id.
    ///
    /// ```
    /// use sacm::edit::{self, EditError, SetName};
    /// use sacm::model::{Document, LangString};
    ///
    /// let mut doc = Document::new();
    /// let error = edit::apply(&mut doc, SetName::new("c1", LangString::new("C1"))).unwrap_err();
    /// assert!(matches!(error, EditError::ElementNotFound { .. }));
    /// assert_eq!(error.to_string(), "the document has no element \"c1\" to edit");
    /// ```
    #[non_exhaustive]
    ElementNotFound {
        /// The element's id, as given.
        element: ElementId,
    },
    /// The element's kind has no such feature (SACM-CMD-008): a `name` on a
    /// `UtilityElement` (§8.6 gives names to `ModelElement`s), `isCounter`
    /// on a kind that is no `AssertedRelationship` (§11.13), an
    /// `assertionDeclaration` on one that is no Assertion (§11.10), a
    /// `content` on one that is no `UtilityElement` (§8.7), a `key` on one
    /// that is no `TaggedValue` (§8.12), or a reference end the kind does
    /// not own, such as a `source` on a Claim. Every element has a `gid`,
    /// `isCitation`, `isAbstract`, `citedElement` and `abstractForm`, so
    /// those never fail here. Edit an element whose kind has the feature.
    ///
    /// ```
    /// use sacm::edit::{self, CreateArgumentPackage, CreateClaim, EditError, SetFlag};
    /// use sacm::model::{Document, ElementKind, Flag, LangString};
    ///
    /// let mut doc = Document::new();
    /// edit::apply(&mut doc, CreateArgumentPackage::root(LangString::new("Arguments")).with_id("p"))?;
    /// edit::apply(&mut doc, CreateClaim::new("p", LangString::new("C1")).with_id("c1"))?;
    /// let error = edit::apply(&mut doc, SetFlag::new("c1", Flag::IsCounter, true)).unwrap_err();
    /// assert!(matches!(
    ///     error,
    ///     EditError::FeatureNotApplicable { kind: ElementKind::Claim, feature: "isCounter", .. }
    /// ));
    /// assert_eq!(error.to_string(), "the Claim \"c1\" has no isCounter");
    /// # Ok::<(), EditError>(())
    /// ```
    #[non_exhaustive]
    FeatureNotApplicable {
        /// The element's id.
        element: ElementId,
        /// The element's kind.
        kind: ElementKind,
        /// The feature, by its XMI name: `name`, `isCounter`,
        /// `assertionDeclaration`, `content`, `key`, or the name of a
        /// reference end, such as `source`.
        feature: &'static str,
    },
    /// [`RemoveReference`] names an id the end does not hold (SACM-CMD-001).
    /// Ids are compared exactly, case included. Remove an id the end holds.
    ///
    /// ```
    /// use sacm::edit::{self, CreateArgumentPackage, CreateClaim, EditError, RemoveReference};
    /// use sacm::model::{Document, LangString, ReferenceEnd};
    ///
    /// let mut doc = Document::new();
    /// edit::apply(&mut doc, CreateArgumentPackage::root(LangString::new("Arguments")).with_id("p"))?;
    /// edit::apply(&mut doc, CreateClaim::new("p", LangString::new("C1")).with_id("c1"))?;
    /// let error = edit::apply(&mut doc, RemoveReference::new("c1", ReferenceEnd::MetaClaim, "m1")).unwrap_err();
    /// assert!(matches!(error, EditError::ReferenceNotHeld { end: "metaClaim", .. }));
    /// assert_eq!(error.to_string(), "the metaClaim holds no id \"m1\" to remove");
    /// # Ok::<(), EditError>(())
    /// ```
    #[non_exhaustive]
    ReferenceNotHeld {
        /// The end, by its XMI name, such as `source`.
        end: &'static str,
        /// The id, as given.
        id: ElementId,
    },
    /// [`ReorderElements`] names an id that is not a direct child of the
    /// parent, or, for [`ReorderElements::roots`], not a root
    /// (SACM-CMD-001): a grandchild, another element's child, the parent
    /// itself, the id of a name, or an id the document does not hold. The
    /// first such id is reported, in list order. Name only the parent's own
    /// children, or only roots.
    ///
    /// ```
    /// use sacm::edit::{self, CreateArgumentPackage, CreateClaim, EditError, ReorderElements};
    /// use sacm::model::{Document, LangString};
    ///
    /// let mut doc = Document::new();
    /// edit::apply(&mut doc, CreateArgumentPackage::root(LangString::new("Arguments")).with_id("p"))?;
    /// edit::apply(&mut doc, CreateClaim::new("p", LangString::new("C1")).with_id("c1"))?;
    /// let error = edit::apply(&mut doc, ReorderElements::roots(["c1"])).unwrap_err();
    /// assert!(matches!(error, EditError::ChildNotFound { parent: None, .. }));
    /// assert_eq!(error.to_string(), "the document has no root \"c1\"");
    /// # Ok::<(), EditError>(())
    /// ```
    #[non_exhaustive]
    ChildNotFound {
        /// The parent's id, or `None` for the roots.
        parent: Option<ElementId>,
        /// The id, as given.
        child: ElementId,
    },
    /// [`ReorderElements`] does not give one order (SACM-CMD-009): its list
    /// is empty, or names an element more than once, the first such id in
    /// list order. Name each child, or each root, to move exactly once.
    ///
    /// ```
    /// use sacm::edit::{self, CreateArgumentPackage, CreateClaim, EditError, ReorderElements};
    /// use sacm::model::{Document, ElementId, LangString};
    ///
    /// let mut doc = Document::new();
    /// edit::apply(&mut doc, CreateArgumentPackage::root(LangString::new("Arguments")).with_id("p"))?;
    /// edit::apply(&mut doc, CreateClaim::new("p", LangString::new("C1")).with_id("c1"))?;
    ///
    /// let empty = edit::apply(&mut doc, ReorderElements::new("p", Vec::<ElementId>::new())).unwrap_err();
    /// assert!(matches!(empty, EditError::InvalidOrder { repeated: None, .. }));
    /// assert_eq!(empty.to_string(), "the new order names no element");
    ///
    /// let twice = edit::apply(&mut doc, ReorderElements::new("p", ["c1", "c1"])).unwrap_err();
    /// assert!(matches!(twice, EditError::InvalidOrder { repeated: Some(ref id), .. } if id.as_str() == "c1"));
    /// assert_eq!(twice.to_string(), "the new order names \"c1\" more than once");
    /// # Ok::<(), EditError>(())
    /// ```
    #[non_exhaustive]
    InvalidOrder {
        /// The first id named more than once, or `None` for an empty list.
        repeated: Option<ElementId>,
    },
}

impl EditError {
    /// The catalog code of the refusal: SACM-CMD-001 for
    /// [`ParentNotFound`](Self::ParentNotFound), SACM-CMD-005 for
    /// [`InvalidParent`](Self::InvalidParent), SACM-ID-002 for
    /// [`InvalidId`](Self::InvalidId), SACM-CMD-004 for
    /// [`DuplicateId`](Self::DuplicateId), SACM-MULT-001 for
    /// [`TooFewReferences`](Self::TooFewReferences) and
    /// [`TooManyReferences`](Self::TooManyReferences), SACM-ID-002 for
    /// [`InvalidReference`](Self::InvalidReference), SACM-CMD-001 for
    /// [`ReferenceNotFound`](Self::ReferenceNotFound), SACM-REF-002 for
    /// [`WrongReferenceType`](Self::WrongReferenceType), SACM-CMD-001 for
    /// [`ElementNotFound`](Self::ElementNotFound),
    /// [`ReferenceNotHeld`](Self::ReferenceNotHeld) and
    /// [`ChildNotFound`](Self::ChildNotFound), SACM-CMD-008 for
    /// [`FeatureNotApplicable`](Self::FeatureNotApplicable) and SACM-CMD-009
    /// for [`InvalidOrder`](Self::InvalidOrder).
    #[must_use]
    pub fn code(&self) -> Code {
        match self {
            Self::ParentNotFound { .. }
            | Self::ReferenceNotFound { .. }
            | Self::ElementNotFound { .. }
            | Self::ReferenceNotHeld { .. }
            | Self::ChildNotFound { .. } => Code::CMD_TARGET_NOT_FOUND,
            Self::InvalidParent { .. } => Code::CMD_INVALID_PARENT,
            Self::InvalidId { .. } | Self::InvalidReference { .. } => Code::ID_INVALID,
            Self::DuplicateId { .. } => Code::CMD_DUPLICATE_ID,
            Self::TooFewReferences { .. } | Self::TooManyReferences { .. } => {
                Code::MULTIPLICITY_VIOLATION
            }
            Self::WrongReferenceType { .. } => Code::REF_WRONG_TYPE,
            Self::FeatureNotApplicable { .. } => Code::CMD_FEATURE_NOT_APPLICABLE,
            Self::InvalidOrder { .. } => Code::CMD_INVALID_ORDER,
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
            Self::TooManyReferences {
                end,
                given,
                allowed,
            } => write!(
                f,
                "the {end} is given {given} ids, and SACM allows at most {allowed}"
            ),
            Self::ElementNotFound { element } => write!(
                f,
                "the document has no element {:?} to edit",
                element.as_str()
            ),
            Self::FeatureNotApplicable {
                element,
                kind,
                feature,
            } => write!(f, "the {kind} {:?} has no {feature}", element.as_str()),
            Self::ReferenceNotHeld { end, id } => {
                write!(f, "the {end} holds no id {:?} to remove", id.as_str())
            }
            Self::ChildNotFound {
                parent: Some(parent),
                child,
            } => write!(
                f,
                "the element {:?} has no child {:?}",
                parent.as_str(),
                child.as_str()
            ),
            Self::ChildNotFound {
                parent: None,
                child,
            } => write!(f, "the document has no root {:?}", child.as_str()),
            Self::InvalidOrder { repeated: None } => f.write_str("the new order names no element"),
            Self::InvalidOrder {
                repeated: Some(repeated),
            } => write!(
                f,
                "the new order names {:?} more than once",
                repeated.as_str()
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
            Self::WrongReferenceType {
                end,
                id,
                kind,
                expected,
            } => write!(
                f,
                "the {end} {:?} names {} {kind}, and the {end} must be {} {expected}",
                id.as_str(),
                article(kind.name()),
                article(expected)
            ),
        }
    }
}

impl std::error::Error for EditError {}

/// The indefinite article before the SACM class name `name`: `an` before a
/// vowel, `a` otherwise.
fn article(name: &str) -> &'static str {
    if name.starts_with(['A', 'E', 'I', 'O', 'U']) {
        "an"
    } else {
        "a"
    }
}
