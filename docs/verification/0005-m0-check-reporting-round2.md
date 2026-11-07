# Verification 0005, round 2: m0-check-reporting (after fix round 1)

- Date: 2026-11-08
- Verdict: **PASS.** LIB-TOOL-014 passes. LIB-TOOL-013 stays verified (round 1), with no regression.
- Row verdict: LIB-TOOL-014: PASS
- Verifier: `sacm-verifier` (independent; read-only)
- Commit inspected: `414263ff8b180832ed635410279028efcc01eb29` (PR #12), `git diff 6019823..414263f`.
  `xtask/src` is byte-identical to round 1.
- Contract: the [round 1 record](0005-m0-check-reporting.md), P-1 to P-4, checked item by item.
- Linux evidence at that commit, downloaded by the verifier:
  - ci.yml [42423497546](https://github.com/lasrod/libSACM/actions/runs/42423497546): every
    job succeeded.
    - `mutants (PR diff)`: 20 mutants, 16 caught, 0 missed, 0 timeout, 4 unviable.
    - The two new tests and the `#[cfg(unix)]` LIB-TOOL-013 test pass on Ubuntu.
  - mutants.yml [42423497528](https://github.com/lasrod/libSACM/actions/runs/42423497528):
    258 mutants, 251 caught, 0 missed, 0 timeout, 7 unviable. These are the same unviable
    mutants as in round 1, and they are listed in `docs/quality/mutants-triage.md`.
- Slice brief: [docs/slices/0005-m0-check-reporting.md](../slices/0005-m0-check-reporting.md)

## Round-1 prescription

| Item | Result |
|---|---|
| P-1: a test for the spanless form | **Met.** `lib_tool_014_an_error_without_a_span_is_one_line_without_a_position` uses the 81-segment dotted-key fixture. Its preconditions check that the parse fails, that there is no span, and that the message is non-empty. It asserts the exact `<path>: <message>` line. The verifier applied six hand mutations in a scratch copy and the test killed all six: <ul><li>`to_string()`;</li><li>a `:1:1:` position;</li><li>a `:0:0:` position;</li><li>the prefix dropped;</li><li>the message dropped;</li><li>the case sent to the positioned branch.</li></ul> |
| P-2: the binary prints the spanless line | **Met.** `lib_tool_014_binary_prints_an_error_without_a_span_on_one_stderr_line` covers all 5 loading commands, with the exact stderr, empty stdout and a failure status. Only `matrix check` adds the root suffix. |
| P-3: the record is corrected | **Met.** The slice log now names the recursion-limit error as the spanless case. |
| P-4: new Linux mutation evidence | **Met.** The PR job and mutants.yml show 0 missed and 0 timeout. The triage document's entry for `414263f` matches the reports. |

## Rows verified

| Row | Title | Evidence (tests) |
|---|---|---|
| LIB-TOOL-014 | Matrix load errors are one line each | The 16 cited `lib_tool_014_*` tests in `xtask/tests/reporting.rs` meet all 4 criteria: <ul><li>the positioned form for a syntax error, an unknown key, a missing field, a wrong type and a later row;</li><li>character columns;</li><li>the spanless form;</li><li>escaped messages and read errors;</li><li>the same line from every loading command;</li><li>the binary's stderr.</li></ul> The Linux mutants runs show 0 missed and 0 timeout. |

## Gates run (by the verifier, on Windows)

All pass:
- fmt;
- clippy with `-D warnings`;
- `cargo test --workspace --all-features` and `cargo test -p xtask` (34 `reporting` tests here; the `#[cfg(unix)]` one runs on Linux);
- rustdoc with `-D warnings`;
- `matrix check` (153 rows);
- `cargo deny check`;
- hook tests (39).

## Findings

| Severity | Row | Finding | Resolution |
|---|---|---|---|
| Info | LIB-TOOL-014 | The triage entry for the fix-round-1 head was accurate but uncommitted. | Committed with this record. |
| Info | LIB-TOOL-013 | No regression: the code is unchanged and all 19 tests pass. | none |

**Not held,** as in round 1:
- the `text.get(..span.start) == None` fallback, because no reachable input is known;
- escaping on the spanless branch, because its only known message is fixed text.

## Follow-ups

- LIB-QA-012 (planned) stays justified.
