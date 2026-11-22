# Follow-ups

Non-blocking findings from verification, docs review and Copilot (ADR 0006 amendment 3).
A docs slice clears them every few slices; add new ones at the end.

| Source | Where | Finding |
|---|---|---|

Cleared at M5's close (slice 0071): every earlier item was done, decided, or moved to a matrix row (LIB-EDIT-020); see `docs/verification/0071-m5-close.md`.
| Slice 0072 | CLAUDE.md and the sacm-expert skill (rule 1) | Both name the sources `cargo xtask fetch-spec` downloads; since LIB-TOOL-022 they should also name XML Schema Part 2. Needs the maintainer's OK to edit CLAUDE.md. |
| Slice 0073a | `crates/sacm/src/validation/metadata.rs` | An `xmi:id` inside a Difference's `<target>` (XMI.xsd `processContents="skip"`) is still checked as an NCName and for duplicates, so a strict save refuses one schema-valid corner. |
| Slice 0073a | XMI writer | Kept fragments re-declare every in-scope prefix on each fragment. Lossless and byte-stable, but verbose on real MagicDraw files. |
| Slice 0073a | `xmi::write` "# Output size" | With `max_depth` raised far above the default by the caller, the documented bound `(2·D + 48)·B` allows a very large write (a 200,000-root proxy chain under depth 1,000,000 aborts on a 20 GB allocation). The docs could warn that the bound is quadratic for a raised depth. |
| Slice 0073b (LIB-EDIT-002) | `crates/sacm/tests/edit_atomic.rs` | The randomized atomicity test in `crates/sacm/tests/edit_atomic.rs` does not cover the href forms of the reference edits and creates; their atomicity is covered only by the `refuse` helpers in `crates/sacm/tests/edit_hrefs.rs`. |
| Slice 0074a (LIB-QA-007) | `.github/workflows/ci.yml`, `mutants-shard` | The PR-diff mutants job does not run `cargo xtask fetch-spec`, so the spec-dependent xtask tests (metamodel, schema) skip there, and mutants killed only by them show as missed. Add `cargo xtask fetch-spec` (with the OMG references cache) to that job, or triage them. |
