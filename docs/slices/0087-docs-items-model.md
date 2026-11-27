---
slice: 0087
name: docs-items-model
state: in-progress
step: tests
rows: [LIB-DOC-012]
mode: pr
---

# Slice 0087 — Item docs of `model`, `compare`, `diagnostic` and `validation`

## Scope

The item docs (`///`) of four modules, about 6,000 lines, rewritten to docs/style.md. The
`edit` and `xmi` item docs, about 4,700 lines each, follow in LIB-DOC-014 and LIB-DOC-015
(split off here). The follow-ups of slice 0086 for the `validation` module docs (move the
per-rule facts to `validate`'s item docs) belong to this slice.

Base: `main` (b26dce7).

## Basis

Docs row: acceptance by the lead; docs/style.md (LIB-DOC-009). Fable 5.2 writes the docs.

## Log

- 2026-11-27 design: LIB-DOC-012 narrowed to four modules; LIB-DOC-014 and -015 added; an
  item's summary may run to two display lines. Row `analyzed`.
- 2026-11-27 tests: three tests in docs_style.rs. The lead's rulings: public items only (a
  block before a `pub(crate)`, `pub(super)`, `pub(in` or non-`pub` item, or inside a
  `#[cfg(test)]` module, is skipped; variants and fields count); attributes between `///`
  lines join the block. Red: 146 long blocks and 211 long summaries. Row `tests-failing`.
- 2026-11-27 implementation: two Fable 5.2 writers in parallel (model; diagnostic, compare,
  validation). Hits 146 + 211 to 0; doctests and the catalog unchanged; the validation
  per-rule facts moved to `validate`; stale claims corrected (artifacts "not supported yet",
  the document-level `Difference` variants, `XMI_UNKNOWN_CONTENT`, `REF_DANGLING`). Row
  `implemented`.
- 2026-11-27 verification: PASS, non-blocking findings only. Fix round: the check counts
  trait-impl items, which rustdoc shows (approved test change, re-verified), and the five
  summaries it then finds; the verifier's wording; follow-ups retagged to LIB-DOC-014/-015.
