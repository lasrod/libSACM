# 0006. Agent roles with enforced write scopes, and the slice loop

- Status: accepted
- Date: 2026-11-07
- Matrix rows: LIB-TOOL-003

## Context

Most code in this repository will be written by AI agents running in a loop. The
failure modes that matter are the ones an agent will not notice in its own work:
tests written to fit the implementation, a requirement quietly narrowed, a "verified"
claim that nobody independent checked. In the predecessor project, a subagent's
`tools:` list turned out not to be a sandbox — background subagents could still write.

## Decision

- Development proceeds in **slices** of 1–5 matrix rows through fixed steps: research
  → design → failing tests → implementation → independent verification → matrix
  update and commit. The main session acts as lead, following the `sacm-slice` skill;
  `/loop /sacm-slice` runs slices back to back and stops at checkpoints.
- Seven roles in `.claude/agents/`: `sacm-researcher`, `sacm-architect`,
  `sacm-test-engineer`, `sacm-implementer`, `sacm-verifier`, `matrix-steward`,
  `sacm-docs-reviewer`.
- Their write authority is enforced by a PreToolUse hook
  (`.claude/hooks/agent_write_guard.py`) reading `.claude/hooks/write-scopes.json`:
  the researcher, architect, verifier, steward and docs reviewer cannot write; the
  test engineer writes only tests and fixtures; the implementer writes only production
  code and docs and cannot touch tests, the matrix, verification records or ADRs.
- Only the lead changes matrix statuses, and a row becomes `verified` only on a PASS
  from the verifier, recorded in `docs/verification/`.
- Domain and craft knowledge live in skills (`sacm-expert`, `rust-library-craft`)
  that every role reads, so roles share one understanding of SACM and of the codebase.

## Consequences

Test-first and independent verification are properties of the setup, not of agent
good behaviour. The hook does not cover Bash or sessions with hooks disabled; the
agent prompts prohibit those routes, and the verifier checks for weakened tests.

## Alternatives considered

- **A lead subagent orchestrating others**: subagents cannot spawn subagents, so the
  lead must be the main session.
- **Prompt-only role boundaries**: shown insufficient in the predecessor project.

## Amendment 1 (2026-11-07): who owns the repository tooling

Accepted by the maintainer after verification 0001. That verification failed the
tooling rows, and the fix round had no owner: `xtask/` and `.claude/hooks/` were outside
every role's write scope.

- **`xtask` is owned like library code.** It is split into a library (`xtask/src/lib.rs`)
  and a thin binary, so its acceptance tests live in `xtask/tests/`. The implementer
  writes `xtask/src/**` and `xtask/Cargo.toml`. The test engineer writes
  `xtask/tests/**`. Reason: every `verified` claim rests on the matrix gate, and a gate
  regression is silent, which is exactly what tests written separately from the code
  are for.
- **The guard stays outside what it guards.** No agent may write
  `.claude/hooks/agent_write_guard.py`, `.claude/hooks/write-scopes.json` or
  `.claude/settings.json`. The hook script runs on every tool call, so an agent able to
  edit it could loosen its own confinement mid-slice. The test engineer may write the
  hook's tests (`.claude/hooks/test_*.py`), because tests cannot change what the guard
  enforces. The lead implements guard changes against those tests, the verifier
  re-verifies them, and **the maintainer reviews the guard diff before it is
  committed**. This is the one exception to "the lead does not build".
- **Read-only roles have two controls.** Their `tools:` list omits the write tools, and
  in practice that has been honoured, even for background subagents (live probe,
  verification 0001). The hook's read-only branch is the second control, for the case
  where the platform hands a subagent write tools anyway. It is verified by tests,
  because it cannot be triggered live while the first control holds.

## Amendment 2 (2026-11-09): proportionate verification

- Status: accepted (the maintainer approved it on 2026-11-09).
- **Context:** M0 grew 7,000 lines of checks for 47 lines of library. Each verifier round on a tooling row found another way a contributor could weaken CI, and each one became a new test or row.
- **Decision:**
  - **Library rows** (SACM model, XMI, validation, editing, the public API) keep the full adversarial verification: complete prescriptions, up to two fix rounds.
  - **Tooling, QA and docs rows** get one verification round against their stated acceptance. A new bypass class is logged in one line; it becomes a row only if it could let a real library defect pass unnoticed.
  - **CI configuration is protected** by branch protection on `main` (required checks) and by review, not by tests that pin workflow YAML.
- **Consequence:** the harness stays small. The verifier's depth goes where users of the library are affected.
