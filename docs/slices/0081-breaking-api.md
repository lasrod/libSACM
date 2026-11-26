---
slice: 0081
name: breaking-api
state: done
step: close
rows: [LIB-API-020, LIB-DIAG-004, LIB-DIAG-005, SACM23-XMI-032]
mode: pr
base: main (028c941)
---

# Slice 0081 — The breaking changes for 0.5.0

## Scope

The three breaking API changes the maintainer decided on lasrod/libSACM#113, made first so
Assurance Forge can adapt before 0.5.0:
- reading defaults to strict (LIB-API-020);
- English prose arguments are `ArgumentValue::Prose` (LIB-DIAG-004);
- building a `Diagnostic` is crate-internal outside `test-support` (LIB-DIAG-005).

It also adds the EMF `version` templates (SACM23-XMI-032), which touch the same template
table. It leaves out the rest of #113 and the docs rewrite (LIB-DOC-009).

## Basis

API rows: the maintainer's decisions, 2026-11-26, on lasrod/libSACM#113 (B1, B2, D6). ADR
0022 is amended. SACM23-XMI-032: XMI 2.5.1 §7.5.3 (`xmi:version`), SACM-XMI-011.

## Design

- `ReadOptions::default()` reads in `Mode::Strict` (ADR 0022 amendment 3; D-STRICT).
- `ArgumentValue::Prose(String)` and `ArgumentValue::as_prose()`. Eight template arguments
  become prose; JSON and `Display` are unchanged (ADR 0030 amendment 6).
- The `Diagnostic` constructors stay behind `test-support`, now locked by an xtask check;
  serde refuses an empty `requirement_id` (ADR 0011 amendment 3).
- Templates `SACM-XMI-011/emf-version-on-root` and `/emf-version-on-xmi-element`.

## Log

- 2026-11-26 design: branch from main 028c941 (after #114); sacm-designer started.
- 2026-11-26 design: brief received. The lead took the designer's recommendations: the
  `encoding` labels are prose, and `as_prose()` is added. Rows analyzed; ADR 0022 am. 3,
  ADR 0030 am. 6, ADR 0011 am. 3, ADR 0018 note and D-STRICT written.
- 2026-11-26 tests: sacm-builder step 1 started.
- 2026-11-26 tests: red confirmed by the lead (snapshot red-0081.tar). The builder's
  deviations are accepted: the serde prose test is in prose_arguments.rs; the
  LIB-COMPAT-001 mode test also drops `assert_ne!(with_mode(Strict), default())`, which
  LIB-API-020 contradicts. Three locks pass at step 1 by design (the two xtask check
  tests exercise test code; the XMI 2.5.1 `version` and the strict refusal are already
  true).
- 2026-11-26 implementation: sacm-builder step 2 started. Approved change to a step-1
  test: message_templates.rs asserts `ArgumentValue::Prose` exactly in place of its
  step-1 projection (a tightening).
- 2026-11-26 implementation: two step-1 tests were wrong, and the lead approved fixing
  them. xmi_version.rs:1151 asserted no `version=` at all, but the XML declaration has
  one; it now asserts exactly one. serde_diagnostics.rs:714 used `"severity":1`, which
  serde_json reports as a syntax error, not a data error; it now uses `"message":1`.
  Departures accepted: lib.rs:90-91 had the same false "the default" wording and was
  fixed; `Findings::xmi_version` picks its template itself (a private change).
- 2026-11-26 verification: draft PR #115 at 1f9094e. sacm-verifier: LIB-API-020,
  LIB-DIAG-005 and SACM23-XMI-032 PASS; LIB-DIAG-004 FAIL, because two prose sites survive
  a mutation back to Text (M13: `InvalidChar2` in input.rs; M11: unheld-outside in
  roots.rs). Fix round: two controls and an `unheld_outside` extraction (builder),
  wording with the verifier's text (builder, lead), CHANGELOG Changed entry and the
  matrix source page (lead).
- 2026-11-26 verification: PASS on a743d8b for all four rows ([record](../verification/0081-breaking-api.md)). Last wording fix (prose_arguments.rs module doc) with the verifier's text.
- 2026-11-26 close: done.
