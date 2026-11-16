# Verification 0055 — Artifact, Resource, Activity, Event and dates

- Date: 2026-11-17
- Row verdict: SACM23-ART-002: PASS
- Row verdict: SACM23-ART-004: PASS
- Row verdict: SACM23-ART-005: PASS
- Row verdict: SACM23-ART-006: PASS
- Row verdict: SACM23-ART-007: PASS
- Verifier: `sacm-verifier` (independent; read-only), two rounds
- Commits:
  - 853d8b8 (round 1). Evidence: CI run
    [43911166157](https://github.com/lasrod/libSACM/actions/runs/43911166157), green on all
    jobs. PR-diff mutants: 144 caught, 50 unviable (`Default` for types without one); 0
    missed, 0 timed out.
  - 4457bbf (round 2). Its fix round changed only tests, goldens, docs and comments; no
    library code.
- Slice brief: [docs/slices/0055-m3-artifact-assets.md](../slices/0055-m3-artifact-assets.md)

## Result

- **The four kinds.** Artifact, Resource, Activity and Event are read, written, created,
  edited and compared in slice 0052's pattern (ADR 0034 amendment 1).
- **Dates** are held exactly as written, with no diagnostic (D-DATE).
- **Resource `location`** follows the MultiLangString rules of SACM23-BASE-004.
- **`occurece`**, the model's spelling, stays a tolerance (SACM23-XMI-009).
- **Quotations** checked against formal/23-05-08 (§4, §8.5, §12.7, §12.9–12.11), XMI 2.5.1
  (rules 4d and 4j, §7.11.6) and ptc/22-03-13.
- **The red snapshot** compared with the final tests: no step-1 test was weakened. The only
  changes were the gates and the ones the brief logs.
- **The reworded verified rows** are true of the code: ART-001, ART-003, VAL-010, VAL-020,
  LIB-RT-001, LIB-RT-002, XMI-007, XMI-014, XMI-016 and LIB-COMPAT-007. XMI-016's test still
  derives every unread kind from `ElementKind::ALL`.

Evidence beyond CI:
- about 30 hand mutations of the reader, writer order, escaping, compare order, edits,
  records and validation. All are killed after round 2.
- behaviour probes:
  - a Property under each of the four kinds;
  - a duplicate location id;
  - location ids freed on delete;
  - an unchanged SetLocation;
  - serde round-trips of every new refusal.

## Findings

| Severity | Row | Finding | Resolution |
|---|---|---|---|
| blocking | ART-002, -004, -005 | Only `date` had a serde golden for `FeatureNotApplicable`. Dropping `location`, `version`, `startTime` or `endTime` from `feature_name` survived. | Four golden refusals added. All four drops are killed. |
| blocking | ART-002 | No test showed that string attributes come before preserved attributes (M40 survived). | New test `sacm23_art_002_string_attributes_are_written_before_preserved_attributes`. |
| blocking | ART-004 | The `XMI_UNREAD_CONTENT` rustdoc named an Artifact as a kind not read yet. | It now names an ArtifactAssetRelationship. |
| non-blocking | ART-002, -005 | The `xmi::write`, `UnrepresentableCharacter`, model and diagnostics-catalog texts left out the new attributes and `location`. | Reworded with the verifier's text. |
| non-blocking | ART-003, XMI-014 | Matrix phrases: where CreateProperty under the four kinds is tested; `location` in XMI-014's `value` sentence. | Reworded by the lead. |
| non-blocking | XMI-016 | The test helper's `property` arm listed three assets. | Lists all seven. |
| non-blocking | — | The D18 entry said the Semantics sections are informative. | Reworded. |

## Logged

- The location-specific strict-write refusal (LIB-COMPAT-007) and the empty `location` id
  read tolerantly (VAL-020) are covered only through shared code.
- A missing `location` gives no finding, although UML's default multiplicity is 1. That is
  D-SEV policy, recorded in the D5 addendum.
