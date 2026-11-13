---
slice: 0038
name: m1-audit-fixes
state: done
step: verified
rows: [LIB-RT-002, LIB-CLI-003, LIB-SEC-002, SACM23-XMI-023]
mode: pr
base: main
---

# Slice 0038 — M1 audit fixes

## Scope

The matrix steward's M1 audit (at 767cc51) found two overstated claims and some gaps.
This slice fixes them before M1 closes:

- **LIB-RT-002:** the requirement said the round trip writes with the default options. Since
  slice 0035 the test writes tolerantly, because a strict write refuses the nameless
  fixtures. The requirement is reworded.
- **LIB-CLI-003:** `roundtrip --strict` was documented but never run by a test. A test is
  added.
- **LIB-SEC-002:** "no unbounded recursion" was tested only on input refused at the root. A
  test with a 100,000-deep readable chain is added.
- **SACM23-XMI-023 (M2):** it already holds for every read diagnostic. A location table
  test proves it.

Matrix hygiene (no status change): missing dependencies, sharper M2 rows (SACM23-VAL-006,
-XMI-020, -XMI-021, -XMI-022, LIB-COMPAT-003, -008, -010, LIB-SEC-003), and stale wording.
Stale docs: README.md, CHANGELOG.md, docs/architecture.md. `docs/follow-ups.md` gains the
write-size hazard (quadratic indentation, LIB-SEC-003).

## Basis

The audit report; ADR 0023 (`roundtrip --strict`); ADR 0005 (iterative parsing); ADR 0022.

## Design

No API change. Tests only, plus the docs.

## Log

- 2026-11-13 choose: the M1 audit findings, as the M1 close slice.
- 2026-11-13 matrix: rows reworded. The re-checked rows wait for the verifier, so the matrix
  check is red until PASS, and nothing is pushed before then.
- 2026-11-13 verify: PASS after three rounds (two matrix wording fixes); record 0038.
