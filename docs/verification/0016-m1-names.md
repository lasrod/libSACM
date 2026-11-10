# Verification 0016, round 1 — m1-names

- Date: 2026-11-10
- Row verdict: SACM23-BASE-002: PASS
- Row verdict: SACM23-BASE-003: PASS
- Re-verified: LIB-API-004 (`InsertError` gains `NameNotAllowed`) and LIB-API-002 (helpers pass `None`): both still hold.
- Verifier: `sacm-verifier` (independent; read-only), at full depth (library rows)
- Commit: `e9a9504` (code unchanged since `e4b26aa`). Evidence: ci.yml 42764387336 and mutants.yml 42764387328, both success. PR diff: 15 caught, 0 missed. Full run: 556 caught, 0 missed.
- Slice brief: [docs/slices/0016-m1-names.md](../slices/0016-m1-names.md)

## Result

- **Gates:** all pass.
- **The standard:** the quotes from §8.3, §8.5, §8.6, §8.7 and §12.14, the ptc properties and defect D8 match the sources. The C++ citations in ADR 0013 are accurate.
- **Hand mutations:** 16 tried. 14 caught. One survivor is equivalent (`is_a(ArtifactElement)`: every concrete ModelElement is one). One, Hash over sorted values, breaks no obligation.
- **API boundary:** without `test-support`, only `MultiLangString` and `SacmElement::name` are new. Every new item is documented.
- **Homes:** every obligation the two rows dropped has a row. ADR 0013 and its amendment agree with the rows that inherit them.

## Findings

| Severity | Row | Finding | Required fix |
|---|---|---|---|
| Medium | matrix | The C++ tolerant name and text shapes had no row. | New row LIB-COMPAT-008; BASE-004 points to it. Done. |
| Medium | XMI-014 | Both forms present, or a repeated element form, was unspecified. | Keep every value with an Error diagnostic; strict save refuses. Done. |
| Medium | XMI-012, -014, -015, LIB-COMPAT-007 | The XMI 2.5.1 citations cannot be checked: the text is not fetched. | New row LIB-TOOL-020, which those rows depend on. Done. |
| Low | XMI-004, LIB-RT-001, BASE-005, ART-005 | Wording: C++ reports a LangString `xmi:id` only; "empty name" was vague; the MultiLangString XMI rule was in BASE-004 only. | Reworded. Done. |

Logged, not required:
- VAL-010 reads "both `lang` absent" as the same language. Record that in `decisions.md` when VAL-010 is analyzed.
