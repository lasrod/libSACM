# Verification 0003: LIB-QA-007 pre-verification prescription

- Date: 2026-11-07
- Kind: **pre-verification prescription**, not a verdict. Written by `sacm-verifier` before any work started, as the complete statement of what LIB-QA-007 must show to pass.
- Inputs: cargo-mutants v27.1.0 source and book (`sourcefrog/cargo-mutants@v27.1.0`) and GitHub's `workflow_dispatch` documentation.
- Measured on Windows, with all output under `target/`:
  - `cargo mutants --list` gives **5 mutants** with the default package selection (sacm-cli only).
  - `--workspace` gives **222**: sacm-cli 5; xtask `matrix.rs` 192, `lib.rs` 13, `fetch.rs` 11, `main.rs` 1; sacm 0, since it has only consts.
  - On Windows, the 3 sacm-cli BrokenPipe guard mutants show as missed, because only Linux tests kill them.
  - The sample `xtask/src/lib.rs` run had one survivor: `repo_root -> Default::default()`.

## Maintainer decisions (2026-11-07)

| Point | Decision |
|---|---|
| D2: where the PR job lives | A `mutants` job in `ci.yml`, whose **only condition is exactly `github.event_name == 'pull_request'`**, pinned by the regression test. |
| D10: which PRs get the full run | `mutants.yml` also runs on PRs that change `.cargo/mutants.toml`, `.github/workflows/mutants.yml`, `crates/**`, `xtask/**`, `Cargo.toml` or `Cargo.lock`, so every Rust PR gets a full sharded run. |
| D14: the authoritative Linux report | Each slice **pushes its branch as a draft PR before verification**, so the verifier reviews the Linux mutants report at the commit under verification. The PR is marked ready only after PASS and the Copilot fixes. |

## Design requirements

| Id | Requirement |
|---|---|
| D1 | `--workspace` on every cargo-mutants command. Without it, the virtual workspace's `default-members` drop xtask's 217 mutants silently. |
| D2 | The PR job's only `if` is exactly `github.event_name == 'pull_request'` (the `${{ }}` form is also allowed). |
| D3 | Checkout `with:` is exactly `fetch-depth: 0`, with no other input. A step with `env: BASE_REF: ${{ github.base_ref }}` runs exactly `git diff "origin/${BASE_REF}...HEAD" > mutants.diff`: a redirect, never a pipe (`bash -e` has no pipefail, and an empty diff passes); three dots. The diff step comes before the mutants step, in the same job. |
| D4 | The step passes only if cargo-mutants exits 0, with no wrapper. Missed (2), timeout (3), baseline failure (4), diff mismatch (5), invalid diff (6), usage error (1) and internal error (70) all fail it. Unviable mutants do not fail it, but the verifier reviews `unviable.txt`. |
| D5 | `cap_lints = true` in `.cargo/mutants.toml`. Without it, `-D warnings` from any source turns mutants unviable, and exit stays 0. Every mutants job also sets `timeout-minutes`. |
| D6 | `all_features = true`. |
| D7 | The test tool is `cargo test`, the default; nextest skips doctests. Neither `test_tool` nor `--test-tool` appears anywhere. |
| D8 | `test_workspace = false`, stated explicitly in the config with its reason: a mutant must be killed by its own package's tests. |
| D9 | The PR job uploads `mutants.out` with `if: always()`; `if-no-files-found` is `ignore` or `warn`. |
| D10 | `mutants.yml` triggers: a weekly schedule, `workflow_dispatch`, and `pull_request` on the D10 paths. `strategy.fail-fast: false`. `matrix.shard` is exactly 0..N−1 and matches the `--shard ${{ matrix.shard }}/N` denominator, with no include or exclude. No `--baseline=skip`. Upload `mutants.out` with `if: always()` and `if-no-files-found: error`, as an artifact whose name contains `matrix.shard`. `timeout-minutes` is set, and top-level `permissions: contents: read`. |
| D11 | The only install command is exactly `cargo install cargo-mutants --locked --version 27.1.0`. It appears identically in `ci.yml`, `mutants.yml` and `CONTRIBUTING.md`. |
| D12 | `.gitignore` contains `/mutants.out*`. |
| D13 | Exclusions live only in `.cargo/mutants.toml`, never as `mutants::skip` in source: `cfg_attr(<anything>, mutants::skip)` is honoured without evaluating the condition. `examine_globs` and `examine_re` are empty. Equivalent or accepted survivors are excluded by an `exclude_re` matching exactly that mutant. |
| D14 | The verifier reviews the Linux report from the draft PR's CI at the commit under verification. |
| D15 | The verifier's own local runs use `--output target/<dir>` and never `--in-place`. |
| D16 | Runtime is measured and recorded. The estimate is about 20–30 s per xtask mutant on a runner, so 4 shards take about 20–30 min each. Size `timeout-minutes` from the measurement. A job-level `CARGO_INCREMENTAL: "1"` is acceptable. |
| D17 | Linux only. Code compiled out on Linux by `cfg` is accepted under class A (P8). |

