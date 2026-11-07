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
| `candidate` | Under consideration, not scheduled. | notes (why) | tests, code |
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
5. Every cited code path and verification record exists; the record mentions the row id.
6. Every test citation is `path::function` with the function defined in that file and
   named `<row id in snake case>` or `<row id in snake case>_<anything>` (Python tests may
   add unittest's `test_` prefix), or `doctest:path` for a file containing a code block.
7. Every traced test in `crates/`, `xtask/` and `.claude/hooks/` — a `#[test]` function, or
   a Python `def test_…`, whose name starts with a row id — is cited by some row. An
   uncited traced test is a finding: either the row is missing it or the test is stale.
8. `depends_on` targets exist, are not the row itself, are not `candidate` or
   `out-of-scope`, are not in a later milestone, and contain no cycles.
9. `functionality-matrix.md` equals `cargo xtask matrix render` output.

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
