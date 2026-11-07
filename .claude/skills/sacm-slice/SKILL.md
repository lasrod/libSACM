---
name: sacm-slice
description: Run one iteration of the libSACM engineering loop as the lead — pick the next ready rows from the functionality matrix, then drive research, API design, failing tests, implementation and independent verification through the project agents, update the matrix and commit. Use when the user runs /sacm-slice, asks to "do the next slice", "continue the loop", or runs it under /loop. Arguments: optional row IDs to force a slice, and `mode=pr` (default, chosen by the maintainer 2026-11-07) or `mode=local`.
---

# One slice of the libSACM loop

You are the **lead**. You choose the work, brief the agents, apply their results to the
matrix, and decide when a row may be called verified. You do not write production code
or tests yourself: the roles exist so that no single actor both builds and judges.

One invocation = at most one slice. A slice is 1–5 matrix rows that form one reviewable
change. Stop and report at the end of the slice; under `/loop`, the next iteration picks
up from the matrix.

## 0. Preflight

1. `git status`. If the tree is dirty:
   - and `docs/slices/` has a brief with `state: in-progress`, **resume that slice** at
     the step its brief records;
   - otherwise stop and ask the user — do not commit or discard someone's changes.
2. `mode=pr` only: if the previous slice's PR is still open, report its state and stop.
   Start the new slice from up-to-date `main`.
3. `cargo xtask matrix check` must pass before you start. If it fails, fixing the matrix
   *is* this slice's work (and only that).
4. `cargo xtask fetch-spec` so the normative sources are present for the agents.

## 1. Choose the slice

`cargo xtask matrix next -n 20`. Take the first ready row, then add ready rows that
belong with it: same milestone, same area or tightly coupled (one cannot be tested
without the other), at most 5 rows, one coherent change. If explicit row IDs were
passed, use those (each must be ready or you must say why it is safe).

If nothing is ready: report which rows block progress and why, and stop the loop
(`ScheduleWakeup` with `stop: true` under `/loop`).

## 2. Brief

Create `docs/slices/NNNN-<short-name>.md` from `references/slice-brief.md` (NNNN =
next number), `state: in-progress`, step `research`. Under `mode=pr` create branch
`slice/NNNN-<short-name>` first.

## 3. Research → `analyzed`

For rows still `planned`, run **sacm-researcher** (specification analysis mode; add
metamodel cartography for model rows; interoperability mode for COMPAT rows). Brief it
with the row IDs, their TOML, and what is already settled.

Apply its result to `docs/matrix/functionality-matrix.toml` yourself: sharpen
`requirement` and `source` with the quoted clause, set `acceptance`, add missing
`depends_on`, set `status = "analyzed"`. New obligations it found become new rows
(next free number in the area). Then `cargo xtask matrix render && cargo xtask matrix check`.

Tooling, quality and docs rows without a SACM source skip research; write their
acceptance criteria yourself.

## 4. Design

If the slice adds or changes public API, run **sacm-architect** with the research
report. If it proposes an ADR, write it to `docs/adr/` (status `accepted` only after
the user approves it at the next checkpoint; until then `proposed` and the design is
used provisionally). Record the brief's key signatures in the slice brief.

## 5. Failing tests → `tests-failing`

Run **sacm-test-engineer** with the rows, acceptance criteria, research and design
briefs. Then confirm yourself: run the new tests and check they fail (or fail to
compile) for the reason the report states. A test that passes before implementation is
a finding — resolve it before continuing.

Cite each test in its row (`tests = ["crates/sacm/tests/x.rs::sacm23_..."]`), set
`status = "tests-failing"`, render, check. `matrix check` passes in this state; the
gate's `cargo test` step fails by design until step 6. Never commit in this state.

If the test engineer asks for a dev-dependency, decide it (rust-library-craft rules)
and add it yourself.

## 6. Implementation → `implemented`

Run **sacm-implementer** with everything above. When it reports, run the local gate
yourself (fmt, clippy, test, doc, matrix). If it reports a test it believes is wrong,
judge it: if the test is wrong, send it back to the test engineer with the reasoning;
if not, send the implementer back.

Cite code paths in each row, set `status = "implemented"`, render, check.

## 7. Verification → `verified`

**Before verifying, under `mode=pr`:** commit and push the slice branch. Open the
slice's PR as a **draft** (`gh pr create --draft`) unless `gh pr list --head <branch>`
already lists one. The maintainer chose this on 2026-11-07.
- **If the branch already has its PR** (on re-verification, or when a slice resumes),
  push to that PR; do not create another.
