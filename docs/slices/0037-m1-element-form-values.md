---
slice: 0037
name: m1-element-form-values
state: in-progress
step: verification
rows: [SACM23-XMI-014, SACM23-XMI-012, SACM23-XMI-017, SACM23-XMI-010, SACM23-XMI-016, LIB-API-009, LIB-API-010, LIB-RT-001]
mode: pr
base: main
---

# Slice 0037 — Element form of values, and the follow-ups

## Scope

The last M1 row, SACM23-XMI-014. Import reads the XML element form of the single-valued
values the reader holds today, as well as the attribute form:
- a `name`'s `lang` and `content`;
- a Claim's `assertionDeclaration`.

Both forms load to an equal model, and export writes the attribute form. `gid`,
`isCitation` and `isAbstract` follow with their own rows.

The slice also clears `docs/follow-ups.md`, which is due at the milestone close
(ADR 0006 amendment 3).

## Basis

XMI 2.5.1 §7.8.4, §9.4.1 and rule 2h; ADR 0021; ADR 0022.

## Design

ADR 0021 amendment 1 (accepted). Value elements for `name.lang`, `name.content` and
`Claim.assertionDeclaration` are read. A value given twice is refused as
`ReadError::RepeatedValue { element, property, locations }` (SACM-MULT-001), even when both
values are equal. The maintainer chose the same refusal for a second `name`
(SACM23-XMI-021). New row SACM23-XMI-026 covers `xsi:nil`.

## Log

- 2026-11-13 choose: the last M1 row, with the follow-ups batch.
- 2026-11-13 design: research and design in parallel (retried after API 403 errors); the
  maintainer chose to refuse repeated values and a second name.
