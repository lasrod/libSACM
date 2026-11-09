---
slice: 0014
name: m1-errors-diagnostics
state: in-progress
step: verification
rows: [LIB-API-004, LIB-DIAG-001]
mode: pr
---

# Slice 0014 — error types and structured diagnostics

## Scope

The two result shapes every later row reports through:
- **LIB-API-004:** the library's error enums for operational failures, with `std::error::Error` and source chains.
- **LIB-DIAG-001:** the `Diagnostic` value (code, severity, row and clause, element ids, location, message) and its deterministic order.

Out of scope: emitting any catalog code. Codes go live with the rows that emit them.

## Basis

LIB rows: the rust-library-craft skill (errors vs diagnostics), D-DIAG and D-SEV,
`docs/sacm/diagnostics.md`, and the assurance-forge C++ `validation/diagnostic.h` and
`validation/codes.h`.

## Design

[ADR 0011](../adr/0011-errors-and-diagnostics.md) (accepted). New module `sacm::diagnostic`:
- **`Severity`** (`Info < Warning < Error`), **`Code`** (a `&'static str` newtype with no constants yet) and **`SourceLocation`** (1-based line and column).
- **`Diagnostic`:** accessors `code`, `severity`, `requirement_id`, `clause`, `affected`, `location`, `message`; a one-line `Display`; a total `Ord` (location, code, affected, row, clause, severity, message).
- **Construction:** only under `test-support`.
- **Errors:** one enum per operation, each arriving with its row. No new error type now; `InsertError` already conforms.

## Log

- 2026-11-10 brief written. Research is skipped: both are LIB rows. The architect is next.
- 2026-11-10 design: the architect follows C++ `diagnostic.h` and `codes.h`, with deviations X1–X9. ADR 0011 is proposed. LIB-API-004's requirement is reworded to per-operation errors, and both rows are `analyzed`. Waiting for the maintainer's approval of ADR 0011.
- 2026-11-10 the maintainer approved ADR 0011, including X1–X9 and the location-first order. The test engineer is next.
- 2026-11-10 tests: 46 tests (28 in `crates/sacm/tests/diagnostic.rs`, 18 in `crates/sacm/tests/errors.rs`). They are red: `sacm::diagnostic` is unresolved. Both rows are `tests-failing`. One finding: an id with a line break makes an `InsertError` message two lines. Lead decision: keep the test, and quote and escape ids in `InsertError` messages. Review items for the verifier: no `Code` constants, `#[non_exhaustive]`, the `# Errors` and `# Panics` sections, and the example without `test-support`. The implementer is next.
- 2026-11-10 implementation: `sacm::diagnostic` (`Severity`, `Code`, `SourceLocation` with `NonZeroU32`, `Diagnostic` with the ADR order), the `test-support` constructors, and quoted, escaped ids in `InsertError` messages. All 46 tests pass. The local mutation run on `diagnostic.rs` and `document.rs` left 0 missed. Both rows are `implemented`, and the full gate passes.
