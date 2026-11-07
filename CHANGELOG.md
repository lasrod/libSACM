# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html). Before 1.0, a breaking
change bumps the minor version and is listed under **Breaking**.

## [Unreleased]

### Added

- Cargo workspace: `sacm` library, `sacm-cli` (`sacm version`), `xtask` automation.
- `sacm::VERSION` and `sacm::SACM_VERSION`.
- Functionality matrix with its gate (`cargo xtask matrix check|render|next|status`).
- Hash-pinned fetching of the OMG SACM 2.3 references (`cargo xtask fetch-spec`).
- CI: rustfmt, clippy, tests on Linux/Windows/macOS, MSRV 1.85, rustdoc, cargo-deny,
  matrix gate, agent-hook tests.
- Agent roles, skills and write-scope hook for the engineering loop.
- Mutation testing with cargo-mutants: a PR job mutates the diff, and a weekly sharded
  full run also runs on every Rust PR. Configuration is in `.cargo/mutants.toml`, and
  every survivor's disposition is in `docs/quality/mutants-triage.md`.
- A stricter matrix gate. A verified row needs exactly one `- Row verdict: <ROW-ID>: PASS`
  line in its verification record. Cited paths must be relative paths inside the
  repository, and area codes must be uppercase ASCII letters. A failing
  `matrix check` returns every problem in its error, one per line, in a deterministic
  order. `XTASK_ROOT` points the `xtask` binary at another tree.
