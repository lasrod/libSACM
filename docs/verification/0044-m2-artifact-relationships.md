# Verification 0044 — Artifact references, argument groups and reference typing

- Date: 2026-11-15
- Row verdict: SACM23-ARG-006: PASS
- Row verdict: SACM23-ARG-009: PASS
- Row verdict: SACM23-ARG-011: PASS
- Row verdict: SACM23-ARG-012: PASS
- Row verdict: SACM23-ARG-013: PASS
- Row verdict: SACM23-VAL-004: PASS
- Row verdict: SACM23-VAL-016: PASS
- Row verdict: SACM23-VAL-022: PASS
- Re-checked, staying verified: SACM23-XMI-010, -016, SACM23-ARG-004, -005, -008, -010,
  SACM23-VAL-003, LIB-COMPAT-001, -007, LIB-DIAG-002, LIB-RT-002
- Verifier: `sacm-verifier` (independent; read-only), 1 round; `sacm-docs-reviewer`, 1 round
- Commit: 8a509d1. Evidence:
  - CI run 43603394600 on 8a509d1: green on Linux, Windows and macOS. Its PR-diff mutants
    caught 55, missed 0, and 26 were unviable.
  - The full `mutants.yml` run 43602003494 on c00afd9 caught 1,427 and missed 4. Those 4
    were the same as the PR-diff misses on c00afd9, and the tests added in 8a509d1 kill them.
  - 65 hand mutations.
  - Probes: empty, text, cross-file, 5,000-deep and Unicode reference elements, with no
    panic.
  - 100,000 ids validate in linear time.
- Slice brief: [docs/slices/0044-m2-artifact-relationships.md](../slices/0044-m2-artifact-relationships.md)

## Result

ArtifactReference, ArgumentGroup, AssertedEvidence, AssertedArtifactSupport and
AssertedArtifactContext follow ADR 0028:
- They are read in both modes, written in ptc order after `abstractForm`, compared and
  created.
- Under an ArgumentGroup, `argumentElement` is a reference end. Under an ArgumentPackage it
  is the composite role.
- Every create refuses a wrongly typed end after `ReferenceNotFound`.
- `validate` reports:
  - a wrongly typed reference (VAL-004);
  - the §11.15/§11.17/§11.18 ArtifactReference rules (VAL-016), once per wrong reference;
  - a repeated id as a Warning (VAL-022).

Every quotation was checked against SACM 2.3 §11.1-§11.4, §11.9, §11.15, §11.17,
§11.18, ptc/22-03-13 and XMI 2.5.1.

## Findings

| Severity | Row | Finding | Resolution |
|---|---|---|---|
| BLOCKING | ARG-006, ARG-013, VAL-004 | CI mutants: the two new `Difference::end` arms and `article` were not pinned. | Tests pin the exact `Display` lines. |
| BLOCKING | docs | Docs review: edits were said to refuse every wrong kind (they leave §11.15/§11.17/§11.18 to `validate`); artifacts (§12) not marked unsupported; an example modelled an ArtifactReference as a claim; "every ModelElement is an ArtifactElement" was backwards (also in ADR 0028 and the ARG-006 row); 9 smaller items. | All fixed; the ADR and row wording corrected, the decision unchanged. |
| FOLLOW-UP | ARG-006, ARG-013 | No test pins that an id named only in a reference end is never generated (`reserved_ids`), for every end but `metaClaim` and the §8.2 ends. | In `docs/follow-ups.md`. |
| FOLLOW-UP | VAL-004 | `article`'s vowel check is pinned only for A; an ImplementationConstraint or Event target is not. | In `docs/follow-ups.md`. |
