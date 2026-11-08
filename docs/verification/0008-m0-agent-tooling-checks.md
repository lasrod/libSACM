# Verification 0008, round 1: m0-agent-tooling-checks

- Date: 2026-11-08
- Verdict: **FAIL** (partial). LIB-TOOL-008 passes, LIB-QA-011 fails.
- Row verdict: LIB-TOOL-008: PASS
- Row verdict: LIB-QA-011: FAIL
- Verifier: `sacm-verifier` (independent; read-only)
- Commit inspected: `eddec72f8b482b27fb868eeca00ed6d2d41b7da8` (PR #15), `git diff main...eddec72`
- Linux evidence:
  - ci.yml run 42487537471: success. In `agent hook tests` (ubuntu-24.04), all 47 tests report `ok` and none is skipped, among them all 8 `test_lib_tool_008_*` tests and both LIB-TOOL-003 real-command tests. `mutants (PR diff)` reports "No mutants to filter", because the slice changes no mutated Rust source.
  - mutants.yml run 42487537531: success, with 0 missed and 0 timeout.
- Claude Code hooks documentation (`https://code.claude.com/docs/en/hooks.md`):
  - "**Blocking error**: your hook exits 2. On events that can block, Claude Code stops the action."
  - "**Non-blocking error**: your hook exits with any other code, or fails in some other way, such as not starting … The action goes ahead".
  - For PreToolUse: "Blocks the tool call".

  So before this slice, the exit code 127 failed open.
- Slice brief: [docs/slices/0008-m0-agent-tooling-checks.md](../slices/0008-m0-agent-tooling-checks.md)

## Rows verified

| Row | Title | Evidence (tests) |
|---|---|---|
| LIB-TOOL-008 | The write guard's launch path is tested | The 8 `test_lib_tool_008_*` tests run the exact `settings.json` command through `bash -c`, with only stub interpreters on PATH. The verifier's verdict is quoted below. |

The verifier's verdict on LIB-TOOL-008: "LIB-TOOL-008 is verified at eddec72. The exact settings.json hook command was run through bash -c with nothing but stub interpreters on PATH. It falls through to `python` when `python3` fails or is missing, uses `python3` without probing `python` when `python3` works, and passes the event on stdin to the guard it execs. With no working interpreter it fails closed: exit 2, with the exact message on stderr and nothing on stdout, which the Claude Code hooks documentation defines as blocking for PreToolUse. NotebookEdit is judged by `notebook_path` for writer and read-only roles. All 8 tests, and the LIB-TOOL-003 real-command tests, ran without a skip on ubuntu CI (ci.yml run 42487537471) and on Windows with Git Bash. Every hand mutation of the command's exec, exit code, message, probe and loop order was killed."

The verifier ran 18 hand mutations of the command. Each was killed except M12, a relative guard path, which still fails closed. It ran 4 mutations of the guard; each was killed except N4, which is equivalent.

## Rows left open

| Row | Status kept | Reason |
|---|---|---|
| LIB-QA-011 | implemented | Nine hand mutations of `check_slice_pr_rules` survive (Q2, Q10, Q29 to Q34, Q31). |

## Findings and the complete prescription

### F1 (Medium, LIB-QA-011): loosening mutations of the check survive

The surviving mutations are:
- **Q10, sentence boundary.** It is not pinned that the condition must be in the same sentence as `gh pr create --draft`.
- **Q2, order.** It is not pinned that the condition must come after the draft creation.
- **Q32, Q30, Q34 and Q33, weakened wording.** The weakened forms of "already lists one", "only when the branch has no pr", "do not create another" and "did not run on the head commit" pass.
- **Q29, paragraph boundary.** A blank line is not tested as a block boundary.
- **Q31, problem texts.** It is not pinned that each problem text names its own rule: two constants could be swapped.

**Prescription.** Each item is a negative control on `PR_RULES_FIXTURE` that gives exactly the one named problem.
1. The draft creation and the condition are in one paragraph but in two sentences. It gives `[PR_REUSE_PROBLEM]`. This kills Q10.
2. The condition comes before the draft creation in the same sentence, without governing it. It gives `[PR_REUSE_PROBLEM]`. This kills Q2.
3. "… unless `gh pr list --head <branch>` already lists none", and the same clause cut off after "already". Each gives `[PR_REUSE_PROBLEM]`. This kills Q32.
4. "do not create a new branch" in the reuse item. It gives `[PR_REUSE_PROBLEM]`. This kills Q34.
5. "Run `gh pr create` only when needed." and "… only when the branch has no previous review." Each gives `[PR_CREATE_PROBLEM]`. This kills Q30.
6. "If `mutants.yml` did not run on the base branch: …". It gives `[FULL_RUN_DISPATCH_PROBLEM]`. This kills Q33.
7. The dispatch condition and the command are in two paragraphs separated by a blank line. It gives `[FULL_RUN_DISPATCH_PROBLEM]`. This kills Q29.
8. A test asserts each problem constant's tag and command as literals, independent of the constants. This kills Q31.

Not held:
- Q28, the word boundary on "lists one";
- `*` and `+` bullets;
- a `## 7` heading without its dot;
- ending a numbered item at its first blank line.

### F2 (Low, LIB-TOOL-008 docs, owned by the lead)

Two statements became false after D1:
- The docstring of `agent_write_guard.py` says nothing is enforced when Python is unavailable.
- The LIB-TOOL-003 `notes` say that the hook then exits non-zero, which is non-blocking.

Both are to be updated in the same PR.

### F3 (Low, slice docs)

The brief's test table says 6 `lib_qa_011_*` tests, but the matrix lists 9.

## Follow-ups (LIB-TOOL-019, planned)

The verifier found launch paths that are still non-blocking under the documented contract:
- the 30-second hook timeout;
- a crash before `main` handles it;
- an interpreter that passes `-c ''` but cannot run the guard, such as Python 2.

It proposed these remedies:
- `"onFailure": "block"` (Claude Code 2.1.295 or later), or a version-checking probe;
- a hook CI step that fails on any runtime skip;
- launch tests that also run under `sh -c`, as Linux and macOS do;
- optionally, a Windows CI runner for the hook tests.
