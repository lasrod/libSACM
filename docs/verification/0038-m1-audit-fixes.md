# Verification 0038 — M1 audit fixes

- Date: 2026-11-13
- Row verdict: LIB-RT-002: PASS
- Row verdict: LIB-CLI-003: PASS
- Row verdict: LIB-SEC-002: PASS
- Row verdict: SACM23-XMI-023: PASS
- Verifier: `sacm-verifier` (independent; read-only), 3 rounds (rounds 2 and 3 checked only
  the matrix wording fixes)
- Commit: the slice commit on `slice/0038-m1-audit-fixes`, on top of 767cc51. Evidence: the
  full gate run locally; no library source changed, so there is no new mutants report. The
  full mutants run on `main` at 767cc51 (run 43426237212) gave 1063 caught, 0 missed and 0
  timeouts.
- Slice brief: [docs/slices/0038-m1-audit-fixes.md](../slices/0038-m1-audit-fixes.md)

## Result

LIB-RT-002 now says what its test does: the fixture round trip writes in tolerant mode. Three
new tests close the M1 audit's gaps:
- `roundtrip --strict` gives the same exit code, stdout and `-o` bytes as without it;
- a 100,000-deep readable package chain reads and drops on a 2 MiB stack;
- every read diagnostic is located at its attribute's name (SACM23-XMI-023).

Hand mutations show that each new test catches its regression: a recursive pass over the
read document, `roundtrip --strict` failing on a read Error, and a diagnostic losing its
location. The other matrix edits (dependencies, M2 rows, wording) and the docs edits were
checked against the code and ADRs.

## Findings

| Severity | Row | Finding | Resolution |
|---|---|---|---|
| BLOCKING | LIB-DIAG-001 | The reworded acceptance cited a check by LIB-DIAG-002, which is planned. | Reworded: "the check in both directions is LIB-DIAG-002 (planned)". |
| BLOCKING | LIB-API-004 | Item 1 credited SACM23-ARG-003 with `SetAssertionDeclarationError`. | Paired with SACM23-ARG-002. |
| FOLLOW-UP | LIB-SEC-002 | `code` did not list the content reader and the document. | Added `xmi/roots.rs` and `model/document.rs`. |
| FOLLOW-UP | several | The deep-chain test deadline, a strict case for SACM23-XMI-004, LIB-API-004 items 3-4, CHANGELOG and architecture.md attributions, and the `errors.rs` module doc. | In `docs/follow-ups.md`. |
