---
slice: 0071
name: m5-close
state: done
step: close
rows: []
mode: pr
base: main (d3037f5)
---

# Slice 0071 — M5 close

## Scope

This slice closes milestone M5 (Modularity: interfaces and bindings), which is 15/15 verified.
- It applies the matrix-steward audit's findings.
- It clears `docs/follow-ups.md`.
- It adds the MSRV check to the documented gate, as the maintainer approved on 2026-11-19.

There are no behaviour changes.

## Basis

Quality slice; the lead's acceptance.

## Log

- 2026-11-19: the audit is applied.
  - The lead: seven M5 acceptance items, the `depends_on` lists, wording, the new candidate row LIB-EDIT-020, a CHANGELOG line, broken links, the MSRV step in CLAUDE.md and CONTRIBUTING.md, and the cleared follow-ups.
  - The builder: `lib.rs`, both READMEs, two test values, and moving the cross-document fixture out of `invalid/` with the messages-golden caveat.
- Record: `docs/verification/0071-m5-close.md`.
