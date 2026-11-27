# Verification 0088 — Difference targets hold no ids; edits refuse unsavable text

- Date: 2026-11-27
- Row verdict: SACM23-XMI-031: PASS
- Row verdict: LIB-TOOL-024: PASS
- Row verdict: LIB-EDIT-025: PASS
- Verifier: `sacm-verifier` (independent; read-only)
- Commit: f073dc9, plus the verifier's wording; evidence: CI 46016466711 (green; the XML Schema job validates the new golden and outputs); 40 hand mutants, logs `target/verify-0088/mutants/`
- Slice brief: [docs/slices/0088-xmi031-edit-text.md](../slices/0088-xmi031-edit-text.md)

## Result

The verifier checked every quotation against the pinned files (XMI.xsd, XML Schema Part 1
§3.10.1 and §3.15.5, Part 2 §3.2.1, SACM 2.3 §2.4, XMI rule 4j), downloaded the Part 1 pin
independently, compared all 56 edit plans with the design's table, probed reads of
non-`Char` input in other encodings and target content that must round-trip, and confirmed
the 14 seam files changed setup only. 39 of 40 hand mutants were killed; the 40th and one
reasoned case are equivalent.

## Findings

| Blocking / non-blocking | Row | Finding | Resolution |
|---|---|---|---|
| Non-blocking | SACM23-XMI-031 | The row cited spec defect D33, which was not recorded; ADR 0042 did not address XMI §7.6.1. | D33 added; the ADR names §7.6.1. |
| Non-blocking | SACM23-XMI-031, LIB-EDIT-025 | Wording: the SACM-ID-002 row, `xmi::write`'s metadata paragraph, two stub lines, the CHANGELOG's "can now be written", architecture.md. | Fixed with the verifier's text. |
| Non-blocking | all | The log did not record a helper extraction in artifact_edits.rs. | Logged. |
| Non-blocking | LIB-TOOL-024, LIB-EDIT-025 | `cargo xtask` help lists fewer downloads than it fetches; the guarantee test does not exercise `ScrubReferences`. | docs/follow-ups.md. |
