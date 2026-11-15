# Verification 0046 — wasm32 build check and public containment queries

- Date: 2026-11-15
- Row verdict: LIB-API-012: PASS
- Row verdict: LIB-QA-017: PASS
- Verifier: `sacm-verifier` (independent; read-only)
- Commit: c93e0c0, fix round 13d9d87; evidence: CI runs
  [43659001815](https://github.com/lasrod/libSACM/actions/runs/43659001815) (c93e0c0) and
  [43660211115](https://github.com/lasrod/libSACM/actions/runs/43660211115) (13d9d87), all
  16 checks green including `wasm32 build`; PR-diff mutants: 3 caught, 0 missed, 1 unviable
- Slice brief: [docs/slices/0046-m2-wasm-and-containment.md](../slices/0046-m2-wasm-and-containment.md)

## Result

**LIB-API-012:**
- `may_contain` agrees with the hand-written table and with `insert` for all 1,444 pairs.
- It agrees with all 15 create operations under every parent kind.
- `child_kinds` is `ALL` filtered by `may_contain`.
- The tests caught every mutant cargo-mutants made, and these hand mutations: swapped
  arguments, a reversed or duplicated `child_kinds`, a dropped kind, a non-`const` method,
  and a create that refuses an extra parent.

**LIB-QA-017:**
- The lint fired in `sacm`'s non-test code on every listed API, including aliased imports
  and calls through `PathBuf`. It did not fire under `cfg(test)` or on lexical `Path`
  methods.
- The regression tests failed on each removal tried against the real files.

## Findings

| Severity | Row | Finding | Resolution |
|---|---|---|---|
| blocking (docs review) | LIB-API-012 | The rustdoc said the reader cannot disagree with `may_contain`, but it refuses some admitted children until SACM23-XMI-016. | Fixed before verification; ADR 0010 amendment 2 qualified. |
| blocking (Copilot) | LIB-QA-017 | `std::path::Path` methods that touch the file system bypassed the lint; `std::net` was missing. | Fixed in 13d9d87; re-check PASS. |
| follow-up | LIB-API-012 | §11.4 role spelling; no pointer to SACM23-VAL-012. | docs/follow-ups.md |
| follow-up | LIB-QA-017 | No guard against an inner `#[allow]`; no wasm run-time smoke test. | docs/follow-ups.md |
| follow-up | LIB-QA-017 | No CHANGELOG line. | Added. |
