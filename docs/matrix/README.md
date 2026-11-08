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
   includes a `verification` path on a row that is not verified. A rejected path is
   never read. Every cited code path and verification record exists. The record of a
   verified row must be strict, and must have exactly one verdict line for that row,
   saying PASS: `- Row verdict: <ROW-ID>: PASS`. Records are strict (ADR 0008 amendment
   1): anything a renderer could show differently is a problem, not something the gate
   models. The gate makes no claim to match what a browser or any renderer shows.
   - **Lines.** Lines end at LF or CRLF and are numbered from 1. A carriage return
     that is not part of a CRLF is a problem at its line, once per line: `lone carriage
     return; use LF or CRLF line endings`. A record that starts with a byte-order mark
     (U+FEFF) is a problem: `starts with a byte-order mark`. The mark is part of line 1,
     so line 1 is then not a candidate.
   - **Hidden lines.** The gate parses the record as core CommonMark 0.31.2 with
     `pulldown-cmark` and no extensions (ADR 0008). First, each tab in the run of
     spaces and tabs that ends a line becomes a space, because pulldown-cmark 0.13 does
     not accept a tab after a closing fence. This keeps every offset and line. A line
     is hidden when it is inside a code block (fenced or indented) or inside an HTML
     block that is one closed comment. Nothing else hides a line.
   - **Raw HTML.** A closed comment starts with `<!--`, ends with `-->`, and holds no
     `-->` or `--!>` before that end, so `<!-->`, `<!--->` and `<!---->` are closed.
     Only three kinds of raw HTML are allowed: an HTML block that is one closed comment
     (trailing whitespace ignored), an inline closed comment, and the inline tags
     `<br>`, `<br/>`, `<br />`, `<ul>`, `</ul>`, `<li>`, `</li>`, in any case, when
     other text comes before them on their line (as in a table cell). Any other raw
     HTML is a problem at each line that holds some: `raw HTML other than a closed
     comment or <br>, <ul>, <li> is not allowed`. A blank line inside an HTML block is
     not. Disallowed HTML hides nothing, so a candidate inside it counts.
   - **Candidates.** A line is a candidate when it starts at column 0 with `- Row
     verdict:`. A hidden candidate does not count and is not malformed. A counted
     candidate is a verdict line when, without its trailing spaces and tabs, it is
     exactly `- Row verdict: <ROW-ID>: <VERDICT>`. Here `<ROW-ID>` has the form of rule
     2 (`SACM23` or `LIB`, an area of uppercase letters, three digits) and `<VERDICT>`
     is PASS, FAIL or OPEN. Any other counted candidate is malformed, a problem at its
     line.
   - **Look-alikes.** A line that is neither hidden nor a candidate is a problem at its
     line, `a verdict-like line is not at column 0`, when it starts with `Row verdict:`
     once its leading spaces, tabs, `>` and list markers are removed. The markers are
     `-`, `*`, `+`, and digits followed by `.` or `)`.
   - **Order.** A record's problems are reported once for each row that cites it: the
     byte-order mark first, then by line (on one line: raw HTML, then lone CR, then
     malformed or verdict-like), then the verdict problem.
6. Every test citation is `path::function`, where the function is a **test** in that file,
   or `doctest:path` for a file containing a code block. A test is named
   `<row id in snake case>` or `<row id in snake case>_<anything>` (Python tests add
   unittest's `test_` prefix). The file of a `path::function` citation must be a scanned
   test file (rule 7). A citation of any other file, whether or not the file exists, is
   the problem `test file <path> is outside the scanned test files`. That problem
   replaces the citation's other checks, except the check of the test's name, so the
   file is not looked up. A `doctest:` citation may name any file, because doctests also
   live in READMEs and other docs. What counts as a test:
   - **Rust:** a function under a test-style attribute, one whose path ends in the
     segment `test`, `test_case` or `rstest` (`#[test]`, `#[tokio::test]`,
     `#[test_case(..)]`, `#[rstest]`). The attribute is on a line before the function or
     on the same line. Other attributes, blank lines and comments may sit between, and an
     attribute may span lines. `#[cfg(test)]`, `#[mytest]` and `#[testing]` mark nothing.
     Paths are read as tokens and judged whole. On one line, whitespace or a comment may
     sit between `#` and `[`, and whitespace or comments may sit around each `::`, also
     across lines. So `# [tokio :: test]` marks a test and `#[test :: helper]` does not.
     Any run of spaces or tabs may follow `fn`. `myfn`, `self.fn` and `r#fn` are not the
     keyword.
   - **Python:** a line-leading `def test_…` or `async def test_…`, at any indent. Any run
     of spaces or tabs may follow `async`, `def` and `class`. A word that only starts with
     one of them (`define`, `classy`) is not the keyword.
   - Text inside a comment or a literal never counts. In Rust, that is line, doc and
     block comments (block comments nest), strings, raw strings with any number of `#`,
     and char and byte literals. In Python, it is `#` comments and strings in single,
     double or triple quotes, with any prefix. A helper does not count either.
   - An ignored or skipped test is still a test, so citing it raises no problem for the
     row. Every cited test is in a scanned test file, so rule 7 reports it instead.
7. The scan covers the scanned test files: the files under `crates/`, `xtask/` and
   `.claude/hooks/` whose extension is `.rs` or `.py`, in any case, except build output.
   Build output is a directory named `target` with a `Cargo.toml` directly beside it, as
   with the workspace's `target/` or a package's `crates/<name>/target/`. Any other
   `target` directory is scanned, such as a module directory `src/target/` or
   `tests/target/`. A traced test is a test, as above, whose name starts with a row id.
   Each traced test the scan finds has at most one problem, the first of these that
   applies:
   1. **It is in a file with a path component `src`.** Traced tests belong in test files,
      outside the implementer's scope. Untraced unit tests and doctests under `src` are
      allowed.
   2. **It is ignored or skipped, so it is not evidence.** In Rust that is `#[ignore]`,
      `#[ignore = "…"]`, or a `#[cfg_attr(<predicate>, …)]` that lists `ignore` after the
      predicate. `ignore` must be the whole path, so `#[ignore :: x]` does not ignore. In
      Python it is a decorator whose dotted name ends in `skip`, `skipIf`, `skipUnless`,
      `skipif` or `expectedFailure`, either on the test or on a class it is indented
      under. The name is read as tokens, so whitespace may follow the `@` and sit around
      each `.` (`@ unittest . skip`). The whole name is judged, so
      `@unittest.skip .helper` does not skip.
   3. **No row cites it.** Either the row is missing it or the test is stale.

   These problems are sorted by file and then by function name. The scan cannot see tests
   that a macro generates, or Python tests skipped at run time (`self.skipTest(...)`,
   `pytest.skip()`). It does not read a Rust attribute after other code on its line. A
   `#` at the end of a line does not open an attribute whose `[` is on the next line, and
   a `fn` whose name is on the next line is not read. A
   `cfg_attr` nested in a `cfg_attr` (`#[cfg_attr(a, cfg_attr(b, ignore))]`) and a
   prelude-qualified `ignore` (`#[::core::prelude::v1::ignore]`,
   `#[std::prelude::v1::ignore]`) do not ignore, so their tests count as running. The
   prelude-qualified `ignore` is LIB-TOOL-016.
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
