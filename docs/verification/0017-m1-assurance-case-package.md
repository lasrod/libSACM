# Verification 0017, round 3 — m1-assurance-case-package

- Date: 2026-11-10
- Row verdict: SACM23-AC-001: PASS
- Row verdict: LIB-API-008: PASS
- Re-verified: LIB-API-004 (`InsertError` gains `ContainmentNotAllowed` and `RootNotAllowed`), LIB-API-002 and SACM23-BASE-003 (test setups moved under packages): all still hold.
- Verifier: `sacm-verifier` (independent; read-only), at full depth (library rows)
- Commit: `cb08073`. Evidence: ci.yml 42803429198 and mutants.yml 42803429180, both success. PR diff: 10 caught, 0 missed. Full run: 564 caught, 0 missed.
- Slice brief: [docs/slices/0017-m1-assurance-case-package.md](../slices/0017-m1-assurance-case-package.md)

## Result

- **Gates:** all pass (623 tests).
- **Containment:** the verifier re-derived the legal (parent, child) pairs from ptc/22-03-13: 339 of 1,444. They match the test's table exactly.
- **The standard:** the quotes from §2.4, §8.6, §9.2 to §9.4, §10.3, §10.4, §11.4, §12.2, §12.6 and §12.14 match the sources. ADR 0014's C++ citations are accurate.
- **Hand mutations:** all killed: H1 to H18, N1 to N3 and R1 to R9. These include "accept any root" and "refuse an interface or binding root".
- **Moved setups:** the test setups moved under a package weaken no assertion.
- **API boundary:** without `test-support`, nothing new is public.

## Findings

| Severity | Round | Row | Finding | Resolution |
|---|---|---|---|---|
| High | 1 | LIB-API-008, AC-001 | A computed role was never observed: two mutations that assign the wrong role survived. | `role_of` became `admits -> bool`. Role identity moves to SACM23-XMI-016, which checks all 339 pairs. |
| Low | 1 | LIB-API-008 | ADR 0010 D1 did not support unrestricted roots. | Superseded in round 2. |
| Medium | 2 | LIB-API-008 | The requirement cited C++ `other_roots`, but C++ refuses a non-package root (`kXmiInvalidRoot`). | The maintainer decided that roots must be package kinds: `InsertError::RootNotAllowed`. |
