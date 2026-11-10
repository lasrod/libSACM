---
slice: 0014
name: m1-errors-diagnostics
state: done
step: close
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
- 2026-11-10 docs review: 2 blocking findings (the `Display` example cites a matrix row that does not exist and an unsourced clause; the module page implies producers exist), plus 8 clarity items. These go to the implementer. Logged for the first emitting row: `clause` does not name its specification.
- 2026-11-10 verification round 1 at `34f8a0e` ([record](../verification/0014-m1-errors-diagnostics.md)): LIB-API-004 PASS, and it is now `verified`. LIB-DIAG-001 FAIL: comparison within a key is not pinned, and there are no adjacent duplicates. Fix round 1 (of 2): the controls go to the test engineer, and the `# Order` doc goes to the implementer.
- 2026-11-10 docs fixes: the real rows (`SACM23-VAL-003`, `SACM23-VAL-007` with clause 8.2), "no function returns diagnostics yet", how each key compares in `# Order`, the Display rules in prose, and links to the matrix and the catalog. Logged for the validation rows: the catalog gives SACM-MULT-001 to clause 11.13 (AssertedRelationship), while SACM23-VAL-011 uses it for §8.6.
- 2026-11-10 fix round 1: within-key controls (code `AA` < `B`, row `R-AA` < `R-B`, clause `10` < `2`, message `m-aa` < `m-b`, affected `[a, z]` < `[b]` and `[aa]` < `[b]`) and adjacent duplicates. Each control fails under its mutation. The full gate passes.
- 2026-11-10 Copilot: `Display` wrote a message's line breaks as they were. Lead decision (ADR 0011 amendment 1): `Display` escapes `\n`, `\r`, U+0085, U+2028 and U+2029, and `message()` stays exact. Two tests were added (red first), then the fix. The full gate passes.
- 2026-11-10 verification round 2 at `9d7bdc8` ([record](../verification/0014-m1-errors-diagnostics-round2.md)): LIB-DIAG-001 PASS, and it is now `verified`. The slice is done.
