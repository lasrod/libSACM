//! Atomic batches of operations (LIB-EDIT-014, D-ATOMIC, ADR 0032).

use std::collections::HashSet;
use std::fmt;

use super::{EditError, Mutation, Operation};
use crate::compare::element_differs;
use crate::diagnostic::Code;
use crate::model::{Document, ElementId, SacmElement};

/// The operations of one [`batch`](super::batch), applied so far to a
/// private copy of the document.
///
/// [`apply`](Self::apply) applies one operation and answers as
/// [`edit::apply`](super::apply) would on the state so far, so a later
/// operation can use an id an earlier one generated.
/// [`document`](Self::document) shows that state. The batch commits when the
/// closure returns `Ok`, and otherwise the document is as it was.
///
/// # Examples
///
/// A refusal the closure ignores changes nothing, and the rest commits:
///
/// ```
/// use sacm::edit::{self, BatchError, CreateArgumentPackage, CreateClaim};
/// use sacm::model::{Document, LangString};
///
/// let mut doc = Document::new();
/// let batch = edit::batch(&mut doc, |tx| -> Result<(), BatchError> {
///     tx.apply(CreateArgumentPackage::root(LangString::new("Arguments")).with_id("p"))?;
///     let refused = tx.apply(CreateClaim::new("nowhere", LangString::new("C0"))).unwrap_err();
///     assert_eq!(refused.index(), 1);
///     tx.apply(CreateClaim::new("p", LangString::new("C1")))?;
///     assert_eq!(tx.document().element_count(), 2);
///     Ok(())
/// })?;
/// assert_eq!(batch.mutations().len(), 2);
/// assert_eq!(doc.element_count(), 2);
/// # Ok::<(), BatchError>(())
/// ```
#[derive(Debug)]
pub struct Batch<'d> {
    /// The private copy, with every accepted operation applied.
    document: &'d mut Document,
    /// The accepted operations' mutations, in order.
    mutations: Vec<Mutation>,
    /// How many times `apply` has been called.
    calls: usize,
}

impl Batch<'_> {
    /// Applies `operation` to the state so far, or refuses it and changes
    /// nothing, as [`edit::apply`](super::apply) does.
    ///
    /// A refusal the closure ignores changed nothing, so the batch still
    /// commits the other operations (ADR 0032). Pass it on with `?` to roll
    /// the whole batch back; the closure's error type then needs
    /// `From<BatchError>`.
    ///
    /// # Errors
    ///
    /// A [`BatchError`] holding the refusal [`edit::apply`](super::apply)
    /// would give, with the number of earlier `apply` calls of the batch as
    /// its [`index`](BatchError::index), and the operation.
    pub fn apply(&mut self, operation: impl Into<Operation>) -> Result<Mutation, BatchError> {
        let index = self.calls;
        self.calls = self.calls.saturating_add(1);
        match super::apply_operation(self.document, operation.into()) {
            Ok(mutation) => {
                self.mutations.push(mutation.clone());
                Ok(mutation)
            }
            Err((operation, error)) => Err(BatchError {
                index,
                operation,
                error,
            }),
        }
    }

    /// The document as the batch has changed it so far, for lookups and
    /// [`edit::check`](super::check).
    #[must_use]
    pub fn document(&self) -> &Document {
        self.document
    }
}

