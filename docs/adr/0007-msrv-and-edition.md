# 0007. Rust edition 2024, MSRV 1.85

- Status: accepted
- Date: 2026-11-07
- Matrix rows: LIB-QA-002

## Context

Safety-tooling users often build on older, qualified toolchains. A low minimum
supported Rust version widens adoption; a very low one forbids useful language
features and many dependencies.

## Decision

Edition 2024, `rust-version = "1.85"` (the first release supporting edition 2024).
CI builds and tests the library and CLI on 1.85. Development uses current stable
(`rust-toolchain.toml`). Raising the MSRV is a minor-version change, announced in the
CHANGELOG, and done only for a concrete benefit.

## Consequences

No let-chains (stable in 1.88) and no dependency requiring a newer compiler. `xtask` is
not part of the MSRV promise.
