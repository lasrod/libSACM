# Mutation-testing triage

Every cargo-mutants survivor gets a disposition here (LIB-QA-007). Every exclusion in `.cargo/mutants.toml` must have a row here with its reason and owning matrix row; `lib_qa_007_mutants_config_is_reviewed` checks that the two agree.

## Configuration in force

- `.cargo/mutants.toml` sets `cap_lints = true`, `all_features = true` and `test_workspace = false`. There is one exclusion, listed in the table below.
- `skip_calls_defaults = true` is the cargo-mutants default: calls to `with_capacity` are not mutated.
- Tests run under cargo-nextest ([ADR 0009](../adr/0009-nextest-for-mutation-testing.md)): `test_tool = "nextest"`, with the `mutants` profile of `.config/nextest.toml`. That profile stops every running test at the first failure, so a mutant is caught whatever the order of the test targets.
- Doctests do not run under mutation, because nextest does not run them. A mutant that only a doctest kills shows as missed; add a unit or integration test that kills it.

## Dispositions

A disposition is one of:
- **killed:** a test now catches the mutant. Name the test, and cite it under the row that owns the code.
- **equivalent:** no input reachable through the public API or a binary tells the mutant apart from the original. Cite the invariant that guarantees this.
- **accepted A, B or C:** only for the classes listed in the [prescription](../verification/0003-m0-mutation-testing-prescription.md), P8.

Only an equivalent or accepted mutant may be excluded. An excluded mutant's Excluded cell is `<key>: <entry>`, naming the `exclude_re`, `exclude_globs` or `skip_calls` entry exactly as the configuration has it. Mutant names include their line and column, so a change that moves an excluded mutant needs its row updated.

