---
slice: 0046
name: m2-wasm-and-containment
state: in-progress
step: design
rows: [LIB-QA-017, LIB-API-012]
mode: pr
base: main
---

# Slice 0046 — wasm32 build check and public containment queries

## Scope

Two requests from Assurance Forge that do not touch the edit code that slice 0045 is
changing.
- **LIB-QA-017 (GitHub issue #58):** CI builds `sacm` for `wasm32-unknown-unknown`, and a
  lint keeps file-system, thread, process, environment and clock APIs out of the library's
  non-test code.
- **LIB-API-012 (GitHub issue #60, second half):** the containment table becomes public on
  `ElementKind`.

Left out: `edit::check` (LIB-EDIT-013), batches (LIB-EDIT-014) and serde (LIB-API-006),
which wait for slice 0045. JavaScript bindings (LIB-API-007).

## Basis

Library and quality rows: acceptance by the lead. The containment table itself is
LIB-API-008's (verified, slice 0017); this slice only exposes it.

## Design

- **LIB-API-012:** `ElementKind::may_contain(self, child: ElementKind) -> bool` (`const`)
  and `ElementKind::child_kinds(self) -> impl Iterator<Item = ElementKind>` (in `ALL`
  order); roots stay `is_package`. ADR 0010 amendment 2 (proposed), deviation D4.
- **LIB-QA-017:** no API. `clippy.toml` lists the platform APIs; workspace lints allow
  `disallowed_methods`/`disallowed_types`; `sacm`'s `lib.rs` denies them outside `cfg(test)`.
  CI job `wasm` builds `sacm` for the target, default and all features.

## Log

- 2026-11-15 choose: Assurance Forge issues #57-#61 evaluated; the rows were planned in
  commit 387a12e (the maintainer chose M2 and a stable serde form). This slice takes the
  two rows independent of slice 0045, in the worktree `C:\development\libSACM-af-issues`,
  so that 0045's tree is not touched.
- 2026-11-15 design: architect brief for LIB-API-012 (parent-side queries only; roles stay
  private; ADR 0010 amendment 2 proposed). LIB-QA-017 lint mechanism checked in a scratch
  crate: aliased imports are caught, `cfg(test)` code is not. Both rows `analyzed`.
- 2026-11-15 tests: blocked. The agent write guard resolves paths against
  `CLAUDE_PROJECT_DIR` (the main checkout), so agents launched from that session cannot
  write in this worktree. Resume from a session opened in the worktree.
