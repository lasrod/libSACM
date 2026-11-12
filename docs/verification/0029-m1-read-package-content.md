# Verification 0029 — Reading package content

- Date: 2026-11-13
- Row verdict: SACM23-XMI-016: PASS
- Row verdict: SACM23-XMI-010: PASS
- Row verdict: SACM23-XMI-001: PASS
- Row verdict: LIB-API-009: PASS
- Verifier: `sacm-verifier` (independent; read-only), 2 rounds; `sacm-docs-reviewer`, 1 round
- Commit: c6260e4. Evidence:
  - CI runs 43232325136 (978cd7f) and 43241720542 (c6260e4);
  - full mutants run 43241706014 (c6260e4): 956 caught, 0 missed, 0 timed out.
- Slice brief: [docs/slices/0029-m1-read-package-content.md](../slices/0029-m1-read-package-content.md)

## Result

`xmi::read` reads these containment children under their role elements:
- an AssuranceCasePackage in `assuranceCasePackage`;
- an ArgumentPackage in `argumentPackage` or `argumentElement`;
- a Claim in `argumentElement`.

The class comes from `xmi:type`/`xsi:type`, or from the role's concrete declared type. Each
child is read with its id, name and parent, in document order. A document built with
`edit::apply` writes, reads back equal, and writes the same bytes again.

Everything else on a child is `Unsupported` at its exact location, never re-roled or
dropped. White space written as CDATA or as character references is accepted. Every read
refusal is located at what it refuses. XMI-001 and LIB-API-009 were re-checked after their
wording and a test location changed. Every hand mutation of the role, type, order, id and
text checks is killed.

## Findings

| Severity | Row | Finding | Resolution |
|---|---|---|---|
| High | SACM23-XMI-016 | `check_text` refused schema-conforming white space written as CDATA or references | Accepted however written; tested in 8 slots (c6260e4) |
| Medium | SACM23-XMI-001 | The requirement still said roots hold only a name | Reworded (c6260e4) |
| Medium | SACM23-XMI-001 | The refusal order of a duplicate root id was unpinned | Two tests (c6260e4) |
| Medium | SACM23-XMI-016 | The refusal order of a duplicate child id was unpinned | One test with six cases (c6260e4) |
| Low | SACM23-XMI-010 | Text-location wording, and the location contract in the docs | Reworded; table rows; `ReadError` docs (c6260e4) |
| Low | SACM23-XMI-016 | A C++ difference was not named, and ADR 0016 D6 was miscited | Note added; ADR 0015 cited (c6260e4) |
