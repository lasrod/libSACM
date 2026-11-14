# Verification 0042 — References, citations and abstract forms

- Date: 2026-11-14
- Row verdict: SACM23-XMI-005: PASS
- Row verdict: SACM23-XMI-015: PASS
- Row verdict: SACM23-BASE-007: PASS
- Row verdict: SACM23-BASE-008: PASS
- Row verdict: SACM23-VAL-003: PASS
- Row verdict: SACM23-VAL-007: PASS
- Row verdict: SACM23-VAL-008: PASS
- Re-checked, staying verified: SACM23-XMI-001, -002, -010, -016, -020, SACM23-VAL-001,
  LIB-API-009, LIB-SEC-002, LIB-COMPAT-001, LIB-DIAG-002, LIB-EDIT-001
- Verifier: `sacm-verifier` (independent; read-only), 2 rounds; `sacm-docs-reviewer`, 1 round
- Commit: 658ccce. Evidence: CI run 43576504257 on f9d0688 (PR-diff mutants: 122 caught,
  0 missed); round 2 changed only tests and one doc comment. Hand mutations: 28 in round
  1, 5 re-applied in round 2. Three equivalent mutants are excluded, with triage rows owned
  by SACM23-ARG-004 and LIB-EDIT-009.
- Slice brief: [docs/slices/0042-m2-references.md](../slices/0042-m2-references.md)

## Result

References between elements are held as unresolved ids (ADR 0026). They are read from the
IDREFS attribute and from the element form (`xmi:idref`, `href="#id"`), and written as
attributes.
- A reference naming no element is kept and reported as SACM-REF-001.
- Generated ids skip referenced ids.
- `isCitation` and `isAbstract` are xsd:booleans defaulting to false.
- `validate` checks:
  - the §8.2 citation OCL in its one direction (SACM-CITE-001);
  - the abstract-form rules: an Error for an abstract element using an `abstractForm`, and
    Warnings for the target (SACM-ABS-001).

Every quotation was checked against SACM 2.3 §8.2, ptc/22-03-13 and XMI 2.5.1. Nothing was
dropped, and no input panicked.

## Findings

| Severity | Row | Finding | Resolution |
|---|---|---|---|
| BLOCKING | XMI-015 | Copilot: an `xmi:idref` holding white space read as one id but wrote back as two. | Refused, with a regression test. |
| BLOCKING | XMI-005, BASE-007/008, VAL-003 | PR-diff mutants: 13 missed (2 test gaps, 8 in unreachable many-valued and ordering code, 3 equivalent). | Pinned, removed, or excluded with triage rows. |
| BLOCKING | BASE-007/008, XMI-005, -015 | Round 1: M04 (Unicode trim of a Boolean), M09-M11 (the id pre-pass for `href` and trimmed links), M17 (one location per token). | Tests kill each. |
| BLOCKING | docs | Docs review: the citation rule was stated on the cited element, plus 16 smaller items. | All fixed. |
| FOLLOW-UP | XMI-015, XMI-005 | Proxy reference elements with other attributes (§7.10.1) are refused; a huge `RepeatedValue` message. | ADR 0026 known gap; `docs/follow-ups.md`. |
