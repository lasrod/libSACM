---
slice: 0065
name: expression-lang-string
state: in-progress
step: verification
rows: [SACM23-BASE-009, SACM23-VAL-009, SACM23-VAL-017]
mode: pr
base: slice/0063-refusal-codes
---

# Slice 0065 — ExpressionLangString and its rules

## Scope

- ExpressionLangString (§8.4) as a form of `LangString`: a name, or a value of a
  `content`, `key` or `location`, may link an ExpressionElement.
- `ReferenceEnd::Expression` is derived from those links, and listed by `referrers`.
- On delete: a scrub keeps the text and drops the link; a reject refuses.
- VAL-009: the link beside a non-empty content is a Warning.
- VAL-017: a concrete Expression must reference concrete elements.

Stacked on slice 0063, PR #84, which waits for the maintainer's approval.

## Basis

- formal/23-05-08 §8.4 and §10.10, ptc/22-03-13, spec defects D6 and D20.
- The maintainer's M4 decisions Q4 to Q7 of 2026-11-17.
- The design brief is `target/slice-0065-design.md` (git-ignored).

## Design

ADR 0038 and ADR 0030 amendment 4 are **proposed**. They record Q4 to Q7, plus three
details:
- the XMI shape;
- `EditError::DerivedEnd`;
- refusing an ExpressionLangString with no expression in an edit (the designer's open
  question).

The merge waits for the maintainer's approval. D-EXPR-CONTENT records Q7.

## Log

- 2026-11-18 choose: M4's last three rows. Slice 0064 (LIB-API-018) waits on ADR 0030
  amendment 3.
- 2026-11-18 analyzed: three rows from the design. Item 13 of BASE-009 is multi-line and was
  inserted by hand. ADR 0038 and ADR 0030 amendment 4 are proposed; D-EXPR-CONTENT is
  written.
- 2026-11-18 tests: `sacm-builder` step 1 wrote 39 tests and 5 fixtures.
  - The lead confirmed the red: under `cfg(slice_0065_api)` only missing API (E0599);
    without it, only the new rows, the golden-directory tests and the messages golden, which
    is red only because the new fixtures add lines. The snapshot is `red-0065.tar`, with
    `red-0065-golden/`.
  - Lead rulings:
    - the builder's interpretations of A9, A12 and VAL-017 A3 are accepted;
    - `edit_atomic` is not extended (A9 is covered by its own sequence);
    - the stale `lib_api_011_an_expression_lang_string_names_sacm23_base_009` is deleted,
      because its behaviour is gone, and its citation removed;
    - renames in step 2, with their citations updated.
- 2026-11-18 implementation: `sacm-builder` step 2.
  - 27 hand mutations killed; four new tests; one equivalent survivor, a dead filter,
    which is removed.
  - The five new diagnostics are built in the 0.1 style for slice 0064 to absorb.
  - Lead edits: citations (four renames and four new tests; LIB-API-011 tests[7] dropped);
    the nine verified rows made false, reworded with the design's text; ADR 0012, 0021,
    0029 and 0033 lines updated.
- 2026-11-18 verify round 1:
  - VAL-017 PASS; VAL-009 passes on its content but is held by BASE-009.
  - BASE-009 FAIL: 13 surviving mutants of the new logic, and false public docs. No wrong
    behaviour was found.
  - Fix round: the builder adds tests and fixes the rustdoc; the lead rewords LIB-EDIT-001,
    -008, -012, XMI-006, -007, LIB-RT-001, the VAL-017 A3 wording and two BASE-009
    requirement phrases, and completes the code lists; two follow-ups logged. All with the
    verifier's text.
