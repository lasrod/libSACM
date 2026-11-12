---
slice: 0024
name: m1-utf16-input
state: in-progress
step: verification
rows: [SACM23-XMI-018, LIB-API-009, LIB-SEC-006]
mode: pr
base: slice/0023-m1-namespace-prefixes (PR #33)
---

# Slice 0024 — UTF-16 input

## Scope

`xmi::read` decodes UTF-16 input that begins with a byte order mark, little- or
big-endian, using std only. It then reads the document exactly as the same document in
UTF-8, with locations counted in characters. UTF-16 without a byte order mark, UTF-32,
UCS-4 and other declared encodings stay refused (LIB-API-009), even when the declaration
names `UTF-16LE` or `UTF-16BE`. LIB-API-009 and LIB-SEC-006 are reworded to match. This
slice does not touch export.

## Basis

XML 1.0 (Fifth Edition) §4.3.3, and Appendix F if needed; ADR 0005 B2; the maintainer's
decision of 2026-11-11. The research fills in the quotations.

## Log

- 2026-11-12 choose: SACM23-XMI-018.
  - SACM23-XMI-003 comes first in the list, but it needs an XMI writer and an ADR on
    `xmi:version`. SACM23-XMI-004 needs an ADR on a missing `xmi:id`. Both wait for a
    maintainer decision.
  - Base: `slice/0023-m1-namespace-prefixes` (PR #33).
- 2026-11-12 research: decoding alone would accept `hst-lhs-008`, a UTF-16 byte order mark
  with `utf-8`. So the declared encoding must be matched against the encoding actually used.
  New W3C counts: Doctype 719, encoding 1, Malformed 297, and a guard count of 73.
- 2026-11-12 maintainer decisions:
  - **Strict UTF-16 rules** (ADR 0005 B2 amendment 1, accepted). A byte order mark with
    `UTF-16LE`/`BE` is `Malformed`, and so is `UTF-16` without a mark. Without a mark,
    only a declared `UTF-16LE`/`BE` that matches the byte order is read.
  - A UTF-8 byte order mark with a non-UTF-8 name stays `Unsupported`.
  - Also decided, for later rows: XMI-003 keeps `xmi:version="2.0"`; XMI-004 accepts a
    missing `xmi:id` and reports it. Both decisions are in the row notes.
- 2026-11-12 matrix: XMI-018 is `analyzed`. LIB-API-009 and LIB-SEC-006 are reworded but
  stay `verified`, and are re-checked in this slice (the slice 0021 precedent).
- 2026-11-12 design: no public API change.
- 2026-11-12 maintainer, revised: the strict rules went too deep into XML conformance for
  this library. Replaced by **minimal UTF-16**:
  - **Read:** a byte order mark, decoded with std. The declaration may be absent or name
    a UTF-16 name.
  - **`Malformed`:** any other declared name, an odd byte count, or an unpaired
    surrogate.
  - **`Unsupported`, as before:** everything else, including UTF-16 without a mark.
  - ADR 0005 B2 amendment 1 and the three rows were rewritten. Expected W3C counts:
    719/2/296, guard 73.
- 2026-11-12 tests: 12 tests in `xmi_utf16.rs` and 1 in the W3C suite test. The LIB-API-009
  and LIB-SEC-006 tests are re-pinned (one rename). The real suite counts match 719/2/296
  and 73. The lead confirmed the red: 9 + 3 fail, and the cases that stay `Unsupported`
  pass.
- 2026-11-12 implementation: UTF-16 with a byte order mark is decoded with std (`Cow<str>`), and the declared name is checked against the mark. Acceptance item 5 was corrected by the lead: a no-root refusal sits where UTF-8 puts it, at the end of the input. The local gate passes.
- 2026-11-12 verification round 1: LIB-SEC-006 PASS; XMI-018 and LIB-API-009 FAIL on test gaps (3 hand mutations survive): the order of the declaration and decode checks (H1), and the exact Display of two reasons (H15-H17). The fix adds asserts only.
