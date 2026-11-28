# Verification 0092 — Property tests, fuzzing, coverage, semver checks, the download test

- Date: 2026-11-28
- Row verdict: LIB-RT-004: PASS
- Row verdict: LIB-QA-006: PASS
- Row verdict: LIB-QA-004: PASS
- Row verdict: LIB-TOOL-012: PASS
- Verifier: `sacm-verifier` (independent; read-only)
- Commit: 554d4a0 (first round d3787e6); CI runs 46139496422 (first round 46107833317)
- Slice brief: [docs/slices/0092-quality-tooling.md](../slices/0092-quality-tooling.md)

LIB-SEC-004 is not verified. The PR's `fuzz (smoke)` run found a library defect: a tolerant
write does not read back equal when a renamed namespace prefix is declared around preserved
content (crash-1aa1ae6a, and crash-cc2fde73 on the second run). It is filed as LIB-COMPAT-022.
LIB-SEC-004 depends on it and stays `implemented` until a smoke run comes back clean.

## Result

The first round passed LIB-QA-004 and LIB-TOOL-012. It failed LIB-RT-004, because the
property generator never had 9 of the 59 edit operations accepted. It failed LIB-QA-006 on 15
missed mutants in the I/O shell of `cargo xtask semver`, and because `semver checks` was not
yet a required check.

The fix round addressed these:

- **LIB-RT-004:** the editing properties tally each operation and fail at setup when one is
  never accepted. Making `RemoveReference` unreachable by hand fails both properties with that
  message. At the default case counts, properties 1 and 6 accept all 59 operations.
- **LIB-QA-006:** three subprocess tests of `cargo xtask semver` use a stub `CARGO`. Over the
  54 `semver` mutants, cargo-mutants reports 52 caught, 0 missed and 2 unviable (no `Default`
  impl). `semver checks` is a required check on `main`.
- **LIB-QA-004:** the coverage job passes at the maintainer's threshold of 98% (98.78% lines).

## Findings

| Blocking / non-blocking | Row | Finding | Resolution |
|---|---|---|---|
| Blocking | LIB-RT-004 | The generator never had 9 of the 59 operations accepted. | Per-operation tally with a setup assertion; all 59 are reached. |
| Blocking | LIB-QA-006 | 15 mutants of the I/O shell were missed; `semver checks` was not required. | Three subprocess tests; required check added. |
| Non-blocking | LIB-TOOL-012 | The fetch.rs module doc did not name the row or the local server. | Fixed with the verifier's text. |
| Non-blocking | LIB-QA-006 | The brief claimed triage rows for the I/O shell; there are none, and none are needed. | Fixed with the verifier's text. |
| Non-blocking | LIB-RT-004 | At the fixed seeds a few operations are accepted only once or twice. | None: the seeds are fixed, and the setup assertion flags any loss. |
| Non-blocking | LIB-QA-006 | A mutant that truncates the job summary instead of appending survives, because each test starts with no summary file. | None: each GitHub step has its own summary file. |
