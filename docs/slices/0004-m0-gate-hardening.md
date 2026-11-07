---
slice: 0004
name: m0-gate-hardening
state: in-progress
step: verification
rows: [LIB-TOOL-005, LIB-TOOL-006, LIB-TOOL-009, LIB-TOOL-010]
mode: pr
---

# Slice 0004: harden the matrix gate

## Scope

`cargo xtask matrix check` gets stricter in four ways, all in `xtask/src/matrix.rs`:
- **LIB-TOOL-005:** a verified row needs one machine-readable PASS line for that row in
  its record. Today any mention of the id is enough.
- **LIB-TOOL-006:** every cited path must be a relative path inside the repository, and
  a rejected path is never read.
- **LIB-TOOL-009:** area codes must be uppercase ASCII letters.
- **LIB-TOOL-010:** a failing check returns every problem in its error, one per line,
  in a deterministic order, instead of printing the problems only to stderr.

| Row | Title | Status at start |
|---|---|---|
| LIB-TOOL-005 | Verification records carry a per-row verdict the gate reads | planned |
| LIB-TOOL-006 | Cited paths stay inside the repository | planned |
| LIB-TOOL-009 | Area table is validated | planned |
| LIB-TOOL-010 | Every gate problem reaches the user | planned |

## Out of scope

- **LIB-TOOL-007 and LIB-TOOL-011** (where traced tests live, and test-scanner
  precision) are left for a scanner slice.
- **LIB-TOOL-008** (the write guard's launch path) is left for a guard slice.
- **Symlinks:** LIB-TOOL-006 is a lexical check, so a symlink inside the repository that
  points outside it is not detected.
- **Other records:** verdict lines are required only in records that verified rows
  cite.

## Normative basis

There is none. These are tooling rows, with no SACM clause behind them. The sources are
the follow-ups from the verification 0001 records, as the rows state.

## Design

`xtask` is internal (`publish = false`) and these rows change no public `sacm` API, so
there is no architect brief. The lead's decisions:

- **D1, verdict line format:** `- Row verdict: <ROW-ID>: <PASS|FAIL|OPEN>`. It is a list
  item with no indent, matched after trailing spaces and tabs are removed, and never
  inside a fenced code block. It lives in the record's header block.
  - **Done by the lead in this slice:** the eight cited records gained lines that match
    the verdicts already written in them. The template and `docs/matrix/README.md`
    rule 5 describe the format.
- **D2, path rule:** a cited path is rejected if it is empty, starts with `/`, contains
  `\` or `:`, or has an empty, `.` or `..` component. Dot-names such as `.github` are
  allowed.
- **D3, reporting:** `cmd_check` returns
  `Err("functionality matrix: <n> problem(s)\n  <p1>\n  <p2>…")` and does not write to
  stderr. `main` already prints `xtask: <error>`. Control characters in values are
  escaped, so each problem stays on one line. Orphan problems are sorted.
- **Mutation testing:** every new line in `xtask/src` must be killed. The PR's
  `mutants` job runs cargo-mutants on the diff.

## Tests

There are 38 tests in `xtask/tests/gate_hardening.rs`: 17 for LIB-TOOL-005, 11 for -006, 4 for -009 and 6 for -010. The row `tests` arrays list each one.

- **Red state:** 29 fail on the missing behaviour. Nine pass already: six are positive controls, and three check that the real repository's data meets the new rules.
- **Changed for the new format, not weakened:** `xtask/tests/matrix.rs` (`evidence_case`, plus two LIB-TOOL-001 tests) and `xtask/tests/run.rs` (two tests now compare only the error's first line).

## Log

- 2026-11-08: slice chosen after PR #9 merged. The rows are tooling rows, so research
  was skipped and the lead wrote their acceptance criteria. Status: analyzed. The lead
  added verdict lines to the eight cited records, the record template and README
  rules 5, 10 and 11.
- 2026-11-08 tests: 38 tests written; 29 fail on missing behaviour. The rows are now `tests-failing`.
  - **Lead decisions:**
    - D4: `main` honours an `XTASK_ROOT` environment variable that overrides the repository root, so the binary can be tested against a temporary tree.
    - D5: a malformed verdict line is reported once for each row that cites the record.
    - D6: a duplicated bad area code is reported once per area entry.
    - D7: only `char::is_control` characters are escaped.
    - Accepted the test engineer's assumptions: no id-shape check on verdict lines; fenced lines, including `~~~` fences, are never verdict lines or malformed; matching is case-sensitive; a rejected test path still gets its name check.
  - **Process note:** the test engineer wrote a reference implementation in the session scratchpad, outside the repository, to check its tests. No repository file outside `xtask/tests` changed.
- 2026-11-08 implementation: all 38 tests pass. A local cargo-mutants run on the diff: 34 mutants, 33 caught, 0 missed, 1 unviable (`verdict_lines` -> `vec![Default::default()]`, because `VerdictLine` has no `Default`). Rows are now `implemented`.
