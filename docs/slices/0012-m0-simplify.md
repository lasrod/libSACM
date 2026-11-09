---
slice: 0012
name: m0-simplify
state: in-progress
step: implementation
rows: [LIB-QA-002, LIB-QA-003, LIB-QA-007, LIB-QA-008, LIB-QA-009, LIB-QA-010, LIB-QA-011, LIB-QA-012, LIB-TOOL-005, LIB-DOC-007]
mode: pr
---

# Slice 0012: simplify the harness and close M0

The harness had outgrown the library. On 2026-11-09 the maintainer approved four steps:

1. close slice 0011;
2. close M0;
3. make tooling verification proportionate;
4. stop pinning CI YAML with tests, and use branch protection instead.

Branch protection on `main` is already on. This slice does the rest.

## Decisions

- **Retired** (`out-of-scope`, tests and code removed):
  - LIB-QA-008: CI gate steps pinned as YAML;
  - LIB-QA-009: the action input table;
  - LIB-QA-010: the `cargo xtask mutants exclusions` command and its CI job;
  - LIB-QA-011: the skill-text check;
  - the planned rows LIB-QA-013 to -016 and LIB-TOOL-016 to -019.
- **Trimmed** (stay `verified`, keep only the parts that check repository policy files):
  - LIB-QA-002: MSRV consistency;
  - LIB-QA-003: the `deny.toml` policy;
  - LIB-QA-007: the `.cargo/mutants.toml` review and the triage cross-check;
  - LIB-QA-012: the `.cargo/mutants.toml` and `.config/nextest.toml` settings.
  
  The tests that pin workflow YAML structure are deleted.
- **LIB-TOOL-005:** the markdown-rs differential test and the `markdown` dev-dependency are deleted.
- **Moved to M7:** LIB-TOOL-012 and LIB-DOC-006.
- **Done here:** LIB-DOC-007 (the documented MSRV matches the manifest), with one small test.
- **`mutants.yml`** no longer runs the full mutation run on PRs that touch only `xtask/**`. The PR-diff job covers them.
- **Process:** ADR 0006 amendment 2. Tooling and quality rows get one verification round against their acceptance; new bypass classes are logged, not chased.
- **Docs:** short and direct.
  - A README with the honest status, and a `docs/README.md` map.
  - Short versions of the loop, architecture, matrix rules, triage, CONTRIBUTING and CHANGELOG.
  - The M0 slice briefs and verification records stay as they are: they are the audit trail.

## Log

- 2026-11-09 the brief is written, and the work is split between the test engineer (test cuts), the implementer (the source cut and docs) and the lead (matrix, ADR, workflows, skill and agents).
- 2026-11-09 done:
  - **Test engineer:** cut the tests, from 30,860 to 17,550 lines (`repo_checks.rs`
    14,823 → 4,040), and wrote the LIB-DOC-007 test.
  - **Implementer:** removed `xtask/src/mutants.rs` and the `markdown` dev-dependency,
    and rewrote the README, docs map, loop, architecture, triage and CHANGELOG.
  - **Lead:**
    - the workflows (the `mutants-exclusions` job removed, `xtask/**` dropped from the
      full-run trigger);
    - ADR 0006 amendment 2, and the shorter verifier definition and slice skill;
    - CONTRIBUTING and the matrix README;
    - in the matrix, 12 rows retired and 2 moved to M7. LIB-QA-002, -003, -007 and -012
      are trimmed and back to `implemented` for re-verification, and LIB-DOC-007 is
      `implemented`;
    - `ci-actions.md` and the `yaml-rust2` dev-dependency deleted.
  - The full gate passes.
