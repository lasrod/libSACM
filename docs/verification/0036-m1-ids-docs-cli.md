# Verification 0036 — Ids, the crate example, and the CLI

- Date: 2026-11-13
- Row verdict: SACM23-VAL-020: PASS
- Row verdict: SACM23-VAL-001: PASS
- Row verdict: LIB-CLI-002: PASS
- Row verdict: LIB-CLI-003: PASS
- Row verdict: LIB-DOC-001: PASS
- Row verdict: SACM23-XMI-001: PASS
- Row verdict: SACM23-XMI-002: PASS
- Row verdict: SACM23-XMI-004: PASS
- Row verdict: SACM23-XMI-007: PASS
- Row verdict: SACM23-XMI-010: PASS
- Row verdict: SACM23-XMI-013: PASS
- Row verdict: SACM23-XMI-016: PASS
- Row verdict: SACM23-XMI-018: PASS
- Row verdict: SACM23-VAL-018: PASS
- Row verdict: LIB-COMPAT-001: PASS
- Row verdict: LIB-COMPAT-007: PASS
- Verifier: `sacm-verifier` (independent; read-only), 2 rounds; `sacm-docs-reviewer`, 1 round
- Commit: c1fe9eb. Evidence: CI run 43405296844 (PR-diff mutants: 71 caught, 0 missed). Under ADR 0006 amendment 3 there is no per-slice full run; no test was weakened.
- Slice brief: [docs/slices/0036-m1-ids-docs-cli.md](../slices/0036-m1-ids-docs-cli.md)

## Result

Ids (ADR 0015 amendment 2):
- **A duplicate `xmi:id`** is `ReadError::DuplicateId` (SACM-ID-001), with the start tag of
  every element carrying it.
- **An empty id** is `ReadError::InvalidId` (SACM-ID-002).
- **Other non-NCName ids** are read, and `validate` reports them as SACM-ID-002, so a strict
  save refuses them.
- **Padded ids** are `Unsupported` until SACM23-XMI-025.
- **Locations:** read diagnostics are located in one pass. Reads of 80,000 `xsi:type`
  diagnostics were timed and are linear.

The CLI (ADR 0023):
- **`sacm validate` and `sacm roundtrip`** give exit codes 0, 1 or 2, and versioned JSON on
  stdout only.
- **The crate root** shows load, inspect, edit, validate and save, succeeding on the default
  strict save.

Eleven verified rows had their tests changed for the new id rules, and all were re-checked.

## Findings

| Severity | Row | Finding | Resolution |
|---|---|---|---|
| Blocking | SACM23-VAL-001, LIB-CLI-002/003 | Four PR-diff mutants missed (refusal text, summary counts, the reload filter) | Exact texts pinned; the reload check extracted and unit-tested (c1fe9eb) |
| Blocking | LIB-CLI-002 | ADR 0023 said JSON mode also writes to stderr; the code and row do not | ADR aligned to the code; the maintainer confirmed the amendment |
| Blocking | SACM23-XMI-001 | An acceptance item still said `Unsupported` for empty and duplicate root ids | Rewritten |
| Blocking (process) | SACM23-XMI-010 | An equivalent Locator mutant had no triage row | Triaged and excluded |
| Blocking (docs review) | LIB-CLI-002/003 | Doctests used test-only API; the README misread `code: null` and `roundtrip --strict` | Fixed |
| Follow-up | several | Wording, page citations, a linearity test | Fixed or on `docs/follow-ups.md` |
