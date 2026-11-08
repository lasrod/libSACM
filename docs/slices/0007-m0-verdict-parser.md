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
- **D6 (fail-safe for comments): superseded by D8 in fix round 1 (strict records).** CommonMark can end an HTML range before a `<!--`
  inside it is closed, while a browser keeps the comment open. A `<!--` inside an
  `Html` or `InlineHtml` range and not closed within it hides everything up to the end
  of the next `-->` that itself lies inside an `Html` or `InlineHtml` range, or to the end
  of the record. A `-->` in text, a code span or a code block is rendered escaped
  (`--&gt;`), so it does not close the comment. This was amended after the Copilot review
  of PR #14. Content inside `<details>` still
  counts, because a reader can expand it. The test engineer found this case.
  - `<!-->` and `<!--->` are complete comments (CommonMark §6.6, and browsers): the
    search for the closing `-->` starts at the opener's third byte.
  - D6 also applies inside `InlineHtml` (for example `<span title="<!--">`). This hides
    more than a browser would, which is the safe direction.

- **D7 (tab after a closer).** pulldown-cmark 0.13.6 does not accept a tab after a
  closing fence; CommonMark §4.5 does. Before parsing, the gate turns trailing tabs into
  spaces, keeping every length (`trailing_tabs_as_spaces`). The lead accepted this, and
  it is recorded in ADR 0008.

- **D8 (strict HTML), fix round 1.** Each `Html` or `InlineHtml` event is a problem unless it is one of these:
  - part of an `HtmlBlock` whose whole source, with trailing whitespace trimmed, is one closed comment: it starts with `<!--`, it ends with `-->`, and neither `-->` nor `--!>` occurs before that end;
  - an `InlineHtml` event that is such a comment;
  - an `InlineHtml` event that is one of `<br>`, `<br/>`, `<br />`, `<ul>`, `</ul>`, `<li>` or `</li>`, matched case-insensitively.

  The problem is `verification record <p>:<line>: raw HTML other than a closed comment or <br>, <ul>, <li> is not allowed`, reported once per line that holds such HTML. `<line>` is the first line of the event. Hidden ranges are now only code blocks and allowed comments.
- **D9 (look-alikes), fix round 1.** Take a line outside every hidden range. Strip any run of spaces, tabs, `>` and list markers (`-`, `*`, `+`, `1.`, `1)`) from its start. If the rest starts with `Row verdict:` and the line is not a column-0 candidate, the line is a problem: `verification record <p>:<line>: a verdict-like line is not at column 0`.
- **D10 (line endings), fix round 1.**
  - A lone CR anywhere is a problem: `verification record <p>:<line>: lone carriage return; use LF or CRLF line endings`. `<line>` is the line, by LF/CRLF splitting, that holds the CR.
  - A leading U+FEFF is a problem: `verification record <p>: starts with a byte-order mark`.
- **D11 (reporting), fix round 1.**
  - A record's problems are reported once per citing row (as D5), in line order, before the verdict problem. The BOM problem comes first.
  - The verdict is still judged on the counted candidates, as before.
- **D12 (differential test), fix round 1.**
  - A dev-dependency `markdown` 1.0 (markdown-rs) serves as an independent CommonMark reference.
  - A seeded generator of at least 20,000 records covers:
    - fences;
    - containers;
    - HTML of types 1 to 7 and comments;
    - blank, paragraph, setext and thematic lines, and link reference definitions;
    - column-0 and look-alike candidates;
    - LF, CRLF and CR endings.
  - For every record the gate accepts as strict, the gate's verdict problems must equal those computed from the reference's view of which column-0 candidates are visible.
  - Every disagreement is fixed or adjudicated in writing.

- **D13, fix round 1.** These decisions refine D8 to D11:
  - **(a)** An allowed inline tag is allowed only when non-whitespace text comes before
    it on its line. A tag at the start of a line can start an HTML block in cmark-family
    renderers, such as GitHub's.
  - **(b)** A blank line inside an HTML block is not reported.
  - **(c)** On one line, problems come in this order: raw HTML, lone CR, malformed.
  - **(d)** A lone CR gives one problem per line.
  - **(e)** After a leading BOM, line 1 is not a column-0 candidate.
  - **(f)** A counted candidate whose id is not a well-formed row id is malformed. This
    closes an existing gap: `- Row verdict:  LIB-TOOL-001: PASS` was read as the id
    ` LIB-TOOL-001` and silently ignored.

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
- 2026-11-08 CI at `88894ef`: green. Full mutants: 445 mutants, 427 caught, 0 missed,
  0 timeout, 18 unviable (seven new `Range` constructors in `hidden_ranges`, which are
  structural).
  - The Copilot review of PR #14 left one valid comment: D6 closed a region at a `-->`
    in a code span or code block, which a browser renders escaped. D6 is amended so
    that only a `-->` in raw HTML closes.
  - Tests go to the test engineer and code to the implementer.
