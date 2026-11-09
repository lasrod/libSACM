---
slice: 0013
name: m1-document-model
state: in-progress
step: verification
rows: [LIB-API-002, LIB-API-003]
mode: pr
---

# Slice 0013 — the document model and element kinds

## Scope

The first library code:
- **LIB-API-002:** a `Document` that owns a root package and its elements, `ElementId`, and typed, read-only navigation.
- **LIB-API-003:** a public `ElementKind` for every concrete SACM 2.3 class, with its class name, its package, and is-a queries against the abstract superclasses.

Out of scope: element attributes beyond what navigation needs, XMI, validation, editing.

## Basis

These are LIB rows. They follow:
- `docs/architecture.md` (Model);
- `docs/sacm/decisions.md` (D-ID);
- `docs/sacm/metamodel-inventory.md`, derived from ptc/22-03-13;
- the assurance-forge C++ library, `libs/sacm`. It is the approved design reference: any deviation needs an ADR.

## Design

[ADR 0010](../adr/0010-document-model.md) (accepted). The module is `sacm::model`.

- **`ElementId`:** a string newtype.
- **`Document`:** `new`, `roots`, `get`, `contains`, `element_count`, `elements`.
- **`SacmElement<'a>`:** `id`, `kind`, `parent`, `children`.
- **Kinds:** `ElementKind` (38 kinds, with `ALL`, `name`, `package`, `is_a`, `is_kind_of`, `is_package`), `AbstractClass` (13) and `MetamodelPackage` (5).
- **Test seam:** the `test-support` feature makes `Document::insert` public.

## Log

- 2026-11-09 brief written. Research is skipped: both are LIB rows, and the inventory exists. The architect is next.
- 2026-11-09 design: the architect's brief follows the C++ `document.h`, `element_id.h` and `element_kind.h`, with deviations D1–D3. The maintainer chose the `test-support` feature as the test seam. ADR 0010 is proposed, and both rows are `analyzed`.
- 2026-11-09 the maintainer approved ADR 0010, including D1–D3.
- 2026-11-09 tests: 41 tests (27 in `crates/sacm/tests/document.rs`, 14 in `crates/sacm/tests/element_kind.rs`). They are red: `sacm::model` is unresolved. Both rows are `tests-failing`. Item 10 of LIB-API-002 and the `#[non_exhaustive]` part of item 8 of LIB-API-003 are left to the verifier's review. The implementer is next.
- 2026-11-09 implementation: `sacm::model` (`ElementId`, `Document`, `SacmElement`, `ElementKind`, `AbstractClass`, `MetamodelPackage`), the `test-support` feature and its self dev-dependency. 43 tests and 29 doctests pass. The local mutation run left 3 missed (`Debug`/`Display` output); the test engineer added 2 tests that kill them and fixed clippy in the tests. Both rows are `implemented`, and the full gate passes.
- 2026-11-09 docs review: 3 blocking findings (hidden `insert` lines make the examples panic; the crate README and root README misstate what is built), plus should-fix clarity items. These go to the implementer. Ergonomic suggestions are logged for later rows, not this slice: `descendants`/`ancestors`, `PartialEq` on `SacmElement`, exact-size iterators, and `ElementKind::from_name`, which the XMI reader will need.
- 2026-11-09 verification round 1 at `871764a` ([record](../verification/0013-m1-document-model.md)): both rows FAIL on test adequacy. Names are not pinned per variant, and exact id preservation and exact duplicate detection are untested. Fix round 1 (of 2) adds 3 tests and goes to the test engineer.
- 2026-11-09 fix round 1: 3 tests (exact per-variant names; ids kept byte for byte; ids differing in case or space are distinct), together with the docs-review fixes. The full gate passes.
- 2026-11-09 verification round 2 at `d2dd904` ([record](../verification/0013-m1-document-model-round2.md)): LIB-API-003 PASS, and it is now `verified`. LIB-API-002 FAIL (its second) on exact `contains` and parent lookup, which the verifier names as its own round 1 miss. The slice stops for the maintainer's decision.
- 2026-11-10 the maintainer approved a third, test-only fix round for LIB-API-002: exact `contains` and an exact parent lookup. Two small doc fixes run in parallel.
- 2026-11-10 fix round 2: near-miss queries for `contains`, and ` arguments` and `argumentsé` as unknown parents (in existing tests). Two doc fixes. The full gate passes.
