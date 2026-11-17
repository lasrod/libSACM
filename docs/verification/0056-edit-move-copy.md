# Verification 0056 — Move and copy elements, referrers, flag and end readers

- Date: 2026-11-17
- Row verdict: LIB-EDIT-017: PASS
- Row verdict: LIB-EDIT-018: PASS
- Row verdict: LIB-API-014: PASS
- Row verdict: LIB-API-015: PASS
- Verifier: `sacm-verifier` (independent; read-only), two rounds
- Commits:
  - 51f999d and 56f0a18, round 1. Evidence: CI run
    [43968011034](https://github.com/lasrod/libSACM/actions/runs/43968011034), green. PR-diff
    mutants: 141 caught, 40 unviable (`Default` and `ControlFlow` returns), 0 missed, 0
    timed out.
  - f2dd5aa, round 2: tests and wording only, with no library code changed. Evidence: CI
    run [43983753114](https://github.com/lasrod/libSACM/actions/runs/43983753114), green, with
    the same mutant counts.
- Slice brief: [docs/slices/0056-edit-move-copy.md](../slices/0056-edit-move-copy.md)
- Decisions: ADR 0036, ADR 0029 amendment 1 and ADR 0030 amendment 2, all accepted by the
  maintainer on 2026-11-17.

## Result

- **Moves:** `MoveElement` moves an element, with its subtree and preserved content, under
  another parent or to the roots. Its check order, its three new refusals and its changed
  ids are the ones ADR 0036 D1 gives.
- **Copies:** `CopyElements` copies elements from another document.
  - Free ids are kept; the others are generated.
  - References to copied elements name the copies.
  - Preserved content is copied exactly, or the copy is refused as `DuplicateId`.
  - A copy is not serialized (ADR 0030 amendment 2).
- **Referrers:** `Document::referrers` equals a full scan after every operation of the set,
  in document pre-order.
- **Flag and end readers:** `SacmElement::flag` and `references`, `Flag::ALL` and
  `applies_to`, and `ReferenceEnd::ALL`, `applies_to` and `bounds` read the tables that the
  reader, the writer and the edits use.
- **A random probe:** 4,000 moves and copies and 1,500 batches agreed with
  `semantic_compare`, with a full scan of referrers, and with a write and read back.
- **The red snapshot,** compared with the final tests: no step-1 test was weakened. The only
  changes are the gates, the one assertion the brief logs, and added tests.
- **Reworded verified rows:** the texts of LIB-API-006, LIB-API-008 and LIB-EDIT-014, and
  the notes on LIB-EDIT-002, -003 and -013, are true of the code.

Evidence beyond CI:
- More than 50 hand mutations of the check orders, the subtree walk, the slot bookkeeping,
  preserved placement, id generation and the referrer index. After round 2, all are killed,
  except the ones shown to be equivalent.
- Three CI survivors are equivalent and excluded in `.cargo/mutants.toml`, each with a row in
  [mutants-triage.md](../quality/mutants-triage.md):
  - `move_changes` `<` → `<=` at 1627:41 and at 1634:71;
  - `release_holder` → `()`.

## Findings

| Severity | Row | Finding | Resolution |
|---|---|---|---|
| blocking | LIB-EDIT-018 | A preserved `xmi:id` that a reference names was not tested as refused (H20b). | `lib_edit_018_a_preserved_id_a_reference_names_is_refused_as_duplicate_id` |
| blocking | LIB-EDIT-018 | Preserved content naming a dropped part id was not tested (H21). | `lib_edit_018_preserved_content_naming_a_dropped_part_id_is_refused_as_duplicate_id` |
| blocking | LIB-EDIT-018 | Copied preserved references joining the index were not tested (H34). | `lib_edit_018_a_copied_preserved_reference_is_avoided_by_later_generated_ids` |
| blocking | LIB-API-014, LIB-EDIT-017 | `compact` renumbering slots was not tested (H44, H44b). | `lib_api_014_referrers_follow_pre_order_after_a_root_delete_and_a_later_create`, `lib_edit_017_a_move_to_the_place_it_holds_after_a_delete_changes_nothing` |
| blocking | LIB-API-014, LIB-EDIT-018 | The slot of a root added by a copy was not tested (H51). | `lib_api_014_referrers_follow_pre_order_into_a_root_copy` |
| blocking | LIB-EDIT-017 | Document-level preserved content on root moves was not tested (H52, H53). | `lib_edit_017_a_root_move_keeps_the_document_content_after_the_root_it_followed` |
| non-blocking | LIB-API-006, LIB-EDIT-018, LIB-API-015 | Matrix wording: the CopyElements exception broke a list; part ids are kept only as free NCNames; the §12.14 bounds note. | Reworded with the verifier's text. |
| non-blocking | LIB-EDIT-018 | Rustdoc: part ids, the gid sentence, `CopyElements::root` in `ElementNotFound`, and `DuplicateId` on copies. | Reworded with the verifier's text. |

## Logged

- The two `move_changes` exclusions are pinned to their line and column. A later edit there
  makes them stop applying, so they fail safe.
- No test covers LIB-API-014's claim that `referrers` costs time in proportion to the
  referrers, not to the document.
