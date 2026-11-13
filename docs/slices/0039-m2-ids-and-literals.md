---
slice: 0039
name: m2-ids-and-literals
state: in-progress
step: verification
rows: [SACM23-VAL-006, SACM23-XMI-020, SACM23-XMI-021, SACM23-XMI-022, SACM23-XMI-025]
mode: pr
base: main
---

# Slice 0039 — Ids and literals the reader refuses today

## Scope

Five reader rows, each narrowing an interim `Unsupported`:
- **SACM23-VAL-006:** an `assertionDeclaration` that is not a literal is a coded refusal
  (SACM-ENUM-001) in both modes.
- **SACM23-XMI-021:** a second `name` is `ReadError::RepeatedValue` (SACM-MULT-001).
- **SACM23-XMI-025:** an `xmi:id` with white space around an NCName is read trimmed, with an
  Info.
- **SACM23-XMI-020:** an element with no `xmi:id` is read with a generated id and an Info
  SACM-XMI-004, and the id is written on save.
- **SACM23-XMI-022:** an `xmi:id` on a LangString is kept and written back.

## Basis

XMI 2.5.1 §7.6.1 (`xmi:id`, `use="optional"`, `xsd:ID`), rule 2h; SACM 2.3
`AssertionDeclaration`; ADR 0015 amendment 2; ADR 0021; ADR 0022. The maintainer decided
these rows' policy on 2026-11-12 and 2026-11-14 (see the row notes).

## Design

[ADR 0024](../adr/0024-ids-read-from-xmi.md) and ADR 0021 amendment 2, both accepted by
the maintainer on 2026-11-14. Everything is additive:

```rust
impl Code {
    pub const ENUM_INVALID_LITERAL: Code; // SACM-ENUM-001
    pub const XMI_MISSING_ID: Code;       // SACM-XMI-004, Info
    pub const ID_TRIMMED: Code;           // SACM-ID-004, Info (new)
}
#[non_exhaustive]
ReadError::InvalidLiteral { element: ElementId, property: String, value: String,
                            location: Option<SourceLocation> }
ReadError::RepeatedValue { property: "name", .. }          // second name
impl SacmElement<'_> { pub fn name_id(self) -> Option<&ElementId>; }
Difference::NameId { id: ElementId, left: Option<ElementId>, right: Option<ElementId> }
```

- **Generated ids:** `<snake_case kind>_<n>`. A pre-pass reserves every `xmi:id` in the input,
  and the counters are committed afterwards.
- **Re-checked rows:** SACM23-XMI-004, SACM23-XMI-012 and SACM23-VAL-020 are reworded, and
  SACM23-VAL-001 and LIB-RT-001 are touched.

## Log

- 2026-11-14 choose: the rows the M2 decision batch unblocked, all in the reader.