| Mutant | Disposition | Row | Reason | Excluded |
|---|---|---|---|---|
| `xtask/src/lib.rs:131:5: replace repo_root -> PathBuf with Default::default()` | killed | LIB-TOOL-001 | `lib_tool_001_repo_root_is_the_workspace_root_above_the_xtask_crate` asserts the root is absolute, sits above `xtask/` and holds the workspace manifest. | — |
| `xtask/src/fetch.rs:168:5: replace curl_download -> Result<(), String> with Ok(())` | killed | LIB-TOOL-002 | `lib_tool_002_curl_download_writes_the_bytes_at_the_url_to_the_output_path` runs the real `curl` on a `file://` URL and requires identical bytes; `lib_tool_002_curl_download_reports_a_failed_transfer_as_an_error` requires `Err`. | — |
| `xtask/src/matrix.rs:432:17: delete match arm '[' in strip_attributes` | killed | LIB-TOOL-001 | `lib_tool_001_brackets_nested_in_an_attribute_do_not_close_it`: attributes with nested brackets are read whole. | — |
| `xtask/src/matrix.rs:432:30: replace += with -= in strip_attributes` | killed | LIB-TOOL-001 | `lib_tool_001_brackets_nested_in_an_attribute_do_not_close_it` | — |
| `xtask/src/matrix.rs:432:30: replace += with *= in strip_attributes` | killed | LIB-TOOL-001 | `lib_tool_001_brackets_nested_in_an_attribute_do_not_close_it` | — |
| `xtask/src/matrix.rs:460:35: replace == with != in defined_functions` | killed | LIB-TOOL-001 | Killed by `lib_tool_001_function_scan_ignores_fn_and_def_at_the_end_of_a_longer_word`. **This mutant is absent from the final set:** the line was rewritten in `3acb370`. The verifier reproduced the kill on the pre-rewrite code. | — |
| `xtask/src/matrix.rs:464:58: replace \|\| with && in defined_functions` | killed | LIB-TOOL-001 | Killed by `lib_tool_001_function_scan_ignores_fn_and_def_at_the_end_of_a_longer_word`. **Absent from the final set** (the line was rewritten in `3acb370`); kill reproduced on the pre-rewrite code. | — |
| `xtask/src/matrix.rs:470:28: replace && with \|\| in defined_functions` | killed | LIB-TOOL-001 | Killed by `lib_tool_001_function_scan_ignores_fn_and_def_at_the_end_of_a_longer_word`. **Absent from the final set:** the line was rewritten in `3acb370`. `--list` at `9046996` shows `474:28` and no `470:28`. The verifier reproduced the kill on the pre-rewrite code. The corresponding `&&` in the rewritten code (`starts_word && !name.is_empty()`, `474:28`) is caught in the final run. | — |
| `xtask/src/matrix.rs:465:35: replace + with - in defined_functions` | killed | LIB-TOOL-001 | Was a timeout (an infinite loop). Killed by `lib_tool_001_function_scan_finds_the_test_an_attribute_marks`, `lib_tool_001_function_scan_finds_definitions_without_a_test_attribute` and `lib_tool_001_function_scan_ignores_fn_and_def_at_the_end_of_a_longer_word`. They run the scanner under a 5 s deadline, so the hang (or the underflow panic) is a failure. **Absent from the final set:** the loop now iterates `match_indices` (`3acb370`). Kill reproduced on the pre-rewrite code. | — |
| `xtask/src/matrix.rs:465:35: replace + with * in defined_functions` | killed | LIB-TOOL-001 | Was a timeout. Killed by `lib_tool_001_function_scan_finds_the_test_an_attribute_marks` and `lib_tool_001_function_scan_finds_definitions_without_a_test_attribute` (5 s deadline). **Absent from the final set** (`3acb370`); kill reproduced on the pre-rewrite code. | — |
| `xtask/src/matrix.rs:818:58: replace == with != in check_cycles::visit` | killed | LIB-TOOL-001 | `lib_tool_001_cycle_message_names_only_the_rows_on_the_cycle` asserts the exact cycle path. | — |
| `xtask/src/matrix.rs:974:58: replace == with != in render_milestone` | killed | LIB-TOOL-001 | `lib_tool_001_render_lists_each_row_under_its_own_milestone_only` | — |
| `xtask/src/matrix.rs:1150:37: replace == with != in cmd_status` | killed | LIB-TOOL-001 | `lib_tool_001_run_matrix_status_counts_each_milestones_rows_by_status` asserts every status line exactly. | — |
| `xtask/src/matrix.rs:1152:56: replace == with != in cmd_status` | killed | LIB-TOOL-001 | `lib_tool_001_run_matrix_status_counts_each_milestones_rows_by_status` | — |
| `xtask/src/matrix.rs:1153:48: delete ! in cmd_status` | killed | LIB-TOOL-001 | `lib_tool_001_run_matrix_status_counts_each_milestones_rows_by_status` | — |
| `xtask/src/matrix.rs:1156:53: replace == with != in cmd_status` | killed | LIB-TOOL-001 | `lib_tool_001_run_matrix_status_counts_each_milestones_rows_by_status` | — |
| `xtask/src/matrix.rs:1157:18: replace > with == in cmd_status` | killed | LIB-TOOL-001 | `lib_tool_001_run_matrix_status_counts_each_milestones_rows_by_status` | — |
| `xtask/src/matrix.rs:1157:18: replace > with < in cmd_status` | killed | LIB-TOOL-001 | `lib_tool_001_run_matrix_status_counts_each_milestones_rows_by_status` | — |
| `xtask/src/matrix.rs:1157:18: replace > with >= in cmd_status` | killed | LIB-TOOL-001 | `lib_tool_001_run_matrix_status_counts_each_milestones_rows_by_status` | — |
| `crates/sacm/src/xmi.rs:241:9: replace ReadOutput::diagnostics -> &[Diagnostic] with Vec::leak(Vec::new())` | equivalent | SACM23-XMI-001 | `read` reports no diagnostic in this release: every `ReadOutput` is built with an empty list (`xmi.rs`, `read`), so the mutant returns what the original does. Remove the exclusion with the first row that reports a diagnostic. | `exclude_re: ^crates/sacm/src/xmi\.rs:\d+:\d+: replace ReadOutput::diagnostics -> &\[Diagnostic\] with Vec::leak\(Vec::new\(\)\)$` |

## History

The run-by-run record (baseline and final runs, unviable mutants per slice, the target-order timeout that led to nextest) is in git history and in [docs/verification](../verification/).
