---
slice: 0051
name: m2-close
state: in-progress
step: verification
rows: [LIB-COMPAT-004, LIB-COMPAT-010, LIB-EDIT-012, SACM23-XMI-005, SACM23-XMI-026, LIB-QA-018]
mode: pr
base: main
---

# Slice 0051 — Closing M2

## Scope

M2's close (skill step 9):
- the full mutants run on `main` (43736240829, green);
- the matrix-steward audit, applied to the matrix;
- every follow-up in `docs/follow-ups.md` cleared. The maintainer chose "clear everything"
  on 2026-11-16.

The rows above change behaviour. Every other change is a test, a doc or matrix wording,
and the rows those touch are re-checked.

## Basis

The M2 audit and the follow-up triage (both by the matrix-steward). Maintainer decisions of
2026-11-16:
- SACM23-XMI-009 leaves the compliance points.
- LIB-COMPAT-004 is a Tolerance and leaves the compliance points.
- A role element in reference form is a not-yet-read proxy (Interim SACM23-XMI-019 for
  `xmi:idref`, SACM23-XMI-027 for `href`).
- Vendor content beside the roots becomes a new M6 row (LIB-COMPAT-014).
- `Document::referrers` and the flag and reference-end readers move to M3 (LIB-API-014,
  LIB-API-015).

## Design

No new public API. Behaviour changes:
- `RepeatedValue` locates each token, with a bounded `Display`.
- `set_text` is linear.
- An empty CDATA under `xsi:nil` counts as no content.
- Proxy role elements are refused under XMI-019 or XMI-027.
- LIB-COMPAT-004 is relabelled a Tolerance.
- The CI `previous` step matches the event type.

## Where each follow-up went

- **Fixed now, as tests:** 3, 5, 6, 9, 12, 15, 17, 20, 21, 23, 28, 29, 30, 31, 36, 48, 49, 54.
- **Fixed now, as code or docs:** 10, 11, 19, 24, 26, 34, 35, 40, 41, 43, 44, 46, 50.
- **Moved to rows:**
  - 1, 2, 13, 14 and 18 → LIB-API-013 (M7);
  - 27 → LIB-API-014 (M3);
  - 33 → LIB-API-015 (M3);
  - 37, plus the breaking parts of 13 and 18 → LIB-API-005;
  - 16 → LIB-TOOL-022;
  - 22 → SACM23-ARG-014 and SACM23-ARG-015;
  - 51 → LIB-COMPAT-014;
  - 52 → LIB-EDIT-016;
  - 53 → LIB-COMPAT-013;
  - 55 → LIB-SEC-003;
  - the reverted Copilot binding fix → LIB-COMPAT-013.
- **Closed, already done or resolved:** 4, 7, 25, 38, 39, 42, 45 and 47.
- **Closed by the lead's decision:**
  - 8: the deep-chain deadline stays as it is.
  - 32: `requirement_id` tells the two `REF_WRONG_TYPE` uses apart, as the catalog already
    documents.

## Log

- 2026-11-16 choose: M2 is complete (58/58). The close tasks are this slice. M3's slices
  are renumbered 0052 to 0054.
- 2026-11-16: the audit is applied (B1-B5, F1-F2, S1-S7). The new rows are LIB-COMPAT-013,
  LIB-COMPAT-014, LIB-API-013, LIB-API-014, LIB-API-015 and LIB-TOOL-022. The follow-up
  tests and the red tests are written by two test engineers. The implementer has done the
  docs, the cleanup and the LIB-COMPAT-004 relabel; the code items are in progress.
- 2026-11-16 implementation: the four code items are done (per-token RepeatedValue locations through character references, with a bounded Display; linear set_text; empty CDATA under xsi:nil; proxy role elements refused under XMI-019 or XMI-027). Full gate green.
