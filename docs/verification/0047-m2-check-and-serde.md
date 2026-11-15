# Verification 0047 — Check an operation, and serde for edit values

- Date: 2026-11-15
- Row verdict: LIB-EDIT-013: PASS
- Row verdict: LIB-API-006: PASS
- Verifier: `sacm-verifier` (independent; read-only)
- Commit: f9b098c (round 1), d336b06 (round 2); evidence: CI runs
  [43666629895](https://github.com/lasrod/libSACM/actions/runs/43666629895) (f9b098c) and
  [43669059572](https://github.com/lasrod/libSACM/actions/runs/43669059572) (d336b06), all 16
  checks green; PR-diff mutants built with all features: 27 caught, 0 missed, 40 unviable
- Slice brief: [docs/slices/0047-m2-check-and-serde.md](../slices/0047-m2-check-and-serde.md)

## Result

**LIB-EDIT-013:**
- `check` equals `apply` on a clone for every operation struct, accepted and refused, with
  both id forms. The documents cover empty, edited, seam-built and XMI-read.
- `commit` cannot fail. Every `expect` in it holds an invariant that `plan` establishes,
  and no input reaches one.
- Hand mutations of the id counter, the reorder ids and the create flags were caught.

**LIB-API-006:**
- Golden files fix the JSON shape byte for byte.
- After round 2 these are all caught, over 46 hand mutations:
  - removing `deny_unknown_fields` from any of the 26 types;
  - defaulting any required key;
  - dropping any feature name from the `EditError` read-back.

## Findings

| Severity | Row | Finding | Resolution |
|---|---|---|---|
| blocking | LIB-API-006 | The `EditError` read-back could lose the `name`, `assertionDeclaration`, `content` and `key` feature names undetected. | Round 2: a test per feature name, refusals taken from `apply`. |
| blocking | LIB-API-006 | Unknown keys were refused in tests for only 5 of the 25 operation structs. | Round 2: a test driven by the golden files covers every object. |
| blocking | LIB-API-006 | Required keys were tested for only some structs (`SetFlag.value`, `InvalidOrder.repeated` could default). | Round 2: a test driven by the golden files covers every key. |
| follow-up | LIB-API-006 | The MSRV job did not build with the feature. | Fixed: `--all-features`. |
| follow-up | LIB-API-006 | "Nothing takes a record as input" ignored `has_errors`; the optional-key list was incomplete. | Fixed in the docs and ADR 0030. |
