# Verification 0015, round 1 — m1-value-types

- Date: 2026-11-10
- Row verdict: SACM23-BASE-001: FAIL
- Row verdict: SACM23-ARG-003: FAIL
- Verifier: `sacm-verifier` (independent; read-only), at full depth (library rows)
- Commit: `1b73f09`. Evidence: ci.yml 42737720081 and mutants.yml 42737720128, both success. In `lang_string.rs` and `argumentation.rs`, 17 mutants caught, 0 missed.
- Slice brief: [docs/slices/0015-m1-value-types.md](../slices/0015-m1-value-types.md)

## Result

**Holds:**
- **Gates:** all pass.
- **The standard:** every quotation in the acceptance matches formal/23-05-08 and ptc/22-03-13. ADR 0012's C++ citations are accurate.
- **Hand mutations:** 29 of 30 caught. The survivor, `pad` → `write_str` in `Display`, is equivalent.
- **Review items:** `#[non_exhaustive]`, private fields, no `id()`/`kind()`, and only the three approved items are public.

**Fails:** the matrix narrowing left three obligations without a row. The code and tests need no change.

## Findings

| Severity | Row | Finding | Required fix |
|---|---|---|---|
| Medium | BASE-001 | F1. No row requires absent and empty to round-trip through XMI (ADR 0012). | Add it to XMI-011, with all nine combinations. |
| Medium | BASE-001 | F2. After narrowing XMI-004, an `xmi:id` on a LangString belongs to no row. C++ reports it at Info. | Add it to XMI-004: not kept, reported with a diagnostic naming the id, never dropped silently. |
| Medium | ARG-003 | F3. `assertionDeclaration` on asserted relationships, and absent meaning `asserted`, belong to no row. | Add it to ARG-007, and the absent and default-writing rules to ARG-002 and ARG-007. |
| Low | ARG-003 | F4. The `AsCited` doc drops "should" and "transitively" (§11.8). | Reword the doc. |

Logged, not required:
- C++ reads content from element text when there is no `content` attribute. The XMI or COMPAT rows decide.
- No VAL row exists yet for the asCited derivation.
- BASE-009 decides whether to add `expression_ref` to LangString.
