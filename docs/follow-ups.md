# Follow-ups

Non-blocking findings from verification, docs review and Copilot (ADR 0006 amendment 3).
A docs slice clears them every few slices; add new ones at the end.

| Source | Where | Finding |
|---|---|---|
| Slice 0067 | `crates/sacm/tests/edit_atomic.rs` | The atomic-batch set (LIB-EDIT-002) does not create Terminology kinds (slice 0062) or the four Terminology and AssuranceCase interface and binding kinds (slice 0067); `create_ends` already has their arms. Add them to `classify`, `probe_index` and `OPERATION_NAMES`. |

Cleared at M4's close (slice 0069): every earlier item was fixed, decided, or moved to a matrix row (LIB-API-019); see `docs/verification/0069-m4-close.md`.
