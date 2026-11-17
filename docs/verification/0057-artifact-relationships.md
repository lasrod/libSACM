# Verification 0057 — ArtifactAssetRelationship and the model spellings

- Date: 2026-11-17
- Row verdict: SACM23-ART-010: PASS
- Row verdict: SACM23-XMI-009: PASS
- Verifier: `sacm-verifier` (independent; read-only), one round
- Commit: 9f46d7e. Evidence: CI run
  [44019057859](https://github.com/lasrod/libSACM/actions/runs/44019057859), green on all
  jobs. PR-diff mutants: 108 caught, 39 unviable (`Default` on types without one), 0 missed,
  0 timed out. No exclusions were needed.
- The wording round after the PASS changed docs only, using the verifier's text. The lead
  checked it.
- Slice brief: [docs/slices/0057-artifact-relationships.md](../slices/0057-artifact-relationships.md)
- Decision: ADR 0034 amendment 2, accepted by the maintainer on 2026-11-17.

## Result

- **ArtifactAssetRelationship** is read, written, created, edited, compared and
  validated, with `ReferenceEnd::ArtifactSource` and `ArtifactTarget` (`[1..*]`, typed
  ArtifactAsset, clause 12.14). It follows the asserted-relationship delete rules, and a
  missing end is a SACM-MULT-001 Error.
- **Spellings:** a tolerant read maps `ArtifactAssertedRelationship`, an Event's
  `occurece` and `argumentationElement` to the resolved names, with a SACM-XMI-013 Warning.
  A strict read keeps them as preserved content with an Error. The resolved name wins over a
  spelled repeat.
- **`ReadError::RepeatedValue`** gains `kind`, so its message is exact for an end name that
  two classes share.
- **Quotations** were checked against formal/23-05-08 (§12.14, §12.6, §12.9, §11.2, §11.4,
  §2.3) and ptc/22-03-13.
- **The red snapshot** was compared with the final tests. The only changes are:
  - the lead-approved fixes to six step-1 tests;
  - the removed gates;
  - one added test;
  - lint-only refactors.

  The JSON goldens equal the drafts byte for byte.
- **Rewritten tests of verified rows** still check at least what they did, for what still
  applies: ART-001, ART-007, ARG-013, XMI-016, XMI-010, LIB-API-011 and LIB-API-015.
- **The eleven reworded verified rows** are true of the code: ART-001, ART-003, ART-007,
  ARG-013, XMI-016, XMI-005, VAL-022, LIB-EDIT-005, LIB-EDIT-009, LIB-API-006 and
  LIB-API-015. A scan of the other verified rows found none made false.

Evidence beyond CI:
- 32 hand mutations of the compare arms, end tables, validation, the FeatureNotApplicable
  wording, the delete cascade and scrub counts, the spelling clauses and messages, the
  resolved-name rule, the reserved ids, strict against tolerant, `write_repeated`, the
  create check order and `targets()`. All were killed.
- Edge probes, each showing no panic, no dropped content, and strict and tolerant distinct:
  - an untyped spelled role;
  - a spelled role holding the spelled type;
  - `occurece` as both an attribute and an element;
  - an empty spelled end;
  - `xsi:type` with the spelled type;
  - mixed spelled and resolved members;
  - deletes over chained relationships.

## Findings

| Severity | Row | Finding | Resolution |
|---|---|---|---|
| non-blocking | XMI-009 | `lib.rs`, both READMEs, metamodel-inventory.md lines 58–59, and decisions.md D-PRESERVE described the spellings imprecisely or in the future tense. | Reworded with the verifier's text. |
| non-blocking | both | CHANGELOG: a blank line split the Changed list. The matrix: the M4 banner sat inside the XMI-009 row. | Fixed. |

## Logged

Three items are in [docs/follow-ups.md](../follow-ups.md):
- the strict SACM-XMI-013 message overstates what a tolerant read keeps for two corner cases;
- the narrowed substrings no longer show the kept element's name;
- the tension between D-STRICT and D-TEXT-MODEL, to raise with the maintainer at M3's close.
