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

## Amendment 3 (2026-11-13): a faster loop

The maintainer asked for the loop to go faster without losing what makes the library
trustworthy. The safeguards stay: tests before code, the independent verifier, the matrix
gate, and mutation testing of each PR's diff. Changed:
- **Bigger slices.** A slice takes 4 to 8 related rows, so the fixed cost of a slice is
  shared.
- **Full mutation runs** happen weekly and at a milestone's close, not per slice. The PR-diff
  job remains the per-slice evidence. A slice that removes tests or changes the mutation
  config still dispatches a full run.
- **Blocking and follow-up findings.** The verifier labels each finding. Only blocking ones
  (wrong behaviour, a test gap in new code, a misreading of the standard, a false claim a
  user could rely on) fail a row. Follow-ups go to `docs/follow-ups.md` and are cleared in
  batches.
- **Research and design run in parallel** as one step, and are skipped for library-only
  rows. The docs reviewer runs only when public API changes, in parallel with the
  implementer.
- **Decisions in batches.** A milestone's open decisions are asked for once, before its first
  slice.
- **The loop merges its own PRs** after a recorded PASS and green CI. Copilot comments that
  do not block become follow-ups.

*Amendment 3 status: accepted (the maintainer, 2026-11-13).*

## Amendment 4 (2026-11-15): less waiting

The maintainer adopted five more speed-ups. The safeguards of amendment 3 stay. Changed:
- **Overlap.** While slice N waits for CI or verification, the next slice's research and
  design run. Those agents only read. Its tests and code wait until slice N's PR merges, or
  stack on its branch.
- **Exact design briefs.** The architect's brief fixes exact names, field and key order,
  golden JSON samples and check order. Test engineers then have nothing to guess.
- **A milestone's decisions in one batch**, asked before its first slice. This applies
  amendment 3's rule strictly, starting with M3.
- **Documentation-only commits skip the heavy CI jobs.** When a push or pull request changes
  only `docs/**`, CI skips the test, MSRV and PR-diff mutation jobs. The format, lint,
  rustdoc, matrix, deny, hook and wasm jobs still run. This is LIB-QA-018. A slice's last
  commit (the verification record and the matrix statuses) then costs minutes, not a full
  run. A merge to `main` brings the slice's code, so it still runs everything.
- **One review for small API changes.** When a slice adds little public API, the verifier
  also reviews its documentation, and the docs reviewer is not run. Slices that add a lot of
  public API keep both.

*Amendment 4 status: accepted (the maintainer, 2026-11-15).*

## Amendment 5 (2026-11-16): fewer roles, one fix round, C++ only when stuck

An independent review (2026-11-16) found the process out of proportion for a 0.x library
with one main consumer. The maintainer adopted:
- **Three working roles:**
  - a **designer**, who does research and API design together;
  - a **builder**, who writes the failing tests first and then the code;
  - an independent **verifier**, who also reviews the docs.

  The lead confirms the red between the builder's two steps. The matrix steward runs only at
  a milestone close. No single actor both builds and judges: the verifier stays independent.
- **One fix round:** one verification round plus one fix round, then the lead asks the
  maintainer. Full depth is reserved for rows that touch reading, writing or data
  integrity. Wording findings are fixed directly, not logged as follow-ups.
- **C++ only when stuck:** the assurance-forge C++ library is consulted only when the
  standard leaves a question open and the project has no answer. ADRs no longer carry a
  "Differences from C++" section.

Mutation testing stays on the PR path.

*Amendment 5 status: accepted (the maintainer, 2026-11-16).*

## Amendment 6 (2026-11-17): mutation testing no longer blocks development

**Evidence.** Mutation testing so far found gaps in tests, never wrong behaviour:
- Across slices 0040 to 0060, every final record shows 0 missed mutants in CI's PR-diff job.
- In slices 0055 to 0060, that job found 3 survivors, all equivalent.
- The real gaps came from the verifier's hand mutations: 7 in slice 0056 and 2 in slice
  0058.

Meanwhile it cost about an hour of the builder's time per code slice, and a 20-minute merge
gate. The full run, 8 shards of 90 minutes, no longer finished.

**Changes:**
- **The builder runs no cargo-mutants.** It keeps targeted hand mutations of the new
  decision logic: match arms, check order, guards.
- **CI's PR-diff mutation job runs in parallel and does not block the merge.** It is not a
  required check.
  - The verifier judges on the tests, its hand mutations and whatever report is ready; a
    row no longer needs the report to PASS.
  - A survivor found later becomes follow-up work, fixed or triaged in a following slice.
- **The full `mutants.yml` run is a release gate,** no longer due at each milestone's close.
  - Before a release, every survivor is killed or triaged, and the publish waits for that.
  - The weekly run stays, as a report.
  - 0.1.0 is published without it, on CI's per-PR evidence (the maintainer, 2026-11-17); the
    gate applies from the next release.
- **More overlap.** While slice N is with the verifier, the next slice's tests may start,
  stacked on N's branch.

*Amendment 6 status: accepted (the maintainer, 2026-11-17).*

