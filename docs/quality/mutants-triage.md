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

- **Run:** [mutants.yml run 42370175164](https://github.com/lasrod/libSACM/actions/runs/42370175164), at `3acb370`. That is the first run with no survivors. `3acb370` is the last commit in slice 0003 to change production code or the mutation configuration. Later commits changed docs and the xtask doc-check tests. Each of them got a full run, and those runs' outcome lists are identical to this one's. The verification record cites the runs at the head it verifies.
- **Totals:** 216 mutants: **212 caught, 0 missed, 0 timed out, 4 unviable**. Every shard exited 0.
- **The PR job** ([ci.yml run 42370175166](https://github.com/lasrod/libSACM/actions/runs/42370175166)) caught 12 of 12.
- **Why 6 fewer mutants:** rewriting `defined_functions` over `match_indices` removed the old loop's index arithmetic, and with it six mutants, including the two that timed out.
- **The 4 unviable mutants** are the same four as in the baseline, at shifted positions:
  - `fetch.rs:110:5`
  - `matrix.rs:530:5` (`load`)
  - `matrix.rs:1014:5` (`ready`)
  - `matrix.rs:1055:5` (`load_from`)
- **The three sacm-cli BrokenPipe guard mutants** (`main.rs:31`) are caught on Linux, by the LIB-CLI-001 Linux tests.

## Slice 0004

Slice 0004 (LIB-TOOL-005, -006, -009, -010) added code to `xtask/src/matrix.rs`. Fix round 1 replaced the fence toggle in `verdict_lines` with CommonMark fences (a new helper, `fence`) and checks the `verification` path at every status.

- **Authoritative run:** the Linux runs on PR #10 at the fix-round-1 head `1655cbf`. The local runs below are Windows pre-checks.
  - mutants.yml [42390257623](https://github.com/lasrod/libSACM/actions/runs/42390257623): 245 mutants, 240 caught, 0 missed, 0 timeout, 5 unviable.
  - ci.yml [42390257629](https://github.com/lasrod/libSACM/actions/runs/42390257629), job `mutants (PR diff)`: 49 mutants, 48 caught, 0 missed, 0 timeout, 1 unviable.
  - Verification 0004 round 2 confirmed that the unviable set below matches the Linux run exactly.
- **Local pre-check, PR diff** (cargo-mutants 27.1.0, `--in-diff` against `main`, Windows): 49 mutants, **48 caught, 0 missed, 0 timed out, 1 unviable** (the `verdict_lines` mutant below).
- **Local pre-check, unviable set** (`--workspace --re "Default::default"`, Windows): 12 mutants, 7 caught, 5 unviable, exactly the five below. The 7 caught include the four `fence` replacements.
- **The 5 unviable mutants** at the fix head (positions from `--list`). The new one is `verdict_lines`. The other four are the earlier ones; `ready` and `load_from` have moved. All are structural: the replacement needs a `Default` that `Outcome`, `Matrix`, `VerdictLine` or `&Row` does not implement.
  - `fetch.rs:110:5` (`fetch_into`, unchanged)
  - `matrix.rs:530:5` (`load`, unchanged)
  - `matrix.rs:794:5` (`verdict_lines`, new in slice 0004; `775:5` at `e216a82`: `replace verdict_lines -> Vec<VerdictLine<'_>> with vec![Default::default()]`)
  - `matrix.rs:1168:5` (`ready`; `1014:5` in slice 0003, `1144:5` at `e216a82`)
  - `matrix.rs:1209:5` (`load_from`; `1055:5` in slice 0003, `1185:5` at `e216a82`)
- **`fence` is viable throughout:** its `Some((Default::default(), …))` replacements compile, because `char` implements `Default`, and they are caught.

## Slice 0005

Slice 0005 (LIB-TOOL-013, -014) changed `xtask/src/lib.rs`, `xtask/src/main.rs` and `xtask/src/matrix.rs` (`load`, `load_from`, `cmd_check`), and added the test target `xtask/tests/check_reporting.rs` (renamed to `reporting.rs`, below).

- **The timeout:** [mutants.yml run 42420891449](https://github.com/lasrod/libSACM/actions/runs/42420891449), shard 1, reported one TIMEOUT: `xtask/src/matrix.rs:449:5: replace is_identifier_char -> bool with true`. The code it mutates is unchanged since slice 0003, and it is LIB-TOOL-001's.
  - **Why it hangs:** under the mutant, every character is an identifier character, so `identifier()` takes the whole rest of the file after each `fn ` that `defined_functions` finds. A scan of the real repository becomes quadratic and runs for minutes. Measured on its own, each of `check_reporting` and `gate_hardening` ran for over 120 s under the mutant.
  - **What kills it:** the `function_scan` and `matrix` targets kill it in under 1 s. `function_scan` runs the scanner under its 5 s deadline.
  - **Why those targets never ran:** cargo-mutants runs `cargo test` without `--no-fail-fast`. `cargo test` runs the integration targets one after another in name order and stops at the first one that fails. `check_reporting` sorts before `function_scan`, and some of its tests scan the real repository (`repo_root()`). So the run hung in `check_reporting` until the timeout, before any failing target was reached.
- **The ordering hazard:** any xtask test target that sorts before `function_scan` and scans the real repository turns this mutant into a timeout. The module doc of `xtask/tests/function_scan.rs` states the rule for the targets that existed then (`matrix`, `repo_checks`, `run`). Nothing enforces it for a new target.
- **The fix in slice 0005:** the test target is renamed from `xtask/tests/check_reporting.rs` to `xtask/tests/reporting.rs`, which sorts after `function_scan`. Linux runs at the fix head:
  - mutants.yml [42422372773](https://github.com/lasrod/libSACM/actions/runs/42422372773) at `6019823`: 258 mutants, 251 caught, 0 missed, 0 timeout, 7 unviable. `is_identifier_char -> true` is caught.
  - ci.yml [42422372810](https://github.com/lasrod/libSACM/actions/runs/42422372810), job `mutants (PR diff)`: 20 mutants, 16 caught, 0 missed, 0 timeout, 4 unviable.
- **Unviable mutants at `6019823` (7):**
  - five carried over from slice 0004, with their lines moved: `fetch.rs:110:5`, `matrix.rs:546:5` (`load`), `829:5` (`verdict_lines`), `1203:5` (`ready`) and `1246:5` (`load_from`);
  - two new, `matrix.rs:1305:48`, `+` replaced with `-` and with `*` in `cmd_check_from`. They are structural: the `+` there joins a `String` and a `&str` (`e + &named`), and `String` implements neither `Sub` nor `Mul`.
- **Fix round 1 head `414263f`:** tests only; the mutated code is unchanged.
  - mutants.yml [42423497528](https://github.com/lasrod/libSACM/actions/runs/42423497528): 258 mutants, 251 caught, 0 missed, 0 timeout, 7 unviable. The unviable mutants are the same 7, at the same lines.
  - ci.yml [42423497546](https://github.com/lasrod/libSACM/actions/runs/42423497546), job `mutants (PR diff)`: 20 mutants, 16 caught, 0 missed, 0 timeout, 4 unviable.
- **The durable fix:** LIB-QA-012. A target's name must not decide whether this mutant is caught or times out.

## Slice 0006

Slice 0006 (LIB-TOOL-007, -011) rewrote the traced-test scanner in `xtask/src/matrix.rs`: the Rust and Python lexers, test and attribute recognition, and `check_scanned_tests`. Linux runs at `bf2ae2d`:
- mutants.yml [42431457415](https://github.com/lasrod/libSACM/actions/runs/42431457415): 421 mutants, 410 caught, 0 missed, 0 timeout, 11 unviable.
- ci.yml [42431457421](https://github.com/lasrod/libSACM/actions/runs/42431457421), job `mutants (PR diff)`: 202 mutants, 198 caught, 0 missed, 0 timeout, 4 unviable.
- **Unviable mutants at `bf2ae2d` (11):**
  - seven carried over from slice 0005, at their new lines: `fetch.rs:110:5`, `matrix.rs:1000:5` (`load`), `1319:5` (`verdict_lines`), `1725:5` (`ready`), `1768:5` (`load_from`), and `1827:48` twice (`cmd_check_from`);
  - four new, all structural: `matrix.rs:513:5` (`test_functions`), `720:5` (`rust_tests`) and `844:5` (`python_tests`), each `vec![Default::default()]`, and `1128:9` (`TestFile::scan`), `Default::default()`. The scanner's `Test` and `TestFile` types do not implement `Default`.
- **Fix round 1 head `48cad81` (D9, D10):**
  - mutants.yml [42436264675](https://github.com/lasrod/libSACM/actions/runs/42436264675): 433 mutants, 422 caught, 0 missed, 0 timeout, 11 unviable. These are the same 11 unviable mutants, at their new lines: `fetch.rs:110:5`, and in `matrix.rs` `557:5`, `764:5`, `888:5`, `1044:5`, `1181:9`, `1372:5`, `1788:5`, `1831:5`, and `1890:48` twice.
  - ci.yml [42436264672](https://github.com/lasrod/libSACM/actions/runs/42436264672), job `mutants (PR diff)`: 217 mutants, 213 caught, 0 missed, 0 timeout, 4 unviable (`557:5`, `764:5`, `888:5`, `1181:9`).
- **Copilot-fix head `2ff54e8` (D12):**
  - mutants.yml [42439740608](https://github.com/lasrod/libSACM/actions/runs/42439740608): 439 mutants, 428 caught, 0 missed, 0 timeout, 11 unviable. These are the same 11 unviable mutants, at their new lines: `fetch.rs:110:5`, and in `matrix.rs` `582:5`, `789:5`, `930:5`, `1107:5`, `1244:9`, `1435:5`, `1851:5`, `1894:5`, and `1953:48` twice.
  - ci.yml [42439740610](https://github.com/lasrod/libSACM/actions/runs/42439740610), job `mutants (PR diff)`: 231 mutants, 227 caught, 0 missed, 0 timeout, 4 unviable (`582:5`, `789:5`, `930:5`, `1244:9`).
- **Target order:** the new test target `xtask/tests/scanner.rs` sorts after `function_scan`. Its lexer-hang case is in `function_scan.rs` (`lib_tool_011_function_scan_finishes_on_every_comment_and_literal_form`), so a lexer mutant that stops advancing fails under that target's 5 s deadline before any real-repository scan runs. See slice 0005 and LIB-QA-012.

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

## Known items carried from earlier verification

- **X-flush (slice 0001, round 6):** closed. All the generated `run` mutants in `xtask/src/lib.rs` are caught on Linux in the final run. In the cargo-mutants log, `lib.rs:87:5` (`replace run -> Result<(), String> with Ok(())`) is killed by the `cli.rs` binary tests (`lib_tool_001_binary_*`). cargo stops at the first failing test binary, so the `run.rs` binary, which holds `lib_tool_001_run_turns_a_flush_failure_after_a_successful_command_into_an_error`, does not run for that mutant. That flush test still pins the behaviour, and it runs in the normal suite.
- **M5 (flush only on success):** not a cargo-mutants operator, so it is outside the generated set. The round-6 addendum gives the reason. It needs no exclusion.
- **R23b (`looks_traced` with an empty area):** the generated `looks_traced` mutants are all caught in both the baseline and the final run. No survivor.
- **`repo_root -> Default::default()`:** killed under LIB-TOOL-001 (see the table above).
- **The three sacm-cli BrokenPipe guard mutants:** caught on Linux by the LIB-CLI-001 Linux tests.
