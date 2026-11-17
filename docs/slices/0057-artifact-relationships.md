---
slice: 0057
name: artifact-relationships
state: done
step: close
rows: [SACM23-ART-010, SACM23-XMI-009]
mode: pr
base: main
---

# Slice 0057 — ArtifactAssetRelationship and the model spellings

## Scope

What this slice adds:
- ArtifactAssetRelationship (§12.14): read, written, created, edited, compared and
  validated, with its `source` and `target` ends.
- A tolerant read of three spellings: ptc/22-03-13's `ArtifactAssertedRelationship` and
  `occurece`, and the text's `argumentationElement`. A strict read keeps them as preserved
  content, with an Error.

Left out: the §10.4 spelling `TerminologyElement`, which is new row SACM23-XMI-028 (M4). Two
rows only: no other M3 row is related.

## Basis

- formal/23-05-08 §12.14, §12.9, §11.2 and §11.4;
- ptc/22-03-13: Artifact::ArtifactAssertedRelationship, Event `occurece`, and
  ArgumentPackage and ArgumentGroup `argumentElement`;
- spec defects D6 and D17.

## Design

- The design brief is `target/slice-0057-design.md` (git-ignored).
- ADR 0034 amendment 2 was accepted by the maintainer on 2026-11-17.

New public API:
- `ReferenceEnd::ArtifactSource` and `ArtifactTarget`;
- `SacmElement::targets()`;
- `edit::CreateArtifactAssetRelationship`;
- `Difference::ArtifactSources` and `ArtifactTargets`;
- `Code::XMI_SPELLING_VARIANT` (SACM-XMI-013).

## Log

- 2026-11-17 choose: the last two SACM rows of M3.
- 2026-11-17 design: done by `sacm-designer`, overlapped with slice 0056's CI. Its questions
  Q1 to Q4 are settled by the maintainer's M3 decisions 3, 5, 7 and 8.
- 2026-11-17: the maintainer accepted ADR 0034 amendment 2, including the serde names
  `artifactSource` and `artifactTarget`.
- 2026-11-17 analyzed: the design text is applied to ART-010 and XMI-009, and XMI-009 is
  retitled; new row SACM23-XMI-028 (M4) is added; ADR 0034 amendment 2 is written as
  accepted. Builder step 1 is running.
- 2026-11-17 tests: `sacm-builder` step 1 wrote 44 row tests, gated with
  `cfg(slice_0057_api)`, and the fixtures and goldens.
  - The lead confirmed the red: under the cfg, 4 test crates fail only on the missing API;
    without it, 19 tests fail on behaviour.
  - The snapshot is `red-0057.tar`, with `red-0057-golden/`, in the session scratchpad.
  - Re-samples: the unread examples now use ArtifactPackageInterface; ART-001 variant (c)
    uses ArtifactPackageBinding, as the lead accepted. One LIB-API-011 test is renamed, and
    its citation follows.
  - Lead decision: step 2 rewrites the stale tests of spellings and unread kinds to the new
    behaviour, and deletes none.
- 2026-11-17 implementation: `sacm-builder` step 2. Lead rulings:
  - Six step-1 tests were wrong, and only these fixes are approved:
    - the four ART-010 delete tests delete an Artifact that holds a Property, so they now
      use `DeleteRecursively`;
    - `…an_attribute_and_elements_for_one_end_are_repeated_value` and
      `…a_strict_read_keeps_each_spelling_as_preserved_content_with_one_error` matched
      substrings that kept fragments cannot contain, because they declare their
      namespaces, and now match narrower ones.
  - A design gap: `ReadError::RepeatedValue` gains `kind`, so it no longer calls an
    ArtifactAssetRelationship's `target` single-valued. A new test covers it.
  - Three renamed tests now have their citations updated, and three new tests are cited.
  - Reworded verified rows: ART-001, ART-003, ART-007, ARG-013, XMI-016, XMI-005, VAL-022,
    LIB-EDIT-005, LIB-EDIT-009, LIB-API-006 and LIB-API-015.
  - Equivalent mutants: none are left; the builder removed the two redundant conditions.
- 2026-11-17 verify: PASS on both rows in one round, with no blocking findings.
  - Seven wording findings were fixed with the verifier's text; the lead checked the text.
  - Three items are logged in docs/follow-ups.md.
  - The record is `docs/verification/0057-artifact-relationships.md`, and M3 is at 18/20.
