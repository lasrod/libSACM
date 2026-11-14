# Verification 0039 — Ids and literals the reader refuses today

- Date: 2026-11-14
- Row verdict: SACM23-VAL-006: PASS
- Row verdict: SACM23-XMI-020: PASS
- Row verdict: SACM23-XMI-021: PASS
- Row verdict: SACM23-XMI-022: PASS
- Row verdict: SACM23-XMI-025: PASS
- Re-checked, staying verified: SACM23-XMI-001, -004, -012, -014, -016, -017,
  SACM23-VAL-001, SACM23-VAL-020, LIB-RT-001
- Verifier: `sacm-verifier` (independent; read-only), 2 rounds; `sacm-docs-reviewer`, 1 round
- Commit: bf1bd5e. Evidence: CI run 43528225531 on 771c31d (PR-diff mutants: 60 caught,
  0 missed); round 2 added only tests and matrix text. Hand mutations: 22 in round 1, 2 in
  round 2.
- Slice brief: [docs/slices/0039-m2-ids-and-literals.md](../slices/0039-m2-ids-and-literals.md)

## Result

Five interim `Unsupported` refusals end, as ADR 0024 and ADR 0021 amendment 2 decide:
- a bad `assertionDeclaration` is `InvalidLiteral` (SACM-ENUM-001);
- a second `name` is `RepeatedValue`;
- an element with no `xmi:id` gets a generated, ordinary id, with the Info SACM-XMI-004;
- a padded id reads as its `xsd:ID` value, with the Info SACM-ID-004;
- a `name`'s `xmi:id` is kept, written, compared and kept unique.

Every quotation was checked against XMI 2.5.1, SACM 2.3 and ptc/22-03-13. Strict and
tolerant reads give the same result. Generation is deterministic and linear. Diagnostics
are located in one sorted pass.

## Findings

| Severity | Row | Finding | Resolution |
|---|---|---|---|
| BLOCKING | SACM23-XMI-025 | Copilot: a padded `xmi:id` after an `xsi:type` made locating quadratic (about 125 s for 20,000 Claims). | Located in one sorted pass (0.45 s), with a regression test. |
| BLOCKING | SACM23-XMI-022 | PR-diff mutant: `Difference::NameId` `Display` was untested. | Pinned by a test. |
| BLOCKING | SACM23-XMI-022 | H6: the generator's bound without the name ids survived. | A test where name ids outnumber the elements. |
| BLOCKING | SACM23-XMI-022, -004 | H36: a name id written unescaped survived. | Escaping tests under both rows. |
| BLOCKING | docs | The docs review found two stale claims (CHANGELOG, LangString "has no id"), plus 11 smaller items. | All fixed. |
| FOLLOW-UP | several | Wording on SACM23-XMI-001, -012, -VAL-001 and LIB-RT-001. | Reworded in round 2. |
| FOLLOW-UP | several | The id counters before deletes exist, the XML Schema source pin, `NAME_ATTRIBUTE_ORDER`, and the API ideas from the docs review. | In `docs/follow-ups.md`. |
