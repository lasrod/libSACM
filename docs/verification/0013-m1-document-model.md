# Verification 0013, round 1 — m1-document-model

- Date: 2026-11-09
- Row verdict: LIB-API-002: FAIL
- Row verdict: LIB-API-003: FAIL
- Verifier: `sacm-verifier` (independent; read-only), at full depth (library rows)
- Commit: `871764a`. Evidence: ci.yml 42621944735 and mutants.yml 42621944781, both success. In `src/model`, 50 mutants caught, 17 unviable, 0 missed.
- Slice brief: [docs/slices/0013-m1-document-model.md](../slices/0013-m1-document-model.md)

## Result

**Holds:**
- **Gates:** all pass.
- **Against the standard:** the counts (38 kinds, 13 abstract classes, 12 package kinds) and the names match ptc/22-03-13 and formal/23-05-08 §§11.3 and 12.14. `is_a` and `is_kind_of` are exact.
- **Against the C++ design:** it follows `element_kind.h`, `document.h` and `element_id.h`, apart from ADR 0010's D1–D3.
- **The rest:** no storage or mutation is public without `test-support`, the enums are `#[non_exhaustive]`, output is deterministic, and nothing recurses or panics.

**Fails:** hand mutations show defects the acceptance rules out but no test catches.

## Findings

| Severity | Row | Finding | Required fix |
|---|---|---|---|
| High | LIB-API-003 (item 2) | Nothing pins each variant's own name. Swapping two names within a group survives (M1–M3, M5, M16, M17); M4 is caught only by a doctest. | A test pins each of the 38 `ElementKind` names and the 13 `AbstractClass` names to its variant, with a table or `name() == format!("{:?}")`. |
| High | LIB-API-002 (requirement and item 2, D-ID) | Exact id preservation is untested. Storing a trimmed, lower-cased or ASCII-stripped id survives (M13–M15). | Insert ids such as `UPPER`, `MixedCase`, `Ünïcödé-ид-標識`, `_acp.1` and `  padded  `. Assert that the bytes are preserved through `get`, `roots` and `elements`, and that the normalised forms are not found. |
| High | LIB-API-002 (item 7) | Exact duplicate detection is untested. Refusing `Claim_top` because `claim_top` exists survives (M18b). | With `claim_top` present, `Claim_top` and `claim_top ` are both accepted as distinct elements. |

Logged, not required:
- the `InsertError` variant per refusal (M8, M12);
- ids made only of whitespace (M7);
- packaging: the path-only self dev-dependency and the `include_str!` of a repository file.
