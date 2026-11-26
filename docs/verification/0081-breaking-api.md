# Verification 0081 — The breaking changes for 0.5.0

- Date: 2026-11-26
- Row verdict: LIB-API-020: PASS
- Row verdict: LIB-DIAG-004: PASS
- Row verdict: LIB-DIAG-005: PASS
- Row verdict: SACM23-XMI-032: PASS
- Verifier: `sacm-verifier` (independent; read-only)
- Commit: a743d8b (first round 1f9094e); evidence: CI 45823986895 (1f9094e, green), CI 45834594390 (a743d8b); hand mutations M1–M28, logs `target/verify-0081-mut*.log`
- Slice brief: [docs/slices/0081-breaking-api.md](../slices/0081-breaking-api.md)

## Result

The verifier ran the gate, compared the tests with the red snapshot, checked every template
argument for prose, and applied 28 hand mutants. JSON and every `Display` are unchanged
byte for byte, apart from the three EMF `version` lines of the 0.1.0 messages golden that
SACM23-XMI-032 intends. The CLI's behaviour does not change. In the first round two prose
sites survived mutation back to `Text`; the fix round added controls that kill both.

## Findings

| Blocking / non-blocking | Row | Finding | Resolution |
|---|---|---|---|
| Blocking | LIB-DIAG-004 | M13: `InvalidChar2`'s `expected` could go back to `Text` unnoticed. | input.rs unit assertion and a `<r a=b/>` case in prose_arguments.rs. |
| Blocking | LIB-DIAG-004 | M11: the unheld-outside refusal could go back to `Text` unnoticed. | `unheld_outside` extracted and unit-tested. |
| Non-blocking | SACM23-XMI-032 | The CHANGELOG did not say the template id changes for an EMF `version`. | A Changed entry. |
| Non-blocking | all | Wording: stale "default tolerant" comments and messages, the `# Modes` sentence on schema validity, a broken assertion message, the matrix source page, the prose_arguments.rs module doc. | Fixed with the verifier's text. |
