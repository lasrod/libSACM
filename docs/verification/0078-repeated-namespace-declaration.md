# Verification 0078 — A repeated namespace declaration is malformed XML

- Date: 2026-11-25
- Row verdict: LIB-SEC-008: PASS
- Verifier: `sacm-verifier` (independent, read-only), one round: PASS at d3268d9; its
  wording was then applied with its text.
- Evidence: CI run 45599536829 (PR-diff mutants shards 0, 2 and 3: 7 caught, 0 missed);
  15 hand mutants; a probe of 28 inputs in both modes; a scan of the W3C suite and the
  corpus; the red snapshot of step 1.
- Slice brief: [docs/slices/0078-repeated-namespace-declaration.md](../slices/0078-repeated-namespace-declaration.md)
- Decisions: D-STRICT; [ADR 0005](../adr/0005-xml-backend.md) B1; LIB-SEC-005's
  earliest-construct-first order.

## Result

- **LIB-SEC-008.** A start tag that declares `xmlns` or `xmlns:xml` twice, which roxmltree
  0.21.2 accepts and XML 1.0 §3.1 (Unique Att Spec) forbids, is refused in both modes as
  `Malformed` (SACM-XML-001) at the repeat, with the existing `duplicate-attribute`
  (`xmlns`) or `duplicate-prefix` (`xmlns:xml`) template. The check runs among the
  post-parse checks, so an element over a limit is still `LimitExceeded`, roxmltree's own
  refusals win, and the earliest offending construct is reported. Every repeated form tried
  (quoting, white space, encodings, the wrapper, preserved content, text the scan skips)
  is refused; nothing conforming is newly refused, and the W3C suite's results are
  unchanged.
- **Tests.** Six `xml_wellformedness.rs` tests and one unit test (300,000 declarations
  refused in 0.74 s; a linear search fails its 5 s deadline). Two mutants are equivalent:
  the repeat check before the `xmlns:xmlns` check, and before `check_name`.

## Findings

| Blocking / non-blocking | Row | Finding | Resolution |
|---|---|---|---|
| Non-blocking | LIB-SEC-008 | The acceptance said 100,000 declarations; the test uses 300,000. | The verifier's text. |
| Non-blocking | LIB-SEC-008 | The `input.rs` module header and the `check_unrepaired` doc did not name the row; ADR 0005 B1 did not list the leniency. | The verifier's text. |
| Non-blocking | LIB-SEC-005 | The older mutant `input.rs:773:5` (`start_tag` → `Ok(Some(0))`) times out rather than being reported missed. | Triaged as killed in `docs/quality/mutants-triage.md`. |
