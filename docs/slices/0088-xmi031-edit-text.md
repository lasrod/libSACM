---
slice: 0088
name: xmi031-edit-text
state: done
step: close
rows: [SACM23-XMI-031, LIB-TOOL-024, LIB-EDIT-025]
mode: pr
---

# Slice 0088 — Difference targets hold no ids; edits refuse unsavable text

## Scope

The last library behaviour of M7: a Difference's `target` content is outside the id space
(SACM23-XMI-031), with XML Schema Part 1 pinned for its rules (LIB-TOOL-024); and edits
refuse text XML 1.0 cannot represent (LIB-EDIT-025, the maintainer's decision of
2026-11-27), so an app never reaches a state it cannot save.

Base: `main` (c838af5).

## Basis

XMI.xsd `Difference`/`target` (`processContents="skip"`), XML Schema Part 1 §3.10.1 and
§3.15.5, SACM 2.3 §2.4; ADR 0042 amendment 2. Design briefs in the session scratchpad
(`design-0088-xmi031.md`, and the LIB-EDIT-025 brief).

## Log

- 2026-11-27 design: SACM23-XMI-031 and LIB-TOOL-024 designed with the QA rows; the lead pins
  XML Schema Part 1; ADR 0042 amendment 2 accepted under §2.4. The maintainer chose that
  edits refuse unsavable text (LIB-EDIT-025, in design). Rows `analyzed`.
- 2026-11-27 design: LIB-EDIT-025 designed (a new `EditError` variant on SACM-XML-003, the last
  check of every text-storing operation; a test-support seam `edit::apply_unchecked_text` keeps
  the writer's refusal reachable). ADR 0019 amendment 4 and ADR 0030 amendment 9 accepted.
  Row `analyzed`.
- 2026-11-27 tests: SACM23-XMI-031 (difference_targets.rs, a fixture and golden), LIB-TOOL-024
  (fetch.rs), LIB-EDIT-025 (unrepresentable_text.rs; 14 existing test files set up bad text
  through the seam `edit::apply_unchecked_text`). Red confirmed: the fixture is refused for a
  repeated id; the variant and the seam are missing. Approved beyond the briefs: stronger
  read assertions in XMI-031 criteria 4, 7 and 8; the Part 1 `what` string mirroring Part 2;
  `lib_edit_017`'s serde test re-anchored on the `KeptRootType` sample. The `Scope::enter`
  unit tests go into model/preserved.rs in step 2. Rows `tests-failing`.
- 2026-11-27 implementation: two builders in parallel on split files. SACM23-XMI-031: a
  `Region`/`Scope` walk shared by the reader and the strict-write check; 24 of 26 hand mutants
  killed, two equivalent conditions dropped from the design. LIB-EDIT-025: the refusal as the
  last check, one shared `first_unrepresentable` rule, the seam; 16 hand mutants killed.
  Approved test fixes: a caller id for the batch's good create; the dangling finding affects
  `["c2", "t1"]`; the two `Scope` unit tests untraced in `src`. Full gate green (3476 tests).
  Rows `implemented`.
- 2026-11-27 implementation note: `sacm23_art_004_set_version_…`'s LIB-EDIT-025 assertions
  moved unchanged into the helper `assert_version_holding_u_0001` (clippy `too_many_lines`).
- 2026-11-27 verification: PASS on all three rows (f073dc9), non-blocking findings only;
  the verifier's wording applied (spec defect D33, ADR 0042 amendment 2's §7.6.1 sentence,
  the SACM-ID-002 row, `xmi::write`'s metadata paragraph, the CHANGELOG, architecture.md).
- 2026-11-27 close: record `docs/verification/0088-xmi031-edit-text.md`. Rows `verified`.
