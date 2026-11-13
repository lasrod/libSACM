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
