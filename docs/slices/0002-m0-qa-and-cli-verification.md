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
