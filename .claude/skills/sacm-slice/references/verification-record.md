# Verification NNNN — <slice name>

- Date: YYYY-MM-DD
- Row verdict: ROW-ID: PASS
- Verifier: `sacm-verifier` (independent; read-only)
- Commit: <sha>; evidence: <CI and mutants run IDs>
- Slice brief: [docs/slices/NNNN-name.md](../slices/NNNN-name.md)

<!-- One `- Row verdict: <ROW-ID>: PASS|FAIL|OPEN` line per row judged, at column 0,
     outside code blocks. The matrix gate needs exactly one PASS line for each verified
     row that cites this record. -->

## Result

One or two sentences: what was shown, and how (tests, mutants report, hand mutations).

## Findings

| Severity | Row | Finding | Resolution |
|---|---|---|---|

Keep the record under one page. The verifier's full report stays in the session, not here.
