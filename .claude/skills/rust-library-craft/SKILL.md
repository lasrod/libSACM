---
name: rust-library-craft
description: The Rust conventions libSACM's public crates follow — API design (Rust API Guidelines), module layout, error and diagnostic types, non_exhaustive and semver discipline, dependency policy, documentation standards, testing conventions (traced test names, golden files, property tests), lints and the local quality gate. Load before designing, writing, testing or reviewing any Rust code in this repository.
---

# Rust library craft for libSACM

`sacm` is a public crate that people will build safety tooling on. Optimize for the
user who reads the docs once and then relies on the API for years: few concepts,
predictable names, no surprises, no panics, stable meanings.

## Repository shape

```text
crates/sacm/        the library (published as `sacm`)
crates/sacm-cli/    the `sacm` binary (published as `sacm-cli`)
xtask/              repository automation (`cargo xtask ...`), not published
docs/               architecture, ADRs, SACM reference, functionality matrix, verification
```

Planned module layout of `sacm` (created as slices need them — see `docs/architecture.md`):
`model` (typed elements, packages, ids, language strings), `xmi` (read/write),
`validation` (constraint checks), `diagnostic` (codes, severities, locations),
`edit` (operations, previews, results), `compare` (semantic equality/diff).

## API conventions

- Follow the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/checklist.html).
- **SACM names stay SACM names**: `Claim`, `AssertedInference`, `ArtifactReference`.
  camelCase attributes become snake_case methods: `isCounter` → `is_counter()`,
  `assertionDeclaration` → `assertion_declaration()`.
- **Private fields, public accessors** on model types (C-STRUCT-PRIVATE), so internals
  can change without breaking users.
- **`#[non_exhaustive]`** on every public enum (element kinds, diagnostic codes, error
  variants, policies) and on option/result structs that will grow.
- **Common traits**: derive `Debug` everywhere; `Clone`, `PartialEq`, `Eq`, `Hash`
  where meaningful; `Default` only when there is an obvious default; `Display` for
  user-facing values (ids, codes, diagnostics). Public types are `Send + Sync` — add a
  static assertion test.
- **Identifiers** are a newtype (`ElementId`), not `String`. References between
  elements are ids resolved through the document, not borrowed pointers.
- **Constructors and builders**: `new` for the obvious case, builders or operation
  structs for many optional fields. No boolean parameters with unclear meaning.
- **Iterators over collections**: return `impl Iterator<Item = &T>` (or a named
  iterator type if it must be stored), not `&Vec<T>`.
- **No dependency types in public signatures** unless an ADR says so (they become
  semver commitments). XML library types never appear publicly.
- **Feature flags** are additive. `serde` support, if added, is behind `serde`.

## Errors vs diagnostics

| Situation | Type |
|---|---|
| Cannot proceed: I/O failure, unreadable XML, rejected edit | `Result<_, Error>` — an enum implementing `std::error::Error`, `#[non_exhaustive]`, with `source()` chains |
| Conformance finding about content | `Diagnostic` value returned alongside the result |

A `Diagnostic` has a stable `code` (`SACM-REF-001`), `severity`, the matrix row /
clause it enforces, affected element ids, an optional source location, and a message.
Codes are API: documented in `docs/sacm/diagnostics.md`, never reused.

Hand-write error types (no `thiserror`/`anyhow` in the library; `anyhow` is fine in
xtask). Error messages are lowercase, without trailing punctuation.

## Never panic on input

No `unwrap`, `expect`, indexing or arithmetic that can panic on input-derived values in
library code. `expect` only for internal invariants, with the invariant as its message.
Every public function that can panic documents it under `# Panics` — and the goal is
that none can.

## Determinism

Serialized output and reported diagnostics are deterministic. Do not iterate a
`HashMap`/`HashSet` to produce output; use `BTreeMap`, `Vec`, or an insertion-ordered
map. Generated ids are deterministic for a given sequence of operations.

## Documentation

- `#![deny(missing_docs)]`-level coverage (the workspace warns; CI denies warnings).
- Crate root: what SACM is, what the crate does, a complete runnable example, links to
  the guide and the functionality matrix.
- Every public item: a one-line summary saying what it is *for*. Important types and
  functions: `# Examples` that compile and run (doctests are tests).
- `# Errors` on every fallible function; `# Panics` where applicable.
- SACM concepts explained in plain words where they enter the API, with the clause
  number from the matrix row.
- Intra-doc links (`[`Claim`]`) rather than prose references.
- `cargo doc` builds with `RUSTDOCFLAGS="-D warnings"`.

## Testing conventions

- **Traced names**: every test is named `<row id in snake case>_<what it checks>`,
  e.g. `sacm23_val_005_relationship_without_target_is_an_error`, and its assertion
  messages start with the row ID. `cargo xtask matrix check` enforces naming and
  rejects tests no row cites.
- **Integration tests** (`crates/sacm/tests/*.rs`) use only the public API. Group by
  area: `xmi_read.rs`, `xmi_write.rs`, `roundtrip.rs`, `validation*.rs`, `edit_*.rs`.
- **Fixtures**: `crates/sacm/tests/data/sacm23/<area>/...-valid.sacm.xmi`,
  `.../invalid/...-<reason>.sacm.xmi`, golden output under `tests/data/golden/`.
  Load them with `include_str!` or a path from `env!("CARGO_MANIFEST_DIR")`.
- **Semantic comparison** for "nothing lost"; **exact bytes** only for determinism.
- **Negative tests** assert diagnostic code + severity + affected ids.
- **Property tests** (`proptest`) for round-trip and atomicity invariants.
- **Unit tests** in `#[cfg(test)] mod tests` for private helpers only.

## Dependencies

Each new dependency needs a reason std cannot meet, a licence allowed by `deny.toml`,
MSRV ≤ 1.85 compatibility, and evidence of maintenance. Prefer few, widely used crates.
Dev-dependencies are cheaper but still reviewed. Record significant ones in an ADR.

## MSRV and semver

- MSRV is 1.85 (`rust-version` in `Cargo.toml`, checked in CI). Raising it is a minor
  version bump, noted in the CHANGELOG.
- Until 1.0, breaking changes bump the minor version and are listed in the CHANGELOG
  under **Breaking**. After 1.0, `cargo semver-checks` gates releases.

## Lints

Workspace lints (`Cargo.toml`): `unsafe_code = forbid`, `missing_docs`,
`unreachable_pub`, `clippy::pedantic`, no `dbg!`/`todo!`/`print*!` in library code.
Silence a lint locally with `#[expect(clippy::..., reason = "...")]`, never crate-wide
without an ADR.

## Local gate (run before every report)

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --all-features
cargo xtask matrix check
cargo deny check            # if installed
```
