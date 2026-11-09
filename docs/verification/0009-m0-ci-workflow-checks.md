# Verification 0009, round 1: m0-ci-workflow-checks

- Date: 2026-11-09
- Verdict: **FAIL.** Both rows fail, each on a small, closed set of untested branches.
- Row verdict: LIB-QA-009: FAIL
- Row verdict: LIB-QA-008: FAIL
- Verifier: `sacm-verifier` (independent; read-only)
- Commit inspected: `a51f7df3448325cbe88e28fdbd1d544bf183a456` (PR #16), with `git diff main...a51f7df`.
- Linux evidence:
  - ci.yml run 42538511192 succeeded on every job. The hook tests ran 47 tests, all OK. The pinned actions were downloaded, MSRV ran rustc 1.85.1, and cargo-deny passed.
  - mutants.yml run 42538511198 succeeded: 453 caught, 0 missed, 0 timeouts, 11 unviable.
  - Both checks live in `xtask/tests/`, so cargo-mutants gives no evidence for these rows. The verifier's 107 hand mutations are the primary evidence; 18 survive.
- Upstream (`gh api`):
  - **Every pin resolves to its recorded ref.** The v2.1.1 and v2.9.3 annotated tags dereference to `3c63498` and `d1527c8`, and `heads/master` is `683b588`.
  - **The reviewed inputs match each `action.yml` at its pin.**
- Slice brief: [docs/slices/0009-m0-ci-workflow-checks.md](../slices/0009-m0-ci-workflow-checks.md)

The verifier's verdict: "Both rows are close. Every gate passes and CI on the head commit is green. Each row fails on a small, closed set of untested branches. LIB-QA-009 also fails because its review record (code comment and doc) misreads how cargo-deny-action uses its inputs."

No regression in the verified rows. LIB-QA-002's changed test keeps the same 11 mutations, with the same names and expected codes. LIB-QA-001, -003 and -007 pass on the pinned workflows. The Copilot fixes (`[gate-action]`, step `env`) hold, and the mutants that would remove them are caught.

## Rows left open

| Row | Status kept | Reason |
|---|---|---|
| LIB-QA-009 | implemented | F9-1, F9-2 |
| LIB-QA-008 | implemented | F8-1 to F8-4 |

## Findings and the complete prescription

| Severity | Row | Finding | Required fix |
|---|---|---|---|
| Medium | LIB-QA-009 | **F9-1. The review record misstates cargo-deny-action.** `entrypoint.sh` at `3c63498` uses its first four args quoted: `rustup default "$1"`, and the credentials and ssh files. Then `shift 4` drops them, and only the remaining five reach `cargo-deny $*`. `use-git-cli` has no effect. `b66acf5` is the annotated `v2` tag object, which dereferences to the same commit; it is not an older commit. Verification 0002 round 3 was right. | Rewrite the `REVIEWED_ACTIONS` comment and `ci-actions.md` to the real mechanism. The four inputs are refused because they switch the toolchain, store git credentials and add URL rewrites. Qualify "a pin fixes the code" for docker actions, whose image is built at run time and downloads its tools then. Add the Dockerfile's build-time downloads to review step 2. The lead corrects D2 and the log line. |
| Medium | LIB-QA-009 | **F9-2.** The input-name comparison is not proved exact: Q9-26 (`key.starts_with(input)`) and Q9-46 (`input.starts_with(key)`) survive. | Controls giving exactly `[action-input]`: `toolchain-file`, `tool` and `components-extra` on rust-toolchain. |
| Medium | LIB-QA-008 | **F8-1.** The not-a-mapping branches of `strategy` and `matrix` are untested: Q8-21, Q8-63 and Q8-64 survive. | Controls giving exactly `[gate-os]`: <ul><li>`matrix: ${{ fromJSON(vars.MATRIX) }}`;</li><li>`strategy` with `fail-fast` and no `matrix`;</li><li>`strategy: ${{ fromJSON(vars.STRATEGY) }}`.</li></ul> |
| Medium | LIB-QA-008 | **F8-2.** The gate-input comparison is not proved exact: Q8-62 survives. | Controls giving exactly `[gate-action]`: `python-version-file`, and a prefix key `python`, on setup-python. |
| Low | LIB-QA-008 | **F8-3.** Q8-16 survives: a sequence or string `pull_request` is accepted. | Controls `pull_request: [opened]` and `pull_request: main`, each giving exactly `[gate-trigger]`. |
| Low | LIB-QA-008 | **F8-4.** Q8-40 survives: a non-string `with` key is accepted. | Control `actions/checkout` with `with: {1: x}`, giving exactly `[gate-action]`. |

Not held:
- **Equivalent or unreachable:** Q8-08, Q8-37, Q8-56, Q9-18 and Q9-28.
- **Low recommendations:** Q9-41 (`*.yaml`), Q9-07, Q9-08, Q9-10 (malformed repo names, refused either way) and Q9-35 (shape problems naming the file).

## New classes (follow-up rows)

- **Imposter commits:** `actions/*` is accepted at any ref, including a SHA from the fork network. This becomes LIB-QA-013.
- **Runners:** the `runs-on` value of non-test gate jobs is unchecked. This becomes LIB-QA-014.
- **Docker image supply chain:** the cargo-deny-action image downloads its tools at build time, without a checksum. This becomes LIB-QA-015.
- **Configuration outside `ci.yml`:** gates can be weakened by `.cargo/config.toml`, `rust-toolchain.toml`, `clippy.toml` and `rustfmt.toml`. This becomes LIB-QA-016.
- **Gate-action input values:** unchecked. Info only, mitigated for Rust by `rust-toolchain.toml`.
