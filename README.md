# libSACM

A Rust library for **OMG SACM 2.3**, the [Structured Assurance Case
Metamodel](https://www.omg.org/spec/SACM/2.3). The goal: read, validate, edit and write
assurance cases as standard XMI.

[![CI](https://github.com/lasrod/libSACM/actions/workflows/ci.yml/badge.svg)](https://github.com/lasrod/libSACM/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
![MSRV 1.85](https://img.shields.io/badge/MSRV-1.85-orange.svg)

## Status

Milestone M1, a minimal editable vertical slice, is done: `sacm::model` provides a
`Document`, read through borrowing views and changed with `sacm::edit::apply`,
`ElementId`, the metamodel's element kinds, element names, each Assertion's
`assertionDeclaration` and the first value types (`LangString`, `MultiLangString`,
`AssertionDeclaration`), and `sacm::diagnostic` provides the `Diagnostic` type.
`sacm::xmi::read` reads `AssuranceCasePackage` and `ArgumentPackage` roots, the
AssuranceCasePackages, ArgumentPackages and Claims in them, their names and each
Claim's `assertionDeclaration`. It refuses unsafe and malformed input and roots
that are not SACM packages with a diagnostic code, and content it cannot represent yet
as `Unsupported`; it reports a missing or padded `xmi:id`, an `xsi:type` or an
`xmi:version` as a diagnostic. Reading and writing take a strict or tolerant `sacm::xmi::Mode`.
`sacm::xmi::write` writes a document, its names, `gid`s, contents, keys and declarations
deterministically; in strict mode, the default, it refuses a document that validation
reports an Error for. A later release may add validation rules, so a strict write may
then refuse a document it wrote before.
`sacm::validation::validate` reports each `ModelElement` with no name and each id that is
not an XML `NCName`.
`sacm::edit::apply` creates AssuranceCasePackage, ArgumentPackage and Claim elements,
each with a name and a given or generated id, or refuses and changes nothing; what it
builds writes and reads back.

Milestone M2 has begun: every element's `gid`, its `isCitation` and `isAbstract` flags,
its `citedElement` and `abstractForm` references (held as ids, kept even when they name no
element), and the descriptions, notes, implementation constraints and tagged values of
each readable element, are read, written and compared. So are ArgumentReasoning,
ArtifactReference and ArgumentGroup elements and the five asserted relationships
(AssertedInference, AssertedEvidence, AssertedContext, AssertedArtifactSupport and
AssertedArtifactContext), with their sources, target, reasoning, `isCounter`,
meta-claims, structure, referenced artifact elements and group members.
`sacm::edit::apply` creates those eight kinds, and descriptions, notes, implementation
constraints and tagged values, and refuses a reference that names no element or an
element its end's declared type does not admit (the stricter §11.15, §11.17 and §11.18
rules, that evidence and artifact relationships link ArtifactReferences, are left to
validation); and
`sacm::validation::validate` reports each `content` or `key` text that is empty or gives
one language twice, each tagged value without a key, each `gid` two elements share, each
element with more than one description, each ArgumentPackage that nests one and also holds
other argumentation elements, each reference that names no element or an element of the
wrong kind, each element with a `citedElement` whose `isCitation` is not true, each
`abstractForm` that breaks a §8.2 rule, each asserted relationship without a source or a
target, each evidence or artifact relationship whose ends are not artifact references,
and each id a reference end repeats. The other element
kinds in XMI, the other validation rules
and the other edit operations are not built yet. Milestone M0, the foundation, is done:
the workspace, CI, the functionality matrix and its gate, and mutation testing.
The [functionality matrix](docs/matrix/functionality-matrix.md) shows what is verified;
nothing else is claimed.

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
