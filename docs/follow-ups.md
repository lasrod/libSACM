# Follow-ups

Non-blocking findings from verification, docs review and Copilot (ADR 0006 amendment 3).
A docs slice clears them every few slices; add new ones at the end.

| Source | Where | Finding |
|---|---|---|
| Verification 0061 | `crates/sacm/src/model/document.rs` (`Document` docs) | The prose says tests build documents through the `test-support` feature, whose items docs.rs does not show. Reword it for the next release. |

Cleared at M3's close (slice 0060): every earlier item was fixed, decided, or moved to the notes of the planned row it belongs to (LIB-COMPAT-004, SACM23-XMI-027); see `docs/verification/0060-m3-close.md`.
