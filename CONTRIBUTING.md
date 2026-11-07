# Contributing to libSACM

Thank you for helping. libSACM is used to read and write other people's safety and
security arguments, so the bar is correctness first, then clarity.

## Before you start

- For anything beyond a small fix, open an issue first so we can agree on the approach.
- Read [docs/architecture.md](docs/architecture.md), the [ADRs](docs/adr/) and the
  [SACM decisions](docs/sacm/decisions.md). Settled interpretations of the standard are
  not reopened in pull requests; raise them in an issue.
- Find or propose the [functionality-matrix](docs/matrix/README.md) row your change
  serves. Every behaviour change traces to a row.

## Development setup

```sh
rustup show                     # installs the toolchain from rust-toolchain.toml
cargo xtask fetch-spec          # downloads the OMG SACM 2.3 references (hash-pinned)
cargo test --workspace
```

Optional: `cargo install cargo-deny` for the dependency gate; Python 3 for the agent
hook tests.

## The gate

Every pull request must pass what CI runs:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test -p xtask
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --all-features
cargo xtask matrix check
cargo deny check
```

## Conventions

- **Tests first**, named after the matrix row they prove:
  `sacm23_arg_007_asserted_relationship_requires_one_target`. The matrix gate rejects
  traced tests that no row cites.
- **Public API**: follow the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
  and `.claude/skills/rust-library-craft/SKILL.md` (it is written for agents, but it is
  the project's style guide for everyone).
- **Documentation**: every public item documented; examples are doctests.
- **No `unsafe`**, no panics reachable from input, deterministic output.
- **SACM terminology** in the API and docs; no GSN terms for SACM concepts.
- **Commits**: [Conventional Commits](https://www.conventionalcommits.org/), with matrix
  row ids in the subject where applicable: `feat(arg): asserted relationships [SACM23-ARG-007]`.
- **CHANGELOG**: add user-visible changes under `[Unreleased]`.

## Minimum supported Rust version

1.85 ([ADR 0007](docs/adr/0007-msrv-and-edition.md)). Raising it is a minor-version
change and needs a reason.

## AI-assisted development

Much of this repository is developed by Claude Code agents running the
[engineering loop](docs/loop.md). Human contributions follow the same rules, and
agent contributions get the same review.

## License

By contributing you agree that your contributions are licensed under the
[MIT License](LICENSE).
