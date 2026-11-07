# Verification 0001, round 2: m0-tooling-verification (after fix round 1)

- Date: 2026-11-07
- Verdict: **FAIL** (partial).
  - LIB-TOOL-002: **PASS**
  - LIB-QA-001: **PASS**
  - LIB-TOOL-001: FAIL
  - LIB-TOOL-003: FAIL
- Verifier: `sacm-verifier` (independent; read-only)
- Inspected: baseline `9d24b60`, plus `a74ca9c` (ADR 0006 amendment 1), plus the uncommitted fix-round-1 diff
- Previous record: [0001-m0-tooling-verification.md](0001-m0-tooling-verification.md)
- Slice brief: [docs/slices/0001-m0-tooling-verification.md](../slices/0001-m0-tooling-verification.md)

## Rows verified

| Row | Title | Evidence |
|---|---|---|
| LIB-TOOL-002 | Hash-pinned fetching of the normative SACM 2.3 sources | 8 tests in `xtask/tests/fetch.rs` and `lib_tool_002_reference_directory_is_git_ignored`. Pins match the fetched files; `git check-ignore` matches; nothing under `third_party/` has ever been committed. |
| LIB-QA-001 | CI runs every quality gate on three platforms | `lib_qa_001_ci_runs_every_gate`. GitHub run [42338058640](https://github.com/lasrod/libSACM/actions/runs/42338058640): push to `main` at `a74ca9c`, 10 of 10 jobs succeeded, and `ci.yml` is unchanged since. |

## Rows left open

| Row | Status kept | Reason |
|---|---|---|
| LIB-TOOL-001 | implemented | **High.** The filesystem half of the gate (`FsRepo::traced_functions`, `TEST_SCAN_ROOTS`, the `.py` scan, `cmd_check`) has no test. Three scratch-copy mutations made the real gate pass over a planted orphan while `cargo test` stayed green: dropping `.claude/hooks` from the scan roots, ignoring `.py` files, and making `cmd_check` always `Ok`. **Low.** Several id, name and whitespace sub-clauses are untested. |
| LIB-TOOL-003 | implemented | **Medium.** The fail-closed tests raise only `RuntimeError`. Narrowing either catch-all to `RuntimeError` leaves all 29 tests green, and through the real command a malformed `tool_input` then fails open. **Medium.** The live probe predates the new hook command; it must be re-run in a fresh session. |

## Gates run (by the verifier)

| Gate | Result |
|---|---|
| fmt, clippy (`-D warnings`), rustdoc (`-D warnings`) | pass |
| `cargo test --workspace --all-features`, `cargo test -p xtask` | pass: 45 matrix, 8 fetch, 4 repo-check, 3 xtask doctests |
| `cargo xtask matrix check` | pass (138 rows) |
| `cargo deny check` | pass |
| hook tests | pass (29) |
| Real hook command, piped by hand (good and broken configurations) | behaves as specified. With no Python on PATH it exits 127, so nothing is enforced; this limit is documented. |

## Findings carried to the maintainer

This is the second FAIL for LIB-TOOL-001 and LIB-TOOL-003, so the loop stopped, as the slice skill requires. The required fixes are narrow:
- LIB-TOOL-001: temp-directory tests for `FsRepo` and `cmd_check`, plus the sub-clause one-liners.
- LIB-TOOL-003: tests that raise `AttributeError`, `TypeError`, `KeyError`, `ValueError` and `OSError` from `decide` and from `load_scopes`, plus a malformed-payload test; then a fresh-session live probe.

Low findings to track, not blocking:
- The fetch rename-then-verify mutation survives.
- The test scanner reads line by line, so multi-line raw strings can look like tests.
- The CI regression test is a substring search.
- The implementer can write inline `#[cfg(test)]` tests in `src/` (now row LIB-TOOL-007).

## Verifier's verdict (excerpt)

> ### Matrix updates allowed
> - May mark verified: **LIB-TOOL-002**: all four criteria are tested; the pins match the real files; the directory is git-ignored and nothing has ever been committed. **LIB-QA-001**: `ci.yml` has every gate and the three-OS test job; the regression test exists; evidence is https://github.com/lasrod/libSACM/actions/runs/42338058640 (push to `main`, `a74ca9c`, 10 of 10 jobs succeeded); no dependencies.
> - Must stay `implemented`: **LIB-TOOL-001** (the High finding). Would change my mind: `FsRepo`/`cmd_check` temp-directory tests, so that the scan-root, `.py`-scan and always-`Ok` mutations each fail at least one test. **LIB-TOOL-003** (the two Mediums). Would change my mind: tests that fail when either catch-all is narrowed to `RuntimeError`, and a fresh-session live probe 1 under the current `settings.json`, recorded.
>
> The LIB-TOOL-003 rewording is sound, not moving the goalposts. It is recorded in an ADR the maintainer accepted, it now claims only what was observed, and the read-only branch it no longer proves live is covered by the real-scopes tests.
