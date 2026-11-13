# Follow-ups

Non-blocking findings from verification, docs review and Copilot (ADR 0006 amendment 3).
A docs slice clears them every few slices; add new ones at the end.

| Source | Where | Finding |
|---|---|---|
| Docs review 0034 | `sacm::edit` | Later API: `Operation::parent()`, an operation name on `EditError`, `CreateClaim::NAME`. |
| Docs review 0035 (later API) | `ReadOutput`, `Diagnostic` | `ReadOutput::has_errors()`; a way for `Diagnostic::clause()` to name its specification (SACM or XMI). |
| Verification 0035 | LIB-COMPAT-007 | "Warnings and Infos never refuse" is untestable until `validate` emits a Warning; the first such rule adds the test (it must kill removing the Error-only filter). |
| Verification 0035 | PR-diff mutants | nextest skips doctests, so a mutant pinned only by a doctest shows as missed; pin such behaviour in a test too. |
| Verification 0037 | `crates/sacm/tests/xmi_value_elements.rs` | Pin that a malformed later occurrence is still listed in `RepeatedValue.locations` (a filter mutation in `repeated_value` survives): add `<name content="A"><content>A</content><content x="1">B</content></name>` expecting 3 locations. |
| Verification 0037 | `crates/sacm/tests/w3c_xml_conformance.rs:458` | The 15 s wall-clock budget fails on a loaded machine (51-120 s seen, 0.39 s alone); loosen it or use a per-read deadline. |
| Audit M1 | `xmi::write`, LIB-SEC-003 (M6) | Write output grows with the square of nesting depth (two spaces of indentation per level): a 20,000-deep chain (about 2 MB of input) writes 1.2 GB, and a 100,000-deep chain aborts on a 40.8 GB allocation. LIB-SEC-003's requirement now covers a write bound. |
| Verification 0038 | `crates/sacm/tests/xmi_read.rs` (LIB-SEC-002 deep chain) | Its 15 s wall-clock deadline on about 4 s of debug work can fail under load; use the deadline only to detect a hang (about 60 s). |
| Verification 0038 | SACM23-XMI-004 | Add a strict-mode case to the U+0001 id test (`WriteError::Invalid`, SACM-ID-002); today only VAL-020 and LIB-COMPAT-007 tests support the strict half of the row. |
| Verification 0038 | LIB-API-004 | Acceptance items 3-4 ("Every variant's Display ...") should name the error types this row's tests cover. |
| Verification 0038 | CHANGELOG, docs/architecture.md | CHANGELOG says "one `#[non_exhaustive]` error enum per operation" (`ParseAssertionDeclarationError` is a struct); architecture.md credits the NCName rule to ADR 0018 (its basis is ADR 0015 amendment 2 and ADR 0019 E6). |
| Verification 0038 | `crates/sacm/tests/errors.rs:4-8` | The module doc says `InsertError` is the only public error type; the others are checked in their own rows' files (see LIB-API-004 item 1). |
