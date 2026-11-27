---
slice: 0090
name: named-ids
state: in-progress
step: tests
rows: [LIB-EDIT-026]
mode: pr
---

# Slice 0090 — Edits never remove an id that is still named

## Scope

Slice 0089's probes found that an accepted edit can make a strict-writable document
unwritable: deleting an element XMI metadata names, or removing a name's or text's id a
reference names. LIB-EDIT-026 makes such edits refuse, so the `xmi::write` round-trip
guarantee loses that exception. One row: a code change of the edit module, with a guarantee
test. Copies from a source a strict write refuses stay a documented limit.

Base: `main` (319210e).

## Basis

XML Schema Part 1 §3.3.4 clause 1; XMI 2.5.1 §7.6.2; SACM 2.3 §2.2. Design brief in the
session scratchpad (`design-0090-edit026.md`). ADR 0019 amendment 5, ADR 0030 amendment 10,
ADR 0042 amendment 3; D-VAL-LAYER.

## Log

- 2026-11-28 design: the lead decided the brief's three questions under the maintainer's
  direction of 2026-11-27 (refuse; metadata blocks the delete under every policy, an opt-in
  removal can come later; copies keep copying; the D-VAL-LAYER clause). Row `analyzed`.
- 2026-11-28 tests: freed_ids.rs (17 tests), the fixture, three JSON samples. Red: the two
  variants missing; probes show every case accepted today and the strict write then failing.
  Approved: the fixture's row comment, `xmi:documentation` (XMI.xsd's lowercase name; the
  schema job would refuse `Documentation`), `lang`s on d1's values; ids inside a deleted
  element's own metadata stay the delete policies'; `unrepresentable_text.rs`'s JSON check
  re-anchored. Row `tests-failing`.
- 2026-11-28 implementation: `edit/named.rs` (checks 5 and 6 of a delete, the text-set check,
  copy check 7), one IDREF-token walk shared with the strict write in `validation/metadata.rs`;
  `xmi::write` keeps two exceptions. 27 hand mutants, all killed after one approved added case
  (M5: a document-level `xmi:idref` naming an id in a deleted element's own metadata). One
  lint-only test edit (`end: *end` to `end`). Full gate green. LIB-DOC-015's acceptance drops
  the freed-id exception. Row `implemented`.
