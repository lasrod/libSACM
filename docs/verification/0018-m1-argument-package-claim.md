# Verification 0018, round 1 — m1-argument-package-claim

- Date: 2026-11-10
- Row verdict: SACM23-ARG-001: PASS
- Row verdict: SACM23-ARG-002: PASS
- Re-checked: LIB-API-002, LIB-API-004, LIB-API-008 and SACM23-ARG-003 still hold. LIB-API-004 stays closed: ARG-002 acceptance item 7 proves the ADR 0011 obligations for `SetAssertionDeclarationError`, as ARG-003 did for its parse error.
- Verifier: `sacm-verifier` (independent; read-only), at full depth (library rows)
- Commit: `61671a7`. Evidence: ci.yml 42817828038 and mutants.yml 42817827932, both success. PR diff: 5 caught, 0 missed. Full run: 568 caught, 0 missed. The docs review fixes that followed change rustdoc and READMEs only.
- Slice brief: [docs/slices/0018-m1-argument-package-claim.md](../slices/0018-m1-argument-package-claim.md)

## Result

- **Gates:** all pass.
- **The standard:** the quotes from formal/23-05-08 §2.2, §11.3, §11.4, §11.8, §11.10 and §11.11 match the sources, as does the ptc default value `asserted`. So do the XMI 2.5.1 quotes, §9.4.1 (p. 57) and §7.8.4 (p. 18), checked against an unpinned download; pinning it is LIB-TOOL-020. The C++ citations are accurate.
- **ARG-001:** it adds no code. Its 9 tests characterise behaviour that LIB-API-008 and LIB-API-002 already provide, as with SACM23-AC-001.
- **Hand mutations:** 24 tried, 23 killed: storage initialisation, the accessor, the setter's order and its write, clone, Display, and the containment variants. The survivor is equivalent: `{kind:?}` and `{kind}` print the same text.
- **API boundary:** without `test-support`, only `SacmElement::assertion_declaration` is new.

## Findings

| Severity | Row | Finding | Resolution |
|---|---|---|---|
| — | — | None blocking. | — |

Logged, not required:
- LIB-API-004 item 1 still names only `InsertError`. Reword it when that row is next touched.
- LIB-EDIT-008 should map both setter refusals to the C++ refusal code.
