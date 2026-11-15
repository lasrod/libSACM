# Verification 0049 — Reading the C++ shapes, xsi:nil, and two validation rules

- Date: 2026-11-15
- Row verdict: LIB-COMPAT-008: PASS
- Row verdict: SACM23-XMI-026: PASS
- Row verdict: LIB-COMPAT-010: PASS
- Row verdict: SACM23-VAL-019: PASS
- Row verdict: SACM23-VAL-021: PASS
- Row verdict: LIB-QA-018: PASS
- Verifier: `sacm-verifier` (independent; read-only), two rounds
- Commit: c2b97e4; evidence:
  - CI run [43701911357](https://github.com/lasrod/libSACM/actions/runs/43701911357): 17/17
    green, every code job run. PR-diff mutants: 110 caught, 0 missed, 17 unviable.
  - Full mutants run [43699001243](https://github.com/lasrod/libSACM/actions/runs/43699001243)
    on b7be4e7, run because tests were replaced: 8 shards, 1,640 caught, 0 missed, 0
    timeout, 329 unviable.
- Slice brief: [docs/slices/0049-m2-reading-and-validation.md](../slices/0049-m2-reading-and-validation.md)

## Result

**LIB-COMPAT-008:**
- The C++ shapes read exactly as their standard form in tolerant mode and are refused as
  `LegacyShape` in strict mode.
- `RepeatedValue` comes first in both modes, and writing back gives the standard form.
- In round 2 the verifier re-applied all seven hand mutations from round 1, and each is now
  killed.

**SACM23-XMI-026:** `xsi:nil` reads as omitted, as XMI 2.5.1 §9.4.1 and §7.15 and XSD
cvc-elt require.

**SACM23-VAL-019 and SACM23-VAL-021:** both rules match §11.8 and the §8.6 OCL (spec defect
D15).

**The re-checked rows:** 22 verified rows had their tests or fixtures changed because the new
rules replace old refusals. Each still makes its point, and the full run is clean.

**LIB-QA-018:** the verifier passed bullets 1-3 and named five checks for bullet 4, which the
lead made on the docs-only push 1b84f84, run
[43704277285](https://github.com/lasrod/libSACM/actions/runs/43704277285):
- the push touched only `docs/` and sits on c2b97e4, whose CI is green;
- `test (windows-latest)` and `test (macos-latest)` skipped their 7 steps, and `MSRV` and the
  mutant shards were skipped;
- the `mutants (PR diff)` aggregator succeeded;
- `test (ubuntu-latest)` and every other job ran and succeeded, and all required checks are
  green;
- run 43701911357 is the code push that ran every job.

## Findings

| Severity | Row | Finding | Resolution |
|---|---|---|---|
| blocking | LIB-COMPAT-008 | Seven hand mutations (refusals and text joins) survived. | Controls C1-C7 in `legacy_shapes.rs`. |
| blocking | LIB-COMPAT-008 | PR-diff mutants missed 13 in new code. | 12 killed; the `legacy_shape` `<` → `<=` mutant is equivalent and triaged. |
| blocking | re-checks | Verified rows still claimed the replaced refusals. | SACM23-BASE-004/005/006, SACM23-XMI-012/014/016, SACM23-ARG-008/010 and LIB-COMPAT-001 reworded. |
| blocking (Copilot) | LIB-QA-018 | A docs-only push after a failed or cancelled code run skipped the code checks. | The `changes` job requires CI success on the previous commit. |
| follow-up | various | Empty CDATA under `xsi:nil`; MissingType advice for the idref form; `&#x20;` in a name; a previous run on another base vouches for PR-diff mutants. | docs/follow-ups.md |
