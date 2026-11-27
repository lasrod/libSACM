---
slice: 0089
name: docs-items-rest
state: done
step: close
rows: [LIB-DOC-014, LIB-DOC-015, LIB-DOC-002, LIB-DOC-010]
mode: pr
---

# Slice 0089 — Item docs of `edit` and `xmi`, examples, and what Assurance Forge asked

## Scope

The rest of the item docs: `edit` (LIB-DOC-014) and `xmi` (LIB-DOC-015), about 4,700 lines
each, to docs/style.md; a runnable example on every public type and fallible function
(LIB-DOC-002); and the facts Assurance Forge had to work out, stated in the item docs
(LIB-DOC-010, lasrod/libSACM#113 D1-D4). Two Fable 5.2 writers on disjoint files.

Base: `main` (213e2fc).

## Basis

Docs rows: acceptance by the lead; docs/style.md (LIB-DOC-009). The `xmi::write` round-trip
guarantee comes from LIB-RT-004's wording, now true because edits refuse unsavable text
(LIB-EDIT-025).

## Log

- 2026-11-28 design: four rows `analyzed`. Today one public type (`WriteAttribute`) and three
  fallible functions (`edit::apply_unchecked_text`, `Batch::apply`, `CopyElements::root`)
  lack an example; `# Errors` and `# Panics` are enforced by clippy pedantic.
- 2026-11-28 tests: 11 tests in docs_style.rs (LIB-DOC-010 is the verifier's to read). Red:
  edit 154 long blocks and 140 summaries; xmi 118 and 69; 18 of 86 public types and 4 of 14
  fallible functions without a runnable `# Examples` section (15 have a doctest and lack only
  the heading, which stays required, as rustdoc's convention and the acceptance have it).
  Rows LIB-DOC-002, -014, -015 `tests-failing`; LIB-DOC-010 `analyzed`.
- 2026-11-28 implementation: two Fable 5.2 writers (edit; xmi with the LIB-DOC-002 and
  D-DESC-MULT items). All hits to 0; every macro-generated operation renders an `# Examples`
  section; check-order tables moved to `apply`'s `# Errors`; stale claims corrected. Writer B
  probed the round-trip guarantee and found it false in two cases (an edit frees an id XMI
  metadata or a reference still names), so `xmi::write` states it with those exceptions; the
  lead adds LIB-EDIT-026 to remove them before 0.5.0 and adjusts LIB-DOC-015's acceptance.
  Rows `implemented`.
- 2026-11-28 verification: FAIL on LIB-DOC-014 (the delete checks left proxies out) and
  LIB-DOC-015 (a third round-trip exception: a copy brings its source's preserved content and
  metadata ids); LIB-DOC-002 and -010 PASS but wait on them. Fix round: the verifier's texts;
  LIB-DOC-015's acceptance names the copy exception; the lead adds row LIB-EDIT-026 (planned)
  so the acceptance's reference resolves. The guarantee stays stated, with its exceptions.
- 2026-11-28 verification: re-verify PASS on all four rows (9132b55). Record
  `docs/verification/0089-docs-items-rest.md`. Rows `verified`.
