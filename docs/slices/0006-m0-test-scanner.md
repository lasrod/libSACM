---
slice: 0006
name: m0-test-scanner
state: done
step: close
rows: [LIB-TOOL-007, LIB-TOOL-011]
mode: pr
---

# Slice 0006: a precise traced-test scanner

## Scope

The traced-test scanner in `xtask/src/matrix.rs` reads source line by line. It does not
know about strings or comments, it counts `#[ignore]`d tests as evidence, and it misses
some test forms. Nothing keeps traced tests out of production source. This slice makes
the scanner precise.
- **LIB-TOOL-011.** The scanner skips comments and literals, in Rust and in Python.
  Ignored and skipped tests are never evidence. It recognises `#[test_case]`,
  `#[rstest]` and Python `async def test_`.
- **LIB-TOOL-007.** A traced test under a `src` directory is reported.

| Row | Title | Status at start |
|---|---|---|
| LIB-TOOL-007 | Traced tests live outside production source | planned |
| LIB-TOOL-011 | Test scanner recognises tests and non-code precisely | planned |

## Out of scope

- **Rust macros that generate tests** (`macro_rules!`, proc macros other than the named
  attributes): a generated test is not visible in the source text.
- **Python runtime skips** (`self.skipTest(...)` and `pytest.skip()` in a test body):
  they cannot be seen statically.
- **Matrix-wide test timing and order:** LIB-QA-012.

## Normative basis

None. These are tooling rows, so the acceptance criteria are the lead's (see the
matrix). Sources: verification 0001, round 2 (a Low finding, and the inline-test
follow-up), and the round-6 prescription (R63, and the notes on other classes).

## Design (fixed by the lead before the tests)

- **D1 (masking):** the scanner first blanks comment and literal contents, keeping
  every newline, then runs the line-based test recognition on the masked text.
  - Rust and Python each have their own lexer, and the criteria list the forms each
    covers.
  - Attribute and decorator arguments are masked too, so `#[test_case("a]b")]` cannot
    unbalance the bracket count.
  - The implementer may use another technique only if the behaviour is the same.
- **D2 (public API):**
  - `traced_tests(text, python) -> Vec<String>` keeps its signature. It returns the
    traced tests that run.
  - New: `pub fn ignored_traced_tests(text: &str, python: bool) -> Vec<String>`, for
    the ignored or skipped traced tests, in source order.
  - New required trait method: `Repo::ignored_functions(&self) -> Vec<(String, String)>`,
    with `(file, name)` pairs like `traced_functions`. It has no default, so every
    `Repo` must decide.
- **D3 (problems):** each scanned traced test gives at most one file-level problem:
  1. under `src`: `<file>: traced test `<name>` is under a `src` directory; traced tests belong in test files`;
  2. otherwise, if ignored or skipped: `<file>: traced test `<name>` is ignored or skipped, so it is not evidence`;
  3. otherwise, if no row cites it: the existing orphan problem.

  These come together, sorted by `(file, name)`, where the orphan problems come today.
  A citation of an ignored test gives no row-level problem, because problem 2 already
  reports it.
- **D4 (test-style attributes):** an attribute is test-style when the last path segment
  is `test`, `test_case` or `rstest`. An attribute is ignore-style when its path is
  `ignore`, or when it is `cfg_attr(...)` and lists `ignore` after the predicate.
- **D5 (Python skips):** a skip is a decorator whose dotted name's last segment is
  `skip`, `skipIf`, `skipUnless`, `skipif` or `expectedFailure`.
  - On a `def`, it skips that function.
  - On a `class`, it skips every `def test_...` indented deeper than the class line,
    until the next non-blank line at the class's indent or less.
- **D6 (comments around attributes):** a comment counts as whitespace between an
  attribute and its function, delimiters included. Examples: a `/* c */` line between
  `#[test]` and `fn ...`, and `/* c */ #[test] fn ...` on one line. This is the safe
  reading: otherwise a traced test is not seen at all.
- **D7 (end of a skipped class):** a comment-only line, or a line that holds only
  string content, does not end a skipped class.
- **D8 (`async def`):** any run of spaces or tabs may separate `async` and `def`.

- **D9 (build output), fix round 1, R1:** a directory named `target` is skipped only
  when it sits directly beside a `Cargo.toml`. Any other `target` directory is scanned.
