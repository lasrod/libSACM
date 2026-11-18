# Verification 0067 — Terminology and assurance case package interfaces and bindings

- Date: 2026-11-18
- Row verdict: SACM23-TERM-007: PASS
- Row verdict: SACM23-TERM-008: PASS
- Row verdict: SACM23-AC-002: PASS
- Row verdict: SACM23-AC-003: PASS
- Verifier: `sacm-verifier` (independent; read-only), one round
- Commit: 0bd2a7f. CI run [44282421924](https://github.com/lasrod/libSACM/actions/runs/44282421924):
  - every test job is green;
  - the PR-diff mutation shards report 0 missed and 0 timeouts, though they do not block (ADR 0006 amendment 6).
- Slice brief: [docs/slices/0067-term-ac-interfaces.md](../slices/0067-term-ac-interfaces.md)
- Decisions:
  - ADR 0039 D1;
  - ADR 0035 amendment 1 (accepted 2026-11-18);
  - the M5 decisions Q1–Q9.

## Result

- **The four kinds:** TerminologyPackageInterface, TerminologyPackageBinding, AssuranceCasePackageInterface and AssuranceCasePackageBinding.
  - They are read in both modes in every role and as roots, and written in the designed attribute order.
  - Their golden files round-trip byte for byte.
  - They are created, edited, copied and moved, and the LIB-EDIT-019 cascade holds on Terminology and AssuranceCase holders.
- **Every SACM 2.3 kind and end is read.** The unread-kind refusals, their 8 templates and `holds_root` are removed.
  - The verifier's probes show no data is lost. A vendor or unknown element as a root is `InvalidRoot` in both modes, and a root in another SACM namespace is `Unsupported`. So no preserved root can reach the writer.
  - The remaining refusals keep their English (`messages-0.1.0.txt`).
- **The standard's text is quoted correctly:** §9.2–9.4 and §10.4–10.6 of formal/23-05-08, and ptc/22-03-13. M5 spec defects 1 and 2 are confirmed.
- **The rewritten tests of verified rows keep their intent.** Each removed assertion sampled a kind or end that is now read; it is replaced by a positive read, or by an `xmi:uuid`, `xmi:Extension` or `xmi:Documentation` sample of the same location class.
- **The 23 rows reworded with a "Slice 0067" note are true.**
- **The two test changes the lead accepted in step 2 are sound.**

Evidence beyond CI: the builder's 20 hand mutants and the verifier's 16 are killed, except one that is equivalent (`first_owner` for `interface`).

## Findings

| Severity | Row | Finding | Resolution |
|---|---|---|---|
| non-blocking | TERM-007, -008, AC-002, -003 | Acceptance items pointed at the git-ignored design brief, or were only a test name. | Reworded with the verifier's text. |
| non-blocking | TERM-007, -008 | The inventory, D-BIND-PART and spec defect D1 misread §10.5 as having a Constraints section. | Corrected; D1 has a dated correction. |
| non-blocking | SACM23-XMI-001 | Acceptance 11 claimed more roots than the tests read. | Narrowed to the four package roots. |
| non-blocking | — | A follow-up row was outside its table; a CHANGELOG bullet mixed codes with a row id. | Fixed. |

## Logged

The atomic-batch set leaves out the Terminology creates and the four new ones. This is recorded in docs/follow-ups.md.
