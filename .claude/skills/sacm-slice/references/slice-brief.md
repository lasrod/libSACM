---
slice: NNNN
name: short-name
state: in-progress   # in-progress | done | stopped
step: research       # research | design | tests | implementation | verification | close
rows: [ROW-ID, ROW-ID]
mode: local          # local | pr
---

# Slice NNNN — <title>

## Scope

What this slice delivers, in one paragraph a reviewer can hold in their head.

| Row | Title | Status at start |
|---|---|---|

## Out of scope

What a reader might expect here that is deliberately left to a later row.

## Normative basis

Key clauses and quotations from the research report (formal/23-05-08 §…).

## Design

Public API added or changed (signatures from the design brief), and any ADR.

## Tests

| Test | Row | Criterion |
|---|---|---|

## Log

- YYYY-MM-DD research: …
- YYYY-MM-DD design: …
- YYYY-MM-DD tests: …
- YYYY-MM-DD implementation: …
- YYYY-MM-DD verification: PASS/FAIL …
