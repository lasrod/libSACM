# Verification 0064 — Every diagnostic carries its template and arguments

- Date: 2026-11-18
- Row verdict: LIB-API-018: PASS
- Verifier: `sacm-verifier` (independent; read-only), three rounds
- Commits:
  - e2d67fb, round 1: FAIL on one test gap (mutant D1, a `null` template or arguments read
    as not given). No wrong behaviour.
  - f46f466, round 2: round 1 fixed. FAIL on a mutant from CI's PR-diff job
    (`details_from_record`'s guard → `true`). The maintainer approved a second fix round.
  - 4f4a281, round 3: PASS. CI run
    [44228339500](https://github.com/lasrod/libSACM/actions/runs/44228339500): every job
    passes except the mutation shards, which do not block (ADR 0006 amendment 6).
- Slice brief: [docs/slices/0064-diagnostic-templates.md](../slices/0064-diagnostic-templates.md)
- Decision: ADR 0030 amendment 3 (accepted 2026-11-18).

## Result

- **Templates:** every `Diagnostic` the crate emits has `template()` and `arguments()`, and its
  message is rendered from them. There are 102 diagnostic templates (30 from validation, 72 from
  the reader) and 204 in all, each listed in the catalog.
- **The English is unchanged:**
  - `messages-0.1.0.txt` has no change;
  - the 102-input message test passes on the code from before the slice (base 3025637 with the
    red snapshot's test).
- **Serde:**
  - the 8 deserialize checks run in the designed order, with the designed messages;
  - `null` is refused;
  - `diagnostics-0.1.0.json`, byte-identical to v0.1.0's golden file, still reads.
- **Order:** keys 8 and 9; `cmp` is `Equal` exactly when the diagnostics are `==`.
- **Construction:**
  - `Diagnostic::create` is gone, and only `emit` builds a diagnostic outside `diagnostic.rs`;
  - the reader's (reason, holder) mapping is one exhaustive `match`, so it has no empty cell
    and cannot panic.
- **The reworded verified rows are true:** LIB-API-017 acceptance 5, LIB-DIAG-001 and
  LIB-API-006.
- **The lead's ruling (b):** the three tests whose input went through `serde_json::Value` had
  only their mechanics changed, so nothing is weakened.

Evidence beyond CI:
- the builder's 26 hand mutants and the verifier's 23 are killed;
- the CI survivor is killed in round 3.

## Findings

| Severity | Finding | Resolution |
|---|---|---|
| blocking | Untested: `null` for `template` and `arguments` (D1). | The null cases assert the reason, and a both-null case was added. |
| blocking | Untested: the reason given for a refusal record that names another refusal's template (the CI mutant). | A case was added. |
| non-blocking | Untested: the order of checks 6 and 7 (D2). | A case was added. |
| non-blocking | The `lib.rs` serde text, the catalog wrap, the CHANGELOG `Debug` sentence, ADR 0011's keys 8 and 9. | Fixed with the verifier's text. |

## Logged

Two items in docs/follow-ups.md:
- R11's in-order arguments against JSON passed through `serde_json::Value`, a question for the
  maintainer;
- the construction check misses qualified paths.
