---
slice: 0010
name: m0-mutation-exclusions
state: done
step: close
rows: [LIB-QA-010]
mode: pr
---

# Slice 0010: mutation exclusions checked mechanically

## Scope

LIB-QA-007 checks that every exclusion in `.cargo/mutants.toml` has a triage row, but
not what each exclusion actually excludes. This slice adds a command, `cargo xtask mutants
exclusions`, and a CI job that runs it. The command asks cargo-mutants for its mutant
listing with the configuration, without it, and with each entry removed. It shows that
each entry excludes exactly the mutants the triage table assigns to it.

| Row | Title | Status at start |
|---|---|---|
| LIB-QA-010 | Mutation exclusions are checked mechanically | planned |

## Out of scope

- **LIB-QA-012** (mutation outcomes independent of test-target order). Its likely fix, nextest
  as cargo-mutants' test tool, changes the LIB-QA-007 config policy (`test_tool` is a
  refused key) and both mutants workflows. It needs its own research.
- Exclusions themselves: there are none today. The command must pass on an empty set and
  catch every mismatch once entries exist.

## Normative basis

None. This is a quality row, so the acceptance criteria are the lead's. They are recorded
in the matrix row.

Facts checked by the lead with cargo-mutants 27.1.0:
- `cargo mutants --workspace --list` takes about 0.5 s and prints one mutant name per line.
  The name is the same string the triage table's Mutant cell holds.
- `--no-config` and `--config <FILE>` exist. A `--config` file replaces `.cargo/mutants.toml`.
  An `exclude_re` entry in such a file removes the matching names from the listing.
- Today the listings with and without the configuration are identical: 464 mutants.

## Design (fixed by the lead)

- **D1: the module.** A new public module `xtask::mutants`, in `xtask/src/mutants.rs`:

  ```rust
  /// Counts printed on success.
  #[derive(Debug, Clone, PartialEq, Eq)]
  pub struct ExclusionSummary {
      /// Exclusion entries checked (strings of exclude_re, exclude_globs, skip_calls).
      pub entries: usize,
      /// Mutants in the --no-config listing but not in the configured one.
      pub excluded: usize,
      /// Mutants in the configured listing.
      pub listed: usize,
  }

  /// `list(Some(text))` lists with a configuration file holding `text`;
  /// `list(None)` lists with `--no-config`. Each returns the mutant names, or
  /// the tool's error.
  pub fn check_exclusions(
      config: &str,
      triage: &str,
      list: &mut dyn FnMut(Option<&str>) -> Result<Vec<String>, String>,
  ) -> Result<ExclusionSummary, Vec<String>>;
  ```

  - **The problems** are `[code] text` strings, sorted, with every problem reported.
  - **Codes:**
    - `exclusion-adds`, `exclusion-untriaged`, `exclusion-mismatch` and `exclusion-empty`;
    - `mutants-tool`, when a listing fails;
    - `config-parse` and `triage-format`, for unreadable input.
  - **(a)** is `list(Some(config))`, **(b)** is `list(None)`, and **(c_e)** is
    `list(Some(config without e))`.
  - **The configuration without `e`:** the TOML table with that one string removed from
    its array, serialised with `toml`.
- **D2: the command.** `cargo xtask mutants exclusions`, dispatched in `xtask/src/lib.rs`.
  - It reads `.cargo/mutants.toml` and `docs/quality/mutants-triage.md`, and runs
    `cargo mutants --workspace --list`. It uses `$CARGO` or `cargo`, from the repository
    root. Configurations are written under `target/xtask-mutants/`.
  - On success it prints the summary line from the acceptance.
  - If cargo-mutants is missing, that is an error, never a skip.
- **D3: the CI job.** The lead adds the `mutants-exclusions` job in `ci.yml`, because
  `.github/` is outside every agent's scope. It is unconditional and runs on ubuntu, and
  its steps are listed in the acceptance. It is not a LIB-QA-008 gate job: its install
  step is a `run` that is not a gate. So its own regression test pins it.
- **D4: who writes what.**
  - **The test engineer:** `xtask/tests/mutants_exclusions.rs`, the `check_exclusions`
    tests with a fake lister, plus the CI job's regression test in
    `xtask/tests/repo_checks.rs`.
  - **The implementer:** `xtask/src/mutants.rs`, the dispatch, and the docs (the triage
    document and CONTRIBUTING.md).
  - **The lead:** `ci.yml`.
- **D5: the triage reader.** The command reads the triage table by the same rules as the
  LIB-QA-007 test reader in `xtask/tests/repo_checks.rs` (`TRIAGE_HEADER`, `table_cells`,
  the Excluded cell `—` or `<key>: <entry>`). It is a separate implementation in
  `xtask/src`, because tests do not export code.

## Tests

| Tests | Row | Criterion |
|---|---|---|
| 20 `lib_qa_010_*` tests in `xtask/tests/mutants_exclusions.rs` | LIB-QA-010 | `check_exclusions` with a fake lister. They cover each problem code, overlapping entries, a `skip_calls` entry, non-array keys, tool errors, sorted output, and the copies without one entry. |
| 10 `lib_qa_010_*` tests in `xtask/tests/repo_checks.rs` | LIB-QA-010 | The `mutants-exclusions` CI job: the real `ci.yml` and exact-code controls (missing, job keys, runner, steps, commands, guards). |

