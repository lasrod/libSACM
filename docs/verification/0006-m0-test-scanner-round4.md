# Verification 0006, round 4: m0-test-scanner (after the F6 test-only round)

- Date: 2026-11-08
- Verdict: **PASS.** LIB-TOOL-011 passes, and LIB-TOOL-007 shows no regression.
- Row verdict: LIB-TOOL-011: PASS
- Verifier: `sacm-verifier` (independent; read-only)
- Commit inspected: `80de984174ca21a22523f4fd2ab80ea564944d94` (PR #13), `git diff c63c21b..80de984`.
  - Only tests, citations and the brief changed.
  - `git diff 2ff54e8..80de984 -- xtask/src .cargo` is empty.
- Contract: P1 to P4 of the [round 3 record](0006-m0-test-scanner-round3.md), checked item by item. After LIB-TOOL-011's second FAIL, the maintainer chose a test-only fix round.
- Linux evidence at that commit:
  - ci.yml [42442240234](https://github.com/lasrod/libSACM/actions/runs/42442240234): all 11 jobs succeeded. `mutants (PR diff)`: 231 mutants, 227 caught, 0 missed, 0 timeout, 4 unviable.
  - mutants.yml [42442240269](https://github.com/lasrod/libSACM/actions/runs/42442240269): 439 mutants, 428 caught, 0 missed, 0 timeout, 11 unviable. They are the same structural set as at `2ff54e8`, and the set equals `cargo mutants --list`, with no exclusions.
- Slice brief: [docs/slices/0006-m0-test-scanner.md](../slices/0006-m0-test-scanner.md)

The verifier's verdict: "LIB-TOOL-011: PASS at `80de984`. The round-3 prescription P1 to P4 is met item by item. The three new tests pin the `def` and `fn` keyword boundary, and each surviving round-3 mutation now fails them, as re-run by the verifier in a fresh scratch copy. The Linux cargo-mutants reports at this head show 0 missed and 0 timeout over the full `--list` set. LIB-TOOL-007 shows no regression."

## Round-3 prescription

| Item | Result |
|---|---|
| P1 (Python `def` boundary) | **Met.** `lib_tool_011_python_def_glued_to_a_test_name_defines_no_test` covers `deftest_…`, `deftest_…(self):` and `async deftest_…`. Each is tested at the top level, in a class body, under a skip, and in a skipped class. |
| P2 (Rust `fn` boundary) | **Met.** `lib_tool_011_rust_fn_glued_to_a_test_name_defines_no_test` covers all four forms. |
| P3 (citation path) | **Met.** `lib_tool_011_a_citation_of_a_name_glued_to_def_or_fn_is_not_defined` gives exactly the "is not defined" problem for the Rust file and for the Python file. |
| P4 (evidence) | **Met.** The tests are cited. The verifier re-ran both mutations: M1 (`python_def`) and M2 (`defined_functions`) are each killed by two tests. The Linux reports show 0 missed and 0 timeout. The triage entry matches the reports. |

## Rows verified

| Row | Title | Evidence (tests) |
|---|---|---|
| LIB-TOOL-011 | Test scanner recognises tests and non-code precisely | The 62 cited `lib_tool_011_*` tests in `xtask/tests/scanner.rs` and `function_scan.rs` cover: <ul><li>every comment and literal form;</li><li>the D4 and D5 tables;</li><li>D12, both ways: whitespace and comments inside paths and names, and keyword boundaries;</li><li>nested skipped classes;</li><li>D3, D9 and D10;</li><li>the real repository.</li></ul> |

LIB-TOOL-007 stays verified from [round 2](0006-m0-test-scanner-round2.md), with no regression.

## Gates run (by the verifier, on Windows)

All pass: fmt, clippy (`-D warnings`), the workspace tests, `cargo test -p xtask`, rustdoc (`-D warnings`), `matrix check` (155 rows), `cargo deny check`, and the hook tests (39).

## Findings

None. The verifier listed the two Info items from round 3 as still open. Both were in fact fixed in `c63c21b`: the period in the LIB-TOOL-016 requirement, and the reflow of README rule 7.
