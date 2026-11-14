# Verification 0041 — Validation rules for gid, descriptions and package homogeneity

- Date: 2026-11-14
- Row verdict: SACM23-VAL-002: PASS
- Row verdict: SACM23-VAL-011: PASS
- Row verdict: SACM23-VAL-012: PASS
- Row verdict: LIB-DIAG-002: PASS
- Re-checked, staying verified: LIB-RT-003, LIB-COMPAT-007, SACM23-VAL-010, SACM23-XMI-016,
  LIB-CLI-002, LIB-CLI-003, SACM23-VAL-018
- Verifier: `sacm-verifier` (independent; read-only), 1 round
- Commit: d5402f9. Evidence: CI run 43565420388 on eef426b (PR-diff mutants: 16 caught,
  0 missed); d5402f9 changes only a test file and the matrix. Hand mutations: 40.
- Slice brief: [docs/slices/0041-m2-validation-rules.md](../slices/0041-m2-validation-rules.md)

## Result

`validate` reports:
- a duplicate non-empty `gid` as SACM-ID-003, an Error (§8.2; the indicative counts as
  "must", D-SEV);
- more than one description as a SACM-MULT-001 Warning (D-DESC-MULT);
- a plain ArgumentPackage that mixes nested plain packages with other argumentation
  content as SACM-PKG-001 (§11.4, D-PKG-HOMOG).

A test now keeps the diagnostic catalog's live rows and the `Code` constants in step, in
both directions. Every quotation was checked against formal/23-05-08.

Older test inputs that broke the new rules were changed, never their assertions, and each
still covers its row.

## Findings

| Severity | Row | Finding | Resolution |
|---|---|---|---|
| FOLLOW-UP | LIB-DIAG-002 | Copilot: the scanner exempted every non-literal `from_static` in diagnostic.rs. | Narrowed to the two constructor contexts, with a test. |
| FOLLOW-UP | LIB-DIAG-002 | A private tuple construction `Code("…")` in diagnostic.rs is not scanned. | In `docs/follow-ups.md`. |
| FOLLOW-UP | SACM23-VAL-002 | The difference from C++ was unrecorded. | ADR 0018 amendment 1 (V7, V8). |
| FOLLOW-UP | SACM23-VAL-010, -012 | The VAL-010 own-test weakened; D-PKG-HOMOG owner exemption to re-examine. | In `docs/follow-ups.md`. |
