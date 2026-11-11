---
slice: 0019
name: m1-xml-input-safety
state: done
step: close
rows: [LIB-SEC-001, LIB-SEC-002, LIB-API-009]
mode: pr
---

# Slice 0019 — XML input safety

## Scope

- **LIB-SEC-001:** a document with a DOCTYPE or an entity declaration is refused
  (SACM-SEC-001) before anything could be resolved, in every mode.
- **LIB-SEC-002:** malformed XML, truncated input and invalid UTF-8 are refused with a
  diagnostic and a location, and no input causes a panic.
- **LIB-API-009:** the reader refuses what it cannot represent (`Unsupported`), and
  never returns a partial document.

This is the crate's first XML input, so the slice decides the XML backend (ADR 0005)
and the shape of the load entry point. Out of scope: reading SACM content into a
`Document` (the XMI rows, starting with SACM23-XMI-001), load modes (LIB-COMPAT-001) and
saving.

## Basis

Quality rows: acceptance by the lead. OWASP XXE prevention; docs/sacm/diagnostics.md
(SACM-SEC-001, SACM-XML-001, SACM-XMI-001); ADR 0005, ADR 0011; the assurance-forge
C++ `io/xmi_reader.cpp`.

## Design

ADR 0005 (accepted: roxmltree 0.21.2, private) and ADR 0015 (accepted: the read entry
point).
- **`sacm::xmi`:** `read(input: &[u8], options: &ReadOptions) -> Result<ReadOutput, ReadError>`.
  `ReadOptions {}` and `ReadOutput {}` are both `#[non_exhaustive]` and empty for now.
- **`ReadError`** (`#[non_exhaustive]`):
  - `Doctype { location }`: SACM-SEC-001.
  - `Malformed { location, reason }`: SACM-XML-001.
  - `Unsupported { location, reason }`: no code.
  - `code()` and `location()` accessors.
- **`Code::XML_MALFORMED`** (SACM-XML-001) and **`Code::XML_DOCTYPE_REJECTED`** (SACM-SEC-001).
- **Pipeline:**
  1. A UTF-16, UTF-32 or UCS-4 BOM, or the BOM-less XML 1.0 Appendix F first bytes,
     gives Unsupported.
  2. A declared encoding other than UTF-8 or ASCII gives Unsupported. It is read from the
     valid UTF-8 prefix, before the UTF-8 check.
  3. Invalid UTF-8 gives Malformed.
  4. roxmltree parses, with DTDs refused: `DtdDetected` gives Doctype, and other errors
     give Malformed.
  5. Well-formed input gives Unsupported until SACM23-XMI-001.
- **Private module:** the input layer is in `xmi/input.rs`.

## Log

- 2026-11-10 brief written. These are library security rows, so there is no SACM
  research; the architect is next.
- 2026-11-11 design:
  - The architect proposed roxmltree 0.21.2. quick-xml 0.42 needs Rust 1.86.
  - The maintainer decided:
    - roxmltree, accepting ADR 0005;
    - DOCTYPE detection in the parser, not the C++ raw scan, because the scan refuses
      valid files that mention it in comments or CDATA (ADR 0005 B3);
    - UTF-16 refused for now, with new row SACM23-XMI-018 to read it, because XML 1.0
      requires it;
    - the read entry point, ADR 0015: bytes in, no file function, Err when no faithful
      Document exists.
  - The lead reworded LIB-SEC-001 and LIB-SEC-002 and wrote their acceptance. "Wrong
    root" moves to SACM23-XMI-001. New row LIB-API-009 (refuse what cannot be
    represented) joins the slice. LIB-COMPAT-001 gains the both-modes rule.
  - Q-XML-BACKEND is closed. All three rows are `analyzed`. The test engineer is next.
- 2026-11-11 tests: 31 in `crates/sacm/tests/xmi_read.rs` (`lib_sec_001_*`, `lib_sec_002_*`, `lib_api_009_*`).
  - The test engineer ran them against a throwaway implementation: all passed. Two deliberate bugs (a raw DOCTYPE scan, byte columns) failed 5.
  - The red was confirmed by the lead: E0432 and E0599, because `sacm::xmi` and the codes are missing.
  - SEC-001 item 3 is reworded: an attribute value can only hold `&lt;!DOCTYPE`.
  - All three rows are `tests-failing`. The implementer is next.
- 2026-11-11 implementation:
  - `sacm::xmi` (`read`, `ReadOptions`, `ReadOutput`, `ReadError`) and the private `xmi/input.rs`, with roxmltree 0.21.2. `Code::XML_MALFORMED` and `Code::XML_DOCTYPE_REJECTED`.
  - Docs: `lib.rs`, both READMEs, `architecture.md` and `diagnostics.md`.
  - The implementer read the declared encoding before the UTF-8 check, so a real Latin-1 file is `Unsupported`, not `Malformed` (ADR 0005 B2). The lead accepted this and added it to LIB-API-009 item 3; the test engineer added 2 integration tests, never seen red. The verifier is asked to mutate the order.
  - The full gate passes (698 tests; MSRV 1.85 checked). All three rows are `implemented`. Verification is next.
- 2026-11-11 CI on `e45b5f2`:
  - One missed mutant, `allow_dtd: false` deleted. It is equivalent, because false is roxmltree's default, so the code now uses `ParsingOptions::default()`.
  - Copilot raised two valid points:
    - BOM-less UTF-16 or UCS-4 was `Malformed`; it is now `Unsupported`, recognised by the XML 1.0 Appendix F first bytes;
    - a lone CR was not a line end; locations now follow XML 1.0 §2.11, for every refusal.
  - The acceptance is updated: LIB-API-009 item 2, and a new LIB-SEC-002 item. The test engineer added 5 red tests; the implementer fixed `input.rs`.
  - The gate passes. The rows are `implemented` again.
- 2026-11-11 verification round 1 at `39105a4`: **FAIL** on LIB-SEC-001 and LIB-API-009; LIB-SEC-002 PASS. LIB-DIAG-001 and LIB-API-004 still hold.
  - Mutants: the PR diff caught 107 of 107, the full run caught 675 of 675.
  - The gaps were obligations that only untraced unit tests proved, or none: case-insensitive `utf-8` (M15), the parser limits (M25), the encoding refusal locations (M11, M17, M18, M22), and a DOCTYPE after a BOM (M02).
  - The lead's fix:
    - tighter acceptance on LIB-SEC-001 items 1 and 6, LIB-SEC-002, and LIB-API-009 items 2 and 3;
    - a leading BOM now takes no column;
    - the parser limits moved to LIB-SEC-003;
    - the stale LIB-DIAG-001 item 4 reworded;
    - ADR 0005 B1 corrected: roxmltree's leniencies become SACM23-XMI-001 obligations.
  - The docs review is applied in the same round: the status lines say plainly that `read` refuses every input, `Doctype` displays `the input has a DOCTYPE, which is refused`, the `# Errors` order is stated, doctested examples are added, the catalog rows reworded, and ADR 0005, ADR 0015 and the CHANGELOG updated.
  - The test engineer added 5 tests and changed 3, all traced. The implementer changed `input.rs`, `xmi.rs`, `diagnostic.rs` and the docs. Verification round 2 is next.
- 2026-11-11 verification round 2 at `b8f2f95` ([record](../verification/0019-m1-xml-input-safety.md)): all three rows PASS, and all are now `verified`. LIB-DIAG-001 and LIB-API-004 still hold. Mutants: the PR diff caught 107 of 107, the full run caught 675 of 675. Every hand mutation that an acceptance item depends on is killed. The slice is done.
