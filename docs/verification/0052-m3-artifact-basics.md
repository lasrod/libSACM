# Verification 0052 — Artifact packages, assets and groups

- Date: 2026-11-16
- Row verdict: SACM23-ART-001: PASS
- Row verdict: SACM23-ART-003: PASS
- Row verdict: SACM23-ART-008: PASS
- Row verdict: SACM23-ART-009: PASS
- Row verdict: SACM23-ART-011: PASS
- Row verdict: LIB-API-011: PASS
- Row verdict: LIB-API-006: PASS
- Row verdict: LIB-EDIT-003: PASS
- Row verdict: SACM23-XMI-001: PASS
- Row verdict: SACM23-XMI-016: PASS
- Verifier: `sacm-verifier` (independent; read-only), two rounds
- Commit: d9bda7d; evidence: CI run
  [43796025138](https://github.com/lasrod/libSACM/actions/runs/43796025138), all checks
  green. PR-diff mutants: 64, of which 39 caught, 0 missed, 0 timeout and 25 unviable.
- Slice brief: [docs/slices/0052-m3-artifact-basics.md](../slices/0052-m3-artifact-basics.md)

## Result

ArtifactPackage, Property, Technique, Participant and ArtifactGroup are read, written,
compared and edited as ADR 0034 states, and the worked fixture writes its golden file
exactly. The parent's kind decides what `artifactElement` is: the composite role under an
ArtifactPackage, the member end under an ArtifactGroup. The kinds not read yet are refused
under their rows. Every `Interim` row gates the compliance points whose units can hold its
kind: SACM23-CP-001 and CP-003 for every kind, plus CP-002 or CP-004 for its own package.

Evidence beyond CI:
- 26 hand mutants in round 1 and 6 in round 2, all killed in the final state;
- the quotations checked against formal/23-05-08 §12 and ptc/22-03-13;
- probes with the CLI.

## Findings

| Severity | Row | Finding | Resolution |
|---|---|---|---|
| blocking | SACM23-ART-001 | Treating every unknown type as the model's spelling survived the suite (M07). | Tests for unknown, abstract, foreign and wrongly-cased types in `artifactElement`, and for the spelling where its kind is not admitted. |
| blocking | ART-001 to -011 | The rustdoc of `xmi::read` and `xmi::write` was stale, and one claim about a nested group element was false. | Rewritten; stale lines in `document.rs`, `compare.rs`, the README and the catalog also fixed. |
| blocking | LIB-API-011 | The requirement text still stated the old gate. The rule let CP-003 miss Terminology rows, although an ArtifactPackage can hold every kind. | The rule is now CP-001 and CP-003 plus the row's own package's point; CP-003 gains TERM-004/005/006. |
