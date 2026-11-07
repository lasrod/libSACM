---
slice: 0002
name: m0-qa-and-cli-verification
state: stopped
step: verification
rows: [LIB-QA-002, LIB-QA-003, LIB-CLI-001]
mode: pr
---

# Slice 0002: Verify the MSRV, dependency-policy and CLI-skeleton rows

## Scope

Independent verification of three M0 rows that were implemented and tested in the initial commit. Their dependencies (LIB-QA-001, LIB-API-001) were verified in slice 0001.

| Row | Title | Status at start |
|---|---|---|
| LIB-QA-002 | MSRV 1.85 is declared consistently and checked | implemented |
| LIB-QA-003 | Dependency licences, advisories and sources are policed | implemented |
| LIB-CLI-001 | CLI skeleton with `sacm version` | implemented |

## Out of scope

- LIB-QA-007 (cargo-mutants): the next slice.
- Other planned M0 tooling rows.

## Normative basis

None. These are quality and CLI rows; their sources are ADR 0007, the project policy for a public MIT library, and D-API. Research, design and test-first steps do not apply to verifying already-implemented rows; a fix round, if one is needed, follows the verifier's complete prescription.

## Design

No public API change is planned.

## Tests

Existing:
- `xtask/tests/repo_checks.rs::lib_qa_002_msrv_is_consistent`
- `xtask/tests/repo_checks.rs::lib_qa_003_dependency_policy_is_enforced`
- `crates/sacm-cli/tests/cli.rs::lib_cli_001_*` (2 tests)

## Log

- 2026-11-07 preflight: main at b86187b (slice 0001 and the Dependabot PRs merged), CI green; references fetched. Branch `slice/0002-m0-qa-and-cli-verification`.
- 2026-11-07 verification round 1: sacm-verifier.
- 2026-11-07 verification round 1: FAIL on all three rows (tests did not kill bypasses the requirements rule out); complete prescription in docs/verification/0002-m0-qa-and-cli-verification.md. Verifier accepted Linux end-to-end tests for LIB-CLI-001.
- 2026-11-07 fix round 1: test engineer added 10 tests (check functions over the real ci.yml/deny.toml/manifests, with every prescribed bypass applied in memory and reported; two Linux-only CLI tests). No production change. Pushed for Linux CI evidence before re-verification.
- 2026-11-07 verification round 2: LIB-CLI-001 PASS -> verified. LIB-QA-002 and LIB-QA-003 FAIL a second time: 13 YAML and cargo-deny bypasses within scope (two also raised by the Copilot review on PR #6). Complete remainder in docs/verification/0002-m0-qa-and-cli-verification-round2.md.
- 2026-11-07 stopped: second FAIL; the maintainer decides on fix round 2 (YAML parser). Also added: sacm-slice step 8.6 (address Copilot review comments), at the maintainer's request.
- 2026-11-07 maintainer approved fix round 2. Lead added dev-dependency yaml-rust2 0.13 to xtask (YAML 1.2, MIT OR Apache-2.0, MSRV 1.85); cargo deny passes.
- 2026-11-07 PR #6 merged by the maintainer during fix round 2 (main 871c26f). LIB-CLI-001 is verified on main; LIB-QA-002 and LIB-QA-003 stay implemented. Fix round 2 continues on branch slice/0002-qa-yaml-checks, to land as a follow-up PR.
- 2026-11-07 fix round 2: test engineer replaced the line-based ci.yml reader with a yaml-rust2 parse; 16 new tests covering Y2-1..3, Y3-1..3 and R3-3..7, with coded problems and every must-kill applied as real YAML/TOML text; sabotage of 16 checks each failed tests. Process note: it ran cargo-deny/yaml probe experiments under the session scratchpad (outside the repository, via Bash), which is outside its write scope; no repository file outside its scope changed.
- 2026-11-07 Copilot comment on PR #7 (log-level injection) fixed in 4b46db5; replies posted on PR #7 and on both PR #6 threads.
- 2026-11-07 verification round 3: LIB-QA-002 PASS -> verified. LIB-QA-003 FAIL: F1 log-level injection (three must-kills still missing) and F2 include-dev defaults to false (dev-dependencies are not licence-checked). See docs/verification/0002-m0-qa-and-cli-verification-round3.md.
- 2026-11-07 stopped: third FAIL for LIB-QA-003; F2 needs the maintainer's policy choice (a) or (b).
