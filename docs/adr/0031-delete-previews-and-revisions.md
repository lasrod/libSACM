# 0031. Delete, delete policies, previews and revisions

- Status: accepted (the maintainer, 2026-11-15)
- Date: 2026-11-15
- Matrix rows: LIB-EDIT-004, LIB-EDIT-005, LIB-EDIT-006, LIB-EDIT-007. It extends ADR 0019,
  ADR 0026, ADR 0028, ADR 0029 and ADR 0030. It resolves ADR 0019 E11 and replaces ADR
  0029's "no previews yet".

## Context

Until now no edit removed an element. Deleting one can leave references dangling, which
D-VAL-LAYER forbids an edit to do. D-POLICY says a cascade never happens implicitly, and
D-PREVIEW says a destructive edit can be previewed and that a stale preview is refused.
SACM 2.3 is silent on editing, so the assurance-forge C++ library's design is the default
(`commands/policies.h`, `commands/operations.h:362`, `commands.cpp:1771-2140`,
`document.cpp:36-111`).

## Decision

### Delete

- **One operation, `DeleteElement::new(element)`, for every kind.** It has three policies,
  each set by a builder and each defaulting to reject:
  - `ReferenceDeletePolicy`: `RejectIfReferenced` (the default),
    `DeleteReferencingRelationships` or `ScrubReferences`;
  - `PackageDeletePolicy`: `RejectIfNonEmpty` (the default) or `DeleteRecursively`;
  - `CrossPackageReferencePolicy`: `RejectIfExternalReferencesExist` (the default) or
    `ApplyReferencePolicy`.
- **The deleted set.** It is the element and everything it contains, plus every relationship
  the policy deletes, each with its contents. It is computed as a fixpoint, because a
  relationship can be the source or target of another relationship.
- **A dangling reference** is a reference held by an element outside the deleted subtree
  whose id names a deleted element. There is one entry per distinct id and end, ordered by
  holder in document pre-order, then by end, then by position.
- **What each reference policy does:**
  - `RejectIfReferenced`: any dangling reference refuses the delete (SACM-CMD-002).
  - `DeleteReferencingRelationships`: an asserted relationship whose `source`, `target` or
    `reasoning` names a deleted element is deleted. Every other dangling id is removed from
    its end. That covers `citedElement`, `abstractForm`, `metaClaim`, `structure`,
    `referencedArtifactElement` and a group's `argumentElement`; none of these ends has a
    lower bound. A group keeps existing when it loses members.
  - `ScrubReferences`: every dangling id is removed. A holder left below an end's lower bound
    is deleted: today, a relationship left with no `source` (§11.13 `[1..*]`) or no `target`
    (`[1]`).
  - Under either non-reject policy, no required end is ever left empty.
- **Contents.** An element is non-empty when it contains any element other than a
  UtilityElement. `NotEmpty` gives the number of such elements at any depth. Descriptions, notes, implementation constraints and tagged values go with
  their owner and do not count.
- **Package boundary.** A reference crosses one when the holder's nearest package is not the
  nearest package of the element the reference names. The nearest package is the element itself or its closest ancestor of any of the
  12 package, interface and binding kinds.
- **Check order** (the first failure is the result):
  1. `ElementNotFound`;
  2. `NotEmpty` (SACM-CMD-006);
  3. the first dangling reference a policy refuses: `CrossPackageReference` (SACM-CMD-007)
     if it crosses a package boundary and the cross-package policy rejects, otherwise
     `Referenced` (SACM-CMD-002).
- **The `Mutation`.**
  - `deleted_ids` lists every deleted element in pre-order before the delete.
  - `changed_ids` lists each surviving holder that lost ids, in pre-order after.
  - `created_ids` is empty, and the parent is not changed.
- **Freed ids may be used again.** A delete does not rewind the per-kind counter, so
  generation stays deterministic: the next generated id is still the smallest unused `n`
  above the counter (ADR 0019). libSACM makes no promise that a freed id is never used again.
  A caller may give it with `with_id`, and a freed id the caller gave, if above the counter,
  may be generated later. An application that needs ids unique across its history gives its
  own (the maintainer, 2026-11-15).

### Preview and revision

