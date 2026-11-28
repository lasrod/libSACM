# Verification 0091 — The user guide, the SACM primer and runnable examples

- Date: 2026-11-28
- Row verdict: LIB-DOC-003: PASS
- Row verdict: LIB-DOC-005: PASS
- Row verdict: LIB-DOC-004: PASS
- Verifier: `sacm-verifier` (independent; read-only)
- Commit: 42f0818 (first round fa0c381); evidence: a probe crate running the guide's claims
  against the API, hand mutants of every new check, each example run twice
- Slice brief: [docs/slices/0091-guide-primer-examples.md](../slices/0091-guide-primer-examples.md)

## Result

The verifier checked every statement of the primer against the pinned formal/23-05-08
(clauses 2, 4, 7.4, 7.5, 8 to 12, Annex E) and the `gid` sentence against XMI 2.5.1 7.6.1. It
checked the guide's claims against verified rows and ran them against the API in a probe. All
four examples run without arguments, exit 0 and print the same bytes on every run. Each new
check was hand-mutated and killed. The first round found a false claim and a non-conforming
example in the guide; the fix round fixed both with the verifier's text.

## Findings

| Blocking / non-blocking | Row | Finding | Resolution |
|---|---|---|---|
| Blocking | LIB-DOC-003 | The guide said `validate` reports an empty name; it does not (`content` is `[0..1]`, D-NAME-MISSING). | Replaced with an AssertedEvidence whose source is a Claim (SACM-REF-002, 11.15). |
| Blocking | LIB-DOC-003 | The language example gave a Claim two descriptions (SACM-MULT-001, 8.6). | A second Claim holds the second description; the doctest asserts the case validates clean. |
| Non-blocking | LIB-DOC-003, -005 | Twelve wording items (Rust blocks only run; tolerant refusals; the LIB-COMPAT-013 gap; primer precision on packages, `isAbstract`, `gid`, AssertedContext, 7.4). | Fixed with the verifier's text. |
| Non-blocking | LIB-DOC-003 | The crate root links the guide on GitHub `main`, so a 0.5.0 reader may see a newer guide. | Follow-up: link the release tag in the release PR. |
