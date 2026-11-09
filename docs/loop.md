# The engineering loop

libSACM is built in **slices**: small increments, each tied to rows of the
[functionality matrix](matrix/functionality-matrix.md) and verified independently.
Claude Code agents with fixed roles do most of the work; the maintainer sets direction
and merges.

## One slice

```text
matrix next → brief → research → design → failing tests → implementation → verification → PR
  (lead)     (lead)  researcher  architect  test engineer    implementer       verifier   (lead)
                         ↓                       ↓                ↓               ↓
                      analyzed             tests-failing     implemented      verified
```

A row becomes `verified` only when the verifier records a PASS. A FAIL sends each finding
to the role that owns it. Tooling and quality rows get one verification round against
their acceptance criteria ([ADR 0006](adr/0006-agent-roles-and-slice-loop.md)).

## Roles

| Role | Writes | Job |
|---|---|---|
| Lead (main session) | matrix, briefs, ADRs, verification records, workflows | Picks the slice, briefs the agents, sets statuses, commits |
| Researcher | nothing | Quotes the standard and writes acceptance criteria |
| Architect | nothing | Designs the public API |
| Test engineer | tests | Writes the failing tests first |
| Implementer | production code and docs | Makes the tests pass |
| Verifier | nothing | Decides PASS or FAIL, independently |
| Docs reviewer | nothing | Reads the docs as a newcomer |
| Matrix steward | nothing | Audits the matrix at milestones |

A hook (`.claude/hooks/agent_write_guard.py`) enforces the write scopes. No agent can
change the hook or its scopes.

## Running it

```text
/sacm-slice                 one slice on a branch, with a pull request
/sacm-slice SACM23-ARG-002  a particular slice
/loop /sacm-slice           slices back to back
```

Every slice lands through a pull request. The branch is pushed as a draft before
verification, so that CI on Linux, Windows and macOS and the mutation-testing jobs
produce the evidence the verifier needs. The maintainer merges; the loop never does.

Run `cargo xtask fetch-spec` once first: the agents need the normative sources.

## When the loop stops

It stops and waits for the maintainer when a milestone is complete, an ADR or an open
interpretation question needs a decision, a slice fails verification twice, a gate fails
for a reason outside the slice, or anything needs pushing, publishing or a decision not
yet delegated.

## Audit trail

- `docs/slices/`: one brief per slice.
- `docs/verification/`: one record per verified slice.
- `git log`: commits tagged with their row ids.
- `cargo xtask matrix status`: progress per milestone.
