# Verification 0013, round 3 — m1-document-model

- Date: 2026-11-10
- Row verdict: LIB-API-002: PASS
- Verifier: `sacm-verifier` (independent; read-only), at full depth. This was a test-only fix round, which the maintainer approved after the row's second FAIL.
- Commit: `b15c93a`. Evidence: ci.yml 42656437863 and mutants.yml 42656438207, both success. In `src/model`, 50 mutants caught, 17 unviable, 0 missed.
- Slice brief: [docs/slices/0013-m1-document-model.md](../slices/0013-m1-document-model.md)

## Result

Both remaining items of the [round 2 prescription](0013-m1-document-model-round2.md) are closed:
- **Exact `contains`:** the near-miss ids and every normalised form are now checked, and X7, X9 and X10 are killed.
- **Exact parent lookup:** ` arguments` and `argumentsé` are now refused as parents, and X12 and X14 are killed.

The edits to `lib.rs` and `model/document.rs` are doc comments only, and no test was weakened. No new class was found.

## Findings

None. Logged, not required: the `InsertError` variant per refusal, ids made only of whitespace, and the packaging of the doctests that need `test-support` (for the release rows).
