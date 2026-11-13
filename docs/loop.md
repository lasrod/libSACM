# The engineering loop

libSACM is built in **slices**: increments of 4 to 8 related rows of the
[functionality matrix](matrix/functionality-matrix.md), each verified independently.
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
verification, so that CI on Linux, Windows and macOS and the mutation testing of the PR's
diff produce the evidence the verifier needs. The full mutation run happens weekly, at each
milestone's close, and for a slice that removes tests or changes the mutation config. Once the verifier passes and CI is green, the loop merges the PR
itself (the maintainer's standing authorization, 2026-11-13).

**Stacked PRs.** The loop does not wait for a merge. Each new slice branches from the
newest open slice PR, and its PR targets that branch.
- **Merge order:** merge from the bottom of the stack, and only PRs that are ready (not
  draft). A draft PR has not passed verification.
- **After a merge:** the next iteration moves the following PR's target to `main`, then
  merges `main` into it, so its CI runs against the new base.
- **Review comments:** the loop fixes them first, on their own branch, and merges the fix
  forward into the later branches. While a ready PR is being changed, it goes back to
  draft. It is ready again only when CI is green and, if its own slice changed, the
  verifier has passed the change.
- **A full stack:** at three ready PRs, the loop pauses and checks for merges about every
  30 minutes.

Run `cargo xtask fetch-spec` once first: the agents need the normative sources.

**Findings that do not block.** The verifier labels each finding blocking or follow-up.
Follow-ups (wording, citations, comments), and non-blocking docs-review and Copilot
comments, go to [follow-ups.md](follow-ups.md). A docs slice clears them every few slices.

**Decisions in batches.** Before a milestone's first slice, the lead collects the
milestone's open decisions and asks the maintainer in one batch.

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
