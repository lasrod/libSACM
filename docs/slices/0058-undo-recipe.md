---
slice: 0058
name: undo-recipe
state: in-progress
step: verification
rows: [LIB-EDIT-011]
mode: pr
base: main
---

# Slice 0058 — Undo and redo, by whole-document copies

## Scope

Undo and redo stay the application's job. The `edit` docs show a tested recipe: clone
before each atomic change, and restore the clone with `std::mem::replace`. Tests pin what a
restored copy is: an equal document, the same XMI, the same revision and id counters, and
previews that apply again.

There is no new public API, no history type and no inverse operations. One row only: it
is the last library row of M3, and the release (LIB-QA-005) is a separate tooling slice.

## Basis

Library row: the standard is silent. ADR 0037 and D-UNDO, decided by the maintainer on
2026-11-17 after a survey of comparable libraries. The design and measurements are in
`target/lib-edit-011-design.md` (git-ignored).

## Log

- 2026-11-17 choose: the last library row of M3.
- 2026-11-17 design: by the lead, because the row adds no public API. It follows the
  designer's research. ADR 0037 is written as accepted, and D-UNDO replaces Q-UNDO.
  The row is `analyzed`.
- 2026-11-17 tests: `sacm-builder` step 1 wrote `tests/undo.rs`, six tests. All six pass
  before step 2, as expected: the row commits to behaviour `Clone` already has, and the
  tests pin it. Only item 7, the docs section and its doctest, is red. The snapshot is
  `red-0058.tar`.
  - Lead decision: the recipe panicked with a limit of 0 (`Vec::remove(0)` on an empty
    stack). It becomes a `VecDeque`: `pop_front` when full, and nothing recorded at limit
    0. Step 2 changes the `History` in `undo.rs` to match the docs. That is the only
    approved change to a step-1 test.
- 2026-11-17 implementation: `sacm-builder` step 2 wrote the "# Undo and redo" section with
  its doctest, the `batch.rs` link, the README sentence and the CHANGELOG line, and changed
  the `History` in `undo.rs` (the approved change). There is no library code.
- 2026-11-17 verify round 1: FAIL. Blocking: no test showed that a restored copy keeps its
  id counters (the verifier's mutant M1). Non-blocking: redo clearing was untested (R3);
  wording. Fix round: `sacm-builder` adds the counter part to test 1 and the redo-clear
  checks, and fixes the cost wording. The lead extends acceptance 1 and rewords ADR 0037
  items 2 and D2, all with the verifier's text.
