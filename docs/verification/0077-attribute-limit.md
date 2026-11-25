# Verification 0077 — An element's attributes are limited

- Date: 2026-11-25
- Row verdict: LIB-SEC-007: PASS
- Verifier: `sacm-verifier` (independent, read-only), two rounds.
  - **Round 1, e4de6a1:** FAIL (one blocking finding and its test gap).
  - **Round 2, 3406ef4:** PASS against the round-1 prescription.
- Evidence: CI runs 45575279876 (round 1: PR-diff mutants 10 caught, 0 missed) and
  45582867156 (round 2); hand mutations in both rounds; release-build probes; the red
  snapshot of step 1.
- Slice brief: [docs/slices/0077-attribute-limit.md](../slices/0077-attribute-limit.md)
- Decisions: [ADR 0041](../adr/0041-read-limits-and-other-sacm-namespaces.md) amendment 2
  (the maintainer, 2026-11-25: a limit, not a fork; default 256); D-STRICT.

## Result

- **LIB-SEC-007.** `Limits::with_max_attributes` (default 256) bounds the attributes of one
  element; the first namespace declaration of each prefix is not counted, a repeated one
  is. The pre-parse scan refuses an over-limit element at its `<` with
  `LimitExceeded` / `Limit::Attributes` (`SACM-SEC-003/attributes`), in both modes, after
  the depth, bindings and copies limits, wherever the element stands. roxmltree 0.21.2's
  quadratic duplicate-attribute check is then capped, and a read at the defaults is
  linear: one element of 100,000 attributes is refused in 0.29 s debug (it took 128 s to
  270 s), and 40,000 repeated `xmlns=""` over 40,000 Claims in 1 ms release (3.7 s in
  round 1).
- **Tests.** 10 `attribute_limit.rs` tests and two private unit tests; every hand mutant of
  the new logic is killed, including a linear-search repeat check (41.7 s against 0.17 s
  on the K6 input).

## Findings

| Blocking / non-blocking | Row | Finding | Resolution |
|---|---|---|---|
| Blocking | LIB-SEC-007 | Repeated `xmlns` / `xmlns:xml`, which roxmltree does not refuse and the slice left uncounted, kept a read at the defaults quadratic. | A declaration that repeats a prefix in the same start tag counts (O(1) per declaration); requirement, ADR 0041 D8 and docs reworded (round 2). |
| Blocking | LIB-SEC-007 | No test covered repeated declarations. | `lib_sec_007_a_repeated_namespace_declaration_counts`, controls K1 to K6 (round 2). |
| Non-blocking | LIB-SEC-002 | Strict mode accepts a duplicated `xmlns` or `xmlns:xml` (XML 1.0 §3.1 WFC: Unique Att Spec), before this slice. | New row LIB-SEC-008 (planned, M6), requirement as the verifier worded it. |
| Non-blocking | LIB-SEC-007 | `xmi.rs` "# Limits", `ReadError::LimitExceeded` and the READMEs did not name the new limit fully. | The verifier's text. |
