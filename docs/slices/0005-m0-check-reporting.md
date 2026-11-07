---
slice: 0005
name: m0-check-reporting
state: done
step: close
rows: [LIB-TOOL-013, LIB-TOOL-014]
mode: pr
---

# Slice 0005: matrix check reporting

## Scope

There are two follow-ups to LIB-TOOL-010 from verification 0004, round 1:
- **LIB-TOOL-013.** Under `XTASK_ROOT`, `matrix check` names the root it checked, so a
  check of another tree cannot pass for a check of the repository.
- **LIB-TOOL-014.** Errors from loading the matrix are one line each,
  `path:line:column: message`, like gate problems. Today they are multi-line TOML
  excerpts.

| Row | Title | Status at start |
|---|---|---|
| LIB-TOOL-013 | Matrix check names an overridden root | planned |
| LIB-TOOL-014 | Matrix load errors are one line each | planned |

## Out of scope

- LIB-TOOL-005 and LIB-TOOL-015. They wait for the maintainer to choose route A, B or C in
  P-005b of `docs/verification/0004-m0-gate-hardening-round2.md`.
- Naming the root in commands other than `matrix check`.

## Normative basis

None. These are tooling rows, so the acceptance criteria are the lead's (see the matrix).

## Design

xtask is repository tooling (`publish = false`), so the lead decides the design. The
architect is not involved.

- **D1:** `xtask::run(args, root, out)` keeps its signature and its output. A new entry
  point carries where the root came from. The lead fixed its shape before the tests were
  written:

  ```rust
  /// Where the repository root came from.
  #[derive(Clone, Copy, Debug, PartialEq, Eq)]
  pub enum RootSource {
      /// The repository xtask was built in (`repo_root`).
      Build,
      /// The `XTASK_ROOT` environment variable.
      Environment,
  }

  pub fn run_from(args: &[&str], root: &Path, source: RootSource, out: &mut dyn Write)
      -> Result<(), String>;
  ```

  - `run(args, root, out)` is `run_from(args, root, RootSource::Build, out)`.
  - The binary passes `Environment` when `XTASK_ROOT` is set, and `Build` otherwise.
  - Only `matrix check` treats the two sources differently.
- **D2:** a root is named because of where it came from, never because of its value.
  So no path comparison and no canonicalisation.
- **D3:** a load error is `docs/matrix/functionality-matrix.toml:<line>:<column>: <message>`:
  - built from `toml::de::Error::message` and `span`;
  - the column is counted in characters;
  - control characters are escaped with the same helper as gate problems.

  `matrix::load` itself returns this form, so every command shares it.
- **D4:** the root suffix is ` (root: <p>)`, with `<p>` escaped. It is added to the
  first line only.

- **D5:** an `XTASK_ROOT` that is set but empty is an error (`XTASK_ROOT is set but empty`),
  and no command runs. The test engineer raised this case.
  - Not specified: whether a non-Unicode argument or an empty `XTASK_ROOT` is reported
    first. No test covers it.

## Tests

All 33 tests are in `xtask/tests/reporting.rs`, and each matrix row cites its own.

| Tests | Row | Criterion |
|---|---|---|
| 19 `lib_tool_013_*` tests | LIB-TOOL-013 | Criteria 1 to 5: the named OK line and error header, the load-error suffix, escaping, D2 (an override equal to the build root, no canonicalisation), unchanged commands and `run`, the binary with and without `XTASK_ROOT`, an empty `XTASK_ROOT` for 13 argument forms |
| 14 `lib_tool_014_*` tests | LIB-TOOL-014 | Criteria 1 to 4: exact `path:line:column: message` for a syntax error, an unknown key, a missing field, a wrong type and a later row; character columns (28 against 37 bytes); escaped messages and read errors; the same line from every loading command; the binary's stderr |

Changed: `gate_hardening.rs::lib_tool_010_binary_prints_every_problem_on_stderr_and_fails`
sets `XTASK_ROOT`, so its expected first line now carries ` (root: <tree>)`. It is still
an exact comparison. It is a verified row's test, and it is red until LIB-TOOL-013 is
implemented.

## Log

- 2026-11-08 research: skipped, because these are tooling rows. The lead wrote the
  acceptance criteria, and both rows are now `analyzed`.
- 2026-11-08 tests: 33 tests in total, 29 at first and then 4 for D5. The new file
  fails to compile only on `RootSource` and `run_from`; the lead confirmed this.
  - Settled on the test engineer's questions: a read error shows the full path; the
    whole line is escaped; an empty `XTASK_ROOT` is an error (D5).
  - Not covered: an error without a span, and a tree whose name holds control
    characters (`#[cfg(unix)]`, so only on Linux CI).
    - **Corrected in verification round 1 (F1):** the claim that "toml 1.2 always gives
      a span" was false. toml's "recursion limit" error has no span: a dotted key of 80
      or more segments triggers it. The spanless case is tested in fix round 1.
  - Both rows are now `tests-failing`.
- 2026-11-08 implementation: `RootSource` and `run_from` in `lib.rs`, a private
  `matrix::cmd_check_from`, `load_error`, the empty-root check in `main.rs`, and README
  rule 11 (text from the implementer, applied by the lead). The lead ran the full local
  gate and it passes. Both rows are now `implemented`.
- 2026-11-08 CI at `802ae47`: ci.yml 42420891408 passed. mutants.yml 42420891449 had one
  TIMEOUT, `matrix.rs:449:5 is_identifier_char -> true`, in LIB-TOOL-001 code that this
  slice did not change.
  - Cause: under that mutant, a scan of the real repository is quadratic.
    `check_reporting` sorted before `function_scan`, which kills the mutant in under
    1 s, and `cargo test` stops at the first failing target.
  - Fix: the test file is renamed to `reporting.rs` and the reason is recorded in the
    triage document.
  - The durable fix is the new row LIB-QA-012.
- 2026-11-08 verification round 1, at `6019823`:
  [record](../verification/0005-m0-check-reporting.md).
  - LIB-TOOL-013 PASS, and it is now `verified`.
  - LIB-TOOL-014 FAIL (F1: the spanless form is reachable and untested; a mutant
    survives). Its prescription is P-1 to P-4.
  - Fix round 1: the tests go to the test engineer. The code needs no change.
- 2026-11-08 verification round 2, at `414263f`:
  [record](../verification/0005-m0-check-reporting-round2.md).
  - LIB-TOOL-014 PASS. P-1 to P-4 are met, and nothing new was found. It is now
    `verified`.
- 2026-11-08 close: both rows are verified, and the slice is done.
