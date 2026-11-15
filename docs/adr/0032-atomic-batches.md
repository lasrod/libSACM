# 0032. Atomic batches of operations

- Status: accepted (the maintainer, 2026-11-15)
- Date: 2026-11-15
- Matrix rows: LIB-EDIT-014. It extends ADR 0019 (whose Consequences anticipate batches),
  ADR 0030 and ADR 0031.

## Context

One user action in a GSN editor is often several SACM operations (GitHub issue #59). For
example, "add a strategy under this goal" creates an ArgumentReasoning and an
AssertedInference that names it. The user must see all of it or none of it, and undo must
treat it as one step. A later operation needs an id that an earlier one generated.

## Decision

- **`edit::batch(&mut Document, |tx| …) -> Result<BatchMutation, E>`** is the primary form.
  - `Batch::apply(operation)` returns each operation's `Mutation`, so a later operation can
    use an id an earlier one generated.
  - `Batch::document()` shows the state so far, for lookups and `edit::check`.
- **The batch commits exactly when the closure returns `Ok`.**
  - An `Err(e)` from the closure is returned unchanged, and the document is as before.
  - A refusal that the closure catches and ignores changed nothing (D-ATOMIC), so the other
    operations still commit (the maintainer, 2026-11-15).
  - `E` needs `From<BatchError>` only if the closure uses `?` on `tx.apply`.
- **`edit::apply_all(&mut Document, operations) -> Result<BatchMutation, BatchError>`** is
  `batch` over a list, for operations sent as data, such as a JSON array.
- **Rollback.**
  - On a refusal or an `Err`, the document is `==` to before, with the same next ids and the
    same revision.
  - The first implementation works on a private copy and swaps it in on success
    (`Document::clone` takes 1.4 ms at 5,000 elements), so a panic in the closure leaves the
    document as before.
  - The mechanism is private and may change.
- **`BatchMutation`.**
  - `mutations()` holds every accepted `Mutation`, in order.
  - Its net `created_ids`, `changed_ids` and `deleted_ids` are what
    `semantic_compare(before, after)` reports, as defined for LIB-EDIT-003: created ids in
    creation order, changed ids in document pre-order after, deleted ids in pre-order
    before. An element created and then deleted inside the batch is in neither list.
- **`BatchError`** gives the refused operation's index (counting `tx.apply` calls from 0),
  the operation, its `EditError` and its `code()`. `source()` is the `EditError`.
- **No batch check or preview in this release.** To check a list, run `apply_all` on a
  clone. Both can be added later without a break.
- **serde** (ADR 0030):
  - `BatchMutation` serializes as `{"mutations", "created_ids", "changed_ids",
    "deleted_ids"}`.
  - `BatchError` serializes as `{"index", "operation", "error"}`.
  - `Batch` is not serialized.
- **Undo.** One `BatchMutation` is one undo step (LIB-EDIT-011).

**Difference from C++:**
- **B1.** C++ has no batches. Assurance Forge applies the operations to a clone and swaps it
  in.

## Consequences

The net ids of a large batch should be computed over the ids the batch touched, not by a
full `semantic_compare`, which is super-linear at 20,000 elements. That comparison stays
the tests' oracle.

## Alternatives considered

- **Only a list of operations.** A later operation could not use a generated id without a
  new id API.
- **Poisoning the batch on any refusal.** That rule is stricter but harder to explain. The
  refused operation changed nothing anyway.
