---
slice: 0044
name: m2-artifact-relationships
state: done
step: verified
rows: [SACM23-ARG-006, SACM23-ARG-009, SACM23-ARG-011, SACM23-ARG-012, SACM23-ARG-013, SACM23-VAL-004, SACM23-VAL-016, SACM23-VAL-022]
mode: pr
base: main
---

# Slice 0044 — Artifact relationships, ArgumentGroup and reference typing

## Scope

The rest of SACM argumentation, on ADR 0026/0027's references:
- **SACM23-ARG-006:** ArtifactReference and its `referencedArtifactElement` (0..*, to
  ArtifactElements).
- **SACM23-ARG-009 / -011 / -012:** AssertedEvidence, AssertedArtifactSupport and
  AssertedArtifactContext.
- **SACM23-ARG-013:** ArgumentGroup, whose `argumentElement` is a membership reference, not
  containment.
- **SACM23-VAL-004:** each reference resolves to an element of its end's declared type
  (SACM-REF-002).
- **SACM23-VAL-016:** the kind-specific source and target rules of §11.15, §11.17 and §11.18.
- **SACM23-VAL-022:** a repeated id in a multi-valued end is a Warning (the maintainer).

## Basis

SACM 2.3 §11; ptc/22-03-13; ADR 0026; ADR 0027; D-SEV; D-VAL-LAYER.

## Design

[ADR 0028](../adr/0028-artifact-references-groups-and-typing.md), accepted by the maintainer on 2026-11-14 (edits refuse wrongly typed references; D-VAL-LAYER amended). API: `SacmElement::{referenced_artifact_elements, group_members}`; `edit::{CreateArtifactReference, CreateArgumentGroup, CreateAssertedEvidence, CreateAssertedArtifactSupport, CreateAssertedArtifactContext}`; `EditError::WrongReferenceType`; `Difference::{ReferencedArtifactElements, GroupMembers}`; `Code::REF_WRONG_TYPE`. SACM-REF-003 moved to LIB-COMPAT-003; the group-delete clause to LIB-EDIT-005.

## Log

- 2026-11-14 choose: the remaining argumentation rows and the typing rules they need.
- 2026-11-14 tests: 97 failing tests over the 8 rows (red on the missing API only); 6 verified rows' tests changed, re-check notes added; VAL-022 reworded to `validate`'s order.
- 2026-11-14 implement: ADR 0028 built; four clippy lints in the new tests fixed by the test engineer; gate green (1707 tests).
- 2026-11-15 verify: PASS in one round; the docs review fixed (edits leave §11.15/§11.17/§11.18 to validate; ArtifactElement wording); 4 CI mutants pinned; record 0044.
