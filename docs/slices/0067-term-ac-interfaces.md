---
slice: 0067
name: term-ac-interfaces
state: done
step: close
rows: [SACM23-TERM-007, SACM23-TERM-008, SACM23-AC-002, SACM23-AC-003]
mode: pr
base: main (2aa2f37)
---

# Slice 0067 — Terminology and assurance case package interfaces and bindings

## Scope

- **The four kinds:** `TerminologyPackageInterface`, `TerminologyPackageBinding`, `AssuranceCasePackageInterface` and `AssuranceCasePackageBinding`.
  - They are read as children and as roots in both modes, written, created, edited, compared, copied and moved.
  - The `interface` end of a TerminologyPackage and an AssuranceCasePackage is read too.
- **Nothing is left unread.** After this slice, no SACM 2.3 kind or end is refused as not read yet. The unread-kind refusal machinery and its templates are removed (ADR 0035 amendment 1).
- **Not in this slice:** validation of the M5 rules and cross-file references, which are slice 0068's.

## Basis

- formal/23-05-08 §9.2, §9.3, §9.4, §10.4, §10.5 and §10.6; ptc/22-03-13.
- The M5 decisions Q1–Q9, ADR 0039 (D1 adds these pairs) and ADR 0035 amendment 1 (accepted 2026-11-18).
- The design brief, `target/slice-0067-design.md` (git-ignored).

## Log

- 2026-11-18 design: the brief is saved. The four rows are analyzed. ADR 0035 amendment 1 is filed as accepted. The four rows are added to ADR 0039's Rows line.
- 2026-11-18 tests: the builder wrote the tests and fixtures (TERM-007 15, TERM-008 13, AC-002 15, AC-003 13, all
  cited) and changed the existing tests the design lists, with intent kept. `interface_ends.rs` is deleted and its
  cases moved to TERM-007 #3 and AC-002 #3. 16 stale citations were dropped; the new tests of XMI-001, XMI-016,
  TERM-001, ART-001, LIB-API-017 and -018 are cited.
  - Red confirmed by the lead: 72 failures, all from the missing behaviour (the kinds are refused as
    Interim) or API. Snapshot red-0067.tar.
- Lead rulings:
  1. the four creates are not added to the edit_atomic create machinery (as for 0062's Terminology
     creates); a follow-up records it;
  2. a tolerant repeated `implements` follows 0066's rule; TERM-007 #3 is reworded.
- 2026-11-18 implementation: the builder's 20 hand mutants are all killed. Two test changes were accepted by the lead:
  1. in TERM-008 #1 and AC-003 #1 the bare-root binding input dropped `participantPackage="x y"` (a strict
     write correctly refuses the dangling ids); no assertion changed;
  2. the verified 0066 test `sacm23_arg_014_reference_edits_keep_implements_at_exactly_one` now asserts
     `WrongReferenceType` (not `FeatureNotApplicable`) for `interface` on a TerminologyPackage or AssuranceCasePackage,
     which this slice makes true; the Claim case is unchanged.
  The lead reworded the design's verified rows (plus SACM23-BASE-009 and LIB-API-018's counts), each with a slice 0067 note.
- 2026-11-18 verify (0bd2a7f): PASS on all four rows; the wording W1–W5 and the CHANGELOG item were fixed by the lead with
  the verifier's text. Record docs/verification/0067-term-ac-interfaces.md.
