# 0029. Set, reference and reorder operations

- Status: accepted (the maintainer, 2026-11-15)
- Date: 2026-11-15
- Matrix rows: LIB-EDIT-008, LIB-EDIT-009, LIB-EDIT-010, LIB-EDIT-012. It extends ADR 0019,
  ADR 0025, ADR 0026, ADR 0027 and ADR 0028. ADR 0019 amendment 2 moves typed before-values to
  LIB-EDIT-011.

## Context

Until now `edit::apply` has only created elements. SACM 2.3 gives existing elements
editable features:
- `name:LangString[1]` (§8.6), `gid:String[0..1]`, `isCitation` and `isAbstract` (§8.2);
- `isCounter:Boolean[1]` (§11.13) and `assertionDeclaration[1]` (§11.10);
- `content:MultiLangString[0..1]` (§8.7) and `key:MultiLangString[1]` (§8.12);
- nine reference ends (§8.2, §11.2, §11.9, §11.10, §11.12, §11.13).

SACM marks no composition ordered, but this crate keeps order (D-ORDER). The C++ library has
one command per feature: `SetName`, `SetGid`, `SetCitation`, `SetMetaClaims`,
`SetRelationshipEnds`, `AddRelationshipSource`, `ReorderPackageElements`, and others.

## Decision

- **Ten operations, in `Operation`, each named after its struct:**
  - `SetName`, `SetGid` (`new` and `remove`), `SetFlag`, `SetAssertionDeclaration`;
  - `SetContent` (`new` and `remove`) and `SetKey` (no remove, because a key is `[1]`);
  - `SetReferences` (`new` and `clear`), `AddReference`, `RemoveReference`;
  - `ReorderElements` (`new(parent, order)` and `roots(order)`).
- **`model::Flag` and `model::ReferenceEnd` become public.**
  - Both are `#[non_exhaustive]`, and `name()` (the XMI name) is their only public method.
  - The tables (`ALL`, owners, declared types, bounds) stay crate-private.
  - A later class whose end has the same SACM name, such as §12.14's `source`, reuses the
    variant, with the declared type looked up per owner.
- **Every end is a list within its bounds.**
  - Set replaces it, in order with repeats. Add appends. Remove drops every occurrence.
  - A single-valued end holds at most one id. `source` needs at least one, and `target`
    exactly one.
- **Check order, for every operation:**
  1. The element is unknown: `ElementNotFound` (SACM-CMD-001).
  2. Its kind lacks the feature: `FeatureNotApplicable` (SACM-CMD-008).
- **Check order, reference operations:** after those two checks come:
  1. for Remove only: the id is not held, `ReferenceNotHeld` (SACM-CMD-001);
  2. the bounds: `TooFewReferences` or `TooManyReferences` (SACM-MULT-001);
  3. ADR 0028's `InvalidReference`, `ReferenceNotFound` and `WrongReferenceType`. These
     check only the ids the end does not already hold (C++'s repair exception, per end).
- **Reorder.**
  - The named subset goes back into the positions it occupies now. Every other child keeps
    its position.
  - Refusals, in order:
    1. an unknown parent (CMD-001);
    2. an empty list (SACM-CMD-009);
    3. then, id by id: an id that is not a direct child or root (`ChildNotFound`, CMD-001),
       and an id named twice (CMD-009).
  - `semantic_compare` reports the existing `ChildOrder` or `RootOrder`.
- **Ids are kept by position (D-PRESERVE).**
  - A replaced name keeps its `xmi:id`.
  - A replaced content or key keeps its role id. Each value keeps the id of the value at its
    position, and ids beyond the new length are freed.
  - Removing the last reference to an id frees it for generation.
- **`changed_ids`.**
  - It is `[element]` (for a reorder, `[parent]`) exactly when the element changes.
    Otherwise it is `[]`, and nothing is written.
  - A root reorder reports no id.
  - `created_ids` and `deleted_ids` are empty.
- **Content is never refused (D-VAL-LAYER).** `citedElement` and `isCitation` are set
  independently, and so are `abstractForm` and `isAbstract` (the maintainer, 2026-11-15).
  Reorder covers every element's children and the roots (the maintainer, 2026-11-15).
- **No previews yet.** The set operations apply without a preview until LIB-EDIT-004 and
  LIB-EDIT-007 (D-PREVIEW).

**Adopted from C++:**
- the names `SetName`, `SetGid` and `SetAssertionDeclaration`;
- CMD-001 for an unknown element and for an id that is not a child;
- the reorder semantics and its check order (`commands.cpp:825-911`);
- the repair exception for held unresolved ids (`operations.h:131-143`).

**Differences from C++, one line each:**
- C++ `SetName` takes a string and a language (`operations.h:241-245`); here it takes a full
  LangString (ADR 0019 E3).
- C++ refuses a name on a non-ModelElement as CMD-005 (`commands.cpp:1546`); here it is
  CMD-008.
- C++ `SetAssertionDeclaration` reports a non-Assertion as CMD-001 (`commands.cpp:620`); here
  it is CMD-008.
- C++ `SetGid` treats `""` as clear (`operations.h:235-238`); here `""` is a value, and
  `SetGid::remove` clears.
- C++ `SetCitation` sets `citedElement` and `isCitation` together (`commands.cpp:1369`); here
  they are separate operations.
- C++ has no operation for `isAbstract`, `isCounter` or `abstractForm`; here `SetFlag` and
  `SetReferences` cover them.
- C++ has one command per end (`SetMetaClaims`, `SetArtifactReferenceElements`,
  `AddMetaClaim`, `AddRelationshipSource`); here one family is keyed by `ReferenceEnd`.
- C++ `SetRelationshipEnds` replaces source, target and reasoning together
  (`operations.h:144`); here each end is edited alone, within its own bounds.
- C++ refuses empty sources or a second target as CMD-005 (`commands.cpp:735-752`); here it is
  `TooFewReferences` or `TooManyReferences` (MULT-001).
- C++ applies the repair exception only to relationship ends, across all three
  (`commands.cpp:713`); here it applies to every end, per end.
- C++ has no reference removal; here `RemoveReference` drops every occurrence and refuses an
  id that is not held.
- C++ `ReorderPackageElements` reorders only ArgumentPackage elements; here it covers any
  element's children and the roots.
- C++ reports an empty or repeated order as CMD-005 (`commands.cpp:842, 870`); here it is
  CMD-009.
- C++ edits content one language at a time, by owner and index or key
  (`operations.h:247-358`); here `SetContent` and `SetKey` address the UtilityElement by id
  and replace the whole MultiLangString.
- C++ records a Modified change even when the value is equal (`commands.cpp:1393`); here
  `changed_ids` is empty when nothing changes.

## Consequences

- Every feature the model holds can now be edited. Strict save still refuses content Errors,
  so validate after editing.
- LIB-EDIT-002's set and LIB-EDIT-003's oracle grow to cover set operations and `RootOrder`.
- Typed before-values are LIB-EDIT-011's (ADR 0019 amendment 2).
- ExpressionLangString names (SACM23-BASE-009) will widen `SetName` together with the creates.
