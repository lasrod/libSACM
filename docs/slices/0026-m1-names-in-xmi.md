---
slice: 0026
name: m1-names-in-xmi
state: in-progress
step: verification
rows: [SACM23-XMI-011, SACM23-XMI-012, LIB-API-010, SACM23-XMI-001, SACM23-XMI-002, LIB-API-009]
mode: pr
base: main (e4c13cd)
---

# Slice 0026 — Names and exact strings in XMI

## Scope

The writer writes a ModelElement's `name` as one `name` child carrying the LangString's
`lang` and `content` attributes, with every string kept exactly: escaped, absent versus
empty preserved, and unrepresentable characters refused. The reader reads the `name` child
of a package root. LIB-API-010 stops refusing names. Parts that need machinery that does
not exist yet move to the rows that build it; research decides which. Candidates are:
strict save, preserved content for a second `name`, and read diagnostics.

## Basis

formal/23-05-08 §8.3 (LangString) and §8.6 (`name:LangString[1]`); XMI 2.5.1; ADR 0012,
ADR 0013, ADR 0017.

## Log

- 2026-11-12 choose: SACM23-XMI-011 (the first ready row) with SACM23-XMI-012. Strings can
  only be round-tripped through names, so the two belong together. LIB-API-010 is reworded.
  Base: `main` (e4c13cd).
- 2026-11-12 maintainer: compliance with the SACM standard comes before the C++ library,
  with quality and simplicity valued. CLAUDE.md rule 6 and the architect's definition are
  reworded. GSN is planned on top of the library later.
- 2026-11-12 research and lead decisions:
  - **Evidence:** every real file (three EMF third-party files, 28 written by C++) uses
    the attribute form `<name lang content/>`.
  - **Rows:** XMI-011 and XMI-012 are `analyzed`. LIB-API-010 is reworded: only a
    non-default `assertionDeclaration` is refused now.
  - **Moved out:** the second-name rule goes to the new XMI-021, the LangString `xmi:id`
    rule to the new XMI-022, and strict save to LIB-COMPAT-007. Each is `Unsupported`
    until then. `xml:lang` is not SACM `lang`, so it is refused.
  - **Kept minimal:** the existing `UnrepresentableCharacter` is reused, and only a
    no-namespace `name` is read.
  - **Re-checked:** XMI-001, XMI-002 and LIB-API-009 used `<name/>` as unread content.
    They switch to `<note/>` and are re-checked in this slice.
  - **ADR 0017 amendment 1** covers names.
- 2026-11-12 design: no architect step at first. This was wrong: the `attribute` field is new public API (see the round-1 entry).
- 2026-11-12 maintainer decisions, for later rows:
  - XMI-022 preserves a LangString `xmi:id` (no code), unlike C++.
  - XMI-021 reports a second `name` as SACM-MULT-001, whose catalog text widens when XMI-021 is built.
- 2026-11-12 implementation: names are written as the first child and read on package roots; everything else on a name stays `Unsupported`. `UnrepresentableCharacter` gains `attribute`. The local gate passes.
- 2026-11-12 verification round 1: LIB-API-010, XMI-001 and XMI-002 PASS; XMI-011, XMI-012 and LIB-API-009 FAIL on test gaps.
  - `lang` exactness is not pinned (N1-N3).
  - The type scope on `name` is not pinned (M14), and neither is `xmlns=""` on `name` (M4).
  - The reader differences from C++ have no ADR lines.
  - LIB-API-009 example and test disagree.
- 2026-11-12 docs review: the crate root feature list is stale (blocking). The `attribute: &str` field will become ambiguous, so it is replaced by `WriteAttribute`. ADR 0017 amendment 1 is now proposed, for the maintainer.
