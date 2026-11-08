# The functionality matrix

[`functionality-matrix.toml`](functionality-matrix.toml) is the canonical record of
what libSACM implements, one row per requirement, and the work queue for the
engineering loop. [`functionality-matrix.md`](functionality-matrix.md) is generated
from it — read that one; edit the TOML.

```sh
cargo xtask matrix render   # regenerate the .md after editing the .toml
cargo xtask matrix check    # the gate (CI runs it)
cargo xtask matrix next     # open rows whose dependencies are all verified
cargo xtask matrix status   # per-milestone progress
```

## Why it is gated

Users read the matrix to decide whether to trust this library with a safety argument.
A row that claims more than its tests prove misleads them; a row that claims less hides
finished work. Both happened to the predecessor project's capability matrix, so here
the claims are checked mechanically wherever they can be, and audited by the
`matrix-steward` agent where they cannot. See [ADR 0004](../adr/0004-functionality-matrix-gate.md).

## Rows

```toml
[[row]]
id = "SACM23-ARG-007"            # <prefix>-<AREA>-<NNN>, never reused
area = "ARG"                      # a declared [[area]] whose prefix matches the id
milestone = "M2"                  # a declared [[milestone]]
title = "AssertedRelationship common features"
requirement = "…testable statement of what must be true…"
source = "formal/23-05-08 §11.13; ptc/22-03-13 Argumentation::AssertedRelationship"
kind = "model"                    # model | xmi | validation | editing | api | security |
                                  # cli | docs | quality | tooling | compliance
status = "planned"
depends_on = ["SACM23-ARG-002"]   # rows that must be verified first
acceptance = ["…", "…"]           # testable criteria (from research)
code = ["crates/sacm/src/…"]      # implementation paths
tests = ["crates/sacm/tests/x.rs::sacm23_arg_007_…"]
verification = "docs/verification/NNNN-….md"
notes = "…"
```

- **`SACM23-*`** rows are obligations of the SACM 2.3 standard. Their `source` cites the
  clause (formal/23-05-08) and/or the model element (ptc/22-03-13).
- **`LIB-*`** rows are library capabilities and quality commitments the standard does not
  state (editing, diagnostics, CLI, docs, CI). Their `source` cites the decision or ADR.

## Statuses

| Status | Meaning | Required evidence | Forbidden |
|---|---|---|---|
| `planned` | Scheduled, not analyzed. | — | tests, code, verification |
| `analyzed` | Normative text quoted, acceptance criteria written. | acceptance | tests, code, verification |
| `tests-failing` | Failing tests encode the criteria. | acceptance, tests | verification |
| `implemented` | Tests pass; not yet independently verified. | acceptance, tests, code | verification |
| `verified` | The verifier passed it; recorded. | acceptance, tests, code, verification record that names the row; every dependency verified | — |
| `candidate` | Under consideration, not scheduled. | notes (why) | tests, code, verification |
| `deferred` | Scheduled for later than its milestone suggests. | notes (why) | tests, verification |
| `out-of-scope` | Deliberately not done. Recorded so it is not re-proposed. | notes (why) | tests, verification |

Only the lead (the main session running the `sacm-slice` skill, or a human maintainer)
changes a status, and only to `verified` on a recorded PASS from the verifier.

## Gate rules (`cargo xtask matrix check`)

1. `schema_version` is 1; milestone ids and area codes are unique.
2. Row ids are unique and well formed; the id's area segment equals `area`, and its
   prefix equals the area's declared prefix.