## Acceptance criteria (verifier wording, applied to the row)

The row's requirement and acceptance criteria C1–C7 in `docs/matrix/functionality-matrix.toml` use the verifier's proposed wording, with the D2 and D10 decisions applied.

## Required checks and mutations (P1–P10)

### Shared rules

- Each check is a function over the configuration text that returns `[code] text` problems.
- It is applied three ways: to the real file, which must report nothing; to each in-memory mutation, which must report its code; and to the controls, which must report nothing.
- A YAML mutation must still parse as one document, using exact-once `edit_line`-style helpers.
- Reuse `load_workflow`, `read_workflow`, `get`, `key_allowlist`, `step_guard_problems`, `merge_key_problems`, `unparsable_mutations` and `shape_mutations`. Generalising them is allowed; weakening any LIB-QA-002/003 assertion is not.
- Tests are named `lib_qa_007_*`.

### P1: the PR job in `ci.yml` (`check_mutants_pr_job`)

The job is found by a step whose parsed `run` equals exactly `cargo mutants --workspace --in-diff mutants.diff`. Every such job is checked. Each of the following must be reported:

1. **The mutants step is missing** (`[mutants-step-missing]`):
   - the job deleted;
   - the step commented out;
   - the job deleted with the command left in a comment;
   - the command inside another job's `run: |`.
2. **The command is not exact** (`[mutants-step-missing]`):
   - `--in-diff mutants.diff` dropped; `--workspace` dropped, or replaced by `-p sacm -p sacm-cli`;
   - `|| true`, `|| exit 0`, `; true` or `|| [ $? -eq 3 ]` appended;
   - `set +e` in a `run: |`;
   - a continuation line `|| true`;
   - a `>-` fold that adds `--check`;
   - the command only in a heredoc or an `echo`;
   - an alias of another step;
   - an added flag: `--check`, `--list`, `--list-files`, `--no-config`, `--config x.toml`, `-e`, `--exclude`, `-E`, `--exclude-re`, `-f`, `--file`, `-F`, `--re`, `--skip-calls`, `--shard 0/2`, `--baseline=skip`, `--iterate`, `--test-tool nextest` or `--cap-lints false`;
   - `--in-diff /dev/null`, or an `--in-diff` file the diff step does not write.
3. **The diff step** (`[mutants-diff]`, `[mutants-step-order]`):
   - missing;
   - `HEAD...HEAD`, `origin/main...origin/main`, or two dots;
   - `--stat`, `--name-only`, or a path filter;
   - `| tee`, `> … || true`, `: > mutants.diff` or `touch`;
   - `BASE_REF` removed, or set to `github.head_ref`;
   - placed after the mutants step, or in another job.
4. **The install step** (`[mutants-install]`):
   - `--version` dropped, `^27.1.0`, `>=27`, `27`, or a different version;
   - `--locked` dropped;
   - `--git`, `--path`, `--registry`, `--index`, or `cargo binstall`;
   - placed after the mutants step;
   - a second, unpinned install anywhere in the job.
