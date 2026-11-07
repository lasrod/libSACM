# Verification 0004, round 2: m0-gate-hardening (after fix round 1)

- Date: 2026-11-08
- Verdict: **FAIL** (partial). LIB-TOOL-006 passes. LIB-TOOL-005 fails a second time. LIB-TOOL-009 and LIB-TOOL-010 show no regression.
- Row verdict: LIB-TOOL-006: PASS
- Row verdict: LIB-TOOL-005: FAIL
- Verifier: `sacm-verifier` (independent; read-only)
- Commit inspected: `1655cbf15289fca2e01652bc78f62db1de29d692` (PR #10), `git diff e216a82..1655cbf`
- Contract: the [round 1 record](0004-m0-gate-hardening.md), P-005 items 1 to 4 with tests T1 to T6, P-006 items 1 to 3, and F3. The verifier checked them item by item.
- Linux evidence, downloaded by the verifier:
  - ci.yml [42390257629](https://github.com/lasrod/libSACM/actions/runs/42390257629): all 11 jobs succeeded. In `mutants (PR diff)`, 49 mutants: 48 caught, 0 missed, 0 timeout, 1 unviable.
  - mutants.yml [42390257623](https://github.com/lasrod/libSACM/actions/runs/42390257623): 245 mutants: 240 caught, 0 missed, 0 timeout, 5 unviable. That is exactly the set listed in `docs/quality/mutants-triage.md`.

## Round-1 prescription

| Item | Result |
|---|---|
| P-005.1 to P-005.4 (opener, closer, content, unclosed fence) | Done (`fence`, `verdict_lines`) |
| T1 to T6 and the D5 test | Present and adequate. T2 has no negative case for whitespace other than a space or tab after a closer (F5). |
| P-006.1 to P-006.3 | **Done.** A verified row is reported exactly once (mutation H11 killed). All 7 non-verified statuses are crossed with the 7 rejection classes, each checking both problems and that the path is never read. The Linux PR job shows 0 missed. |
| F3 | Closed. The unviable list matches the Linux run, and the lead has filled in the run ids. |

## Rows verified

| Row | Title | Evidence |
|---|---|---|
| LIB-TOOL-006 | Cited paths stay inside the repository | P-006 items 1 to 3 are met. The Linux PR mutants job at `1655cbf` shows 0 missed. The surviving mutation H12 is equivalent under the criteria: it calls `check_record` instead of `check_path` on a non-verified row, and `check_record` runs `check_path` first, so a rejected path is still never read. |

LIB-TOOL-009 and LIB-TOOL-010 stay verified (round 1). There is no regression at `1655cbf`: the diff touches only `check_evidence`, `fence` and `verdict_lines`, all their tests pass, and the Linux mutants run shows 0 missed.

## Rows left open

| Row | Status kept | Reason |
|---|---|---|
| LIB-TOOL-005 | implemented | F4 (Medium) and F5 (Low), below. This is the row's second FAIL, so it stops for the maintainer's decision. |

## Findings

### F4 (Medium, LIB-TOOL-005): a fence inside a list item gives a false PASS

The verifier owns this as a miss in round 1: P-005 described only top-level fences. CommonMark ends a fence that sits inside a list item when that item ends. `verdict_lines` keeps the fence open, so its fence state drifts from what the reader sees. It then hides the real FAIL verdict and counts a PASS example.

In every probe below, the gate passes the row. A CommonMark parser (markdown-it-py 4.2.0) renders the FAIL verdict visible, or the PASS inside code.

| Probe | Input |
|---|---|
| P1 | `"- example:\n  ```\n  log\n- Row verdict: LIB-TOOL-001: FAIL\n\n```\n- Row verdict: LIB-TOOL-001: PASS\n```\n"` |
| P1b | `"1. Probe:\n\n   ```\n   log\n\n- Row verdict: LIB-TOOL-001: FAIL\n\nExample:\n\n```\n- Row verdict: LIB-TOOL-001: PASS\n```\n"`. This has the same form as the real record `0001-m0-tooling-verification-round4.md`. |
| P6 | `"- a\n  ```\n  x\n     ```\n- Row verdict: LIB-TOOL-001: FAIL\n\n```\n- Row verdict: LIB-TOOL-001: PASS\n```\n"` |
| P8 | `"- a\n  - b\n    ```\n    x\n  ```\n- Row verdict: LIB-TOOL-001: FAIL\n\n```\n- Row verdict: LIB-TOOL-001: PASS\n```\n"` |
| P9 | `"- a\n  ```\n  x\n```\n- Row verdict: LIB-TOOL-001: PASS\n```\n"` |

Block quotes do not cause this: `fence` never matches a `>` line.

### F5 (Low, LIB-TOOL-005): untested whitespace after a closer

Mutation H16, replacing the closer's space-and-tab trim with `trim()`, survives the whole suite. The code is correct today: a closer followed by U+00A0 gives no verdict, which matches CommonMark.

### Doc (Low)

README rule 5 says "Fences follow CommonMark", which F4 makes untrue.

## P-005b: the complete remainder for LIB-TOOL-005

**Obligation:** if CommonMark 0.31.2 puts a line inside a fenced code block, that line is never a verdict line and never malformed. If the gate cannot decide, it reports a problem, so the row cannot PASS. This covers fences inside list items, nested or not, and closers at any indent.

**Not required here** (see LIB-TOOL-015): a backtick in a backtick fence's info string, HTML blocks and comments, tab-stop arithmetic beyond "a tab is not fence indent", and lazy continuation.

**The maintainer chooses one route, and it is recorded in an ADR or design note.**
- **A.** A CommonMark parser in `xtask` only, for example pulldown-cmark with source offsets. It maps fenced-code byte ranges to source lines. This needs the maintainer's approval for a new dependency, and `cargo deny` must pass. It would also close most of LIB-TOOL-015.
- **B.** Track list-item containers by hand, per CommonMark §5.2 and §5.3:
  - bullet markers `-`, `+` or `*`, or ordered markers `1.` or `1)` with up to 9 digits;
  - 0 to 3 spaces before the marker;
  - a content indent of the marker width plus 1 to 4 spaces;
  - the item ends at a non-blank line indented less than its content indent;
  - a fence closes when its item ends, and closers are measured relative to the content indent.
- **C.** Conservative, and the smallest. A fence opened with n ≥ 2 spaces of indent makes the record ambiguous if a non-blank line inside it has fewer than n leading spaces, a would-be closer included.
  - The ambiguity is reported as `verification record <p>:<line>: …`, once per citing row (D5).
  - The verifier checked that this catches P1, P1b, P6, P8 and P9.
  - Under C, the T1 cases with 2- and 3-space fences and the T2 case "opener indented 2 spaces, closed unindented" become ambiguity problems. The reason must be recorded. None of them can ever give a PASS.

**Tests,** named `lib_tool_005_...`:
1. **T7: P1, P1b, P6, P8 and P9 never pass the row.**
   - Under A or B: P1, P1b, P6 and P8 each give exactly the FAIL problem, and P9 gives the no-verdict baseline.
   - Under C: each gives the ambiguity problem at the right line.
   - It must kill these mutations: not closing at the end of the item (P1); a wrong content indent for an ordered marker (P1b); measuring closers by absolute indent (P6); no nesting (P8); under C, skipping fence lines in the check (P9); under C, a threshold of n ≥ 3 (P1).
2. **T8: controls that must pass the row.**
   - A closed list-item fence, then PASS.
   - The same, with a blank line inside the fence.
   - Under C, content indented exactly n spaces.
   - Every real record still passes. C must not flag `0001-round4`.
   - It must kill: blank lines treated as under-indented; `<` changed to `<=`; every list-item fence flagged.
3. **Route A only:** opener and closer lines are neither verdict lines nor malformed, and the line after the closer counts. Check this with look-alikes on the opener's info string, on the closer, and on the next line.
4. T1 to T6 and D5 stay. Any expectation that changes under C carries its reason, in the test comment and in the slice log.
5. **Evidence:** the Linux `mutants (PR diff)` run at the fix head shows 0 missed and 0 timeout in `verdict_lines`, `fence` and any new helper. Any unviable mutant is listed in the triage document.
6. **F5:** `"```\ncode\n```\u{A0}\n{PASS}\n"` gives exactly the no-verdict baseline. This kills H16.
7. README rule 5 and the `verdict_lines` doc comment describe the rule that is actually implemented, and name the LIB-TOOL-015 exclusions.

## Gates run (by the verifier, on Windows)

All pass:
- fmt;
- clippy with `-D warnings`;
- `cargo test --workspace --all-features` and `cargo test -p xtask`. `gate_hardening` has 45 tests;
- rustdoc with `-D warnings`;
- `matrix check` (152 rows);
- `cargo deny check`;
- hook tests (39).

## Hand mutations (scratch copy only)

All of these were run in a scratch copy under `target/` and were killed:
- `run >= 3` → `> 3`, and → `>= 2`;
- `trim_start_matches(' ')` → `trim_start()`;
- `check_path` dropped on a non-verified row;
- closer `len >= run` → `==`, and → `>`;
- the rest of the closer line left untrimmed, or ignored;
- the run recorded as 3;
- `~` dropped;
- a verified path checked twice;
- the closer's character ignored;
- indent `> 3` → `> 4`;
- a closer accepted at any indent.

Two survived:
- H12, which is equivalent under the criteria;
- H16, which is F5.

## Follow-ups

- **LIB-TOOL-015 stays the right home** for a backtick in an info string and for HTML blocks and comments. The verifier asked for two more seeds there, P2 and P7 (in the row). If route A is chosen, most of LIB-TOOL-015 can close in the same fix.
- **Optional row (the lead's call):** pin that a valid `verification` path on a non-verified row is never read (H12).
