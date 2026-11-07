# Verification 0005, round 1: m0-check-reporting

- Date: 2026-11-08
- Verdict: **FAIL** (partial). LIB-TOOL-013 passes, LIB-TOOL-014 fails.
- Row verdict: LIB-TOOL-013: PASS
- Row verdict: LIB-TOOL-014: FAIL
- Verifier: `sacm-verifier` (independent; read-only)
- Commit inspected: `601982399ef812c85b551d29d4d142e2e5f17fa4` (draft PR #12), with
  `git diff main...6019823`.
- Linux evidence at that commit, downloaded by the verifier:
  - ci.yml [42422372810](https://github.com/lasrod/libSACM/actions/runs/42422372810): every
    job succeeded.
    - In `mutants (PR diff)`: 20 mutants, 16 caught, 0 missed, 0 timeout, 4 unviable.
    - `test (ubuntu-latest)`: the `#[cfg(unix)]` test
      `lib_tool_013_a_tree_whose_name_holds_control_characters_is_named_escaped` passed.
  - mutants.yml [42422372773](https://github.com/lasrod/libSACM/actions/runs/42422372773):
    258 mutants, 251 caught, 0 missed, 0 timeout, 7 unviable.
    - `is_identifier_char -> true`, a timeout at `802ae47`, is now caught.
- Slice brief: [docs/slices/0005-m0-check-reporting.md](../slices/0005-m0-check-reporting.md)

## Rows verified

| Row | Title | Evidence (tests) |
|---|---|---|
| LIB-TOOL-013 | Matrix check names an overridden root | The 19 cited `lib_tool_013_*` tests in `xtask/tests/reporting.rs` meet all 5 criteria. They cover: <ul><li>the exact named OK line, problem header and load-error suffix;</li><li>escaping, on Windows and in the Linux-only tree test;</li><li>D2 (an override equal to the build root is still named, and the root is not canonicalised);</li><li>unchanged other commands, and `run` equal to `run_from(Build)`;</li><li>the binary with, without and with an empty `XTASK_ROOT`.</li></ul> The Linux mutants runs show 0 missed and 0 timeout. |

## Rows left open

| Row | Status kept | Reason |
|---|---|---|
| LIB-TOOL-014 | implemented | F1. The spanless form in criterion 1 is reachable but untested, and a mutant there survives. |

## Gates run (by the verifier, on Windows)

All pass:
- fmt;
- clippy with `-D warnings`;
- `cargo test --workspace --all-features` and `cargo test -p xtask` (32 of 33 `reporting` tests; the `#[cfg(unix)]` test runs on Linux);
- rustdoc with `-D warnings`;
- `matrix check` (153 rows);
- `cargo deny check`;
- hook tests (39).

## Findings and the complete prescription

### F1 (Major, LIB-TOOL-014): the spanless error has no test, though it is reachable

The brief said "toml 1.2 always gives a span". That is false.
- **Source:** in toml 1.2.0, a dotted key with 80 or more segments gives the error `recursion limit` with no span (`src/de/parser/key.rs:67-68`, `LIMIT = 80` in `de/parser/mod.rs:71`).
- **Reproduction:** `schema_version = 1` followed by an 81-segment `a.a.…a = 1` makes the binary print `xtask: docs/matrix/functionality-matrix.toml: recursion limit`.
- **Survivor:** in a scratch copy, the verifier replaced the spanless `format!("{MATRIX_PATH}: {message}")` with `error.to_string()`. The whole xtask suite still passed.

**Prescription:**
- **P-1.** Add the test `lib_tool_014_an_error_without_a_span_is_one_line_without_a_position`.
  - It uses the dotted-key fixture.
  - Preconditions: the parse fails, `span()` is `None`, and the message is non-empty.
  - It asserts exactly `Err(format!("{MATRIX_PATH}: {message}"))`, with no `\n`.
  - It must kill these mutants: `to_string()`; a `:1:1:` or `:0:0:` position added; the prefix dropped; the message dropped; the case sent to the positioned branch.
- **P-2.** The binary prints the spanless line exactly once on stderr under `XTASK_ROOT`, with the suffix for `matrix check`. Stdout is empty and the status is a failure. Check it for one more loading command, without the suffix.
- **P-3.** Correct the brief's "Not covered" note, and name the recursion-limit error as the spanless case.
- **P-4.** Provide new Linux mutation evidence (the PR-diff job and mutants.yml) at the fix head, with 0 missed and 0 timeout, and update the triage document for that head.

**Not required:**
- the `text.get(..span.start) == None` fallback, since no input reaching it was found;
- escaping on the spanless branch: the message is escaped before the branch, and the only spanless message known is fixed text.

### F2 (Minor, LIB-TOOL-014): the brief's log says the path is unreachable

This is fixed by P-3.

### Info

- **Triage document:** at `6019823`, the triage section still has placeholder run links. The uncommitted working-tree edit fills them correctly. Commit it with the fix round's run ids.
- **LIB-TOOL-010 test:** the change to `gate_hardening.rs::lib_tool_010_binary_prints_every_problem_on_stderr_and_fails` does not weaken it. The comparison is still exact, and the unnamed form is still asserted exactly by the library tests.
- **Mutation testing:** the two new unviable mutants, `matrix.rs:1305:48` with `+` replaced by `-` and by `*`, are compile errors (E0369). They hide no survivor: dropping the suffix is killed by `lib_tool_013_a_load_failure_from_xtask_root_ends_with_the_root`.
- **Exclusions:** `.cargo/mutants.toml` has no exclusion keys, and there is no `mutants::skip`.
- **API:** the new public items are only `RootSource` and `run_from`, as D1 approved.

## Follow-ups

- LIB-QA-012 (planned) is justified. The ordering fix only works because `reporting.rs` sorts after `function_scan`, and the only safeguard is a comment.
