---
slice: 0021
name: m1-interchange-roots
state: in-progress
step: verification
rows: [SACM23-XMI-001, LIB-SEC-005, LIB-API-009, LIB-SEC-002]
mode: pr
---

# Slice 0021 — Interchange roots

## Scope

`sacm::xmi::read` returns its first content: an AssuranceCasePackage or ArgumentPackage
root, bare or inside `xmi:XMI`. Any other root is refused with SACM-XMI-001. Inputs
that roxmltree silently repairs are refused or reported first (ADR 0005 B1). Content
the reader cannot represent yet stays `Unsupported`, never dropped. Scope is set by
research and design below.

## Basis

formal/23-05-08 clause 2; XMI 2.5.1 (now pinned, LIB-TOOL-020); ADR 0005, ADR 0015;
the assurance-forge C++ `io/xmi_reader.cpp`.

## Design

ADR 0016 (accepted) covers the deviations from C++: D1 the wrapper is matched by
namespace, D2 a type does not override the name, D3 no partial document, D4 unread
attributes are refused, D5 only two root kinds are read for now, D6 other namespaces are
`Unsupported`, D7 the empty wrapper has a location.

```rust
impl ReadOutput { fn document(&self) -> &Document; fn diagnostics(&self) -> &[Diagnostic]; fn into_document(self) -> Document; }
ReadError::InvalidRoot { location: Option<SourceLocation>, element_name: String }  // SACM-XMI-001
ReadError::NoPackage { location: Option<SourceLocation> }                        // SACM-XMI-001
pub const Code::XMI_INVALID_ROOT: Code = "SACM-XMI-001"; // after C++ kXmiInvalidRoot
```

Display: `the root element "sacm:Claim" is not a SACM package at line 1, column 1`;
`the XMI element holds no SACM package at line L, column C`. Private `xmi/roots.rs`
builds the document through `insert_node`. The LIB-SEC-005 checks run in `input.rs`:
the declaration grammar in `decode`, and the rest after the parse.

## Log

- 2026-11-11 choose: SACM23-XMI-001 alone; it is the only ready XMI row.
- 2026-11-11 research: XMI-001 analyzed. The parser well-formedness gaps split out
  to new row LIB-SEC-005 (in this slice). LIB-API-009 items 1 and 3 reworded, and
  LIB-SEC-002 test inputs to change (both re-checked here). Follow-up rows
  SACM23-XMI-019, LIB-COMPAT-009, LIB-TOOL-021; notes on XMI-003/004 and the
  interface, binding, ART and TERM rows; specification defect D10. The empty wrapper is
  SACM-XMI-001, as in C++.
- 2026-11-11 design: architect brief; ADR 0016 proposed; awaiting approval.
- 2026-11-11 design: maintainer approved ADR 0016 and the empty-wrapper refusal.
- 2026-11-11 tests and implementation: 30 XMI-001 and 21 SEC-005 tests; 7 verified tests updated, none weakened; the gate passes.
- 2026-11-11 fix round 1 (before verification): MSRV let-chain (Copilot); 23 missed mutants: equivalents simplified away, one excluded with a triage row (diagnostics() is always empty), 8 tests kill the rest; every read in the reader tests runs under a 5 s deadline.
- 2026-11-11 verification round 1: FAIL on LIB-SEC-005. The post-parse scan ended a
  comment inside `<!--` (`<!-->…-->`), so a later surrogate was repaired silently.
  SACM23-XMI-001 is blocked by that dependency. LIB-API-009 and LIB-SEC-002 re-checks:
  PASS, provided the fix lands.
- 2026-11-11 fix round 2: constructs end after their opening delimiter; tests T1-T3;
  `check_text` doc reworded; `xmi:documentation` as the wrapper-child example; D10
  citation, LIB-API-009 wording, triage line. The maintainer confirmed the amended
  ADR 0016 D1/D3 (an XMI-namespace child of `xmi:XMI` is `Unsupported`, not
  SACM-XMI-001; a foreign `XMI` element is `Unsupported` before its children).
