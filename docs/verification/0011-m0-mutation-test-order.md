# Verification 0011, round 1: m0-mutation-test-order

- Date: 2026-11-09
- Verdict: **FAIL** for LIB-QA-012, on test adequacy only. LIB-QA-007 does not regress.
- Row verdict: LIB-QA-012: FAIL
- Verifier: `sacm-verifier` (independent; read-only)
- Commit inspected: `bfb0d99d737d14df299a832991a2a0f7fbb0e1b9` (PR #18), with `git diff main...bfb0d99`. `7e657bd..bfb0d99` changes only docs.
- Contract: LIB-QA-012's items 1 to 8, judged against [ADR 0009](../adr/0009-nextest-for-mutation-testing.md), which the maintainer accepted.
- Linux evidence:
  - mutants.yml 42582691741 at `7e657bd`, and 42585528364 at `bfb0d99`: each 544 caught, 0 missed, 0 timeouts, 15 unviable.
  - The caught and unviable sets are byte-identical to the last `cargo test` full run (42571009239 at `e03ba86`), so no mutant relied on a doctest alone.
  - The logs show `cargo nextest run … --profile=mutants --no-tests=warn`.
  - ci.yml 42582691844 and 42585528357: success.
- **Copilot did not review.** Its review at `7e657bd` reported an error, so it is re-requested.
- Slice brief: [docs/slices/0011-m0-mutation-test-order.md](../slices/0011-m0-mutation-test-order.md)

## What holds

- **Item 1:** the `.cargo/mutants.toml` keys are exact.
- **Item 2:** `cargo nextest show-config version` (0.9.151) reports the requirement 0.9.111 as "ok". `--profile mutants` is a known profile, and `default` is untouched.
- **Item 3:** the exact install sits after the cargo-mutants install, in both mutants jobs.
- **Item 4:** other values are refused by LIB-QA-007, and by LIB-QA-012 too.
- **Item 6:** shown by both Linux runs.
- **Item 7, reproduced** for `is_identifier_char -> bool with true`, with a slow, first-sorting `aaa_slow` target:
  - with the slice configuration, it is caught in 36 s, and nextest "immediately terminat[es] 15 tests still running";
  - with `main`'s configuration, TIMEOUT;
  - with `terminate = "wait"`, TIMEOUT;
  - with `--test-tool cargo`, the baseline is refused, so there is no silent fallback.
- **Item 8:** met.
- **LIB-QA-007:** no test was dropped (37 ⊂ 40), and all 13 hand mutations of the changed code are killed.

## Rows left open

| Row | Status kept | Reason |
|---|---|---|
| LIB-QA-012 | implemented | P1 to P4 |

## Findings and the complete prescription (tests only, `xtask/tests/repo_checks.rs`)

| Severity | Finding | Required fix |
|---|---|---|
| Medium | **Q21:** `runs_cargo_mutants` with `contains` replaced by `starts_with("cargo mutants")` survives. A `run: \|` block, or a prefixed command, is untested. | **P1.** Controls in both workflows: (a) `cargo mutants` on a later line of a `run: \|` block, with no install, gives `[nextest-install-missing]`; (b) a prefixed command (`time cargo mutants …` or `env X=1 cargo mutants …`), with no install, gives `[nextest-install-missing]`; (c) the exact install after such a multi-line step gives `[nextest-install-order]`. They must kill `starts_with`, and "a line begins with `cargo mutants`". |
| Low | **Q24:** `names_nextest` without its `uses` branch survives. | **P2.** A `uses: <owner>/<name-containing-nextest>@v1` step with no `with`, in place of the install, gives exactly `[nextest-install-inexact]`. |
| Low | **Q27 and Q29:** dropping `--vers V` or `--vers=V` survives. | **P3.** `--vers 0.9.100` and `--vers=0.9.100` each give `[nextest-install-version]`; `--vers 0.9.151` gives `[nextest-install-inexact]`. |
| Low | **`NEXTEST_INSTALL_VERSION`** is not tied to the version in `NEXTEST_INSTALL`. | **P4.** Derive the constant from the string, or assert that they agree. It must kill an edit to one without the other. |

Not held:
- Q34, equivalent: Actions rejects a step with both `uses` and `run`.
- A CONTRIBUTING drift check: recommended, not required.

## New class (follow-up)

A `.cargo/config.toml` `[alias] nextest = …` shadows cargo-nextest, with only a warning. This is added to LIB-QA-016's requirement.