3. `title`, `requirement` and `source` are non-empty; `milestone` exists.
4. The evidence rules in the status table above hold.
5. Every cited path (code, test file, doctest file, verification record) is a relative,
   `/`-separated path inside the repository: it has no `\` or `:`, does not start with
   `/`, and has no empty, `.` or `..` component. This applies at every status, so it
   includes a `verification` path on a row that is not verified. A rejected path is never
   read. Every cited code path and verification record exists. The record of a verified
   row has exactly one verdict line for that row, saying PASS:
   `- Row verdict: <ROW-ID>: PASS`, with no indent and outside fenced code blocks.
   Fences are recognised at the top level of the record, as CommonMark defines them:
   0 to 3 spaces of indent, then 3 or more backticks or tildes, closed by a run of the
   same character that is at least as long. Two cases are not yet handled. A fence
   inside a list item, which CommonMark closes when the item ends, is LIB-TOOL-005,
   still open. HTML blocks and a backtick in a fence's info string are LIB-TOOL-015. A line
   starting `- Row verdict:` that is not well formed is a problem.
6. Every test citation is `path::function`, where the function is a **test** in that file,
   or `doctest:path` for a file containing a code block. A test is named
   `<row id in snake case>` or `<row id in snake case>_<anything>` (Python tests add
   unittest's `test_` prefix). What counts as a test:
   - **Rust:** a function under a test-style attribute, one whose path ends in the
     segment `test`, `test_case` or `rstest` (`#[test]`, `#[tokio::test]`,
     `#[test_case(..)]`, `#[rstest]`). The attribute is on a line before the function or
     on the same line. Other attributes, blank lines and comments may sit between, and an
     attribute may span lines. `#[cfg(test)]`, `#[mytest]` and `#[testing]` mark nothing.
   - **Python:** a line-leading `def test_…` or `async def test_…`, at any indent.
   - Text inside a comment or a literal never counts. In Rust, that is line, doc and
     block comments (block comments nest), strings, raw strings with any number of `#`,
     and char and byte literals. In Python, it is `#` comments and strings in single,
     double or triple quotes, with any prefix. A helper does not count either.
   - An ignored or skipped test is still a test, so citing it raises no problem for the
     row. Rule 7 reports it instead.
7. The scan covers `crates/`, `xtask/` and `.claude/hooks/`. A traced test is a test, as
   above, whose name starts with a row id. Each traced test it finds has at most one
   problem, the first of these that applies:
   1. **It is in a file with a path component `src`.** Traced tests belong in test files,
      outside the implementer's scope. Untraced unit tests and doctests under `src` are
      allowed.
   2. **It is ignored or skipped, so it is not evidence.** In Rust that is `#[ignore]`,
      `#[ignore = "…"]`, or a `#[cfg_attr(<predicate>, …)]` that lists `ignore` after the
      predicate. In Python it is a decorator whose dotted name ends in `skip`, `skipIf`,
      `skipUnless`, `skipif` or `expectedFailure`, either on the test or on a class it is
      indented under.
   3. **No row cites it.** Either the row is missing it or the test is stale.

   These problems are sorted by file and then by function name. The scan cannot see tests
   that a macro generates, or Python tests skipped at run time (`self.skipTest(...)`,
   `pytest.skip()`).
8. `depends_on` targets exist, are not the row itself, are not `candidate` or
   `out-of-scope`, are not in a later milestone, and contain no cycles.
9. `functionality-matrix.md` equals `cargo xtask matrix render` output.
10. Every area code is one or more uppercase ASCII letters.
11. A failing check reports every problem, one per line, in a deterministic order. An
    error from reading or parsing the matrix is one line too: `read <path>: <error>`, or
    `docs/matrix/functionality-matrix.toml:<line>:<column>: <message>`. The line and
    column are 1-based, and the column is counted in characters. Control characters in
    a problem or an error are escaped. Under `XTASK_ROOT`, the first line of the report
    (the OK line, the problem count or the load error) ends with ` (root: <root>)`. An
    `XTASK_ROOT` that is set but empty is an error, and no command runs.

What the gate cannot check — whether the tests actually prove the requirement, whether
rows are missing, whether a planned row quietly landed — is the matrix steward's audit.

## Milestones and ordering

Milestones are build order (M0 → M7). `matrix next` lists open rows whose dependencies
are all verified, ordered by milestone then by position in the file. Within a
milestone, keep rows in a sensible build order; the loop takes them from the top.

## Adding or changing rows

- New obligations discovered during research get the next free number in their area.
- Never renumber or reuse an id: tests and verification records embed them.
- Splitting a row: keep the original id for the part already in progress, add new ids
  for the rest, and move dependencies accordingly.
- A row whose requirement changes after it was verified goes back to `analyzed`, with a
  note saying what changed.
