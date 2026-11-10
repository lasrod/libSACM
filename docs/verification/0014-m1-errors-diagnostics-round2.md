# Verification 0014, round 2 — m1-errors-diagnostics

- Date: 2026-11-10
- Row verdict: LIB-DIAG-001: PASS
- Verifier: `sacm-verifier` (independent; read-only), at full depth (library row)
- Commit: `9d7bdc8`. Evidence: ci.yml 42723200945 and mutants.yml 42723200964, both success. In `diagnostic.rs`, 0 missed; every unviable mutant is `Default::default()` on a type without `Default`.
- Round 1: [0014-m1-errors-diagnostics.md](0014-m1-errors-diagnostics.md). LIB-API-004 passed there and has no regression.

## Result

- **The round 1 prescription holds.** The `# Order` rustdoc says text keys compare as `str` and affected ids compare id by id, with a prefix first. The within-key controls kill E1–E4, D7d and D7e. Adjacent duplicates kill E5.
- **ADR 0011 amendment 1 is correct.** `Display` escapes LF, CR, NEL, LS and PS, and `message()` stays exact. 21 hand mutations of the escaping were killed. The 2 that survive are equivalent.
- **Gates:** all pass, locally and in CI.

Logged, not required:
- VT and FF are not escaped. XML 1.0 cannot carry them, so the five escaped characters are the line breaks XMI content can hold. Revisit if a producer puts non-XML text in a message.
- Each producer row needs its own sorted-output control.
