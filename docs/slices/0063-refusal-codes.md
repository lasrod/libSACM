---
slice: 0063
name: refusal-codes
state: in-progress
step: verification
rows: [LIB-API-017]
mode: pr
base: main
---

# Slice 0063 — Every read and write refusal has a code, a template and arguments

## Scope

GitHub issue #82, first half:
- every `ReadError` and `WriteError` gets a catalog code (`code()` returns `Code`:
  Breaking, 0.2.0);
- each gets a stable message template id and named arguments, so an application can word
  it in any language;
- refusals get serde.

Every English message stays byte-identical. Diagnostics follow in slice 0064
(LIB-API-018).

## Basis

Library row; the standard is silent. The maintainer's decisions Q8 to Q11 of 2026-11-17.
The design brief is `target/slice-0063-design.md` (git-ignored).

## Design

ADR 0030 amendment 3, **proposed**. The design's five open API-detail questions are built on
its recommendations:
1. write codes SACM-XMI-001, SACM-XML-003 and SACM-XMI-016;
2. a public opaque `Details` field;
3. the bare code text as the default template;
4. four arguments keep English from elsewhere;
5. one `## Message templates` table in the catalog.

The merge waits for the maintainer's approval.

## Log

- 2026-11-18 choose: #82's refusal half (the designer split #82 into slices 0063 and 0064).
- 2026-11-18 analyzed: LIB-API-017 is added (analyzed) and LIB-API-018 (planned), and ADR
  0030 amendment 3 is written as proposed. Builder step 1 is running.
- 2026-11-18 tests: `sacm-builder` step 1.
  - It recorded `messages-0.1.0.txt` first: 241 messages from 53 fixtures. Its test passes,
    and must keep passing.
  - 27 `lib_api_017_` tests, plus 131 code() assertions gated old and new
    (`cfg(slice_0063_api)`).
  - The lead confirmed the red: under the cfg, only the missing API; without it, only the
    catalog, CLI and xtask checks. The snapshot is `red-0063.tar`, with `red-0063-golden/`.
  - Lead rulings:
    - template.rs names `ReadError::Unsupported` through an `Unread` carrier, and the
      serde tag comes from refusal.rs, so LIB-API-011's verified check is untouched;
    - the two CLI tests whose names say "without a code" are renamed in step 2.
- 2026-11-18 implementation: `sacm-builder` step 2.
  - 103 refusal templates, the renderer, `Argument`, `ArgumentValue` and `Details`, six
    new codes, serde for refusals, and the CLI codes. The messages golden is unchanged.
  - 24 hand mutations, all killed; one new test,
    `lib_api_017_a_code_is_checked_against_its_variant_before_the_template`.
  - Two CLI tests renamed, with their citations updated.
  - Verified rows reworded with the design's text: XMI-001, VAL-001, -006, -020,
    XMI-021, -008, LIB-COMPAT-007, LIB-API-009 (also its requirement), LIB-API-010,
    LIB-CLI-002 and LIB-API-006. Notes on LIB-API-011 and LIB-DIAG-002.
