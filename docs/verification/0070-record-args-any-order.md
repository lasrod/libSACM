# Verification 0070 — Record arguments read in any key order

- Date: 2026-11-19
- Row verdict: LIB-API-019: PASS
- Verifier: `sacm-verifier` (independent; read-only), one round
- Commit: abb4755. CI run [44428985406](https://github.com/lasrod/libSACM/actions/runs/44428985406)
  passed every job. Its PR-diff mutants caught 6 of the 9 mutants. The other 3 are unviable:
  `Default` on types that have none.
- Slice brief: [docs/slices/0070-record-args-any-order.md](../slices/0070-record-args-any-order.md)
- Decision: ADR 0030 amendment 5 (accepted 2026-11-18), which relaxes R11.

## Result

- **Any key order is read.**
  - With `serde`, a `ReadError`, `WriteError` or `Diagnostic` record reads its `arguments` in
    any key order. Each argument is matched by name and held in template order.
  - Every golden record passed through `serde_json::Value` reads back `==`. The test checks
    that this is not vacuous.
- **Bad arguments are still refused, now with named messages.** A missing, extra, repeated or
  wrongly typed argument gives `needs the argument …`, `has no argument …`,
  `… is given twice`, or the existing kind message.
- **Serialization writes the template order.** The code's-text path keeps "takes 0 arguments,
  not N".
- **The changed existing tests keep their intent.**
  - Each removed "out of order" case has a `lib_api_019_*` twin that asserts `==`.
  - The atomic-batch set (LIB-EDIT-002) now creates every kind.
- **The reworded verified rows LIB-API-017 and -018 are true.**

Evidence beyond CI: the verifier ran 14 hand mutants.
- 12 are killed.
- M9 is equivalent.
- M10 changes only which message wins when an argument is both missing and extra, which is not
  promised. The renamed-argument case added in the fix round pins the extra-name message.

## Findings

| Severity | Finding | Resolution |
|---|---|---|
| non-blocking | The private `record` module doc said arguments match "by name, kind and order". | Reworded with the verifier's text. |
| non-blocking | The `lib.rs` serde sentence read as if only diagnostics accept any order, and did not say "each name once". | Reworded with the verifier's text. |
| non-blocking | No test pinned the extra-before-missing message order. | A renamed-argument case was added. |
