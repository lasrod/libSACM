# Verification 0019, round 2 — m1-xml-input-safety

- Date: 2026-11-11
- Row verdict: LIB-SEC-001: PASS
- Row verdict: LIB-SEC-002: PASS
- Row verdict: LIB-API-009: PASS
- Re-checked: LIB-DIAG-001 (item 4 reworded) and LIB-API-004 still hold. `ReadError` is proven by LIB-SEC-002 item 6 and the Display items, following the ARG-002 precedent.
- Verifier: `sacm-verifier` (independent; read-only), at full depth (library rows)
- Commit: `b8f2f95`. Evidence: ci.yml 42944385159 and mutants.yml 42944385088, both success. PR diff: 107 caught, 0 missed. Full run: 675 caught, 0 missed.
- Slice brief: [docs/slices/0019-m1-xml-input-safety.md](../slices/0019-m1-xml-input-safety.md)

## Result

- **Gates:** all pass, including MSRV 1.85 and a lib build without features.
- **roxmltree 0.21.2 checked against its source:**
  - DTDs are off by default.
  - `DtdDetected` is raised before the internal subset is read.
  - Nested content is parsed iteratively.
  - `TextPos` counts LF only, and `offset_of` inverts it.
- **C++ and the standard:** the C++ raw scan and the deviations in ADR 0005 (B1 to B3) and ADR 0015 are named correctly. SACM 2.3 clause 2 holds.
- **Hand mutations:** 43 tried. Every one that an acceptance item depends on is killed by traced tests. Four private-helper details are killed only by unit tests (M04, M19, M26, M42). One survives (M43): two leading U+FEFF are located as if both were a BOM, which only malformed input can show.
- **Probes:** about 30 hostile inputs, including 70,000 namespaces and 200,000 attributes. Each returns a value, with no panic and no recursion.
- **API boundary:** only `sacm::xmi::{read, ReadOptions, ReadOutput, ReadError}` and the two `Code` constants are new. No roxmltree type is public.

## Findings

| Severity | Round | Row | Finding | Resolution |
|---|---|---|---|---|
| Medium | 1 | LIB-API-009 | Case-insensitive UTF-8 and ASCII declarations were unproven (M15). | Item 3 tightened; the refusal is at the root, not the encoding name. |
| Medium | 1 | LIB-API-009 | Parser capacity limits had no traced test (M25). | Moved to LIB-SEC-003. The unit test covers all three limits. |
| Low | 1 | LIB-API-009 | Encoding-refusal and ASCII locations were checked by unit tests only (M11, M17, M18, M22). | Traced location assertions added. |
| Low | 1 | LIB-SEC-001 | A DOCTYPE after a BOM was untested (M02). | A leading BOM takes no column, and a test was added. |
| Low | 1 | LIB-SEC-002 | No reason text was asserted exactly (M27). | The exact `<r></s>` Display is now asserted. |
| Low | 1 | ADR 0005 | B1 overclaimed: roxmltree is lenient in three cases. | B1 corrected. SACM23-XMI-001 must refuse or report those cases. |
| Low | 2 | LIB-API-009 | A missing full stop in item 2. | Fixed. |

Logged for LIB-SEC-003: many attributes and namespaces cost quadratic time in roxmltree, and roxmltree has a `debug_assert` on input over 4 GiB.
