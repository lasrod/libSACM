# 0010. Document model: arena storage, ElementId, borrowing views

- Status: accepted (the maintainer approved it, including D1–D3, on 2026-11-09)
- Date: 2026-11-09
- Matrix rows: LIB-API-002, LIB-API-003

## Context

The first library slice needs a `Document`, element identity and element kinds. The
assurance-forge C++ library (`libs/sacm`) is the approved design reference:
- `model/document.h` and `model/element_id.h`;
- `metadata/element_kind.h`;
- a hand-written model, checked against the metamodel inventory by a test.

## Decision

- **Storage.** The `Document` keeps its elements in a private arena of nodes
  (`id, kind, parent, children`), a list of roots, and an `ElementId → node` index that is
  only ever looked up. `ElementId` is the `xmi:id`, kept exactly as given (D-ID).
- **Views.** Read-only views borrow the document: `SacmElement<'a>`, with `id`, `kind`,
  `parent` and `children`. No index or arena handle is public. Mutation happens only
  through later edit operations and the XMI reader, as in C++.
- **Kinds.** `ElementKind` lists the 38 concrete SACMElement classes, in the C++ order.
  The value types LangString, MultiLangString and ExpressionLangString have no kind.
  `AbstractClass` and `MetamodelPackage` are separate enums. All three are written by hand.
  A test checks them against `docs/sacm/metamodel-inventory.md`.
- **Test seam.** An off-by-default cargo feature, `test-support`, makes the crate-internal
  `Document::insert` public for tests. It is the counterpart of C++ `detail::Access`. It
  is documented as not part of the stable API, and the crate dev-depends on itself with
  it, so a plain `cargo test` works.
- **Deviations from C++:**
  - **D1.** One `roots()` list of any package kind, in document order, instead of
    `roots()` plus `other_roots()`. The C++ split comes from typed storage, and it loses
    the order of mixed roots.
  - **D2.** A public generic `children()` over one ordered containment list, instead of
    per-role vectors only. It keeps document order across roles. Per-role accessors
    come later, as filters.
  - **D3.** `Document` is `Clone`. C++ deletes copying because of its raw-pointer
    indexes, which the Rust design does not have.

## Consequences

- **Later rows build on this without breaking changes:** typed views (`Claim<'a>`),
  attributes, the XMI reader and edits.
- **Navigation is testable now** through the `test-support` feature, without committing
  to a public construction API.
- **Packaging:** the inventory test uses `include_str!` on a repository file, so it will
  not build in the packaged crate. This is to be settled in the release rows.

## Alternatives considered

- **A public `DocumentBuilder` now.** Rejected: it commits to a construction API before
  the edit design, against C++.
- **Deferring LIB-API-002's tests.** Rejected: it blocks the rows that depend on it.
- **Generating `ElementKind` from the metamodel.** Rejected: hand-written code plus a
  drift test is simpler, as in C++.

## Amendment 1 (2026-11-13): the node as it stands

Later rows added to the node. Each element's node now holds:
- `id`, `kind`, `parent` and `children`, as above;
- its `name` (ADR 0012, ADR 0013);
- its `assertionDeclaration` (SACM23-ARG-003);
- the containment `role` it is held in, computed once when it is inserted (`None` for a
  root; ADR 0017 amendment 2).

The `Document` also holds one generated-id counter per kind (ADR 0019). The counters are
not SACM content and are not compared (ADR 0020). `Document` gains `PartialEq` and `Eq`
from ADR 0020. Exhaustive destructures of the node and the document make a new field fail to
compile until comparison handles it.

**Open:** "`ElementId` is the `xmi:id`" assumes every element has one. The maintainer
decided on 2026-11-12 that an element with no `xmi:id` loads with a generated id
(SACM23-XMI-020). Whether that id is marked, whether it is written back, and how comparison
treats it needs its own ADR before that row's tests.

## Amendment 2 (2026-11-15): public containment queries (slice 0046; LIB-API-012)

Containment legality becomes public on `ElementKind`, so that an application offers only the
children the library accepts (GitHub issue #60):
- `const fn may_contain(self, child: ElementKind) -> bool`: whether a composite role of this
  kind's class admits `child`;
- `fn child_kinds(self) -> impl Iterator<Item = ElementKind>`: the kinds it may contain, in
  the order of `ElementKind::ALL`.

Both read the one table in `model::containment` that insertion, the XMI reader and the edit
operations use, so they cannot disagree. The reader may still refuse an admitted child it
cannot read yet (SACM23-XMI-016). `is_package` already answers which kinds may be a
root. Role names and the role tables stay crate-private until SACM23-XMI-016.

**Deviation from C++:**
- **D4.** C++ has no public containment query: `metadata/element_kind.h` exposes only
  `is_package_kind`, and parent legality is internal (`io/xmi_reader.cpp` attach_child,
  `commands.cpp` parent checks).

*Amendment 2 status: accepted (the maintainer, 2026-11-15).*
