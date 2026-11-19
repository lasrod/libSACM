---
slice: 0070
name: record-args-any-order
state: in-progress
step: verification
rows: [LIB-API-019]
mode: pr
base: main (74ae779, the 0.2.0 release)
---

# Slice 0070 — Record arguments read in any key order

## Scope

With `serde`, a `ReadError`, `WriteError` or `Diagnostic` record reads its `arguments` object in any key order. They are matched by name and held in the template's order. A missing, extra, repeated or wrongly typed argument is still refused, and serialization still writes the template's order.

Why: JSON passed through `serde_json::Value` without `preserve_order` has its keys sorted, and an application such as Assurance Forge may store records that way.

The slice also clears one test follow-up: the atomic-batch set does not create the Terminology kinds, or the Terminology and AssuranceCase interfaces and bindings.

## Basis

ADR 0030 amendment 5, accepted by the maintainer on 2026-11-18, relaxes R11. Library API, with acceptance by the lead; it changes no type or signature, so there is no designer step.

## Log

- 2026-11-19: the row is analyzed. It is a one-row slice, because nothing else in M5 is related.
- 2026-11-19 tests: 6 LIB-API-019 tests (cited). Red confirmed from the builder's run: arguments out of order are refused today.
  Changed existing tests: the "out of order" cases in serde_refusals.rs and serde_diagnostics.rs are removed (each now
  has a lib_api_019 twin that must read `==`); the missing and extra messages become named. edit_atomic.rs now creates
  every kind, the Terminology, AssuranceCase interface/binding and ArtifactAssetRelationship creates included
  (follow-up from slice 0067; all 40 pass). LIB-API-017 and -018 acceptance reworded. Snapshot red-0070.tar.
- 2026-11-19 implementation: the builder implemented by-name argument reading with the named messages; its report did not
  arrive (hand-back refused), so the lead checked the tree and runs the gate; the verifier checks the mutation adequacy.
  `lib_api_017_arguments_missing_extra_out_of_order_…` is renamed `…_missing_extra_or_of_the_wrong_kind_are_refused`.
