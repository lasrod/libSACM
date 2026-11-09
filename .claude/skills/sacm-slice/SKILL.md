---
name: sacm-slice
description: Run one iteration of the libSACM engineering loop as the lead — pick the next ready rows from the functionality matrix, then drive research, API design, failing tests, implementation and independent verification through the project agents, update the matrix and commit. Use when the user runs /sacm-slice, asks to "do the next slice", "continue the loop", or runs it under /loop. Arguments: optional row IDs to force a slice, and `mode=pr` (default, chosen by the maintainer 2026-11-07) or `mode=local`.
---

# One slice of the libSACM loop

You are the **lead**. You choose the work, brief the agents, update the matrix, and decide
when a row is verified. You do not write production code or tests: no single actor both
builds and judges.

- **One invocation is one slice:** 1 to 5 matrix rows that make one reviewable change.
- **Keep it simple** (ADR 0006 amendment 2): add a check only if it protects the
  library's users, and write briefs and records short.

## 0. Preflight

1. **Run `git status`.** If the tree is dirty, resume the slice whose brief says
   `state: in-progress`. Otherwise ask the user.
2. **`mode=pr`:** if the previous slice's PR is still open, report it and stop. Otherwise
   start from an up-to-date `main`.
3. **`cargo xtask matrix check` must pass.** If it fails, fixing it is the slice.
4. **Run `cargo xtask fetch-spec`.**

## 1. Choose

Run `cargo xtask matrix next -n 20`. Take the first ready row, plus ready rows that belong
with it (the same milestone and area, or tightly coupled).

If nothing is ready, report the blockers and stop the loop.

## 2. Brief

Copy `references/slice-brief.md` to `docs/slices/NNNN-<name>.md`, with
`state: in-progress`. Under `mode=pr`, first create the branch `slice/NNNN-<name>`.

## 3. Research → `analyzed`

- **Planned SACM rows:** run **sacm-researcher**. Apply its result to the matrix yourself:
  `requirement`, `source` (with the quoted clause) and `acceptance`, and set
  `status = "analyzed"`. New obligations become new rows. Then run
  `cargo xtask matrix render && cargo xtask matrix check`.
- **Tooling, QA and docs rows:** skip research, and write their acceptance yourself.

## 4. Design

- **Public API added or changed:** run **sacm-architect**.
- **ADRs:** an ADR is `proposed` until the user approves it.
- **The brief:** record the key signatures in it.

## 5. Failing tests → `tests-failing`

- **Run sacm-test-engineer.**
- **Confirm the red yourself:** the new tests fail for the stated reason. A test that
  passes before implementation is a finding.
- **Update the matrix:** cite the tests in their rows, and set `tests-failing`.
- **Do not commit** in this state.
- **Dev-dependencies:** add them yourself, by the rust-library-craft rules.

## 6. Implementation → `implemented`

- **Run sacm-implementer,** then the local gate yourself.
- **A disputed test:** decide who is wrong and send it back to that agent.
- **Update the matrix:** cite the code, and set `implemented`.

## 7. Verification → `verified`

1. **`mode=pr`:** commit and push.
   - Open a draft PR (`gh pr create --draft`), unless `gh pr list --head <branch>` already
     lists one.
   - Request Copilot's review: `gh pr edit N --add-reviewer "@copilot"`.
2. **Wait for CI.** Download the `mutants` job's `mutants.out`, and the `mutants.yml`
   shards if that workflow ran.
   - For a library row: if `mutants.yml` did not run on the head, dispatch it
     (`gh workflow run mutants.yml --ref <branch>`).
   - For a tooling row: the PR-diff job is enough.
3. **Run sacm-verifier** with the rows, the acceptance, the diff and the run evidence,
   but not your opinion. If public API changed, run **sacm-docs-reviewer** in parallel.
   - **Library rows:** full depth. At most two fix rounds; after a second FAIL, stop and
     ask the user.
   - **Tooling, QA and docs rows:** one round against the acceptance. A fix round is
     re-checked only against its own prescription.
4. **On FAIL:** route each finding to its owner (tests to the test engineer, code and
   docs to the implementer, matrix, ADRs and workflows to you), then re-verify against
   the previous prescription.
5. **On PASS:** write a short record from `references/verification-record.md`, set the
   rows to `verified` with `verification = "<record>"`, then render and check.

**`mode=local`:** nothing is pushed, so there is no Linux mutants report, and mutated rows
cannot PASS. Stop and ask the user.

## 8. Close

1. **CHANGELOG:** add an entry for user-visible changes only.
2. **The brief:** set `state: done`.
3. **Run the full gate.**
4. **Commit:** `feat(<area>): <summary> [<ROW-IDs>]`, with the session's attribution
   trailer.
5. **`mode=pr`:**
   - push, and update the PR body (`gh pr edit N --body-file …`);
   - address every Copilot comment: route it, fix it, reply on its thread with the
     commit, and leave the thread unresolved;
   - when PASS and Copilot are done, run `gh pr ready`;
   - never merge: the user merges.

## 9. Stop and report when

- a milestone's last row is verified (run **matrix-steward** and present the summary);
- an ADR or open question blocks the work;
- a library row fails twice;
- the gate fails for a reason outside the slice;
- anything needs a decision the user has not delegated.

## Report

```markdown
## Slice NNNN — <name>: verified | stopped at <step>
- Rows: ROW-ID (old → new), ...
- Verifier: PASS/FAIL (record)
- PR / commit: ...
- Next ready rows: ...
- Needs your decision: ...
```
