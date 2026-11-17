---
slice: 0056
name: edit-move-copy
state: done
step: close
rows: [LIB-EDIT-017, LIB-EDIT-018, LIB-API-014, LIB-API-015]
mode: pr
base: main
---

# Slice 0056 — Move and copy elements, referrers, flag and end readers

## Scope

What an editor needs before M4 and M5:
- moving an element under another parent or to the roots;
- copying elements from another document (accepting a draft, GitHub issue #61);
- listing the references that name an element;
- public flag and reference-end readers for a generic property panel.

Left out: undo (LIB-EDIT-011), and any serialized form of a copy.

## Basis

Library rows: the containment table (LIB-API-008, ADR 0014), XMI 2.5.1 §7.6.1 for id
uniqueness, and formal/23-05-08 §8.2 (`gid`), §11.13 and §12.14 for the end bounds.

## Design

- The design brief is `target/slice-0056-design.md` (git-ignored).
- ADR 0036, with ADR 0029 amendment 1 and ADR 0030 amendment 2, was accepted by the
  maintainer on 2026-11-17. It settles the design's open questions 1 to 7.

New public API:
- `edit::MoveElement` and `edit::CopyElements`, with `Operation` variants;
- the `EditError` variants `InvalidRoot`, `ParentInSubtree` and `PositionOutOfRange`;
- `Document::referrers` and `model::Referrer`;
- `SacmElement::flag` and `SacmElement::references`;
- `Flag::ALL` and `Flag::applies_to`;
- `ReferenceEnd::ALL`, `ReferenceEnd::applies_to` and `ReferenceEnd::bounds`.

No new diagnostic codes.

## Log

- 2026-11-16 choose: the four rows the review moved to M3 as consumer needs.
- 2026-11-16 design: done by `sacm-designer`. The row texts changed:
  - the Mutation follows LIB-EDIT-003;
  - a copy takes a list, and "Strategy" was dropped (D-TERM);
  - the bounds take the kind;
  - batch net ids also take the parents.
- 2026-11-17 analyzed: the design text was applied to the matrix, and the ADRs were written
  as proposed.
- 2026-11-17: the maintainer accepted ADR 0036.
- 2026-11-17 tests: `sacm-builder` step 1 wrote 42 tests, gated with `cfg(slice_0056_api)`.
  - The lead confirmed the red: under the cfg, seven test crates fail to compile, and only
    on the missing API. The snapshot is `red-0056.tar` in the session scratchpad; the
    golden JSON drafts are in `red-0056-golden/` beside it.
  - The golden files in the tree stay unchanged until step 2, because `record.rs` unit
    tests read them.
  - Lead decisions:
    - LIB-EDIT-018 drops "a reference holding white space": a typed end never holds one,
      so the check would be dead code.
    - An empty copy under an unknown parent is `ParentNotFound`. Step 2 adds that one
      assertion to `lib_edit_018_refusals_come_in_order_and_change_nothing`.
- 2026-11-17 implementation: `sacm-builder` step 2. The 42 step-1 tests pass unchanged,
  except for that one assertion. The builder added 6 tests, which kill the mutants they
  name; all 48 are cited.
  - The builder holds these mutants equivalent:
    - `document.rs` `move_changes` `<` → `<=` ×2: the two slot paths belong to
      different elements, so they are never equal;
    - `release_holder` → `()`: a stale holder adds no referrer;
    - the hand mutant "D4 takes only the parent before": a move away and back has
      the same parent before and after. D4 keeps both parents, as the design says.
  - Reworded verified rows:
    - LIB-API-006: `CopyElements` is not serialized and has no golden.
    - LIB-API-008: moves and copies check the containment table in their plans.
    - LIB-EDIT-014: the random sequences hold no moves or copies; targeted tests cover
      those.
    - LIB-EDIT-002, -003 and -013: notes only.
- 2026-11-17 CI on 51f999d: PR-diff mutants missed the three the builder holds
  equivalent. 56f0a18 excludes each by its exact name, with a triage row. CI on 56f0a18
  is green (141 caught, 0 missed).
- 2026-11-17 verify round 1, by `sacm-verifier`: LIB-API-015 PASS; FAIL on LIB-EDIT-017,
  LIB-EDIT-018 and LIB-API-014.
  - No behaviour was wrong. The fails are seven hand mutants (H20b, H21, H34, H44/H44b,
    H51, H52/H53) that no test kills.
  - The verifier agreed that the three excluded mutants are equivalent.
  - Fix round: `sacm-builder` adds the seven named tests and fixes the rustdoc wording,
    using the verifier's text. The lead rewords LIB-API-006, LIB-EDIT-018 and LIB-API-015
    in the matrix, also with the verifier's text.
- 2026-11-17 verify round 2: PASS on all four rows. Each of the seven new tests kills its
  mutant, and CI on f2dd5aa is green. The record is
  `docs/verification/0056-edit-move-copy.md`; the rows are verified, and M3 is at 16/20.
