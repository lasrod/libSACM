# Verification 0009, round 3: m0-ci-workflow-checks (after fix round 2)

- Date: 2026-11-09
- Verdict: **PASS.** LIB-QA-009 passes, and LIB-QA-008 (verified in round 2) has not regressed.
- Row verdict: LIB-QA-009: PASS
- Verifier: `sacm-verifier` (independent; read-only)
- Commit inspected: `2cffcbd540ce8aa1b47037c80d69f3650ff6f009` (PR #16), with `git diff 750483d..2cffcbd`.
- Fix round 2 was limited to comments and docs. The maintainer approved it after LIB-QA-009's second FAIL.
- Contract: R1 and R2 of the [round 2 record](0009-m0-ci-workflow-checks-round2.md).
- Upstream: the verifier re-read `entrypoint.sh` and `action.yml` of EmbarkStudios/cargo-deny-action at `3c6349835b2b7b196a839186cb8b78e02f7b5f25` (`gh api`).
- Linux evidence:
  - ci.yml run 42543871742 at `2cffcbd` succeeded on all 11 jobs.
  - The last finished full mutation run is 42541851411 at `7b6a3dd`: 453 caught, 0 missed. Since then the slice changed only comments and one string literal, which cargo-mutants does not mutate.
- Slice brief: [docs/slices/0009-m0-ci-workflow-checks.md](../slices/0009-m0-ci-workflow-checks.md)

The diff changes only these in `xtask/tests/repo_checks.rs`:
- the `REVIEWED_ACTIONS` cargo-deny-action comment;
- the `manifest-path` input comment;
- one doc comment;
- one `assert!` message string.

It also touches `docs/quality/ci-actions.md` and the brief. No condition, table entry, check function or other test changed.

## Round 2 prescription

| Item | Result |
|---|---|
| R1 | Met. The comment matches `entrypoint.sh` line for line. `$1` runs `rustup default`. `$2` sets `credential.helper store` and the two `insteadOf` rewrites, then writes `$HOME/.git-credentials`. `$3` and `$4` go to `/root/.ssh/id_rsa` and `/root/.ssh/known_hosts`. After `shift 4`, seven args reach `cargo-deny $*`. The conclusion is kept: the four inputs are refused, and `use-git-cli` is deprecated. |
| R2 | Met. The `manifest-path` comment and `ci-actions.md` state the `(cd "$(dirname "$4")"; rustup show \|\| rustup toolchain install)` effect, which LIB-QA-003's root-`Cargo.toml` rule (`repo_checks.rs:383`) confines. |

## Rows verified

| Row | Title | Evidence (tests) |
|---|---|---|
| LIB-QA-009 | Third-party CI actions are pinned and their inputs reviewed | The 14 cited `lib_qa_009_*` tests in `xtask/tests/repo_checks.rs`. Their mutation kills are recorded in rounds 1 and 2 (F9-2: Q9-26 and Q9-46; Q9-41). |

## Gates run (by the verifier, on Windows)

All exit 0:
- fmt;
- clippy (`-D warnings`);
- `cargo test --workspace --all-features` and `cargo test -p xtask` (95 `lib_qa_00*` tests);
- rustdoc (`-D warnings`);
- `matrix check` (161 rows);
- `cargo deny check`;
- the hook tests (47).

## Findings

| Severity | Row | Finding | Resolution |
|---|---|---|---|
| Info | none | The round 2 record said ci.yml run 42541851389 had "12 jobs". It had 11. | Corrected in the round 2 record. |
| Info | LIB-QA-009 | rustup also searches parent directories for `rust-toolchain.toml`. The repository has a root one, and only the root manifest is allowed, so the statement holds as written. | None. |
