---
slice: 0034
name: m1-atomic-edits-and-results
state: done
step: verified
rows: [LIB-EDIT-002, LIB-EDIT-003]
mode: pr
base: slice/0033-m1-no-xmi-version (PR #44, merged); the PR targets main
---

# Slice 0034 — Atomic edits and edit results

## Scope

Every `edit::apply` either succeeds and leaves the document structurally valid, or fails
and leaves it unchanged. "Unchanged" means semantically equal (`==`, ADR 0020), with the
same next generated id (LIB-EDIT-002). A successful edit returns what an audit log needs: the
ids created, changed and deleted, and the operation that ran (LIB-EDIT-003). Undo itself
is LIB-EDIT-011.

## Basis

Quality and API rows: acceptance by the lead. D-ATOMIC, D-RESULT, D-VAL-LAYER; ADR 0019;
ADR 0020.

## Design

ADR 0019 amendment 1 (accepted). `Mutation` gains `operation() -> &Operation`, and
`changed_ids()` and `deleted_ids()` (both empty today), alongside `created_ids()`.
`Operation::operation_name() -> &'static str` is added. The three id lists are defined as
`semantic_compare(before, after)`'s right-only, per-element and left-only ids.

## Log

- 2026-11-13 choose: the next M1 rows. Semantic comparison (slice 0031) makes "unchanged"
  checkable.
- 2026-11-13 design: ADR 0019 amendment 1 accepted by the maintainer.
- 2026-11-13 verification: two equivalent accessor mutants excluded. Round 1 PASS for
  LIB-EDIT-003, FAIL for LIB-EDIT-002 on one false rustdoc sentence, fixed in 26ddea9.
  Round 2 PASS (record 0034). M1 41/48.
