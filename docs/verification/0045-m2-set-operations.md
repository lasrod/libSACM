# Verification 0045 — Set, reference and reorder operations

- Date: 2026-11-15
- Row verdict: LIB-EDIT-008: PASS
- Row verdict: LIB-EDIT-009: PASS
- Row verdict: LIB-EDIT-010: PASS
- Row verdict: LIB-EDIT-012: PASS
- Re-checked, staying verified: LIB-EDIT-002, LIB-EDIT-003, LIB-DIAG-002, LIB-RT-003,
  SACM23-BASE-006
- Verifier: `sacm-verifier` (independent; read-only), 1 round; `sacm-docs-reviewer`, 1 round
- Commit: 2d4b5e1. Evidence:
  - CI run 43655486121 on 2d4b5e1: green on Linux, Windows and macOS. PR-diff mutants caught
    72, missed 0, and 42 were unviable.
  - The full `mutants.yml` run 43653926124 on b0f9f69 (same code; comments only changed since).
  - About 20 hand mutations.
  - A speed probe at 32k ids and children.
- Slice brief: [docs/slices/0045-m2-set-operations.md](../slices/0045-m2-set-operations.md)

## Result

The ten operations of ADR 0029 change an existing element:
- Every refusal leaves the document `==` to before, and the next generated ids unchanged.
- Each check order is pinned, including:
  - the pass-by-pass reference checks;
  - the per-end exception for held ids.
- `changed_ids` agrees with `semantic_compare`, and an edit that changes nothing writes nothing.
- Ids are kept by position. A freed value or reference id can be generated again, and a name's
  id stays in use.
- No edit adds a dangling or wrongly typed reference, or removes a name or key (LIB-EDIT-002,
  S5–S8).

Every quotation was checked against SACM 2.3 §8.2, §8.6, §8.7, §8.12, §11.9, §11.10,
§11.12, §11.13 and XMI 2.5.1 §9.5.2.

## Findings

| Severity | Row | Finding | Resolution |
|---|---|---|---|
| BLOCKING | LIB-EDIT-008 | CI mutants: `SetFlag::value -> true` survived. | A test pins `false` (2d4b5e1). |
| BLOCKING | docs | Docs review: a wrong bound condition, the wrong end in `TooFewReferences`, and no example of editing a relationship's ends (plus 10 smaller items). | All fixed (c844b2b). |
| FOLLOW-UP | LIB-EDIT-012 | `Document::set_text` is quadratic when a large content is replaced with an equally large one (32k values take 694 ms). | In `docs/follow-ups.md`. |
| FOLLOW-UP | LIB-EDIT-009, ADR 0029 | A stale line number in the triage, and two C++ line citations. | Fixed. |
