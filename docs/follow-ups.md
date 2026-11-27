# Follow-ups

Non-blocking findings from verification, docs review and Copilot (ADR 0006 amendment 3).
A docs slice clears them every few slices; add new ones at the end.

Cleared at M5's close (slice 0071): every earlier item was done, decided, or moved to a matrix row (LIB-EDIT-020); see `docs/verification/0071-m5-close.md`. Cleared in part at M6's close (slice 0080): see `docs/verification/0080-m6-close.md`.

| Source | Where | Finding |
|---|---|---|
| Slice 0073a | XMI writer | Kept fragments re-declare every in-scope prefix on each fragment. Lossless and byte-stable, but verbose on real MagicDraw files. |
| Slice 0073a | `xmi::write` "# Output size" | With `max_depth` raised far above the default by the caller, the documented bound `(2·D + 48)·B` allows a very large write (a 200,000-root proxy chain under depth 1,000,000 aborts on a 20 GB allocation). The docs could warn that the bound is quadratic for a raised depth. |
| Slice 0073b (LIB-EDIT-002) | `crates/sacm/tests/edit_atomic.rs` | The randomized atomicity test in `crates/sacm/tests/edit_atomic.rs` does not cover the href forms of the reference edits and creates; their atomicity is covered only by the `refuse` helpers in `crates/sacm/tests/edit_hrefs.rs`. |
| Slice 0074a (LIB-QA-007) | `.github/workflows/ci.yml`, `mutants-shard` | The PR-diff mutants job does not run `cargo xtask fetch-spec`, so the spec-dependent xtask tests (metamodel, schema) skip there, and mutants killed only by them show as missed. Add `cargo xtask fetch-spec` (with the OMG references cache) to that job, or triage them. |
| Slice 0074b (SACM23-CP-001 to -004) | `crates/sacm/tests/data/sacm23/cp/`, `xtask/tests/compliance.rs` | The unit documents hold no proxy (a composition role in reference form), so the walk's link branch is never reached (walk mutation W2 survives). Proxies are covered by the SACM23-XMI-029 and -030 tests. A unit document could add one cross-file containment proxy. |
| Slice 0075a (LIB-COMPAT-002) | `crates/sacm/src/xmi/roots.rs` | Three hand mutants are equivalent, so the code they mutate cannot matter: `found.preserved_at.insert` for the document holder (about line 2520) and for a kept proxy (about line 3710), and the `is_expression` gate of `path_at` (about line 6105). Drop that code, or say in a comment why it cannot matter. |
| Slice 0075a (LIB-COMPAT-002) | template `SACM-XMI-001/xmi-namespace-root` | It says "is in the XMI namespace" also for EMF's namespace `http://www.omg.org/XMI`. |
| Slice 0075a (LIB-API-017) | `crates/sacm/tests/message_templates.rs` | The test `lib_api_017_template_rs_declares_the_100_refusal_templates` now asserts 102; rename it without the number (and its matrix citation). |
| Slice 0075a (LIB-COMPAT-002) | `crates/sacm/src/xmi/roots.rs`, open content | Under the tolerant alias an EMF-namespace `Extension` opens its content, but an EMF-namespace Difference (`xmi:Difference`, `Add`, `Replace`, `Delete` in `http://www.omg.org/XMI`) does not open its `target`. No corpus file has one; decide when a file needs it. |
| Slice 0075b verification (LIB-COMPAT-017) | `ReadOptions::with_subclass` | It accepts a `name` that is not an NCName (`""`, `"a:b"`), which can match an equally odd type value; it writes back unchanged, so nothing is lost. |
| lasrod/libSACM#113 (Files and modes) | `xmi::read`, tolerant mode | If Assurance Forge starts reading tolerantly (SACM 2.2 or EMF files), it wants a summary of what the tolerant read changed or kept, to tell the user before a strict save. AF reads strictly today; decide when it does not. |
| Slice 0083 verification (LIB-DIAG-006) | `diagnostic/catalog.json`, docs/sacm/diagnostics.md | The `row` argument of SACM-XMI-002/other-namespace, SACM-XMI-008/other-revision-root and SACM-XMI-015's two namespace templates is not used in their English; it labels the refusal's frame. A translator needs a note on it. |
| Slice 0084 verification (LIB-API-005) | `impl Iterator` returns (`Document::roots`, …) | Their auto traits (`Send`, `Sync`) are public through leakage; the Send/Sync test covers named types only. |
| Slice 0085 verification (LIB-DOC-009) | `xtask/tests/docs_style.rs` | The four-line check can be passed with a wall of text in a block quote, an unindented lazy continuation that splits one rendered item, or a `<br>` that splits a paragraph; long table cells are exempt by design. The verifier reads for these. |
| Slice 0085 verification (release) | README.md, crates/sacm/README.md, crates/sacm-cli/README.md | `sacm = "0.4"` and `sacm 0.4.0` need the bump at the 0.5.0 release. |
| Slice 0086 (LIB-DOC-011) | private module docs (`xmi/input.rs` and others) | `cargo doc --document-private-items` reports unresolved links (`[23]`, `[32]`, XML production numbers) and redundant link targets; the gate builds public docs only. |
| Slice 0086 (LIB-DOC-015) | `xmi::read`, `xmi::write` item docs | They now hold the detail the `xmi` module links to and are very long; keep those facts when LIB-DOC-015 shortens them. |
| Slice 0086 fix round (LIB-TOOL-004) | `xtask/tests/metamodel.rs`, `assert_inventory_wording` | It matches a sentence of the inventory's Provenance callout on one source line, so rewrapping the same words breaks it; compare with line breaks normalised. |
| Slice 0087 fix round (LIB-DOC-012) | `xtask/tests/docs_style.rs` | The docs of methods declared inside a `pub trait` (rustdoc shows them) are not checked; the reader counts trait impls, not trait definitions. |
| Slice 0087 verification (LIB-DOC-012) | `xtask/tests/docs_style.rs`, `impl_blocks` | No control backs "the innermost `impl` block decides" (swapping to the outermost survives); add an inherent `impl` nested in a trait impl's `fn` body whose private `fn` must not count. |
| Slice 0088 verification | `cargo xtask` help | It says fetch-spec downloads "the pinned OMG SACM 2.3 and XMI 2.5.1 references"; it also fetches XML Schema Parts 1 and 2 and the W3C XML conformance suite. |
| Slice 0088 verification (LIB-EDIT-025) | `unrepresentable_text.rs`, the guarantee test | Its random sequences delete only with `DeleteReferencingRelationships`; `ScrubReferences` is not exercised there (LIB-EDIT-019 keeps its bounds). |
| Slice 0089 verification (LIB-DOC-002) | `xtask/tests/docs_style.rs` | The fallible-function finder has no control for a `pub const fn` (mutant M10 survives); no such function exists today. |
