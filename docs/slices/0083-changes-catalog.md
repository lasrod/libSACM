---
slice: 0083
name: changes-catalog
state: in-progress
step: tests
rows: [LIB-RT-005, LIB-DIAG-006]
mode: pr
---

# Slice 0083 — Changes between documents, and the catalog as JSON

## Scope

The last two items of lasrod/libSACM#113: `compare::changes`, the ids created, changed and
deleted between two documents (A11), and the catalog of codes and templates as JSON, shipped
in the crate (A10). It also kills the two mutants CI's PR-diff job left in slice 0082
(`str == ElementId` always true; `Specification`'s visitor message). Only these two rows are
ready in this area.

Base: `main` (2d713da).

## Basis

Library rows: acceptance by the lead from the design brief (session scratchpad
`design-0083.md`). ADR 0020 amendment 1, ADR 0030 amendment 8, ADR 0011 amendment 5.

## Log

- 2026-11-27 design: designed during slice 0082; the maintainer approved it (the catalog is
  a file in the crate plus `CATALOG_JSON`) and the three ADR amendments. Rows `analyzed`.
- 2026-11-27 tests: 22 tests (changes.rs, batch.rs, catalog_json.rs, xtask/tests/catalog.rs)
  and the changes.json golden; red confirmed (missing API only). Accepted beyond the design: a
  reorder case that tells `after`'s pre-order from `before`'s; an ArgumentReasoning vs
  ArtifactReference pair where only `Kind` differs; changes.json in serde.rs's required-key
  test and `Changes` in its listed types. The two 0082 survivors get cases in api_additions.rs.
  Rows `tests-failing`.
- 2026-11-27 implementation: done; 43 hand mutants, all killed (one only by the LIB-COMPAT-003
  and SACM23-XMI-019 tests). Deviations accepted: a code's row needs its code in the first
  cell too (with a unit test); a short shape list in diagnostics.md; three more generator
  errors. Rows `implemented`.
