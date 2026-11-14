# Verification 0040 — Descriptions, notes, tagged values and gid

- Date: 2026-11-14
- Row verdict: SACM23-BASE-004: PASS
- Row verdict: SACM23-BASE-005: PASS
- Row verdict: SACM23-BASE-006: PASS
- Row verdict: SACM23-VAL-010: PASS
- Re-checked, staying verified: SACM23-XMI-001, -004, -010, -014, -016, SACM23-VAL-018,
  SACM23-VAL-020, LIB-RT-001, LIB-RT-002, and the moved samples under LIB-API-009,
  LIB-SEC-002, SACM23-VAL-001, LIB-COMPAT-001 and SACM23-XMI-002
- Verifier: `sacm-verifier` (independent; read-only), 2 rounds; `sacm-docs-reviewer`, 1 round
- Commit: 17af49d. Evidence: CI run 43547000150 on e2078ec (PR-diff mutants: 181 caught,
  0 missed); round 2 added only tests and matrix text. Hand mutations: 53 in round 1, 3 in
  round 2. A sweep of 20,000 perturbed fixtures had no panic and no loss.
- Slice brief: [docs/slices/0040-m2-descriptions-and-gid.md](../slices/0040-m2-descriptions-and-gid.md)

## Result

Description, ImplementationConstraint, Note and TaggedValue are read, written, compared
and created on every ModelElement, in document order (ADR 0025).
- Their `content`, and a TaggedValue's `key`, are absent or a MultiLangString, kept in order
  with duplicates.
- Their role and value `xmi:id`s are kept by position, in the file's id space.
- `gid` is held exactly on every element.
- `validate` reports an empty MultiLangString, a repeated language and a key-less
  TaggedValue as SACM-MULT-001.

Every quotation was checked against SACM 2.3 §8.2-§8.12, ptc/22-03-13, and XMI 2.5.1 §7.8
and Annex B.5.2.

## Findings

| Severity | Row | Finding | Resolution |
|---|---|---|---|
| BLOCKING | BASE-004/005, VAL-020 | PR-diff mutants: 19 missed (multi-value `Display`, indentation, value position, the kinds the shorthand checks apply to, `Create*` accessors). | Pinned by 10 tests. |
| BLOCKING | BASE-004/005 | M50: the `RepeatedValue` property of a value field was unpinned. | Exact strings asserted for content and key. |
| BLOCKING | XMI-001, -016, LIB-RT-002 | Still described utility children and `gid` as unread. | Reworded. |
| BLOCKING | docs | Docs review: wrong clause numbers, the two `content`s unexplained, a stale comment (plus 15 smaller items). | All fixed. |
| FOLLOW-UP | VAL-020 | Copilot: an id finding's subject was allocated for valid ids. | Fixed. |
| FOLLOW-UP | LIB-COMPAT-008, D-TEXT-MODEL | The shorthand forms deferred to it; the text direction of §8.5 and §8.7. | Widened and recorded. |
| FOLLOW-UP | LIB-RT-002 and API ideas | The acceptance list of fixture names; ergonomics ideas from the docs review. | In `docs/follow-ups.md`. |
