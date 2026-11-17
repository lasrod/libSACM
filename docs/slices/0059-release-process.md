---
slice: 0059
name: release-process
state: in-progress
step: verification
rows: [LIB-QA-005]
mode: pr
base: main
---

# Slice 0059 — The release process

## Scope

A documented, repeatable release process (`docs/releasing.md`), and packages fit to
publish:
- each crate ships the MIT `LICENSE`, which it lacks today;
- no package carries tests or test data (`sacm` would carry 4.4 MB of them today);
- tests keep the versions and the CHANGELOG consistent.

Left out: the publish itself, the tag and the GitHub release of 0.1.0. They wait for the
maintainer's go.

## Basis

Quality row: acceptance by the lead. The maintainer decided on 2026-11-17: prepare the
release and the dry run, then stop and ask.

## Log

- 2026-11-17 choose: the last row of M3.
- 2026-11-17 design: by the lead, because the row is a QA row with no public API. A dry
  run of the packaging found that both packages lack LICENSE and carry their tests. The row
  is `analyzed`.
- 2026-11-17 tests: `sacm-builder` step 1 wrote `xtask/tests/release.rs`, five tests. The
  lead confirmed the red: the package-contents test fails, because neither package has
  LICENSE and both list `tests/`, and the license test fails, because the crate LICENSE
  copies are missing. The version and CHANGELOG test and the two checker tests pass, as
  guards. The snapshot is `red-0059.tar`.
- 2026-11-17 implementation: `sacm-builder` step 2 made these changes:
  - an `include` list in each manifest;
  - `docs/releasing.md`;
  - the CHANGELOG line.
  The write guard blocked the two `crates/*/LICENSE` copies and the CONTRIBUTING.md line.
  The maintainer chose that the lead writes them, and the lead did. The dry run
  (`cargo publish --dry-run --workspace --locked`) succeeds, and packages `sacm` and
  `sacm-cli` with LICENSE and without tests.
