# Follow-ups

Non-blocking findings from verification, docs review and Copilot (ADR 0006 amendment 3).
A docs slice clears them every few slices; add new ones at the end.

| Source | Where | Finding |
|---|---|---|
| Docs review 0034 | `sacm::edit` | Later API: `Operation::parent()`, an operation name on `EditError`, `CreateClaim::NAME`. |
| Docs review 0035 (later API) | `ReadOutput`, `Diagnostic` | `ReadOutput::has_errors()`; a way for `Diagnostic::clause()` to name its specification (SACM or XMI). |
| Verification 0035 | LIB-COMPAT-007 | "Warnings and Infos never refuse" is untestable until `validate` emits a Warning; the first such rule adds the test (it must kill removing the Error-only filter). |
| Verification 0035 | PR-diff mutants | nextest skips doctests, so a mutant pinned only by a doctest shows as missed; pin such behaviour in a test too. |
