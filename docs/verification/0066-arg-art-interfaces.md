# Verification 0066 — Argument and artifact package interfaces and bindings

- Date: 2026-11-18
- Row verdict: SACM23-ARG-014: PASS
- Row verdict: SACM23-ARG-015: PASS
- Row verdict: SACM23-ART-012: PASS
- Row verdict: SACM23-ART-013: PASS
- Row verdict: LIB-EDIT-019: PASS
- Verifier: `sacm-verifier` (independent; read-only), two rounds
- Commits:
  - ff70158, round 1: FAIL.
    - LIB-EDIT-019: three delete rules untested (M20, M14, M35).
    - The four kinds' rows: a false CHANGELOG claim ("validated").
    - No wrong behaviour.
  - 16986d5, round 2: PASS. CI run
    [44261458158](https://github.com/lasrod/libSACM/actions/runs/44261458158).
- PR-diff mutants: round 1 (run 44254811358) gave 0 missed and 1 timeout, which is triaged.
  The jobs do not block (ADR 0006 amendment 6).
- Slice brief: [docs/slices/0066-arg-art-interfaces.md](../slices/0066-arg-art-interfaces.md)
- Decisions: ADR 0039 (accepted 2026-11-18); the M5 decisions Q1–Q9.

## Result

- **Reading and writing:**
  - ArgumentPackageInterface, ArgumentPackageBinding, ArtifactPackageInterface and
    ArtifactPackageBinding are read in both modes, as children and as roots.
  - They are written in the designed attribute order, and both golden files round-trip byte for
    byte.
  - The Terminology and AssuranceCase kinds and their `interface` end stay refused, as Interim.
- **Ends and edits:**
  - `interface`, `implements` and `participantPackage` are typed to the holder's family
    (`owner_on`, `target_on`), with the clauses 11.4, 11.5, 11.6, 12.2, 12.4 and 12.5.
  - The ends are never synced.
  - Lower bounds count distinct ids in creates, reference edits and copy.
  - The four creates check in the designed order.
- **LIB-EDIT-019:**
  - The scrub policies delete an interface or binding left below its bound, to a fixpoint.
  - Check 4 refuses a non-empty one under `RejectIfNonEmpty`. It runs after the dangling
    references, in pre-order, and counts preserved child elements.
  - `RejectIfReferenced` cascades nothing.
- **Standard text and serde:** the quoted standard text was checked against formal/23-05-08 and
  ptc/22-03-13, and the serde goldens hold.
- **The 11 reworded verified rows are true:** LIB-API-015, SACM23-XMI-005, LIB-EDIT-009,
  SACM23-VAL-004, SACM23-VAL-022, SACM23-XMI-016, SACM23-XMI-001, SACM23-ART-001, LIB-RT-002,
  SACM23-VAL-012 and LIB-EDIT-005.
- **The tests changed after the red snapshot keep their intent:**
  - the unread samples switched to Terminology and AssuranceCase kinds;
  - the oracles in edit_atomic.rs, delete.rs and flag_and_end_readers.rs are extended;
  - the "eight package kinds" test is now four.

Evidence beyond CI:
- the builder's 39 hand mutants;
- the verifier's 29, of which every one is killed after round 2.

## Findings

| Severity | Row | Finding | Resolution |
|---|---|---|---|
| blocking | LIB-EDIT-019 | Untested: `RejectIfReferenced` cascading nothing (M20), check 4 guarding only interfaces and bindings (M14), check 4 counting a preserved child (M35). | One case and two tests added. |
| blocking | ARG-014, -015, ART-012, -013 | The CHANGELOG said the kinds are validated. | Reworded: references are checked; the M5 rules come in SACM23-VAL-013 and -014. |
| non-blocking | all | ADR 0039 Consequences 3, four stale notes, acceptance items pointing at the design brief, the `edit.rs` table row 4, two test clean-ups, the slice log, the CI timeout triage. | Fixed with the verifier's text. |
