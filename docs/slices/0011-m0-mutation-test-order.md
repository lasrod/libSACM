---
slice: 0011
name: m0-mutation-test-order
state: in-progress
step: verification
rows: [LIB-QA-012]
mode: pr
---

# Slice 0011: mutation outcomes do not depend on test-target order

## Scope

cargo-mutants runs the tests under nextest, with a profile that kills every running test at
the first failure ([ADR 0009](../adr/0009-nextest-for-mutation-testing.md)). A mutant that any
fast test kills is then caught, even when another test target becomes slow under it. A check
pins every part of the setup. The LIB-QA-007 checks are updated to accept exactly this
configuration.

| Row | Title | Status at start |
|---|---|---|
| LIB-QA-012 | Mutation outcomes do not depend on test-target order | planned |

## Out of scope

- The prebuilt nextest download (LIB-QA-015 territory).
- Running doctests under mutation. cargo-mutants 27.1.0 cannot do it with nextest. They
  still run in the normal gate.

## Normative basis

None. This is a quality row. The research facts (cargo-mutants v27.1.0 and cargo-nextest
0.9.151 sources, plus a scratch demonstration) are summarised in ADR 0009's Context.

## Design (fixed by the lead; ADR 0009, accepted by the maintainer on 2026-11-09)

- **D1: configuration.** The lead adds this configuration, because `.cargo/` and `.config/`
  are outside every agent's scope:

  ```toml
  # .cargo/mutants.toml
  test_tool = "nextest"
  additional_cargo_test_args = ["--profile=mutants", "--no-tests=warn"]
  ```

  ```toml
  # .config/nextest.toml
  nextest-version = { required = "0.9.111" }

  [profile.mutants]
  fail-fast = { max-fail = 1, terminate = "immediate" }
  ```
- **D2: CI.** The lead adds `- run: cargo install cargo-nextest --locked --version 0.9.151`
  right after the cargo-mutants install step, in the `mutants` job of `ci.yml` and in
  `mutants.yml`.
- **D3: checks (test engineer).**
  - **The LIB-QA-007 checks:**
    - `REVIEWED_MUTANTS_KEYS` gains `test_tool` and `additional_cargo_test_args`, each
      accepted only with exactly the D1 value;
    - `UNREVIEWED_MUTANTS_KEYS` keeps a control for each key with any other value (e.g.
      `test_tool = "cargo"`, `additional_cargo_test_args = ["--", "--skip", "lib_"]`);
    - the reviewed step lists of both mutants jobs gain the nextest install step.
  - **The new LIB-QA-012 check** covers acceptance items 1 to 5. It reads `.config/nextest.toml`
    with `toml`.
  - **The module doc** of `xtask/tests/function_scan.rs`: its ordering rule is marked as
    superseded by ADR 0009.
- **D4: docs (implementer).** `docs/quality/mutants-triage.md` gets the configuration in force
  and a slice 0011 section.
- **D5: the lead** writes `CONTRIBUTING.md`, which gets the nextest install command for local
  runs, as the maintainer decided in slice 0010.

## Tests

| Tests | Row | Criterion |
|---|---|---|
| 12 `lib_qa_012_*` tests in `xtask/tests/repo_checks.rs` | LIB-QA-012 | `check_nextest_setup` checks the real files and fixtures with controls. It covers each `mutants.toml` key, the `mutants` profile and its `fail-fast`, the `default` profile, unreviewed keys, the version requirement compared numerically, the pinned install before `cargo mutants` in both workflows, the full run's PR path, unreadable files, equivalent forms, and LIB-QA-007's acceptance of exactly this setup. |
| 3 new `lib_qa_007_*` tests, and updated LIB-QA-007 checks and fixtures | LIB-QA-007 | Exactly the reviewed nextest values and install step are accepted; every other value or spelling is refused. |

## Log

- 2026-11-09 research: a general-purpose research agent read the cargo-mutants v27.1.0 and
  cargo-nextest 0.9.151 sources and ran a scratch demonstration. With a slow,
  first-sorting target, the `is_identifier_char -> true` mutant gave:
  - TIMEOUT at 60 s under `cargo test`;
  - TIMEOUT at 60 s under nextest with its default fail-fast;
  - caught, in 0.76 s, under nextest with `terminate = "immediate"`.
- 2026-11-09 design: adopting nextest reverses slice 0003's D7. The maintainer chose "Adopt
  nextest", recorded as ADR 0009 (accepted). The row is now `analyzed`, with acceptance by the
  lead.
- 2026-11-09 tests: the test engineer wrote 12 `lib_qa_012_*` tests, 3 new `lib_qa_007_*`
  tests, and updated LIB-QA-007 checks and fixtures. It also marked the `function_scan.rs`
  ordering rule as superseded.
  - Lead decisions:
    - LIB-QA-007 accepts a missing nextest step, and LIB-QA-012 reports it;
    - `[nextest-unreviewed-key]` is confirmed;
    - the string form of `nextest-version` is accepted;
    - `.config/nextest.toml` is added to the full run's PR paths, pinned as
      `[nextest-full-run-path]`.
  - The real-file LIB-QA-012 test was red with 6 problems. LIB-QA-012 is
    `tests-failing`; LIB-QA-007 stays `verified` and cites 40 tests.
- 2026-11-09 implementation (lead):
  - D1: `.cargo/mutants.toml` keys and `.config/nextest.toml`.
  - D2: the nextest install after the cargo-mutants install in `ci.yml`'s `mutants` job
    and in `mutants.yml`, plus the `.config/nextest.toml` path in `mutants.yml`.
  - D5: `CONTRIBUTING.md`.
  - All 134 `repo_checks` tests pass, and `cargo xtask mutants exclusions` still passes.
    The implementer is updating the triage document.
- 2026-11-09 the implementer updated `mutants-triage.md` (configuration in force, slice 0005 note, a slice 0011 section with placeholders for the Linux runs) and the CHANGELOG. The full local gate passes, and LIB-QA-012 is `implemented`.
- 2026-11-09 PR #18 (draft). At `7e657bd`, CI run 42582691844 succeeded; `mutants (PR diff)`
  found no mutants to filter. mutants.yml run 42582691741 was the first full run under
  nextest: 544 caught, 0 missed, 0 timeouts, 15 unviable. The log shows `cargo nextest run
  ... --profile=mutants --no-tests=warn`.
  - The lead compared outcome sets with the last `cargo test` full run (42571009239 at
    `e03ba86`): the caught set (544) and the unviable set (15) are identical. So no mutant
    relied on a doctest alone.
  - The lead filled in the triage placeholders. Copilot left no comments.
