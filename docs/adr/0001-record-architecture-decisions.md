# 0001. Record architecture decisions

- Status: accepted
- Date: 2026-11-07

## Context

libSACM is a public library developed largely by agent roles working in short slices.
Neither a new contributor nor a fresh agent session can recover *why* the code is the
way it is from the code alone, and an agent that cannot see a decision will
eventually undo it.

## Decision

Record each decision with lasting consequences as a numbered ADR in `docs/adr/`, using
`template.md`. Interpretations of the SACM standard go in `docs/sacm/decisions.md`
instead, because they are about the specification rather than about this code.

## Consequences

Agents read the ADR index before designing. A change that contradicts an accepted ADR
must supersede it explicitly.
