---
slice: 0074a
name: schema-checks
state: in-progress
step: verification
rows: [LIB-TOOL-004, LIB-TOOL-023]
mode: pr
base: main (6ca9884)
---

# Slice 0074a — Schema and metamodel checks

## Scope

The evidence for the M6 compliance claim, in xtask only, with no library change.

- **LIB-TOOL-004.** An xtask test reads the pinned ptc/22-03-13 and fails if the crate or
  the hand-kept metamodel inventory drifts from it.
- **LIB-TOOL-023.** `cargo xtask schema` generates the SACM XML Schema (ADR 0043), and
  `cargo xtask schema validate` checks every strict golden file and strict output against it
  with xmllint. A new, required CI job runs it.

The claim itself (SACM23-CP-001 to -004, the conformance statement and the README) is slice
0074b. This slice also changes the conformance statement's "before" wording and adds spec
defects D29 to D32.

## Basis

- SACM 2.3 clause 2 (§2.2 to §2.5).
- XMI 2.5.1 §2.2.1, §7.2, §7.8, clause 8, production 2a.
- M6 decisions Q1 and Q10 (the maintainer, 2026-11-19); Q-A and Q-B of the slice 0074 design
  (the maintainer, 2026-11-22).

## Design

- ADR 0043 (accepted); D-SCHEMA; D-STRICT amended.
- The design brief, `target/slice-0074-design.md` (git-ignored), sections 3, 4, 5 ("before"
  wording), the reference generator in its appendix, and the defects list (renumbered D29 to
  D32).
- The xtask API (`xtask::metamodel`, `xtask::schema`) is unpublished; its shape is in section
  3 of the design brief.

## Log

- 2026-11-22 design: ADR 0043 is filed as accepted, with D-SCHEMA and D-STRICT amended.
  LIB-TOOL-004 is reworded and LIB-TOOL-023 is new; both are analyzed. The lead added the
  `SACM XML Schema` job to `ci.yml` (it becomes a required check once it is green on `main`).
- 2026-11-22 tests: the builder wrote 11 tests for LIB-TOOL-004 and 13 for LIB-TOOL-023, and
  the fixture `xmi-metadata-schema-valid.sacm.xmi` (its four `(none)` lines added to
  `messages-0.1.0.txt`). The lead confirmed the red: ungated, only the run-dispatch test fails
  (`schema` is unrecognized); gated (`--cfg slice_0074a_api`), only the two missing modules.
  Snapshot red-0074a.tar.
  - The lead's rulings on the builder's interpretations, all accepted:
    - `Property.association` names the Association, or `""` for an unnamed one, for every
      member end;
    - a Data property's `type_name` is the href fragment;
    - the refusal names the relative path, and the model is checked before XMI.xsd;
    - paths display through `join(..).display()`;
    - the conflict error uses Debug kind names, in candidate order;
    - only `cmd_validate` prints the `xmllint:` version line;
    - `judge` with no status is checked by prefix and suffix.
  - The sample schema is 96 lines: the pinned hash wins over the brief's "97".
- 2026-11-22 implementation: the builder found a step-1 test helper wrong. `complex_type` in
  `xtask/tests/schema.rs` stopped at the first nested `</xsd:complexType>`, which is inside a
  reference element. Tests 3 and 4 then saw only part of a type. The lead authorized changing
  exactly that helper, to count depth; no assertion changes.
  - The lead's rulings on the builder's other choices:
    - the stricter `METADATA_ONLY` match, `judge` returning `Err` when xmllint exits 3 or 4
      with no validity line, the extra `invalid` and `error` report lines, and the extra model
      refusals are accepted;
    - the version line is `xmllint --version`'s first line, as given.
  - Local `cargo xtask schema validate`: 23 goldens and 65 strict outputs, of which 85 are
    valid and 3 are invalid in XMI metadata only (the two listed fixtures and one output
    derived from them). The 0073b cross-document-links golden and its outputs are valid. A
    stray attribute on a golden fails, so the check is not vacuous.
  - Of 48 hand mutations, 5 survived. The lead had the four testable ones closed with
    assertions in the cited tests (no new tests). The fifth, file order, is invisible on NTFS;
    the Linux CI jobs run that test.
  - The lead corrected the inventory's LIB-COMPAT-004 bullet to ADR 0041 D4: SACM 2.4 is
    refused in both modes, and a SACM-like namespace counts only on a package-kind root.
- 2026-11-22 verify round 1 (56ae885): LIB-TOOL-004 PASS; LIB-TOOL-023 FAIL. No test had
  exactly one invalid file, so the mutant `invalid_files > 0` → `> 1` survived. CI's
  `SACM XML Schema` job passed: 85 valid, 3 invalid in XMI metadata only.
  - Fix round:
    - the builder adds the verifier's one-invalid-file block, the `Y.sacm.xmi` file-order
      case, two assertion messages, the `run` rustdoc, and triage rows for the defensive
      guards;
    - the lead fixes the acceptance wording and ADR 0043 S7 ("and their strict outputs"),
      makes the CI loop log each input that gives no strict output, and logs a follow-up:
      the PR-diff mutants job has no spec.
  - The maintainer approved adding `SACM XML Schema` to main's required checks
    (2026-11-22). The lead added it.
