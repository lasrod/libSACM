# Verification 0037 — Element form of values

- Date: 2026-11-13
- Row verdict: SACM23-XMI-014: PASS
- Row verdict: SACM23-XMI-010: PASS
- Row verdict: SACM23-XMI-012: PASS
- Row verdict: SACM23-XMI-016: PASS
- Row verdict: SACM23-XMI-017: PASS
- Row verdict: LIB-API-009: PASS
- Row verdict: LIB-API-010: PASS
- Row verdict: LIB-RT-001: PASS
- Row verdict: LIB-COMPAT-007: PASS
- Verifier: `sacm-verifier` (independent; read-only), 2 rounds; `sacm-docs-reviewer`, 1 round
- Commit: dee5036. Evidence: CI run 43417741895 (PR-diff mutants: 28 caught, 0 missed). Under ADR 0006 amendment 3 there is no per-slice full run; the cases removed from earlier tests now read by design, and the code behind them is in this diff.
- Slice brief: [docs/slices/0037-m1-element-form-values.md](../slices/0037-m1-element-form-values.md)

## Result

`xmi::read` reads a `name`'s `lang` and `content`, and a Claim's `assertionDeclaration`, from
value elements as well as attributes. Both forms read equal, and `write` writes the
attribute form. The value is the element's text, exactly.

A value given twice is `ReadError::RepeatedValue` (SACM-MULT-001), naming every occurrence,
in both modes and even when the values are equal (ADR 0021 amendment 1). Each occurrence is
checked whole before it counts as a repeat. Eighteen hand mutations were run in round 1 and
seven in round 2; all are killed except one location-list mutation, which is on the
follow-ups list. Every quote was checked against XMI 2.5.1.

## Findings

| Severity | Row | Finding | Resolution |
|---|---|---|---|
| Blocking | LIB-COMPAT-007 | The new one-Error refusal wording was pinned only by a doctest (a missed mutant) | Pinned in a test (dee5036) |
| Blocking | SACM23-XMI-014 | The check-whole-before-repeat order was untested (two hand mutants survived) | New precedence test (dee5036) |
| Blocking (docs review) | SACM23-XMI-014 | The read-mode advice said a strict read reports every variation | Says what it recognises, and that neither mode validates |
| Follow-up | several | Rule 2h citations, the XMI-021 title, the XMI-012 sentence, D-STRICT, notes | Fixed |
| Follow-up | SACM23-XMI-014 | A malformed later occurrence in `locations` is untested | On `docs/follow-ups.md` |
