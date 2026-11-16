---
slice: 0052
name: m3-artifact-basics
state: in-progress
step: verification
rows: [SACM23-ART-001, SACM23-ART-003, SACM23-ART-008, SACM23-ART-009, SACM23-ART-011]
mode: pr
base: main
---

# Slice 0052 — Artifact packages, assets and groups

## Scope

M3's first slice reads, writes and edits these classes of the Artifact package:
- ArtifactPackage;
- Property;
- Technique;
- Participant;
- ArtifactGroup, with its `artifactElement` member end.

**Left out:**
- Artifact, Resource, Activity, Event and ArtifactAssetRelationship: dates, versions,
  locations and relationship ends come in slices 0053 and 0054. Until then they are refused
  as `Interim`.
- The machine-readable model's misspellings (SACM23-XMI-009).
- ArtifactPackage interfaces (SACM23-ART-012, M5).
- `Document::referrers` and the public end readers (LIB-API-014/015). They come later in M3
  and must cover the new end.

## Basis

formal/23-05-08 clause 12: §12.2 ArtifactPackage, §12.3 ArtifactGroup, §12.6
ArtifactAsset.property, §12.8 Property, §12.12 Technique, §12.13 Participant. ptc/22-03-13 as listed in
`docs/sacm/metamodel-inventory.md`, Package `Artifact`. The maintainer's M3 decisions of
2026-11-15, items 5 and 6. [ADR 0034](../adr/0034-artifact-packages-assets-and-groups.md),
accepted 2026-11-15.

## Design

The architect's brief of 2026-11-15 is in `target/slice-0052-architect-brief.md`
(git-ignored), and the test and implementation briefs quote it.
- **New public API:**
  - `ReferenceEnd::ArtifactGroupMember` (`"artifactElement"`);
  - `SacmElement::properties()`; `group_members()` answers for both group kinds;
  - `Difference::ArtifactGroupMembers`;
  - `edit::{CreateArtifactPackage, CreateProperty, CreateTechnique, CreateParticipant,
    CreateArtifactGroup}`.
- **No new diagnostic codes.**
- **Two corrections to the brief:**
  - The `interface` refusal landed in slice 0050.
  - A role element in reference form is a proxy (slice 0051). An `artifactElement` carrying
    `xmi:idref` under an ArtifactPackage is refused under SACM23-XMI-019, and one carrying
    `href` under SACM23-XMI-027. Under an ArtifactGroup the same element is a member
    reference.

## Log

- 2026-11-16 choose: M3's first slice, the rows that unblock the other ART rows.
  Researched and designed on 2026-11-15, with ADR 0034 approved.
- 2026-11-16 analyzed: acceptance written from the architect's brief.
- 2026-11-16: the maintainer decided that an `Interim` row gates SACM23-CP-001 plus the
  compliance point of its own package, not all four (LIB-API-011 acceptance [0]).
- 2026-11-16 tests: 58 `sacm23_art_*` tests and 7 tests of verified rows, all cited; red confirmed (the reader still refuses ArtifactPackage). The files that use the new API are gated by `cfg(slice_0052_api)` until the implementer removes the gates. The member check order follows SACM23-ARG-013.
- 2026-11-16: after an independent review (Fable 5.2), the maintainer adopted a simplification plan that runs as the next slice. This slice is unaffected.
- 2026-11-16 implementation: ArtifactPackage, Property, Technique, Participant and ArtifactGroup read, write and edit; the five creates; `ArtifactGroupMember`; `properties()`; `Difference::ArtifactGroupMembers`; nine new `Interim` kind entries and the `ModelSpelling` tolerance. SACM23-XMI-001 and -016 were reworded. Gate green: 2,308 workspace tests, 1,794 without features, 434 xtask.
- 2026-11-16 verification round 1: FAIL. Behaviour is right and the PR-diff mutants show 0 missed. LIB-API-006, LIB-EDIT-003, SACM23-XMI-001 and -016 pass. Blocking items:
  - the M07 test gap (an unknown type in `artifactElement` refused as the model spelling);
  - stale rustdoc in `xmi.rs`;
  - LIB-API-011's requirement text.
  The verifier also found that the lead's prefix rule let CP-003 miss Terminology rows. An ArtifactPackage can hold every kind, so every Interim row now gates CP-001 and CP-003, plus its own package's point; CP-003 gains TERM-004/005/006.
- 2026-11-16 fix round 1: done. New tests cover an unknown, abstract, foreign or wrongly-cased type in `artifactElement`, and the model spelling preserved where its kind is not admitted. The gate rule is now CP-001 and CP-003 plus the row's own package's point, with a test for a TERM row against CP-003. The rustdoc of `xmi.rs`, `document.rs` and `compare.rs`, the README and the catalog are brought up to date. Gate green: 2,311 workspace tests, 1,796 without features, 435 xtask.
