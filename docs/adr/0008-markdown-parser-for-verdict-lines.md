# 0008. A CommonMark parser decides which verdict lines count

- Status: accepted (the maintainer approved route A on 2026-11-08: "Ok lets get a realy markdown parser")
- Date: 2026-11-08
- Matrix rows: LIB-TOOL-005, LIB-TOOL-015

## Context

The gate (`cargo xtask matrix check`) reads each verified row's verification record. The record must hold exactly one line `- Row verdict: <ROW-ID>: PASS` for that row.
- **What the record shows:** example verdict lines inside code blocks are what readers see as examples. They must not count, or a record could quote a PASS and hide its real FAIL.
- **The current code:** the gate decides what is "inside a code block" with a hand-written, line-based model of CommonMark fences (`fence`, `verdict_lines`).
- **Its history:** verification 0004 failed LIB-TOOL-005 twice with that approach. Round 1 found mismatched and nested fences (F1). Round 2 found fences inside list items, which CommonMark closes when the item ends (F4).
- **More of the same:** LIB-TOOL-015 lists further cases that hide a line in rendered Markdown: HTML blocks and comments, and a backtick fence opener whose info string holds a backtick.

Each fix modelled one more CommonMark rule, and the verifier then found the next one. The verifier's prescription offered three routes:
- A, a real parser;
- B, list items tracked by hand;
- C, an ambiguity rule.

## Decision

- **The parser:** `xtask` depends on [`pulldown-cmark`](https://crates.io/crates/pulldown-cmark) 0.13 with `default-features = false`.
  - It is a CommonMark 0.31.2 parser, used by rustdoc and mdBook.
  - With no default features it adds `bitflags`, `memchr` and `unicase` (all MIT-licensed, alone or as a dual-licence option).
  - The dependency is only in `xtask`, which is `publish = false`. The `sacm` library does not gain it.
- **Hidden ranges:** the gate parses each verification record with `Options::empty()`, which is core CommonMark with no extensions, and uses the parser's source offsets (`into_offset_iter`). As amended by amendment 1, the hidden lines are only those inside code blocks (fenced or indented) and inside HTML blocks that are one closed comment. All other raw HTML is a problem, not a hidden range.
- **Which lines count:** a source line is a candidate when it starts at column 0 with `- Row verdict:`. A candidate counts, either as a verdict line or as a malformed one, only when its first byte lies outside every hidden range.
  - The verdict format and the problems the gate reports are unchanged (`docs/matrix/README.md` rule 5).
  - Line numbers in problems stay 1-based source lines.
- **Removed:** the hand-written fence model (`fence` and its callers).

## Consequences

- **A known parser deviation, worked around.** pulldown-cmark 0.13.6 does not accept a
  tab after a closing code fence. CommonMark 0.31.2 §4.5 does, and so does cmark, which
  GitHub uses. Left alone, this gives a false PASS: a closer followed by a tab leaves the
  fence open, which hides a real FAIL.
  - Before parsing, the gate turns each tab in a line's trailing run of spaces and tabs
    into a space (`trailing_tabs_as_spaces`). Lines end at `\n` or `\r`, as in §2.1.
  - The change keeps every length, so offsets and line numbers are unchanged. The parser
    is still the only judge of what is hidden.
  - In core CommonMark, a trailing tab and a trailing space differ only in the inline
    hard-break rule (§6.7), which hides nothing.
  - The lead accepted this in slice 0007. If a later release fixes pulldown-cmark, the
    step becomes a no-op, and it can go.
- **Easier:**
  - Code blocks are decided by a CommonMark parser, including list items, block quotes, nesting and info strings. With strict records (amendment 1), the gate refuses anything else that a renderer could show differently, so it never has to model what a browser hides.
  - Less hand-written code to mutation-test.
- **Harder:**
  - One more dependency to keep current. `cargo deny` checks it, and Dependabot proposes updates.
  - A behaviour change in a future pulldown-cmark release would change the gate. The tests pin the cases that matter, so a change shows up as a red test, not as a silent difference.
- **Committed to:**
  - Core CommonMark. GitHub renders records with GFM extensions (tables, strikethrough, autolinks, task lists). None of them can hide a line that starts with `- Row verdict:`, so the difference does not matter here.
  - Records are kept strict (amendment 1, below), so the gate never has to model what a browser hides.

## Alternatives considered

- **B, track list items by hand (CommonMark §5.2–5.3).** It would add more hand-written state of the kind that has already failed twice, and it would still leave HTML blocks and info strings (LIB-TOOL-015) open.
- **C, report ambiguous fences.** Smallest and fail-safe, but it bans some legitimate record layouts, and it does not close LIB-TOOL-015.
- **Other parsers:**
  - `markdown` (markdown-rs) 1.0: MIT, one dependency, and CommonMark-exact with line positions. It is the fallback if pulldown-cmark ever falls short; it is less widely used in Rust.
  - `comrak` 0.55: BSD-2-Clause, about twelve dependencies, and focused on GFM. Heavier than needed.

## Amendment 1: strict records (2026-11-08)

- Status: accepted. After the third FAIL of LIB-TOOL-005 (verification 0007, round 1), the maintainer chose "strict records" and an in-slice differential check.
- **Context.**
  - Verification 0007 compared the gate against three CommonMark references on about 50,000 generated records. It found a second pulldown-cmark deviation (the end of a type-1 HTML block).
  - Its other findings were not deviations:
    - a browser hides comment text that CommonMark does not, and the reverse;
    - lone CR line endings;
    - verdict text that a reader sees but the gate ignores: indented items, and text inside HTML blocks.
  - Modelling every renderer is open-ended.
- **Decision: anything ambiguous is reported as a problem, not modelled.**
  - The parser still decides code blocks (fenced and indented) and closed comments. Those are the only hidden ranges.
  - Raw HTML in a record is a problem, except two things:
    - a closed comment that is complete within its own HTML block or inline event;
    - the inline tags `<br>`, `<ul>`, `</ul>`, `<li>`, `</li>`, which the real records use in tables. They match case-insensitively; `<br>` may be written `<br/>` or `<br />`.
  - These are also problems:
    - a visible verdict-like line that is not at column 0 (indented, nested or block-quoted);
    - a lone CR;
    - a leading byte-order mark.

  The slice 0007 brief lists the exact problem texts.
- **Consequence.**
  - A record passes only when it is plain Markdown.
  - pulldown-cmark deviations inside raw HTML can no longer give a false PASS.
  - The earlier comment fail-safe (an unclosed comment hides nothing) is superseded: an
    unclosed comment is disallowed HTML.
  - A repeatable differential test against markdown-rs (`markdown` 1.0, a dev-dependency of `xtask` only) checks the parser's code-block and comment decisions on generated records.

## Amendment 2 (2026-11-09): the differential test is removed

The markdown-rs differential test and the `markdown` dev-dependency were removed in slice 0012 (ADR 0006 amendment 2). The pulldown-cmark reader and its tests in `xtask/tests/verdict_markdown.rs` remain.
