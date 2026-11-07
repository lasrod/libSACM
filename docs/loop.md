# The engineering loop

libSACM is built in **slices**: small, independently verified increments taken in
order from the [functionality matrix](matrix/functionality-matrix.md). Most of the work
is done by Claude Code agents with fixed roles; the maintainer approves direction at
checkpoints. This page is for humans who want to run, steer or audit the loop.

## Roles

| Role | Agent | Writes | Job |
|---|---|---|---|
| Lead | main session + `sacm-slice` skill | matrix, briefs, ADRs, verification records, commits | Chooses the slice, briefs the agents, applies results, decides "verified" only on a PASS |
| Researcher | `sacm-researcher` | nothing | Quotes the standard, writes acceptance criteria, inventories the metamodel, researches interop |
| Architect | `sacm-architect` | nothing | Designs the public Rust API for the slice |
| Test engineer | `sacm-test-engineer` | tests only (`crates/*/tests/**`, `xtask/tests/**`, hook tests) | Writes the failing tests and fixtures first |
| Implementer | `sacm-implementer` | production code (`crates/*/src`, `xtask/src`) and docs, **not** tests/matrix/`.claude` | Makes the tests pass, documents the API |
| Verifier | `sacm-verifier` | nothing | Independently decides PASS/FAIL |
| Docs reviewer | `sacm-docs-reviewer` | nothing | Reviews docs and ergonomics as a newcomer |
| Matrix steward | `matrix-steward` | nothing | Audits the matrix against reality at milestones |

Write scopes are enforced by `.claude/hooks/agent_write_guard.py`
([ADR 0006](adr/0006-agent-roles-and-slice-loop.md)). No agent can write the guard, its
scopes or `.claude/settings.json`: the lead changes the guard against tests the test
engineer wrote, and the maintainer reviews that diff before it is committed. Shared knowledge lives in the
skills `sacm-expert` (the standard) and `rust-library-craft` (the codebase conventions).

## One slice

```text
matrix next ─► brief ─► research ─► design ─► failing tests ─► implementation ─► verification ─► commit
   (lead)       (lead)  researcher  architect  test engineer     implementer        verifier       (lead)
                          │                        │                  │                │
                          ▼                        ▼                  ▼                ▼
                       analyzed              tests-failing       implemented        verified
```

A FAIL from the verifier routes each finding to the role that owns it, at most twice;
then the loop stops and asks the maintainer.

## Running it

```text
/sacm-slice                 one slice on a branch, pushed, PR opened (mode=pr, the default)
/sacm-slice SACM23-ARG-002  force a particular slice
/sacm-slice mode=local      one slice committed locally, not pushed
/loop /sacm-slice           slices back to back; each waits for the previous PR to be merged
```

Every slice lands through a pull request: CI runs on all three platforms, and the
maintainer merges. The loop never merges its own PRs.

Before the first run: `cargo xtask fetch-spec` (the agents need the normative PDF and
model locally).

## Checkpoints — the loop stops and waits for the maintainer when

- a milestone is complete (the steward audits it and the lead presents a summary);
- an ADR is proposed or an open question in `docs/sacm/decisions.md` blocks progress;
- the verifier fails a slice twice;
- a gate fails for a reason outside the slice;
- anything would require pushing, publishing or a decision not yet delegated.

## Auditing the loop

- `docs/slices/` — one brief per slice: scope, normative basis, design, tests, log.
- `docs/verification/` — one record per verified slice, with the verifier's report.
- `git log` — one commit per slice, subject tagged with its row ids.
- `cargo xtask matrix status` — progress per milestone.