5. **Unknown steps** (`[mutants-step-unknown]`): any step outside the allowlist. The allowlist is checkout, `dtolnay/rust-toolchain@`, `Swatinem/rust-cache@`, install, diff, mutants and upload, each matched by its `uses` prefix or exact `run`. This covers a step that writes `$GITHUB_ENV` or `$GITHUB_PATH`, or that rewrites the diff.
6. **Checkout** (`[mutants-checkout]`): `with` not exactly `{fetch-depth: 0}`, such as `ref`, `repository` or `path` added. Control: `fetch-depth: "0"` is accepted.
7. **Job condition and keys** (`[job-key]`, `[mutants-job-if]`):
   - `if: false`, `${{ false }}`, `== 'push'`, `== 'pull_request' && false`, or `… || false`;
   - `continue-on-error: true` or `${{ true }}`;
   - `needs` on a skipped job;
   - `strategy`, `container`, `services`, `defaults.run.shell`, `defaults.run.working-directory`;
   - a `<<` merge.
   
   Control: the `${{ github.event_name == 'pull_request' }}` form is accepted.
8. **Runner** (`[mutants-runner]`): Windows, macOS, `self-hosted` or `${{ matrix.os }}`.
9. **Step keys and guards** (`[step-key]`, `[step-guard]`), on any step: `if`, `continue-on-error`, `shell`, `working-directory`, or `<<`.
10. **Upload condition** (`[mutants-artifact]`): an `if` other than `always()` or `${{ always() }}`, including none at all.
11. **Environment** (`[mutants-env]`): an env key at any level outside {`CARGO_TERM_COLOR`, `CARGO_INCREMENTAL`, and `BASE_REF` on the diff step only}, for example `RUSTFLAGS`, `CARGO_ENCODED_RUSTFLAGS`, `CARGO_BUILD_RUSTFLAGS`, `CARGO_MUTANTS_OUTPUT` or `CARGO_MUTANTS_JOBS`.
12. **Upload step** (`[mutants-artifact]`): removed; `path` not `mutants.out`; or `continue-on-error` set.
13. **PR trigger** (`[pr-trigger]`): `on.pull_request` removed; `on: [push]`; or `paths`, `paths-ignore`, `branches`, `branches-ignore` or `types: [closed]` added.
14. **Workflow-level `defaults`** (`[workflow-defaults]`).
15. The parse and shape mutation sets.

Controls, all accepted: quoted keys and values; a flow-style `with`; added `name` and `id`; a job `timeout-minutes`; the `${{ }}` forms.

### P2: the full-run workflow `mutants.yml` (`check_mutants_full_run`)

Each of the following must be reported:

1. The file is missing (`[full-run-missing]`). Report it; never panic.
2. **Schedule** (`[full-run-schedule]`): removed; empty; no `cron`; or a cron that is not weekly. A weekly cron is one entry with 5 fields, `*` for day-of-month and month, and a single day-of-week from 0 to 6. Catch `0 3 * * *` and `0 0 30 2 *`.
3. **Manual dispatch removed** (`[full-run-dispatch]`).
4. **PR trigger** (`[full-run-pr-trigger]`): the `pull_request` trigger removed, or any of the six D10 paths dropped.
5. **The command is not exact** (`[full-run-step-missing]`): `--workspace` dropped; `--in-diff` added; any filter or flag from P1.2; `--baseline=skip`; `|| true` or a variant; `--shard` dropped.
6. **Shards** (`[full-run-shards]`): the denominator differs from the number of shards; an index missing or duplicated; `include`/`exclude` used; `fail-fast: true`, or `fail-fast` absent.
7. **Job keys**, as in P1.7, including `continue-on-error: ${{ matrix.shard == 3 }}` and a runner that isn't ubuntu.
8. **Steps**, as in P1.4, P1.5, P1.9 and P1.11.
9. **Artifact** (`[full-run-artifact]`): removed; no `always()`; `path` not `mutants.out`; `if-no-files-found` not `error`; or a name without `matrix.shard`.
10. Top-level `defaults`, a `<<` merge, and the parse and shape sets.
11. **Permissions** (`[full-run-permissions]`): `write-all`, or no top-level `permissions`.

### P3: version consistency (`[mutants-version-mismatch]`)

One constant install command in the test must appear exactly in `ci.yml`, `mutants.yml` and `CONTRIBUTING.md`. Must report: the version changed in one file only.

