# Verification 0010, round 3: m0-mutation-exclusions (after fix round 2)

- Date: 2026-11-09
- Verdict: **PASS**
- Row verdict: LIB-QA-010: PASS
- Verifier: `sacm-verifier` (independent; read-only)
- Commit inspected: `e03ba86db1cc9f9e06754c8f79d8e2df42969315` (PR #17), with `git diff f37dcfa..e03ba86`.
- Fix round 2 was a single-test round, which the maintainer approved after the row's second FAIL.
- Contract: F9 of the [round 2 record](0010-m0-mutation-exclusions-round2.md).
- Linux evidence:
  - ci.yml run 42571009090 succeeded on attempt 2, on all 12 jobs, including `mutants (PR diff)` (92 caught, 4 unviable, 0 missed, 0 timeouts) and `mutants exclusions`.
  - Attempt 1 failed only in `cargo-deny`. The action's Docker build failed to download the cargo-deny release tarball, an infrastructure flake.
  - mutants.yml run 42571009239 succeeded: 0 missed, 0 timeouts.
- Slice brief: [docs/slices/0010-m0-mutation-exclusions.md](../slices/0010-m0-mutation-exclusions.md)

The verifier's verdict: "No open findings. F1 to F9 are all met. I found no new class."

## Round 2 prescription

| Item | Result |
|---|---|
| F9 | Met. `a_listing_whose_output_holds_a_colour_code_is_an_error` runs `run_listing` on `cargo --color always --list`. It requires the error that starts with the command's text and contains `with a control character, which is no mutant name`. The verifier reproduced F1e (`run_listing` bypassing `listing_lines`) in a scratch copy, and the new test fails under it. The test passes under `NO_COLOR=1 CARGO_TERM_COLOR=never` too. |

## Rows verified

| Row | Title | Evidence (tests) |
|---|---|---|
| LIB-QA-010 | Mutation exclusions are checked mechanically | The 30 cited `lib_qa_010_*` tests in `xtask/tests/mutants_exclusions.rs` and `xtask/tests/repo_checks.rs`, plus the unit tests in `xtask/src/mutants.rs`. The mutation kills are recorded in rounds 1 to 3. |

## Gates run (by the verifier, on Windows)

All exit 0:
- fmt;
- clippy (`-D warnings`);
- `cargo test --workspace --all-features` and `cargo test -p xtask`;
- rustdoc (`-D warnings`);
- `matrix check` (161 rows);
- `cargo deny check`;
- the hook tests (47);
- `cargo xtask mutants exclusions` (`0 entries, 0 excluded mutants, 559 mutants listed`).

The listings with and without the configuration are byte-identical (559 mutants), so the excluded set and the triaged set are both empty.

## Not held (possible follow-ups)

- F1g, a listing line of only whitespace;
- escaping of control characters in a program path in `command_text`;
- H32 and H33, equivalent in practice.
