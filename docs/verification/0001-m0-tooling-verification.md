# Verification 0001 — m0-tooling-verification

- Date: 2026-11-07
- Verdict: **FAIL** (partial). Per row: LIB-API-001 **PASS**; LIB-TOOL-001 FAIL; LIB-TOOL-002 FAIL; LIB-TOOL-003 FAIL
- Verifier: `sacm-verifier` (independent; read-only)
- Commit / diff inspected: `4b4a8e9` (initial commit) plus the uncommitted slice brief
- Slice brief: [docs/slices/0001-m0-tooling-verification.md](../slices/0001-m0-tooling-verification.md)

## Rows verified

| Row | Title | Evidence (tests) |
|---|---|---|
| LIB-API-001 | Library and standard versions are exposed | `lib_api_001_reports_library_and_sacm_versions`; doctests in `crates/sacm/src/lib.rs` (crate example, `VERSION`, `SACM_VERSION`) |

## Rows left open

| Row | Status kept | Reason |
|---|---|---|
| LIB-TOOL-001 | implemented | Many acceptance clauses and README gate rules have no test that fails when the rule is removed; citations resolve to any `fn`/`def`, not only tests. |
| LIB-TOOL-002 | implemented | Acceptance criteria 2–4 have no automated test; `cmd_fetch` calls `curl` directly and is untestable as written. |
| LIB-TOOL-003 | implemented | Read-only half of the live probe never reached the hook; no test checks the real `write-scopes.json`; malformed config and a crashed `python3` fail open. |

## Gates run

| Gate | Result |
|---|---|
| `cargo fmt --all -- --check` | pass |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | pass |
| `cargo test --workspace --all-features` | pass (1 + 2 + 19 + 3 doctests) |
| `cargo test -p xtask` | pass (19) |
| `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --all-features` | pass |
| `cargo xtask matrix check` | pass |
| `cargo deny check` | pass (warnings: duplicate `winnow`, unused allowed licences) |
| `python -m unittest discover -s .claude/hooks` | pass (11) |

## Live probes (run by the lead, LIB-TOOL-003)

1. `sacm-test-engineer` Write to `crates/sacm/src/probe_scope.rs` → denied by the hook:
   `PreToolUse:Write hook error: sacm-test-engineer may not write crates/sacm/src/probe_scope.rs. Its write scope is ['crates/*/tests/**', 'crates/*/benches/**', 'fuzz/**']. Hand this change back to the lead with the reason it is needed.`
2. `sacm-verifier` (background subagent) Write to `probe-readonly-role.txt` → refused by the harness before the hook: "No such tool available: Write".

## Findings and resolutions

Resolutions are pending: the fixes are in `xtask/` and `.claude/hooks/`, which no agent role's write scope covers. Routed to the maintainer for a decision on tooling ownership.

New rows created from the verifier's follow-ups: LIB-TOOL-005 (per-row verdicts in records), LIB-TOOL-006 (cited paths confined to the repository), LIB-DOC-006 (docs claim only verified capability).

## Verifier's report

