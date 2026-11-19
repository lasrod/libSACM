# Verification 0071 — M5 close

- Date: 2026-11-19
- Milestone: M5, Modularity: interfaces and bindings, 15/15 verified (records 0066, 0067, 0068, 0070).
- Audit: `matrix-steward`, read-only, on the slice 0068 branch. The lead checked the fixes against the code. They are wording and test-data only; the new test values have a killed mutant. No verifier round was run (sacm-slice step 5.4).
- Slice brief: [docs/slices/0071-m5-close.md](../slices/0071-m5-close.md)

## Audit findings and what was done

| Finding | Done |
|---|---|
| Seven M5 acceptance items still said "gives none in this slice" or described the bounds before slice 0067 (AC-002, AC-003, ARG-014 twice, ARG-015, TERM-007, TERM-008). | Reworded to the behaviour the tests show. |
| `lib.rs` "What works today" did not mention M5. | Added: interfaces, bindings, `href`s, the modularity checks and the creates. |
| The crate README did not mention `href`s or the modularity checks. It still planned editing that is done, and said `sacm = "0.1"`. | Fixed. |
| The README Status stopped at M3 and 0.1.0. | Now says M0 to M5, 0.1.0 and 0.2.0. |
| No edit can create a cross-document reference. | New candidate row LIB-EDIT-020 (M6). The CHANGELOG says so. |
| Dependencies were missing (VAL-013, -014, -015; CP-002 to -004 on VAL-023). | Added. |
| The XMI-027 white-space clause was tested only in the fragment. | Two values added; the fragment-only mutant is killed. |
| Wording: ARG-015, the eight interface and binding rows, XMI-015, LIB-EDIT-019, LIB-API-017, broken links, the lib.rs `ArtifactReference` sentence. | Fixed. |

## Follow-ups cleared

| Item | Disposition |
|---|---|
| 0067: the edit_atomic set lacked the Terminology and interface creates. | Done in slice 0070. |
| 0068: `invalid/reference-cross-document-href.sacm.xmi` reads cleanly. | Moved to `reference-cross-document-href-valid.sacm.xmi`; LIB-RT-002 covers it. |
| 0068: `invalid/modularity-findings.sacm.xmi` reads. | Kept in `invalid/`, because it gives `validate` Errors by design; its round trip is tested. |
| Verification 0068: an MSRV step for the documented gate. | `cargo +1.85 check -p sacm --all-features --all-targets` added to CLAUDE.md and CONTRIBUTING.md (the maintainer, 2026-11-19). |
