# Follow-ups

Non-blocking findings from verification, docs review and Copilot (ADR 0006 amendment 3).
A docs slice clears them every few slices; add new ones at the end.

| Source | Where | Finding |
|---|---|---|
| Slice 0067 | `crates/sacm/tests/edit_atomic.rs` | The atomic-batch set (LIB-EDIT-002) does not create Terminology kinds (slice 0062) or the four Terminology and AssuranceCase interface and binding kinds (slice 0067); `create_ends` already has their arms. Add them to `classify`, `probe_index` and `OPERATION_NAMES`. |

Cleared at M4's close (slice 0069): every earlier item was fixed, decided, or moved to a matrix row (LIB-API-019); see `docs/verification/0069-m4-close.md`.
| Slice 0068 | `invalid/reference-cross-document-href.sacm.xmi`, `invalid/modularity-findings.sacm.xmi` | Both read cleanly since slice 0068 (the second with `validate` findings only), but sit under `invalid/`, so the LIB-RT-002 round-trip walk skips them. Move them out, with their `messages-0.1.0.txt` lines, in a later slice. |
| Verification 0068 | the local gate (CLAUDE.md) | It has no MSRV step, so a let-chain was caught only by CI. The lead's gate script now runs `cargo +1.85 check -p sacm --all-features --all-targets`; consider adding it to the documented gate. |
