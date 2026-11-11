---
slice: 0023
name: m1-namespace-prefixes
state: in-progress
step: verification
rows: [SACM23-XMI-002]
mode: pr
base: main
---

# Slice 0023 — Namespace-prefix independence

## Scope

Import matches elements and attributes by namespace URI and local name, never by prefix.
So the same document with any prefixes, including a default namespace, loads identically.
This includes QName-valued attributes such as `xsi:type`, if the reader uses them. The slice
leaves out export namespace pinning (SACM23-XMI-003) and `xmi:id` identity
(SACM23-XMI-004).

## Basis

Namespaces in XML 1.0; XMI 2.5.1. The research fills in the exact clauses.

## Log

- 2026-11-12 choose: SACM23-XMI-002, the first ready row. Base: `main` (d837a77).
- 2026-11-12 research: the reader already matches by expanded name and resolves type values.
  - Two gaps need a code change: white space around a type value is refused, though XSD
    collapses it; a `xmlns=""` root says `namespace ""`, not "no namespace".
  - Lead decisions:
    - the white-space collapse stays in this row;
    - ADR 0016 D2 gets a clarifying note naming C++ `xmi_reader.cpp:597-658` (no change in
      policy);
    - XMI rule 2g on `xsi:type` goes to SACM23-XMI-006's notes.
- 2026-11-12 design: no public API change, so no architect step.
- 2026-11-12 tests: 18 tests in `xmi_namespace_prefixes.rs`. The lead confirmed the red:
  only the white-space and `xmlns=""` tests fail, for the stated reasons.
- 2026-11-12 implementation: the type value is trimmed of XML white space before it is resolved; `Some("")` counts as no namespace. The local gate passes.
