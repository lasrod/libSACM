# Verification 0003: m0-mutation-testing (round 1)

- Date: 2026-11-07
- Verdict: **FAIL** for LIB-QA-007. **P9 confirmed**: the 10 new killing tests under the already-verified LIB-TOOL-001 and LIB-TOOL-002 are accepted.
- Verifier: `sacm-verifier` (independent; read-only)
- Inspected: `2c32b39..89be9e2` on `slice/0003-m0-mutation-testing` (draft PR #9). The tree is clean, and no scratch copies remain.
- Contract: [the pre-verification prescription](0003-m0-mutation-testing-prescription.md)

## Linux evidence (the verifier checked these runs itself)

| Run | Commit | Result |
|---|---|---|
| Baseline, `mutants.yml` [42368743129](https://github.com/lasrod/libSACM/actions/runs/42368743129) | `d8efe70` | 222 mutants: 199 caught, 17 missed, 2 timeout, 4 unviable (cargo-mutants 27.1.0) |
| Full run, `mutants.yml` [42370175164](https://github.com/lasrod/libSACM/actions/runs/42370175164) | `3acb370` | 216 mutants: 212 caught, 0 missed, 0 timeout, 4 unviable |
| Head, `mutants.yml` [42370476157](https://github.com/lasrod/libSACM/actions/runs/42370476157) | `89be9e2` | All 4 shards succeeded, with outcome lists byte-identical to `3acb370` |
| Head, `ci.yml` [42370476101](https://github.com/lasrod/libSACM/actions/runs/42370476101) | `89be9e2` | PR job 12/12 caught; every job green |

Correction to the lead's assumption: the PR path filter matches the whole PR diff against the base, so `mutants.yml` re-runs on every push to the PR, docs-only pushes included.

## What passed

- **D1–D17:** met.
- **Maintainer decisions:** D2 and D10 are met, and D14 is met under `mode=pr`.
- **P1–P5:** every listed mutation and control is present and asserts its code. The verifier also looked for bypasses outside the list and found none.
- **P7 (tests first):** reproduced. All 37 tests fail against the pre-slice files, and the ordering of the SKILL check is accepted.
- **P8:** the 19 table rows are exactly the 19 baseline survivors. Each is killed by the test named in the Linux per-mutant logs. The five rewritten-line mutants were reproduced on the pre-rewrite code, and are killed there; the two hangs fail at the 5-second deadline. The `match_indices` rewrite is behaviour-preserving.
- **P10:** nothing was weakened, and the citations are complete.
- **The 4 unviable mutants:** structural (`Matrix: Default` is not satisfied).
- The `function_scan.rs` deadline comment stays as it is: it is accurate, and still guards against index arithmetic returning.

## Findings: the complete remainder for fix round 1

| Id | Severity | Finding | Required fix |
|---|---|---|---|
| F1 | Major (**new class**, the verifier's own omission in P6) | An excluded mutant never appears in any report list, but `sacm-verifier.md` only says to review the missed, timeout and unviable lists. A future "equivalent" exclusion would therefore go unchecked. | (a) `sacm-verifier.md`: for every exclusion a slice adds or changes (the `.cargo/mutants.toml` diff, and triage rows marked equivalent or accepted), the verifier re-derives the claim against P8.2. It also shows the excluded set equals the triaged set by comparing `cargo mutants --workspace --list` with `--list --no-config`. A claim it can break is a finding. (b) `check_verifier_mutants` and its fixture require this paragraph. Deleting it, or dropping `--no-config`, is reported as `[doc-verifier]`. |
| F2 | Minor | `mode=local` has no way to obtain the Linux report. | **Maintainer decision (2026-11-07): in `mode=local`, no row whose code cargo-mutants mutates may PASS until a Linux report for that commit exists, and a local Windows run is not a substitute.** (a) `SKILL.md` step 7 states this before the verifier runs. (b) `sacm-verifier.md` says what follows when no Linux report for the commit under verification is provided: test adequacy is not shown, so no such row may PASS. (c) The doc checks require both statements, and deleting either is reported. |
| F3 | Minor | The removed sentence was the only stated mechanical path for checking a hand-reasoned mutation. | `sacm-verifier.md`: a hand-reasoned mutation may be applied only in a scratch copy under `target/`, never in the working tree, or else listed for the lead. The doc check requires this, and deleting it is reported. |
| F4 | Minor | Inaccuracies in `docs/quality/mutants-triage.md`: (a) the 465:35 rows name no test; (b) the five rewritten-line rows do not say those mutants are absent from the final set; (c) the X-flush line names the wrong killer (the Linux log shows `lib.rs:87:5` killed by the `cli.rs` binary tests); (d) the final positions of the unviable mutants shifted (530, 1014, 1055); (e) the "final run" links `3acb370`, not the head. | Correct each point: name the killers, state absence plus the reproduced pre-rewrite kill, reword X-flush, give the final positions, and say the final run is the last run at a commit that changed code or configuration. The verification record cites the runs at the verified head. |

## Matrix updates

- LIB-QA-007 stays `implemented` until F1–F4 are fixed. Also required: the doc checks extended (each deletion reported, then green on the real files), the gates passing, and green `mutants.yml` and `ci.yml` runs at the new head.
- LIB-TOOL-001 and LIB-TOOL-002 stay `verified`. Their 10 new tests:
  - LIB-TOOL-001:
    - `function_scan.rs`: 3 tests
    - `matrix.rs`: the cycle message, nested brackets and render milestone tests
    - `run.rs`: the status counts and `repo_root` tests
  - LIB-TOOL-002: `fetch.rs`, the two `curl_download` tests.

## Follow-ups suggested

- A mechanised check that each `exclude_re` and `skip_calls` entry matches exactly one mutant.
- GitHub's 300-file limit on `paths` filters.
- A LIB-TOOL-002 test of `--fail`/`--location` against a local HTTP server.
- The implementer definition should say that hand-checked mutations belong in `target/` scratch copies.
- `docs/loop.md` should mention the draft PR before verification.
