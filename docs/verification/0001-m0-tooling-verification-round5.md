# Verification 0001, round 5: m0-tooling-verification (after fix round 3)

- Date: 2026-11-07
- Verdict: **FAIL** for LIB-TOOL-001, the only row in scope.
- Verifier: `sacm-verifier` (independent; read-only)
- Inspected: the uncommitted fix-round-3 diff against `b9be378`. It contains tests and matrix updates only: new `xtask/tests/cli.rs` (3 tests) and 2 tests added to `xtask/tests/matrix.rs`. No production change, and no existing test was removed or weakened.
- Previous records: [round 1](0001-m0-tooling-verification.md), [round 2](0001-m0-tooling-verification-round2.md), [round 3](0001-m0-tooling-verification-round3.md), [round 4](0001-m0-tooling-verification-round4.md)

## Rows verified

None.

## Rows left open

| Row | Status kept | Reason |
|---|---|---|
| LIB-TOOL-001 | implemented | **Medium.** The criterion "the `cargo xtask` binary exits non-zero … whenever a command fails, so CI fails when the gate does" is demonstrated only for errors raised inside `main`. No test shows that `matrix check` through the binary fails when the gate fails. Mutation R28e makes the check arm discard `cmd_check`'s result. With an orphan planted, the binary then printed the violation and exited **0**, and all 77 xtask tests stayed green. R28f, which dispatches `cmd_render` instead, also survives. The verifier records that its round-3 wording of the required fix was narrower than the criterion. |

## Required fix (verifier)

- Move argument dispatch into the xtask library, e.g. `pub fn run(args: &[&str], root: &Path) -> Result<(), String>`. `main` then only calls `run(&args, &repo_root())` and maps the result to an exit code.
- Add `lib_tool_001_` tests: `run(["matrix", "check"], tree)` returns `Err` on a fixture tree with a planted orphan and `Ok` on a consistent tree. Ideally also cover the dispatch of `render`, `next` and `status` on fixtures.
- Keep the binary tests for the exit-code mapping.
- What would change the verdict: that test, plus a re-run in which R28e, R28f and R28g are killed.

## Mutation results (run by the verifier in a fresh scratch copy)

- **Killed:** R28, R28b, R28c, R28d, R28h, R28i, R28j, R28k, R21, R23, R23c, R20. This confirms every claim the test engineer made, and the `help` positive control works.
- **Survived:**
  - R28e (discard `cmd_check` result) and R28f (dispatch to `cmd_render`): **Medium**.
  - R28g (check arm deleted): fails safe.
  - R30/R31 (per-problem lines dropped, or sent to stdout): Low, tracked as LIB-TOOL-010.
  - R23b (empty-area check in `looks_traced`): Low, fails safe.

Process note from the verifier: mutation harnesses must use a fresh copy (with mtimes reset) and a fresh target directory. A reused target directory produced stale mutant builds and spurious failures.

## Gates run (by the verifier)

| Gate | Result |
|---|---|
| fmt, clippy (`-D warnings`), rustdoc (`-D warnings`) | pass |
| `cargo test --workspace --all-features` and `cargo test -p xtask` | pass: 3 cli, 8 fetch, 59 matrix, 4 repo-check, doctests |
| `cargo xtask matrix check` | pass (140 rows) |
| `cargo deny check` | pass |
| hook tests | pass (39) |

## Follow-up rows created

- LIB-TOOL-009: area-table validation, i.e. area codes are non-empty, uppercase and unique.
- LIB-TOOL-010: every gate problem reaches stderr, one per line, in a deterministic order.
