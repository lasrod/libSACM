# Verification 0048 — Atomic batches and deleting elements

- Date: 2026-11-15
- Row verdict: LIB-EDIT-004: PASS
- Row verdict: LIB-EDIT-005: PASS
- Row verdict: LIB-EDIT-006: PASS
- Row verdict: LIB-EDIT-007: PASS
- Row verdict: LIB-EDIT-014: PASS
- Verifier: `sacm-verifier` (independent; read-only)
- Commit: 467a0c8; evidence: CI run
  [43680791074](https://github.com/lasrod/libSACM/actions/runs/43680791074), all 16 checks
  green. PR-diff mutants: 95 caught, 0 missed, 58 unviable; an earlier run on 03590fe missed
  `BatchError`'s `Display`, which is now pinned.
- Slice brief: [docs/slices/0048-m2-batches-and-delete.md](../slices/0048-m2-batches-and-delete.md)

## Result

The delete rules, the dangling-reference list, the package crossings and the check order
match ADR 0031. Preview equals apply by construction. Revisions behave as specified, and
tests compare them only for equality.

The net ids of a batch take a shortcut: only the elements its operations touched are
compared. The verifier could not break it. The only per-element difference not carried
from step to step is the parent's child order.

Checks beyond the tests:
- **Hand mutations:** 39. Every behaviour mutant was killed; the three that survived are
  equivalent.
- **Round trips:** after each of 554 deletes, over every start document and all 12 policy
  combinations, the XMI written was deterministic and read back `==`.
- **Freed ids:** given again with `with_id` 1,140 times, with no `DuplicateId`.
- **Standard:** the spec multiplicities quoted were checked against the SACM 2.3 text.

## Findings

| Severity | Row | Finding | Resolution |
|---|---|---|---|
| blocking (docs review) | LIB-EDIT-005, LIB-EDIT-014 | The docs used GSN words in an example, promised undo, and promised that a freed id is never generated again. | Fixed. The maintainer chose to make no promise about reusing freed ids: a delete does not rewind the counter. |
| blocking (Copilot) | LIB-EDIT-005 | Scrub planning was quadratic in the sources of one relationship. | Fixed in 467a0c8. |
| follow-up | LIB-EDIT-007 | ADR 0031 and SACM-CMD-003 overclaimed when a preview of a clone is refused. | Reworded. |
| follow-up | LIB-EDIT-005 | The README said delete was not built. | Reworded. |
| follow-up | LIB-EDIT-003/005 | The mutants exclusion for `deleted_ids` is stale. | docs/follow-ups.md (with the M2 close run) |
| follow-up | LIB-EDIT-004 | Only `edit::preview`'s example previews and applies a delete. | docs/follow-ups.md |
