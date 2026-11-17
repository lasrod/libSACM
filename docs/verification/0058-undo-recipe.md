# Verification 0058 — Undo and redo by whole-document copies

- Date: 2026-11-17
- Row verdict: LIB-EDIT-011: PASS
- Verifier: `sacm-verifier` (independent; read-only), two rounds
- Commits:
  - 428f224, round 1: FAIL on one test gap. CI run
    [44027589623](https://github.com/lasrod/libSACM/actions/runs/44027589623) was green. The
    PR-diff mutants job had nothing to mutate, because only docs changed.
  - 34b8437, round 2: PASS. Its CI run 44030212469 was cancelled by the push of the close
    commit abecaff, whose run
    [44031049912](https://github.com/lasrod/libSACM/actions/runs/44031049912) is green on all
    jobs.
- Slice brief: [docs/slices/0058-undo-recipe.md](../slices/0058-undo-recipe.md)
- Decisions: ADR 0037 and D-UNDO, accepted by the maintainer on 2026-11-17.

## Result

- **No new public API.** The `edit` docs gain "# Undo and redo": a recipe whose `History`
  is the same, word for word, as the one `tests/undo.rs` uses. It does not panic at a limit
  of 0, and it pushes only on `Ok`.
- **Six tests pin what a restored copy is:**
  - it is `==` to the document before the change;
  - it writes the same XMI, with preserved content and part ids;
  - it has the same revision and generated-id counters;
  - a preview taken before an undone change applies again;
  - a refused change records no step;
  - an accepted draft is one step.
- **The red snapshot** was compared with the final tests: the only change is the approved
  `History` change, to a `VecDeque` with nothing recorded at a limit of 0. Round 2 only added
  checks.
- **ADR 0037 and D-UNDO** are true of the code, and the ADR names no local paths.

Hand mutations of the behaviour the tests pin:
- killed:
  - `Clone` resetting the id counters or the revision;
  - dropping preserved content, `other_ids` or name ids;
  - `apply_preview` ignoring the revision;
  - a refused edit renewing the revision;
  - the recipe without its limit guard, without `pop_front`, or without clearing redo;
- not killed by this row's tests: `==` ignoring preserved content, which the byte check
  covers for this row, and the compare tests cover elsewhere.

## Findings

| Severity | Row | Finding | Resolution |
|---|---|---|---|
| blocking | LIB-EDIT-011 | No test showed that a restored copy keeps its id counters: M1, a `Clone` that reset them, survived. | Test 1 gained a part where an undone `claim_2` is generated again, not the freed `claim_1`. M1 is killed. |
| non-blocking | LIB-EDIT-011 | Nothing tested that a new change after an undo clears redo (R3). | The doctest and the draft test check it. R3 is killed. |
| non-blocking | LIB-EDIT-011 | The cost figure did not match the measurement. ADR 0037 left out `location` and said "the next create" without "of the same kind". | Reworded with the verifier's text. |
