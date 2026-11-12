# Verification 0027 — Validation: a missing name

- Date: 2026-11-12
- Row verdict: SACM23-VAL-018: PASS
- Verifier: `sacm-verifier` (independent; read-only), 2 rounds; `sacm-docs-reviewer`, 1 round
- Commit: f068309. Evidence:
  - CI runs 43168348007 (d6f4f44) and 43172348755 (f068309);
  - full mutants run 43168360154 (d6f4f44): 905 caught, 0 missed, 0 timed out. The library
    code is unchanged since then.
- Slice brief: [docs/slices/0027-m1-validate-missing-name.md](../slices/0027-m1-validate-missing-name.md)

## Result

`validation::validate` (ADR 0018) reports each nameless ModelElement as one SACM-MULT-001
Error:
- with row SACM23-VAL-018, clause 8.6, the element's id and no location;
- with the message `{Kind} {id:?} has no name (name:LangString[1])`.

It keeps the element and sorts the result. It reports nothing for UtilityElements, or for
names with absent or empty fields. `has_errors` and `Code::MULTIPLICITY_VIOLATION` are added,
and the D-SEV rule for multiplicities is in place. The quotations were checked against
formal/23-05-08 §8.6 and §8.3, ptc/22-03-13 and XMI 2.5.1 §7.8.2. Every hand mutation but one
equivalent mutant is killed, and the full cargo-mutants run is clean.

## Findings

| Severity | Row | Finding | Resolution |
|---|---|---|---|
| Medium | SACM23-VAL-018 | The `{:?}` quoting of the id was unstated and untested | The requirement says so; a literal awkward-id test (f068309) |
| Low | SACM23-VAL-018 | The crate README claimed no diagnostic is reported | README and front-page example updated (f068309) |
