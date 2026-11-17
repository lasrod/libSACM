---
slice: 0060
name: m3-close
state: in-progress
step: verification
rows: [LIB-QA-005, SACM23-ART-003, SACM23-ART-005]
mode: pr
base: main
---

# Slice 0060 — M3 close: audit fixes and follow-ups

## Scope

This slice fixes what the M3 close audit (matrix-steward) and docs/follow-ups.md found:
- **Row texts that say more than is true.** The lead rewords them; the audit's findings 3 to
  11.
- **Two test gaps:** a Resource's `location` ids as reference targets, and Properties under
  all eight ArtifactAsset kinds.
- **CHANGELOG:** its `[Unreleased]` section becomes the 0.1.0 notes.
- **docs.rs** builds without `test-support`.
- **The follow-ups:** each is fixed, decided, or moved to the planned row it belongs to.

Out of scope: the publish of 0.1.0, which is new row LIB-QA-019 and waits for the
maintainer's go.

## Basis

Quality rows; acceptance by the lead. Maintainer decisions of 2026-11-17:
- a "First release published" row in M3;
- docs.rs builds without `test-support`;
- strict reads keep the model's spellings with an Error, recorded as D-STRICT's one exception
  and spec defect D19.

## Log

- 2026-11-17 audit: `matrix-steward` reported 11 findings at M3's close; the full
  `mutants.yml` run 44043002416 on main is in progress.
- 2026-11-17 design: by the lead.
  - The matrix edits for findings 3 to 11 are applied, and new row LIB-QA-019 is added.
  - Recorded: D-STRICT's exception, spec defect D19, and the ci.yml base limit (accepted as
    a known limit).
  - Builder step 1 is running.
- 2026-11-17 tests: `sacm-builder` step 1. The docs.rs test is red, as expected.
  - New: `sacm23_art_005_a_reference_to_a_location_id_dangles_and_is_written` (passes; it
    pins existing behaviour).
  - ART-003 now covers all eight asset kinds, the three-deep test included.
  - Renames, with their citations updated: two LIB-API-011 spelling tests became
    `sacm23_xmi_009_…`; the LIB-EDIT-012 timing test now says n log n.
  - Follow-up tests: the LIB-COMPAT-004 closure line, the kept element names, and
    `is_iso_date` (real month lengths).
  - Lead rulings:
    - the closure line pins LIB-COMPAT-004 only, as acceptance 3 says;
    - the ART-010 xtask sample becomes ART-008, because the new CP dependencies put
      ART-010 in CP-002 and CP-004;
    - the strict SACM-XMI-013 message tail becomes "reads the name as R" (an approved
      message change, with its tests updated).
  - Two follow-ups moved to the notes of LIB-COMPAT-004 and SACM23-XMI-027.
- 2026-11-17 implementation: `sacm-builder` step 2 rewrote `[Unreleased]` as the 0.1.0
  notes, and changed docs.rs to `serde` only, both READMEs, the `RepeatedValue` and
  `MissingType` rustdoc, diagnostics.md, `XmiMetadata` and the strict SACM-XMI-013 tail.
  - Lead ruling: in the ART-008 sample test, the test's own matrix copy also lists
    ART-008 under CP-001, standing in for the second path ART-010 had through
    LIB-COMPAT-005. Setup only; no assertion changed.
  - The lead cleared docs/follow-ups.md.
- 2026-11-17 verify round 1: FAIL, on wording only.
  - Blocking: XMI-007's attribute order; SACM-XMI-013 missing from the `ReadOutput` and
    `Mode::Strict` rustdoc (XMI-001, LIB-COMPAT-001); five false CHANGELOG claims.
  - Fix round: the builder fixed the rustdoc, the CHANGELOG, releasing.md and two doc
    comments; the lead fixed the matrix, the D-STRICT and CP-003 citations, and moved
    D19. All use the verifier's text.
