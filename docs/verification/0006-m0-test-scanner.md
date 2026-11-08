# Verification 0006, round 1: m0-test-scanner

- Date: 2026-11-08
- Verdict: **FAIL.** LIB-TOOL-007 and LIB-TOOL-011 both fail.
- Row verdict: LIB-TOOL-007: FAIL
- Row verdict: LIB-TOOL-011: FAIL
- Verifier: `sacm-verifier` (independent; read-only)
- Commit inspected: `bf2ae2d3f98d4e5567d05e5fb56041606277cfe0` (draft PR #13), `git diff main...bf2ae2d`
- Linux evidence at that commit, downloaded by the verifier:
  - ci.yml [42431457421](https://github.com/lasrod/libSACM/actions/runs/42431457421): every job succeeded. `agent hook tests` on Ubuntu now runs `test_lib_tool_003_windows_paths_match_scopes_case_insensitively`. In `mutants (PR diff)`: 202 mutants, 198 caught, 0 missed, 0 timeout, 4 unviable.
  - mutants.yml [42431457415](https://github.com/lasrod/libSACM/actions/runs/42431457415): 421 mutants, 410 caught, 0 missed, 0 timeout, 11 unviable. All 11 are structural, and the set equals `cargo mutants --list`, with no exclusions.
- Slice brief: [docs/slices/0006-m0-test-scanner.md](../slices/0006-m0-test-scanner.md)

## Rows left open

| Row | Status kept | Reason |
|---|---|---|
| LIB-TOOL-007 | implemented | F1 |
| LIB-TOOL-011 | implemented | F1 and F2 |

## Also confirmed

- **Gates:** all pass locally (fmt, clippy, tests, rustdoc, `matrix check`, `cargo deny`, hook tests: 39, none skipped).
- **Real repository:** the new scanner lists the same 339 running traced tests as main's scanner.
- **No verified row's test is weakened.** The tests are LIB-TOOL-001 `only_marked_tests_count_as_traced`, the `Repo` doubles, and the restructured LIB-TOOL-003 hook test. That hook test keeps all three old assertions, adds the posix branch, and exercises the guard's only `os.name` branch both ways.

## Findings and the complete prescription

### F1 (Medium, both rows): the scan problems cover only walked files

`walk_sources` skips every directory named `target` at any depth. That includes module directories that cargo compiles, such as `crates/x/src/target/mod.rs` and `crates/x/tests/target/mod.rs`. Meanwhile `check_citations` resolves a citation against any readable file.

D3 removed the row-level problem for a cited ignored test because "problem 2 already reports it". That holds only for walked files. The verifier reproduced the gap on disk:
- A cited `#[ignore]`d traced test in `crates/demo/tests/a/target/mod.rs`, `crates/demo/src/target/mod.rs`, `tests/slow.rs` or `crates/demo/tests/slow.txt` gives no problem.
- An uncited traced test in `crates/demo/src/target/mod.rs` gives no `src` problem.

**Prescription. The lead chose these options:**
- **R1. Build output.** A directory named `target` is skipped only when it sits directly beside a `Cargo.toml`, as the repository's own `target/` or a package's `target/` does.
  - Any other `target` directory is scanned.
  - The rule appears in README rule 7 and in the `FsRepo` rustdoc.
- **R2, option (b): citations are restricted to scanned files.** A test citation whose file the scan does not cover gets the row-level problem `test file <path> is outside the scanned test files`. A file is not covered when:
  - it is not under `crates/`, `xtask/` or `.claude/hooks/`;
  - or it is not `.rs` or `.py`;
  - or it is under R1 build output.

  The walk and this check share one rule. This narrows the older LIB-TOOL-001 hole, but does not close it: round 2 notes that a scanned test file can still be one that never runs (LIB-TOOL-017). D3 is restated in the brief and the README.
- **R3. Tests through `FsRepo` on disk, and with `cmd_check(..).is_err()`:**
  1. `crates/demo/src/target/mod.rs` holds a running traced test. Uncited and cited, it gives exactly one `src` problem and no row problem.
  2. The same file holds a cited `#[ignore]`d traced test: exactly one `src` problem.
  3. `crates/demo/tests/target/mod.rs` holds an `#[ignore]`d traced test. Cited and uncited, it gives exactly one ignored problem.
  4. `.claude/hooks/target/test_x.py` holds a skipped test: it is scanned (no `Cargo.toml` beside it) and gives exactly one ignored problem.
  5. Build output is still skipped. `target/debug/x.rs` at the root, and `crates/demo/target/x.rs` with `crates/demo/Cargo.toml` present, give nothing.
- **R4. Tests for R2(b).** Citations of these files each give the new problem:
  - `tests/x.rs` (outside the roots);
  - `crates/demo/target/x.rs` (build output under R1);
  - `crates/demo/tests/x.txt`.

  A citation inside the roots gives none. The tests must kill these mutations:
  - the check removed;
  - only the root prefix checked, not the extension;
  - only the extension checked, not the build-output rule.
- **R5. Evidence.**
  - Update README rules 6 and 7, the `check` and `FsRepo` rustdoc, and the CHANGELOG.
  - Provide a new Linux cargo-mutants report (PR diff and the full shards) for the fix commit, with 0 missed and 0 timeout. Triage any new unviable mutants.

### F2 (Medium, LIB-TOOL-011): a class-skip mutation survives

Replacing `skipped_class.get_or_insert(indent)` with `skipped_class = Some(indent)` passes every test. Under that mutant:
1. a skipped class nested inside a skipped class resets the skip to the inner class's indent;
2. a later `def test_` in the outer class then counts as running.

**Prescription:**
- **R6.** Add a test with an outer skipped class holding a skipped inner class and a test, then a top-level test:
  - running `[c]`, ignored `[a, b]`;
  - the same with an undecorated `Inner`: running `[c]`, ignored `[a, b]`.

  It must kill two mutations: `get_or_insert` replaced by an assignment, and `skipped_class` cleared on any nested `class` line.

### Low

**Prelude-qualified `ignore`.** `#[::core::prelude::v1::ignore]` and `#[std::prelude::v1::ignore]` count as running.
- **Lead decision:** a known limit. It is listed in the README and the rustdoc, and carried in follow-up row LIB-TOOL-016.

**Limits missing from the README.** Two limits are listed in the rustdoc but not in README rules 6 and 7:
- a nested `cfg_attr(a, cfg_attr(b, ignore))`;
- an attribute after other code on its line.

They are to be added.

## Not held against these rows

- **Documented limits:** macro-generated tests, run-time skips, backslash continuations, tab width and nested `cfg_attr`.
- **Unrecognised forms:** `xfail`, `@ name`, `# [attr]`, and attributes after code.
- **Compiled-out tests:** tests excluded by `cfg`.
- **Python strings:** PEP 701 f-strings that reuse quotes.
- **Case:** a case-variant `SRC` component.

## Follow-ups

LIB-TOOL-016 (planned) collects the scanner forms not yet recognised:
- `cfg(any())`- and `cfg(FALSE)`-gated tests;
- module-level `pytestmark` and `__unittest_skip__`;
- `@pytest.mark.xfail`;
- `@ name`, written with a space;
- `# [attr]`;
- the prelude-qualified `ignore`.
