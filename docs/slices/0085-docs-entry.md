---
slice: 0085
name: docs-entry
state: done
step: close
rows: [LIB-DOC-009, LIB-DOC-006]
mode: pr
---

# Slice 0085 — The style guide and the entry documents

## Scope

The docs style guide (docs/style.md, summarised in the `rust-library-craft` skill) and the
rewrite of the documents a new user reads first: the crate root, README.md, the two crate
READMEs and docs/architecture.md, which claim only verified capability (LIB-DOC-006). Module
docs, item docs and the docs/sacm references follow in LIB-DOC-011 to LIB-DOC-013, split off
from LIB-DOC-009 in this slice.

Base: `main` (42909f0).

## Basis

Docs rows: acceptance by the lead. The maintainer's style rules (2026-11-26) and the crate
root draft the maintainer chose in the blind pilot (Fable 5.2, scratchpad
`docs-pilot/variant-1.md`). Fable 5.2 writes the docs.

## Log

- 2026-11-27 design: LIB-DOC-009 narrowed to the guide and the entry documents; LIB-DOC-011
  to -013 added; skill rules written by the lead. Rows `analyzed`.
- 2026-11-27 tests: xtask/tests/docs_style.rs, 9 tests. The lead's rules: no prose paragraph
  and no list item (without its nested items) over four display lines, a line counting
  ceil(chars / 100); tables, code, headings, quotes, HTML, badges and link definitions are
  exempt; the crate root links the matrix by URL. Red: 34 places plus the missing style
  guide, the 85-line "What works today" item among them. Rows `tests-failing`.
- 2026-11-27 implementation: Fable 5.2 wrote docs/style.md (56 lines) and rewrote the crate
  root (548 to 217 lines), the READMEs and docs/architecture.md; stale claims corrected (CLI
  version and exit codes, the serde version, the planned list). Four crate-root doctests
  removed: each operation has a doctest in its item docs; the SACM concept prose goes to the
  primer (LIB-DOC-005). Rows `implemented`.
- 2026-11-27 verification: FAIL on both rows, two false claims (a tolerant read "keeps what
  strict refuses"; "nothing dropped", though comments and processing instructions are not
  kept). Fix round: the verifier's texts, the stale strict-write sentence in diagnostic.rs,
  five test controls; the matrix banner moved back; two follow-ups.
- 2026-11-27 verification: re-verify PASS on both rows (1bf2889); the verifier's last
  diagnostic.rs wording applied after the PASS. Record `docs/verification/0085-docs-entry.md`.
  Rows `verified`.
