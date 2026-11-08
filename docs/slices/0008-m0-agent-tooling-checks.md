---
slice: 0008
name: m0-agent-tooling-checks
state: in-progress
step: verification
rows: [LIB-TOOL-008, LIB-QA-011]
mode: pr
---

# Slice 0008: agent tooling checks

## Scope

Two rows that pin how the agent tooling behaves:

- **LIB-TOOL-008.** Tests cover how the write guard is launched, not only what it decides:
  - the interpreter fall-through in the `settings.json` hook command;
  - a fail-closed exit when no Python works;
  - NotebookEdit calls judged by `notebook_path`.
- **LIB-QA-011.** A doc check pins three rules of the `sacm-slice` skill: reuse the slice's PR, dispatch the full mutation run, and create a PR only when none exists.

| Row | Title | Status at start |
|---|---|---|
| LIB-TOOL-008 | The write guard's launch path is tested | planned |
| LIB-QA-011 | Slice PR reuse and full-run dispatch are pinned | planned |

## Out of scope

- The guard's decision logic, which LIB-TOOL-003 (verified) already covers.
- Other skill rules.

## Normative basis

None. These are tooling and quality rows, so the acceptance criteria are the lead's.

## Design (fixed by the lead)

- **D1: fail closed.** The hook command becomes:

  ```sh
  for py in python3 python; do "$py" -c '' >/dev/null 2>&1 && exec "$py" "$CLAUDE_PROJECT_DIR/.claude/hooks/agent_write_guard.py"; done; echo "agent write guard: no working Python interpreter" >&2; exit 2
  ```

  Today, when neither interpreter works, the command exits 127. Claude Code treats that as a non-blocking error, so the write goes ahead: the guard fails open. Exit code 2 blocks the write.

  This also blocks the main session's writes while no Python works. That is the intended fail-closed trade-off, and the maintainer reviews it at the PR.

  Under ADR 0006 amendment 1, `settings.json` is outside every agent's reach. The lead makes the change.
- **D2: where the tests live.**
  - LIB-TOOL-008's tests go in `.claude/hooks/test_agent_write_guard.py`, in the test engineer's scope. They run the real command through `bash -c` with stub interpreters on a temporary PATH.
  - LIB-QA-011's tests go in `xtask/tests/repo_checks.rs`, with a check function over the skill text, as the other repo checks do.

## Tests

| Tests | Row | Criterion |
|---|---|---|
| 8 `test_lib_tool_008_*` tests in `.claude/hooks/test_agent_write_guard.py` | LIB-TOOL-008 | They run the real `settings.json` command through `bash -c` with stub interpreters on a PATH that holds nothing else: <ul><li>a failing or missing `python3` falls through to `python`;</li><li>a working `python3` is used, and `python` is never probed;</li><li>no working interpreter exits 2 with the exact message;</li><li>NotebookEdit is judged by `notebook_path`, through the real command and through `decide`, for writer and read-only roles.</li></ul> Allows are proven by the stub log. No test skips: `hook_bash()` finds Git Bash on Windows and fails if bash is missing. |
| 6 `lib_qa_011_*` tests in `xtask/tests/repo_checks.rs` | LIB-QA-011 | `check_slice_pr_rules` checks <ul><li>the real skill;</li><li>each rule removed, partly removed, or moved to another step;</li><li>a missing step;</li><li>equivalent wording and wrapping.</li></ul> |

## Log

- 2026-11-08 research: skipped, because these are tooling and quality rows. The lead wrote the acceptance criteria, and both rows are now `analyzed`. The lead decided D1 and D2.
- 2026-11-08 tests: 14 tests. 13 pass today: the behaviour already exists, and these
  rows pin it. The fail-closed test was red: exit 1 or 127 instead of 2.
- 2026-11-08 implementation: the lead applied D1 to `.claude/settings.json`, as
  ADR 0006 amendment 1 requires: `exec` the first working interpreter, else
  `exit 2` with the message. All 47 hook tests pass. Both rows are `implemented`.
  - Found on the way: the two LIB-TOOL-003 real-command tests skip at run time when
    bash is missing or is the WSL launcher. They go to the test engineer, to move them
    to `hook_bash()` and make them fail instead.
- 2026-11-08 the LIB-TOOL-003 real-command tests now use `hook_bash()` and fail
  instead of skipping. `find_bash()` is removed. Under a Git Bash PATH and under a
  PowerShell-style PATH, all 47 hook tests run and pass, with 0 skipped.
- 2026-11-08 Copilot review of PR #15: 2 valid comments on the LIB-QA-011 check.
  1. The PR query must govern the draft creation: `gh pr create --draft` and "unless
     `gh pr list --head <branch>` ... lists one" must be in one sentence.
  2. The commands must name the slice branch: `--head <branch>` and `--ref <branch>`
     must appear as whole code spans.

  Step 8.5 is tightened the same way. 3 new tests, each with negative controls that
  the old check passed.
