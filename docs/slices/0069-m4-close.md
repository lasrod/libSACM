---
slice: 0069
name: m4-close
state: done
step: close
rows: []
mode: pr
base: main (eb9c662)
---

# Slice 0069 — M4 close

## Scope

The close of milestone M4 (Terminology model, 12/12 verified):
- the matrix-steward audit's findings;
- `docs/follow-ups.md` cleared;
- the maintainer's two decisions on serde records.

There are no behaviour changes.

## Basis

- Quality slice; the lead's acceptance.
- The maintainer's decisions of 2026-11-18:
  - arguments in any key order (new row LIB-API-019, ADR 0030 amendment 5);
  - a record's `reason`, `shape` and `element_name` are kept as given (ADR 0030 Records).

## Log

- 2026-11-18: the matrix-steward audit was applied. The builder fixed the `Document`, `lib.rs` and
  README docs; the lead made the matrix, ADR, triage, CHANGELOG and follow-up edits. The record is
  `docs/verification/0069-m4-close.md`.
