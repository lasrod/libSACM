# 0046. Quality tooling: property tests, fuzzing, coverage and semver checks

- Status: accepted (the maintainer, 2026-11-27: the semver check is a required check; the
  coverage threshold is set after a first measurement)
- Date: 2026-11-28
- Matrix rows: LIB-RT-004, LIB-SEC-004, LIB-QA-004, LIB-QA-006.

## Context
Before 1.0 the library needs evidence beyond example tests. Edits must keep the document
savable and readable, the reader must survive any input, the tests' reach must be known,
and the public API must not break without being declared.

## Decision
- **D1. Property tests** with proptest 1.11, a dev-dependency with `std` only. Each property
  has a fixed seed and case count, so a run is reproducible. Failures are not persisted to
  files: a found case becomes a named regression test. `PROPTEST_CASES` and
  `PROPTEST_RNG_SEED` widen the weekly run.
- **D2. Fuzzing** with cargo-fuzz in `fuzz/`, outside the workspace: libfuzzer-sys carries
  the NCSA licence, which `deny.toml` does not allow in distributed code. A pinned nightly
  runs it. One oracle per target is shared with a stable replay test, so every found input
  is replayed in the normal test run. No corpus is committed; regression inputs live in
  `crates/sacm/tests/data/fuzz`.
- **D3. Coverage** with cargo-llvm-cov over the `sacm` crate's own tests, published in the
  job summary and as an artifact. No outside service. The threshold `COVERAGE_MIN_LINES` is
  set once the maintainer has seen a first measurement; until then the job only reports.
- **D4. Semver checks** with cargo-semver-checks, the `serde` feature only (ADR 0045 D7),
  against the newest release tag. The release type follows from the CHANGELOG and the
  version: before 1.0, a break listed under `[Unreleased] ### Breaking` is allowed; from
  1.0, a break needs the major version bump. The job is a required check.

## Rejected
- A hand-written random generator alone: it cannot shrink a failing case.
- cargo-afl: it needs AFL++ installed.
- Codecov or another hosted service: an outside service and a token for no added value.
- The cargo-semver-checks GitHub Action: it does not pin the tool's version.

## Consequences
CI gains three jobs: `coverage` and `fuzz (smoke)` advise, and `semver checks` is required.
A weekly workflow fuzzes each target for 20 minutes and runs the properties with 4096 cases.
Mutation runs use 16 cases per property, so they stay fast.
