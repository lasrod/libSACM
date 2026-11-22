# Verification 0074a — Schema and metamodel checks

- Date: 2026-11-22
- Row verdict: LIB-TOOL-004: PASS
- Row verdict: LIB-TOOL-023: PASS
- Verifier: `sacm-verifier` (independent, read-only), two rounds.
  - **Round 1, 56ae885:** PASS for LIB-TOOL-004. FAIL for LIB-TOOL-023, on one test gap.
  - **Round 2, b1db295:** PASS for LIB-TOOL-023; LIB-TOOL-004's PASS holds.

  CI run [45021356526](https://github.com/lasrod/libSACM/actions/runs/45021356526) is on round 2. Its `SACM XML Schema` job is now a required check on `main` (the maintainer, 2026-11-22).
- Slice brief: [docs/slices/0074a-schema-checks.md](../slices/0074a-schema-checks.md)
- Decisions:
  - [ADR 0043](../adr/0043-the-sacm-xml-schema.md), D-SCHEMA and the D-STRICT amendment;
  - M6 decisions Q1 and Q10;
  - Q-A and Q-B of the slice 0074 design (the maintainer, 2026-11-22).

## Result

- **LIB-TOOL-004, metamodel coverage.** `xtask::metamodel` reads the pinned ptc/22-03-13 and applies the five recorded corrections. The tests show that the crate agrees with the model:
  - `ElementKind` (38), `AbstractClass` (13), is-a, `may_contain` (339 of 1,444 pairs), `ReferenceEnd` and its bounds, and `Flag`;
  - the hand-kept inventory;
  - a strict read and write of every attribute and value composition.
- **LIB-TOOL-023, the SACM XML Schema.** `cargo xtask schema` generates the schema deterministically, to the pinned SHA-256. `cargo xtask schema validate` runs xmllint on every strict golden file and every strict output.
  - Locally and in CI: 23 goldens and 65 outputs, 85 valid, 3 invalid in XMI metadata only (the two listed fixtures and the golden's strict output, ADR 0043 S7).
  - Every input without an output is a refused strict save or an unreadable input.
  - The schema rejects a stray attribute and a bad enumeration literal. As S8 states, it does not check IDREF targets.
- **How it was shown:**
  - the cited tests, run with the spec present;
  - the CI job's log;
  - the verifier's 49 hand mutations, with the defensive guards that are unreachable on the pinned model triaged as equivalent;
  - the D29 to D32 quotations, checked against the PDFs.

## Findings

| Blocking / non-blocking | Row | Finding | Resolution |
|---|---|---|---|
| Blocking | LIB-TOOL-023 | No test had exactly one invalid file (`invalid_files > 0` → `> 1` survived). | The verifier's block, in the cited test (round 1). |
| Non-blocking | LIB-TOOL-023 | The file-order mutant survived on NTFS. | An upper-case name in the order test (round 1). |
| Non-blocking | LIB-TOOL-023 | The job was not yet a required check. | Added with the maintainer's approval. |
| Non-blocking | both | Acceptance, ADR 0043 S7, assertion and rustdoc wording; untested defensive guards. | Fixed with the verifier's text; 11 triage rows. |
| Non-blocking | LIB-QA-007 | The PR-diff mutants job has no spec. | `docs/follow-ups.md`. |
| Non-blocking | both | The 11 "equivalent" triage rows can be told apart through the `pub` xtask API (round 2). | `docs/follow-ups.md`, with the distinguishing inputs; two reasons reworded. |
