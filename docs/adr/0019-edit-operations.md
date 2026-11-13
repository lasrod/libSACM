# 0019. The edit API: operations as values

- Status: accepted (the maintainer, 2026-11-12, with the NCName rule E6)
- Date: 2026-11-12
- Matrix rows: LIB-EDIT-001. LIB-EDIT-002, LIB-EDIT-003 and every later create row extend it.

## Context

LIB-EDIT-001 is the first public editing API. Previews, audit results (LIB-EDIT-003),
batching and a later GSN layer all need an operation to be a value. The assurance-forge C++
library has one aggregate per operation (`commands/operations.h`), applied by
`Document::apply` (`model/document.h:119`).

## Decision

- **`sacm::edit::apply(document: &mut Document, operation: impl Into<Operation>) ->
  Result<Mutation, EditError>`.** It checks the operation completely first, so a refused
  operation leaves the document unchanged.
- **One struct per kind:** `CreateAssuranceCasePackage`, `CreateArgumentPackage` and
  `CreateClaim`. They are gathered in `#[non_exhaustive] enum Operation`, with a `From` impl
  for each.
  - The package operations have `root(name)` and `new(parent, name)`. `CreateClaim` has only
    `new(parent, name)`.
  - All three have `with_id(id)`.
- **The name is required, as a `LangString`** (ADR 0013): an edit never leaves a ModelElement
  without a name. Any value is accepted, `LangString::default()` included.
- **Parent legality comes from the crate's one containment table,** the same one the reader
  and writer use.
- **Read accessors:** `parent()`, `id()` and `name()` on each operation struct, so that previews, audit logs and a later GSN layer can read an operation they hold. `impl From<&ElementId> for ElementId` lets a returned id be passed on as the next parent (the maintainer, 2026-11-12).
- **`Mutation::created_ids()`** lists the ids created, in creation order, with the
  operation's own element first.
- **`EditError`** is `#[non_exhaustive]`, and `code()` returns `Code`. The checks run in this
  order:
  1. `ParentNotFound` (SACM-CMD-001);
  2. `InvalidParent` (SACM-CMD-005);
  3. `InvalidId` (SACM-ID-002): the caller id is not an XML NCName;
  4. `DuplicateId` (SACM-CMD-004).
- **Ids.** A caller id must be an NCName (XMI 2.5.1 §7.6.1: `xmi:id` is `xsd:ID`).
  - A generated id is `<snake_case kind>_<n>`, for example `claim_1`. It is the smallest `n`
    above a private per-kind counter that is not in use.
  - The counter advances only on success.
  - The result is deterministic per sequence of operations (D-ID), and `Clone` copies the
    counter.
  - Ids read from files are held exactly as read; only new ids are checked.
- **`Document::insert` stays test-only** (`test-support`). It is the seam for states and
  kinds the edit API does not produce.

**Adopted from C++:**
- one operation struct per kind;
- the operation names;
- the codes CMD-001, CMD-004, CMD-005 and ID-002;
- the check order;
- the `<snake_kind>_<n>` scheme with a per-kind counter that skips used ids (`commands.cpp:43-90`);
- `created_ids()`.

**Differences from C++, one line each:**
- **E1.** A free function `edit::apply` instead of `Document::apply` (`document.h:119`), as for
  ADR 0018.
- **E2.** `Result<Mutation, EditError>` instead of `MutationResult { applied, diagnostics }`
  (`mutation.h:37-48`), by ADR 0011 X1.
- **E3.** The name is a full `LangString`. C++ takes `std::string name`, with `language` on
  `CreateClaim` only (`operations.h:23-46`). This follows ADR 0012/0013.
- **E4.** Parents come from the containment table, so an ArtifactPackage and package
  interfaces and bindings are allowed too (§12.2, ADR 0014). C++ narrows the parent kinds
  (`commands.cpp:147, 191, 228`).
- **E5.** An ArgumentPackage can be created as a root (ADR 0010 D1, ADR 0016). C++ requires
  a parent.
- **E6.** A caller id must be an NCName (XMI 2.5.1 §7.6.1). C++ refuses only empty ids or
  ids containing whitespace (`commands.cpp:99`).
- **E7.** A caller id does not advance the counter. C++ `advance_id_counter`
  (`commands.cpp:1997-2009`) parses its suffix and can throw on a long one.
- **E8.** `EditError::code()` returns `Code`, not `Option<Code>`, because every edit refusal
  has a catalog code.

## Consequences

Later rows add operation structs, `Operation` and `EditError` variants, builder methods, and
`Mutation` accessors (`changed_ids`, `deleted_ids`, `operation()`). All of these are
additive. Atomic batches of several operations, as the GSN layer will want, belong in a later
row.

## Alternatives considered

- Plain methods (`Document::create_claim`): previews, audit and batching need values, and
  adding commands later would give two ways to do one thing.
- A generic `create(kind, …)`: kinds will grow their own required fields, and misuse such as
  a Claim root could be written.
- Accepting any non-empty id, as C++ does: it gives output that fails `xsd:ID`.

## Amendment 1 (2026-11-13): edit results and atomicity (slice 0034; LIB-EDIT-002, LIB-EDIT-003)

- **`Mutation`** holds two things: the operation as given (`operation()`), and three id
  lists.
  - `created_ids` and `deleted_ids` are the ids that `semantic_compare(before, after)`
    reports as right-only and left-only.
  - `changed_ids` are the ids, present on both sides, for which it reports any per-element
    difference.

  So creating a child does not change its parent: the parent's own features and the relative
  order of its existing children are unchanged, and the parent is in `operation()`.
  `changed_ids` and `deleted_ids` are empty for every operation in this release.
  `Operation::operation_name()` gives the stable struct name (`"CreateClaim"`) for logs. It
  is a promise, like a diagnostic code.
- **Atomicity.** `edit::apply` is the only public mutation.
  - On success, the document is structurally valid: ids are unique, parent and child links
    agree, roots are packages, every child is in a role of its parent's class, and every
    Assertion has a declaration.
  - On refusal, the document is `==` to before and generates the same next ids.
  - Content findings from `validate()` are not part of structural validity (D-VAL-LAYER): an edit may add or remove them, and
    `apply` does not check them.
  - References are not part of the model yet. The structural checks gain "references
    resolve" and the multiplicities of required ends (D-VAL-LAYER) with the first
    reference row.

**Adopted from C++:** created, changed and deleted ids; a create does not report its parent
as modified (`commands.cpp:162-171`); the stable operation name (`operations.h:406`).

**Differences from C++:**
- **E9.** `Mutation` holds the typed `Operation`, not `std::string operation`
  (`mutation.h:38`).
- **E10.** It has three typed id lists instead of `std::vector<ChangeRecord>` with serialized
  `before` and `after` strings (`mutation.h:18-33`). The first row whose operation destroys
  data (a set or a delete) adds typed before-values, without a break.
- **E11.** There is no `document_revision` (`mutation.h:44`) until the preview rows
  (D-PREVIEW).
- **E12.** `changed_ids()` is its own accessor. C++ has only `created_ids()` and
  `deleted_ids()`, and leaves the changed ids to a filter over `changes`.

*Amendment 1 status: accepted (the maintainer, 2026-11-13).*
