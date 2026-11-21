# Follow-ups

Non-blocking findings from verification, docs review and Copilot (ADR 0006 amendment 3).
A docs slice clears them every few slices; add new ones at the end.

| Source | Where | Finding |
|---|---|---|

Cleared at M5's close (slice 0071): every earlier item was done, decided, or moved to a matrix row (LIB-EDIT-020); see `docs/verification/0071-m5-close.md`.
| Slice 0072 | CLAUDE.md and the sacm-expert skill (rule 1) | Both name the sources `cargo xtask fetch-spec` downloads; since LIB-TOOL-022 they should also name XML Schema Part 2. Needs the maintainer's OK to edit CLAUDE.md. |
| Slice 0073a | `crates/sacm/src/xmi/roots.rs` | For a type-spelled or `xsi:type` proxy that names no root, the SACM-XMI-013 or SACM-XMI-010 diagnostic affects the id the proxy names, which is no element of the document. It should affect the container. Fix with SACM23-XMI-029 (slice 0073b). |
| Slice 0073a | `release_metadata` in `crates/sacm/src/model/document.rs` | The hand mutant that empties its references loop survives: no public observation of the stale reference count was found after `SetContent::remove`. Find the observable and pin it, or triage it as equivalent. |
| Slice 0073a | `crates/sacm/src/validation/metadata.rs` | An `xmi:id` inside a Difference's `<target>` (XMI.xsd `processContents="skip"`) is still checked as an NCName and for duplicates, so a strict save refuses one schema-valid corner. |
| Slice 0073a | XMI writer | Kept fragments re-declare every in-scope prefix on each fragment. Lossless and byte-stable, but verbose on real MagicDraw files. |
