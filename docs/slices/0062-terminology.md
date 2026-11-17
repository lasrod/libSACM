---
slice: 0062
name: terminology
state: in-progress
step: verification
rows: [SACM23-TERM-001, SACM23-TERM-002, SACM23-TERM-003, SACM23-TERM-004, SACM23-TERM-005, SACM23-TERM-006, SACM23-XMI-028]
mode: pr
base: main
---

# Slice 0062 — The Terminology model

## Scope

The Terminology kinds of clause 10 (TerminologyPackage, Category, Term, Expression and
TerminologyGroup) are read, written, created, edited, compared and validated. The §10.4
spelling `TerminologyElement` is read tolerantly. This lets the first consumer's sample
files, which hold a terminology package, open in strict mode.

Left out:
- ExpressionLangString and its rules (slice 0064);
- the interface and binding kinds (M5).

## Basis

- formal/23-05-08 §10.2 to §10.11, ptc/22-03-13 Terminology, spec defects D9 and D21.
- The maintainer's M4 decisions of 2026-11-17: Q1 to Q14 as recommended, with the docs
  neutral on Category's `category` direction.

## Design

- The design brief is `target/slice-0062-design.md` (git-ignored). Part A holds the
  decisions; Part B is the design.
- ADR 0034 amendment 3 records the design as accepted.

## Log

- 2026-11-17 choose: M4's first slice. The designer researched M4's decisions and designed
  this slice; a second designer researched SACM 2.4 Beta1 and GSN on SACM
  (`target/research-sacm24-gsn.md`).
- 2026-11-17 decisions: the maintainer accepted all the recommendations. D20 is an Error.
  #82 breaks `code()`, so the next release is 0.2.0. LIB-COMPAT-004 stays in M6, split.
- 2026-11-17 analyzed:
  - the seven rows hold the design's text;
  - recorded: ADR 0034 amendment 3, D-EXPR-NAMES, and spec defects D20 and D21;
  - LIB-COMPAT-004 is split; new rows LIB-COMPAT-017 (candidate) and -018 (deferred);
    CP-003 now depends on VAL-017.
- 2026-11-17 tests: `sacm-builder` step 1 wrote 112 tests (46 gated with
  `cfg(slice_0062_api)`) and 4 fixtures and goldens.
  - The lead confirmed the red: under the cfg, only the missing API (E0599, E0432);
    without it, only the terminology rows and the expected golden-directory and
    readable-kind tests. The snapshot is `red-0062.tar`, with `red-0062-golden/`.
  - Lead ruling, XMI-028 item 4: `TerminologyElement` behaves as `argumentationElement`
    does. A tolerant read with a kind the role does not admit gives SACM-XMI-003; an
    untyped spelled role gives only SACM-XMI-005.
  - Re-samples: the unread examples are now the interface and binding kinds
    (TERM-007/-008, ART-012).
- 2026-11-17 implementation: `sacm-builder` step 2. All 112 tests pass unweakened.
  - One setup fix to a step-1 test: `…the_existing_edits_apply_to_a_term` now clears
    `t1`'s `citedElement` before deleting `t2`. Assertions are unchanged.
  - Renames: `…the_other_eight_package_kinds…` and `…reference_end_all_lists_these_two…`,
    with their citations updated.
  - Verified rows reworded with the design's text: XMI-001, XMI-007, XMI-014, XMI-016,
    ART-001, ART-010, ART-011, LIB-API-009, LIB-API-015, LIB-COMPAT-003 and LIB-RT-001.
    Note-only: LIB-API-011, LIB-COMPAT-001, -007, -015, XMI-002, -008, -018, VAL-001,
    LIB-RT-002 and LIB-SEC-002/005/006.
  - Hand mutations: 26 of 27 killed; the survivor was a dead bound check, now removed.
