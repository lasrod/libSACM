# Verification 0062 — The Terminology model

- Date: 2026-11-18
- Row verdicts, all PASS: SACM23-TERM-001, SACM23-TERM-002, SACM23-TERM-003,
  SACM23-TERM-004, SACM23-TERM-005, SACM23-TERM-006, SACM23-XMI-028.
- Verifier: `sacm-verifier` (independent; read-only), two rounds
- Commits:
  - 684d874, round 1:
    - PASS on TERM-001 and -006;
    - TERM-003 to -005 passed on their content but were held by TERM-002;
    - FAIL on TERM-002 and XMI-028, each for one test gap.
  - aa5b2af, round 2: PASS. The CI run is cited in the PR. The PR-diff mutation job does not
    block (ADR 0006 amendment 6).
- Slice brief: [docs/slices/0062-terminology.md](../slices/0062-terminology.md)
- Decisions:
  - ADR 0034 amendment 3, D-EXPR-NAMES and spec defects D20 and D21;
  - the maintainer's M4 decisions of 2026-11-17, including that a Category's `category` end
    is documented with no direction.

## Result

- **The model:** TerminologyPackage, Category, Term, Expression and TerminologyGroup are read,
  written, created, edited, compared and validated, as the rows say.
  - Four new ends, with `category`'s clause depending on the holder: 10.8 on a Category,
    10.9 otherwise.
  - `value` and `externalReference`.
  - Seven operations.
- **The spelling:** `TerminologyElement` is read tolerantly with SACM-XMI-013, and kept
  strictly with an Error. Anywhere else it is ordinary content.
- **Direction (D21):** nothing documents a direction for a Category holder.
- **Quotations:** checked against formal/23-05-08 (§2.5, §8.4, §9.2, §10.2 to §10.11),
  ptc/22-03-13 Terminology and XMI 2.5.1 (§7.8.4, rule 4j).
- **The red snapshot,** compared with the final tests:
  - the goldens are unchanged;
  - the only test changes are the removed gates, lint attributes, one approved setup fix
    (TERM-004), two renames and the round-1 additions;
  - the re-samples keep their assertions.
- **Reworded verified rows** are true of the code:
  - XMI-001, -007, -014, -016;
  - ART-001, -010, -011;
  - LIB-API-009, LIB-API-015, LIB-COMPAT-003, LIB-RT-001.

  So are the note-only rows.

Evidence beyond CI: 40 hand mutations in round 1 (the end tables, per-kind clauses, the
placements, check order, the spelling, the missing-value rule, compare and write order). After
round 2, all are killed except M29, a redundant filter that is equivalent.

## Findings

| Severity | Row | Finding | Resolution |
|---|---|---|---|
| blocking | XMI-028 | No test of the spelling under the wrong package kind (M23 and its mirror survived). | Two cases: TerminologyElement in an ArgumentPackage, and argumentationElement in a TerminologyPackage. |
| blocking | TERM-002 | No test of SACM-REF-003's clause on a Category holder (M02 survived). | A case giving clause 10.8. |
| non-blocking | TERM-002 | The fixture comment and an ADR alternative implied a direction. D21's sentence about SACM 2.2 was inexact. | Reworded with the verifier's text. |
| non-blocking | TERM-002 to -006 | The `applies_to` and `ElementKind` docs, three lists in diagnostics.md, TERM-001's notes, and the README release line. | Reworded with the verifier's text. |

## Logged

- M29: the `.filter(!targets.is_empty())` in `plan.rs` (Features::Terminology) is redundant.
