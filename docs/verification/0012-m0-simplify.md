# Verification 0012 — m0-simplify

- Date: 2026-11-09
- Row verdict: LIB-QA-002: PASS
- Row verdict: LIB-QA-003: PASS
- Row verdict: LIB-QA-007: PASS
- Row verdict: LIB-QA-012: PASS
- Row verdict: LIB-DOC-007: PASS
- Row verdict: LIB-TOOL-005: PASS
- Verifier: `sacm-verifier` (independent; read-only), a proportionate round (ADR 0006 amendment 2)
- Commit: `82bb791` plus the LIB-TOOL-005 trim; evidence: ci.yml run 42603670488, success on all 11 jobs
- Slice brief: [docs/slices/0012-m0-simplify.md](../slices/0012-m0-simplify.md)

## Result

Each row's cited tests exist, run, and assert its trimmed or new acceptance. Hand mutations in a scratch copy were all killed, for example:
- the MSRV changed in clippy.toml, ci.yml, a member manifest, the README badge or CONTRIBUTING;
- `deny.toml` yank and git-source settings;
- `cap_lints` deleted, a phantom triage row, the `.gitignore` entry;
- nextest `max-fail`, `terminate`, the version requirement and the default profile;
- the verdict reader accepting FAIL, code blocks no longer hiding lines, extensions enabled.

Nothing verified was lost silently. Removed tests belong to retired rows, or to trimmed rows whose acceptance no longer requires them, and no doc describes removed tooling.

## Findings

| Severity | Row | Finding | Resolution |
|---|---|---|---|
| High | LIB-TOOL-005 | Its acceptance still claimed the removed markdown-rs differential test. | Trimmed, with a note, and re-verified in one round. |
| Low | docs | The matrix README rule on changed requirements, and stale lines in ADRs 0008 and 0009. | Fixed: a rule update and ADR amendments. |

Logged, not required: QA-002 and QA-003 read ci.yml with `contains`, so a commented-out command still passes. This is accepted under amendment 2.
