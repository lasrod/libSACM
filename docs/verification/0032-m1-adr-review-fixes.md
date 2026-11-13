# Verification 0032 — ADR review fixes

- Date: 2026-11-13
- Row verdict: LIB-DOC-008: PASS
- Verifier: `sacm-verifier` (independent; read-only), 2 rounds
- Commit: 943780e. Evidence: CI run 43353447065 (7612a50), and the gates run by the verifier
  on an export of 943780e. Docs and test changes only, so there is nothing to mutate.
- Slice brief: [docs/slices/0032-m1-adr-review-fixes.md](../slices/0032-m1-adr-review-fixes.md)

## Result

The ADRs describe the current design:
- ADR 0017 is rewritten as the current state plus its history;
- ADR 0021 records the reader's content decisions;
- ADRs 0010, 0013 and 0015 are amended;
- the index lists every ADR;
- decisions.md points to each interpretation an ADR makes.

The review's behaviour findings are new rows, linked to the compliance points. Two tests
check the index and the links. Every quote was checked against XMI 2.5.1 and SACM 2.3.

## Findings

| Severity | Row | Finding | Resolution |
|---|---|---|---|
| Medium | LIB-DOC-008 | ADR 0015 called a missing `xmi:id` (optional by §7.6.1) an input defect, and claimed every refusal says its kind | Limits and defects separated; only the literal refusal is labelled today (943780e) |
| Medium | LIB-DOC-008 | ADR 0021's interim list mixed in non-conforming input and named no row for some items | Two lists: conforming input with closing rows, and non-conforming input (943780e) |
| Medium | LIB-DOC-008 | The ADR 0017 rewrite lost three C++ differences | Restored in ADR 0021 |
| Medium | LIB-DOC-008 | ADR 0011 claimed a test of every catalog constant | Reworded to the per-row checks that exist |
| Medium | LIB-DOC-008 | No compliance point depended on SACM23-XMI-024 or SACM23-VAL-020 | CP-002, CP-003 and CP-004 depend on both |
| Medium | LIB-DOC-008 | Three ADR interpretations had no D- entry | D-XMI-VERSION, D-ROLE-UNQUALIFIED, D-ROOT-NAME |
| Low | LIB-DOC-008 | ADR 0017 wrapper, `#[non_exhaustive]`, rule 1a; row wording | Fixed |
| Logged | LIB-API-011 | Code and test comments cite the old ADR 0017 amendments | Recorded on LIB-API-011 |
| Logged | LIB-DOC-008 | The index test compares the first word of a "superseded by NNNN" status | To widen when the first ADR is superseded |
