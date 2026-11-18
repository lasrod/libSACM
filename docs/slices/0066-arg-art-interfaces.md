---
slice: 0066
name: arg-art-interfaces
state: in-progress
step: verification
rows: [SACM23-ARG-014, SACM23-ARG-015, SACM23-ART-012, SACM23-ART-013, LIB-EDIT-019]
mode: pr
base: main (fc59b20)
---

# Slice 0066 — Argument and artifact package interfaces and bindings

## Scope

M5's first slice. It covers:
- `ArgumentPackageInterface`, `ArgumentPackageBinding`, `ArtifactPackageInterface` and `ArtifactPackageBinding`:
  - read, as children and as roots, in both modes;
  - written;
  - created;
  - edited, compared, copied and moved.
- The ends `interface`, `implements` and `participantPackage`.
- Deleting a package an interface implements or a binding binds (LIB-EDIT-019).

Left for later:
- the Terminology and AssuranceCase kinds: slice 0067;
- validation of their content and placement, and cross-file references: slice 0068.

## Basis

- formal/23-05-08 §11.5, §11.6, §12.4 and §12.5; ptc/22-03-13.
- The M5 decisions Q1 to Q9 (the maintainer, 2026-11-18; `target/m5-decisions.md`).
- ADR 0039 (accepted 2026-11-18).
- The design brief, `target/slice-0066-design.md` (git-ignored).

## Design

The brief gives the API, the XMI shape, the check order, the delete algorithm and the golden samples. Its three open questions were decided by the lead within the maintainer's decisions:
1. **`messages-0.1.0.txt` may change for `golden/nested.golden.sacm.xmi` only.** That fixture now reads instead of being refused, which is a change of behaviour; no message's English changes. Record it in LIB-API-017's notes; `HEADER` stays.
2. **A missing `implements` gives no `validate` finding in this slice.** SACM23-VAL-014 is reworded in slice 0068 to cover it.
3. **An AssuranceCasePackage's or TerminologyPackage's `interface` end stays refused** until slice 0067.

## Log

- 2026-11-18 design: the designer's brief is saved. The five rows are analyzed, with LIB-EDIT-019 new, and ADR 0039 is filed as accepted.
- 2026-11-18 tests: the builder wrote 67 tests (all cited) and fixtures. Red confirmed by the lead:
  - 43 ungated tests fail, because a strict read refuses the four kinds and the `interface` end
    as `Unsupported` (Interim);
  - the gated tests fail to compile only for the missing API (E0599, E0432).
  - Snapshot red-0066.tar. Existing tests changed with their intent kept: the unread samples switch
    to TERM and AC kinds, and the oracles are extended (see the builder's list in the PR).
- Lead rulings on the design discrepancies:
  1. fixture M stays as designed; test 5 adds `e_note`;
  2. a tolerant `implements="a b"` follows today's single-valued rule (`repeat-kept`), so ARG-014
     acceptance 3 is reworded;
  3. `messages-0.1.0.txt` may gain `(none)` lines for the four new fixtures, because no English
     changes (note on LIB-API-017).
- 2026-11-18 implementation:
  - the builder's 39 hand mutants: 38 killed; M23 (an early return for `RejectIfReferenced`
    in `Tree::deleted`, already on main) survives;
  - the step-1 test `lib_edit_019_the_package_policy_guards_the_holder` was strengthened (three
    added cases, from the design's check-4 text);
  - lint-only edits to step-1 tests;
  - `xmi_roots.rs::sacm23_xmi_001_the_other_eight_package_kinds_are_unsupported_at_the_root` now
    checks four kinds; its name is kept, because slice 0067 removes the last four;
  - the lead reworded 11 verified rows (incl. XMI-001 "eight" → "four"), each with a slice 0066
    note.