### P4: the configuration `.cargo/mutants.toml` (`check_mutants_config(toml, workspace_sources, triage_text, matrix_ids)`)

1. **Parse failure** (`[config-parse]`). If the file is absent, the test fails.
2. **Unreviewed keys** (`[unreviewed-key]`). The reviewed keys are `cap_lints`, `all_features`, `test_workspace`, `exclude_re`, `exclude_globs`, `skip_calls`, `skip_calls_defaults`, `timeout_multiplier`, `minimum_test_timeout`, `build_timeout_multiplier`, `sharding` and `error_values`. Must report each of: `additional_cargo_args`, `additional_cargo_test_args`, `output`, `profile`, `test_tool`, `test_package`, `copy_target`, `copy_vcs`, `gitignore`, `features`, `no_default_features`, `examine_globs` and `examine_re`.
3. **`cap_lints`** absent or `false` (`[config-cap-lints]`).
4. **`all_features`** absent or `false` (`[config-features]`).
5. **`test_workspace`** absent (`[config-test-scope]`).
6. **`exclude_globs`** (`[config-exclusion]`). Each entry must be the exact path of an existing `.rs` file under a member's `src/`, with no glob metacharacters, and must never cover all of a package's sources. Must report each of: `xtask/**`, `xtask/src/*.rs`, `*.rs`, `**`, `src`, `matrix.rs`, a file that doesn't exist, and `crates/sacm-cli/src/main.rs` (all of that package's sources).
7. **`exclude_re`** (`[config-exclusion]`). Each entry must be `^` + a regex-escaped existing source path + `:` + (`\d+:\d+` or literal digits) + `: ` + the fully escaped mutant description + `$`. Must report each of: `""`, `".*"`, `xtask`, `matrix`, `replace .* with true`, `^xtask/src/matrix\.rs` (no end anchor), `^xtask/src/matrix\.rs:.*$` (a whole file), and a path that doesn't exist.
8. **Cross-check with the triage document.**
   - `[exclusion-undocumented]`: an `exclude_re`, `exclude_globs` or `skip_calls` entry with no triage entry, or whose entry has an empty reason or a row id that is not in the matrix.
   - `[exclusion-stale]`: a triage entry marked "excluded" with no config entry.
   - The `skip_calls_defaults` choice is stated in the triage document.
9. **Source skips** (`[source-skip]`). Scan every `.rs` file under each member's `src/`, after removing whitespace and comments. Must report each of: `#[mutants::skip]`, `#![mutants::skip]`, `#[::mutants::skip]`, `#[mutants :: skip]`, `mutants/*c*/::skip`, `#[cfg_attr(test, mutants::skip)]`, `#[cfg_attr(any(), mutants::skip)]`, a multi-line `cfg_attr`, and a `mutants` dependency in any manifest.

### P5: `.gitignore`

Removing `/mutants.out*` is reported. `lib_tool_002` must still pass.

### P6: process documents (a text check; the verifier also reads them)

The tests check that:
- `CONTRIBUTING.md` contains the exact install command, `--workspace`, `--in-diff`, and a statement that the authoritative run is on Linux;
- `sacm-verifier.md` names `missed`, `timeout` and `unviable`, `--output target/`, and the ban on `--in-place`;
- `SKILL.md` says how the report for the commit under verification is obtained before step 7.

Deleting any of these phrases must be reported. The verifier also checks by reading:
- no existing verifier check is lost;
- the stale "cargo mutants --file once adopted" sentence is replaced;
- "in addition to hand-reasoned criterion mutations" survives.

**The maintainer approves the verifier-definition diff explicitly at the PR.**

### P7: tests first

Every new `lib_qa_007_*` test is run against the real files before the lead adds the configuration. Each must fail, with its expected code, and the slice brief records it.

### P8: baseline triage (`docs/quality/mutants-triage.md`)

1. **Header.** It records:
   - the run URLs and head SHA;
   - the cargo-mutants version (from `outcomes.json`);
   - the config in force;
   - the total mutant count, which must equal `cargo mutants --workspace --list`;
   - per-shard counts of caught, missed, timeout and unviable;
   - the measured times and the chosen `timeout-minutes`.