/// Runs `operations` on `document` as one change, committed exactly when
/// the closure returns `Ok`: the body of [`edit::batch`](super::batch).
pub(super) fn run<E>(
    document: &mut Document,
    operations: impl FnOnce(&mut Batch<'_>) -> Result<(), E>,
) -> Result<BatchMutation, E> {
    // A private copy, swapped in on success: an `Err` or a panic in the
    // closure leaves `document` as it was (ADR 0032).
    let (mut working, mutation) = run_on(document, document.clone(), operations)?;
    working.renew_revision();
    *document = working;
    Ok(mutation)
}

/// What [`run`] would return, run on a copy that is then dropped: the body
/// of [`edit::preview_batch`](super::preview_batch) (LIB-EDIT-024).
pub(super) fn preview<E>(
    document: &Document,
    operations: impl FnOnce(&mut Batch<'_>) -> Result<(), E>,
) -> Result<BatchMutation, E> {
    run_on(document, document.clone(), operations).map(|(_, mutation)| mutation)
}

/// Runs `operations` on `working`, a copy of `before`, and returns it with
/// the batch's mutation: the one code path of [`run`] and [`preview`].
fn run_on<E>(
    before: &Document,
    mut working: Document,
    operations: impl FnOnce(&mut Batch<'_>) -> Result<(), E>,
) -> Result<(Document, BatchMutation), E> {
    let mut batch = Batch {
        document: &mut working,
        mutations: Vec::new(),
        calls: 0,
    };
    operations(&mut batch)?;
    let mutations = batch.mutations;
    let net = NetIds::of(before, &working, &mutations);
    let mutation = BatchMutation {
        mutations,
        created: net.created,
        changed: net.changed,
        deleted: net.deleted,
    };
    Ok((working, mutation))
}

/// What a committed batch did: each accepted operation's [`Mutation`], and
/// the net ids the whole batch created, changed and deleted.
///
/// The net ids are what [`semantic_compare`](crate::compare::semantic_compare)
/// of the document before and after the batch reports (ADR 0032), computed
/// from the ids the batch touched rather than by comparing whole documents:
/// - [`created_ids`](Self::created_ids): in the document after and not
///   before, in creation order;
/// - [`changed_ids`](Self::changed_ids): in both, with a difference in the
///   element itself, in pre-order after;
/// - [`deleted_ids`](Self::deleted_ids): in the document before and not
///   after, in pre-order before.
///
/// So an element created and deleted in one batch is in none of them, and
/// one deleted and created again with its id is at most changed, and its
/// parent is changed if its position among the children moved.
///
/// One `BatchMutation` is one change in an undo history. It records ids, not
/// the old content, so to undo it keep the document from before
/// ([`Document::clone`]). For undo and redo, see
/// [the `edit` docs](crate::edit#undo-and-redo) (LIB-EDIT-011).
///
/// # Examples
///
/// ```
/// use sacm::edit::{self, CreateArgumentPackage, CreateClaim, DeleteElement, Operation, SetName};
/// use sacm::model::{Document, ElementId, LangString};
///
/// let mut doc = Document::new();
/// edit::apply(&mut doc, CreateArgumentPackage::root(LangString::new("Arguments")).with_id("p"))?;
/// let batch = edit::apply_all(&mut doc, [
///     Operation::from(CreateClaim::new("p", LangString::new("Temporary")).with_id("tmp")),
///     Operation::from(DeleteElement::new("tmp")),
///     Operation::from(SetName::new("p", LangString::new("Renamed"))),
/// ])?;
/// assert_eq!(batch.mutations().len(), 3);
/// // `tmp` came and went: net, only `p` changed.
/// assert!(batch.created_ids().is_empty() && batch.deleted_ids().is_empty());
/// assert_eq!(batch.changed_ids(), [ElementId::from("p")]);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
pub struct BatchMutation {
    mutations: Vec<Mutation>,
    #[cfg_attr(feature = "serde", serde(rename = "created_ids"))]
    created: Vec<ElementId>,
    #[cfg_attr(feature = "serde", serde(rename = "changed_ids"))]
    changed: Vec<ElementId>,
    #[cfg_attr(feature = "serde", serde(rename = "deleted_ids"))]
    deleted: Vec<ElementId>,
}

impl BatchMutation {
    /// The [`Mutation`] of every accepted operation, in order. A refused
    /// operation the closure ignored is not here.
    #[must_use]
    pub fn mutations(&self) -> &[Mutation] {
        &self.mutations
    }

    /// The ids of the elements the batch created, net: in the document after
    /// and not before, in the order the batch first created them.
    #[must_use]
    pub fn created_ids(&self) -> &[ElementId] {
        &self.created
    }

    /// The ids of the elements the batch changed, net: in the document
    /// before and after, with a difference in the element itself, in
    /// [pre-order](crate::model::Document::elements) after.
    #[must_use]
    pub fn changed_ids(&self) -> &[ElementId] {
        &self.changed
    }

    /// The ids of the elements the batch deleted, net: in the document before
    /// and not after, in [pre-order](crate::model::Document::elements)
    /// before.
    #[must_use]
    pub fn deleted_ids(&self) -> &[ElementId] {
        &self.deleted
    }

    /// Whether the batch changed nothing, net: it created, changed and
    /// deleted no id.
    ///
    /// [`mutations`](Self::mutations) may still hold some, such as a create
    /// and a delete of one element, and a committed batch still gives the
    /// document a new revision. A reorder of the roots lists no id, so a
    /// batch that only reorders the roots is empty although the document
    /// changed.
    ///
    /// # Examples
    ///
    /// ```
    /// use sacm::edit::{self, BatchError, CreateArgumentPackage, CreateClaim, DeleteElement};
    /// use sacm::model::{Document, LangString};
    ///
    /// let mut doc = Document::new();
    /// edit::apply(&mut doc, CreateArgumentPackage::root(LangString::new("Arguments")).with_id("p"))?;
    /// let batch = edit::batch(&mut doc, |tx| -> Result<(), BatchError> {
    ///     tx.apply(CreateClaim::new("p", LangString::new("Temporary")).with_id("tmp"))?;
    ///     tx.apply(DeleteElement::new("tmp"))?;
    ///     Ok(())
    /// })?;
    /// assert_eq!(batch.mutations().len(), 2);
    /// assert!(batch.is_empty());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.created.is_empty() && self.changed.is_empty() && self.deleted.is_empty()
    }
}

/// Why a batch refused an operation: its index among the batch's
/// [`Batch::apply`] calls, counting from 0, the operation and its
/// [`EditError`].
///
/// [`edit::apply_all`](super::apply_all) returns it for the first refusal,
/// and [`Batch::apply`] for each. `Display` writes one line, and
/// [`source`](std::error::Error::source) is the `EditError`.
///
/// # Examples
///
/// ```
/// use sacm::diagnostic::Code;
/// use sacm::edit::{self, CreateArgumentPackage, CreateClaim, Operation};
/// use sacm::model::{Document, LangString};
///
/// let mut doc = Document::new();
/// let operations = [
///     Operation::from(CreateArgumentPackage::root(LangString::new("Arguments")).with_id("p")),
///     Operation::from(CreateClaim::new("nowhere", LangString::new("C1"))),
/// ];
/// let error = edit::apply_all(&mut doc, operations).unwrap_err();
/// assert_eq!((error.index(), error.code()), (1, Code::CMD_TARGET_NOT_FOUND));
/// assert_eq!(doc.element_count(), 0); // the whole batch was rolled back
/// assert_eq!(
///     error.to_string(),
///     "operation 1 (CreateClaim) of the batch was refused: the document has no element \"nowhere\" to be the parent"
/// );
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
pub struct BatchError {
    index: usize,
    operation: Box<Operation>,
    error: EditError,
}

impl BatchError {
    /// The refused operation's index: the number of [`Batch::apply`] calls
    /// before it, refused ones included, or its position in the list given
    /// to [`edit::apply_all`](super::apply_all).
    #[must_use]
    pub fn index(&self) -> usize {
        self.index
    }

    /// The refused operation, exactly as given.
    #[must_use]
    pub fn operation(&self) -> &Operation {
        &self.operation
    }

    /// The refusal, as [`edit::apply`](super::apply) gives it.
    #[must_use]
    pub fn error(&self) -> &EditError {
        &self.error
    }

    /// The refusal's catalog code: [`EditError::code`] of
    /// [`error`](Self::error).
    #[must_use]
    pub fn code(&self) -> Code {
        self.error.code()
    }
}

impl fmt::Display for BatchError {
    /// Writes the index, the operation's name and the refusal on one line.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "operation {} ({}) of the batch was refused: {}",
            self.index,
            self.operation.operation_name(),
            self.error
        )
    }
}

