# Verification 0015, round 2 — m1-value-types

- Date: 2026-11-10
- Row verdict: SACM23-BASE-001: PASS
- Row verdict: SACM23-ARG-003: PASS
- Verifier: `sacm-verifier` (independent; read-only), at full depth (library rows)
- Commit: `702d40a`. Evidence: ci.yml 42741372511 and mutants.yml 42741372203, both success. In the slice files, 17 caught, 0 missed.
- Round 1: [0015-m1-value-types.md](0015-m1-value-types.md)

## Result

- **F1 holds.** XMI-011 states that absent and empty values round-trip, for all nine combinations (ADR 0012).
- **F2 holds.** XMI-004 states that a LangString `xmi:id` is reported at Info with the id named, never dropped silently. This matches C++ `xmi_reader.cpp:794-802`.
- **F3 holds.** ARG-002 and ARG-007 state that an absent value reads as `asserted` and that the writer omits `asserted`. This matches C++ `xmi_writer.cpp:95-98`.
- **F4 holds.** The `AsCited` doc matches §11.8.
- **New docs:** accurate against formal/23-05-08 and ptc/22-03-13.
- **Gates:** all pass.

## Findings

| Severity | Row | Finding | Required fix |
|---|---|---|---|
| Low | ARG-003 | `Display` used `write_str`, so it ignored width and alignment, unlike the crate's other `Display` impls. The `pad` → `write_str` mutant was not equivalent. | Recommended: restore `f.pad` and add a width test. Done after this round: `sacm23_arg_003_display_honours_width_and_alignment` (red first), then `f.pad`. |

Logged, not required:
- XMI-004 discards a LangString `xmi:id` with a diagnostic. A line in `decisions.md` could say that XMI identity on a value type is not SACM content. Its slice also picks a diagnostic code.
- ARG-007 could repeat "all six literals round-trip".
- Still open: C++ reads `content` from element text, and no VAL row covers the asCited derivation.
