# Verification 0009, round 2: m0-ci-workflow-checks (after fix round 1)

- Date: 2026-11-09
- Verdict: **partial.** LIB-QA-008 passes. LIB-QA-009 fails, on comments only.
- Row verdict: LIB-QA-008: PASS
- Row verdict: LIB-QA-009: FAIL
- Verifier: `sacm-verifier` (independent; read-only)
- Commit inspected: `7b6a3dd786945e08e69bcb56f03e6048ca463e47` (PR #16), with `git diff a51f7df..7b6a3dd` in full.
- Contract: the prescription table of the [round 1 record](0009-m0-ci-workflow-checks.md), item by item.
- Linux evidence:
  - ci.yml run 42541851389 succeeded on all 11 jobs (corrected in round 3).
  - mutants.yml run 42541851411 succeeded: 453 caught, 0 missed, 0 timeouts, 11 unviable. The unviable mutants are `Default` and `+` replacements in `xtask/src/{fetch,matrix}.rs`, none of them slice code.
  - Both checks live in `xtask/tests/`, so the primary evidence is the verifier's hand mutations in a `target/scratch` copy.
- Slice brief: [docs/slices/0009-m0-ci-workflow-checks.md](../slices/0009-m0-ci-workflow-checks.md)

The verifier's verdict: "LIB-QA-008 passes: every item in its round 1 prescription is closed and tested. LIB-QA-009 fails only on what is left of F9-1. Its tests (F9-2) pass, but the `REVIEWED_ACTIONS` code comment still gets cargo-deny-action wrong. That comment is a review record the prescription explicitly asked to correct. The fix is comment-only, with no change to behaviour or tests."

## Round 1 prescription

| Item | Result |
|---|---|
| F9-1, review record | Partly met. `ci-actions.md` and the brief are correct, but the `REVIEWED_ACTIONS` comment is still wrong (R1, R2). |
| F9-2 (Q9-26, Q9-46) | Both killed. Met. |
| F8-1 (Q8-21, Q8-63, Q8-64) | Killed, including each half of the not-a-mapping branches. Met. |
| F8-2 (Q8-62, both directions) | Both killed. Met. |
| F8-3 (Q8-16) | Killed, both the sequence and the string form. Met. |
| F8-4 (Q8-40) | Killed. Met. |
| Q9-41 (recommended) | Killed by `lib_qa_009_yaml_and_yml_workflows_are_read`: four mutations. |

No check function changed, and the `workflow_files(dir)` refactor is equivalent. No control was removed.

## Rows verified

| Row | Title | Evidence (tests) |
|---|---|---|
| LIB-QA-008 | CI gate checks match live steps, not text | The 15 cited `lib_qa_008_*` tests in `xtask/tests/repo_checks.rs`. |

## Rows left open

| Row | Status kept | Reason |
|---|---|---|
| LIB-QA-009 | implemented | R1, R2: the review-record comment. Behaviour and tests are adequate. |

## Gates run (by the verifier, on Windows)

All exit 0:
- fmt;
- clippy (`-D warnings`);
- `cargo test --workspace --all-features` and `cargo test -p xtask`;
- rustdoc (`-D warnings`);
- `matrix check` (161 rows);
- `cargo deny check`;
- the hook tests (47).

LIB-QA-001, -002, -003 and -007 show no regression.

## Findings and the complete prescription

| Severity | Row | Finding | Required fix |
|---|---|---|---|
| Low (blocks LIB-QA-009) | LIB-QA-009 | **R1.** The `REVIEWED_ACTIONS` comment credits the `insteadOf` URL rewrites to `"$3"`/`"$4"` and says they are written to `~/.ssh`. At `3c63498`, the `"$2"` (`credentials`) block sets `credential.helper store`, adds the two `insteadOf` rewrites and writes `$HOME/.git-credentials`. `"$3"` and `"$4"` are written to `/root/.ssh/id_rsa` and `/root/.ssh/known_hosts`. | Reword the comment to match. Keep the conclusion (all four refused; `use-git-cli` deprecated). Optionally say "the remaining seven args (five inputs and two flags)". |
| Low (round 1 miss) | LIB-QA-009 | **R2.** After `shift 4`, the entrypoint runs `(cd "$(dirname "$4")"; rustup show \|\| rustup toolchain install)`. `$4` is then the `manifest-path` value, so the manifest's directory chooses which `rust-toolchain.toml` is installed. | Add this to the `manifest-path` input comment, and in one clause to the cargo-deny-action bullet of `ci-actions.md`. Note that LIB-QA-003's root-`Cargo.toml` rule confines it to the repository's own file. |

To re-verify, the verifier needs only a comment and doc diff, the gate, and green CI at the new head. There is no new class this round. LIB-QA-015 also covers the `rustup toolchain install` the action runs inside its container.
