# Contributing to libSACM

libSACM reads and writes other people's safety and security arguments, so correctness
comes first, then clarity.

## Before you start

- **Discuss larger changes first.** For anything beyond a small fix, open an issue.
- **Read the design:** [docs/architecture.md](docs/architecture.md), the
  [ADRs](docs/adr/) and the [SACM decisions](docs/sacm/decisions.md). Settled
  interpretations of the standard are not reopened in pull requests.
- **Every behaviour change traces to a row** of the
  [functionality matrix](docs/matrix/README.md).

## Setup

```sh
rustup show                     # installs the toolchain from rust-toolchain.toml
cargo xtask fetch-spec          # downloads the OMG SACM 2.3 and XMI 2.5.1 references (hash-pinned)
cargo test --workspace
```

## The gate

CI runs this on every pull request, and `main` requires it:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test -p xtask
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --all-features
cargo xtask matrix check
cargo deny check
python -m unittest discover -s .claude/hooks -p "test_*.py"
```

## Mutation testing

CI runs [cargo-mutants](https://mutants.rs) on each pull request's diff. A weekly full run
covers the whole workspace. A mutant that no test catches fails the build.

To run it locally, install both tools:

```sh
cargo install cargo-mutants --locked --version 27.1.0
cargo install cargo-nextest --locked --version 0.9.151
```

Then run the whole workspace, or only your change:

```sh
cargo mutants --workspace
git diff origin/main...HEAD > mutants.diff
cargo mutants --workspace --in-diff mutants.diff
```

- **nextest:** tests run under nextest, and its `mutants` profile stops at the first
  failure ([ADR 0009](docs/adr/0009-nextest-for-mutation-testing.md)).
- **Doctests:** they do not run under mutation, so kill a mutant with a unit or
  integration test.
- **The authoritative run is Linux.** Some sacm-cli tests are Linux-only.
- **Results** go to the git-ignored `mutants.out/`.
- **Exclusions:** every exclusion in `.cargo/mutants.toml` needs a row in
  [docs/quality/mutants-triage.md](docs/quality/mutants-triage.md).

## Conventions

- **Tests come first,** and each is named after the matrix row it proves:
  `sacm23_arg_007_asserted_relationship_requires_one_target`.
- **Public API** follows the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
  and `.claude/skills/rust-library-craft/SKILL.md`.
- **Every public item is documented,** and the examples are doctests.
- **Code:** no `unsafe`, no panic on input, deterministic output.
- **Terms:** SACM terms only; no GSN vocabulary for SACM concepts.
- **Commits** follow [Conventional Commits](https://www.conventionalcommits.org/), with
  row IDs: `feat(arg): asserted relationships [SACM23-ARG-007]`.
- **Third-party GitHub Actions** are pinned by commit SHA with a `# <ref>` comment. To
  bump one, read its `action.yml` at the new SHA first.
- **CHANGELOG:** user-visible changes go under `[Unreleased]`.

## Minimum supported Rust version

1.85 ([ADR 0007](docs/adr/0007-msrv-and-edition.md)). Raising it is a minor-version
change.

## AI-assisted development

Claude Code agents develop most of this repository, through the
[engineering loop](docs/loop.md). Human and agent contributions follow the same rules and
get the same review.

## License

Contributions are licensed under the [MIT License](LICENSE).
