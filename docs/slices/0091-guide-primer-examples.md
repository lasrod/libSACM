---
slice: 0091
name: guide-primer-examples
state: in-progress
step: tests
rows: [LIB-DOC-003, LIB-DOC-005, LIB-DOC-004]
mode: pr
---

# Slice 0091 — The user guide, the SACM primer and runnable examples

## Scope

The last docs rows of M7: a user guide with tested code (LIB-DOC-003), a SACM primer for
readers who have not read the specification (LIB-DOC-005), and runnable example programs
(LIB-DOC-004). Three Fable 5.2 writers on disjoint files.

## Basis

Docs rows: acceptance by the lead; docs/style.md (LIB-DOC-009); SACM 2.3 formal/23-05-08 for
the primer. The lead's layout: the guide and primer are Markdown inside the crate
(`crates/sacm/GUIDE.md`, `PRIMER.md`), so they ship with it; their code blocks run as doctests
the way the README's do, and the crate root links them.

## Log

- 2026-11-28 design: rows `analyzed`.
- 2026-11-28 tests: nine tests (docs_style.rs, examples.rs), eight red for the missing guide,
  primer, examples, doctest hooks and links; one control test. The crate root links both by
  URL (docs.rs). Lead decisions: LIB-QA-005's package-contents test allows `GUIDE.md` and
  `PRIMER.md` in the `sacm` package (the layout ships them); the nested example run is skipped
  in the mutants nextest profile. Rows `tests-failing`.
- 2026-11-28 implementation: three Fable writers. W1 the guide (eight doctested blocks), the
  crate root links, the doctest hooks, `include`, the crate README, docs/style.md and the
  LIB-QA-005 change (`GUIDE.md` and `PRIMER.md` allowed in `sacm` only, with a `sacm-cli`
  case); W2 the primer; W3 the four examples. The write guard refused the crate-root Markdown;
  the maintainer approved adding `crates/sacm/GUIDE.md` and `PRIMER.md` to the builder scope.
  The lead added the nextest `default-filter`. Full gate green. Rows `implemented`.

- 2026-11-28 lead decision: LIB-QA-012's reviewed-key check accepts `default-filter` in
  `[profile.mutants]` only with the exact value that skips the example run; any other filter
  is reported (it could hide tests from mutants, ADR 0009).
- 2026-11-28 verification: LIB-DOC-004 and LIB-DOC-005 PASS; LIB-DOC-003 FAIL on two blocking
  findings (the guide said `validate` reports an empty name; its language example gave a
  Claim two descriptions). Fix round: both fixed and twelve wording items applied with the
  verifier's text.
