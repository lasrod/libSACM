---
slice: 0003
name: m0-mutation-testing
state: done
step: close
rows: [LIB-QA-007]
mode: pr
---

# Slice 0003: Mutation testing with cargo-mutants

## Scope

Adopt `cargo-mutants` so the gaps verifiers found by hand in slices 0001 and 0002 are found mechanically, the same way on every PR. The maintainer brought this row forward from M7 for that reason.

| Row | Title | Status at start |
|---|---|---|
| LIB-QA-007 | Mutation testing | planned |

## Out of scope

- LIB-QA-009 (pinning third-party actions): this slice avoids adding a third-party action. cargo-mutants is installed with `cargo install --locked` at a pinned version.
- Raising coverage of the `sacm` crate's future model code: the crate barely exists yet; this slice sets up the mechanism and triages today's survivors.

## Normative basis

None (a quality row). The source is project policy and the verifier's test-adequacy check.

## Design (proposed; pre-verified before work starts)

- **Tool:** `cargo-mutants` 27.1.0 (MIT), installed in CI with `cargo install cargo-mutants --locked --version 27.1.0`. No third-party action.
- **Configuration:** `.cargo/mutants.toml`. Every exclusion has a stated reason, and nothing is excluded silently.
- **On pull requests:** a `mutants` job in `ci.yml` on ubuntu runs `cargo mutants --in-diff <PR diff>` over the workspace (`sacm`, `sacm-cli`, `xtask`). The job fails on any missed mutant in changed code. Linux, because some kills come from Linux-only tests.
- **Full run:** a separate workflow (`.github/workflows/mutants.yml`) runs on a weekly schedule and on `workflow_dispatch`. It runs the whole workspace, sharded, and uploads `mutants.out` as an artifact.
- **Baseline:** the first full run, triggered on this branch, lists today's survivors. Each survivor is either killed by a new test, cited under the row that owns the code, or recorded as an equivalent mutant in `docs/quality/mutants-triage.md` with a reason. The known candidates are X-flush/M5 from slice 0001 and R23b.
- **Process:** `sacm-verifier` reviews the mutants report for the slice's code instead of hand-picking mutations. Its definition and the `sacm-slice` skill say so.
- **Ownership:** `ci.yml`, `mutants.yml` and `.cargo/mutants.toml` are repository configuration. No agent's write scope covers them, so the lead edits them, as with `deny.toml` in slice 0002. The test engineer writes the regression tests that pin them, and the verifier verifies them.

## Tests

To be listed after the test step.

## Log

- 2026-11-07 preflight: main at 2c32b39 (PR #8 merged), no open PRs, matrix gate OK. Branch `slice/0003-m0-mutation-testing`.
- 2026-11-07 pre-verification: the verifier gave the complete prescription (docs/verification/0003-m0-mutation-testing-prescription.md). Maintainer decisions: D2 a pinned if in ci.yml; D10 a full run on all Rust PRs; D14 a draft PR before verification. Row criteria reworded.
- 2026-11-07 tests first (P7): the test engineer wrote 37 lib_qa_007_* tests in xtask/tests/repo_checks.rs. 36 fail against the real files with their expected codes; the SKILL.md check already passes (the lead's step-7 edit). Red confirmed by the lead. Row -> tests-failing.
- 2026-11-07 implementation (lead, configuration): mutants job in ci.yml, mutants.yml, .cargo/mutants.toml, /mutants.out* in .gitignore, a CONTRIBUTING section, the triage document skeleton, and the verifier-definition and skill text, all copied from the test engineer's reference fixtures. All 67 repo_checks tests pass; the full gate passes. Row -> implemented. Pushed as a draft PR for the Linux baseline run.
- 2026-11-07 baseline full run (mutants.yml run 42368743129 at d8efe70): 222 mutants; 199 caught, 17 missed, 2 timeout, 4 unviable; about 4 min per shard.
- 2026-11-07 triage: the test engineer wrote 10 killing tests (8 cited under LIB-TOOL-001, 2 under LIB-TOOL-002). The implementer rewrote the defined_functions loop over match_indices, so the two timeout mutants (465:35) no longer exist; it hand-checked the new lines' mutants with sed applied to the working tree, restored byte-identical (process note: not a copy). Pushed for a confirming full run.
- 2026-11-07 confirming full run (mutants.yml run 42370175164 at 3acb370): 216 mutants, 212 caught, 0 missed, 0 timeout, 4 unviable; all shards exited 0. PR job (ci.yml run 42370175166): 12/12 caught. The triage document is complete: all 19 baseline survivors killed, with no exclusions.
- 2026-11-07 verification: sacm-verifier, against the 0003 prescription.
- 2026-11-07 verification round 1: FAIL (docs only: F1 exclusions unchecked; F2 mode=local had no report; F3 no stated mechanical path for hand-reasoned mutations; F4 triage text); P9 confirmed. docs/verification/0003-m0-mutation-testing.md. Maintainer decision on F2: in mode=local, no PASS for mutated-code rows without a Linux report.
- 2026-11-07 fix round 1: the test engineer extended the doc checks first (red: three [doc-verifier] problems and one [doc-skill]); the lead wrote the sacm-verifier.md and SKILL.md paragraphs and corrected the triage text (F4); follow-ups: the implementer definition (scratch copies under target/) and docs/loop.md (the draft PR step). All 67 repo_checks pass.
- 2026-11-07 verification round 2: FAIL. F1 and F3 met. Remaining: R2-1 (stale 'later commits are docs-only' line), R2-1b (the 470:28 row note), R2-2 (the mode=local paragraph says to push; loop.md not scoped to mode=pr), R2-3 (the implementer scratch-copy example copies HEAD, not the working tree). All are lead-owned docs. The verifier-definition diff is approved as-is. docs/verification/0003-m0-mutation-testing-round2.md.
- 2026-11-07 stopped: second FAIL; awaiting maintainer approval of fix round 2.
- 2026-11-07 maintainer approved fix round 2. The lead fixed R2-1 (triage 'Final run' sentence), R2-1b (the 470:28 row), R2-2 (SKILL mode=local: stop at a checkpoint, no push; loop.md scoped to mode=pr) and R2-3 (implementer scratch copy: working tree, no target/ or .git, its own target dir; example tested). The gate passes.
- 2026-11-07 verification round 3: LIB-QA-007 PASS at 4f1641c (ci.yml 42380451077, mutants.yml 42380450887). The verifier-definition diff is approved as-is across the slice. docs/verification/0003-m0-mutation-testing-round3.md. New follow-up rows LIB-QA-010 and LIB-TOOL-012.
- 2026-11-07 closed. Slice 0003 verified: LIB-QA-007, with 10 new killing tests under LIB-TOOL-001 and LIB-TOOL-002.
