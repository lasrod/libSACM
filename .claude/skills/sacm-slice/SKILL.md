---
name: sacm-slice
description: Run one iteration of the libSACM engineering loop as the lead — pick the next ready rows from the functionality matrix, then drive research, API design, failing tests, implementation and independent verification through the project agents, update the matrix and commit. Use when the user runs /sacm-slice, asks to "do the next slice", "continue the loop", or runs it under /loop. Arguments: optional row IDs to force a slice, and `mode=local` (default) or `mode=pr`.
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

Run **sacm-verifier** — with the row IDs, acceptance criteria, the diff, and the
reports, but *not* your opinion of the work. If the slice changed public API, run
**sacm-docs-reviewer** in parallel.

- **FAIL** (or blocking docs findings): route each finding to the role that owns it
  (tests → test engineer, code/docs → implementer, requirement → research), then
  re-verify. At most **two** fix rounds; after a second FAIL, stop and report to the
  user with the findings.
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
5. `mode=local`: do not push. `mode=pr`: push the branch and open a PR with the slice
   brief as the body; stop until it is merged.

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
