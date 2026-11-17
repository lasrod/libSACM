# Verification 0060 — M3 close: audit fixes and follow-ups

- Date: 2026-11-17
- Re-checked rows: all PASS, and all stay verified:
  - SACM23-ART-003, SACM23-ART-005, SACM23-XMI-001, SACM23-XMI-005, SACM23-XMI-007,
    SACM23-XMI-009, SACM23-VAL-003;
  - LIB-COMPAT-001, LIB-EDIT-009, LIB-EDIT-012, LIB-EDIT-014, LIB-EDIT-017, LIB-EDIT-018;
  - LIB-API-010, LIB-API-011, LIB-API-014, LIB-API-016, LIB-QA-005.
- LIB-QA-019 stays planned. The CHANGELOG precondition of its release is met; the publish
  waits for the maintainer's go.
- Verifier: `sacm-verifier` (independent; read-only), two rounds
- Commits:
  - 63eddde, round 1: FAIL on wording only. Evidence: CI run
    [44049863378](https://github.com/lasrod/libSACM/actions/runs/44049863378), green. The
    publish dry run passed.
  - 30e1feb, round 2: PASS. The changes were text only; the fix's CI run is cited in the PR.
- Slice brief: [docs/slices/0060-m3-close.md](../slices/0060-m3-close.md)

## Result

- **matrix-steward's M3 close audit:** all 11 findings are fixed.
  - The planned row LIB-QA-019 holds M3 open until the publish.
  - The compliance points close over SACM23-XMI-009 and -028.
  - Row texts are true of the code: `location` ids as part ids, the eight asset kinds,
    write order, writer features, the read diagnostics, and the M4 and M5 templates and
    notes.
- **Tests:**
  - new: `sacm23_art_005_a_reference_to_a_location_id_dangles_and_is_written`;
  - ART-003 covers all eight kinds;
  - docs.rs builds with `serde` only;
  - from the follow-ups: the LIB-COMPAT-004 closure line, the kept element names, and real
    ISO dates.

  No test was weakened. The approved changes were three renames, the strict SACM-XMI-013
  tail "reads the name as R", and the ART-008 sample setup.
- **CHANGELOG `[Unreleased]`** reads as accurate 0.1.0 notes. It has no "until" a verified
  row, the tolerant-read exceptions and strict-only refusals are stated, and a Known limits
  list is included.
- **Recorded:** D-STRICT's one exception (the maintainer, 2026-11-17), and spec defect D19,
  quoted against formal/23-05-08 §12.14 and ptc/22-03-13.
- **The follow-ups in docs/follow-ups.md:** 13 at M3's close, all resolved:
  - fixed in this slice, ten: the XMI-005 advice, `XmiMetadata`, the XMI-013 tail, the
    kept names, the timing-test name, the LIB-API-011 [2] assertion, docs.rs, `is_iso_date`,
    the packaged-tests note, and the D-STRICT tension, now recorded;
  - accepted as a known limit, one: the ci.yml base filter;
  - moved to the notes of the planned rows they belong to, two: LIB-COMPAT-004 and
    SACM23-XMI-027.

Hand mutations, all killed:
- the `property` role removed from ArtifactAssetRelationship;
- the XMI-013 tail reverted;
- docs.rs features;
- the leap and month rules in `is_iso_date`;
- location ids resolved;
- a CP gaining LIB-COMPAT-004.

## Findings

| Severity | Row | Finding | Resolution |
|---|---|---|---|
| blocking | SACM23-XMI-007 | The reworded attribute order left out `assertionDeclaration` and `isCounter`. | The verifier's sentence. |
| blocking | SACM23-XMI-001, LIB-COMPAT-001 | The `ReadOutput` and `Mode::Strict` rustdoc did not name SACM-XMI-013. | The verifier's text. |
| blocking | LIB-QA-005, LIB-QA-019 | Five CHANGELOG claims were false: tolerant-read exceptions, strict-only refusals, refusal locations, deletes and dangling references, and the W3C scope. | The verifier's text. |
| non-blocking | several | Input limits were missing from Known limits; the doctests sentence in releasing.md; the doc comments on docs.rs; D17 beside D19; D19's placement; parentheticals in VAL-003, LIB-EDIT-009 and LIB-API-010. | Applied as given. |

## Logged

- Some lines in the CHANGELOG (the W3C line) and in the `Mode::Strict` rustdoc run past the
  wrap width. fmt does not check this.
