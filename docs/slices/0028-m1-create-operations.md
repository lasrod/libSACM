---
slice: 0028
name: m1-create-operations
state: in-progress
step: verification
rows: [LIB-EDIT-001]
mode: pr
base: slice/0027-m1-validate-missing-name (PR #38)
---

# Slice 0028 — Create operations

## Scope

The first public editing API. It creates elements through explicit operations that take a
parent, a `name` LangString and an optional caller-provided id. Generated ids are
deterministic for a given sequence of operations. An invalid parent is refused
(SACM-CMD-005), and so is an id that already exists (SACM-CMD-004). The kinds are
AssuranceCasePackage, ArgumentPackage and Claim at first; each later model row extends the
operation to its class. Atomic mutations (LIB-EDIT-002) and mutation results (LIB-EDIT-003)
come next, and the design must leave room for them.

## Basis

A library row: the acceptance is written by the lead from the design. ADR 0010 (the model),
ADR 0011 (errors), D-ID (ids), and the assurance-forge C++ command API.

## Log

- 2026-11-12 choose: LIB-EDIT-001, the maintainer's choice over LIB-COMPAT-001. Load modes
  come with the first tolerant variation that changes behaviour. Base:
  `slice/0027-m1-validate-missing-name` (PR #38).
- 2026-11-12 design (architect): accepted by the maintainer as ADR 0019, with E6 (caller ids must be NCNames). LIB-EDIT-001 is `analyzed`, with acceptance written by the lead.
  - API: `edit::apply`, `Operation`, the three create structs, `Mutation::created_ids`, `EditError` (CMD-001, CMD-005, ID-002, CMD-004), and generated ids `<kind>_<n>`.
- 2026-11-12 tests: 30 tests in `edit_create.rs`, with an independent parent-legality table; 8 seeded faults are all caught. The red is a compile failure.
- 2026-11-12 implementation: `sacm::edit` (`apply`, `Operation`, three create structs, `Mutation`, `EditError`), `is_ncname`, and a per-kind id counter on `Document`. The local gate passes.
  - Note for the verifier: unreachable `insert_node` error arms map to `InvalidId`.
