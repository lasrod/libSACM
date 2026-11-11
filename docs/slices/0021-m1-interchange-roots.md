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
- 2026-11-11 verification round 2: FAIL on LIB-SEC-005, so the slice stopped for the
  maintainer. The comment fix holds (a 385k-input sweep found no desync). But the PI
  check ends the target at white space or `?`, not where the XML Name ends, so
  `<?xml>?>` and `<?Xml/?>` read Ok (HIGH). A PI with no white space after its target
  (`<?pi>x?>`, XML [16]) is accepted (MEDIUM). LIB-API-009 and LIB-SEC-002 re-checks:
  PASS. SACM23-XMI-001: blocked only by its LIB-SEC-005 dependency.
- 2026-11-11 the maintainer authorised fix round 3: the PI target ends at the XML Name, `<?pi>x?>` is refused (XML [16]), and a tree-oracle sweep becomes a permanent test.
- 2026-11-11 fix round 3: the PI target ends at the XML Name; [16] is refused; tests T4-T6; the T7 tree-oracle sweep (12,000 generated inputs, 0 disagreements, about 2 s).
- 2026-11-11 round 3 CI: one missed mutant (`+` to `*` in the [16] location, equal for 2-character targets); killed by 1-, 3- and multi-byte-character targets.
- 2026-11-11 verification round 3: FAIL on LIB-SEC-005, so the slice stopped for the
  maintainer. The round-2 fixes hold (sweeps of about 9M accepted inputs found no
  disagreement). A new class: roxmltree reads an empty prefix as none, so
  `<:ArgumentPackage …></ArgumentPackage>` reads Ok (HIGH). A PI target containing `:`
  is accepted, against Namespaces 1.0 §7 (MEDIUM; item 4 now contradicts it). The
  [4]/[4a] range edges are unpinned (MEDIUM). The triage line is now 240:9.
- 2026-11-11 the maintainer chose: fix the three round-3 findings now, narrow LIB-SEC-005
  to a closed list, and find the rest systematically in the new M1 row LIB-SEC-006 (W3C
  XML Conformance Test Suite) as the next slice; colon PI targets are refused
  (Namespaces 1.0 §7). ADR 0005 B1 records that other leniencies may remain until then.
- 2026-11-11 fix round 4: empty-prefix names and colon PI targets refused; tests T8-T10; T7 extended (12,000 inputs, 0 disagreements); the input.rs unit test split for clippy.
- 2026-11-11 verification round 4: FAIL on gaps in docs and tests only; the reader has no
  defect (every round-1 to round-3 defect re-inserted is killed; sweeps of 16M accepted
  inputs found no disagreement). MEDIUM: the public docs read as if the refused list
  were complete. LOW: no row-named test of the colon-before-[16] order (H13); names
  quoted wrongly when white space comes before `>` or `=` (H11, X1); triage line 242;
  the roxmltree citation for empty prefixes; ADR 0005 B1 calls round 1 a parser
  leniency. Logged for LIB-SEC-006: roxmltree accepts `xmlns:xmlns="urn:x"`
  (Namespaces 1.0 §3).
- 2026-11-11 fix round 5 (the maintainer authorised it): the public docs say the refused list is not complete until LIB-SEC-006; cases that kill H13, H11 and X1; citations, ADR 0005 B1 wording and the LIB-SEC-006 refusal definition fixed; triage line 245.
