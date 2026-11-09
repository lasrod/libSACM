---
slice: 0009
name: m0-ci-workflow-checks
state: done
step: close
rows: [LIB-QA-009, LIB-QA-008]
mode: pr
---

# Slice 0009: CI workflow checks

## Scope

Two rows that check the CI workflows by their parsed structure, not by their text.

- **LIB-QA-009.** Every third-party action is pinned by a full commit SHA. A
  reviewed table records each action and SHA, with the inputs the repository may
  pass it. A version bump therefore fails the check until someone reviews it.
- **LIB-QA-008.** Each gate of LIB-QA-001 must be a live step in `ci.yml`: the
  exact command as a whole `run` value, in a job that is neither conditional nor
  allowed to fail, under an environment, OS matrix and triggers that do not weaken
  it.

| Row | Title | Status at start |
|---|---|---|
| LIB-QA-009 | Third-party CI actions are pinned and their inputs reviewed | planned |
| LIB-QA-008 | CI gate checks match live steps, not text | planned |

## Out of scope

- SHA pinning of GitHub's own `actions/*` actions. They stay on major tags, which
  Dependabot updates.
- Checking the reviewed inputs against each action's `action.yml` from inside the
  test, which would need the network. The verifier checks them at the pinned SHAs
  with `gh api`.
- The `mutants` jobs, which LIB-QA-007 already checks.

## Normative basis

None. These are quality rows, so the acceptance criteria are the lead's. They are
recorded in the matrix rows.

## Design (fixed by the lead)

- **D1: what is pinned.** Every `uses` in `.github/workflows/*.yml`, whether of a
  step or of a job, whose owner is not `actions` (compared without regard to case)
  is `<owner>/<repo>@<40 lowercase hex>`. The pins, resolved on 2026-11-09:

  | Action | Ref | Commit |
  |---|---|---|
  | `EmbarkStudios/cargo-deny-action` | v2.1.1 (= `v2`) | `3c6349835b2b7b196a839186cb8b78e02f7b5f25` |
  | `Swatinem/rust-cache` | v2.9.3 (= `v2`) | `d1527c8f67ef9af357290475d4af4b0fab8f12e0` |
  | `dtolnay/rust-toolchain` | `master` | `683b588c487a69d01f6c6ddf40f036ff682cc2c8` |

  `dtolnay/rust-toolchain` chooses its toolchain from the ref name (`@stable`),
  and at `master` the `toolchain` input is required. So every use gets an
  explicit `toolchain: stable`, or `"1.85"` in the MSRV job. Each pin carries a
  `# <ref>` comment, which Dependabot keeps up to date.
- **D2: the reviewed table** lives in `xtask/tests/repo_checks.rs`. Each entry
  holds the action, the SHA, the ref it was resolved from, and the inputs the
  repository may pass. Each input carries a comment on how it reaches the
  action's command line. The inputs are listed in LIB-QA-009's acceptance.
  cargo-deny-action refuses `rust-version`, `credentials`, `ssh-key` and
  `ssh-known-hosts`. Its entrypoint uses them quoted and drops them before
  `cargo-deny $*`, but they switch the toolchain, store git credentials and add
  URL rewrites. `use-git-cli` has no effect. (Corrected after verification
  round 1, F9-1.)
- **D3: gates as live steps.** The LIB-QA-008 check reuses the existing workflow
  readers (`load_workflow`, `read_workflow`, `key_allowlist`,
  `guarded_step_problems` and `check_deny_job`). The problem codes are
  `gate-missing`, `job-key`, `step-key`, `step-guard`, `gate-env`, `gate-os`,
  `gate-trigger`, `workflow-defaults`, and the shared `shape`, `merge-key` and
  `yaml-parse`. The verified LIB-QA-001 test stays as it is.
- **D4: who writes what.**
  - The test engineer writes both checks and their tests in
    `xtask/tests/repo_checks.rs`.
  - The lead pins the workflows, because `.github/` is outside every agent's
    scope.
  - The implementer writes `docs/quality/ci-actions.md`.
- **D5: code citations.** Both rows cite `.github/workflows/ci.yml`; LIB-QA-009
  also cites `.github/workflows/mutants.yml`.

## Tests

| Tests | Row | Criterion |
|---|---|---|
| 14 `lib_qa_009_*` tests in `xtask/tests/repo_checks.rs` | LIB-QA-009 | `check_action_pins` over the real workflows and over fixtures with controls: <ul><li>every unpinned form;</li><li>unreviewed pins;</li><li>inputs outside the reviewed table;</li><li>the `actions` owner exemption;</li><li>shape and parse problems.</li></ul> |
| 15 `lib_qa_008_*` tests in `xtask/tests/repo_checks.rs` | LIB-QA-008 | `check_ci_gates` over the real `ci.yml` and over fixtures with controls: <ul><li>gates as whole `run` values;</li><li>unconditional jobs and steps;</li><li>`[gate-step]`;</li><li>the env allowlist and `RUSTDOCFLAGS`;</li><li>the OS matrix;</li><li>triggers;</li><li>the deny job.</li></ul> |

## Log

- 2026-11-09 research: skipped, because these are quality rows. The lead wrote the
  acceptance criteria, and both rows are now `analyzed`. The lead decided D1 to D5.
