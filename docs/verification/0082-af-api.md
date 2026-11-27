# Verification 0082 — The additive API Assurance Forge asked for

- Date: 2026-11-27
- Row verdict: LIB-API-013: PASS
- Row verdict: LIB-API-021: PASS
- Row verdict: LIB-EDIT-023: PASS
- Row verdict: LIB-EDIT-024: PASS
- Verifier: `sacm-verifier` (independent; read-only)
- Commit: 341d5f3 (first round 24eb9e8); evidence: CI 45922605472 (341d5f3); hand mutations, logs `target/verify-0082/mutants*.log` and `r2-mutants.log`
- Slice brief: [docs/slices/0082-af-api.md](../slices/0082-af-api.md)

## Result

The verifier ran the gate, compared the tests with the red snapshot, checked the SACM
quotations and every one of the 44 `set_clause` sites, and applied hand mutants to the new
decision logic. SetTaggedValue is atomic and follows the designed check order; a batch
preview leaves the document, its revision and its id counter unchanged; the JSON matches the
goldens and ADR 0030 amendment 7. In the first round LIB-API-013 failed on three test gaps;
the fix round added tests that kill every listed mutant, with no change to production code.

## Findings

| Blocking / non-blocking | Row | Finding | Resolution |
|---|---|---|---|
| Blocking | LIB-API-013 | Ten emit sites could name the wrong specification unnoticed. | A test over every diagnostic template input, and a repeated-href input for SACM23-VAL-022. |
| Blocking | LIB-API-013 | `BatchMutation::is_empty` was not tested per list. | Create-only and delete-only batches. |
| Blocking | LIB-API-013 | A bad `lang`'s position was tested only at index 0. | A bad lang at index 1. |
| Non-blocking | LIB-API-013 | CHANGELOG: "exact language" was inexact; `==` of old records was not mentioned. | Fixed with the verifier's text. |
| Non-blocking | LIB-EDIT-023, LIB-API-013 | Two equivalent mutants: the duplicate create-path expression check, and `#[serde(default)]` on an `Option`. | None; triage as equivalent if CI reports them. |
