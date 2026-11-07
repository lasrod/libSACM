# Verification 0002, round 2: m0-qa-and-cli-verification (after fix round 1)

- Date: 2026-11-07
- Verdict: **FAIL** (partial).
  - LIB-CLI-001: **PASS**
  - LIB-QA-002: FAIL
  - LIB-QA-003: FAIL
- Row verdict: LIB-CLI-001: PASS
- Row verdict: LIB-QA-002: FAIL
- Row verdict: LIB-QA-003: FAIL
- Verifier: `sacm-verifier` (independent; read-only)
- Inspected: `b944c24` (PR #6), the fix round 1 diff since `b86187b`
- CI: [run 42356192597](https://github.com/lasrod/libSACM/actions/runs/42356192597), 10 of 10 jobs passed. Both Linux CLI tests show `ok` in the ubuntu test job and in the MSRV job (rustc 1.85.1). The cargo-deny action runs cargo-deny 0.20.2.
- Round 1 record and prescription: [0002-m0-qa-and-cli-verification.md](0002-m0-qa-and-cli-verification.md)

## Rows verified

| Row | Title | Evidence |
|---|---|---|
| LIB-CLI-001 | CLI skeleton with `sacm version` | `lib_cli_001_version_reports_tool_and_sacm_revision`, `lib_cli_001_unknown_subcommand_fails_with_usage`, `lib_cli_001_closed_stdout_is_success` and `lib_cli_001_stdout_write_error_fails_with_message`. The last two are Linux-only and passed in CI. Conditions (a) to (g) are met. Windows behaviour with a closed pipe was checked once by hand; there is no recorded test for it. |

## Rows left open: complete remainder (fix round 2)

Every mutation prescribed in round 1 is now killed by the real check functions; the verifier ran them in a fresh copy. The verifier then found these bypasses within the same scope, and calls them its own misses. Its round-1 must-kill lists were narrower than the criteria, and for Q2-1 the whole-line method it specified could not see YAML structure. The PR #6 Copilot review reported two of them independently: the block-scalar and flow-style forms.

**LIB-QA-002**

| Item | Required | Must kill |
|---|---|---|
| R2-1 | Read the MSRV step's `run` value the way GitHub parses it: comments stripped, continuation lines folded, quotes resolved. The value must be exactly `cargo +{msrv} test -p sacm -p sacm-cli`. Either (a) parse with a YAML-parser dev-dependency that rejects duplicate keys, or (b) use a strict line reader: every active line is one `key: value` with a plain value, with no flow collections, block scalars, anchors, tags, `<<` merges or continuation lines. | a continuation line `--no-run` or `\|\| true`; everything already killed |
| R2-2 | The MSRV job's keys are a subset of {`name`, `runs-on`, `steps`, `env`, `permissions`}. | `needs:` on a job that gets skipped; job-level `defaults:`; `if:` and `continue-on-error:` |
| R2-3 | The MSRV step's keys are a subset of {`run`, `name`, `id`, `env`}. No step in the job has `if`, `continue-on-error` or `shell`. The workflow has no top-level `defaults:`. | step `shell: "true {0}"`; workflow-level `defaults` with that shell |

**LIB-QA-003**

| Item | Required | Must kill |
|---|---|---|
| R3-1 | Read the deny step's `with:` inputs the way R2-1 reads `run`. | the flow-style `with: { command: check bans }` and `with: { arguments: --exclude-dev }`; a continuation of `command` |
| R3-2 | The deny job's keys follow R2-2. The deny step's keys are a subset of {`uses`, `name`, `id`, `with`}. | `needs:` on a job that gets skipped |
| R3-3 | No crate is excluded from the graph. `graph.exclude` and `graph.targets` are absent or empty. `graph.exclude-dev` and `graph.exclude-unpublished` are absent or false. `graph.all-features = true` stays. None of the legacy top-level keys `targets`, `exclude`, `features`, `all-features`, `no-default-features` or `exclude-dev` is present. | each listed key, including top-level `exclude = ["clap"]` (cargo-deny 0.20.2 honours it with only a deprecation warning) |
| R3-4 | `sources.private` is absent or empty. | `sources.private = [...]` |
| R3-5 | `advisories.disable-yank-checking` is absent or false. | `disable-yank-checking = true` |
| R3-6 | `[licenses.private]` has only `ignore`; `ignore-sources` and `registries` are absent or empty. `licenses.include-dev` and `licenses.include-build` are not `false`. | `ignore-sources = [crates.io]`, which removes every crates.io crate from the licence check |
| R3-7 | For each reviewed section, the test keeps the key set listed by cargo-deny 0.20.2 and reports any other key. A key cargo-deny adds in future is then reviewed before it can narrow the checks. `[bans]` and `[output]` are not checked. | an unreviewed key (in-memory mutation) |

Every mutation killed today, and every positive control, must still hold.

**Not held to**, in addition to round 1: environment-variable sabotage in `env:`; workflow triggers, `paths-ignore` and branch protection (LIB-QA-001 and LIB-QA-008); multi-document YAML; duplicate keys that GitHub itself rejects; deprecated v1 keys and a removed `version` (verified harmless, because cargo-deny rejects them); `db-path`, `maximum-db-staleness`, `git-fetch-with-cli`, `required-git-spec` and the `unused-*` lint levels; `graph.features` and `no-default-features` while `all-features = true`; harmless action inputs.

## The test engineer's six stricter checks

None of them is wrong in a way that matters. Each false rejection fails loudly and names the offending line. Note that `arguments: --workspace` replaces the action's default `--all-features`. That is harmless only while R3-3 requires `graph.all-features = true`.

## Gates (verifier)

All pass on Windows: fmt, clippy `-D warnings`, `cargo test --workspace --all-features`, `cargo test -p xtask`, rustdoc `-D warnings`, `matrix check` (145 rows), `cargo deny check`, the hook tests, and `cargo +1.85 test -p sacm -p sacm-cli`.

## Stop

This is the second FAIL for LIB-QA-002 and LIB-QA-003, so the loop stops for the maintainer, as the slice skill requires.
