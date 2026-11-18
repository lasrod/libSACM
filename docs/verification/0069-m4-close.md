# Verification 0069 — M4 close

- Date: 2026-11-18
- Milestone: M4, Terminology model: 12/12 verified (records 0062, 0063, 0064 and 0065).
- Audit: `matrix-steward`, read-only, on 4f4a281. The lead checked the docs fixes against the
  code; they are wording only, so there is no verifier round (sacm-slice step 5.4).
- Slice brief: [docs/slices/0069-m4-close.md](../slices/0069-m4-close.md)

## Audit findings and what was done

| Finding | Done |
|---|---|
| The `Document` rustdoc said the release reads three package roots and 24 kinds (false since slice 0062). | It now says four roots, every kind but the eight interface and binding kinds, and 30 creates. The builder checked the counts against `ElementKind::ALL`, `Unread::kind` and `Operation`. |
| `lib.rs` and both READMEs did not mention ExpressionLangString or the M4 rules (LIB-DOC-001). | Added. |
| LIB-API-018 did not say that the CLI JSON carries no templates. | A note. |
| LIB-API-018's `depends_on` left out the rows whose templates it counts. | Added: BASE-009, VAL-009, VAL-017, TERM-003 and XMI-028. |
| LIB-DIAG-002 gave a stale live-code count (32). | Now "42 after slice 0065". |
| M4's goal left out the refusal and diagnostic templates. | Reworded. |

## Follow-ups cleared

| Item | Disposition |
|---|---|
| 0061: the `Document` docs name `test-support` | Fixed by the rustdoc above. |
| 0063: `debug_assert_eq!` in `unsupported_at` | Dropped: the table test and `lib_api_017_each_kept_refusal_list_entry_has_the_code_of_its_tolerant_warning` already catch a wrong pairing. |
| 0063: should R11 check `reason`, `shape` and `element_name`? | The maintainer said to keep them as given; ADR 0030 Records now says so. |
| 0063: the mutants-triage row describes `Option<Code>` | Marked historical. |
| 0065: a JSON array LangString is now refused | Recorded under Breaking in the CHANGELOG. |
| 0065: `reference_tokens` reserves the ids of a preserved `expression` attribute | Dropped as deliberate; BASE-009's acceptance wording now says so. |
| 0064: R11's in-order arguments against `serde_json::Value` | The maintainer chose to accept any order: new row LIB-API-019 (M5, planned) and ADR 0030 amendment 5 (accepted). |
| 0064: the construction check misses qualified paths | Dropped: `Diagnostic::new` exists only with `test-support`, so a default build fails on it. |
