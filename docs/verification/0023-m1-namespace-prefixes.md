# Verification 0023 — Namespace-prefix independence

- Date: 2026-11-12
- Row verdict: SACM23-XMI-002: PASS
- Verifier: `sacm-verifier` (independent; read-only), 2 rounds
- Commit: 6fe4541. Evidence:
  - CI run 43116528967;
  - full mutants run 43116522908: 848 caught, 0 missed, 0 timed out.
- Slice brief: [docs/slices/0023-m1-namespace-prefixes.md](../slices/0023-m1-namespace-prefixes.md)

## Result

18 tests check each of the 12 acceptance items through `xmi::read`, and they show that the
choice of prefix never changes the result:
- any SACM, XMI or XSI prefix, and default namespaces;
- declarations on the wrapper or on the root, and a prefix rebound on an inner element;
- type values resolved as QNames, with XSD white-space collapse;
- exact URI comparison;
- refusals with the same kind, code and location under any prefix.

The verifier checked every quotation against XMI 2.5.1, Namespaces 1.0 and XML Schema 1.0.
Every hand mutation of the trim, the namespace filter and the QName resolution is killed.

## Findings

| Severity | Row | Finding | Resolution |
|---|---|---|---|
| Medium | SACM23-XMI-002 | `str::trim()`, which strips Unicode white space, survived every test | U+00A0 and U+0085 controls; acceptance item 7 says so (6fe4541) |
