---
slice: 0001
name: m0-tooling-verification
state: stopped
step: verification
rows: [LIB-TOOL-001, LIB-TOOL-002, LIB-TOOL-003, LIB-API-001]
mode: local
---

# Slice 0001 — Verify the locally verifiable M0 foundation rows

## Scope

Independent verification of the foundation tooling and the first API row, all
implemented in the initial commit (`4b4a8e9`): the functionality-matrix gate, the
hash-pinned reference fetcher, the agent write-scope hook (including the live probe its
acceptance criteria require), and the version constants.

| Row | Title | Status at start |
|---|---|---|
| LIB-TOOL-001 | Functionality-matrix gate | implemented |
| LIB-TOOL-002 | Hash-pinned fetching of the normative SACM 2.3 sources | implemented |
| LIB-TOOL-003 | Agent write scopes are enforced by a hook | implemented |
| LIB-API-001 | Library and standard versions are exposed | implemented |

## Out of scope

- LIB-QA-001 (CI): its acceptance requires a green GitHub run, which requires pushing —
  not authorized for this slice. LIB-QA-002 and LIB-QA-003 depend on it.
- LIB-CLI-001: depends on LIB-API-001 being verified first; next slice.

## Normative basis

None (tooling and API rows; sources are ADRs 0003, 0004, 0006 and D-VERSION).
Research, design, test and implementation steps do not apply — the rows were
implemented test-first in the initial commit.

## Design

No public API change.

## Tests

Existing: 14 `lib_tool_001_*` and 2 `lib_tool_002_*` xtask unit tests, 11
`test_lib_tool_003_*` hook tests, `lib_api_001_reports_library_and_sacm_versions` plus
doctests.

## Log

- 2026-11-07 preflight: initial structure approved and committed (`4b4a8e9`); references fetched.
- 2026-11-07 verification: live hook probes (test-engineer out-of-scope write denied by hook; read-only role had no Write tool). sacm-verifier: LIB-API-001 PASS → verified; LIB-TOOL-001/002/003 FAIL (test adequacy, hook fail-open paths) — see docs/verification/0001-m0-tooling-verification.md.
- 2026-11-07 stopped: fix round blocked — xtask/ and .claude/hooks/ are outside every agent write scope; maintainer decision needed on tooling ownership.
