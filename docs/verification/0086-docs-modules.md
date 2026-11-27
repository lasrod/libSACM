# Verification 0086 — Module docs and the docs/sacm references

- Date: 2026-11-27
- Row verdict: LIB-DOC-011: PASS
- Row verdict: LIB-DOC-013: PASS
- Verifier: `sacm-verifier` (independent; read-only)
- Commit: 0d09a5d (first round b17ffcc); evidence: gate and probes in `target/scratch/base0086`
- Slice brief: [docs/slices/0086-docs-modules.md](../slices/0086-docs-modules.md)

## Result

The verifier read every public module's docs against the guide and for length, checked each
present-tense claim against the code (probing a strict write and a flag's round trip),
resolved every anchor, diffed every table of docs/sacm (no line changed), compared the
quotations of the standard, and regenerated the catalog unchanged. In the first round three
claims in the module docs were false; the fix round applied the verifier's texts and cuts.

## Findings

| Blocking / non-blocking | Row | Finding | Resolution |
|---|---|---|---|
| Blocking | LIB-DOC-011 | `xmi`: "values are held as written", though a flag is read as its value. | String values as written; a flag as its value. |
| Blocking | LIB-DOC-011 | `xmi`: "nothing is dropped", though comments and processing instructions are not kept. | Said so, as the crate root does. |
| Blocking | LIB-DOC-011 | `edit`: the Refusals link missed the move and copy checks. | Both headings named. |
| Non-blocking | LIB-DOC-011, LIB-DOC-013 | A restated signature table, two bullets that do not lead with the point, two prose callouts, four passages a caller does not need, a false test comment. | Fixed or cut with the verifier's text. |
| Non-blocking | LIB-DOC-011 | Reference detail in `validation` and `edit` belongs in item docs. | docs/follow-ups.md, for LIB-DOC-012. |
