---
slice: 0045
name: m2-set-operations
state: in-progress
step: verification
rows: [LIB-EDIT-008, LIB-EDIT-009, LIB-EDIT-010, LIB-EDIT-012]
mode: pr
base: main
---

# Slice 0045 — Set operations

## Scope

The edit operations that change an existing element:
- **LIB-EDIT-008:** attribute edits. These cover names, descriptions, notes, tagged values,
  `gid`, the citation and abstract flags, assertion declarations and `isCounter`.
- **LIB-EDIT-009:** reference edits, which set, add and remove a reference end's ids. They
  refuse a dangling or wrongly typed target, as ADR 0028 does.
- **LIB-EDIT-010:** reordering a package's contained elements.

Delete (LIB-EDIT-004/005) is not part of this slice.

## Basis

- The edit model: ADR 0019, ADR 0020 and D-VAL-LAYER.
- The reference ends: ADR 0026, ADR 0027 and ADR 0028.
- SACM 2.3 §8 and §11, for the features that are edited.
- The C++ library's edit commands, where the standard is silent.

## Design

[ADR 0029](../adr/0029-set-reference-and-reorder-operations.md), accepted by the maintainer on 2026-11-15; ADR 0019 amendment 2 (before-values to LIB-EDIT-011).
- API: `edit::{SetName, SetGid, SetFlag, SetAssertionDeclaration, SetContent, SetKey, SetReferences, AddReference, RemoveReference, ReorderElements}`.
- `model::{Flag, ReferenceEnd}` become public; `name()` is their only public method.
- New `EditError` variants: `ElementNotFound`, `FeatureNotApplicable` (SACM-CMD-008), `TooManyReferences`, `ReferenceNotHeld`, `ChildNotFound`, `InvalidOrder` (SACM-CMD-009).
- New codes: `Code::{CMD_FEATURE_NOT_APPLICABLE, CMD_INVALID_ORDER}`.

## Log

- 2026-11-15 choose: the edit operations on existing elements. LIB-EDIT-009 unblocks
  LIB-RT-004, LIB-API-005, LIB-DOC-003 and LIB-EDIT-011.
- 2026-11-15 research: rows analyzed; LIB-EDIT-012 (UtilityElement content and key) split from LIB-EDIT-008. The maintainer decided: new codes SACM-CMD-008 (kind without the feature) and SACM-CMD-009 (bad order list); citation flag and reference set independently; reorder any element's children and the roots; before-values moved to LIB-EDIT-011. Lead: ids kept by position; remove drops every occurrence.
- 2026-11-15 design: ADR 0029 accepted; D-RESULT and D-PREVIEW reworded; matrix wording aligned.
- 2026-11-15 tests: 86 failing tests over the 4 rows plus the extended LIB-EDIT-002/003 set (red on the missing API only); re-check notes on LIB-EDIT-002, -003, LIB-DIAG-002, LIB-RT-003, SACM23-BASE-006.
- 2026-11-15 implement: ADR 0029 built; one clippy fix in the tests; the `changed_ids` mutant exclusion removed, `IdUse::is_unused` kept as equivalent with a new reason; gate green (1815 tests).
