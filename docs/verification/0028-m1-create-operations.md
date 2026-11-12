# Verification 0028 — Create operations

- Date: 2026-11-12
- Row verdict: LIB-EDIT-001: PASS
- Verifier: `sacm-verifier` (independent; read-only), 2 rounds; `sacm-docs-reviewer`, 1 round
- Commit: f9d824a. Evidence:
  - CI runs 43185532546 (58a45b3) and 43223411758 (f9d824a);
  - full mutants run 43185521032 (58a45b3): 935 caught, 0 missed, 0 timed out. Library code
    is unchanged since then apart from doc comments.
- Slice brief: [docs/slices/0028-m1-create-operations.md](../slices/0028-m1-create-operations.md)

## Result

`sacm::edit::apply` (ADR 0019) creates AssuranceCasePackages, ArgumentPackages and Claims:
- roots or children, with a required name stored exactly;
- a caller id that must be an NCName, or a generated `<kind>_<n>` id that is deterministic
  and skips ids in use.

It refuses an unknown parent, an illegal parent, a non-NCName id and a duplicate id, in that
order, with catalog codes. A refused operation leaves the document unchanged. Parent legality
is checked against an independent table for all 38 parent kinds. The id-search mutants and
every NCName range edge are killed. One equivalent mutant (`commit_generated_id`) is excluded
until deletes exist.

## Findings

| Severity | Row | Finding | Resolution |
|---|---|---|---|
| Major | LIB-EDIT-001 | Generation skipping was under-tested (a generated id could fail as `DuplicateId`) | Tests with several used ids, and with every element a candidate (f9d824a) |
| Moderate | LIB-EDIT-001 | NCName range edges were unpinned | Edge tests for first and later characters (f9d824a) |
| Major | LIB-EDIT-001 | Accessors and `From<&ElementId>` were not approved | The maintainer kept them: ADR 0019 line and an acceptance item (f9d824a) |
| Minor | LIB-EDIT-001 | Catalog rows and status note, and citations | Fixed (f9d824a) |