> ## Verification result: FAIL — LIB-API-001 passes; LIB-TOOL-001, LIB-TOOL-002, LIB-TOOL-003 fail
>
> All four rows' implementations behave correctly as far as I could exercise them. The three tooling rows fail because their tests do not cover what their acceptance criteria claim, and LIB-TOOL-003 additionally lacks half of its live-probe criterion.
>
> ### Findings
> | Severity | Row | Finding | Required fix |
> |---|---|---|---|
> | High | LIB-TOOL-001 | Acceptance-criterion clauses with no test — removing the implementing code leaves `cargo test` green: unknown area; unknown milestone; id-area-segment ≠ `area` field; verified row with no `verification`; verification record that does not exist; dependency on a candidate/out-of-scope row; unknown dependency; self-dependency; missing rendered page. | Add one failing-input test per clause, each asserting its message. |
> | High | LIB-TOOL-001 | README gate rules with no test at all: rule 1 (`schema_version`, unique milestone ids, unique area codes), rule 3 (empty title/requirement/source), rule 5 (cited code path exists), rule 6 (malformed citation without `::`; `doctest:` file missing or without a code block). Forbidden-evidence cells untested: code on planned/analyzed/candidate, verification record on a non-verified row, tests on deferred/out-of-scope; analyzed-needs-acceptance. | Add tests, or a table-driven status × evidence test. |
> | Medium | LIB-TOOL-001 | A citation resolves if `fn <name>`/`def <name>` appears anywhere in the file (`defined_functions`) — comments, strings, non-test helpers — contradicting ADR 0004 ("a row cannot claim a test that does not exist"). The orphan check requires `#[test]`/`def test_`; the citation check does not. | Resolve citations with the `traced_tests` parser; test that citing a non-test helper is rejected. |
> | Medium | LIB-TOOL-001 | `next` partly tested: no assertion that verified/candidate/deferred rows are excluded, nor that rows within one milestone keep file order. | Extend the test: two ready rows in one milestone out of id order, plus a verified row. |
> | Low | LIB-TOOL-001 | Record check is a substring test ("LIB-X-001: FAIL" satisfies it). Cited paths not confined to the repository. Orphan detection misses single-line `#[test] fn x()` and multi-line attributes. No unit test of `render` output. | Track as follow-up rows. |
> | High | LIB-TOOL-002 | Only criterion 1 is tested. Criteria 2 (hash before move; mismatch deletes partial and fails), 3 (good existing file not re-downloaded; bad one is an error), 4 (git-ignored) have no automated test; `cmd_fetch` shells out to `curl` directly. Criteria 3, 4 and the success path confirmed manually; the fresh-download mismatch path was not exercised. | Inject the downloader; test mismatch → no `.part`, error; good existing file → downloader never called; bad existing → error. Add a repo check that `.gitignore` ignores `/third_party/sacm-2.3/`. |
> | High | LIB-TOOL-003 | Live probe only half met: probe 1 shows `agent_type` reaches the hook and the hook runs end to end; probe 2 never reached the hook (no Write tool). No live evidence of the hook denying a read-only role — the case ADR 0006 says the hook exists for. | Run a read-only role as a background subagent and attempt Write/Edit, or amend the criterion to what can be demonstrated, then re-verify. |
> | High | LIB-TOOL-003 | Nothing tests that the real `write-scopes.json` encodes the required roles; role tests use an in-test fixture. Giving `sacm-verifier` `"allow": ["**"]`, dropping implementer denies, or deleting `sacm-researcher` leaves every test green. | Test `decide()` against the real file for every role, and that every `.claude/agents/*.md` is listed. |
> | Medium | LIB-TOOL-003 | Fails closed only on unreadable/unparseable config. Structurally wrong config (`[]`, `{"agents": []}`) raises `AttributeError` → exit 1 → non-blocking → write allowed; a `null` agent entry is allowed. If `python3` crashes, the `|| python` fallback re-runs with stdin consumed → empty input → exit 0 → allowed. | Catch-all deny for subagent write calls; validate structure in `load_scopes`; test both. |
> | Low | LIB-TOOL-003 | No test that `settings.json` registers the matcher `Write\|Edit\|MultiEdit\|NotebookEdit`; `MultiEdit` never tested; Windows case-insensitivity untested. | Add a settings-matcher test and a MultiEdit case. |
> | Low | LIB-API-001 | Row did not cite its doctests. | Add `doctest:crates/sacm/src/lib.rs` when marking verified. |
> | Medium | docs (not blocking) | `crates/sacm/src/lib.rs` and `crates/sacm/README.md` describe the model, XMI, validation and editing in the present tense; none exists yet. | Reword as planned scope before any publish. |
> | Info | out of scope | LIB-CLI-001 broken-pipe behaviour has no criterion or test; LIB-QA-003 rejects only GPL-family licences; LIB-QA-001 gate check is a substring search. | Address when those rows are verified. |
>
> ### Matrix updates allowed
> - May mark verified: **LIB-API-001** (with the `doctest:crates/sacm/src/lib.rs` citation).
> - Must stay `implemented`: LIB-TOOL-001, LIB-TOOL-002, LIB-TOOL-003.
>
> (Full report, including the mutation-analysis table, was returned to the lead in session; the table's conclusion is summarized in the findings above.)
