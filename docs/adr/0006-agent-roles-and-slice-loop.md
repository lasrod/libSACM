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
