---
slice: NNNN
name: short-name
state: in-progress   # in-progress | done | stopped
step: research       # research | design | tests | implementation | verification | close
rows: [ROW-ID, ROW-ID]
mode: pr             # pr | local
---

# Slice NNNN — <title>

## Scope

One paragraph: what this slice delivers, and what it deliberately leaves out.

## Basis

The SACM clauses it implements (formal/23-05-08 §…), or "quality row: acceptance by the
lead".

## Design

Public API added or changed, and any ADR. Omit this section if there is none.

## Log

- YYYY-MM-DD <step>: one line per event.

Keep the brief under one page.
