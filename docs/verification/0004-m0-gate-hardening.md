# Verification 0004, round 1: m0-gate-hardening

- Date: 2026-11-08
- Verdict: **FAIL** (partial). LIB-TOOL-009 and LIB-TOOL-010 pass; LIB-TOOL-005 and LIB-TOOL-006 fail.
- Row verdict: LIB-TOOL-009: PASS
- Row verdict: LIB-TOOL-010: PASS
- Row verdict: LIB-TOOL-005: FAIL
- Row verdict: LIB-TOOL-006: FAIL
- Verifier: `sacm-verifier` (independent; read-only)
- Commit inspected: `e216a8291de9e3909cf1896cb9142f96148f8edd` (draft PR #10), `git diff main...HEAD`
- Linux evidence, downloaded by the verifier:
  - ci.yml [42387860881](https://github.com/lasrod/libSACM/actions/runs/42387860881): all 11 jobs succeeded. `mutants (PR diff)`: 34 mutants, 33 caught, 0 missed, 0 timeout, 1 unviable.
  - mutants.yml [42387860879](https://github.com/lasrod/libSACM/actions/runs/42387860879): 233 mutants, 228 caught, 0 missed, 0 timeout, 5 unviable.
- Slice brief: [docs/slices/0004-m0-gate-hardening.md](../slices/0004-m0-gate-hardening.md)

## Rows verified

| Row | Title | Evidence |
|---|---|---|
| LIB-TOOL-009 | Area table is validated | The 4 cited `lib_tool_009_*` tests meet all 3 criteria. The Linux mutants report shows 0 missed. |
| LIB-TOOL-010 | Every gate problem reaches the user | The 6 cited `lib_tool_010_*` tests meet all 4 criteria: the exact error shape, one line per problem, escaping, sorted orphans, independence from listing order, and the binary's stderr. The Linux mutants report shows 0 missed. |

## Rows left open

| Row | Status kept | Reason |
|---|---|---|
| LIB-TOOL-005 | implemented | F1: a PASS line inside a nested, mismatched or indented fenced block still counts, which gives a false PASS. That breaks criterion 1. |
| LIB-TOOL-006 | implemented | F2: a bad `verification` path on a non-verified row is not reported (criterion 1). |

## Gates run (by the verifier, on Windows)

All pass:
- fmt;
- clippy (`-D warnings`);
- `cargo test --workspace --all-features` and `cargo test -p xtask`;
- rustdoc (`-D warnings`);
- `matrix check` (149 rows);
- `cargo deny check`;
- hook tests (39).

## Findings and the complete prescription

### F1 (Medium, LIB-TOOL-005): fenced lines are counted as verdict lines

`verdict_lines` flips the fence state on any line that starts with ```` ``` ```` or `~~~`, and it never recognises an indented fence. Six probe records whose only PASS line sits inside a CommonMark fence passed the row. They used:
- `~~~` inside ```` ``` ````, and the reverse;
- ```` ``` ```` inside ```` ```` ````;
- an info-string line inside an open fence;
- fences indented 2 and 3 spaces.

A real FAIL line next to a fenced PASS example was reported as "2 verdict lines" instead of FAIL.

**P-005: implementation in `verdict_lines`.** A fence line, or a line inside a fence, is never a verdict line and never malformed.
1. **Opening fence:** 0 to 3 spaces of indent, then 3 or more of one character (all backticks or all tildes). It opens only when no fence is open, and it records the character and the run length.
2. **Closing fence:** 0 to 3 spaces of indent, then a run of the same character at least as long as the opening run, then only spaces or tabs.
3. **Content:** inside an open fence, every other line is content. That includes a fence of the other character, a shorter run, and a run followed by an info string.
4. **Unclosed fence:** it runs to the end of the document.

**Tests**, named `lib_tool_005_...` and cited:

| Test | What it checks | Mutations it kills |
|---|---|---|
| T1 | Each of the six probe inputs gives exactly the no-verdict baseline. | Closing on the other character, on a shorter run or on an info-string line; not recognising 1 to 3 spaces of indent. |
| T2 | Positive controls, each passing: <ul><li>```` ```` ```` / ```` ``` ```` / ```` ```` ````, then PASS;</li><li>```` ``` ```` … ```` ````` ```` (a longer closer), then PASS;</li><li>`~~~~` / `~~~~~`, then PASS;</li><li>a closing fence with trailing spaces or a tab, then PASS;</li><li>a closing fence indented 3 spaces, then PASS.</li></ul> | An equal-run requirement; an untrimmed closer; only unindented closers. |
| T3 | ```` ``` ```` indented 4 spaces is not a fence, so a following PASS counts. | Accepting any indent. |
| T4 | Two backticks or two tildes is not a fence. | A lower run threshold. |
| T5 | A FAIL line, then a ```` ```` ```` block holding ```` ``` ```` / PASS / ```` ``` ````, gives exactly the single "verdict FAIL, not PASS" problem. | |
| T6 | `lib_tool_005_line_numbers_count_every_line_of_the_record` gains a nested fence that holds a malformed look-alike. | |

The PASS evidence is the Linux `mutants (PR diff)` report at the fix head, with 0 missed and 0 timeout in `verdict_lines` and its helpers.

### F2 (Low, LIB-TOOL-006): a bad `verification` path on a non-verified row is not reported

`check_path` runs on `verification` only for verified rows. The probes `implemented` + `../outside.md`, `implemented` + `/etc/passwd` and `tests-failing` + `C:/x.md` each reported only "cites a verification record".

**P-006:**
1. Run `check_path` on `verification` whenever the field is present, at every status. On a verified row it must still be reported exactly once.
2. Add a `lib_tool_006_...` test for every status except `verified`, with one path from each rejection class: empty, `/x`, `a\b`, `C:/x`, `a//b`, `./a`, `../a`. The problems must contain both the rejection and the "cites a verification record" problem, and the path must never be asked of the `Repo`.
3. The PASS evidence is the Linux PR mutants job at the fix head, with 0 missed.

### F3 (Low, LIB-QA-007 doc): the triage document is stale

`docs/quality/mutants-triage.md` lists 4 unviable mutants. The head has 5:
- `fetch.rs:110:5`;
- `matrix.rs:530:5` (`load`);
- `matrix.rs:775:5` (`verdict_lines`, new; structural, because `VerdictLine` has no `Default`);
- `matrix.rs:1144:5` (`ready`, moved);
- `matrix.rs:1185:5` (`load_from`, moved).

The fix: update the document, citing the run at the fix head.

## Also confirmed

- All eight lead-added verdict lines match the verdicts their records state.
- The adjusted LIB-TOOL-001 tests are not weakened: the `#[test]` counts are unchanged and nothing was deleted.
- `XTASK_ROOT` cannot make the CI gate pass wrongly: CI never sets it, `xtask::run` does not read it, and tests set it only on child processes.
- The output is deterministic: rows come in file order, orphans are sorted, and two runs are byte-equal.

## Follow-ups suggested by the verifier

- Pin D5 (a malformed line is reported once per citing row) with a test. Folded into fix round 1.
- `matrix check` should say which root it checked when `XTASK_ROOT` overrides it (LIB-TOOL-013).
- Parse and read errors from `load_from` (TOML errors span several lines) should also be one per line (LIB-TOOL-014).
