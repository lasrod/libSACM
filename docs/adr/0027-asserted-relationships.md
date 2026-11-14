# 0027. Asserted relationships and multi-valued references

- Status: accepted (the maintainer, 2026-11-14; a repeated id in a multi-valued end is a Warning, SACM23-VAL-022)
- Date: 2026-11-14
- Matrix rows: SACM23-ARG-004, SACM23-ARG-005, SACM23-ARG-007, SACM23-ARG-008,
  SACM23-ARG-010, SACM23-VAL-005. It extends ADR 0019, ADR 0020 and ADR 0026, and D-ORDER.

## Context

SACM 2.3 §11 relates argument assets through asserted relationships:
- AssertedRelationship (§11.13) has `source:ArgumentAsset[1..*]`, `target:ArgumentAsset[1]`,
  `reasoning:ArgumentReasoning[0..1]` and `isCounter:Boolean[1] = false`.
- Every Assertion has `metaClaim:Claim[0..*]` (§11.10).
- ArgumentReasoning has `structure:ArgumentPackage[0..1]` (§11.12).

`source` and `metaClaim` are the first multi-valued reference ends. §11.14
(AssertedInference) and §11.16 (AssertedContext) have no Constraints section.

## Decision

- **New ends and flag.**
  - `metaClaim` (Assertion, many), `reasoning`, `source` (many) and `target`
    (AssertedRelationship), and `structure` (ArgumentReasoning) join ADR 0026's
    reference-end table.
  - `isCounter` joins the flag table, which gains owners: it is read only on asserted
    relationships, and elsewhere it is `Unsupported`.
  - Multi-valued ends hold every token of the attribute, and each reference element
    appends one id. An attribute and elements for the same end is `RepeatedValue` (XMI
    2.5.1 rule 2h).
  - Single-valued ends keep ADR 0026's rules.
- **Order and repeats.** A multi-valued end is held in document order with its repeats,
  written as held and compared in order. D-ORDER is extended to reference ends, so equal
  documents write equal bytes.
  - A repeated id (`source="a a"`) is never refused or merged. The file is schema-valid
    xsd:IDREFS (D-STRICT, D-PRESERVE).
  - Its finding is a later row, SACM23-VAL-022, at the severity the maintainer chooses.
- **Accessors.** `meta_claims()` and `sources()` return `&[ElementId]`. `target()`,
  `reasoning()` and `structure()` return `Option<&ElementId>`. `is_counter()` returns `bool`.
  A kind without the end gives empty, `None` or false.
- **`assertionDeclaration`** is read and written on every Assertion kind (SACM23-XMI-017's
  inheritance rule).
- **Write order.** Superclass first, then ptc order within a class: `… abstractForm
  assertionDeclaration metaClaim isCounter reasoning source target`, and `structure` on
  ArgumentReasoning.
- **Readable kinds.** ArgumentReasoning, AssertedInference and AssertedContext in
  `argumentElement`.
- **Create operations.**
  - `CreateArgumentReasoning`, `CreateAssertedInference` and `CreateAssertedContext` take
    the required sources and target, and an optional `reasoning`, `isCounter` and `structure`.
  - Checks, after ADR 0019's four:
    - at least one source: `EditError::TooFewReferences` (SACM-MULT-001);
    - no reference id holds XML white space: `EditError::InvalidReference` (SACM-ID-002),
      because such an id would be written back as two ids (found by Copilot on PR #54);
    - every reference names an element: `EditError::ReferenceNotFound` (SACM-CMD-001),
      because an edit never creates a dangling reference (D-VAL-LAYER).
  - Target types are checked with SACM23-VAL-004 and LIB-EDIT-009 (the next slice).
    Repeated sources are not refused, because they are a content finding.
  - Setting `metaClaim` and the other ends after creation is LIB-EDIT-009.
- **Missing ends.** A relationship read with no source or no target is kept (XMI ignores
  multiplicities, §7.8.2). `validate` reports each missing end as SACM-MULT-001, an Error
  (SACM23-VAL-005, clause 11.13), so a strict save refuses it.
- **Comparison.** `Difference::{IsCounter, MetaClaims, Reasoning, Sources, Target, Structure}`,
  named after their accessors, after `Gid` in table order.
- **§11.14 and §11.16** class descriptions are not constraints, because they have no
  Constraints section. Kind-specific end typing is SACM23-VAL-016, for §11.15, §11.17 and
  §11.18.

**Differences from C++, one line each:**
- C++ holds `targets` as a vector and reports a second one in validate (`validate.cpp:552`);
  here `target` is single-valued (§11.13 `[1]`), and two ids are `RepeatedValue`.
- C++ has one `CreateAssertedRelationship { kind, … }` (`operations.h:98`); here there is
  one struct per kind (ADR 0019).
- C++ create refuses wrongly typed ends (`commands.cpp:486-495`); here only existence is
  checked until SACM23-VAL-004.
- C++ compares reference ends as sorted multisets; here they are compared in document order.
- C++ concatenates an attribute and elements for one end (`xmi_reader.cpp:1964-1968`); here
  that is `RepeatedValue`.
- C++ swaps source and target for GSN-extension relationships (`xmi_reader.cpp:1585-1599`);
  here nothing is swapped (D-TERM).
- C++ reports a missing source and target as one finding; here there is one finding per
  missing end.

## Consequences

- Arguments with inferences, context and reasoning load, compare, save and can be built by
  edits.
- A strict save refuses a relationship missing its source or target. The release notes list
  this (ADR 0022).
- SACM23-ARG-004's tests kill the two mutants that ADR 0026 excluded until a multi-valued end
  existed.
