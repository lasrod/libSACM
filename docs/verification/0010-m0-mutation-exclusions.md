# Verification 0010, round 1: m0-mutation-exclusions

- Date: 2026-11-09
- Verdict: **FAIL**
- Row verdict: LIB-QA-010: FAIL
- Verifier: `sacm-verifier` (independent; read-only)
- Commit inspected: `d7bd5cfe4b4d5b8ff5ee05883622593f338b6a69` (PR #17), with `git diff main...d7bd5cf`.
- Linux evidence:
  - ci.yml run 42557790292 succeeded. The `mutants exclusions` job printed `mutant exclusions OK: 0 entries, 0 excluded mutants, 553 mutants listed`. `mutants (PR diff)`: 87 caught (85 in `mutants.rs`, 2 in `lib.rs`), 0 missed, 0 timeouts, 3 unviable.
  - mutants.yml run 42557790303 succeeded: 0 missed, 0 timeouts.
  - The 3 unviable mutants are `Default::default()` returns for types without a `Default` (`check_exclusions`, `exclusion_entries`, `triage_assignments`). Their `vec![]` and `Ok(())` siblings are caught.
- Real cargo-mutants 27.1.0 in scratch copies:
  - A correct `exclude_re` + `exclude_globs` + `skip_calls` configuration gives `OK: 3 entries, 5 excluded mutants, 548 mutants listed`. The `--config` copies replace `.cargo/mutants.toml`, as the fake lister models.
  - An overlapping pair, entries matching nothing, an untriaged mutant, an invalid regex and a missing cargo-mutants each give the expected problems.
- Slice brief: [docs/slices/0010-m0-mutation-exclusions.md](../slices/0010-m0-mutation-exclusions.md)

No regression:
- the changed LIB-QA-007 case keeps its intent and its `[config-exclusion]` code;
- LIB-QA-007, -008 and -009 pass;
- item 5's controls are complete;
- item 6 is met.

## Rows left open

| Row | Status kept | Reason |
|---|---|---|
| LIB-QA-010 | implemented | F1 (High) and F2 to F8 |

## Findings and the complete prescription

| Severity | Finding | Required fix |
|---|---|---|
| **High** | **F1. In CI the listings are coloured, so any correct exclusion fails.** `ci.yml` sets the workflow-level `CARGO_TERM_COLOR: always`. cargo-mutants then writes ANSI codes into `--list` output, even to a pipe. The verifier reproduced a correct configuration giving 15 problems under `CARGO_TERM_COLOR=always`. `CLICOLOR_FORCE=1` forces colour even with `--colors never`, and `NO_COLOR=1` overrides both. The job passes today only because there are 0 entries. | (a) Every listing passes `--colors never`, and the child command sets `NO_COLOR=1` and removes `CLICOLOR_FORCE`. (b) A listing line holding a control character (ESC included) is a `[mutants-tool]` error naming the command, never compared and never silently stripped. (c) Tests on the built command's args, envs and current dir, for `None` and `Some(..)`, plus the line validation as a pure function. (d) Lead's end-to-end check: in a scratch copy with a correct, triaged `exclude_re`, `CARGO_TERM_COLOR=always CLICOLOR_FORCE=1 cargo xtask mutants exclusions` prints the OK line. Mutations to kill: drop `--colors never`, drop the env neutralisation, drop the control-character check. |
| Medium | **F2 (H5).** A duplicated entry is untested. Removing the entry by value instead of by index survives, and under it `exclude_re = ['X', 'X']` passes. | A control with the same string twice under one key, giving exactly 2 `[exclusion-empty]` plus the `[exclusion-mismatch]` for its triage rows. |
| Medium | **F3 (H12).** `break` instead of `continue` after a failed (c) listing survives. | A control where an earlier entry's (c) fails and a later entry has a problem. Assert both problems, and that every (c) is listed (2 + entries calls). |
| Medium | **F4 (H24).** `any(is_str)` instead of `all(is_str)` survives. | `exclude_re = ['<valid>', 1]` gives exactly `[config-exclusion]`, with no (c) listed. |
| Low-Medium | **F5 (H26).** Reading on past the end of the triage table survives. | A row-shaped line after a blank line that follows the table is ignored: no `[triage-format]` and no assignment. |
| Low | **F6 (H7/H30).** A triage row naming an entry that is not in the configuration counting as triaged survives. | A mutant excluded by an entry, whose only triage row names an `exclude_re` not in the configuration, gives exactly `[exclusion-untriaged]` (and the entry's mismatch, if any). |
| Low | **F7 (H1).** Dropping `current_dir(root)` survives. | Covered by F1 (c): assert `get_current_dir() == Some(root)`. |
| Low | **F8 (H29).** The rustdoc says nothing is listed for unreadable input; the tests allow either. | Assert that no call is made in the `config-parse`, `config-exclusion` and `triage-format` tests. |

Not held: H32 and H33 (equivalent in practice).
