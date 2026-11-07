# Verification 0001, round 6: m0-tooling-verification (after fix round 4)

- Date: 2026-11-07
- Verdict: **PASS**. LIB-TOOL-001: PASS.
- Verifier: `sacm-verifier` (independent; read-only)
- Contract: the verifier's own pre-verification prescription,
  [round6-prescription](0001-m0-tooling-verification-round6-prescription.md), items 0–11 and its PASS condition.
- Inspected: branch `slice/0001-m0-tooling-verification`, the fix-round-4 diff on top of `0ac00fc`. It is committed as `d3d0305` (process documents only, no code) and `dd801a1` (code and tests).
- Condition: CI must run the `cfg(unix)` variant of the non-Unicode test. **Met:** [CI run 42352469780](https://github.com/lasrod/libSACM/actions/runs/42352469780) on PR #5 at `dd801a1` passed all 10 jobs, and `lib_tool_001_binary_refuses_a_non_unicode_argument ... ok` ran on ubuntu-latest, macos-latest and windows-latest.
- Previous records: rounds [1](0001-m0-tooling-verification.md), [2](0001-m0-tooling-verification-round2.md), [3](0001-m0-tooling-verification-round3.md), [4](0001-m0-tooling-verification-round4.md), [5](0001-m0-tooling-verification-round5.md)

## Rows verified

| Row | Title | Evidence |
|---|---|---|
| LIB-TOOL-001 | Functionality-matrix gate | All 80 cited tests:<br>• `xtask/tests/matrix.rs` (59)<br>• `xtask/tests/run.rs` (17)<br>• `xtask/tests/cli.rs` (4) |

## Item-by-item (verifier)

| Item | Result |
|---|---|
| 0 Structure | Met. `run(args, root, out)` holds all dispatch, flushes, never prints `xtask:` and never exits. `main` decodes with `args_os`, refuses non-Unicode arguments (lossy text is used for display only, because `OsStr::display` needs Rust 1.87 and the MSRV is 1.85), calls `run` and maps the result. There is one `USAGE`. The docs are fixed, and `run` has `# Errors` and a doctest. |
| 1 `matrix check` | Met. The page is pre-set to CRLF, so a stray render would change its bytes. |
| 2 `matrix render` | Met (stale, empty and unwritable cases) |
| 3 `matrix next` | Met: exact header; B, D, X order; `-n`/`--limit`; no-ready case; all error cases with empty output |
| 4 `matrix status` | Met |
| 5 `fetch-spec` | Met, offline (G1 and G2 planted, byte-identical afterwards) |
| 6 help, unrecognized and near-miss arguments | Met |
| 7 Write failures become errors | Met (no panic under `catch_unwind`) |
| 8 Non-Unicode argument | Met. Run on Windows locally; the `cfg(unix)` variant ran green in CI on Ubuntu and macOS. |
| 9 Existing binary tests unchanged | Met (additions only) |
| 10 Existing tests adapted mechanically | Met. 0 assertions removed. `cmd_fetch` moved from `println!` to a writer, and all 9 LIB-TOOL-002 tests stay green, so **LIB-TOOL-002 is not reopened**. |
| 11 Criterion 7 overclaim | Resolved by option (b). The criterion is narrowed, and LIB-TOOL-011 is open. |

## Mutation results

Run by the verifier in a fresh scratch copy: mtimes reset, a fresh target directory, and network blocked through dead proxies. Baselines were green before and after.

**Every prescribed mutation was killed:**
- R28e/f/g
- R32–R62, including all sub-variants
- the binary-level R28 family

**One unprescribed mutation survived, X-flush** (`run` ignores a flush error). It is Info only, outside items 0–11, and nearly equivalent in practice: every output line ends with a newline. It goes to LIB-QA-007 triage.

## Gates run (by the verifier)

| Gate | Result |
|---|---|
| fmt, clippy (`-D warnings`), rustdoc (`-D warnings`) | pass |
| `cargo test --workspace --all-features` and `cargo test -p xtask` | pass: cli 4, fetch 8, matrix 59, repo_checks 4, run 17, doctests 4 |
| `cargo xtask matrix check` | pass (143 rows) |
| `cargo deny check` | pass |
| hook tests | pass (39) |
| `cargo +1.85 test -p sacm -p sacm-cli` and `cargo +1.85 test -p xtask` | pass |
| clippy for `x86_64-linux-android`, which type-checks the `cfg(unix)` branch | pass |

## Findings

None blocking. One process point: the governance edits in `d3d0305` are Markdown only and touch no code.
