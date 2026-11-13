# Follow-ups

Non-blocking findings from verification, docs review and Copilot (ADR 0006 amendment 3).
A docs slice clears them every few slices; add new ones at the end.

| Source | Where | Finding |
|---|---|---|
| Verification 0033 | `crates/sacm/tests/compare.rs:1617-1622` | A reader input labelled "the pinned form" still has `xmi:version`. |
| Verification 0034 | `crates/sacm/src/lib.rs` | "each returns a `Mutation`" follows the create operations; only `apply` returns one. |
| Verification 0034 | `crates/sacm/src/edit.rs` module docs | One line runs past the wrap width. |
| Verification 0032 | `crates/sacm/src/xmi/roots.rs:4, 488, 511`; `tests/xmi_names.rs`, `tests/xmi_assertion_declaration.rs`, `tests/xmi_write.rs` | Comments cite "ADR 0017 amendment 1/2" for rules now in ADR 0017's body or ADR 0021 (also on LIB-API-011). |
| Docs review 0034 | `sacm::edit` | Later API: `Operation::parent()`, an operation name on `EditError`, `CreateClaim::NAME`. |
| Docs review 0035 | `crates/sacm/src/lib.rs:127-129` | Add read-mode guidance: tolerant to open other tools' files, strict to have variations reported as Errors; strict save for a file you hand on. |
| Docs review 0035 | `crates/sacm/README.md:95-97`, `README.md:23-24`, `lib.rs:56-59` | Say that a later release may add validation rules, so a strict write may refuse a document it wrote before. |
| Docs review 0035 | `CHANGELOG.md` | Name `WriteError::Invalid`/`diagnostics()` as added; add the convention that a new validation Error rule is listed under **Changed** with its code. |
| Docs review 0035 | `crates/sacm/src/xmi.rs` (`Invalid` Display) | With one error, write "validation reports 1 error, SACM-MULT-001 on …" without "the first is". |
| Docs review 0035 | `crates/sacm/src/xmi/roots.rs:124` | The SACM-XMI-011 message should name XMI 2.5.1 §7.5.3, as SACM-XMI-010's names its spec. |
| Docs review 0035 | `crates/sacm/src/lib.rs:18-20`, `crates/sacm/README.md:32-34` | Simplify the `xmi::write` bullet; the README's "Planned" strict-save line reads as already true. |
| Docs review 0035 (later API) | `ReadOutput`, `Diagnostic` | `ReadOutput::has_errors()`; a way for `Diagnostic::clause()` to name its specification (SACM or XMI). |
| Implementer 0035 | `crates/sacm/src/xmi.rs` `Mode` doc | "the mode decides only the severity of SACM-XMI-010" should say "on a read"; the mode also decides whether `write` validates. |