- 2026-11-09 tests: 25 tests. 24 pass now, because LIB-QA-008 pins behaviour that
  already exists. The LIB-QA-009 real-workflows test is red: 16 `action-unpinned`
  problems.
  - Lead decisions on the report:
    - Scope the verified LIB-QA-002 guard-bypass test to the MSRV job, because
      pinning removes its target line.
    - The cargo-deny job's `env` is judged too.
    - New LIB-QA-008 rule `[gate-step]`: every `run` step of a gate job is a gate, so
      no step can write `$GITHUB_ENV` or `$GITHUB_PATH`.
- 2026-11-09 the test engineer applied those decisions, and extended `[gate-step]`
  to the cargo-deny job (accepted). The verified LIB-QA-002 test's guard mutation
  now finds its line inside the MSRV job, with the same 11 controls. 26 tests in all
  (13 per row); the rows are `tests-failing`.
- 2026-11-09 implementation:
  - The lead pinned the three third-party actions in `ci.yml` and `mutants.yml`
    (D1). Every rust-toolchain use gets an explicit `toolchain:`.
  - The implementer wrote `docs/quality/ci-actions.md`.
  - All 106 `repo_checks` tests pass, and the full local gate passes. Both rows are
    `implemented`.
  - For the verifier, a claim that turned out wrong (F9-1): at v2.1.1,
    cargo-deny-action passes `rust-version`, `credentials`, `ssh-key` and
    `ssh-known-hosts` unquoted. The verifier showed that `b66acf5` is the `v2` tag
    object for the same commit, and that Verification 0002 round 3 was right: the
    four are used quoted, then dropped by `shift 4`.
- 2026-11-09 PR #16 is a draft. CI run 42536937710 and mutants.yml run 42536937727
  succeeded on `7a325c8`. The full run caught every mutant, with 0 missed and 0
  timeouts.
- 2026-11-09 Copilot review of PR #16: 2 valid comments on LIB-QA-008.
  1. A gate job could run an `actions/*` step such as `github-script` that writes
     `$GITHUB_ENV`.
  2. A step-level `env: BASH_ENV` on a reviewed action could do the same.

  Fixed before verification. LIB-QA-008 item 2 now requires two things:
  - every `uses` step of a gate job is a reviewed gate action with its reviewed
    inputs (`[gate-action]`);
  - no step of a gate job has `env` (`[gate-env]`).

  2 new tests and 2 changed controls; LIB-QA-008 now has 15 tests.
- 2026-11-09 verification round 1, at `a51f7df`:
  [record](../verification/0009-m0-ci-workflow-checks.md). FAIL for both rows,
  which stay `implemented`.
  - LIB-QA-009: F9-1 (the review record misstates cargo-deny-action) and F9-2
    (input names are not proved to be compared exactly).
  - LIB-QA-008: F8-1 to F8-4 (untested branches for a non-mapping
    `strategy`/`matrix`, prefix inputs, a non-mapping `pull_request`, a non-string
    `with` key).
  - New rows from new classes: LIB-QA-013 to -016.
  - Fix round 1: tests and the code comment go to the test engineer, the doc to the
    implementer, D2 and the log to the lead.
- 2026-11-09 fix round 1:
  - **Test engineer:**
    - the `REVIEWED_ACTIONS` comment is corrected (F9-1);
    - 11 controls (F9-2, F8-1 to F8-4), plus a new test reading `*.yaml` workflows (Q9-41);
    - each target mutant was applied in a scratch copy and is killed.
  - **Implementer:** `ci-actions.md` is corrected. `credentials` is named as adding
    the `insteadOf` rewrites, the pin's limit for docker images is stated (LIB-QA-015),
    and the review reads the Dockerfile.
  - **Incident:** the test engineer's mutant script ran an old scratchpad helper,
    which wrote an untracked mutated copy, `xtask/tests/zz_mutant.rs`. The lead
    confirmed it differed by one mutated line and deleted it.
  - LIB-QA-009 now has 14 tests and LIB-QA-008 has 15. The full local gate passes.
- 2026-11-09 verification round 2, at `7b6a3dd`:
  [record](../verification/0009-m0-ci-workflow-checks-round2.md).
  - LIB-QA-008 PASS, and it is now `verified`.
  - LIB-QA-009 FAIL (its second), on comments only: R1, the `REVIEWED_ACTIONS`
    comment credits the URL rewrites to the wrong inputs; R2, the
    toolchain-selecting effect of `manifest-path` is undocumented. Its behaviour
    and tests are adequate. It stays `implemented`.
  - After a second FAIL, the slice stops for the maintainer's decision.
- 2026-11-09 the maintainer approved a third fix round for LIB-QA-009, limited to
  comments and docs (R1 and R2). R1 and the code comment for R2 go to the test
  engineer, the doc clause for R2 to the implementer.
- 2026-11-09 fix round 2 (comments only):
  - R1: the `REVIEWED_ACTIONS` comment now credits `credentials` (`$2`) with the
    credential helper and the `insteadOf` rewrites, and puts `$3`/`$4` in
    `/root/.ssh`.
  - R2: the `manifest-path` comment and `ci-actions.md` say that the manifest's
    directory chooses the installed toolchain, confined by LIB-QA-003.
  - The only non-comment change is one assertion message string. The full gate
    passes.
- 2026-11-09 verification round 3, at `2cffcbd`:
  [record](../verification/0009-m0-ci-workflow-checks-round3.md). LIB-QA-009 PASS,
  and it is now `verified`. LIB-QA-008 shows no regression.
- 2026-11-09 close: the slice is done.
