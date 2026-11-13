# Verification 0035 — Load and save modes

- Date: 2026-11-13
- Row verdict: LIB-COMPAT-001: PASS
- Row verdict: LIB-COMPAT-007: PASS
- Row verdict: SACM23-XMI-001: PASS
- Row verdict: SACM23-XMI-002: PASS
- Row verdict: SACM23-XMI-024: PASS
- Row verdict: LIB-API-010: PASS
- Verifier: `sacm-verifier` (independent; read-only), 1 round plus a re-check of one requirement line; `sacm-docs-reviewer`, 1 round
- Commit: 3a2f4af (code); the docs fixes after it change no code. Evidence: CI run 43386895132 (PR-diff mutants: 34 caught, 0 missed). Under ADR 0006 amendment 3 there is no per-slice full run.
- Slice brief: [docs/slices/0035-m1-load-and-save-modes.md](../slices/0035-m1-load-and-save-modes.md)

## Result

`xmi::Mode` sets how strictly reading and writing apply SACM 2.3 (ADR 0022). Reading:
- tolerant by default;
- both modes read and refuse identical inputs, over a 52-input table;
- SACM-XMI-010 (`xsi:type`) is an Error in strict mode and a Warning in tolerant mode;
- SACM-XMI-011 (`xmi:version`) is Info;
- each is reported per attribute, at its name, with the clause and the affected ids pinned.

Writing is strict by default. A strict write validates after `NoRoot` and refuses with
`WriteError::Invalid`, holding every Error in `validate`'s order. Tolerant writes as held.
No test was weakened: format tests write in tolerant mode, which only skips validation.
Twenty-eight hand mutations were killed, and three were equivalent. ADR 0022's C++ claims
were checked against `options.h`, `xmi_reader.cpp` and `xmi_writer.cpp`.

## Findings

| Severity | Row | Finding | Resolution |
|---|---|---|---|
| Blocking (docs review) | LIB-COMPAT-001/007 | The docs claimed strict mode is SACM 2.3 conformance, and that a strict read reports what the standard disallows | Reworded to what it checks today (15184e7) |
| Blocking (CI) | LIB-COMPAT-001/007 | Three PR-diff mutants were missed (message subject, error count) | Tests added (3a2f4af) |
| Blocking | SACM23-XMI-001 | The requirement still said the reader reports no diagnostics | Corrected |
| Follow-up | several | Row and ADR wording, ADR pointers, a stale mutation exclusion | Fixed; test-owned items on `docs/follow-ups.md` |
