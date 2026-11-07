# Verification 0001, round 4: m0-tooling-verification (live hook probe)

- Date: 2026-11-07
- Verdict: **PASS** for LIB-TOOL-003. LIB-TOOL-001 was out of scope this round and stays `implemented`.
- Row verdict: LIB-TOOL-003: PASS
- Row verdict: LIB-TOOL-001: OPEN
- Verifier: `sacm-verifier` (independent; read-only)
- Inspected: HEAD `756dd91` (pushed, working tree clean); unchanged since round 3
- Previous records: [round 1](0001-m0-tooling-verification.md), [round 2](0001-m0-tooling-verification-round2.md), [round 3](0001-m0-tooling-verification-round3.md)

## Rows verified

| Row | Title | Evidence |
|---|---|---|
| LIB-TOOL-003 | Agent write scopes are enforced by a hook | The 39 cited hook tests, plus the live probe below, which meets all five round-3 conditions. |

## Live probe evidence (recorded verbatim)

The probe was run by the lead in a fresh Claude Code session on 2026-11-07. That session started after the last commit changing `.claude/settings.json`: `b0122b3`, 2026-11-07 14:27:33 +0900. The file was not edited before or during the probe.

**Correction to the round-3 record:** it says `settings.json` was "unchanged in `b0122b3`". In fact `b0122b3` is the last commit that changed it. The intended meaning was "unchanged by the fix-round-2 diff". Condition 1 concerns the content since 14:27:33, so the correction does not affect it.

1. **A scoped role writes outside its scope.** `sacm-test-engineer`, spawned as a background project subagent, called Write on `C:\development\libSACM\crates\sacm\src\probe_scope.rs`, a file that did not exist. The tool returned:

   ```
   PreToolUse:Write hook error: sacm-test-engineer may not write crates/sacm/src/probe_scope.rs. Its write scope is ['crates/*/tests/**', 'crates/*/benches/**', 'fuzz/**', 'xtask/tests/**', '.claude/hooks/test_*.py']. Hand this change back to the lead with the reason it is needed.
   ```

2. **A read-only role.** `matrix-steward`, spawned as a background project subagent, reported its tools as Read, Grep, Glob, Bash and SubagentHandback. Its attempted Write on `C:\development\libSACM\probe-readonly-role.txt` returned:

   ```
   Error: No such tool available: Write. Write is not among the tools available here. Continue with the tools you have.
   ```

3. **Afterwards.** Neither file exists, and `git status --short` is empty.

## Verifier's assessment of the round-3 conditions

| # | Condition | Result |
|---|---|---|
| 1 | Session started after the current `settings.json`; file not edited | Met, on the lead's record. The file content equals `b0122b3` (clean tree). |
| 2 | A scoped role, spawned as a project subagent, writes to an out-of-scope path that does not exist | Met |
| 3 | Refused by the hook, with the guard's own reason text | Met. The text comes only from the guard's scope-deny branch, which is reached only when `agent_type` is `sacm-test-engineer`, so this also proves `agent_type` reaches the hook. The verifier reproduced the identical reason byte for byte by piping the same payload into the real `settings.json` command. |
| 4 | No file created; no new path in `git status` | Met (confirmed independently) |
| 5 | A read-only background subagent has no write tool | Met (corroborated: the verifier itself, as a read-only role, had no Write or Edit tool) |

None of the round-3 exclusions apply. The refusal did not come from the harness or the permission prompt, the reason text is present, and the probe ran in a session started after the current command.

## Gates run (by the verifier)

| Gate | Result |
|---|---|
| `cargo fmt --all -- --check` | pass |
| clippy `-D warnings` | pass |
| `cargo test --workspace --all-features` and `cargo test -p xtask` | pass: 57 matrix, 8 fetch, 4 repo-check, 1 version, 2 CLI, doctests |
| rustdoc `-D warnings` | pass |
| `cargo xtask matrix check` | pass (140 rows) |
| `cargo deny check` | pass |
| hook tests | pass (39) |

## Remaining notes (not blocking)

- The hook's read-only deny branch was never exercised live, because the first control (no Write tool) stopped the call first. The unit tests and the verifier's run of the real command cover that branch.
- `NotebookEdit` was not probed live; the settings matcher that routes it to the hook is tested.
- The round-3 Low survivors (P15, P20) are tracked as LIB-TOOL-008.
