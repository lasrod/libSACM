# Verification 0002, round 2 addendum: Copilot findings and the YAML-parse design

- Date: 2026-11-07
- Verifier: `sacm-verifier` (independent; read-only)
- Supplements: [round 2](0002-m0-qa-and-cli-verification-round2.md). The verdict there is unchanged: LIB-QA-002 FAIL, LIB-QA-003 FAIL, LIB-CLI-001 PASS.
- Trigger: the Copilot review on PR #6 raised two inline comments, on `xtask/tests/repo_checks.rs` lines 163 and 210.

## Copilot findings: confirmed by mutation

Each mutation was run against the real `repo_checks` binary in a fresh copy.

| Mutation | Result |
|---|---|
| MSRV step replaced by a `run: \|` heredoc that only prints the required line | survived |
| Deny step replaced by a heredoc that only prints the `uses:` line | survived |
| `with: { command: "check bans" }` | survived |
| `with: { arguments: --exclude sacm-cli }` | survived |
| `with: { manifest-path: crates/sacm/Cargo.toml }` | survived |
| `with: { command-arguments: --hide-inclusion-graph }` | survived |

Both findings fall within the round-1 prescription (Q2-1, Q3-5). They are **the verifier's miss, not a new class**.

## Design for fix round 2: accepted

Parse `ci.yml` with a real YAML parser: a maintained dev-dependency of `xtask`, e.g. `yaml-rust2`. That closes the whole syntax class at once: comments, block scalars, flow collections, continuation lines and quoting. Conditions:

1. **Dependency checks.** `cargo deny check` still passes. The dependency's licence is checked, because `private.ignore` covers only xtask itself. The commit says why this crate was chosen. Building it on 1.85 is not required (ADR 0007).
2. **Parse failure fails the test.** A parse failure, or more than one YAML document, fails the test. It never skips the check or reports zero problems.
3. **YAML 1.2.** `on`, `yes` and `no` must stay strings.
4. **Shape errors are reported, never skipped.** For example: `jobs` is not a mapping, `steps` is not a sequence, a step is not a mapping, `run` is not a string, `with` is not a mapping.
5. **Merge keys and aliases.** A `<<` key is reported by the key allowlists. Aliases are resolved, and the resolved values are checked like any others.

## Checks that must hold (YAML data view)

"The MSRV job" is every job whose steps contain the MSRV step, and there must be at least one; "the deny job" is defined the same way for the deny step.

| Item | Check |
|---|---|
| Y2-1 | Some step's `run` value is exactly `cargo +{msrv} test -p sacm -p sacm-cli`, compared as a string after parsing. This replaces R2-1. |
| Y2-2 | The MSRV job's keys ⊆ {`name`, `runs-on`, `steps`, `env`, `permissions`}. |
| Y2-3 | The MSRV step's keys ⊆ {`run`, `name`, `id`, `env`}. No step in the job has `if`, `continue-on-error` or `shell`. There is no workflow-level `defaults`. |
| Y3-1 | Some step's `uses` value starts with `EmbarkStudios/cargo-deny-action@`. |
| Y3-2 | The deny job's keys follow Y2-2. The deny step's keys ⊆ {`uses`, `name`, `id`, `with`}. No step in the job has `if`, `continue-on-error` or `shell`. |
| Y3-3 | `with`, if present, is a mapping, in either block or flow style. `command`, if set, is exactly `check`. There is no `command-arguments`. `arguments` tokens ⊆ {`--all-features`, `--workspace`}. `manifest-path` is `Cargo.toml` or `./Cargo.toml`. |
| R3-3 to R3-7 | Unchanged (TOML side). |

## Mutations that must be killed

These are in addition to every mutation killed today. Each one is applied in memory to the real text, and the real files must pass first.

**MSRV (LIB-QA-002):**
- a heredoc that only prints the line (Copilot 1);
- the line placed inside another job's `run: |`, with the MSRV job deleted;
- a plain continuation line `--no-run`, or `|| true`;
- `run: >-` folding in `--no-run`;
- `needs:` on a job that gets skipped;
- a `<<: *anchor` merge key;
- a step-level `shell: "true {0}"`;
- a job-level or workflow-level `defaults.run.shell`.

**Deny (LIB-QA-003):**
- a heredoc that only prints the `uses:` line (Copilot 1);
- flow-style `command: "check bans"`, quoted or unquoted (Copilot 2);
- flow-style `arguments: --exclude …`;
- flow-style `manifest-path` pointing elsewhere;
- flow-style `command-arguments`;
- a continuation line on `command`;
- `needs:` on a job that gets skipped;
- a `<<` merge key;
- `with` given as a non-mapping;
- every must-kill in R3-3 to R3-7.

**Positive controls, which must still pass:**
- the real files;
- `with: { command: check }`;
- block-style `command: check`;
- `command: "check"`;
- `arguments: --all-features`;
- an inline `rust-version = { workspace = true }`;
- every deny.toml variant that is approved today;
- a trailing comment on the MSRV step.

## PASS condition

Every check above holds, and every must-kill here and in the round-2 record is shown killed in a scratch copy with the positive controls accepted. The verifier would then PASS LIB-QA-002 and LIB-QA-003.
