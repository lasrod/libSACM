---
slice: 0086
name: docs-modules
state: done
step: close
rows: [LIB-DOC-011, LIB-DOC-013]
mode: pr
---

# Slice 0086 — Module docs and the docs/sacm references

## Scope

The rest of the documentation a reader meets before the item docs: every module's `//!` docs
(LIB-DOC-011), and the references in docs/sacm (LIB-DOC-013): diagnostics.md,
compliance-points.md, decisions.md, and the introduction of the generated metamodel
inventory (the text before its first `##` heading). Item docs follow in LIB-DOC-012.
docs/sacm/reference/specification-defects.md is a record of defects and stays as it is.

Base: `main` (895b574).

## Basis

Docs rows: acceptance by the lead; docs/style.md (LIB-DOC-009). Fable 5.2 writes the docs.
decisions.md is settled policy: restructured only, every decision keeps its meaning.

## Log

- 2026-11-27 design: LIB-DOC-013 scoped (policy meaning kept; machine-read tables keep their
  shape; the generated inventory only in its introduction). Rows `analyzed`.
- 2026-11-27 tests: two tests in xtask/tests/docs_style.rs; red: 84 module-doc blocks and 13
  in docs/sacm. The lead's rulings: the decisions table of decisions.md keeps its rows and
  wording (it is policy, and two tests read it); diagnostics.md's Meaning cells may be
  tightened, shape kept; block quotes stay exempt (quotations of the standard stay verbatim),
  and the verifier reads prose callouts. Rows `tests-failing`.
- 2026-11-27 implementation: Fable 5.2 rewrote the module docs (opening line, entry points,
  `# Rules`, detail) and the prose of diagnostics.md and compliance-points.md; no table cell,
  code or template changed, and the catalog regenerates unchanged. Five stale claims
  corrected (two said a strict write refuses a validation Error). Line counts rose where
  prose became tables. Rows `implemented`.
- 2026-11-27 verification: LIB-DOC-013 PASS; LIB-DOC-011 FAIL on three false claims (values
  held as written, nothing dropped, the Refusals link). Fix round: the verifier's texts, four
  cuts in `edit` and `xmi`, two callouts in docs/sacm, a false comment in
  relationship_validation.rs; the moves of reference detail logged for LIB-DOC-012.
- 2026-11-27 verification: re-verify PASS on both rows (0d09a5d). Record
  `docs/verification/0086-docs-modules.md`. Rows `verified`.
