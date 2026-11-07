# libSACM

A Rust library for **OMG SACM 2.3** — the [Structured Assurance Case
Metamodel](https://www.omg.org/spec/SACM/2.3). Read, inspect, validate, edit and
write assurance cases as standard XMI.

[![CI](https://github.com/lasrod/libSACM/actions/workflows/ci.yml/badge.svg)](https://github.com/lasrod/libSACM/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
![MSRV 1.85](https://img.shields.io/badge/MSRV-1.85-orange.svg)

> **Status: early development.** The repository structure, quality gates and
> engineering process are in place; the SACM model is being built slice by slice.
> Nothing is claimed that the [functionality matrix](docs/matrix/functionality-matrix.md)
> does not show as verified.

## What is SACM?

An *assurance case* is a structured argument, supported by evidence, that a system is
acceptably safe, secure or otherwise fit for its purpose — the kind of argument
required by standards such as ISO 26262, IEC 61508, DO-178C and UL 4600. SACM is the
OMG standard for representing assurance cases so that tools can exchange them: claims,
the relationships that support or challenge them, the evidence they rest on, the
vocabulary they use, and how case modules fit together.

## What libSACM aims to be

- **Complete**: a typed model of the whole SACM 2.3 metamodel — assurance case,
  argumentation, artifact and terminology packages, with interfaces and bindings.
- **Faithful**: nothing conforming is lost on load → save. Strict mode writes conformant
  SACM 2.3; tolerant mode reads third-party and legacy files and reports exactly what it
  accepted.
- **Checked**: the specification's constraints are validated and reported as
  machine-readable diagnostics with stable codes, clause references and locations.
- **Safe to edit**: every public mutation either succeeds and leaves the document valid
  or fails and changes nothing; destructive edits can be previewed.
- **Easy to use and well documented**, with no `unsafe` code and no panics on input.
- **Neutral**: no tool-, notation- or layout-specific concepts. GSN tools, CAE tools and
  custom pipelines can all build on it.

## Workspace

| Crate | What it is |
|---|---|
| [`sacm`](crates/sacm) | The library. |
| [`sacm-cli`](crates/sacm-cli) | The `sacm` command-line tool (`validate`, `roundtrip`, `convert`, ...). |
| `xtask` | Repository automation (`cargo xtask ...`); not published. |

## Documentation

- [Architecture](docs/architecture.md) and [decision records](docs/adr/)
- [Functionality matrix](docs/matrix/functionality-matrix.md) — what is implemented and verified
- [SACM interpretation decisions](docs/sacm/decisions.md), [diagnostics catalog](docs/sacm/diagnostics.md),
  [metamodel inventory](docs/sacm/metamodel-inventory.md)
- [How the library is built](docs/loop.md) — the agent-driven engineering loop

## Relationship to assurance-forge

libSACM is a Rust implementation based on the SACM 2.3 library of
[assurance-forge](https://github.com/lasrod/assurance-forge) (C++), by the same author.
It inherits that project's analysis of the standard — its metamodel inventory,
interpretation decisions, specification-defect findings and diagnostic codes — and
re-implements the library in Rust as an independent, reusable crate.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Security issues: [SECURITY.md](SECURITY.md).

## License

[MIT](LICENSE). SACM is a specification of the Object Management Group; this project
is not affiliated with or endorsed by OMG. The OMG specification documents are not
redistributed here (`cargo xtask fetch-spec` downloads them for local reference).
