# Verification 0014, round 1 — m1-errors-diagnostics

- Date: 2026-11-10
- Row verdict: LIB-API-004: PASS
- Row verdict: LIB-DIAG-001: FAIL
- Verifier: `sacm-verifier` (independent; read-only), at full depth (library rows)
- Commit: `34f8a0e`. Evidence: ci.yml 42719150480 and mutants.yml 42719150611, both success. In `diagnostic.rs` and `document.rs`, 39 mutants caught, 25 unviable (`Default::default()` on types without `Default`), 0 missed.
- Slice brief: [docs/slices/0014-m1-errors-diagnostics.md](../slices/0014-m1-errors-diagnostics.md)

## Result

**Holds:**
- **Gates:** all pass, locally and in CI.
- **ADR 0011:** the public items match it. There is no crate-wide `Error`, and `Code` has no constants.
- **Error rules:** `InsertError` is `#[non_exhaustive]`, has one-line, escaped messages, and `source()` is `None`.
- **Construction:** an outside crate without `test-support` cannot build `Code`, `SourceLocation` or `Diagnostic`. The module example compiles there.
- **Hand mutations:** the Display, SourceLocation, Severity, builder, `InsertError` and key-priority mutations are all caught.

**Fails:** the comparison within each key field is not pinned.

## Findings

| Severity | Row | Finding | Required fix |
|---|---|---|---|
| Medium | LIB-DIAG-001 (item 7) | All test strings per key have the same length, so comparing by length first survives (E1–E4, D7d). The `# Order` rustdoc does not say how keys compare. | The rustdoc says text keys compare as `str` and affected ids compare id by id, with a prefix first. Add controls: code `AA` < `B`, also directly on `Code`; row `R-AA` < `R-B`; clause `10` < `2`; message `m-aa` < `m-b`; affected `[a, z]` < `[b]`. |
| Low | LIB-DIAG-001 (item 2) | The row's own test has no adjacent duplicates, so `dedup()` survives it (E5). | Add adjacent duplicates to `lib_diag_001_affected_keeps_the_given_order_and_duplicates`. |

Logged, not required:
- A multi-line message makes `Display` multi-line. Producer rows escape ids and test one line.
- `Severity`'s `#[non_exhaustive]` is not stated in ADR 0011.
- Each producer row needs its own sorted-output control.
