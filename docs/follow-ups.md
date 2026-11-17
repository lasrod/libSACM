# Follow-ups

Non-blocking findings from verification, docs review and Copilot (ADR 0006 amendment 3).
A docs slice clears them every few slices; add new ones at the end.

| Source | Where | Finding |
|---|---|---|
| Verification 0061 | `crates/sacm/src/model/document.rs` (`Document` docs) | The prose says tests build documents through the `test-support` feature, whose items docs.rs does not show. Reword it for the next release. |

Cleared at M3's close (slice 0060): every earlier item was fixed, decided, or moved to the notes of the planned row it belongs to (LIB-COMPAT-004, SACM23-XMI-027); see `docs/verification/0060-m3-close.md`.
| Verification 0063 | `xmi/refusal.rs` `unsupported_at` | It takes its code from the template and its row from `unread` without checking they agree; a `debug_assert_eq!(unread.code(), details.code())` would catch a site that pairs them wrongly. |
| Verification 0063 | ADR 0030 amendment 3, R11 | Question for the maintainer: should R11 also refuse a record whose `reason`, `shape` or `element_name` disagrees with its rendered arguments, rather than keep it as given? |
| Verification 0063 | `docs/quality/mutants-triage.md` | The `WriteError::code` row describes the 0.1 signature (`-> Option<Code>`); update it when the file is next touched. |
| Verification 0065 | `model/lang_string.rs` JSON | A JSON array LangString (`["en","x"]`), which 0.1's derived `Deserialize` accepted, is now refused. It is outside ADR 0030 R6's object contract; record it under Changed if 0.2.0's notes should say so. |
| Verification 0065 | `xmi/roots.rs` `reference_tokens` | A preserved `expression="claim_1"` attribute reserves `claim_1` from id generation, and an unknown vendor attribute does not, because `reference_tokens` counts every `ReferenceEnd::ALL` name. BASE-009 acceptance 6's "as any unknown attribute is" holds for diagnostics only. |
