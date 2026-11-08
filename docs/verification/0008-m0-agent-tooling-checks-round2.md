# Verification 0008, round 2: m0-agent-tooling-checks (after fix round 1)

- Date: 2026-11-08
- Verdict: **PASS.** LIB-QA-011 passes. LIB-TOOL-008 stays verified, with no regression.
- Row verdict: LIB-QA-011: PASS
- Verifier: `sacm-verifier` (independent; read-only)
- Commit inspected: `262c41bd7e88f18f7c8d2b79cfded8141c3a806a` (PR #15), with `git diff eddec72..262c41b`. That diff adds four tests to `xtask/tests/repo_checks.rs`, changes the guard's docstring, and updates the docs and matrix.
- Contract: F1 items 1 to 8 of the [round 1 record](0008-m0-agent-tooling-checks.md), checked item by item.
- Linux evidence: CI run 42489698185 succeeded on every job. In it, the ubuntu `agent hook tests` job ran 47 tests with none skipped, including all LIB-TOOL-008 tests and both LIB-TOOL-003 real-command tests. Since `eddec72`, no file that cargo-mutants mutates has changed.
- Slice brief: [docs/slices/0008-m0-agent-tooling-checks.md](../slices/0008-m0-agent-tooling-checks.md)

The verifier's verdict: "LIB-QA-011 is verified at 262c41b. `check_slice_pr_rules` passes the real sacm-slice skill. Each of its three rules (reuse the slice's PR, found with `gh pr list --head <branch>`, dispatch `gh workflow run mutants.yml --ref <branch>` when the full run did not run on the head commit, and run `gh pr create` only when the branch has no PR) is matched only within its own step, its own block and its own sentence. Thirteen tests with controls show that a rule removed, moved, weakened, decoupled from its condition or pointed at the wrong branch gives exactly the problem that names it. Every hand mutation that loosens the check is killed, including all eight that survived round 1."

## Round 1 prescription

| Item | Mutation | Result |
|---|---|---|
| 1 | Q10 (no split into sentences) | Killed. Met. |
| 2 | Q2 (the condition may appear anywhere in the sentence) | Killed. Met. |
| 3 | Q32 ("already lists one" weakened) | Killed. Met. |
| 4 | Q34 ("do not create another" weakened) | Killed. Met. |
| 5 | Q30 (the "only when" entry weakened) | Killed. Met. |
| 6 | Q33 ("did not run on the head commit" weakened) | Killed. Met. |
| 7 | Q29 (paragraph boundary) | Killed. Met. |
| 8 | Q31, Q31b, Q31c (problem texts swapped) | All killed. Met in substance (see the Low finding). |

## Rows verified

| Row | Title | Evidence (tests) |
|---|---|---|
| LIB-QA-011 | Slice PR reuse and full-run dispatch are pinned | The 13 cited `lib_qa_011_*` tests in `xtask/tests/repo_checks.rs`. |

LIB-TOOL-008 stays verified (round 1). The hook command, the guard code and the hook tests are byte-for-byte unchanged since `eddec72`. The F2 docstring and the LIB-TOOL-003 notes are accurate.

## Gates run (by the verifier, on Windows)

All pass:
- fmt;
- clippy (`-D warnings`);
- `cargo test --workspace --all-features` and `cargo test -p xtask` (80 `repo_checks` tests);
- rustdoc (`-D warnings`);
- `matrix check` (157 rows);
- `cargo deny check`;
- the hook tests (47, none skipped, under a Git Bash PATH and under a PowerShell-style PATH).

## Findings

| Severity | Row | Finding | Resolution |
|---|---|---|---|
| Low (non-blocking) | LIB-QA-011 | Q36 survives: `PR_CREATE_PROBLEM` could drop "only when the branch has no PR" from its text. The problem's tag, step and command are still pinned. | Optional. Not done in this slice. |
