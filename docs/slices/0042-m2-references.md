---
slice: 0042
name: m2-references
state: in-progress
step: verification
rows: [SACM23-XMI-005, SACM23-XMI-015, SACM23-BASE-007, SACM23-BASE-008, SACM23-VAL-003, SACM23-VAL-007, SACM23-VAL-008]
mode: pr
base: main
---

# Slice 0042 — References, citations and abstract forms

## Scope

References between elements, proven on the first properties that hold them:
- **SACM23-XMI-005:** non-containment references are written as IDREFS attributes and
  read back. A reference to an unknown id is kept and diagnosed, never dropped.
- **SACM23-XMI-015:** references are also read in the element form (XMI 2.5.1 rule 2c), to
  an equal model.
- **SACM23-BASE-007:** `isCitation` and `citedElement` on every SACMElement.
- **SACM23-BASE-008:** `isAbstract` and `abstractForm` on every SACMElement.

The 13 rows that depend on SACM23-XMI-005 (the argumentation relationships, artifacts,
terminology) build on this.

## Basis

XMI 2.5.1 §7.10 and rules 2a-2i; SACM 2.3 §8.2; ADR 0010, ADR 0020, ADR 0024; D-ID,
D-PRESERVE.

## Design

[ADR 0026](../adr/0026-references-between-elements.md), accepted by the maintainer on 2026-11-14, with D-VAL-LAYER reworded. API: `SacmElement::{is_citation, cited_element, is_abstract, abstract_form}`; `Difference::{IsCitation, IsAbstract, CitedElement, AbstractForm}`; `Code::{REF_DANGLING, CITATION_INVALID, ABSTRACT_FORM_INVALID}`. Validation rows VAL-003, -007 and -008 join the slice. New row SACM23-XMI-027 owns cross-document links.

## Log

- 2026-11-14 choose: SACM23-XMI-005 is the M2 critical path (13 rows depend on it); paired
  with the first properties that hold references.
