# Verification 0010, round 2: m0-mutation-exclusions (after fix round 1)

- Date: 2026-11-09
- Verdict: **FAIL**, on one Low finding (F9), which the verifier names as its own round 1 miss inside F1's scope.
- Row verdict: LIB-QA-010: FAIL
- Verifier: `sacm-verifier` (independent; read-only)
- Commit inspected: `335251e61cb70a4a52d2f7dac1b0cc88392d079b` (PR #17), with `git diff d7bd5cf..335251e`.
- Contract: F1 to F8 of the [round 1 record](0010-m0-mutation-exclusions.md).
- Linux evidence:
  - ci.yml run 42561394451, `mutants (PR diff)`: 92 caught, 0 missed, 0 timeouts, 4 unviable.
  - mutants.yml run 42561394441: 0 missed, 0 timeouts.
  - The new unviable mutant is `listing_command -> Default::default()`, because `Command` has no `Default`. Its behaviour is covered by `a_listing_command_is_colourless_and_runs_from_the_root`.
- **F1 (d), reproduced independently** with cargo-mutants 27.1.0:
  - A triaged `exclude_re` for the two `fetch_into` mutants prints `mutant exclusions OK: 1 entries, 2 excluded mutants, 557 mutants listed`. It does so with no colour variables, under `CARGO_TERM_COLOR=always CLICOLOR_FORCE=1`, and with `NO_COLOR` empty.
  - Under `CARGO_TERM_COLOR=always`, the listing holds 3202 ESC bytes; with the safeguards, 0.
  - With the safeguards removed, the command gives 2 `[mutants-tool]` problems and exits 1.
- Gates, all exit 0:
  - fmt;
  - clippy (`-D warnings`);
  - `cargo test --workspace --all-features` and `cargo test -p xtask`;
  - rustdoc (`-D warnings`);
  - `matrix check` (161 rows);
  - `cargo deny check`;
  - the hook tests (47).
- Slice brief: [docs/slices/0010-m0-mutation-exclusions.md](../slices/0010-m0-mutation-exclusions.md)

## Round 1 prescription

| Item | Result |
|---|---|
| F1 (a) `--colors never`, `NO_COLOR=1`, `CLICOLOR_FORCE` removed | Met. |
| F1 (b) a control character is a `[mutants-tool]` error | Met in code; the call site is untested (F9). |
| F1 (c) builder and line-check tests | Met. |
| F1 (d) end-to-end check | Met, and reproduced. |
| F2 (H5) to F8 (H29) | All met. Each hand mutation is killed. |

## Rows left open

| Row | Status kept | Reason |
|---|---|---|
| LIB-QA-010 | implemented | F9 |

## Findings and the complete prescription

| Severity | Finding | Required fix |
|---|---|---|
| Low | **F9.** No test shows that `run_listing` passes its stdout through `listing_lines`. Mutation F1e (the old non-empty-lines filter in `run_listing`) survives. Its effect fails closed: wrong `[exclusion-*]` problems instead of `[mutants-tool]`, never a false pass. This is the complete remainder of F1. | One unit test in `xtask/src/mutants.rs`. It calls `run_listing` on a real command whose stdout holds ESC, for example `Command::new(cargo()).args(["--color", "always", "--list"])` (169 ESC bytes measured, even under `NO_COLOR=1`). It asserts an `Err` that starts with the command's text and contains `with a control character, which is no mutant name`, without depending on cargo's exact help text. It must kill F1e. |

Not held:
- F1g (a whitespace-only line);
- escaping of control characters in a program path in `command_text`;
- H32 and H33 (equivalent in practice).