- **`edit::preview(&Document, &Operation) -> Result<Preview, EditError>`** works for every
  operation and never changes the document.
  - It refuses exactly as `check` and `apply` do.
  - `Preview::mutation()` is exactly what `apply` would return.
  - `Preview::dangling_references()` lists each `DanglingReference`: its holder, end and id,
    and whether it crosses a package boundary.
  - To learn whether a delete applies under some policy, preview it with that policy.
- **`edit::apply_preview(&mut Document, &Preview) -> Result<Mutation, EditError>`.**
  - It refuses with `PreviewExpired` (SACM-CMD-003), changing nothing, unless the document's
    revision equals the preview's.
  - Otherwise it applies the preview's operation and returns a `Mutation` equal to
    `preview.mutation()`.
  - A delete without a preview stays `edit::apply`.
- **`Document::revision() -> Revision`.** A revision is opaque: it can be compared for
  equality and hashed, and nothing else.
  - It identifies one state of a document within the process (the maintainer, 2026-11-15),
    so a preview taken on a copy never applies to the original if either
    had changed before the preview was taken.
  - A new, default or read document gets a fresh revision, and so does every successful
    edit, even one that changes nothing.
  - `Clone` copies the revision, and a refusal keeps it.
  - `==` ignores it (ADR 0020).
- **serde** (ADR 0030):
  - `DeleteElement`'s three policy keys may be omitted on input, and they default to reject.
    Policy values are their variant names.
  - `DanglingReference` reads and writes.
  - **`Preview` is written only** (the maintainer, 2026-11-15): a preview read back proves
    nothing about the current document.
  - A `Revision` is a JSON number.
  - A `Preview` is written as `{"revision", "mutation", "dangling_references"}`; its
    operation is the mutation's.
- **Codes.** `Code::CMD_DELETE_REFERENCED` (SACM-CMD-002), `CMD_PREVIEW_EXPIRED` (-003),
  `CMD_PACKAGE_NOT_EMPTY` (-006) and `CMD_EXTERNAL_REFERENCES` (-007), the C++ names.

**Adopted from C++:**
- `DeleteElement` and the three policy enums, with reject defaults;
- `ScrubReferences`;
- the rule that UtilityElements do not count as contents;
- the nearest-package rule;
- the check order CMD-006, then CMD-007, then CMD-002;
- a general preview, guarded by a revision;
- the code names.

**Differences from C++:**
- **R1.** A relationship that scrubbing leaves with no source or no target is deleted
  (§11.13). C++ keeps an AssertedInference with no sources while its reasoning survives
  (`relationship_invalid_after_scrub`).
- **R2.** The cascade deletes a relationship only through `source`, `target` or
  `reasoning`; its other ends just lose the id. C++ deletes through any end
  (`traverse.cpp:98`).
- **R3.** `ApplyReferencePolicy` replaces C++ `DeleteExternalReferencingRelationships`,
  since under `ScrubReferences` nothing need be deleted.
- **R4.** The policies are private fields set by builders. In C++ they are public fields.
- **R5.** `preview` returns `Result<Preview, EditError>`, holding the `Mutation` that
  `apply` would return. C++ returns `OperationPreview { can_apply, effects, diagnostics }`
  (`mutation.h:52`).
- **R6.** `apply_preview(&mut doc, &preview)` replaces C++ `apply(op, expected_revision)`
  (`document.h:119`): nobody passes a revision by hand.
- **R7.** A revision identifies a state within the process. In C++ it is a per-document
  counter (`document.h:108`).
- **R8.** There is no `ChangeRecord::RelationshipDeleted` with before-summaries. Instead
  `deleted_ids` and `dangling_references` report what goes, and before-values are left to
  undo (ADR 0019 amendment 2).

## Consequences

- The arena compacts on delete. Handles are remapped, and reference counts are released.
  This is O(n) per delete.
- `edit::apply`, `apply_preview`, `batch` and `apply_all` (ADR 0032) are the public
  mutations, and each is atomic.
- Revision values depend on how many documents the process has made. Tests compare them only
  for equality.

## Alternatives considered

- **A per-document revision counter.** A preview taken on a copy could apply to the
  original when the counters match.
- **A separate package delete.** One operation, with a package policy, covers it.
- **A preview that lists every policy's outcome at once.** Previewing with the policy wanted
  is simpler and exact.
