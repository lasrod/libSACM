# Verification 0006, round 2: m0-test-scanner (after fix round 1)

- Date: 2026-11-08
- Verdict: **PASS.** LIB-TOOL-007 and LIB-TOOL-011 both pass.
- Row verdict: LIB-TOOL-007: PASS
- Row verdict: LIB-TOOL-011: PASS
- Verifier: `sacm-verifier` (independent; read-only)
- Commit inspected: `48cad81e8494776b640fcbaa3a6b87d6fd7d4bd5` (PR #13), `git diff bf2ae2d..48cad81`
- Contract: R1 to R6 of the [round 1 record](0006-m0-test-scanner.md), checked item by item, with the lead's options: R1 a `target` directly beside a `Cargo.toml`, and R2 option (b).
- Linux evidence at that commit, downloaded by the verifier:
  - ci.yml [42436264672](https://github.com/lasrod/libSACM/actions/runs/42436264672): all 11 jobs succeeded. In `mutants (PR diff)`: 217 mutants, 213 caught, 0 missed, 0 timeout, 4 unviable.
  - mutants.yml [42436264675](https://github.com/lasrod/libSACM/actions/runs/42436264675): 433 mutants, 422 caught, 0 missed, 0 timeout, 11 unviable. All 11 are structural. The set of mutants equals `cargo mutants --list`, and there are no exclusions.
- Slice brief: [docs/slices/0006-m0-test-scanner.md](../slices/0006-m0-test-scanner.md)

## Round-1 prescription

| Item | Result |
|---|---|
| R1 (build output) | **Met.** `is_build_output` treats `target` as build output only when a `Cargo.toml` sits directly beside it. `FsRepo::walk` and the citation check share the predicate `is_scanned_test_file`. The rule is documented in README rule 7, the `Repo` and `FsRepo` rustdoc, and the CHANGELOG. |
| R2(b) (citations restricted to scanned files) | **Met.** A citation of a file outside the scan gives `test file <path> is outside the scanned test files`. That check comes after the name check and replaces the citation's other checks. `doctest:` citations are exempt. |
| R3 (tests on disk) | **Met.** <ul><li>`src/target/mod.rs` gives exactly the `src` problem, whether the test runs or is ignored, cited or uncited.</li><li>`tests/target/mod.rs`, `tests/a/target/mod.rs` and `.claude/hooks/target/test_x.py` give exactly the ignored problem.</li><li>Build output is skipped in every scan root. A `target` with no `Cargo.toml` beside it is scanned.</li></ul> |
| R4 (tests for R2(b)) | **Met.** <ul><li>16 files outside the scan give exactly the scope problem.</li><li>9 files inside the scan give none, including `.RS`, `.PY`, and a `target` with no `Cargo.toml` beside it.</li><li>A missing file outside the scan gives only the scope problem. A missing file inside the scan still gives "does not exist".</li></ul> |
| R5 (evidence) | **Met.** The docs are updated. The Linux reports at `48cad81` show 0 missed and 0 timeout. The triage entry for that head matches the reports exactly. |
| R6 (nested skipped classes) | **Met.** `lib_tool_011_a_class_nested_in_a_skipped_class_keeps_the_outer_skip` covers a skipped and an undecorated `Inner`, with a control for a class that is not skipped. |
| Lows | **Met.** README rule 7 lists nested `cfg_attr`, an attribute after other code, and the prelude-qualified `ignore` (D11, LIB-TOOL-016). |

The verifier re-ran each F1 reproduction in a fresh scratch copy, and each now gives exactly the prescribed problem.

## Rows verified

| Row | Title | Evidence (tests) |
|---|---|---|
| LIB-TOOL-007 | Traced tests live outside production source | The 8 cited `lib_tool_007_*` tests in `xtask/tests/scanner.rs` cover: <ul><li>the `src` problem, cited or not, ignored or not, `src/target/` modules included;</li><li>exact component matching;</li><li>untraced unit tests and doctests under `src` are allowed;</li><li>the sort order;</li><li>`FsRepo` on disk;</li><li>the real repository.</li></ul> |
| LIB-TOOL-011 | Test scanner recognises tests and non-code precisely | The 47 cited `lib_tool_011_*` tests in `scanner.rs` and `function_scan.rs` cover: <ul><li>every comment and literal form in Rust and Python;</li><li>the attribute and skip tables;</li><li>nested skipped classes;</li><li>D3 problems;</li><li>D9 build output;</li><li>the D10 citation scope;</li><li>the real repository (348 running traced tests, the same as main's scanner, plus the new tests).</li></ul> |

Changed tests in verified rows (LIB-TOOL-001, LIB-TOOL-003 and LIB-TOOL-006) are not weakened. Their fixtures moved under the scan roots and keep their assertions.

## Gates run (by the verifier, on Windows)

All pass:
- fmt;
- clippy (`-D warnings`);
- `cargo test --workspace --all-features` and `cargo test -p xtask`;
- rustdoc (`-D warnings`);
- `matrix check` (154 rows);
- `cargo deny check`;
- hook tests (39, none skipped).

## Findings

| Severity | Row | Finding | Resolution |
|---|---|---|---|
| Info (new class) | — | The round-1 record overstated R2(b). It narrows the LIB-TOOL-001 hole but does not close it. A scanned test file can still be one that never runs, in two ways: <ul><li>a `.rs` file that no module tree includes;</li><li>a `.py` file under `crates/` or `xtask/`, outside CI's unittest discovery of `.claude/hooks`.</li></ul> | The round-1 sentence is corrected. Follow-up row LIB-TOOL-017. |
