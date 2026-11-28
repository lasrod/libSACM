---
slice: 0092
name: quality-tooling
state: in-progress
step: tests
rows: [LIB-RT-004, LIB-SEC-004, LIB-QA-004, LIB-QA-006, LIB-TOOL-012]
mode: pr
---

# Slice 0092 — Property tests, fuzzing, coverage, semver checks, the download test

## Scope

The QA rows of M7 and the release-gate mutants triage: property tests of the round-trip and
edit guarantees (LIB-RT-004), fuzzing of the reader and serde records (LIB-SEC-004), line
coverage (LIB-QA-004), semver checks as a required check (LIB-QA-006), the download test
(LIB-TOOL-012). No library source changes.

## Basis

ADR 0046 (accepted with the maintainer's answers of 2026-11-27); D-RT, D-ATOMIC; ADR 0045 D7.
Writers on disjoint files: W1 library tests, W2 fuzz crate and CI, W3 xtask, W4 mutants
triage.

## Log

- 2026-11-28 design: rows `analyzed`; ADR 0046.
- 2026-11-28 tests (W3): semver.rs (nine tests; the ninth pins the help line, accepted),
  repo_checks.rs CI checks, fetch.rs. Red confirmed: semver.rs fails only for the missing
  module; six repo checks fail for the missing jobs and workflow. The four LIB-TOOL-012 tests
  pass against the existing `curl_download` (test-only row; W3 hand-mutated `--location` and
  `--fail` and saw them fail). Lead decision: LIB-QA-018's gate-mutation test is rewritten to
  mutate inside each job (`in_job`, `GATED_JOBS`), since `coverage` and `fuzz-smoke` repeat
  existing gate lines; every old mutation is kept.
- 2026-11-28 implementation. W1: six properties (no library bug found; strengthened so lists
  commit and asserted relationships are accepted), two oracles, the stable replay. W3:
  `cargo xtask semver` (24 of 25 hand mutants killed by the tests, the 25th by a unit test).
  W2: the fuzz crate; ci.yml `coverage`, `semver checks`, `fuzz (smoke)`; fuzz.yml; the spec
  fetch in the mutants jobs. W4: the triage (8 equivalent mutants excluded, 6 xtask ones
  killed once the spec is fetched, 5 timeouts confirmed loops). The maintainer widened the
  builder scope to `.github/workflows/**`, `.github/dependabot.yml` and `CONTRIBUTING.md`; the
  LIB-TOOL-003 hook test follows. Lead: mutants.toml exclusions, nextest `slow-timeout` and the
  `edit_atomic` override (ADR 0009 amendment 2), reviewed by LIB-QA-012's check (a missing
  value is a problem). The fuzz input `unclosed-root` has no `.xmi` extension, so the 0.1.0
  message golden does not read it. Rows `implemented`.
- 2026-11-28 lead: LIB-SEC-004's documented run is the PR's `fuzz (smoke)` run, since GitHub
  refuses `workflow_dispatch` of a workflow not on the default branch (HTTP 404); the first
  full fuzz.yml run is dispatched on main after the merge, as a release gate.
- 2026-11-28 verification: LIB-QA-004 and LIB-TOOL-012 PASS; LIB-RT-004, LIB-SEC-004 and
  LIB-QA-006 FAIL. The PR's `fuzz (smoke)` run found a library defect (a tolerant write of a
  renamed `xmi` prefix does not read back equal): filed as LIB-COMPAT-022, which LIB-SEC-004
  now depends on; both go to the next slice with the regression input. Fix round: the
  property generator reaches every operation (LIB-RT-004); a subprocess test of `cargo xtask
  semver` and triage rows for the I/O shell (LIB-QA-006); the fetch.rs module doc. The lead
  made `semver checks` a required check on main (the maintainer's decision of 2026-11-27).

- 2026-11-28 lead: the maintainer set the coverage threshold to 98% (the first measurement
  was 98.78%); `COVERAGE_MIN_LINES: "98"` in ci.yml, and LIB-QA-004 says so. Fix round done:
  each property that edits records how often each of the 59 operations was tried and
  accepted, and fails at setup if one is never accepted (skipped when `PROPTEST_CASES` lowers
  the case count); three subprocess tests of `cargo xtask semver` (job summary, failing check,
  missing cargo-semver-checks) are cited in LIB-QA-006.
