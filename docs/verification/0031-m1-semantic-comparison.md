# Verification 0031 — Comparing documents

- Date: 2026-11-13
- Row verdict: LIB-RT-001: PASS
- Row verdict: LIB-RT-002: PASS
- Row verdict: LIB-RT-003: PASS
- Verifier: `sacm-verifier` (independent; read-only), 2 rounds plus a re-check of the Copilot fix; `sacm-docs-reviewer`, 1 round
- Commit: 15a2c4a. Evidence:
  - CI run 43351776336 (PR-diff mutants: 21 caught, 0 missed);
  - full mutants run 43351770072 (15a2c4a): 981 caught, 0 missed, 0 timed out;
  - earlier, at f53f06f: CI 43348452354 and full mutants run 43348446949 (983 caught, 0 missed).
- Slice brief: [docs/slices/0031-m1-semantic-comparison.md](../slices/0031-m1-semantic-comparison.md)

## Result

`compare::semantic_compare` lists every difference between two documents by SACM content,
matched by id, in the fixed order ADR 0020 sets. `Document: Eq` means it finds none.
Exhaustive destructures make a new model field fail to compile until it is compared.

Every readable fixture loads, saves and reloads equal and writes the same bytes; the one
pending golden is checked to be still refused. Documents built with `edit::apply` round-trip
equal and valid. Thirteen hand mutations (order, parent, root-ness, recursion, kind,
name, fixture walk) are killed.

## Findings

| Severity | Row | Finding | Resolution |
|---|---|---|---|
| Medium | LIB-RT-001 | No test separated right's pre-order from its insertion order (a hand mutant survived) | Test extended; the mutant is killed (f53f06f) |
| Low | LIB-RT-001 | Six `Display` mutants survived (the values were not checked) | `lib_rt_001_display_shows_both_values` (830dc74) |
| Low | LIB-RT-002 | One writer golden holds kinds no row reads | Named pending list, checked to be still refused |
| Docs | LIB-RT-001 | Nothing said how to compare ignoring order | Applied (docs review), with a doctest |
| Low | LIB-RT-001 | Copilot: `==` built every difference only to test emptiness | One shared walk; `==` stops at the first difference and builds none (15a2c4a); re-checked, 25 hand mutants killed |
| Logged | LIB-RT-001 | The linear-time claim is untested | Not in the acceptance |
