---
slice: 0029
name: m1-read-package-content
state: done
step: verified
rows: [SACM23-XMI-010, SACM23-XMI-016, LIB-API-009, SACM23-XMI-001]
mode: pr
base: slice/0028-m1-create-operations (PR #39)
---

# Slice 0029 — Reading package content

## Scope

`xmi::read` reads the containment children of packages, under their role elements and with
their type attributes, into the same kind, parent and order that `xmi::write` writes. A
document built with `edit::apply` then reloads. Every diagnostic produced while reading
carries a source location. Content the model cannot hold yet stays `Unsupported` (ADR 0015).
That includes references, utility content and preserved content. Research decides the exact
set.

## Basis

formal/23-05-08 (the composite roles in clauses 9 to 12); XMI 2.5.1 §7.8.6 and §7.6.3;
ADR 0010, ADR 0015, ADR 0016, ADR 0017.

## Log

- 2026-11-12 choose: the maintainer's choice of reading over LIB-EDIT-002/003. XMI-010 and
  XMI-016 move from M2 to M1, because edited documents must reload and M1's round-trip rows
  need them. Base: `slice/0028-m1-create-operations` (PR #39).
- 2026-11-12 research. Lead decisions, each the minimal option within existing policy:
  - M1: only ACP, AP and Claim children are read (verified kinds, ADR 0016 D5).
  - M2: a non-conforming child is `Unsupported`, by ADR 0015's interim rule, as ADR 0016 D6 does for roots.
  - M3: `assertionDeclaration` stays with XMI-017.
  - M4: namespace-qualified role elements are refused, as for `name`.
  - M5: XMI-010 is split. The M1 part is "every refusal is located"; read-diagnostic locations become the new XMI-023 (M2).
  - VAL-006 moves to M2 with XMI-017.
  - The `argumentationElement` spelling (from the §11.4 text, not the model) is fixed in XMI-009 and the skill.
- 2026-11-12 tests: 14 XMI-016 tests and 10 XMI-010 tests. The red is confirmed: 11 XMI-016 tests fail; XMI-010 already pins existing behaviour except 8 child rows. The lead reworded XMI-010 items 1 and 2; agreeing type attributes read. LIB-API-009 is re-checked here, because its sample's refusal location moves.
- 2026-11-12 implementation: children are read iteratively, held pending and inserted in document order. Two behaviour notes for the verifier: a duplicate root id is refused earlier, and `check_text` now refuses white-space-only CDATA and character references. XMI-001 is reworded and re-checked here. The local gate passes.
- 2026-11-12 verification round 1: LIB-API-009 PASS; XMI-016, XMI-010 and XMI-001 FAIL.
  - High: `check_text` refused schema-conforming white space written as CDATA or character references. The fix is to accept it; XMI-016 gains an item for it.
  - Unpinned orders: M23 (a duplicate root id) and M12/M13 (a duplicate child id).
  - XMI-010 text-location wording; the `ReadError` docs on locations; a C++ difference note on qualified role elements; the D6 citations become ADR 0015.
- 2026-11-13 verification round 2: PASS at c6260e4. XMI-016 and XMI-010 are verified; XMI-001 and LIB-API-009 are re-verified.
