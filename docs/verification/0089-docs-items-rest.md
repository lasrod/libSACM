# Verification 0089 — Item docs of `edit` and `xmi`, examples, and what Assurance Forge asked

- Date: 2026-11-28
- Row verdict: LIB-DOC-014: PASS
- Row verdict: LIB-DOC-015: PASS
- Row verdict: LIB-DOC-002: PASS
- Row verdict: LIB-DOC-010: PASS
- Verifier: `sacm-verifier` (independent; read-only)
- Commit: 9132b55 (first round e9a439c); evidence: gate logs `target/v0089-*.log`, probes `target/scratch/probe0089`, mutants `target/v0089-mutants.log`
- Slice brief: [docs/slices/0089-docs-items-rest.md](../slices/0089-docs-items-rest.md)

## Result

The verifier confirmed that only comments changed, checked every row of `DeleteElement`'s
deleted-ids table and the claims read from the code with probes, confirmed both documented
round-trip exceptions and found a third (what a copy brings), checked that all 21
macro-generated operations and every public type render a runnable `# Examples` section,
and resolved every heading link in the rendered HTML. The fix round applied its texts.

## Findings

| Blocking / non-blocking | Row | Finding | Resolution |
|---|---|---|---|
| Blocking | LIB-DOC-015 | The round-trip guarantee missed a third exception: a copy brings its source's preserved content and metadata ids. | Stated; LIB-DOC-015's acceptance names it. |
| Blocking | LIB-DOC-014 | The moved delete checks left proxies out, though the code counts them. | Proxies named in checks 2 and 4 and `NotEmpty`. |
| Non-blocking | LIB-DOC-014, -015 | Wording: copies, "no typed reference", in-use ids, the new revision of a no-op set, `location` ids. | Fixed with the verifier's text. |
| Non-blocking | LIB-DOC-015 | The guarantee has no verified row behind it yet. | Kept with its corrected exceptions until LIB-RT-004 and LIB-EDIT-026 test it (the lead). |
| Non-blocking | LIB-DOC-002 | No control for a `pub const fn` in the fallible-function finder. | docs/follow-ups.md. |