- **Wait for the PR's CI,** so the verifier gets the Linux evidence for the commit under
  verification:
  - the `mutants` job's `mutants.out` artifact (`gh run download`);
  - the full-run shards from `mutants.yml`;
  - the Linux-only tests.
- **If `mutants.yml` did not run on the head commit:** its path filter skips a PR
  whose diff touches no Rust code or mutation config. Dispatch it on the slice branch
  (`gh workflow run mutants.yml --ref <branch>`) and wait for that run.
- **Give the verifier** the run URLs, the head SHA and the downloaded reports.
- **Mark the PR ready** (`gh pr ready`) only after PASS and the Copilot fixes (step 8.6).

**Under `mode=local`:** nothing is pushed, so there is no Linux report. The maintainer
decided on 2026-11-07 that a row whose code cargo-mutants mutates may not PASS until a
Linux report for that commit exists; a local Windows run is not a substitute. Such rows
stay `implemented`, and the verifier keeps them open. To verify them, stop at a
checkpoint (step 9). Ask the maintainer either to authorize pushing the branch, or to
re-run the slice under `mode=pr`. Do not push on your own.

Run **sacm-verifier** — with the row IDs, acceptance criteria, the diff, and the
reports, but *not* your opinion of the work. If the slice changed public API, run
**sacm-docs-reviewer** in parallel.

- **FAIL** (or blocking docs findings): route each finding to the role that owns it
  (tests → test engineer, code/docs → implementer, requirement → research), then
  re-verify. At most **two** fix rounds; after a second FAIL, stop and report to the
  user with the findings.
  - The verifier's "Required fix" must be **complete** for the row, not the next step
    (see "Prescriptions must be complete" in its definition). If it reads as one
    mutation deep, ask the verifier for the complete prescription before starting the
    fix round.
  - Brief the fix round against that full list. When re-verifying, give the verifier
    its previous prescription, so it checks item by item and labels anything outside
    it as a new class.
- **PASS**: write `docs/verification/NNNN-<short-name>.md` from
  `references/verification-record.md`, quoting the verdict and listing every row it
  covers. Set those rows to `status = "verified"` with
  `verification = "docs/verification/NNNN-<short-name>.md"`. Rows the verifier said
  must stay open keep their status. Render, check.

## 8. Close the slice

1. Add a CHANGELOG `[Unreleased]` entry for user-visible changes.
2. Set the slice brief `state: done`.
3. Full local gate once more.
4. Commit: `feat(<area>): <summary> [<ROW-IDs>]` (or `fix`/`docs`/`test`/`chore`), body
   listing rows moved to verified. Include the attribution trailer the session requires.
5. `mode=pr` (default): push the branch to the slice's PR. Step 7 opened that PR as a
   draft, so reuse it. Run `gh pr create` only if the branch has no PR. Update the PR
   body from the slice brief (`gh pr edit N --body-file …`), and watch its CI run. Under `/loop`, the next iteration waits
   (preflight step 2) until the maintainer has merged it. `mode=local`: commit only,
   do not push. Never merge a PR yourself; merging is the maintainer's review.
6. `mode=pr`: **wait for the Copilot pull-request review to finish, then address
   every comment** before reporting the PR as ready (the maintainer's standing
   request):
   - Fetch the review and its inline comments:
     `gh pr view N --json reviews` and `gh api repos/{owner}/{repo}/pulls/N/comments`.
   - Route each comment like a verifier finding: tests to the test engineer, code and
     docs to the implementer, guard and matrix changes to yourself. Verify the fix
     (the verifier for anything that touches a cited row), commit and push it, and
     let CI run.
   - Reply on each thread with the fix commit and its evidence
     (`gh api …/pulls/N/comments/ID/replies`). Leave threads unresolved for the
     maintainer.
   - A comment that is wrong, or outside the slice: reply with the reason, and open a
     matrix row if it points at a real gap.

## 9. Checkpoints — when to stop the loop for the user

Stop (and under `/loop`, end with `stop: true`) and report when:

- a milestone's last row becomes verified — then run **matrix-steward** for that
  milestone, apply its exact edits, and present the milestone summary for review;
- a proposed ADR or an open question blocks the next slice;
- the verifier fails a slice twice;
- the gate fails for a reason outside the slice;
- anything would require pushing, publishing or a decision the user has not delegated.

## Report (end of every invocation)

```markdown
## Slice NNNN — <name>: verified | stopped at <step>
- Rows: ROW-ID (old → new status), ...
- Tests added: N (files)
- Verifier: PASS/FAIL (link to record)
- Commit: <sha> <subject>
- Next ready rows: ...
- Needs your decision: ... (if any)
```
