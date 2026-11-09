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

## Mutation testing

CI runs [cargo-mutants](https://mutants.rs) on every pull request, over the PR's diff. A weekly sharded run, which also runs on PRs that touch Rust code or the mutation config, covers the whole workspace. A mutant that no test catches fails the build. Install the pinned version with:

```sh
cargo install cargo-mutants --locked --version 27.1.0
```

Run the whole workspace with `cargo mutants --workspace`. `--workspace` is required: without it, only the default members are mutated and xtask is skipped. To check just your change, mutate only the diff against `main`:

```sh
git diff origin/main...HEAD > mutants.diff
cargo mutants --workspace --in-diff mutants.diff
```

Results go to `mutants.out/`, which is git-ignored. Configuration lives in `.cargo/mutants.toml`. Every exclusion there must be listed, with its reason and owning matrix row, in [docs/quality/mutants-triage.md](docs/quality/mutants-triage.md).

Each excluded mutant needs its own triage row, naming the entry that excludes it. To check that every `exclude_re`, `exclude_globs` and `skip_calls` entry excludes exactly the mutants its triage rows name, run:

```sh
cargo xtask mutants exclusions
```

The command compares `cargo mutants --workspace --list` with the configuration, with `--no-config`, and with each entry removed in turn. Its copies of the configuration go to `target/xtask-mutants/`. The CI job `mutants-exclusions` runs it on every pull request. It needs cargo-mutants installed, and a missing cargo-mutants is an error, not a pass. Mutant names include their line and column, so a change that moves an excluded mutant needs its triage row updated.

The authoritative run is on Linux CI. Some tests only run on Linux (for example, the sacm-cli closed-pipe tests), so on Windows and macOS a few sacm-cli mutants show as missed even though Linux catches them.

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
