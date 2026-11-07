# Verification 0003, round 3: m0-mutation-testing (after fix round 2)

- Date: 2026-11-07
- Verdict: **PASS**. LIB-QA-007: PASS.
- Verifier: `sacm-verifier` (independent; read-only)
- Head verified: `4f1641cdcaa7a8874d5725f89c177b7a2cee8f74` (draft PR #9)
- Contract: the remainder in the [round 2 record](0003-m0-mutation-testing-round2.md) (R2-1, R2-1b, R2-2, R2-3), plus its closing-evidence list. Earlier contracts: [round 1](0003-m0-mutation-testing.md) and [the prescription](0003-m0-mutation-testing-prescription.md).

## Rows verified

| Row | Title | Evidence |
|---|---|---|
| LIB-QA-007 | Mutation testing | The 37 cited `lib_qa_007_*` tests. The Linux mutation runs below. `docs/quality/mutants-triage.md`, in which all 19 baseline survivors are killed and nothing is excluded. |

## Linux evidence at the verified head

The verifier downloaded these runs itself.

- `ci.yml` [42380451077](https://github.com/lasrod/libSACM/actions/runs/42380451077): all 11 jobs succeeded. They include `mutants (PR diff)`, which caught 12 of 12, and `test (ubuntu-latest)`.
- `mutants.yml` [42380450887](https://github.com/lasrod/libSACM/actions/runs/42380450887): all 4 shards succeeded. 216 mutants: **212 caught, 0 missed, 0 timeout**, and 4 unviable, which are triaged and structural.
- Every shard's outcome lists are byte-identical to the full runs at `3acb370`, `89be9e2`, `9046996` and `f16e411`.
- `cargo mutants --workspace --list` and `--list --no-config` return the same 216 lines, so the excluded set is empty, matching the triage.

## Round-2 prescription

| Id | Result |
|---|---|
| R2-1 | **Met.** Every sentence of "Final run" is true at `4f1641c`, and each later commit's full run is identical. |
| R2-1b | **Met.** The `470:28` row records that the mutant is absent from the final set (`--list` lists `474:28` and no `470:28`). It also records that the kill was reproduced on the pre-rewrite code. |
| R2-2a | **Met.** The SKILL `mode=local` paragraph: such rows stay `implemented`, and the lead stops at a checkpoint and asks for a push or a `mode=pr` re-run ("Do not push on your own"). It stays before **sacm-verifier**, and the check still pins it. |
| R2-2b | **Met.** In `docs/loop.md`, the PR and draft-PR paragraphs are scoped to `mode=pr`, and the `mode=local` text uses the decision's wording. |
| R2-3 | **Met.** The implementer's scratch-copy rule states that the copy holds the working tree (including uncommitted changes and untracked, non-ignored files), excludes `target/` and `.git`, and uses its own target directory. The verifier tested the example: it copies uncommitted and untracked files and excludes ignored files, `target/` and `.git`. |

## Doc checks against the real files

These were probed in the scratch copy `target/verify-r3/copy`, never in the working tree:
- **SKILL:** deleting the `mode=local` paragraph, relabelling it, moving it below **sacm-verifier**, weakening "may not PASS", or deleting the draft-PR paragraph each produces `[doc-skill]`.
- **Verifier:** deleting the F1, F2 or F3 paragraph each produces `[doc-verifier]`.
- After the files were restored, both checks pass.

## The verifier-definition diff, across the slice

**Approved as-is.** `git diff 2c32b39..4f1641c -- .claude/agents/sacm-verifier.md` adds 33 lines and removes 4. The only text removed is the stale "`cargo mutants --file <path>` once it is adopted" sentence, which the F3 paragraph replaces. All ten required check titles are kept and pinned, and no check is lost or weakened. **The maintainer approves this diff at PR #9.**

## Rows already verified that gained tests in this slice (P9, accepted in round 1)

- **LIB-TOOL-001:**
  - `xtask/tests/function_scan.rs`: 3 tests.
  - `xtask/tests/matrix.rs`: `lib_tool_001_cycle_message_names_only_the_rows_on_the_cycle`, `lib_tool_001_brackets_nested_in_an_attribute_do_not_close_it`, `lib_tool_001_render_lists_each_row_under_its_own_milestone_only`.
  - `xtask/tests/run.rs`: `lib_tool_001_run_matrix_status_counts_each_milestones_rows_by_status`, `lib_tool_001_repo_root_is_the_workspace_root_above_the_xtask_crate`.
- **LIB-TOOL-002** (`xtask/tests/fetch.rs`): `lib_tool_002_curl_download_writes_the_bytes_at_the_url_to_the_output_path`, `lib_tool_002_curl_download_reports_a_failed_transfer_as_an_error`.

## Gates (run by the verifier)

All pass:
- fmt;
- clippy (`-D warnings`);
- `cargo test --workspace --all-features` and `cargo test -p xtask`;
- rustdoc (`-D warnings`);
- `matrix check` (146 rows);
- `cargo deny check`;
- the hook tests (39).

## Observations (not blocking)

- **The R2-3 example's file list.** `git ls-files --cached` also lists tracked files that were deleted from the working tree, so tar warns about them. Starting with `rm -rf target/scratch` and adding `--ignore-failed-read` would clean that up.
- **What the doc checks match.** They match phrases, not meaning. `docs/loop.md` and `sacm-implementer.md` are not pinned by any check.

## Follow-ups

- LIB-QA-010: a mechanised check that each exclusion matches exactly one mutant.
- LIB-TOOL-012: test `curl_download` against a local HTTP server for `--fail` and `--location`.

## Addendum: the Copilot review fixes (rounds 4 and 5, PR #9)

Copilot left four comments when PR #9 was marked ready.
- **Fixed in `9429bf9` and `893a55b`:**
  - **4502142536:** the `.gitignore` check trimmed leading whitespace.
  - **4502142569:** the scratch copy was reused over a stale tree.
  - **4502142601:** step 8.5 created a duplicate PR.
  - **4502142636:** there was no full-run dispatch when `mutants.yml` was skipped.
- **Round 4 (verifier, head `893a55b`): FAIL on 4502142536 only.**
  - **R4-1:** `trim_end` also strips trailing tabs, which git keeps.
  - **R4-2:** a re-include was caught only if it contained the literal `mutants.out`, so `!*` and `!/mutants.ou?` passed.
  - The verifier recorded that this check had been weaker than round 3 stated: `trim` hid both gaps. The real `.gitignore` was correct throughout.
- **Fixed in `474f850`:**
  - only U+0020 trailing spaces are stripped;
  - every line starting with `!` is reported (D12);
  - new controls for a trailing tab, an escaped trailing space, `!*`, `!/mutants.ou?`, an escaped `\!` and CRLF.
  - `f1a7a7c` fixes the step-7 wording (O1, O2) and adds LIB-QA-011 (O3).
- **Round 5 (verifier, head `f1a7a7c7fe39114d87d216e700157058c254acb2`): PASS.**
  - All four comments pass.
  - All 21 hand mutations of the checker are killed. They were applied in a scratch copy under `target/`, never in the working tree.
  - Every case agrees with `git check-ignore`.
  - CI [42383300662](https://github.com/lasrod/libSACM/actions/runs/42383300662) succeeded. Its `mutants (PR diff)` job caught 12 of 12.
  - mutants.yml [42383300659](https://github.com/lasrod/libSACM/actions/runs/42383300659): 216 mutants, 212 caught, 0 missed, 0 timeout, 4 unviable. The outcome lists are identical to the earlier runs.
  - **LIB-QA-007 stays verified.**
- **Non-blocking:** the checker refuses some harmless files: a UTF-8 BOM before the rule, an unrelated `!` line, and a re-include inside an excluded directory. Refusing them is the safe direction under D12.
