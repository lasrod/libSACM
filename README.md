# libSACM

A Rust library for **OMG SACM 2.3**, the [Structured Assurance Case
Metamodel](https://www.omg.org/spec/SACM/2.3). The goal: read, validate, edit and write
assurance cases as standard XMI.

[![CI](https://github.com/lasrod/libSACM/actions/workflows/ci.yml/badge.svg)](https://github.com/lasrod/libSACM/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
![MSRV 1.85](https://img.shields.io/badge/MSRV-1.85-orange.svg)

## Status

Milestones M0 (foundation), M1 (a minimal editable vertical slice) and M2 (base model and
argumentation) are done, M3's artifact model is complete, and the first release, 0.1.0,
was published on 2026-11-17. Today `sacm::xmi::read` reads `AssuranceCasePackage`, `ArgumentPackage`,
`ArtifactPackage` and `TerminologyPackage` roots, the interfaces and bindings of every
package, as roots and children, and, in them, nested packages, the
argumentation elements (Claim, ArgumentReasoning, ArtifactReference, ArgumentGroup and the
five asserted relationships), the artifact elements Property, Technique, Participant,
ArtifactGroup, Artifact, Event, Resource, Activity and ArtifactAssetRelationship, and the
terminology elements Category, Term, Expression and TerminologyGroup, each with its name,
`gid`, flags, references, descriptions, notes, implementation constraints and tagged
values. A name, or a value of a content, key or location, may be an
`ExpressionLangString` (§8.4) that links an ExpressionElement. Content that is not SACM 2.3 is kept as preserved content (SACM-XMI-003). A
tolerant read, the default, opens
every well-formed file with SACM 2.3 package roots: what a strict read refuses, such as a
value that is no literal or conforming content not read yet (such as XMI metadata),
it keeps as preserved content with a Warning (SACM-XMI-014 for content not read
yet); and it reads the names formal/23-05-08 and ptc/22-03-13 spell differently
(`ArtifactAssertedRelationship`, an Event's `occurece`, `argumentationElement` and
`TerminologyElement`) as the names it writes, with a Warning (SACM-XMI-013). Nothing is dropped.

`sacm::xmi::write` writes the same kinds deterministically; a strict save, the default,
checks only that the output is schema-valid, so a draft saves, and an application calls
`validate` for model checks. `sacm::edit::apply` creates, edits, moves, copies and
deletes them atomically, `sacm::validation::validate` checks names, ids, gids, contents,
references, terms' values, ExpressionLangStrings, concrete Expressions and the
argumentation and artifact rules, and
`sacm::compare::semantic_compare` lists differences. Undo and redo are the application's, by whole-document copies, as the `edit`
docs show. The interfaces and bindings of every package are read, written, edited and validated, and a reference to an element of another file (an `href` such as `other.sacm.xmi#c1`) is read, written back and counted, but never resolved. The [functionality matrix](docs/matrix/functionality-matrix.md) shows
what is verified; nothing else is claimed.

## What SACM is

An *assurance case* is a structured argument, backed by evidence, that a system is
acceptably safe or secure, as standards such as ISO 26262 and UL 4600 require. SACM is
the OMG standard format for assurance cases, so that tools can exchange them.

libSACM aims to be a typed model of the whole metamodel that loses nothing on load and
save, validates the standard's constraints with stable diagnostic codes, and makes every
edit atomic. It has no `unsafe` code, no panics on input, and no notation- or
tool-specific concepts.

## How it is built

Work is done in small slices, each tied to rows of the
[functionality matrix](docs/matrix/functionality-matrix.md). For each slice the tests
are written first, the code makes them pass, and an independent verifier must pass the
slice before a row counts as verified. Mutation testing checks that the tests catch real
faults. Agents with fixed roles do most of the work; the maintainer reviews and merges.
See [docs/loop.md](docs/loop.md).

## Build and test

Minimum supported Rust version: **1.85** (`rust-version` in `Cargo.toml`).

```sh
cargo build --workspace
cargo test --workspace --all-features
cargo xtask matrix check     # the matrix gate
cargo xtask fetch-spec       # download the OMG SACM 2.3 and XMI 2.5.1 references (not redistributed)
```

## Workspace

| Crate | What it is |
|---|---|
| [`sacm`](crates/sacm) | The library. |
| [`sacm-cli`](crates/sacm-cli) | The `sacm` command-line tool: `version`, `validate` and `roundtrip`. |
| `xtask` | Repository tooling (`cargo xtask ...`); not published. |

## Links

- [docs/README.md](docs/README.md): a map of the documentation.
- [Architecture](docs/architecture.md) and [decision records](docs/adr/).
- [CONTRIBUTING.md](CONTRIBUTING.md) and [SECURITY.md](SECURITY.md).
- libSACM follows the design of the C++ SACM library in
  [assurance-forge](https://github.com/lasrod/assurance-forge), by the same author.

## License

[MIT](LICENSE). SACM is an Object Management Group specification; this project is not
affiliated with or endorsed by OMG.
