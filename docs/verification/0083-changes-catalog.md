# Verification 0083 — Changes between documents, and the catalog as JSON

- Date: 2026-11-27
- Row verdict: LIB-RT-005: PASS
- Row verdict: LIB-DIAG-006: PASS
- Verifier: `sacm-verifier` (independent; read-only)
- Commit: a3bab18 (first round 24143e0); evidence: CI 45942866496 (24143e0, tests green), CI 45946433911 (a3bab18); hand mutations, logs `target/verify-0083/mutants.log` and `r2/`
- Slice brief: [docs/slices/0083-changes-catalog.md](../slices/0083-changes-catalog.md)

## Result

The verifier ran the gate, compared the tests with the red snapshot, rebuilt the catalog with
its own parser (217 templates, 46 codes, carriers and severities equal), confirmed the file
ships and that the staleness test fails on a changed template, and applied hand mutants.
`changes` shares `compare_document` and `compare_element` with `semantic_compare`, is linear
and does not recurse. In the first round both rows failed on test gaps: document preserved
content in `changes`, and the generator's refusals; the fix round added tests only.

## Findings

| Blocking / non-blocking | Row | Finding | Resolution |
|---|---|---|---|
| Blocking | LIB-RT-005 | `document_changed` could ignore document preserved content unnoticed. | Two preserved-content cases. |
| Blocking | LIB-DIAG-006 | The generator's documented refusals were untested; it could write a partial catalog. | Tests for every refused and skipped input; the live-row rule as a traced test. |
| Non-blocking | LIB-RT-005, LIB-DIAG-006 | Wording: "the order of a batch's net lists", the `CATALOG_JSON` summary, the module docs, a long Cargo.toml line. | Fixed with the verifier's text. |
| Non-blocking | LIB-DIAG-006 | The `row` argument of four templates is unused in their English. | docs/follow-ups.md. |
| Non-blocking | LIB-DIAG-006 | Three parser mutants survive (catalog.rs 350:20, 378:18, 428:38); only input the macro already rejects reaches them. | docs/follow-ups.md: kill with three malformed-entry cases. |
