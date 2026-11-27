---
slice: 0082
name: af-api
state: in-progress
step: design
rows: [LIB-API-013, LIB-API-021, LIB-EDIT-023, LIB-EDIT-024]
mode: pr
---

# Slice 0082 — The additive API Assurance Forge asked for

## Scope

The additive lookups and edits of lasrod/libSACM#113: the small API additions of
LIB-API-013 (A1, A2, A4, A5, A7–A9), ancestors and descendants (A3), one operation that sets
a tagged value by key (A2), and a preview of a batch (A6). Nothing here breaks 0.4.0 code.
It leaves out the per-id summary of a comparison (LIB-RT-005) and the JSON catalog
(LIB-DIAG-006), which come next.

Base: `main` (b901cb7).

## Basis

Library rows: acceptance by the lead from the design brief. The tagged-value operation
follows SACM 2.3 §8.5 (TaggedValue) and ADR 0030 for its JSON.

## Log

- 2026-11-27 design: slice opened on `slice/0082-af-api` from `main`.
- 2026-11-27 design: brief done. The maintainer took every recommendation: any key value
  matches; `index_for_lang` falls back to the first value with text; `position` with a new
  template; no XMI-020 field. ADR 0011 am. 4, 0029 am. 2, 0030 am. 7 and 0032 am. 1
  accepted. Rows `analyzed`.
- 2026-11-27 tests: 55 tests in four files; red confirmed (missing API only; changed goldens
  fail existing tests for the intended reason). Approved deviations from the design: §7.6.3
  is in the XMI clause set; SACM23-VAL-015's input is a non-interface sub-package; the
  cascade test adds `CrossPackageReferencePolicy::ApplyReferencePolicy`; the LegacyShape case
  adds a later `argument_package_1`. Approved forced edits to existing tests: serde.rs,
  serde_diagnostics.rs, serde_refusals.rs, prose_arguments.rs, both message_templates.rs.
  Rows `tests-failing`.
- 2026-11-27 implementation: approved test changes after step 2: the SetTaggedValue golden
  check matches the sample without its trailing comma; xtask's constructor list gains
  `with_specification` (seven); mechanical clippy fixes in the four new files; 13 `[]` in six
  existing files typed `[] as [ElementId; 0]`, as `PartialEq<str>` makes `[]` ambiguous
  (Breaking in the CHANGELOG); cases added to kill three survivors (case-sensitive step 1,
  the 7.6.3 and VAL-020 7.6.1 specifications).
