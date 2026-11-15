---
slice: 0050
name: m2-preserved-content
state: in-progress
step: verification
rows: [LIB-COMPAT-003, SACM23-XMI-008, LIB-API-011, LIB-TOOL-021]
mode: pr
base: main
---

# Slice 0050 — Preserved content, and the gated list of refusals

## Scope

This slice closes M2:
- **LIB-COMPAT-003:** content that is not valid SACM where it stands is kept exactly as
  written, in both modes, with SACM-XMI-003.
- **SACM23-XMI-008:** a strict save refuses a document that holds such content
  (SACM-XMI-006).
- **LIB-API-011:** every remaining `Unsupported` comes from one gated list that names its
  row.
- **LIB-TOOL-021:** XMI.xsd is pinned.

Left out: the "discard preserved content" edit (LIB-EDIT-016, M6) and `xmi:Extension`
(SACM23-XMI-019, M6).

## Basis

XMI 2.5.1 §2.3.1, §7.3.3 and §7.8.3; XML 1.0 §3.1; formal/23-05-08 §8.12; D-PRESERVE,
D-STRICT and D-ROOT-NAME as amended. [ADR 0033](../adr/0033-preserved-content.md), accepted
by the maintainer on 2026-11-15.

## Design

ADR 0033, plus the architect's exact brief: the crate-private representation, the `Unread`
and `Refusal` table, the write check order, the Display texts, and a worked fixture with
locations and the expected output. The brief is condensed in the git-ignored
`target/slice-0050-design.md`.

Public API:
- `Code::{XMI_UNKNOWN_CONTENT, XMI_STRICT_SAVE_REFUSED, REF_PRESERVED_TARGET}`;
- `SacmElement::has_preserved_content`;
- `WriteError::PreservedContent { elements }`;
- `Difference::PreservedContent { id }`.

## Log

- 2026-11-15 choose: the last M2 rows. The research and design ran during slice 0049's CI
  waits (ADR 0006 amendment 4). The maintainer answered four questions and approved ADR
  0033.
- 2026-11-15 design: rows `analyzed`. SACM23-XMI-027 and LIB-COMPAT-004 were added to
  SACM23-CP-001..004, SACM23-XMI-019 was widened, and LIB-EDIT-016 (M6) was added.
- 2026-11-15 tests: three test engineers in parallel. preserved_content.rs and
  strict_save_preserved.rs (read, write, compare, the worked fixture, XMI-008);
  preserved_edits.rs, interim_refusals.rs (sacm and xtask) and the fetch pin; and the
  migration of 31 existing test files that used non-conforming content as their
  `Unsupported` samples (74 renamed tests). 105 new tests cited. The red is the missing API
  only. Rows `tests-failing`. About 40 verified rows need their wording changed; the
  matrix-steward drafts it while the implementer works.
- 2026-11-15 implementation: done (model/preserved.rs, xmi/preserve.rs, xmi/refusal.rs; the
  reader checkpoints each part and preserves it whole on a defect). The lead judged five
  disputed tests wrong and had them fixed: the catalog and serde code lists, the delete
  refusal order, and `xsi:nil` with content, now preserved whole per ADR 0033. The lead
  decided that a single delete reports a parent whose preserved child moves (LIB-EDIT-003's
  definition), with a test. 92 rewordings across 42 verified rows (matrix-steward draft)
  applied. The stale `WriteError::code` mutants exclusion was removed. Full gate green. Rows
  `implemented`.
- 2026-11-15 verification round 1: FAIL. Blocking: a duplicate attribute after `end=""`
  plus an edit (not well-formed); `interface` wrongly preserved (it is a conforming end);
  MSRV broken by a let-chain; two messages with lost line continuations; the
  XMI_UNKNOWN_CONTENT advice; and 6 missed PR-diff mutants. Fixes: an edit that sets an
  end replaces a preserved attribute of its name, and a write uses the element form when a
  read gives both (ADR 0033); `interface` is refused through AC-002 or ARG-014; tests pin
  the messages, the empty-end write and the mutants (one more equivalent mutant triaged).
- 2026-11-16 verification round 2: FAIL. The Copilot binding fix (e8c6f1d) made every read
  O(elements x bindings); 6 PR-diff and 8 hand mutants survived in the edit and write
  paths; one message was pinned on the wrong path. The loop stopped and asked. The
  maintainer chose to revert e8c6f1d (38ed1be; the unbound-prefix case went to
  follow-ups) and allowed a third round: six control tests, the ArgumentGroup message
  pinned, and the dead release path removed.
