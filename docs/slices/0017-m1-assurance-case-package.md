---
slice: 0017
name: m1-assurance-case-package
state: in-progress
step: verification
rows: [SACM23-AC-001, LIB-API-008]
mode: pr
---

# Slice 0017 — AssuranceCasePackage and nesting

## Scope

- **SACM23-AC-001:** AssuranceCasePackage as the top-level package of a SACM model, holding
  sub-packages through its four composition roles, in document order.
- **LIB-API-008:** containment legality. Every insert refuses a child kind that no
  SACM containment role of the parent admits.

Out of scope: ArgumentPackage contents (SACM23-ARG-001), the artifact and terminology
sub-packages (SACM23-ART-001, SACM23-TERM-001), interfaces and bindings (SACM23-AC-002,
-003), XMI (SACM23-XMI-001) and editing. Research decides what the row keeps, and gives
every obligation it drops a home row.

## Basis

formal/23-05-08 clause 9; ptc/22-03-13 `AssuranceCase::AssuranceCasePackage`; the
assurance-forge C++ `libs/sacm` model.

## Design

No new stable API, and no new ADR (the rule is D-VAL-LAYER; ADR 0014 covers the C++
deviation).
- **Private `model/containment.rs`:** `pub(crate) enum ContainmentRole` (12 roles) and
  `pub(crate) const fn role_of(parent: ElementKind, child: ElementKind) -> Option<ContainmentRole>`,
  built from `is_a`/`is_kind_of`. Not public, as in C++ (`name_tables.h`, "not installed").
  Role names arrive with SACM23-XMI-016.
- **`test-support` `InsertError::ContainmentNotAllowed { id, kind, parent, parent_kind }`**:
  `element "c1" of kind Claim cannot be contained in element "case" of kind
  AssuranceCasePackage`. It is checked last in `insert_node`, after NameNotAllowed. Roots
  are not checked.
- **Docs:** `insert` (what it checks, `# Errors`, a refusal example) and `roots()`
  (roots may be of any kind).

## Log

- 2026-11-10 brief written. The researcher is next.
- 2026-11-10 research, AC-001: §9.2 gives four composition roles (`assuranceCasePackage`, `argumentPackage`, `artifactPackage`, `terminologyPackage`), none ordered, no constraints. "Mandatory unit of interchange" is the compliance point's wording (§2.4), owned by CP-001, XMI-001 and XMI-013. The document accepts any kind under any parent today, so AC-001 alone would be green before any code. New rows proposed: LIB-API-008 (containment legality on every insert) and SACM23-XMI-016 (the four roles in XMI).
- 2026-11-10 the maintainer decided: the slice takes LIB-API-008; ArtifactPackage admits every ArtifactElement, as §12.2 and ptc type it, departing from C++ `name_tables.cpp:124` (ADR to follow). Research on LIB-API-008 is next.
- 2026-11-10 research, LIB-API-008: 12 composite roles have element-kind types, all 0..*, none ordered; they give 339 legal (parent, child) pairs of 1,444, and in each the child fits exactly one role. The four UtilityElement kinds contain no elements. Multiplicity stays with validation. The containment check runs last, after NameNotAllowed. New spec defect D9 (§10.4 role spelling; its prose omits TerminologyGroup, which the declared type admits; the model is followed, as C++ does). ADR 0014 (accepted) records the ArtifactPackage decision. New row SACM23-XMI-016 (containment children conform to their role in XMI, M2); ART-001, TERM-001 and XMI-009 reworded. Both rows are `analyzed`. The architect is next.
- 2026-11-10 design: the role table stays private, as in C++; the one new item is `InsertError::ContainmentNotAllowed` under `test-support`. LIB-API-008's drift test now works through `insert` against the inventory, and the role-uniqueness item moved to SACM23-XMI-016, where role names become observable. LIB-API-004 is re-verified with this slice. The test engineer is next.
- 2026-11-10 tests: 17 `lib_api_008_*` in `crates/sacm/tests/containment.rs` (the spelled-out 339-pair table, and the inventory drift oracle), 8 `sacm23_ac_001_*` in `crates/sacm/tests/assurance_case_package.rs`, and the new variant in the `lib_api_004_*` error tests; one `errors.rs` setup moved its Claims under an ArgumentPackage. The red was confirmed by the lead: E0599, no `ContainmentNotAllowed`. The 8 AC-001 tests already pass, as expected: AC-001's facts are positive, and LIB-API-008 makes them exclusive, so they guard against over-restriction. Six LIB-API-008 tests, the acceptance and oracle checks, are the same kind. Both rows are `tests-failing`. The implementer is next.
- 2026-11-10 implementation: private `model/containment.rs` (`ContainmentRole`, `role_of`), and the check plus `ContainmentNotAllowed` in `model/document.rs`, with the `insert` and `roots()` docs. The implementer found one wrong test (it paired an id with itself, so DuplicateId rightly came first); the test engineer fixed the pairing. The full gate passes (619 tests). Both rows are `implemented`. Verification is next.
