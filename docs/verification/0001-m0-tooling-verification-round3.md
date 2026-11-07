# Verification 0001, round 3: m0-tooling-verification (after fix round 2)

- Date: 2026-11-07
- Verdict: **FAIL**. LIB-TOOL-001: FAIL (one Medium finding). LIB-TOOL-003: open, pending only the fresh-session live probe.
- Verifier: `sacm-verifier` (independent; read-only)
- Inspected: the uncommitted fix-round-2 diff against `b0122b3`. That diff is tests, matrix and the rendered page only; there were no production code changes.
- CI on `b0122b3`: [42342817205](https://github.com/lasrod/libSACM/actions/runs/42342817205), 10 of 10 jobs succeeded.
- Previous records: [round 1](0001-m0-tooling-verification.md), [round 2](0001-m0-tooling-verification-round2.md)

## Rows verified

None this round.

## Rows left open

| Row | Status kept | Reason |
|---|---|---|
| LIB-TOOL-001 | implemented | **Medium.** No test runs the `xtask` binary. If `main.rs` mapped `Err` to `ExitCode::SUCCESS` (mutation R28, run), `cargo xtask matrix check` would exit 0 over any violation, CI would pass, and all 72 xtask tests would stay green. Required: a `lib_tool_001_` test running `CARGO_BIN_EXE_xtask` on a failing command, asserting a non-zero exit and an `xtask:` stderr message; and `xtask/src/main.rs` added to the row's `code`. |
| LIB-TOOL-003 | implemented | Every criterion except the live probe passes. **The fresh-session live probe is the only remaining requirement.** |

## What the live probe must show (verifier's conditions)

1. It runs in a Claude Code session started after `.claude/settings.json` reached its current content (last changed 2026-11-07; unchanged in `b0122b3`). The file is not edited before or during the probe.
2. `sacm-test-engineer` (or `sacm-implementer`), spawned as a project subagent, calls Write on an out-of-scope path that does not exist, e.g. `crates/sacm/src/probe_scope.rs`.
3. The refusal comes from the PreToolUse hook and carries the guard's own reason text: `sacm-test-engineer may not write crates/sacm/src/probe_scope.rs. Its write scope is [...]. Hand this change back to the lead with the reason it is needed.`
4. Afterwards the file does not exist, and `git status` shows no new path.
5. Separately, a read-only role run as a background subagent is recorded as having no write tool.

These do not count: a refusal by the harness or by the permission prompt rather than the hook; a refusal without the guard's reason text; a probe in a session started before the current command. If the probe passes and is recorded, LIB-TOOL-003 may be marked verified without another code round.

## Mutation results (run by the verifier in a scratch copy)

- **Killed:** every round-2 survivor, including the scan roots, the `.py` scan, `cmd_check` always `Ok`, the id, name and notes clauses, and both fail-closed catch-alls narrowed to `RuntimeError`. Also killed: a further 40+ Rust and Python mutations covering scan behaviour, path separators, CRLF, the status table, scope parsing, path resolution, the write-tool set, the settings matcher and the hook command.
- **Survived:**
  - R28: `Err` mapped to SUCCESS in `main.rs`. **Medium.**
  - R20: `load_from` error swallowed. Low, practically equivalent.
  - R21: empty id-area segment. Low.
  - R23: `looks_traced` digit check. Low, fails safe.
  - P20: the interpreter probe replaced by `py=python3`. Low; fails open on a machine where only `python` works. The current command handles that case correctly, checked by hand.
  - P15: `notebook_path` ignored. Low, fails safe.

## Gates run (by the verifier)

fmt, clippy (`-D warnings`), all tests (57 matrix, 8 fetch, 4 repo-check, 1 version, 2 cli, doctests), rustdoc (`-D warnings`), `matrix check` (139 rows), `cargo deny`, and the hook tests (39, none skipped under Git Bash): **all pass**.