- **D10 (scanned files), fix round 1, R2(b):** one predicate decides whether a file is
  scanned. The file must:
  - be under `crates/`, `xtask/` or `.claude/hooks/`;
  - have the extension `.rs` or `.py`;
  - not be under D9 build output.

  The walk uses this predicate. A test citation of a file outside it gets the row
  problem `test file <path> is outside the scanned test files`. Every cited test
  therefore lies in a scanned file, so the D3 problems cover it.
  - A missing file outside the scan gives only the scope problem; the file is not
    looked up.
  - `doctest:` citations are not subject to D10, because doctests can live in READMEs
    and docs.
  - The `Cargo.toml` lookup uses `Repo::exists`, so the trait gains no new method.
- **D11:** a prelude-qualified `ignore` (`#[::core::prelude::v1::ignore]`) is a known
  limit, carried in LIB-TOOL-016.

- **D12 (whitespace), from the Copilot review of PR #13:** names are read as tokens,
  past whitespace and masked comments. So:
  - an attribute path's `::` may have whitespace or comments around it:
    `#[tokio :: test]`, `#[tokio/* c */::test]`, and a path split across lines;
  - `#` and `[` may be separated by whitespace (`# [test]`);
  - a decorator's dots may have whitespace around them (`@unittest. skip`,
    `@ unittest.skip`);
  - the keywords `class`, `def`, `async def` and `fn` may be followed by any run of
    spaces or tabs.

  The whole path or dotted name is judged, never just its first segment.

## Tests

Most of the 46 tests are in `xtask/tests/scanner.rs`. One is in `function_scan.rs`.

| Tests | Row | Criterion |
|---|---|---|
| 7 `lib_tool_007_*` tests | LIB-TOOL-007 | Criteria 1 to 3: <ul><li>the `src` problem, cited or not, ignored or not;</li><li>exact component matching (`source/`, `srcs/`, `my_src/`, `src_old/`, `src.rs`);</li><li>untraced unit tests and doctests under `src` are allowed;</li><li>sorting with the other scan problems;</li><li>`FsRepo` on disk;</li><li>the real repository.</li></ul> |
| 39 `lib_tool_011_*` tests | LIB-TOOL-011 | Criteria 1 to 5: <ul><li>every Rust and Python comment and literal form, with resume controls and tails at the end of the file;</li><li>lifetimes against char literals;</li><li>the D4 attribute table, both ways;</li><li>the D5 skips at `def` and `class` level;</li><li>D3 problems;</li><li>`FsRepo`;</li><li>the real repository;</li><li>a self-check that the scanner finds exactly the tests of its own test files;</li><li>a deadline test in `function_scan.rs`, which catches a lexer hang early.</li></ul> |

Existing tests changed, and none weakened:
- **`matrix.rs::lib_tool_001_only_marked_tests_count_as_traced`:** its `#[ignore]` became `#[should_panic]`. The old fixture is re-pinned under the new rule in `lib_tool_011_the_ignored_test_lib_tool_001_used_to_count_is_ignored`.
- **The test doubles** in `matrix.rs`, `gate_hardening.rs` and `function_scan.rs` gain `ignored_functions`.
- **`.claude/hooks/test_agent_write_guard.py::test_lib_tool_003_windows_paths_match_scopes_case_insensitively`:**
  - It was `skipUnless(os.name == "nt")`. The hook CI job runs on Ubuntu, so the test had never run in CI.
  - It now runs on every OS. It patches the guard module's `os` reference to `nt` and to `posix`, and asserts both rules through `glob_to_regex`, `matches_any` and `decide`.
  - The guard itself is unchanged.

## Log

- 2026-11-08 research: skipped, because these are tooling rows. The lead wrote the
  acceptance criteria, and both rows are now `analyzed`.
- 2026-11-08 tests: 46 tests. The lead confirmed that four targets (`scanner`, `matrix`,
  `gate_hardening`, `function_scan`) fail to compile, only on `ignored_traced_tests`
  and `Repo::ignored_functions`. The hook tests pass (39, none skipped).
  - The lead's decisions: D6 to D8, and restructuring the Windows-only LIB-TOOL-003
    hook test (option a) instead of exempting conditional skips.
  - Both rows are now `tests-failing`.
- 2026-11-08 implementation: the Rust lexer `mask_rust` and the Python lexer
  `mask_python` blank comments and literals while keeping every line. The other
  pieces:
  - attribute and decorator recognition across lines (D4 to D8);
  - `check_scanned_tests`, which replaces `check_orphans` (D3);
  - a per-check cache of masked files: `matrix check` 0.58 s, against 0.9 s before.

  On the real repository, the running traced tests are the same 339 as with the old
  scanner. A local cargo-mutants run turned up four misses; three were fixed by
  simplifying code and one by a doctest on bracket nesting. The README rules 6 and 7
  text came from the implementer and the lead applied it. The lead ran the full local
  gate and it passes. Both rows are now `implemented`.
  - Follow-up for the test engineer: integration fixtures for attribute bracket nesting
    outside literals.
