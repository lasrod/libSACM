# Verification 0030 — assertionDeclaration in XMI

- Date: 2026-11-13
- Row verdict: SACM23-XMI-017: PASS
- Row verdict: LIB-API-010: PASS
- Row verdict: SACM23-XMI-016: PASS
- Verifier: `sacm-verifier` (independent; read-only), 1 round; `sacm-docs-reviewer`, 1 round
- Commit: 2b1f92a. Evidence:
  - CI run 43324245436 (PR-diff mutants: 19 caught, 0 missed);
  - full mutants run 43324261474 (2b1f92a): 960 caught, 0 missed, 0 timed out.
- Slice brief: [docs/slices/0030-m1-assertion-declaration-in-xmi.md](../slices/0030-m1-assertion-declaration-in-xmi.md)

## Result

`xmi::read` reads a Claim's `assertionDeclaration` as one of the six literals, matched
exactly. An explicit `asserted` loads equal to an absent one. Any other value is
`Unsupported` at the attribute's name, quoting the value, and is never read as `asserted`.
`xmi::write` writes every non-default declaration directly after `xmi:id` on all six
Assertion kinds, byte-stable on a round trip. `WriteError::Unsupported` is removed (ADR 0017
amendment 2). A completeness test shows that every feature a `Document` holds is written.

LIB-API-010 and SACM23-XMI-016 were re-checked after their wording changed. SACM23-XMI-010
and LIB-API-009 still hold with their updated table and sample. Nine hand mutations were
killed: namespace, trimming, location, default filter, kind check, storage, case,
reason and attribute order. Every quote was checked against the local normative sources.

## Findings

| Severity | Row | Finding | Resolution |
|---|---|---|---|
| Low | LIB-API-010 | Amendment 1's check-order bullet still named the declaration refusal | Marked superseded by amendment 2 |
| Low | SACM23-XMI-017 | The §11.11 `assertionDeclared` slip had no defect record | Recorded as D12 in `specification-defects.md` |
| Low | LIB-API-010 | From outside the crate, "exactly two variants" is shown only as "every reachable refusal is one of the two" | Logged: an in-crate exhaustive-match test would make it mechanical |
| Low | SACM23-XMI-017 | The XSD 1.0 and XML 1.0 quotes have no local copy | Logged; the exact-match behaviour does not depend on them |
| Docs | SACM23-XMI-017 | The docs did not say why a misspelt value is refused, or how to fix it | Applied (docs review) |
