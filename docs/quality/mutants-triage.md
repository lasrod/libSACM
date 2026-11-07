# Mutation-testing triage

This is where every cargo-mutants survivor in the workspace gets its disposition (LIB-QA-007). Every exclusion in `.cargo/mutants.toml` must appear here, with its reason and owning matrix row, and a test (`lib_qa_007_mutants_config_is_reviewed`) checks that the two lists agree exactly.

- Author: the lead (slice 0003). The verifier independently re-derives every *equivalent* and *accepted* claim.
- Configuration in force: `.cargo/mutants.toml` sets `cap_lints = true`, `all_features = true` and `test_workspace = false`. It has no exclusions yet. `skip_calls_defaults = true`, which is cargo-mutants' default: calls to `with_capacity` are not mutated.

## Baseline run

**Pending.** It is the first full run of `.github/workflows/mutants.yml` on the slice 0003 draft PR. Once it finishes, this section records:
- the run URL and head SHA;
- the cargo-mutants version;
- the total mutant count;
- per-shard counts of caught, missed, timed-out and unviable mutants;
- the measured durations, and the `timeout-minutes` chosen from them.

## Dispositions

A disposition is one of:
- **killed**: a test now catches the mutant. It is named, and cited under the row that owns the code.
- **equivalent**: no input reachable through the public API or a binary can tell the mutant apart from the original, in return values, error text, bytes written, exit status, files or panics. The entry cites the invariant that guarantees this.
- **accepted A**: the code is compiled out on Linux by `cfg`.
- **accepted B**: the behaviour cannot be exercised hermetically on a Linux runner. The entry says why each hermetic technique fails, and names a follow-up row.
- **accepted C**: a timeout caused by a real hang.

Only an equivalent or accepted mutant may be excluded, and only by an exact `exclude_re` or `exclude_globs` entry.

| Mutant | Disposition | Row | Reason | Excluded |
|---|---|---|---|---|

## Known items carried from earlier verification

These are pending the baseline run.
- **X-flush (slice 0001, round 6):** closed by `lib_tool_001_run_turns_a_flush_failure_after_a_successful_command_into_an_error`. To be confirmed on Linux.
- **M5 (flush only when the command succeeds):** not a cargo-mutants operator, so it is outside the generated set. The round-6 addendum gives the reason. It needs no exclusion.
- **R23b (`looks_traced`, empty-area check):** maps to the generated `looks_traced` mutants. Any survivor will be killed.
- **`repo_root -> Default::default()`:** missed in the verifier's Windows sample. It will be killed under LIB-TOOL-001.
- **The three sacm-cli BrokenPipe guard mutants:** expected to be caught on Linux by the LIB-CLI-001 Linux tests.
