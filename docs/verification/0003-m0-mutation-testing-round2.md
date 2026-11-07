# Verification 0003, round 2: m0-mutation-testing (after fix round 1)

- Date: 2026-11-07
- Verdict: **FAIL** for LIB-QA-007. Every remaining item is a small documentation fix.
- Verifier: `sacm-verifier` (independent; read-only)
- Inspected: `89be9e2..9046996` (draft PR #9). This round changed no code or configuration. The only lines removed from `repo_checks.rs` are two doc-comment lines.
- Linux evidence at `9046996`:
  - `ci.yml` [42375258991](https://github.com/lasrod/libSACM/actions/runs/42375258991): 11 of 11 jobs passed. The PR mutants job caught 12 of 12.
  - `mutants.yml` [42375259067](https://github.com/lasrod/libSACM/actions/runs/42375259067): 216 mutants, 212 caught, 0 missed, 0 timeout, 4 unviable. The outcome lists are byte-identical to those at `3acb370`.
- Exclusions, checked under the new F1 rule: `--workspace --list` and `--list --no-config` both give 216 identical lines. The excluded set is empty, which matches the triaged set.

## Round-1 findings

| Finding | Result |
|---|---|
| F1 (exclusions re-derived) | **Met.** The paragraph is required by the check. Deleting it, dropping `--no-config` or dropping `re-derive` from the real file is reported. |
| F2 (no Linux report) | **Partly met.** The verifier paragraph and the SKILL statement are present, and the check pins both. But the SKILL paragraph also tells the lead to push in `mode=local` (R2-2). |
| F3 (scratch copy) | **Met.** |
| Tests first | **Reproduced.** The old files give exactly 3 `[doc-verifier]` problems and 1 `[doc-skill]` problem. |
| F4 (triage text) | (a), (c) and (d) are met. The 465:35 killers were re-derived on the pre-rewrite code. (b) is not met for the 470:28 row (R2-1b), and (e) is not met (R2-1). |
| Regression | None. |

**The verifier-definition diff is approved as-is.** It adds 15 lines and removes none, all inside check 3, and no existing check is lost or weakened. *Observation:* the F1 and F2 checks match phrases, not meaning. Pinning "no substitute" would be cheap hardening.

## Findings: the complete remainder for fix round 2

| Id | Severity | Finding | Required fix |
|---|---|---|---|
| R2-1 | Minor (inside F4(e); partly the verifier's miss: "code or configuration" should have read "code, tests or configuration") | `mutants-triage.md` says `3acb370` is "the last commit to change code or the mutation configuration; later commits change only docs". `9046996` changes `xtask/tests/repo_checks.rs`, so that is false. | Make every sentence of "Final run" true at the cited head. Either cite the run at `9046996` (or later) as the last commit to change code, tests or configuration, or reword the claim so it doesn't say later commits are docs-only. |
| R2-1b | Minor (inside F4(b)) | The `470:28` row lacks the "absent from the final set; kill reproduced on the pre-rewrite code" note. | Add both. Absence is confirmed by `--list` at `9046996`, which shows 474:28 and no 470:28. The pre-rewrite killer is `lib_tool_001_function_scan_ignores_fn_and_def_at_the_end_of_a_longer_word`. |
| R2-2 | Minor (inside F2(a)) | The SKILL step-7 `mode=local` paragraph ends "or push before verifying code". That contradicts `mode=local` and the push rule. `docs/loop.md` doesn't scope its draft-PR paragraph to `mode=pr`. | **(a)** Under `mode=local`, mutated-code rows stay `implemented` and are kept open. To verify them, the lead stops at a checkpoint and asks the maintainer to authorize a push, or to re-run the slice under `mode=pr`. The paragraph stays before **sacm-verifier** and keeps the phrases `mode=local`, `linux report` and `may not pass`. **(b)** In `docs/loop.md`, scope the draft-PR paragraph to `mode=pr`, and word the `mode=local` sentence as "until a Linux report for that commit exists". |
| R2-3 | Minor, **new class**, non-blocking (the implementer definition was outside the prescription) | The example in `sacm-implementer.md`, `git archive HEAD \| tar -x -C target/scratch`, copies the last commit, not the uncommitted work. `tar -C` also fails when the directory is missing. And copies that share a `CARGO_TARGET_DIR` reuse test binaries whose `env!("CARGO_MANIFEST_DIR")` points at the other copy. | Replace the example with one that copies the working tree, including uncommitted and untracked non-ignored files, excludes `target/` and `.git`, and creates the directory first. Or state those properties instead of giving a command. Add: build in the copy's own target directory. |

## Closing evidence required

- The full gate passes.
- The fixes are pushed, and `ci.yml` and `mutants.yml` are green at the new head.
- The outcome lists there are compared with `9046996`'s.
- The record cites those runs and the new head SHA.
- Re-verification covers the three documents' diff plus those runs.

## Stop

This is the second FAIL for LIB-QA-007, so the loop stops for the maintainer, as the slice skill requires.
