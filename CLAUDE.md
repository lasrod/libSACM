# libSACM — instructions for Claude Code

A public Rust library (MIT) implementing **OMG SACM 2.3**, the Structured Assurance
Case Metamodel: typed model, XMI import/export, validation, editing. Based on the C++
SACM library in [assurance-forge](https://github.com/lasrod/assurance-forge) (same
author), re-implemented as an independent crate. Quality matters: people will build
safety tooling on this.

## Start here

- What is built and what is next: `cargo xtask matrix status`, `cargo xtask matrix next`,
  [docs/matrix/functionality-matrix.md](docs/matrix/functionality-matrix.md).
- How work is done: [docs/loop.md](docs/loop.md). Run a slice with `/sacm-slice`;
  run the loop with `/loop /sacm-slice`.
- Design: [docs/architecture.md](docs/architecture.md), [docs/adr/](docs/adr/).
- The standard: load the `sacm-expert` skill. Normative sources: `cargo xtask fetch-spec`
  → `third_party/sacm-2.3/` (never commit them).
- Rust conventions: load the `rust-library-craft` skill.

## Layout

```text
crates/sacm/          library crate `sacm`
crates/sacm-cli/      `sacm` binary
xtask/                cargo xtask: matrix gate, fetch-spec
docs/matrix/          functionality matrix (edit the .toml, then `cargo xtask matrix render`)
docs/sacm/            decisions, diagnostics catalog, metamodel inventory, spec defects
docs/adr/ docs/slices/ docs/verification/
.claude/agents/       the seven roles     .claude/skills/   sacm-expert, rust-library-craft, sacm-slice
.claude/hooks/        agent write-scope guard (write-scopes.json)
```

## Non-negotiables

1. **Every behaviour traces to a matrix row**, and every test is named after its row
   (`sacm23_arg_007_...`, `lib_edit_004_...`). The matrix gate enforces both directions.
2. **Tests before implementation.** A row is `verified` only on a recorded PASS from
   `sacm-verifier`. Only the lead (main session) edits matrix statuses.
3. **Never drop conforming SACM data.** Preserve with a diagnostic, or refuse — never
   discard silently. Strict and tolerant behaviour stay distinct.
4. **SACM terms only** in the API and docs. No GSN vocabulary for SACM concepts; no
   layout.
5. **No `unsafe`, no panics on input, deterministic output.**
6. **Settled decisions are policy**: [docs/sacm/decisions.md](docs/sacm/decisions.md).
   Quote the standard; do not recall it.
7. **Agent roles keep their boundaries** — the write-scope hook enforces Write/Edit;
   no role writes outside its scope through Bash either.

## Gate (run before claiming anything is done)

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features && cargo test -p xtask
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --all-features
cargo xtask matrix check
cargo deny check
python -m unittest discover -s .claude/hooks -p "test_*.py"
```

## Git

Conventional commits with row ids: `feat(arg): ... [SACM23-ARG-007]`. Do not push,
open PRs or publish crates unless the maintainer has authorized it for the task at hand.
