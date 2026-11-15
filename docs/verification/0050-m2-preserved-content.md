# Verification 0050 — Preserved content, and the gated list of refusals

- Date: 2026-11-16
- Row verdict: LIB-COMPAT-003: PASS
- Row verdict: SACM23-XMI-008: PASS
- Row verdict: LIB-API-011: PASS
- Row verdict: LIB-TOOL-021: PASS
- Verifier: `sacm-verifier` (independent; read-only), three rounds; the third round was
  allowed by the maintainer after a second FAIL
- Commit: 04f799a; evidence:
  - CI run [43728674537](https://github.com/lasrod/libSACM/actions/runs/43728674537): 17/17
    green. PR-diff mutants: 297, of which 253 caught, 0 missed and 44 unviable.
  - Full mutants run [43728671061](https://github.com/lasrod/libSACM/actions/runs/43728671061):
    8 shards, 2,129 mutants, of which 1,784 caught, 0 missed, 0 timeout and 345 unviable.
- Slice brief: [docs/slices/0050-m2-preserved-content.md](../slices/0050-m2-preserved-content.md)

## Result

Content that does not conform is read and kept exactly as written, in both modes, with
SACM-XMI-003. It is written back byte-identically, refused by a strict save
(SACM-XMI-006), compared exactly, and kept out of the id space. Every remaining
`Unsupported` comes from one labelled list that is gated against the compliance points.
XMI.xsd is pinned.

Evidence beyond the CI runs:
- **Hand mutations:** 17 in round 3, every one killed, including a data-loss case where an
  unnamed holder dropped its reference elements.
- **Probes:**
  - about 60 input and edit sequences, each giving well-formed output that reads back
    equal;
  - a 200,000-deep fragment;
  - a read of 3,000 bindings × 20,000 Claims in 1.26 s.
- **Re-checks:** the 31 migrated test files and the 92 rewordings of 42 verified rows were
  re-checked and found true. Those rows stay verified on the clean full run.

## Findings

| Severity | Row | Finding | Resolution |
|---|---|---|---|
| blocking | LIB-COMPAT-003 | A tolerant write could emit an attribute twice after `end=""` plus an edit, or straight from a read. | An edit that leaves an end non-empty replaces the preserved attribute; the writer uses the element form (ADR 0033). |
| blocking | LIB-COMPAT-003, LIB-API-011 | `interface` (a conforming end) was preserved. | Refused through AC-002 / ARG-014. |
| blocking | all | MSRV broken by a let-chain; two messages with lost line continuations; misleading `XMI_UNKNOWN_CONTENT` advice. | Fixed and pinned. |
| blocking | LIB-COMPAT-003 | The Copilot binding fix made every read O(elements × bindings). | Reverted at the maintainer's request; the unbound-prefix case is in docs/follow-ups.md. |
| blocking | LIB-COMPAT-003 | Missed PR-diff mutants and surviving hand mutants in the edit and write paths. | Control tests; the dead release path removed; two equivalent mutants triaged. |
| follow-up | LIB-COMPAT-003 | ADR 0033 wording on holder bindings and on which edits replace. | Fixed. |
| follow-up | LIB-COMPAT-003 | No test of RemoveReference leaving the end non-empty beside a preserved attribute. | docs/follow-ups.md |
