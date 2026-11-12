---
slice: 0030
name: m1-assertion-declaration-in-xmi
state: in-progress
step: verification
rows: [SACM23-XMI-017, LIB-API-010, SACM23-XMI-016]
mode: pr
base: slice/0029-m1-read-package-content (PR #40)
---

# Slice 0030 — assertionDeclaration in XMI

## Scope

`xmi::read` reads a Claim's `assertionDeclaration` attribute: all six literals, with an
explicit `asserted` the same as an absent one. `xmi::write` writes every non-default
declaration instead of refusing it. Real files that state the declaration then load, and
every Claim round-trips. LIB-API-010 loses its only refusal case, so it is reworded and
re-checked. Left out:
- the element form of the value (SACM23-XMI-014);
- reporting an unknown literal as a coded diagnostic (SACM23-VAL-006, M2). Until then an
  unknown literal is `Unsupported` (ADR 0015) and never mapped to the default.

## Basis

formal/23-05-08 §11.10 (`assertionDeclaration:AssertionDeclaration[1] = asserted`) and
§11.4; XMI 2.5.1 §9.4.1 and §7.8.4; ADR 0015, ADR 0017.

## Design

ADR 0017 amendment 2 (accepted). No public read or edit API is added. The writer writes
`assertionDeclaration="<literal>"` after `xmi:id` for every non-default declaration on the
six Assertion kinds. `WriteError::Unsupported` is removed, and its shape is reserved. The
reader matches the six literals exactly, after XML attribute-value normalisation. Tests
build non-default Claims with the `test-support` seam `Document::set_assertion_declaration`.

## Log

- 2026-11-13 choose: the maintainer's choice of XMI-017 over LIB-EDIT-002/003 and
  LIB-RT-001/002, because the reader refuses most real files at their first Claim.
  XMI-017 moves from M2 to M1. VAL-006 stays in M2.
- 2026-11-13 research and design: XMI-017 analyzed; LIB-API-010 reworded ("The writer never
  drops content"); XMI-016's attribute refusal narrowed to non-Assertion children. Both
  verified rows are re-checked in this slice. ADR 0017 amendment 2 accepted by the
  maintainer.
