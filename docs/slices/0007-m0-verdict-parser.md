---
slice: 0007
name: m0-verdict-parser
state: in-progress
step: verification
rows: [LIB-TOOL-005, LIB-TOOL-015]
mode: pr
---

# Slice 0007: a CommonMark parser decides which verdict lines count

## Scope

The gate reads `- Row verdict: <ROW-ID>: PASS` lines in verification records, and it
must ignore lines that rendered Markdown hides. Today a hand-written fence model decides
which lines are hidden, and verification 0004 failed it twice. This slice replaces it
with the CommonMark parser `pulldown-cmark`, as ADR 0008 records.
- **LIB-TOOL-005:** this closes P-005b (fences inside list items, F5).
- **LIB-TOOL-015:** this closes it as well (HTML blocks and comments, and a backtick in
  a fence's info string).

| Row | Title | Status at start |
|---|---|---|
| LIB-TOOL-005 | Verification records carry a per-row verdict the gate reads | implemented (stopped after its second FAIL in 0004; route A chosen) |
| LIB-TOOL-015 | Verdict lines hidden by Markdown do not count | planned |

## Out of scope

- **GFM extensions** (tables, strikethrough, autolinks, task lists): none of them can
  hide a line that starts with `- Row verdict:` (ADR 0008).
- **Any change to the verdict format or to the problem texts.**

## Normative basis

None: these are tooling rows. The criteria are the lead's, from P-005b in
`docs/verification/0004-m0-gate-hardening-round2.md` and from the LIB-TOOL-015 seeds.
CommonMark 0.31.2 is the reference for what is hidden.

## Design (fixed by the lead before the tests)

- **D1, the parser.** `pulldown-cmark` 0.13 with `default-features = false`, in
  `xtask/Cargo.toml` only, under ADR 0008. It parses with `Options::empty()`, and
  `cargo deny` must pass.
- **D2, hidden ranges.** Byte ranges come from `into_offset_iter`. They cover:
  - every `CodeBlock` (from `Start` to `End`, fenced or indented);
  - every `HtmlBlock` (from `Start` to `End`);
  - every `Html` and `InlineHtml` event.
- **D3, candidates.** A candidate is a source line that starts at column 0 with
  `- Row verdict:`. It counts only when its first byte is outside every hidden range.
  - A counted candidate is either a verdict line or a malformed one, as today. The
    problem texts and the D5 rule (a malformed line is reported once per citing row)
    are unchanged.
  - Lines are split as today, so CRLF behaves as before and line numbers stay 1-based.
- **D4, the old code goes.** `fence` and the hand-written fence state are removed. The
  `VerdictLine` and `check_record` interfaces stay, unless the implementer shows that a
  simpler shape kills the same mutants.
- **D6 (fail-safe for comments).** CommonMark can end an HTML range before a `<!--`
  inside it is closed, while a browser keeps the comment open. A `<!--` inside an
  `Html` or `InlineHtml` range and not closed within it hides everything up to the end
  of the next `-->`, or to the end of the record. Content inside `<details>` still
  counts, because a reader can expand it. The test engineer found this case.
  - `<!-->` and `<!--->` are complete comments (CommonMark §6.6, and browsers): the
    search for the closing `-->` starts at the opener's third byte.
  - D6 also applies inside `InlineHtml` (for example `<span title="<!--">`). This hides
    more than a browser would, which is the safe direction.

- **D7 (tab after a closer).** pulldown-cmark 0.13.6 does not accept a tab after a
  closing fence; CommonMark §4.5 does. Before parsing, the gate turns trailing tabs into
  spaces, keeping every length (`trailing_tabs_as_spaces`). The lead accepted this, and
  it is recorded in ADR 0008.

## Tests

There are 37 new tests, all in `xtask/tests/verdict_markdown.rs`. That target sorts after `function_scan`, and none of its tests scan the real repository. Every case also runs with CRLF.

| Tests | Row | Criterion |
|---|---|---|
| 18 `lib_tool_005_*` | LIB-TOOL-005 | <ul><li>P-005b probes P1, P1b, P6, P8 (exactly FAIL) and P9 (exactly no-verdict);</li><li>list-item and nested-item fences, closers judged relative to the item, and the T8 controls;</li><li>opener and closer look-alikes, the line after a closer, and F5 (U+00A0);</li><li>block quotes and indented or tab-indented code;</li><li>extension syntax not hiding;</li><li>malformed lines inside and outside hidden ranges, and D5;</li><li>pinned problem texts;</li><li>a source-inspection test for D1 and D4 (`pulldown_cmark`, `Options::empty`, no `fn fence(`, the dependency only in `xtask`).</li></ul> |
| 19 `lib_tool_015_*` | LIB-TOOL-015 | <ul><li>HTML comments and HTML blocks of types 1 to 7;</li><li>seeds P2 and P7, the latter with and without the blank line;</li><li>controls after closed comments, ended blocks and tilde or backtick info strings;</li><li>D6: unclosed comments in HTML events, where the region ends at the end of `-->`, a comment with no closer, inline HTML, short comments `<!-->`, `<!--->` and `<!---->`, and openers outside HTML events or in `<details>`.</li></ul> |

Existing tests: no expectation changed. T1 to T6 and D5 hold under CommonMark. The `verdict_lines` test helper in `gate_hardening.rs` gains a doc paragraph: it models top-level fences only, and still agrees on all 15 real records.

The test engineer found one equivalent mutant: a region ending at the start rather than the end of `-->`. No byte of `-->` can begin a candidate, so it is to be triaged.

## Log

- 2026-11-08 research: skipped, because these are tooling rows. The lead wrote ADR 0008
  (accepted; the maintainer chose route A and a real Markdown parser) and the
  acceptance criteria.
  - LIB-TOOL-005 keeps its status, `implemented`, with criterion 1 rewritten and three
    criteria added (P-005b).
  - LIB-TOOL-015 is now `analyzed`.
- 2026-11-08 tests: 26 tests in `xtask/tests/verdict_markdown.rs`. 13 are red: the
  false PASS on P1, P1b, P6, P8, P9, P2 and P7, and hidden HTML. 13 are controls and
  pins.
  - The lead's decisions: P7 and LIB-TOOL-015 criterion 3 are reworded to match
    CommonMark, and D6 is added (with a new criterion 4 on LIB-TOOL-015).
  - The source-inspection test stays.
- 2026-11-08 tests: 37 tests in total (26, then 8 for D6, then 3 for D6 closing).
  - The lead confirmed the red state below.
  - Both rows are now `tests-failing`.
- 2026-11-08 implementation: `hidden_ranges` (D2 and D6), with D6 closers found by one
  `match_indices` pass and a binary search. `verdict_lines` sweeps sorted ranges, and
  `fence` is gone.
  - D7 covers the tab after a closer. The implementer found it through existing T2
    tests; without it they give a false PASS.
  - pulldown-cmark 0.13.6 adds bitflags, memchr and unicase, and `cargo deny` passes.
  - A local cargo-mutants run gave 25 mutants: 17 caught, 8 unviable, 0 missed. The
    known equivalent mutant is gone.
  - The lead applied the README rule 5 text.
  - Follow-up: a lone-CR test for D7 goes to the test engineer.
- 2026-11-08 D7 tests: 3 tests (a tab after a closer, a lone CR ends the closer line,
  non-trailing tabs keep their meaning). They kill three mutants: CR dropped from the
  split, the normalisation removed, and every tab rewritten. Both rows are now
  `implemented`.
