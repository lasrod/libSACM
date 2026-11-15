---
slice: 0048
name: m2-batches-and-delete
state: done
step: close
rows: [LIB-EDIT-014, LIB-EDIT-004, LIB-EDIT-005, LIB-EDIT-006, LIB-EDIT-007]
mode: pr
base: main
---

# Slice 0048 — Atomic batches and deleting elements

## Scope

The remaining M2 edit rows:
- **LIB-EDIT-014 (GitHub issue #59):** several operations apply as one atomic change.
- **LIB-EDIT-004:** a delete preview lists the element, everything it contains, and every
  relationship and reference that would dangle, without changing the document.
- **LIB-EDIT-005 and LIB-EDIT-006:** deleting needs an explicit policy, for a referenced
  element (SACM-CMD-002) and for a non-empty package or cross-package references
  (SACM-CMD-006, SACM-CMD-007). It never leaves a dangling reference.
- **LIB-EDIT-007:** a preview taken before the document changed is refused (SACM-CMD-003).

Left out: undo (LIB-EDIT-011, M7), which needs these rows.

## Basis

D-ATOMIC, D-PREVIEW, D-POLICY, D-RESULT, D-VAL-LAYER; ADR 0019, ADR 0026, ADR 0028 and
ADR 0029. The C++ library's delete and preview commands, where the standard is silent.

## Design

- [ADR 0031](../adr/0031-delete-previews-and-revisions.md) and
  [ADR 0032](../adr/0032-atomic-batches.md), accepted by the maintainer on 2026-11-15.
- **Delete:** `DeleteElement::new(id)` with `with_reference_policy`
  (`RejectIfReferenced` | `DeleteReferencingRelationships` | `ScrubReferences`),
  `with_package_policy` (`RejectIfNonEmpty` | `DeleteRecursively`) and
  `with_cross_package_policy` (`RejectIfExternalReferencesExist` | `ApplyReferencePolicy`).
- **Preview:** `edit::preview(&Document, &Operation) -> Result<Preview, EditError>`;
  `Preview::{revision, operation, mutation, dangling_references}`; `DanglingReference::{holder,
  end, id, crosses_package}`; `edit::apply_preview(&mut Document, &Preview)`.
- **Revision:** `Document::revision() -> Revision`, opaque and unique per state in the
  process.
- **Batches:** `edit::batch(&mut Document, |tx: &mut Batch| -> Result<(), E>) ->
  Result<BatchMutation, E>`, `Batch::{apply, document}`, `edit::apply_all(&mut Document,
  ops) -> Result<BatchMutation, BatchError>`.
- **New `EditError` variants:** `Referenced` (CMD-002), `PreviewExpired` (CMD-003),
  `NotEmpty` (CMD-006) and `CrossPackageReference` (CMD-007).

## Log

- 2026-11-15 choose: the remaining M2 edit rows. Batches unblock the GSN layer (#59), and
  delete unblocks undo (LIB-EDIT-011).
- 2026-11-15 design: the architect's brief; ADRs 0031 and 0032 accepted. The maintainer
  chose three reference policies (with scrub), a revision unique per state, a batch that
  commits around a swallowed refusal, freed ids never regenerated, and a write-only
  `Preview`. All five rows `analyzed`.
- 2026-11-15 tests: two test engineers in parallel. delete.rs and preview.rs (LIB-EDIT-004
  to -007, with the deletes added to the edit_atomic set and an independent ADR 0031 oracle),
  and batch.rs with the serde samples (LIB-EDIT-014; seeded pseudo-random sequences).
  105 tests cited; red confirmed (missing API only). The lead settled the readings: created
  ids in creation order, refusal order is dangling-list order, `NotEmpty` counts at any
  depth, a preview writes three keys, dangling means outside the target's subtree, and a
  crossing is judged against the named element. Rows `tests-failing`.
- 2026-11-15 implementation: done. The test engineer fixed four test-side problems (an
  invalid nested-package fixture, the catalog test's code lists, clippy lints, and a boxed
  W3C test variant because `Document` grew). Docs review: three blocking findings (GSN words
  in an example, an undo promise, and "a freed id is never generated again"), plus nine
  follow-ups, all fixed. The maintainer chose not to promise anything about reusing freed
  ids: a delete does not rewind the counter, and an application that needs ids unique
  across its history gives its own (ADR 0031 and LIB-EDIT-005 reworded).
- 2026-11-15 implementation: full gate green. Rows `implemented`.
- 2026-11-15 verification: PR #65. The PR-diff mutants missed `BatchError`'s `Display`
  (now pinned). Copilot found the scrub planning quadratic; fixed in 467a0c8 (20,000
  sources: 13.4 s to 45 ms). CI on 467a0c8 green: 95 caught, 0 missed.
- 2026-11-15 verification: PASS for all five rows (record 0048). Wording follow-ups fixed;
  the stale mutants exclusion and fuller delete examples are in docs/follow-ups.md. Rows
  `verified`.
