---
slice: 0031
name: m1-semantic-comparison
state: done
step: verified
rows: [LIB-RT-001, LIB-RT-002, LIB-RT-003]
mode: pr
base: slice/0030-m1-assertion-declaration-in-xmi (PR #41)
---

# Slice 0031 — Comparing documents

## Scope

A public function compares two documents by their SACM content and lists every difference,
naming the element id and the feature. It ignores everything a `Document` does not hold:
formatting, attribute order, prefixes, and the state of generated ids. With it, two
round-trip guarantees become checkable:
- loading, saving and reloading every valid fixture gives an equal document (LIB-RT-002);
- a document built with the edit API saves, reloads, validates without errors and equals
  the original (LIB-RT-003).

Left out: the atomic-edit rows (LIB-EDIT-002/003), which can use the comparison next.

## Basis

Quality rows: acceptance by the lead. D-RT ("Semantic round trip … is guaranteed"),
ADR 0010 (order), ADR 0012 and 0013 (names).

## Design

ADR 0020 (accepted). The new module `sacm::compare` has:
- `semantic_compare(left: &Document, right: &Document) -> Vec<Difference>`;
- `#[non_exhaustive] enum Difference { RootOrder, LeftOnly, RightOnly, Kind, Parent, Name,
  AssertionDeclaration, ChildOrder }`, with public fields, `element_id()` and one-line
  `Display`;
- `impl PartialEq + Eq for Document`, meaning that `semantic_compare` finds nothing.

Elements are matched by id. The generated-id counters are not compared.

## Log

- 2026-11-13 choose: the maintainer's choice of comparison over the M1 edit rows and over
  changing a loaded Claim, because it unblocks M1's remaining round-trip and editing rows.
- 2026-11-13 design: ADR 0020 accepted by the maintainer. Acceptance for RT-001/002/003
  written by the lead.
- 2026-11-13 tests: LIB-RT-002 amended by the lead: `nested.golden.sacm.xmi` holds kinds no
  row reads, so it is on a named pending list that must still be refused.
- 2026-11-13 verification: six `Display` mutants closed by a test (830dc74). Round 1 FAIL on
  a pre-order test gap, closed in f53f06f with the docs review. Round 2 PASS (record 0031).
  M1 37/46.
