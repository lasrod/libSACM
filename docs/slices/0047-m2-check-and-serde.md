---
slice: 0047
name: m2-check-and-serde
state: done
step: close
rows: [LIB-EDIT-013, LIB-API-006]
mode: pr
base: main
---

# Slice 0047 — Check an operation, and serde for edit values

## Scope

Two Assurance Forge requests on the edit API's values:
- **LIB-EDIT-013 (GitHub issue #60, first half):** `edit::check` answers whether `apply`
  would accept an operation, without changing the document. It shares one code path with
  `apply`.
- **LIB-API-006 (GitHub issue #57):** an optional `serde` feature. It serializes
  operations, mutations, edit errors, diagnostics and the value types they carry, in a
  documented, stable JSON form. `Document` is not serialized.

Left out: atomic batches (LIB-EDIT-014), which build on `check` in the next slice.

## Basis

Library rows: acceptance by the lead. Design: ADR 0019, ADR 0029, ADR 0011. The maintainer
decided on 2026-11-15 that the serialized form is a stability promise.

## Design

- **LIB-EDIT-013** (ADR 0019 amendment 3, accepted): `edit::check(&Document, &Operation)
  -> Result<(), EditError>`. Each operation gets a crate-private `plan(&self, &Document)
  -> Result<Plan, EditError>`; `apply` = `plan` + an infallible `commit`.
- **LIB-API-006** (ADR 0030, accepted): the `serde` feature, with rules R1-R7 (internal
  `"type"` tag = `operation_name()`, snake_case keys, every key written, unknown keys and
  names refused, `EditError` carries `"code"`). `Code` gets a catalog table built by the
  same macro as the constants; `EditError` reads back through a private mirror;
  `Diagnostic`'s `requirement_id` and `clause` become `Cow<'static, str>`.
- The lead added the `serde` feature, the optional `serde` dependency and the
  `serde_json` dev-dependency to `crates/sacm/Cargo.toml`, so the tests fail on missing
  impls rather than being skipped.

## Log

- 2026-11-15 choose: the two ready Assurance Forge rows. Both touch every operation
  struct. LIB-EDIT-013 unblocks LIB-EDIT-014.
- 2026-11-15 design: architect brief. The maintainer accepted ADR 0019 amendment 3 and
  ADR 0030, and chose to read errors and diagnostics back as well. Both rows `analyzed`.
- 2026-11-15 tests: 7 `lib_edit_013_*` tests (the operation set now covers all 25
  operations) and 29 `lib_api_006_*` tests with five golden files; red confirmed (E0425
  `edit::check`, E0277 missing serde impls). The lead accepted the test engineer's readings
  of R1-R7 (a bare struct has no tag; reserved codes and the CLI's `requirement` key are
  refused). Rows `tests-failing`.
- 2026-11-15 implementation: `edit::check` and the `plan`/`commit` split over all 25
  operations (`edit/plan.rs`); the serde impls, with a hand-written `EditError` impl through
  the private mirror `edit/record.rs` (a derive would give only `Deserialize<'static>`) and
  a `catalog!` macro for `Code`. Docs review: nothing blocking; nine doc follow-ups applied,
  two recorded. Full gate green. Rows `implemented`.
- 2026-11-15 verification: LIB-EDIT-013 PASS; LIB-API-006 FAIL in round 1 (three test gaps
  with surviving mutants: unknown keys, required keys, feature-name read-back), fixed by
  three golden-file-driven tests in d336b06; round 2 PASS (record 0047). Rows `verified`.
