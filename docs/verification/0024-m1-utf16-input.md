# Verification 0024 — UTF-16 input

- Date: 2026-11-12
- Row verdict: SACM23-XMI-018: PASS
- Row verdict: LIB-API-009: PASS
- Row verdict: LIB-SEC-006: PASS
- Verifier: `sacm-verifier` (independent; read-only), 2 rounds
- Commit: 7827bca. Evidence:
  - CI runs 43126766526 (aea216d) and 43130925455 (7827bca);
  - full mutants run 43126781332 (aea216d): 854 caught, 0 missed, 0 timed out. Library code
    is unchanged since aea216d.
- Slice brief: [docs/slices/0024-m1-utf16-input.md](../slices/0024-m1-utf16-input.md)

## Result

The minimal UTF-16 support (ADR 0005 B2 amendment 1) holds:
- UTF-16 with a byte order mark reads exactly as the same characters in UTF-8.
- A contradicting declared name, an odd byte count and an unpaired surrogate are
  `Malformed`, in that order of checks.
- Everything else stays `Unsupported`.

LIB-API-009 and LIB-SEC-006 were re-checked against their reworded text. The W3C suite
gives Doctype 719, encoding 2 and Malformed 296, with 73 cases in the guard. Every hand
mutation of the decoding, the byte order, the name check and the reason text is killed.

## Findings

| Severity | Row | Finding | Resolution |
|---|---|---|---|
| Major | SACM23-XMI-018 | The order of the declaration and decode checks was not pinned | Name test also reads a lone surrogate and a trailing byte (7827bca) |
| Minor | SACM23-XMI-018 | The contradicting-name reason's Display was not pinned | Exact Display in LE and BE (7827bca) |
| Minor | LIB-API-009 | The UTF-32 BOM reason was not checked against the Display invariants | `assert_refusal_invariants` plus BOM-only inputs (7827bca) |
