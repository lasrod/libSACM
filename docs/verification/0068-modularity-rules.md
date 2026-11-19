# Verification 0068 — Modularity rules and cross-document references

- Date: 2026-11-19
- Row verdict: SACM23-VAL-013: PASS
- Row verdict: SACM23-VAL-014: PASS
- Row verdict: SACM23-VAL-015: PASS
- Row verdict: SACM23-VAL-023: PASS
- Row verdict: SACM23-XMI-027: PASS
- Verifier: `sacm-verifier` (independent; read-only), two rounds.
- Commits:
  - 903c05a, round 1: FAIL.
    - B1, all rows: a let-chain broke MSRV 1.85.
    - B2–B7: untested branches in XMI-027 and VAL-023.
  - babc873, round 2: PASS. In CI run [44468462030](https://github.com/lasrod/libSACM/actions/runs/44468462030), MSRV and every test job are green. Its PR-diff mutants do not block (ADR 0006 amendment 6).
- Slice brief: [docs/slices/0068-modularity-rules.md](../slices/0068-modularity-rules.md)
- Decisions:
  - ADR 0040 (accepted 2026-11-19);
  - the M5 decisions Q2–Q8;
  - D-IFACE-CITE, D-BIND-PART, D-SEV, D-PKG-HOMOG, D-PRESERVE and D-ORDER, as updated;
  - spec defects D22–D27.

## Result

- **VAL-013:**
  - Non-citation content in an ARG, ART or TERM interface, or in a TERM binding, is a SACM-PKG-001 Error.
  - ARG and ART bindings may hold their family's relationships.
  - An interface citation of the wrong kind, or one outside the implemented package, is a Warning.
  - ACP interfaces and bindings have no citation rule.
- **VAL-014:**
  - An interface with no `implements` is a SACM-MULT-001 Error.
  - So is a binding with fewer than two distinct participants, counting ids and `href`s.
  - A binding named as a participant is a Warning.
- **VAL-015:** an AssuranceCasePackageInterface's sub-packages follow the §9.3 OCL.
- **VAL-023:** an interface whose `isCitation` is not true, outside the package it implements at any depth, is a SACM-PKG-002 Warning.
- **XMI-027:** an `href` with a document part, on every element reference end, has this behaviour:
  - it is read in both modes and held as `model::Href` (`SacmElement::hrefs`);
  - it is written in element form, ids first;
  - it is compared (`Difference::Hrefs`);
  - it is counted in bounds as a distinct value in edits, copy and the LIB-EDIT-019 delete;
  - it is never a SACM-REF finding, and another file is never resolved.

  Proxies, `expression` hrefs, XPointer and XLink stay Interim under SACM23-XMI-029 (M6).
- **The standard:** the quotations of SACM 2.3 and XMI 2.5.1 are checked against the sources.
- **Changed existing tests:** the tests changed after the red snapshot keep their intent, and the 0066 and 0067 interface tests became stricter.
- **Reworded verified rows:** each is true.

Evidence beyond CI:
- the builder's 44 hand mutants plus the 7 of the fix round;
- the verifier's 50, every one killed after round 2.

## Findings

| Severity | Row | Finding | Resolution |
|---|---|---|---|
| blocking | all | A let-chain broke the 1.85 build. | Rewritten. The lead's gate now runs `cargo +1.85 check`. |
| blocking | XMI-027 | `href` de-duplication in delete, edits and copy; the `Hrefs` Display; `repeated_hrefs`; `set_hrefs`'s guards were all untested. | Cases and a unit test added. |
| blocking | VAL-023 | The `isCitation` boundary was untested. | A case for each family. |
| non-blocking | all | The XMI-027 requirement, the SACM-XMI-005 row, both READMEs, the "no citation" wording, the `depends_on` lists, two acceptance items, two titles, LIB-EDIT-009, VAL-012 and an edit_atomic comment. | Fixed with the verifier's text. |

## Logged

Two items in docs/follow-ups.md:
- the two `invalid/` fixtures that now read;
- an MSRV step for the documented gate.