- 2026-11-08 verification round 1, at `bf2ae2d`:
  [record](../verification/0006-m0-test-scanner.md). Both rows FAIL.
  - F1: an ignored or `src` test passes in a file the walk skips (a `target` module
    directory) or never visits (citations resolve against any file).
  - F2: a class-skip mutation survives.
  - Lead decisions: R1 becomes D9, R2(b) becomes D10, and D11 records the known limit.
  - New row: LIB-TOOL-016.
  - Fix round 1: the tests (R3, R4, R6) go to the test engineer, then the code and docs
    (R1, R2, R5) go to the implementer.
- 2026-11-08 fix round 1, tests: 9 new tests for R3, R4 and R6. Existing fixtures that
  cited files outside D10 (`t.rs`, `x.py`) were moved under the scan roots, and none was
  weakened. 7 `scanner` tests fail today, on D9 and D10.
- 2026-11-08 fix round 1, implementation: `is_scanned_test_file` and `is_build_output`
  form a single predicate for D9 and D10. Both the walk (`FsRepo::walk`) and
  `check_citations` use it. The README rules 6 and 7 text came from the implementer,
  and the lead applied it. All 54 `scanner` tests pass, and the lead ran the full
  local gate. A local cargo-mutants run of the PR diff gave 217 mutants: 213 caught,
  4 unviable, 0 missed.
- 2026-11-08 verification round 2, at `48cad81`:
  [record](../verification/0006-m0-test-scanner-round2.md). Both rows PASS: R1 to R6
  are met, and the F1 reproductions now give the prescribed problems.
  - Info: the round-1 record overstated R2(b). The sentence is corrected, and
    follow-up row LIB-TOOL-017 is added.
  - Both rows are now `verified`.
- 2026-11-08 close: the slice is done.
- 2026-11-08 Copilot review of PR #13: 3 comments, all valid. They are whitespace in
  attribute paths, in decorator names, and after `class`.
  - The lead added D12 and widened it to `# [` and to whitespace after `def` and `fn`.
    `# [` and `@ name` move out of LIB-TOOL-016.
  - Regression tests went to the test engineer.
  - 12 regression tests (D12): attribute paths, `# [`, decorator dots, `class`,
    `def`, `async def` and `fn` with any whitespace, and the negatives (`r#fn`, `.fn`,
    `classy`, `define`). 10 fail today.
  - LIB-TOOL-011 goes back to `tests-failing`, and its round-2 verification no longer
    holds. It needs re-verification after the fix. LIB-TOOL-007 is untouched and stays
    verified.
  - D12 implementation: `attribute_open`, token-reading `attribute_path` and
    `last_dotted_segment`, `after_keyword`, and a shared `defined_functions` keyword
    rule. The README text came from the implementer and the lead applied it. All 66
    `scanner` tests pass, and the real repository yields the same 361 running traced
    tests. A local cargo-mutants run of the fix gave 36 mutants: 34 caught,
    2 unviable, 0 missed.
    - The two remaining forms go to LIB-TOOL-016: `#` at a line end with `[` on the
      next line, and `fn` with its name on the next line.
    - LIB-TOOL-011 is `implemented` again.
- 2026-11-08 verification round 3, at `2ff54e8`:
  [record](../verification/0006-m0-test-scanner-round3.md). LIB-TOOL-011 FAIL (F6).
  - The implementation is correct, but the `def`/`fn` keyword boundary is not pinned:
    two hand mutations survive.
  - The fix is test-only (P1 to P4).
  - This is the row's second FAIL, so the slice stops for the maintainer.
  - LIB-TOOL-007 shows no regression.
- 2026-11-08 the maintainer chose a test-only fix round for F6 (P1 to P4).
- 2026-11-08 F6 fix round, tests only: 3 tests pin the `def`/`fn` keyword boundary
  (P1 to P3). Each fails under the matching surviving mutation: M1 in `python_def`,
  M2 in `defined_functions`.
- 2026-11-08 verification round 4, at `80de984`:
  [record](../verification/0006-m0-test-scanner-round4.md). LIB-TOOL-011 PASS
  (P1 to P4 met), and it is `verified` again. LIB-TOOL-007 stays verified.
- 2026-11-08 close: the slice is done.
