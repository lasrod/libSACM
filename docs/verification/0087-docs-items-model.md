# Verification 0087 — Item docs of `model`, `compare`, `diagnostic` and `validation`

- Date: 2026-11-27
- Row verdict: LIB-DOC-012: PASS
- Verifier: `sacm-verifier` (independent; read-only)
- Commit: c5bd2ed (first round d390639), plus the verifier's one-line wrap; evidence: `target/verify-0087-gate.log`, `-mutations.log`, `-fix-gate.log`, `-fix-mutations.log`
- Slice brief: [docs/slices/0087-docs-items-model.md](../slices/0087-docs-items-model.md)

## Result

The verifier confirmed that only comments changed in the library, replayed the red, probed
nine claims of the rewritten docs against the code (all hold), traced every removed fact to
where it lives, checked the points Assurance Forge raised, regenerated the catalog unchanged
and mutated the item-doc check. Both rounds passed; the fix round made the check count the
items of trait impls, which rustdoc shows, and applied the verifier's wording.

## Findings

| Blocking / non-blocking | Row | Finding | Resolution |
|---|---|---|---|
| Non-blocking | LIB-DOC-012 | The check skipped trait-impl items; five long summaries hid there. | The check counts them; the five rewritten. |
| Non-blocking | LIB-DOC-012 | Wording: `Document` equality, the compare table, the strict-read bullet, the record's catalog check, `ElementId`'s part ids, `XMI_UNKNOWN_CONTENT`, lower camel case. | Fixed with the verifier's text. |
| Non-blocking | LIB-DOC-012 | Methods declared in a `pub trait` are unchecked; the innermost-impl rule has no control. | docs/follow-ups.md. |
