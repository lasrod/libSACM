---
name: sacm-slice
description: Run one iteration of the libSACM engineering loop as the lead — pick the next ready rows from the functionality matrix, then drive design, failing tests, implementation and independent verification through the project agents (designer, builder, verifier), update the matrix and commit. Use when the user runs /sacm-slice, asks to "do the next slice", "continue the loop", or runs it under /loop. Arguments: optional row IDs to force a slice, and `mode=pr` (default, chosen by the maintainer 2026-11-07) or `mode=local`.
---

# One slice of the libSACM loop

You are the **lead**. You choose the work, brief the agents, confirm the red, update the
matrix, and decide when a row is verified. You do not write production code or tests: no
single actor both builds and judges.

- **One invocation is one slice:** 4 to 8 related matrix rows (one area or milestone) that
  make one reviewable change; fewer only when nothing else related is ready (ADR 0006
  amendment 3).
- **Three roles** (ADR 0006 amendment 5): **sacm-designer** (research and API design),
  **sacm-builder** (tests, then code, in two separately briefed steps) and
  **sacm-verifier** (independent; also reviews the docs). **matrix-steward** runs only at
  a milestone's close.
- **Keep it simple** (ADR 0006 amendment 2): add a check only if it protects the
  library's users, and write briefs and records short.

## 0. Preflight

1. **Run `git status`.** If the tree is dirty, resume the slice whose brief says
   `state: in-progress`. Otherwise ask the user.
2. **`mode=pr`: find the base (stacked PRs).** Slices stack, so the loop never waits for a
   merge just to start the next one. The maintainer merges the stack from the bottom.
   1. **List the open slice PRs, and only those:**
      `gh pr list --json number,headRefName,baseRefName,isDraft,reviewDecision --jq '[.[] | select(.headRefName | startswith("slice/"))]'`.
      Other PRs never count as the base, a resume or the limit.
   2. **Retarget:** an open PR whose base branch is already merged into `main` moves to
      `main` (`gh pr edit N --base main`).
      - Retargeting alone triggers no CI. So merge `origin/main` into the head and push
        (`git merge`, never rebase), and the PR's CI runs against its new base.
      - Slice PRs are merged with merge commits, so this merge is clean unless other
        work has landed on `main`.
   3. **Changing a ready PR.** This covers the maintainer's review comments, which come
      first, and any forward merge or retarget merge.
      - **Before the change:** mark the PR draft (`gh pr ready N --undo`).
      - **Review comments:** fix each one on its own branch: route it to its owner, fix
        it, and reply on its thread with the commit; leave the thread unresolved. Then
        merge that branch forward into each later branch of the stack (`git merge`, never
        rebase or force-push).
      - **Its own slice's content changed** (a review fix, or a merge conflict resolved in
        its files): repeat step 5 for that change, as a fix round against the
        reviewer's prescription.
      - **Only merged in** (cleanly, from below or from `main`): fresh green CI on the
        new head is enough.
      - **Afterwards:** mark it ready again through step 6.
   4. **A draft slice PR** is an unfinished slice, or a ready PR that is being changed.
      Resume it, as in item 1.
   5. **Three or more ready slice PRs:** the stack is full. Report it.
      - Under `/loop`, schedule a wakeup in about 1800 s and end this iteration.
      - Otherwise, stop.
   6. **Otherwise:** the base is the newest open slice PR's branch, or `main` if there is
      none. Check it out, up to date with `origin`.
3. **`cargo xtask matrix check` must pass.** If it fails, fixing it is the slice.
4. **Run `cargo xtask fetch-spec`.**

## 1. Choose and brief

1. **Choose.** Run `cargo xtask matrix next -n 20`. Take 4 to 8 ready rows that belong
   together (the same milestone and area, or tightly coupled). The lead picks the order;
   the maintainer does not choose slices. If nothing is ready, report the blockers and
   stop the loop.
2. **Before a milestone's first slice,** collect every open decision its rows will need
   (ADRs, interpretations the standard leaves open, policy) and put them to the maintainer
   in one batch, in plain language, so slices do not stall waiting for answers.
3. **Brief.** Under `mode=pr`, create the branch `slice/NNNN-<name>` from the base found
   in step 0. Copy `references/slice-brief.md` to `docs/slices/NNNN-<name>.md`, with
   `state: in-progress`, and record the base in it.

## 2. Design → `analyzed`

- **Run sacm-designer** for SACM rows, and for any row that adds or changes public API.
  Ask for an exact brief: quoted clauses, acceptance criteria, exact names, field and key
  order, check order, diagnostic codes and golden samples, so the builder has nothing to
  guess (ADR 0006 amendment 4).
- **Library-only, tooling, QA and docs rows** without public API: skip the designer, and
  write their acceptance yourself.
