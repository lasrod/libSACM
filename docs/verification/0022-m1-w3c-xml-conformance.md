# Verification 0022 — W3C XML conformance: not-well-formed input is refused

- Date: 2026-11-11
- Row verdict: LIB-SEC-006: PASS
- Verifier: `sacm-verifier` (independent; read-only), 2 rounds
- Commit: 8aba433. Evidence:
  - CI runs 43036936788 (f79bc9f) and 43040320628 (8aba433);
  - full mutants runs 43031814423 (49ca4dd) and 43036941954 (f79bc9f): 0 missed, 0 timed out.
- Slice brief: [docs/slices/0022-m1-w3c-xml-conformance.md](../slices/0022-m1-w3c-xml-conformance.md)

## Result

`read` refuses all 1,017 applicable not-well-formed cases of the W3C XML Conformance Test
Suite 20130923. The breakdown is pinned: Doctype 719, Unsupported from the encoding steps 35,
and Malformed 263. The 71 guard cases are never Malformed. The verifier recounted these
independently. Hand mutations of the `xmlns:xmlns` check and of the fetch and extract code
are all killed. PR #30 merged before verification ran; the round 1 fixes are in PR #31.

## Findings

| Severity | Row | Finding | Resolution |
|---|---|---|---|
| Medium | LIB-SEC-006 | A case-insensitive check survived every test, although `xmlns:XMLNS` is conforming (Namespaces 1.0 §3) | Case-variant prefixes are pinned as read (f79bc9f) |
| Medium | LIB-SEC-006 | `ran 1017 cases` was captured and never appeared in CI logs (item 11) | The line is written to stderr; it appears in every test job (f79bc9f) |
| Low | LIB-SEC-006 | The tar-naming assertion matched the temp directory's name; the spawn-failure path was untested | `extract_with`; the test asserts the message starts with the command (f79bc9f, 8aba433) |
| Low | LIB-SEC-006 | A fresh download with the marker present was not shown to be extracted | Stale-marker test (f79bc9f) |
| Low | LIB-SEC-006 | The docs said leniencies outside the suite "stay guarded" by the sweep | Reworded in the docs, ADR 0005 B1 and the row (f79bc9f) |
