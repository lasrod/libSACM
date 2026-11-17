# Verification 0063 — Every read and write refusal has a code, a template and arguments

- Date: 2026-11-18
- Row verdict: LIB-API-017: PASS
- Verifier: `sacm-verifier` (independent; read-only), two rounds
- Commits:
  - d029b07, round 1: FAIL on test gaps (8 hand mutants and 12 CI mutants survived) and on
    two doc claims. CI run
    [44136358763](https://github.com/lasrod/libSACM/actions/runs/44136358763) was green;
    the PR-diff mutation job does not block (ADR 0006 amendment 6).
  - 7f21d45, round 2: PASS. The CI run is cited in the PR.
- Slice brief: [docs/slices/0063-refusal-codes.md](../slices/0063-refusal-codes.md)
- Decision: ADR 0030 amendment 3, **proposed**. The row is verified against the ADR as
  written. If the maintainer changes its API details, the row is reworded and re-verified.

## Result

- **Codes:** every `ReadError` and `WriteError` has a catalog code, and `code()` returns
  `Code` (Breaking). `Unsupported` takes the code a tolerant read gives the same input.
- **Templates:** 103 templates and one renderer. Each refusal has `template()` and
  `arguments()`.
- **The English is byte-identical:** `messages-0.1.0.txt` was confirmed against a
  `git archive fc87074` copy (main).
- **Serde:** refusals serialize in ADR 0030 amendment 3's shape, and the R11 input checks
  are tested. A deserialized record renders its template.
- **The CLI** prints the codes.
- **The red snapshot,** compared with the final tests: 131 `code()` assertions gated as old
  and new forms. Each new form keeps or tightens the old intent, and nothing was weakened.
- **The 13 reworded verified rows are true:**
  - XMI-001, VAL-001, -006, -020, XMI-021, -008;
  - LIB-COMPAT-007, LIB-API-009, LIB-API-010, LIB-CLI-002 and LIB-API-006;
  - and, by their notes, LIB-API-011 and LIB-DIAG-002.

Evidence beyond CI:
- 26 hand mutations in round 1 and 19 in round 2. After round 2 all are killed.
- Every mutant CI missed in the slice's code is killed by a test that is not a doctest,
  except `refusal.rs:243`, which is triaged as equivalent.

## Findings

| Severity | Row | Finding | Resolution |
|---|---|---|---|
| blocking | LIB-API-017 | Untested: the parser-limit templates (M07), the unheld root-or-element choice (M08) and the name-value text template (M10). | Asserts and tests added; `fn unheld` is extracted and unit-tested. |
| blocking | LIB-API-017 | Untested: that a record renders its template (M11, M12), and the `Details` eq, hash and Debug (M13; CI 909, 917, 925). | Tests added. |
| blocking | LIB-API-017 | The accessors and serde messages were killed only by doctests (CI 762, 772, 840, 981, 1012). | Integration tests added. |
| blocking | LIB-API-017 | The SACM-XMI-003 row and the record docs overstated what holds. | Reworded with the verifier's text. |
| non-blocking | LIB-API-017, LIB-API-006 | The SACM-XMI-014 row's wording, a `Details` example, a CHANGELOG blank line, and a comma in LIB-API-006. | Fixed. |

## Logged

Three items in docs/follow-ups.md:
- a `debug_assert` in `unsupported_at`;
- the R11 question for the maintainer;
- the historical triage row.
