# Verification 0043 — Asserted relationships, reasoning and meta-claims

- Date: 2026-11-14
- Row verdict: SACM23-ARG-004: PASS
- Row verdict: SACM23-ARG-005: PASS
- Row verdict: SACM23-ARG-007: PASS
- Row verdict: SACM23-ARG-008: PASS
- Row verdict: SACM23-ARG-010: PASS
- Row verdict: SACM23-VAL-005: PASS
- Re-checked, staying verified: SACM23-XMI-001, -010, -014, -016, -017, LIB-COMPAT-001,
  -007, LIB-RT-002, SACM23-VAL-018
- Verifier: `sacm-verifier` (independent; read-only), 2 rounds; `sacm-docs-reviewer`, 1 round
- Commit: 081908e. Evidence: CI run 43593908549 on 081908e (PR-diff mutants: 100 caught,
  0 missed). Hand mutations: 47 in round 1 and 6 in round 2. A perturbation sweep of 4,000
  inputs in each of 2 modes had no panic. The two mutants ADR 0026 excluded for this row are
  killed, and their exclusions are removed.
- Slice brief: [docs/slices/0043-m2-asserted-relationships.md](../slices/0043-m2-asserted-relationships.md)

## Result

Asserted relationships, ArgumentReasoning and meta-claims follow ADR 0027 on ADR 0026's
references:
- `metaClaim` and `source` are multi-valued, kept in document order with repeats, and
  compared in order.
- `target`, `reasoning`, `structure` and `isCounter` are held on the kinds that own them.
- AssertedInference and AssertedContext are read, written in superclass-first attribute
  order, and created by edits. A create refuses no source, a reference id holding white
  space, and a reference naming no element.
- `validate` reports a missing source or target as SACM-MULT-001.

Every quotation was checked against SACM 2.3 §11.10-§11.16, ptc/22-03-13 and XMI 2.5.1.

## Findings

| Severity | Row | Finding | Resolution |
|---|---|---|---|
| BLOCKING | ARG-008/010 | Copilot: a create could reference an id holding white space, which a tolerant save split. | `EditError::InvalidReference` (SACM-ID-002), with tests. |
| BLOCKING | ARG-004, ARG-007 | Round 1: `dangling_references` was quadratic in one end's ids (160,000 ids took 30 s; a strict save runs validation). | A set per end, and 150,000-id tests. |
| BLOCKING | ARG-007, ARG-008/010 | Round 1: M09b (the declaration's element form on relationships) and M42 (merging adjacent repeated sources) survived. | Tests kill both. |
| BLOCKING | docs | Docs review: order-free comparison, unchecked reference types, the crate root's argument example, `Code` docs (plus 10 smaller items). | All fixed. |
| FOLLOW-UP | several | A "what points at this element" lookup; a stale mutant name in a test's doc. | In `docs/follow-ups.md`. |
