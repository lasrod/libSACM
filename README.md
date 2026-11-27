# libSACM

A Rust library that implements **OMG SACM 2.3**, the [Structured Assurance Case
Metamodel](https://www.omg.org/spec/SACM/2.3): it reads, validates, edits and writes
assurance cases as standard XMI.

[![CI](https://github.com/lasrod/libSACM/actions/workflows/ci.yml/badge.svg)](https://github.com/lasrod/libSACM/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
![MSRV 1.85](https://img.shields.io/badge/MSRV-1.85-orange.svg)

## What SACM is

An *assurance case* is a structured argument, backed by evidence, that a system is
acceptably safe or secure, as standards such as ISO 26262 and UL 4600 require. SACM is
the OMG standard format for assurance cases, so that tools can exchange them.

## What libSACM does

- **All four package kinds.** `AssuranceCasePackage`, `ArgumentPackage`, `ArtifactPackage`
  and `TerminologyPackage`, with their elements, interfaces and bindings.
- **Reads and writes XMI.** Strict SACM 2.3 by default. A tolerant read opens files from
  other tools: the dialect EMF writes, and older SACM revisions.
- **Loses nothing.** Content that is not SACM 2.3 is kept as written and reported. A
  strict write refuses it; nothing is dropped silently.
- **Validates.** The standard's rules, reported with stable diagnostic codes.
- **Edits atomically.** Create, change, move, copy and delete; a refused edit changes nothing.
- **Compares.** Every difference in SACM content between two documents.
- **Stays neutral and safe.** No notation- or tool-specific concepts, no `unsafe` code,
  and malformed input is diagnosed, never a panic.

It claims the standard's four model compliance points: Assurance Case, Argumentation,
Artifact and Terminology. The [conformance statement](docs/sacm/compliance-points.md) says
what that covers and how it is checked.

## Status

| | |
|---|---|
| Released | 0.4.0, on 2026-11-26. The API is pre-1.0 and not stable yet. |
| Done | Milestones M0 to M6: the foundation through interoperability and the compliance claims. |
| In progress | M7, the 1.0 candidate, to be released as 0.5.0. |
| Planned in M7 | A user guide, a SACM primer, runnable examples, property-based tests, fuzzing, coverage and semver checks. |

The [functionality matrix](docs/matrix/functionality-matrix.md) shows what is verified,
requirement by requirement; nothing else is claimed.

## Use it

```toml
[dependencies]
sacm = "0.4"
```

The [crate documentation](https://docs.rs/sacm) has the entry points, a complete example
and the rules. The [`sacm` README](crates/sacm/README.md) has the same example.

## Workspace

| Crate | What it is |
|---|---|
| [`sacm`](crates/sacm) | The library. |
| [`sacm-cli`](crates/sacm-cli) | The `sacm` command-line tool: `version`, `validate`, `roundtrip` and `convert`. |
| `xtask` | Repository tooling (`cargo xtask ...`); not published. |

## Build and test

Minimum supported Rust version: **1.85** (`rust-version` in `Cargo.toml`).

```sh
cargo build --workspace
cargo test --workspace --all-features
cargo xtask matrix check     # the matrix gate
cargo xtask fetch-spec       # download the OMG SACM 2.3 and XMI 2.5.1 references (not redistributed)
```

## How it is built

Work is done in small slices, each tied to rows of the functionality matrix.

1. **Tests first.** They define what the slice must do.
2. **Code second.** It makes them pass.
3. **Independent verification.** A row counts as verified only when a separate verifier
   passes it. Mutation testing checks that the tests catch real faults.

Agents with fixed roles do most of the work; the maintainer reviews and merges. See
[docs/loop.md](docs/loop.md).

## Links

- [docs/README.md](docs/README.md): a map of the documentation.
- [Architecture](docs/architecture.md) and [decision records](docs/adr/).
- [Documentation style](docs/style.md).
- [CONTRIBUTING.md](CONTRIBUTING.md) and [SECURITY.md](SECURITY.md).
- libSACM follows the design of the C++ SACM library in
  [assurance-forge](https://github.com/lasrod/assurance-forge), by the same author.

## License

[MIT](LICENSE). SACM is an Object Management Group specification; this project is not
affiliated with or endorsed by OMG.
