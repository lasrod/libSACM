# Mutation-testing triage

This is where every cargo-mutants survivor in the workspace gets its disposition (LIB-QA-007). Every exclusion in `.cargo/mutants.toml` must appear here with its reason and owning matrix row; the test `lib_qa_007_mutants_config_is_reviewed` checks that the two lists agree exactly.

- **Author:** the lead (slice 0003). The verifier independently re-derives every *equivalent* and *accepted* claim.
- **Configuration in force:** `.cargo/mutants.toml` sets `cap_lints = true`, `all_features = true` and `test_workspace = false`. There are **no exclusions**. `skip_calls_defaults = true` is the cargo-mutants default: calls to `with_capacity` are not mutated.

## Baseline run

- **Run:** [mutants.yml run 42368743129](https://github.com/lasrod/libSACM/actions/runs/42368743129) on PR #9, head `d8efe70`, using cargo-mutants 27.1.0 (version taken from `outcomes.json`). All work was on ubuntu-latest.
- **Totals:** 222 mutants (`--workspace`): 199 caught, **17 missed**, **2 timed out**, 4 unviable.

| Shard | Caught | Missed | Timeout | Unviable | Duration |
|---|---:|---:|---:|---:|---|
| 0 | 53 | 2 | 0 | 1 | 3 min 07 s |
| 1 | 48 | 6 | 2 | 0 | 3 min 53 s |
| 2 | 55 | 0 | 0 | 1 | 3 min 50 s |
| 3 | 43 | 9 | 0 | 2 | 3 min 53 s |

- **The PR job** (`ci.yml` `mutants`) took 55 s and tested no mutants: that commit changed no `src` code.
- **Timeouts:** about 4 minutes per shard, against the estimate of 20–30 minutes. `timeout-minutes: 60` is kept for both jobs, which leaves about 15 times headroom for the coming `sacm` model code.
- **The 4 unviable mutants** don't compile, because their replacement values need a `Default` that the return types (`Matrix`, `Outcome`, `&Row`) don't implement. They are structural and hide no survivor:
  - `xtask/src/fetch.rs:110:5: replace fetch_into -> Result<Vec<Outcome>, String> with Ok(vec![Default::default()])`
  - `xtask/src/matrix.rs:527:5: replace load -> Result<Matrix, String> with Ok(Default::default())`
  - `xtask/src/matrix.rs:1011:5: replace ready -> Vec<&Row> with vec![Box::leak(Box::new(Default::default()))]`
  - `xtask/src/matrix.rs:1052:5: replace load_from -> Result<Matrix, String> with Ok(Default::default())`

## Final run

- **Run:** [mutants.yml run 42370175164](https://github.com/lasrod/libSACM/actions/runs/42370175164), head `3acb370`.
- **Totals:** 216 mutants: **212 caught, 0 missed, 0 timed out**, and the same 4 unviable. Every shard exited 0.
- **The PR job** ([ci.yml run 42370175166](https://github.com/lasrod/libSACM/actions/runs/42370175166)) caught 12 of 12.
- **Why 6 fewer mutants:** rewriting `defined_functions` over `match_indices` removed the old loop's index arithmetic, and with it six mutants, including the two that timed out.
- **The three sacm-cli BrokenPipe guard mutants** (`main.rs:31`) are caught on Linux, by the LIB-CLI-001 Linux tests.

## Dispositions

A disposition is one of:
- **killed:** a test now catches the mutant. It is named, and cited under the row that owns the code.
- **equivalent:** no input reachable through the public API or a binary tells the mutant apart from the original. The entry cites the invariant that guarantees this.
- **accepted A, B or C:** only for the classes listed in the [prescription](../verification/0003-m0-mutation-testing-prescription.md), P8.

Only an equivalent or accepted mutant may be excluded. Every baseline survivor was **killed**.

| Mutant | Disposition | Row | Reason | Excluded |
|---|---|---|---|---|
| `xtask/src/lib.rs:131:5: replace repo_root -> PathBuf with Default::default()` | killed | LIB-TOOL-001 | `lib_tool_001_repo_root_is_the_workspace_root_above_the_xtask_crate` asserts the root is absolute, sits above `xtask/` and holds the workspace manifest. | — |
| `xtask/src/fetch.rs:168:5: replace curl_download -> Result<(), String> with Ok(())` | killed | LIB-TOOL-002 | `lib_tool_002_curl_download_writes_the_bytes_at_the_url_to_the_output_path` runs the real `curl` on a `file://` URL and requires identical bytes; `lib_tool_002_curl_download_reports_a_failed_transfer_as_an_error` requires `Err`. | — |
| `xtask/src/matrix.rs:432:17: delete match arm '[' in strip_attributes` | killed | LIB-TOOL-001 | `lib_tool_001_brackets_nested_in_an_attribute_do_not_close_it`: attributes with nested brackets are read whole. | — |
| `xtask/src/matrix.rs:432:30: replace += with -= in strip_attributes` | killed | LIB-TOOL-001 | `lib_tool_001_brackets_nested_in_an_attribute_do_not_close_it` | — |
| `xtask/src/matrix.rs:432:30: replace += with *= in strip_attributes` | killed | LIB-TOOL-001 | `lib_tool_001_brackets_nested_in_an_attribute_do_not_close_it` | — |
| `xtask/src/matrix.rs:460:35: replace == with != in defined_functions` | killed | LIB-TOOL-001 | `lib_tool_001_function_scan_ignores_fn_and_def_at_the_end_of_a_longer_word`. The line was rewritten in `3acb370`, and the final run catches every mutant of the new code. | — |
| `xtask/src/matrix.rs:464:58: replace \|\| with && in defined_functions` | killed | LIB-TOOL-001 | `lib_tool_001_function_scan_ignores_fn_and_def_at_the_end_of_a_longer_word`. Rewritten in `3acb370`, as above. | — |
| `xtask/src/matrix.rs:470:28: replace && with \|\| in defined_functions` | killed | LIB-TOOL-001 | `lib_tool_001_function_scan_ignores_fn_and_def_at_the_end_of_a_longer_word`. The new `starts_word && !name.is_empty()` mutant is caught. | — |
| `xtask/src/matrix.rs:465:35: replace + with - in defined_functions` | killed | LIB-TOOL-001 | Was a timeout (an infinite loop). The loop now iterates `match_indices`, so this index arithmetic no longer exists (`3acb370`). The deadline tests in `xtask/tests/function_scan.rs` also turn any hang in the scanner into a failure. | — |
| `xtask/src/matrix.rs:465:35: replace + with * in defined_functions` | killed | LIB-TOOL-001 | Was a timeout; removed with the loop, as above. | — |
| `xtask/src/matrix.rs:818:58: replace == with != in check_cycles::visit` | killed | LIB-TOOL-001 | `lib_tool_001_cycle_message_names_only_the_rows_on_the_cycle` asserts the exact cycle path. | — |
| `xtask/src/matrix.rs:974:58: replace == with != in render_milestone` | killed | LIB-TOOL-001 | `lib_tool_001_render_lists_each_row_under_its_own_milestone_only` | — |
| `xtask/src/matrix.rs:1150:37: replace == with != in cmd_status` | killed | LIB-TOOL-001 | `lib_tool_001_run_matrix_status_counts_each_milestones_rows_by_status` asserts every status line exactly. | — |
| `xtask/src/matrix.rs:1152:56: replace == with != in cmd_status` | killed | LIB-TOOL-001 | `lib_tool_001_run_matrix_status_counts_each_milestones_rows_by_status` | — |
| `xtask/src/matrix.rs:1153:48: delete ! in cmd_status` | killed | LIB-TOOL-001 | `lib_tool_001_run_matrix_status_counts_each_milestones_rows_by_status` | — |
| `xtask/src/matrix.rs:1156:53: replace == with != in cmd_status` | killed | LIB-TOOL-001 | `lib_tool_001_run_matrix_status_counts_each_milestones_rows_by_status` | — |
| `xtask/src/matrix.rs:1157:18: replace > with == in cmd_status` | killed | LIB-TOOL-001 | `lib_tool_001_run_matrix_status_counts_each_milestones_rows_by_status` | — |
| `xtask/src/matrix.rs:1157:18: replace > with < in cmd_status` | killed | LIB-TOOL-001 | `lib_tool_001_run_matrix_status_counts_each_milestones_rows_by_status` | — |
| `xtask/src/matrix.rs:1157:18: replace > with >= in cmd_status` | killed | LIB-TOOL-001 | `lib_tool_001_run_matrix_status_counts_each_milestones_rows_by_status` | — |

## Known items carried from earlier verification

- **X-flush (slice 0001, round 6):** closed. The generated `run` mutants in `xtask/src/lib.rs` are all caught on Linux in the final run, including by `lib_tool_001_run_turns_a_flush_failure_after_a_successful_command_into_an_error`.
- **M5 (flush only on success):** not a cargo-mutants operator, so it is outside the generated set. The round-6 addendum gives the reason. It needs no exclusion.
- **R23b (`looks_traced` with an empty area):** the generated `looks_traced` mutants are all caught in both the baseline and the final run. No survivor.
- **`repo_root -> Default::default()`:** killed under LIB-TOOL-001 (see the table above).
- **The three sacm-cli BrokenPipe guard mutants:** caught on Linux by the LIB-CLI-001 Linux tests.
