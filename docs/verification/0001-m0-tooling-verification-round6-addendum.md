# Verification 0001, round 6 addendum: LIB-TOOL-001 flush contract

- Date: 2026-11-07
- Verdict: **PASS**. LIB-TOOL-001 stays verified, and the round-6 Info finding "X-flush" is closed.
- Verifier: `sacm-verifier` (independent; read-only)
- Trigger: a Copilot review comment on PR #5 at `xtask/src/lib.rs` line 90. It said nothing tests flush failures or which error wins when both fail. This is the same gap as the round-6 X-flush survivor.
- Inspected: uncommitted changes on top of `13c382c`. `xtask/tests/run.rs` gained 161 lines and lost none; the matrix gained three citations. Nothing under `xtask/src/` changed.
- Record being supplemented: [round 6](0001-m0-tooling-verification-round6.md)

## Tests added (test engineer)

| Test | What it checks |
|---|---|
| `lib_tool_001_run_turns_a_flush_failure_after_a_successful_command_into_an_error` | Uses a writer that accepts every write and fails every flush. For `help`, `matrix check`, `render`, `next` and `status`: the command writes its output, then `run` returns `Err` starting with `write output: ` that carries the flush error. No panic. |
| `lib_tool_001_run_keeps_the_command_error_when_flushing_also_fails` | With the same writer, three failing commands each return their own error. A failing `matrix check` returns exactly `functionality matrix: 1 problem(s)`, `next -n notanumber` returns exactly `invalid row count: notanumber`, and an unrecognized argument returns its own error. |
| `lib_tool_001_run_flushes_out_after_the_command_writes` | On success there is at least one flush, and no non-empty write comes after the last flush. |

## Mutation results

The mutations were applied to the real `xtask/src/lib.rs` in a fresh scratch copy (mtimes reset, fresh target directory).

| Mutation | Killed by |
|---|---|
| M1: no flush | `…_turns_a_flush_failure_…`, `…_flushes_out_after_…` |
| M2: flush result ignored (round-6 X-flush) | `…_turns_a_flush_failure_…` |
| M3: flush error takes precedence over the command error | `…_keeps_the_command_error_…` |
| M4: flush before the command | `…_flushes_out_after_…` |
| M5 (supplementary): flush only when the command succeeds | survives. Info: outside the contract and not visible to users, since std flushes stdout when `main` returns. Moved to LIB-QA-007 triage. |

## Gates (verifier)

All pass: fmt, clippy `-D warnings`, `cargo test --workspace --all-features`, `cargo test -p xtask` (`run.rs` 20/20), `cargo xtask matrix check` and rustdoc `-D warnings`.