2. **Every missed or timed-out mutant**, by its exact name, gets exactly one disposition:
   - **Killed:** the test is named and cited under the row that owns the code, and appears in the final run's `caught.txt`.
   - **Equivalent:** the entry names the exact mutant and shows that for every input reachable through the public API or a binary, including I/O and error paths, the observable behaviour is identical (return values, error text, bytes written, exit status, files, panics). It cites the invariant that makes the distinguishing case unreachable.
     - Not valid as reasons: "unlikely", "hard to test", "fails safe", "low severity", "covered manually", "covered on another OS", "needs the network", "Info only".
     - The verifier re-derives each claim. If it can construct a distinguishing input, the mutant must be killed.
   - **Accepted:** allowed only in three classes:
     - **A:** code compiled out on Linux by `cfg`;
     - **B:** cannot be exercised hermetically on a Linux runner. The entry says why each of these does not apply: injected writers or downloaders, temporary trees, `file://` URLs with real `curl`, a fake executable on `PATH`, `/dev/full`, socket pairs. It also names a follow-up row;
     - **C:** a timeout caused by a real hang, where the hang is itself the detection.
     
     A survivor that changes behaviour asserted by a criterion of a `verified` row is **never** accepted. It must be killed under that row.
   - **Equivalent and accepted mutants** are excluded per P4.7. As evidence, `--list` compared with `--list --no-config` differs by exactly the triaged excluded set.
3. **Known items, each with its own line:**
   - **X-flush:** closed by `lib_tool_001_run_turns_a_flush_failure_after_a_successful_command_into_an_error`. Confirm the generated `run` mutants are caught on Linux.
   - **M5:** not a cargo-mutants operator. Record it as outside the generated set, with no exclusion.
   - **R23b:** maps to the `looks_traced` mutants at `matrix.rs:493–496`. Any survivor is killed, for example by asserting that `lib__001_x` is not traced.
   - **`repo_root -> Default::default()`:** killed under LIB-TOOL-001, for example by asserting that `xtask::repo_root().join("xtask/Cargo.toml")` is a file.
   - **The three sacm-cli guard mutants:** caught on Linux. A Linux miss is a LIB-CLI-001 finding.
4. **Final state:** a full run at the slice's final commit has every shard exit 0, and is linked from the document.

### P9: killing tests

Each killing test is named for, and cited under, the row that owns the code. It asserts observable behaviour, and appears in the final `caught.txt`. Verified rows stay verified, and their new tests are listed in the 0003 record.

### P10: gates and regression

- The full gate passes.
- No LIB-QA-001/002/003 or LIB-TOOL test is weakened or deleted.
- LIB-QA-007 cites as code: `.github/workflows/ci.yml`, `.github/workflows/mutants.yml`, `.cargo/mutants.toml` and `.gitignore`. It also cites every `lib_qa_007_*` test.
- The PR's CI shows the `mutants` job and the `mutants.yml` shards green at the final commit, with the run URLs recorded.

## Not held to

- Branch protection, and whether GitHub fires or auto-disables the schedule.
- What cargo-mutants' operators cannot generate (macro bodies, consts, `cfg`, and semantic mutations such as M5). These are still reasoned about by hand.
- The PR job not seeing test-only changes, beyond the D10 path filter.
- `sacm` having 0 mutants today.
- Exact runtimes.
- Results on macOS and Windows.
- LIB-QA-009.
- A mechanised one-mutant-per-`exclude_re` check (a follow-up row).
- A merged report across shards.
- `error_values`.
- Attributes whose path ends in `test`.

## Ownership (the verifier's conditions)

- **The lead** edits `ci.yml`, `mutants.yml`, `.cargo/mutants.toml`, `.gitignore`, `CONTRIBUTING.md`, `sacm-verifier.md`, `SKILL.md` and the matrix.
- **The test engineer** writes all `lib_qa_007_*` tests and every killing test. It writes no configuration, including through Bash.
- **The triage document** is written by the implementer or the lead, and the author is recorded. The verifier re-derives every claim in it.
- **The verifier-definition diff** is reviewed line by line, and the maintainer approves it at the PR.
- **New citations under rows that are already verified** are listed in the 0003 record.
