# Verification 0034 — Atomic edits and edit results

- Date: 2026-11-13
- Row verdict: LIB-EDIT-002: PASS
- Row verdict: LIB-EDIT-003: PASS
- Verifier: `sacm-verifier` (independent; read-only), 2 rounds; `sacm-docs-reviewer`, 1 round
- Commit: 26ddea9. Evidence:
  - CI run 43374915966 (PR-diff mutants: 7 caught, 0 missed);
  - full mutants run 43374911277 (26ddea9): 987 caught, 0 missed, 0 timed out.
- Slice brief: [docs/slices/0034-m1-atomic-edits-and-results.md](../slices/0034-m1-atomic-edits-and-results.md)

## Result

`edit::apply` is the only public mutation, and it is all or nothing:
- 767 operations over 11 starting documents, each applied alone and in sequence;
- after every success, the document is structurally valid, checked against a containment
  table written in the test from the standard;
- after every refusal, the document is `==` to before and generates the same next ids.

`Mutation` holds the operation as given, plus the ids created, changed and deleted. All
three match the `semantic_compare` oracle. `Operation::operation_name()` is stable.
Seventeen hand mutations were killed. The two `changed_ids` and `deleted_ids` empty-slice
mutants are equivalent in this release, since every `Mutation` is built with both lists
empty. They are excluded with triage rows. The ADR's C++ claims were checked against
`mutation.h` and `commands.cpp`.

## Findings

| Severity | Row | Finding | Resolution |
|---|---|---|---|
| Medium | LIB-EDIT-002 | The `apply` docs promised an edit neither adds nor removes `validate()` findings, against D-VAL-LAYER | Replaced: `apply` does not check content findings; call `validate` again after editing (26ddea9) |
| Low | LIB-EDIT-002 | Test and row clause citations were wrong or incomplete | Corrected (26ddea9) |
| Low | LIB-EDIT-003 | Two accessors' empty-slice mutants survived | Equivalent in this release; excluded with triage rows naming the rows that make them killable |
| Docs | LIB-EDIT-003 | "Atomic" was ambiguous; the parent rule and refusal logging were unclear | Applied (docs review) |
| Logged | LIB-EDIT-002 | References and required-end multiplicities are not yet structural checks | Noted in ADR 0019 amendment 1, for the first reference row |
| Logged | — | The lib.rs line says "each returns a `Mutation`" after naming the create operations | For the next slice touching lib.rs |
