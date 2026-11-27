# Verification 0090 — Edits never remove an id that is still named

- Date: 2026-11-28
- Row verdict: LIB-EDIT-026: PASS
- Verifier: `sacm-verifier` (independent; read-only)
- Commit: db5704d plus the V-F6 case (first round 2c12e93); evidence: `target/verify-0090/` (stress runs, 40 + 8 hand mutants); the lead confirmed V-F6 killed by the added case
- Slice brief: [docs/slices/0090-named-ids.md](../slices/0090-named-ids.md)

## Result

The verifier checked the reading of XML Schema Part 1 §3.3.4, XMI 2.5.1 §7.6.2 and XMI.xsd's
IDREF/IDREFS attributes against the pinned files, confirmed the edit check and the strict
write share one token walk, and ran the guarantee under stress (20 inputs × 800 seeds × 40
steps) in both rounds with no counterexample. The first round found an over-refusal and nine
untested pieces of logic; the fix round fixed both and left one untested condition (V-F6),
which an added refused case kills. The maintainer allowed that second small fix round.

## Findings

| Blocking / non-blocking | Row | Finding | Resolution |
|---|---|---|---|
| Blocking | LIB-EDIT-026 | The text-set check refused an edit when the `expression` naming the freed id was held by the text being removed. | The check counts only expressions the set keeps; accepted and still-refused cases. |
| Blocking | LIB-EDIT-026 | Nine mutants survived (V07, V19, V20, V21, V24, V24b, V24c, V26), then V-F6 in the fix. | A case for each; each confirmed killed. |
| Non-blocking | LIB-EDIT-026 | Wording: `PartReference`'s fix for the `expression` end, the Savable bullet, the round-trip bullet. | Fixed with the verifier's text. |
| Non-blocking | LIB-EDIT-026 | `PartReference`'s Display says "a name or text" for an id in a text's metadata (golden-locked). | Kept. |
