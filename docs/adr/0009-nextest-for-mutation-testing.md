# 0009. cargo-mutants runs tests under nextest, stopping at the first failure

- Status: accepted (the maintainer chose "Adopt nextest" on 2026-11-09)
- Date: 2026-11-09
- Matrix rows: LIB-QA-012, LIB-QA-007

## Context

cargo-mutants 27.1.0 runs `cargo test` for each mutant by default. `cargo test` runs the integration-test binaries one after another in name order, and stops at the first one that fails. So a mutant can be reported as a TIMEOUT when an early-sorting binary becomes slow under it, even though a later binary kills it in under a second.

This happened in slice 0005 (mutants.yml run 42420891449, `is_identifier_char -> true`). It was worked around by renaming a test target so that it sorts after `function_scan`. Nothing enforces that ordering.

Slice 0003 decided the opposite in **D7** (`docs/verification/0003-m0-mutation-testing-prescription.md`): "The test tool is `cargo test`, the default; nextest skips doctests." Its LIB-QA-007 checks therefore refuse the `test_tool` and `additional_cargo_test_args` keys.

The research for slice 0011 established these facts. They are cited from cargo-mutants v27.1.0 and cargo-nextest 0.9.151 sources, and demonstrated on a scratch copy:
- **The command it runs:** with `test_tool = "nextest"`, cargo-mutants runs `cargo nextest run --verbose --package=<pkg>@<ver> --all-features [additional_cargo_test_args]`. It never passes a nextest profile or a fail-fast flag of its own.
- **nextest's default fail-fast** (`max-fail = 1`, `terminate = "wait"`) starts no new tests after a failure, but waits for the running ones. On the demonstration, that still timed out.
- **Stopping immediately:** `fail-fast = { max-fail = 1, terminate = "immediate" }` (nextest 0.9.111 or later) kills every running test at the first failure. With a slow, first-sorting target added, the mutant was caught in 0.76 s. Under `cargo test` and under default nextest, it was a TIMEOUT at 60 s.
- **Outcomes:** cargo-mutants classifies them by exit code only. nextest's exit 4 (no tests run) would count as caught; `--no-tests=warn` makes such a package exit 0, so its mutants are missed instead.
- **Doctests:** nextest does not run them, so a mutant that only a doctest kills shows as missed.

## Decision

- **`.cargo/mutants.toml`:**
  - `test_tool = "nextest"`;
  - `additional_cargo_test_args = ["--profile=mutants", "--no-tests=warn"]`.
- **`.config/nextest.toml`:**
  - `nextest-version = { required = "0.9.111" }`;
  - a profile `[profile.mutants]` with `fail-fast = { max-fail = 1, terminate = "immediate" }`.
  
  Developers' normal `cargo nextest run` (the default profile) is unchanged.
- **CI:** every job that runs cargo-mutants (the `mutants` job in `ci.yml` and the shards in `mutants.yml`) installs nextest with exactly `cargo install cargo-nextest --locked --version 0.9.151`. That is a pinned crates.io source build, verified by Cargo.lock checksums, which is the same pattern as cargo-mutants. Listing mutants (`cargo mutants --list`, used by `cargo xtask mutants exclusions`) does not need nextest.
- **The LIB-QA-007 checks** accept `test_tool` only as exactly `"nextest"`, and `additional_cargo_test_args` only as exactly that list. Every other value is still refused. Their reviewed step lists accept the nextest install step, exactly as above, in each mutants job.
- **LIB-QA-012's check** fails if any part of this is lost: the two keys, the profile and its fail-fast setting, the version requirement, or the install step in each mutants job.
- **Doctests are not run under mutation.** They still run in the normal gate (`cargo test --workspace --all-features`).
  - A mutant that only a doctest kills is reported as missed, and so fails the run, which is safe.
  - Its triage is to add a unit or integration test that kills it.

This deviates from slice 0003's D7. It does not concern the assurance-forge C++ library, which has no mutation-testing decision.

## Consequences

- **Timeouts:** whether a mutant is caught no longer depends on test-target names. The ordering rule in `xtask/tests/function_scan.rs`'s module doc becomes history, not a constraint.
- **CI cost:** each mutants job builds cargo-nextest from source, about 3 minutes.
- **Local runs:** developers need cargo-nextest 0.9.111 or later. `CONTRIBUTING.md` gives the install command.
- **Doctests:** mutants that only doctests kill become visible as missed, rather than silently relying on doctests. Doctests must not be the only test of a behaviour.
- **Overriding the tool:** with `--test-tool cargo`, `cargo test` would reject `--profile=mutants`. Overriding the tool is therefore not supported, and the LIB-QA-007 checks refuse `--test-tool` on the mutants commands.

## Alternatives considered

- **Keep `cargo test` and enforce the target order with a check.** Rejected: it guards only the one known hazard, not the guarantee LIB-QA-012 asks for.
- **`--no-fail-fast` for `cargo test`.** Rejected: the slow target still runs until the timeout.
- **nextest with its default fail-fast.** Rejected: it waits for running tests, and the demonstration still timed out.
- **The prebuilt nextest binary (`get.nexte.st`).** Rejected for now: an unpinned download without a checksum, the class that LIB-QA-015 tracks.
