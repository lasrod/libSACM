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
