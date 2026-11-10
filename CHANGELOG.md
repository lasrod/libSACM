# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html). Before 1.0, a breaking
change bumps the minor version and is listed under **Breaking**.

## [Unreleased]

### Added

- Cargo workspace: the `sacm` library, `sacm-cli` (`sacm version`) and `xtask`.
- `sacm::VERSION` and `sacm::SACM_VERSION`.
- The functionality matrix and its gate (`cargo xtask matrix check|render|next|status`):
  every row's status must match its tests and verification record.
- Hash-pinned download of the OMG SACM 2.3 references (`cargo xtask fetch-spec`).
- CI gates: rustfmt, clippy, tests on Linux, Windows and macOS, MSRV 1.85, rustdoc,
  cargo-deny, the matrix gate and the agent-hook tests. Third-party actions are pinned
  by commit SHA.
- Mutation testing with cargo-mutants under cargo-nextest: a PR job mutates the diff,
  and a weekly sharded run covers the workspace. Survivors are triaged in
  `docs/quality/mutants-triage.md`.
- Agent roles, skills and a write-scope hook for the engineering loop.
- `sacm::model`: the read-only document model (`Document`, `ElementId` and the
  `SacmElement` view, with roots, look-up by id, parent, children and pre-order
  traversal) and the metamodel's element kinds (`ElementKind`, `AbstractClass`,
  `MetamodelPackage`) [LIB-API-002, LIB-API-003].
- The `test-support` feature, which makes `Document::insert` and the diagnostic
  constructors public for tests. It is not part of the stable API.
- `sacm::diagnostic`: `Diagnostic` (code, severity, matrix row, clause, affected ids,
  source location, message), `Severity`, `Code` and `SourceLocation`, with a one-line
  `Display` and one total order. No code is emitted yet [LIB-DIAG-001].
- Error convention (ADR 0011): one `#[non_exhaustive]` error enum per operation.
  `InsertError` messages quote and escape ids [LIB-API-004].
- `sacm::model::LangString`: an optional language tag and optional content, with
  absent, empty and non-empty kept distinct (ADR 0012) [SACM23-BASE-001].
- `sacm::model::AssertionDeclaration`: the six SACM 2.3 literals, with `asserted` as the
  default and exact `as_str`/`FromStr`; a failed parse is a
  `ParseAssertionDeclarationError` [SACM23-ARG-003].
- `sacm::model::MultiLangString`: an ordered list of `LangString` values; duplicates are
  kept and equality depends on order [SACM23-BASE-002].
- `SacmElement::name`: an element's `name`, or `None` when it is missing; nothing invents
  a name (ADR 0013) [SACM23-BASE-003].
- The unstable `test-support` `Document::insert` takes a name, and refuses one for the
  four UtilityElement kinds with `InsertError::NameNotAllowed` [SACM23-BASE-003].

### Changed

- Simplified the harness: removed CI YAML pin tests and the mutation-exclusions command.
