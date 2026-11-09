# Verification 0013, round 2 — m1-document-model

- Date: 2026-11-09
- Row verdict: LIB-API-003: PASS
- Row verdict: LIB-API-002: FAIL
- Verifier: `sacm-verifier` (independent; read-only), at full depth
- Commit: `d2dd904`. Evidence: ci.yml 42627316101 and mutants.yml 42627315911, both success. In `src/model`, 50 mutants caught, 17 unviable, 0 missed. Copilot recommends approval.
- Slice brief: [docs/slices/0013-m1-document-model.md](../slices/0013-m1-document-model.md)

## Result

- **LIB-API-003:** per-variant names are pinned. M1–M5, M16, M17 and four more swaps are killed.
- **LIB-API-002:** exact id preservation (M13–M15) and exact duplicate detection (M18b, M18c) are killed. But the same D-ID obligation also covers `contains` and the parent lookup in `insert`, and those are not demonstrated. The verifier names this as its own round 1 miss.
- **No behaviour change** since round 1. The docs and examples are correct.

## Findings

| Severity | Row | Finding | Required fix |
|---|---|---|---|
| High | LIB-API-002 (item 2, D-ID) | `contains` is never queried with near-miss ids. X7 (trimmed query), X9 (a query extending an id) and X10 (non-ASCII stripped) survive. | Query `contains` with the near-miss ids of `get_of_an_unknown_id_is_none` plus `claim_topé`, and with every normalised form in `keeps_inserted_ids_exactly`. |
| High | LIB-API-002 (item 7, D-ID) | The parent lookup is not shown to be exact. X12 (non-ASCII stripped) and X14 (leading space trimmed) survive. | Add ` arguments` and `argumentsé` to the unknown-parent controls. |
| Low | docs | The first "aims to provide" bullet in `lib.rs` lacks "(planned)". | Mark it planned. |

Logged, not required:
- the `InsertError` variant per refusal, and whitespace-only ids;
- packaging: the doctests that call `insert` need `test-support`, which the path-only self dev-dependency does not supply in a published crate;
- the "document order" wording, which should cover insertion order.