- **Apply the result to the matrix yourself:** `requirement`, `source` (with the quoted
  clause) and `acceptance`; new obligations become new rows. Set `status = "analyzed"`,
  then run `cargo xtask matrix render && cargo xtask matrix check`.
- **ADRs:** an ADR is `proposed` until the user approves it. Record the key signatures in
  the brief.
- **Overlap (ADR 0006 amendment 4):** start the next slice's design while the current
  slice waits for CI or the verifier; the designer only reads. Its tests and code wait
  for the current PR to merge, or stack on its branch.

## 3. Failing tests → `tests-failing` (builder, step 1)

- **Run sacm-builder for step 1** with the slice brief and the design brief: tests and
  fixtures only, then stop.
- **Confirm the red yourself:** the new tests fail for the stated reason. A test that
  passes before implementation is a finding.
- **Snapshot the red** for the verifier, in the session scratchpad:
  `git ls-files -z -co --exclude-standard -- 'crates/*/tests/*' 'crates/*/benches/*' fuzz xtask/tests '.claude/hooks/test_*.py' | tar --null -T - -cf <scratchpad>/red-NNNN.tar`.
- **Update the matrix:** cite the tests in their rows, and set `tests-failing`.
- **Do not commit** in this state.
- **Dev-dependencies:** add them yourself, by the rust-library-craft rules.

## 4. Implementation → `implemented` (builder, step 2)

- **Run sacm-builder for step 2** with a new brief: the confirmed red, and that no step-1
  test may be weakened.
- **Run the local gate yourself.**
- **A disputed test:** decide who is wrong. If the test is, brief the builder to change
  exactly that test, and log the change in the slice brief so the verifier sees it.
- **Update the matrix:** cite the code, and set `implemented`.

## 5. Verification → `verified`

1. **`mode=pr`:** commit and push.
   - Open a draft PR (`gh pr create --draft --base <base>`), unless
     `gh pr list --head <branch>` already lists one.
   - **Stacked:** when the base is not `main`, start the PR body with
     "Stacked on #N: merge #N first."
2. **Wait for CI.** Download the PR-diff shards' reports (`mutants-diff-shard-N` in the
   CI run). The PR-diff job is the per-slice mutation evidence. The full `mutants.yml` run
   happens only at a milestone's close (step 7) or weekly, unless the slice changes the
   mutation config or removes tests.
   - If the slice removes or weakens tests, dispatch `mutants.yml` on the head
     (`gh workflow run mutants.yml --ref <branch>`), because a test-only diff gives the
     PR-diff job nothing to mutate.
3. **Run sacm-verifier** with the rows, the acceptance, the diff, the run evidence and the
   red snapshot, but not your opinion. It also reviews the public docs and rustdoc of what
   the slice changed; there is no separate docs review.
   - **Depth:** full for rows that touch reading, writing or data integrity; tooling, QA
     and docs rows get one round against their acceptance.
   - **Findings** are **blocking** (wrong behaviour, a test gap in new code, a misreading
     of the standard, a false claim a user could rely on) or **non-blocking**. Only
     blocking findings fail a row.
4. **One fix round.** Route every finding at once: tests, code and docs to the builder
   (a test changes only where a finding names it), matrix, ADRs and workflows to you.
   - **Wording** (docs, comments, citations) is fixed in this round with the verifier's
     exact text. It is never logged as a follow-up.
   - **Other non-blocking findings** go to `docs/follow-ups.md`.
   - **Re-verify** against the previous prescription only. A wording-only round after a
     PASS needs no re-verification; check the text yourself.
   - **Still FAIL after the fix round:** stop and ask the maintainer.
5. **On PASS:** write a short record from `references/verification-record.md`, set the
   rows to `verified` with `verification = "<record>"`, then render and check.

**`mode=local`:** nothing is pushed, so there is no Linux mutants report, and mutated rows
cannot PASS. Stop and ask the user.

## 6. Close

1. **CHANGELOG:** add an entry for user-visible changes only.
2. **The brief:** set `state: done`.
3. **Run the full gate.**
4. **Commit:** `feat(<area>): <summary> [<ROW-IDs>]`, with the session's attribution
   trailer.
5. **`mode=pr`:**
   - push, and update the PR body (`gh pr edit N --body-file …`);
   - when PASS and CI are green on the final head, run `gh pr ready` and **merge the PR
     yourself** (`gh pr merge N --merge --match-head-commit <sha>`; the maintainer's
     standing authorization, 2026-11-13). Never force-push; never merge with red CI or
     without a recorded PASS;
   - then retarget the next PR, if any, to `main` and merge `main` into it.
   - Under `/loop`, go straight on to the next slice. It stacks on this PR.

## 7. Stop and report when

- a milestone's last row is verified (dispatch the full `mutants.yml` run on `main` and
  triage it, run **matrix-steward**, clear `docs/follow-ups.md`, and present the summary);
- an ADR or open question blocks the work;
- a row still fails after its fix round;
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