## Log

- 2026-11-09 research: skipped, because this is a quality row. The lead wrote the
  acceptance criteria, and the row is now `analyzed`. The lead decided D1 to D5.
- 2026-11-09 tests: 25 tests.
  - In `xtask/tests/mutants_exclusions.rs`: 15 `check_exclusions` tests with a fake
    lister. They are compile-red, because `xtask::mutants` does not exist yet.
  - In `repo_checks.rs`: 10 CI-job tests. The real-`ci.yml` one is red.
  - Lead decisions:
    - an unknown disposition is `[triage-format]`;
    - a non-array exclusion key is `[config-exclusion]`, which gets one more test;
    - triage rows name exact mutants, with their positions, so a moved line needs a
      re-triage;
    - the overlap semantics are as asserted.
- 2026-11-09 implementation (lead): the `mutants-exclusions` job is added to `ci.yml`
  (D3). The real-`ci.yml` test now passes. The implementer is writing
  `xtask/src/mutants.rs`.
- 2026-11-09 implementation (implementer): `xtask/src/mutants.rs` (`check_exclusions`,
  `Lister`, `ExclusionSummary`, `cmd_exclusions`), the dispatch in `lib.rs`, and
  `escape_controls` made `pub(crate)`. Unit tests in `src`. The triage doc and CHANGELOG
  are updated.
  - Local `cargo mutants -f xtask/src/mutants.rs`: 88 mutants, 85 caught, 0 missed,
    3 unviable.
  - The `CONTRIBUTING.md` edit was refused by the write guard: D4 had wrongly assigned
    that file to the implementer. The maintainer chose to have the lead write it, and
    the lead did.
  - The test engineer made the verified LIB-QA-007 case "every source of xtask, one by
    one" build its list from the real workspace, because the new source file broke its
    hard-coded list. Its intent and codes are unchanged.
  - The real command prints `mutant exclusions OK: 0 entries, 0 excluded mutants, 553
    mutants listed`.
  - A scratch copy with an untriaged `exclude_re` gives 2 `[exclusion-mismatch]` and
    2 `[exclusion-untriaged]`.
  - The full local gate passes. The row is `implemented`.
- 2026-11-09 PR #17 is a draft. CI run 42557790292 and mutants run 42557790303
  succeeded at `d7bd5cf`. Copilot left no comments.
- 2026-11-09 verification round 1:
  [record](../verification/0010-m0-mutation-exclusions.md). FAIL, and the row stays
  `implemented`.
  - F1 (High): the workflow-level `CARGO_TERM_COLOR: always` colours cargo-mutants'
    `--list` output, so in CI any correct exclusion would fail.
  - F2 to F8: untested branches (H5, H12, H24, H26, H7/H30, H1, H29).
  - Fix round 1: F1 and F7 go to the implementer (a colour-proof command builder and
    line validation, with unit tests); F2 to F6 and F8 go to the test engineer. The
    lead runs the F1 (d) end-to-end check.
- 2026-11-09 fix round 1:
  - **Implementer:** `listing_command` passes `--colors never`, sets `NO_COLOR=1`,
    removes `CLICOLOR_FORCE` and runs from the root. `listing_lines` makes any
    control character in a listing a `[mutants-tool]` error. Both have unit tests
    (F1, F7). Local `cargo mutants -f xtask/src/mutants.rs`: 94 mutants, 90 caught,
    0 missed, 4 unviable.
  - **Test engineer:** 4 new tests and 2 changed controls cover F2 to F6 and F8. Each
    named hand mutation was applied in `target/scratch` and is killed. The row now cites
    30 tests.
  - **Lead's F1 (d) check:** in a scratch copy, a correct, triaged `exclude_re` (the
    two `fetch_into` mutants) gives `mutant exclusions OK: 1 entries, 2 excluded
    mutants, 557 mutants listed`, both normally and under
    `CARGO_TERM_COLOR=always CLICOLOR_FORCE=1`.
  - The full local gate passes.
- 2026-11-09 verification round 2, at `335251e`:
  [record](../verification/0010-m0-mutation-exclusions-round2.md). FAIL (the row's
  second), on one Low finding.
  - F9 (the verifier's own round 1 miss): no test shows that `run_listing` applies
    `listing_lines`. The fix is one unit test.
  - F1 to F8 are met, and F1 (d) was reproduced independently.
  - The slice stops for the maintainer's decision.
- 2026-11-09 the maintainer approved a third, single-test fix round for F9. It goes to
  the implementer, as a unit test in `src`.
- 2026-11-09 fix round 2 (F9): the implementer added the unit test
  `a_listing_whose_output_holds_a_colour_code_is_an_error`, which runs `run_listing` on
  `cargo --color always --list`. The lead applied F1e in a scratch copy, and the test
  fails under it. The full gate passes.
- 2026-11-09 verification round 3, at `e03ba86`:
  [record](../verification/0010-m0-mutation-exclusions-round3.md). PASS, and
  LIB-QA-010 is now `verified`. CI attempt 1 lost its `cargo-deny` job to the GitHub
  release-download flake; the re-run passed.
- 2026-11-09 close: the slice is done.
