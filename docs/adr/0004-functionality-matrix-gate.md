# 0004. The functionality matrix is a gated TOML artifact

- Status: accepted
- Date: 2026-11-07
- Matrix rows: LIB-TOOL-001

## Context

The matrix does two jobs: it tells users what the library can be trusted to do, and it
tells the engineering loop what to do next. A matrix nobody can check drifts into
fiction — the assurance-forge capability matrix once called shipped features
"planned", and the reverse failure (claiming what is not tested) is worse for a
safety-case library. Agents edit the matrix constantly, so its format must be easy to
edit correctly and impossible to edit inconsistently without noticing.

## Decision

- The canonical matrix is `docs/matrix/functionality-matrix.toml`, one `[[row]]` per
  requirement. `docs/matrix/functionality-matrix.md` is generated from it by
  `cargo xtask matrix render` and never edited by hand.
- `cargo xtask matrix check` runs in CI and enforces the rules in
  `docs/matrix/README.md`: ID form, status vocabulary, the evidence each status
  requires, resolvable citations, traced test names, no orphaned traced tests,
  dependency resolution/acyclicity/ordering, verification records, rendered page in
  sync.
- `cargo xtask matrix next` lists the open rows whose dependencies are all verified,
  in milestone order. This is how the loop picks work.
- One matrix for both standard obligations (`SACM23-*`) and library capabilities
  (`LIB-*`).

## Consequences

Status changes are mechanical to check and easy to review in diffs. Test names are
long, because they carry their row ID. A test cannot exist without a row and a row
cannot claim a test that does not exist.

## Alternatives considered

- **Markdown table as source** (assurance-forge's approach): readable, but wide rows
  are hard for agents to edit without breaking the table, and parsing it is fragile.
- **Separate conformance and capability matrices**: two sources of truth that drift.
