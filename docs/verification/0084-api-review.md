# Verification 0084 — Public API review before 1.0

- Date: 2026-11-27
- Row verdict: LIB-API-005: PASS
- Row verdict: LIB-API-022: PASS
- Row verdict: LIB-API-023: PASS
- Row verdict: LIB-API-024: PASS
- Row verdict: LIB-API-025: PASS
- Verifier: `sacm-verifier` (independent; read-only)
- Commit: fb3c567 (first round 7cce175); evidence: CI 45954455790 (7cce175, green; PR-diff mutants 0 missed), CI 45959047591 (fb3c567); hand mutations, logs `target/verify-0084-mutants.log`, `-mutants2.log`, probe `target/verify-0084-probe/`
- Slice brief: [docs/slices/0084-api-review.md](../slices/0084-api-review.md)

## Result

The verifier ran the gate, compared the tests with the red snapshot (the href rename changed
names only), counted the 55 constructors that take a name, confirmed every operation's JSON
is unchanged, probed 24 snippets of 0.4 user code for inference breaks, spot-checked the
public API against the Rust API Guidelines, and applied 47 hand mutants, all killed. Both
rounds passed; the fix round settled non-blocking findings.

## Findings

| Blocking / non-blocking | Row | Finding | Resolution |
|---|---|---|---|
| Non-blocking | LIB-API-022 | `impl Into<LangString>` breaks inference for a name typed by the parameter. | A Breaking entry in the CHANGELOG. |
| Non-blocking | LIB-API-023 | `Href` lacked the `str` comparison `ElementId` has; adding it after 1.0 would break inference. | Added, with `Borrow<str>`, tests and a Breaking entry. |
| Non-blocking | LIB-API-023 | `Display` padding was untested. | Padding asserts. |
| Non-blocking | LIB-API-005 | ADR 0045 did not list what was checked and found compliant, nor every additive gap. | The checked list and the deferred items in ADR 0045. |
| Non-blocking | LIB-API-024, -022, -025 | Wording: `Mutation::is_empty` after a root reorder, the `LangString` conversion, ADR 0040's old names. | Fixed with the verifier's text. |
| Non-blocking | LIB-API-005 | Auto traits of `impl Iterator` returns are public through leakage, untested. | docs/follow-ups.md. |