impl std::error::Error for BatchError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

/// The net ids of a batch from `before` to `after`.
struct NetIds {
    created: Vec<ElementId>,
    changed: Vec<ElementId>,
    deleted: Vec<ElementId>,
}

impl NetIds {
    /// The net ids of the batch whose accepted operations returned
    /// `mutations`, from `before` to `after`, computed over the ids the
    /// batch touched (ADR 0032 Consequences).
    ///
    /// An element in both documents can differ only if an operation reported
    /// it, or if it is the parent of an element the batch created or
    /// deleted: an element deleted and created again with its id may stand
    /// elsewhere among its parent's children. It may also be the parent,
    /// before or after the batch, of an element an operation reported as
    /// changed: an element moved away and back changes only its parent's
    /// order (ADR 0036 D4). A created element's parent is
    /// looked up after the batch and a deleted one's before it: an id that
    /// is in the other document too was also deleted or created again, and
    /// that other mutation names it.
    fn of(before: &Document, after: &Document, mutations: &[Mutation]) -> Self {
        let mut seen: HashSet<&ElementId> = HashSet::new();
        let mut created = Vec::new();
        for id in mutations.iter().flat_map(Mutation::created_ids) {
            if after.contains(id) && !before.contains(id) && seen.insert(id) {
                created.push(id.clone());
            }
        }

        let gone: HashSet<&ElementId> = mutations
            .iter()
            .flat_map(Mutation::deleted_ids)
            .filter(|id| before.contains(*id) && !after.contains(*id))
            .collect();
        let deleted = in_pre_order(before, &gone);

        let mut touched: HashSet<&ElementId> = HashSet::new();
        for mutation in mutations {
            touched.extend(mutation.changed_ids());
            // A changed element may have moved, which may change the order or
            // the preserved placement of its parent before and after (ADR
            // 0036 D4): a move away and back reports only that.
            for id in mutation.changed_ids() {
                for document in [before, after] {
                    if let Some(parent) = document.get(id).and_then(SacmElement::parent) {
                        touched.insert(parent.id());
                    }
                }
            }
            let created = mutation.created_ids().iter().map(|id| (id, after));
            let deleted = mutation.deleted_ids().iter().map(|id| (id, before));
            for (id, document) in created.chain(deleted) {
                touched.insert(id);
                if let Some(parent) = document.get(id).and_then(SacmElement::parent) {
                    touched.insert(parent.id());
                }
            }
        }
        let differing: HashSet<&ElementId> = touched
            .into_iter()
            .filter(|id| element_differs(before, after, id))
            .collect();
        let changed = in_pre_order(after, &differing);

        Self {
            created,
            changed,
            deleted,
        }
    }
}

/// The ids of `ids` that `document` holds, in its pre-order.
fn in_pre_order(document: &Document, ids: &HashSet<&ElementId>) -> Vec<ElementId> {
    if ids.is_empty() {
        return Vec::new();
    }
    document
        .elements()
        .map(SacmElement::id)
        .filter(|id| ids.contains(id))
        .cloned()
        .collect()
}
