# Verification 0051 — Closing M2

- Date: 2026-11-16
- Row verdict: LIB-COMPAT-010: PASS
- Row verdict: LIB-EDIT-012: PASS
- Row verdict: SACM23-XMI-005: PASS
- Row verdict: SACM23-XMI-026: PASS
- Row verdict: LIB-QA-018: PASS
- Row verdict: LIB-API-011: PASS
- Row verdict: LIB-COMPAT-003: PASS
- Row verdict: LIB-EDIT-005: PASS
- Verifier: `sacm-verifier` (independent; read-only), three rounds. The third round was
  allowed by the maintainer after a second FAIL.
- Commit: 3037da3; evidence:
  - CI run [43780966734](https://github.com/lasrod/libSACM/actions/runs/43780966734): every
    check green on Linux, Windows and macOS. PR-diff mutants: 55, of which 48 caught,
    0 missed and 7 unviable.
  - Full mutants run [43772198287](https://github.com/lasrod/libSACM/actions/runs/43772198287)
    on d115c01: 2,160 mutants, of which 1,812 caught, 0 timeout and 346 unviable. Two were
    missed: `token_starts` (lone CR) and `Document::compact`. Both are killed by tests added
    in round 1, as the PR-diff runs on af6193a and 3037da3 show.
- Slice brief: [docs/slices/0051-m2-close.md](../slices/0051-m2-close.md)

## Result

The rows whose behaviour this slice changed were re-verified.
- **LIB-COMPAT-010, LIB-API-011:** role elements in reference form are refused as proxies
  under SACM23-XMI-019 (`xmi:idref`) or SACM23-XMI-027 (an unqualified `href`). A qualified
  `xlink:href` or `xmi:href` stays preserved content or `MissingType`.
- **SACM23-XMI-005:** `RepeatedValue` locates each id.
- **LIB-EDIT-012:** `set_text` is n log n.
- **SACM23-XMI-026:** an empty CDATA under `xsi:nil` counts as no content.
- **LIB-QA-018:** the docs-only check matches the event type.
- **LIB-COMPAT-003, LIB-EDIT-005:** ids inside preserved content are not references for a
  delete.

The planned rows LIB-COMPAT-004, SACM23-XMI-019 and SACM23-XMI-027 were checked for text
only and stay planned. LIB-COMPAT-004's relabel as a tolerance is now recorded in D-STRICT
and D-NS-PIN.

Evidence beyond CI:
- hand mutants on every new branch, all killed in the final state;
- probes of every proxy shape in both modes;
- a spot check of 18 of the cleared follow-ups;
- a re-check of the audit's rewordings.

## Findings

| Severity | Row | Finding | Resolution |
|---|---|---|---|
| blocking | SACM23-XMI-005 | Missed mutant: a lone CR in `token_starts`. | Lone-CR case added (round 1). |
| blocking | LIB-COMPAT-003 | Missed mutant in `Document::compact`: a name id became REF-003 after a delete. | Test added (round 1). |
| blocking | SACM23-XMI-005, LIB-COMPAT-010 | Matrix and catalog text stale after the change. | Reworded (round 1). |
| blocking | LIB-API-011 | A typed or concrete-role `href` proxy named XMI-019, against the decision. | It now names XMI-027 (round 1). |
| blocking | LIB-COMPAT-004 | The relabel conflicted with D-STRICT and D-NS-PIN; the label called the input a defect. | Decisions amended; label `(strict mode never reads it; …)` (round 1). |
| blocking | LIB-EDIT-005 | The preserved-content sentence was untested. | Test added (round 1). |
| blocking | LIB-API-011, LIB-COMPAT-010 | A qualified `href` treated as unqualified survived the suite. | Two tests added (round 3). |
| blocking | LIB-API-011 | Acceptance [2] still listed LIB-COMPAT-004 as a compliance-point dependency. | Reworded (round 3). |
| follow-up | — | Seven wording, naming and CI-scope items. | `docs/follow-ups.md`. |
