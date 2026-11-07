# Verification 0002, round 3: m0-qa-and-cli-verification (after fix round 2)

- Date: 2026-11-07
- Verdict: **FAIL** (partial). LIB-QA-002: **PASS**. LIB-QA-003: FAIL.
- Verifier: `sacm-verifier` (independent; read-only)
- Inspected: `f9a8c82` (PR #7, on `main` at `871c26f`). The follow-up `4b46db5`, the `log-level` fix, was read but not verified.
- CI: run 42359606355 on `f9a8c82` succeeded, including the MSRV and cargo-deny jobs.
- Contract: the [round-2 record](0002-m0-qa-and-cli-verification-round2.md) (R3-3 to R3-7) and its [addendum](0002-m0-qa-and-cli-verification-round2-addendum.md) (conditions 1–5, Y2/Y3, every must-kill and every control), plus all kills from rounds 1 and 2.

## Rows verified

| Row | Title | Evidence |
|---|---|---|
| LIB-QA-002 | MSRV 1.85 is declared consistently and checked | All four criteria are met. Y2-1..Y2-3 and design conditions 2–5 hold. Every MSRV must-kill from rounds 1 and 2 and the addendum is killed by the real checks in a fresh scratch copy, and every positive control is accepted. `cargo +1.85 test -p sacm -p sacm-cli` passes locally and in CI. Cited: 11 `lib_qa_002_*` tests in `xtask/tests/repo_checks.rs`. |

## Rows left open

**LIB-QA-003 (implemented).** Both findings fall inside the verifier's earlier prescriptions; the verifier records them as its own misses.

| Id | Finding | Required fix |
|---|---|---|
| F1 | **`log-level` injection** (Copilot, PR #7). cargo-deny-action@v2 (`b66acf5`) ends with an unquoted `cargo-deny $*`, so the five inputs after `shift 4` are word-split and globbed. Survivors at `f9a8c82`: flow `"warn --exclude sacm-cli"`, block `warn --exclude-dev`, `LOG-LEVEL: …`, and `with: *alias`. Run against real cargo-deny, `"warn --exclude-dev"` turned a failing licence check into `licenses ok`. | `log-level`, if set, must be exactly one of off/error/warn/info/debug/trace, with the input name case-insensitive. **Must kill:** flow and block injection; `"warn --exclude-dev"`; `LOG-LEVEL: "warn --exclude-dev"`; an alias `with: *anchor` carrying the injection; a non-string value; `log-level: ""`. **Controls:** `warn` (flow and block), `off` (must stay a string), `rust-version: "1.85.0"`. `4b46db5` has the rule and most of the must-kills. It lacks the upper-case input name, the alias and the empty value. |
| F2 | **`include-dev` defaults to false.** Probed with cargo-deny 0.20.2: dev-dependencies are not licence-checked by default, while build dependencies are. "Absent" therefore means the same as `false`, and design condition 1 ("the dependency's licence is checked") is not met. `yaml-rust2`, `encoding_rs`, `hashlink`, `arraydeque` and `foldhash` are not checked, although their licences are permissive. | **(a), recommended:** add `include-dev = true` to `[licenses]` (probed: `cargo deny check` still passes), and require it in the test. Must kill: the key removed; `false`. Control: `true`. **(b):** record in `docs/sacm/decisions.md` that dev-dependencies, which are not distributed, are outside the licence policy, and amend R3-6 and condition 1 to match. |

## Verifier's other conclusions

- The test engineer's stricter checks are all acceptable. Matching input names case-insensitively agrees with GitHub's parser.
- `rust-version`, `credentials`, `ssh-key` and `ssh-known-hosts` are consumed quoted, so they cannot narrow the check. With F1 fixed, the five spliced inputs each have an exact rule or an allowlist.
- Not findings:
  - a top-level `Defaults:`, which GitHub rejects as invalid;
  - the deprecated advisory keys, which cargo-deny refuses;
  - `$GITHUB_PATH`/`$GITHUB_ENV` sabotage, which falls under the existing "not held to" for `env:`.
- R3-7's key lists match cargo-deny 0.20.2's own `unexpected-keys` lists exactly.
- 27 sabotages of the check code were each killed by 1–5 named tests.

## Follow-ups suggested

- Pin `EmbarkStudios/cargo-deny-action` by commit SHA, and review the action's input names whenever its version is bumped.
- If F2 (b) is chosen, a decision record.

## Stop

LIB-QA-003 has now failed verification three times. F2 needs a policy choice from the maintainer, (a) or (b), so the loop stops for the maintainer.