- 2026-11-08 Copilot fix, tests: 7 new D6 tests (a `-->` in a code span, a fenced or
  indented code block, or text does not close; a `-->` in a later HTML block or in
  inline HTML does close). 5 are red today. There are now 47 tests in
  `verdict_markdown.rs`.
  - Two existing D6 tests pinned the bug: they closed comments with a `-->` in text.
    Their records now use a raw-HTML `<!-- end -->` or a type-6 line, and their
    expected results are unchanged.
  - LIB-TOOL-015 goes back to `tests-failing`.
- 2026-11-08 Copilot fix, implementation: `hidden_ranges` now collects `-->` closers
  only from `Html` and `InlineHtml` event ranges. All 47 `verdict_markdown` tests pass.
  - A local cargo-mutants run of the fix gave 17 mutants: 10 caught, 7 unviable,
    0 missed.
  - Hand mutants restricting the closers to `Html` only or to `InlineHtml` only are
    each killed by a control test.
  - The README text came from the implementer, and the lead applied it.
  - LIB-TOOL-015 is `implemented` again.
- 2026-11-08 verification round 1, at `123c71e`:
  [record](../verification/0007-m0-verdict-parser.md). Both rows FAIL.
  - F1: a second pulldown-cmark deviation, the type-1 HTML block end.
  - F2: lone CR and leading BOM.
  - F3: D6 over-hides.
  - F4: visible look-alikes that the gate ignores.
  - F5: two surviving hand mutants. F6: the documents overclaim.
  - This is LIB-TOOL-005's third FAIL. The slice stops for the maintainer's decisions
    on P-5 and P-6.
- 2026-11-08 the maintainer chose strict records (ADR 0008 amendment 1), with the
  differential check (P-2) in this fix.
  - The lead added D8 to D12; D6 is superseded.
  - Fix round 1 starts with the tests.
- 2026-11-08 fix round 1, tests (first pass):
  - New tests for D8 to D11 and P-7. Existing HTML tests are updated to the D8
    outcome, and 14 of them are renamed.
  - Three D9 changes in `gate_hardening.rs`.
  - The differential test `verdict_differential.rs` runs 24,000 records against
    markdown-rs, in about 5 s. It finds 917 disagreements today.
  - Adjudications are recorded in the test: CRLF normalisation, the laziness class
    (§4.4 Ex. 113, §4.6 Ex. 187), and setext laziness.
  - The lead decided D13.
  - Process: the test engineer wrote scratch files outside its scope through Bash.
    They were removed, and the instruction was repeated.
- 2026-11-08 fix round 1, tests (second pass, D13):
  - 3 new tests, and exact ordering throughout.
  - Malformed ids follow `parse_id` and README rule 2.
  - The differential laziness adjudication is narrowed to its indented-code shape. It
    excludes 250 of 24,000 records, and setext excludes 59.
  - `verdict_markdown` has 64 tests, 39 of them red. The differential test is red, and
    5 tests in `gate_hardening` are red.
  - LIB-TOOL-005 criterion 3 is reworded for D13 f.
  - Both rows are now `tests-failing`.
- 2026-11-08 fix round 1, implementation:
  - `scan_record` and `RecordScan` replace `verdict_lines`. The hidden lines are only
    code blocks and closed-comment HTML blocks.
  - D6 is removed, and D7 is kept. D8 to D11 and D13 are implemented.
  - All red tests pass, and the differential test gives 0 disagreements on the default
    seed (17,901 compared, 5,855 refused).
  - A local cargo-mutants run gave 52 mutants: 51 caught, 1 unviable. One hand mutant
    survived (H1, a multi-digit ordered marker).
  - The lead applied the README rule 5 text and corrected ADR 0008 for amendment 1.
  - Follow-ups for the test engineer:
    - make the differential test robust against reference panics (seeds 1 and
      123456789) and the code-span `<br>` laziness (seeds 7 and 42);
    - add a test for H1;
    - pin the look-alike order (D13 c: last on its line).
  - Process: the implementer also wrote through Bash outside its scope (a probe
    project in the scratchpad, files directly under `target/`). They were removed, and
    the instruction was repeated.
- 2026-11-08 fix round 1, test follow-ups:
  - The differential test catches markdown-rs panics (`to_html.rs:197`) and counts them
    as exclusions, bounded at 0.5%.
  - Two adjudications are widened (laziness after a container: indented code, an empty
    list item, or a tag in a code span; and setext laziness).
  - Seeds 1, 7, 42, 123456789, 2024 and 31337, at 100,000 records each, give 0
    disagreements. The default run is 20,000 records, about 7 s in debug.
  - Added: the H1 test (multi-digit ordered markers), and the order of a look-alike on
    its line (last).
  - Both rows are `implemented`.
