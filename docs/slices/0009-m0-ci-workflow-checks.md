---
slice: 0009
name: m0-ci-workflow-checks
state: in-progress
step: verification
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
  `credentials`, `ssh-key` and `rust-version` are refused for cargo-deny,
  because the action splices them into its arguments.
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
| 13 `lib_qa_009_*` tests in `xtask/tests/repo_checks.rs` | LIB-QA-009 | `check_action_pins` over the real workflows and over fixtures with controls: <ul><li>every unpinned form;</li><li>unreviewed pins;</li><li>inputs outside the reviewed table;</li><li>the `actions` owner exemption;</li><li>shape and parse problems.</li></ul> |
| 13 `lib_qa_008_*` tests in `xtask/tests/repo_checks.rs` | LIB-QA-008 | `check_ci_gates` over the real `ci.yml` and over fixtures with controls: <ul><li>gates as whole `run` values;</li><li>unconditional jobs and steps;</li><li>`[gate-step]`;</li><li>the env allowlist and `RUSTDOCFLAGS`;</li><li>the OS matrix;</li><li>triggers;</li><li>the deny job.</li></ul> |

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
  - For the verifier: at v2.1.1, cargo-deny-action passes `rust-version`,
    `credentials`, `ssh-key` and `ssh-known-hosts` in its unquoted `args`.
    Verification 0002 round 3 recorded them as quoted at the older `@v2` commit
    `b66acf5`. Confirm the v2.1.1 reading at `3c63498`.
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
