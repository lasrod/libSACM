# Verification 0002, round 4: m0-qa-and-cli-verification (LIB-QA-003)

- Date: 2026-11-07
- Verdict: **PASS**. LIB-QA-003: PASS.
- Row verdict: LIB-QA-003: PASS
- Verifier: `sacm-verifier` (independent; read-only)
- Inspected: `9404177` on `slice/0002-qa-003-final` (PR #8), diffed against `main` at `9f59c85`. It changes `xtask/tests/repo_checks.rs` (test engineer) and `deny.toml` (lead). The rest is the matrix and the slice brief. Nothing outside those scopes changed.
- CI: [run 42362800537](https://github.com/lasrod/libSACM/actions/runs/42362800537), 10 of 10 jobs passed. The cargo-deny job shows `advisories ok, bans ok, licenses ok, sources ok`.
- Contract: [round 3](0002-m0-qa-and-cli-verification-round3.md) findings F1 and F2, plus every must-kill and control from [round 1](0002-m0-qa-and-cli-verification.md), [round 2](0002-m0-qa-and-cli-verification-round2.md) and its [addendum](0002-m0-qa-and-cli-verification-round2-addendum.md).

## Rows verified

| Row | Title | Evidence |
|---|---|---|
| LIB-QA-003 | Dependency licences, advisories and sources are policed | The 17 `lib_qa_003_*` tests in `xtask/tests/repo_checks.rs`. F1 and F2 are closed, and every earlier must-kill and control holds against the real checks in a fresh scratch copy. |

## F1 (`log-level` injection): closed

`log-level` must be one of off/error/warn/info/debug/trace, and the input name is compared case-insensitively.

**Killed** by `lib_qa_003_ci_deny_job_runs_every_check`, each run against the real `ci.yml`:
- the prescribed forms: flow and block injection, `"warn --exclude-dev"`, `LOG-LEVEL:`, a `with: *x` alias, the non-strings `5` and `true`, and `""`;
- extra forms the verifier added: `Log-Level`, an escaped newline, a `|` block scalar, `TRACE`, and a `${{ }}` expression.

**Accepted** (controls):
- `log-level: warn` and `log-level: off`, each in flow and block form;
- `rust-version: "1.85.0"`;
- `arguments: --all-features`.

## F2 (`include-dev`): closed

`deny.toml` now has `include-dev = true`; this was the maintainer's choice. The test requires it to be exactly the boolean `true`. Removing the key, or setting it to `false`, `"true"` or `1`, is caught.

The verifier confirmed with cargo-deny 0.20.2 that dev-dependencies are now licence-checked:
- With Zlib and BSD-3-Clause removed from `allow`, the dev-only crates `foldhash` and `encoding_rs` are rejected.
- A GPL path dev-dependency on `sacm` fails the licence check.
- Without `include-dev`, both cases pass. This proves the setting is what checks dev-dependencies.

## Mutation summary

- **Sabotaged checks:** 15 sabotages of the checks themselves (S1–S15) were each killed by named tests. Examples: input names compared case-sensitively, the `log-level` arm removed, a missing `include-dev` accepted, `deny_input_problems` never called.
- **Earlier must-kills:** a representative re-run of the earlier `ci.yml` and `deny.toml` must-kills found every one still killed.
- **Regression:** after the last restore, the whole suite passes. No test was weakened.

## Notes (not findings)

- **Fixture construction.** Several fixture tests insert their own `with:` after the deny step line. If the real deny step ever gains a `with:`, those tests fail loudly with a duplicate-key error. That is a false rejection, never a silent pass.
- **Assumption.** Input-name case folding is ASCII only. Folding of non-ASCII look-alikes is a new class, not covered by this row.

## Follow-ups

- LIB-QA-009: pin `EmbarkStudios/cargo-deny-action` by commit SHA, and review its input names on every bump.

## Gates (verifier)

All pass: fmt, clippy `-D warnings`, `cargo test --workspace --all-features`, `cargo test -p xtask`, rustdoc `-D warnings`, `matrix check` (145 rows), `cargo deny check`, and the hook tests (39).
