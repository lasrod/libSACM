# Verification 0006, round 3: m0-test-scanner (after the Copilot fix, D12)

- Date: 2026-11-08
- Verdict: **FAIL.** LIB-TOOL-011 fails; LIB-TOOL-007 shows no regression.
- Row verdict: LIB-TOOL-011: FAIL
- Verifier: `sacm-verifier` (independent; read-only)
- Commit inspected: `2ff54e8a76cfd8130665d86e03b6e470626c6880` (PR #13), with `git diff 5ad1529..2ff54e8`
- Context: LIB-TOOL-011 was verified in [round 2](0006-m0-test-scanner-round2.md). Three Copilot comments on PR #13 then showed gaps, and the lead demoted the row and added D12 to the brief.
- Linux evidence at that commit:
  - ci.yml [42439740610](https://github.com/lasrod/libSACM/actions/runs/42439740610): all 11 jobs succeeded. `mutants (PR diff)`: 231 mutants, 227 caught, 0 missed, 0 timeout, 4 unviable.
  - mutants.yml [42439740608](https://github.com/lasrod/libSACM/actions/runs/42439740608): 439 mutants, 428 caught, 0 missed, 0 timeout, 11 unviable (structural). The set equals `cargo mutants --list`, and there are no exclusions.
- Slice brief: [docs/slices/0006-m0-test-scanner.md](../slices/0006-m0-test-scanner.md)

The verifier's verdict: "LIB-TOOL-011: FAIL at 2ff54e8. The implementation fixes the three Copilot comments and follows D12 without over-accepting. The real repository and two large external corpora scan the same as before the fix. But no test fails when the whitespace requirement after `def` (python_def) or after `fn`/`def` (defined_functions) is dropped. LIB-TOOL-007: no regression."

## Confirmed

- **Copilot fixes:** all three comments are fixed and pinned. Against the code at `5ad1529`, 10 of the 12 new tests fail. The other 2 are guards.
- **D12 does not over-accept.** These are all rejected, and tests pin each rejection: `#[test :: helper]`, `#[ignore :: x]`, `cfg_attr(miri, ignore :: x)`, `@unittest.skip .helper`, `.fn` and `r#fn`.
- **No test was weakened.** The three moved template consts are byte-identical to the inline arrays they replace.
- **Same scan results as before the fix:**

  | Scanned | Result |
  |---|---|
  | The real repository | 361 tests and 840 defined names, the same |
  | The cargo registry sources | 36,053 Rust tests, the same |
  | Python 3.14 `Lib` | 31,964 Python tests, the same |

  The only differences are the intended ones: `self.fn` and `.def` are no longer read as definitions.
- **LIB-TOOL-007:** no regression.
- **Gates:** all pass. The triage entry for `2ff54e8` matches the reports.

## Findings

### F6 (Medium, test adequacy): the `def`/`fn` keyword boundary is not pinned

Two hand mutations survive every test:
- **`python_def`:** the mutation is `after_keyword(code, "def")` replaced by `strip_prefix("def")` plus a trim. Under it, `deftest_lib_tool_011_x()`, a valid Python call statement, counts as a running traced test.
- **`defined_functions`:** the same mutation applied to `fn`/`def`. Under it, `#[test]\nfnlib_tool_011_x() {}` counts as a test.

The existing negatives, such as `define` and `fnord`, leave a remainder after the keyword that is not a test name. They cannot detect either mutation.

**Prescription (complete):**
- **P1. Python `def` boundary.** No running and no ignored test may result from:
  - `deftest_lib_tool_011_x()`;
  - `deftest_lib_tool_011_x(self):`, also indented in a class body;
  - `async deftest_lib_tool_011_x(self):`;
  - each of the above under `@unittest.skip("x")`.
- **P2. Rust `fn` boundary.** No running and no ignored test may result from:
  - `#[test]\nfnlib_tool_011_x() {}`;
  - `#[test] fnlib_tool_011_x() {}`;
  - `#[test]\npub fnlib_tool_011_x() {}`;
  - `#[test]\n#[ignore]\nfnlib_tool_011_x() {}`.
- **P3. Citation path.** A citation of each of these files gives exactly the "is not defined" problem:
  - a Rust file holding only `fnlib_tool_011_x`;
  - a Python file holding only `deftest_lib_tool_011_x()`.
- **P4. Evidence.**
  - Cite the new tests.
  - Provide new Linux ci.yml and mutants.yml reports with 0 missed and 0 timeout, and a mutant set equal to `--list`.
  - Update the triage entry.
  - The verifier re-runs both surviving mutations.

### Info

- **LIB-TOOL-016:** a missing period in the requirement. The lead fixed it.
- **README rule 7:** one long line. The lead reflowed it.

## Not held to

- the LIB-TOOL-016 forms;
- form feed or Unicode whitespace after keywords;
- backslash continuations;
- PEP 614 parenthesized decorators.

## Status

This is LIB-TOOL-011's **second FAIL**: it failed in round 1 and passes only in round 2. The slice process stops the row for the maintainer's decision. The fix is test-only (P1 to P4).
