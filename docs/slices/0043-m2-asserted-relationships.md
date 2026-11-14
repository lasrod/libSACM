---
slice: 0043
name: m2-asserted-relationships
state: done
step: verified
rows: [SACM23-ARG-004, SACM23-ARG-005, SACM23-ARG-007, SACM23-ARG-008, SACM23-ARG-010, SACM23-VAL-005]
mode: pr
base: main
---

# Slice 0043 — Asserted relationships, reasoning and meta-claims

## Scope

The core of SACM argumentation, built on ADR 0026's references:
- **SACM23-ARG-004:** Assertion `metaClaim`, the first multi-valued reference end.
- **SACM23-ARG-005:** ArgumentReasoning, with its optional `structure` (an ArgumentPackage).
- **SACM23-ARG-007:** the shared AssertedRelationship features: `source` (1..*), `target`
  (1), `reasoning`, `isCounter` and `assertionDeclaration`.
- **SACM23-ARG-008 / -010:** AssertedInference and AssertedContext, the first concrete
  relationships.

Slice 0044 follows with ArtifactReference, the artifact relationships, ArgumentGroup and
reference-target typing (SACM23-ARG-006, -009, -011, -012, -013, SACM23-VAL-004).

## Basis

SACM 2.3 §11 (Argumentation); ptc/22-03-13; ADR 0026; ADR 0013; ADR 0019; D-VAL-LAYER.

## Design

[ADR 0027](../adr/0027-asserted-relationships.md), accepted by the maintainer on 2026-11-14 (a repeated id is a Warning, SACM23-VAL-022, a later row). API: `SacmElement::{meta_claims, sources, target, reasoning, structure, is_counter}`; `edit::{CreateArgumentReasoning, CreateAssertedInference, CreateAssertedContext}`; `EditError::{TooFewReferences, ReferenceNotFound}`; `Difference::{IsCounter, MetaClaims, Reasoning, Sources, Target, Structure}`. SACM23-VAL-005 joins the slice. D-ORDER now covers reference ends.

## Log

- 2026-11-14 choose: the relationship core; it unblocks the remaining argumentation rows and
  LIB-EDIT-008/009.
- 2026-11-14 verify: PASS in two rounds (a quadratic dedupe fixed; two hand mutants pinned); Copilot found a white-space reference id, fixed; record 0043.
