---
slice: 0084
name: api-review
state: done
step: close
rows: [LIB-API-005, LIB-API-022, LIB-API-023, LIB-API-024, LIB-API-025]
mode: pr
---

# Slice 0084 — Public API review before 1.0

## Scope

The review of the whole public API against the Rust API Guidelines (LIB-API-005), and its
fixes: names take plain text, uniform traits and conversions, collection and result helpers,
and one spelling for the href form. The rename of the href constructors is the one break.
`EditError` templates are deferred (LIB-API-026, candidate). The slice also kills three
parser mutants the catalog generator kept from slice 0083 (docs/follow-ups.md).

Base: `main` (59aecac).

## Basis

Library rows: the Rust API Guidelines checklist; acceptance by the lead from the design brief
(session scratchpad `design-0084.md`). ADR 0045.

## Log

- 2026-11-27 design: review done during slice 0083. The maintainer took every
  recommendation: rename the href constructors; keep `check`/`preview` on `&Operation`; one
  `ElementId`; `Difference` variants stay constructible. ADR 0045 accepted. Rows `analyzed`.
- 2026-11-27 tests: 18 tests in api_review.rs; red confirmed (missing API only). Two
  LIB-API-005 tests pass today by design: they lock decisions D1 and D2, which change no code.
  The public-type list gained the 54 operation structs and three delete policies (test-support
  types left out, D7); 55 constructors take a name, not 57. Three catalog parser cases kill
  slice 0083's survivors; a lone `/` must end its line (`[], /`), since `[/],` does not kill
  350:20. That follow-up is closed. Rows `tests-failing`.
- 2026-11-27 implementation: done; 23 hand mutants killed; no step-1 test changed. The href
  rename moved callers in three test files (names in assertion messages too) and the docs.
  ADR 0045 D3 reworded to match the variants ("`{id, left, right}` for most"). Rows
  `implemented`.
- 2026-11-27 verification: PASS on all five rows, eight non-blocking findings. Fix round:
  `Href` compares with `str` now (adding it after 1.0 would break inference), a padding
  test, the verifier's wording in `Mutation::is_empty` and `LangString`, the CHANGELOG
  Breaking entry for name inference, ADR 0045's checked list and deferred items, an ADR 0040
  rename note; one follow-up.
- 2026-11-27 verification: re-verify PASS on all five rows (fb3c567); the last wording
  (CHANGELOG `Href` entry, LIB-API-023 acceptance, ADR 0045 wrap) applied by the lead.
  Record `docs/verification/0084-api-review.md`. Rows `verified`.
