---
slice: 0055
name: m3-artifact-assets
state: in-progress
step: implementation
rows: [SACM23-ART-002, SACM23-ART-004, SACM23-ART-005, SACM23-ART-006, SACM23-ART-007]
mode: pr
base: main
---

# Slice 0055 — Artifact, Resource, Activity, Event and dates

## Scope

The four remaining ArtifactAsset kinds:
- **Artifact** (§12.7), with `version` and `date`;
- **Resource** (§12.10), with a MultiLangString `location`;
- **Activity** (§12.11), with `startTime` and `endTime`;
- **Event** (§12.9), with `date`.

Each is read, written, created, edited and compared in slice 0052's pattern. Date-typed
values are held exactly as written (D-DATE).

Left out:
- the model's spellings (`occurece` stays a tolerance until SACM23-XMI-009);
- ArtifactAssetRelationship (SACM23-ART-010).

## Basis

- formal/23-05-08 §4, §8.5 and §12.7–12.11.
- ptc/22-03-13, package Artifact.
- XMI 2.5.1 §8.2.1 rules 4d and 4j, and §7.11.6.
- D-DATE, D-TEXT-MODEL and D-SEV.

## Design

The design brief is `target/slice-0055-design.md` (git-ignored). The design is ADR 0034
amendment 1, accepted.

New public API:
- **Kinds:** `ElementKind` gains Artifact, Resource, Activity and Event.
- **Accessors:** `SacmElement::version()`, `date()`, `start_time()`, `end_time()`,
  `location()`, `location_id()` and `location_value_ids()`.
- **Edits:** `CreateArtifact`, `CreateResource`, `CreateActivity` and `CreateEvent`, and
  `SetVersion`, `SetDate`, `SetStartTime`, `SetEndTime` and `SetLocation`.
- **Differences:** `Version`, `Date`, `Location`, `LocationId`, `LocationValueIds`,
  `StartTime` and `EndTime`.
- **New `WriteAttribute` variants.**

No new diagnostic codes.

## Log

- 2026-11-16 choose: the five ART rows. ART-002 unblocks ART-004, ART-006 and ART-007.
- 2026-11-16 design: done by `sacm-designer`. The maintainer chose to keep dates as written
  (D-DATE replaces Q-DATE) and approved ADR 0034 amendment 1.
  - New spec defects: D17 (no resolvable date type; the model spells Event's attribute
    `occurece`) and D18 (editorial).
  - The verified rows to reword after the implementation: ART-001, ART-003, VAL-010,
    VAL-020, LIB-RT-001, LIB-RT-002 and LIB-API-011.
- 2026-11-16 tests: `sacm-builder` step 1 wrote 35 tests, a fixture and a golden.
  - The lead confirmed the red: a strict read refuses the four kinds as Interim.
  - Existing tests whose unread-kind sample was one of the four now use an ArtifactAssetRelationship (ART-010), with no weaker check. The builder listed each one.
  - The red snapshot is `red-0055.tar` in the session scratchpad.
  - The ART-003 asset list stays at its three kinds, as the row text says.
- 2026-11-17 implementation: done by `sacm-builder` step 2. Four new tests kill mutants
  (H17, H27 and `read_string_element`'s `&&`), and are cited in ART-002, -004 and -005.
  - The lead found two step-1 tests wrong, and the builder changed exactly these:
    - `sacm23_art_005_create_resource_legal_parents_refusals_and_generated_id` expected
      `<location>` at 6 spaces. The Resource sits at depth 3 inside `xmi:XMI`, so 8 and 10
      are right.
    - `serde.rs` `optional_keys` lacked the create keys that the design gives
      `#[serde(default)]` (`version`, `date`, `location`, `start_time`, `end_time`).
  - Not step-1 tests, changed because the slice made them stale:
    - `sacm23_xmi_016_every_conforming_kind_outside_the_readable_set_is_unsupported_in_its_role`
      counts 14 unread kinds, not 18.
    - Two identical match arms in `batch.rs` and `edit_atomic.rs`, split only for a gate,
      are merged.
  - Reworded the verified rows the change made false: ART-001, ART-003, VAL-010, VAL-020,
    LIB-RT-001, LIB-RT-002, XMI-007, XMI-014, XMI-016 and LIB-COMPAT-007 (LIB-API-011 got
    a note only).
- 2026-11-17 verify, round 1: FAIL, with three blocking findings: no serde golden for the
  other four `FeatureNotApplicable` features; no test that string attributes come before
  preserved attributes; and a stale `XMI_UNREAD_CONTENT` rustdoc example. The verifier
  also gave wording fixes. The fix round adds four golden refusals (lead approved) and one
  order test, and applies the verifier's exact text. The lead fixed ART-003's and XMI-014's
  matrix phrases.
