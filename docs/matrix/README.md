# The functionality matrix

[`functionality-matrix.toml`](functionality-matrix.toml) is the record of what libSACM
implements: one row per requirement. It is also the loop's work queue.
[`functionality-matrix.md`](functionality-matrix.md) is generated from it: read the `.md`,
edit the `.toml`.

```sh
cargo xtask matrix render   # regenerate the .md after editing the .toml
cargo xtask matrix check    # the gate (CI runs it)
cargo xtask matrix next     # open rows whose dependencies are all verified
cargo xtask matrix status   # progress per milestone
```

**Why it is gated:** users read the matrix to decide whether to trust the library. A row
that claims more than its tests prove misleads them, so the claims are checked by machine
where possible ([ADR 0004](../adr/0004-functionality-matrix-gate.md)).

## Rows

```toml
[[row]]
id = "SACM23-ARG-007"            # <prefix>-<AREA>-<NNN>, never reused
area = "ARG"
milestone = "M2"
title = "AssertedRelationship common features"
requirement = "…what must be true, testably…"
source = "formal/23-05-08 §11.13; ptc/22-03-13 Argumentation::AssertedRelationship"
kind = "model"
status = "planned"
depends_on = ["SACM23-ARG-002"]
acceptance = ["…"]
code = ["crates/sacm/src/…"]
tests = ["crates/sacm/tests/x.rs::sacm23_arg_007_…"]
verification = "docs/verification/NNNN-….md"
notes = "…"
```

- **`SACM23-*` rows** are obligations of the SACM 2.3 standard. Their `source` cites the
  clause or the model element.
- **`LIB-*` rows** are library capabilities and quality commitments the standard does not
  state. Their `source` cites the decision or ADR.

## Statuses

| Status | Meaning | Required | Forbidden |
|---|---|---|---|
| `planned` | Scheduled, not analyzed. | — | tests, code, verification |
| `analyzed` | Acceptance criteria written. | acceptance | tests, code, verification |
| `tests-failing` | Failing tests encode the criteria. | acceptance, tests | verification |
| `implemented` | Tests pass, not yet verified. | acceptance, tests, code | verification |
| `verified` | The verifier passed it. | acceptance, tests, code, a record with its PASS line, all dependencies verified | — |
| `candidate` | Under consideration. | notes | tests, code, verification |
| `deferred` | Later than its milestone suggests. | notes | tests, verification |
| `out-of-scope` | Deliberately not done. | notes | tests, verification |

Only the lead changes a status, and a status becomes `verified` only on a recorded PASS.

## Gate rules (`cargo xtask matrix check`)

The edge cases of each rule are pinned by tests in `xtask/tests/`.

1. `schema_version` is 1. Milestone IDs and area codes are unique.
2. Row IDs are unique and well formed, and their area segment and prefix match the row's
   `area`.
3. `title`, `requirement` and `source` are non-empty, and the `milestone` exists.
4. The status table holds.
5. **Cited paths and verification records.**
   - Every cited path is relative and `/`-separated, inside the repository, with no `\`,
     `:`, `.` or `..` components.
   - Cited code and records exist.
   - A verified row's record holds exactly one line `- Row verdict: <ROW-ID>: PASS`.
   - Records are strict Markdown ([ADR 0008](../adr/0008-markdown-parser-for-verdict-lines.md)).
     Lines inside code blocks or HTML comments do not count. Other raw HTML (except
     `<br>`, `<ul>` and `<li>` inside a line), lone carriage returns, a byte-order mark,
     and verdict-like lines not at column 0 are problems.
6. **Test citations** have the form `path::function`, naming a test in a scanned test file
   (rule 7), or `doctest:path`.
   - A test is a Rust function under a test attribute (`#[test]`, `#[tokio::test]`,
     `#[test_case]`, `#[rstest]`), or a Python `def test_…`.
   - It is named after its row (`lib_tool_001_…`, or `test_lib_tool_001_…`).
   - Text inside comments and literals never counts.
7. **The scan** covers `.rs` and `.py` files under `crates/`, `xtask/` and `.claude/hooks/`,
   except build output. A traced test (its name starts with a row ID) is a problem if:
   - it sits under `src`;
   - it is ignored or skipped;
   - or no row cites it.
8. **Dependencies** (`depends_on`) exist, are not the row itself, are not `candidate` or
   `out-of-scope`, are not in a later milestone, and form no cycle.
9. **`functionality-matrix.md`** equals the `render` output.
10. **Area codes** are uppercase ASCII letters.
11. **Reporting.** A failing check reports every problem, one per line, in a fixed order.
    A load error is one line, with its line and column.

What the gate cannot check, such as whether tests really prove a requirement, is the
`matrix-steward` agent's audit.

## Milestones and changes

- **Milestones are the build order,** M0 to M7. `matrix next` lists ready rows by
  milestone, then by their position in the file.
- **IDs:** never renumber or reuse an ID. A new obligation gets the next free number in
  its area.
- **Changed requirements:** a verified row whose requirement changes goes back to
  `analyzed`, with a note.
