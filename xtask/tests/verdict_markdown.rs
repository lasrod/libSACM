//! Acceptance tests for which `- Row verdict:` lines count (slice 0007).
//!
//! - LIB-TOOL-005: a verdict-line candidate is a source line that starts at
//!   column 0 with `- Row verdict:`. It counts only when `CommonMark` 0.31.2
//!   puts it outside every code block, HTML block and inline HTML, as the
//!   parser of ADR 0008 (`pulldown-cmark` with `Options::empty()`) decides.
//!   This closes P-005b of `docs/verification/0004-m0-gate-hardening-round2.md`:
//!   the probes P1, P1b, P6, P8 and P9 (T7), the controls (T8), item 3 and F5.
//! - LIB-TOOL-015: lines inside closed HTML comments do not count, and a
//!   backtick fence "opener" whose info string holds a backtick is not an
//!   opener (seeds P2 and P7).
//!
//! # Strict records (slice 0007 fix round 1, D8 to D11)
//!
//! The maintainer chose strict records (ADR 0008 amendment 1): anything a
//! renderer could show differently is a problem, not something the gate
//! models. D6, the comment fail-safe, is superseded.
//! - **D8, strict HTML.** Each `Html` or `InlineHtml` event is a problem,
//!   once per line that holds one, unless it belongs to an HTML block that is
//!   one closed comment, is an inline closed comment, or is one of the inline
//!   tags `<br>`, `<br/>`, `<br />`, `<ul>`, `</ul>`, `<li>`, `</li>` in any
//!   case with non-whitespace text before it on its line. A closed comment
//!   starts with `<!--`, ends with `-->`, and holds no `-->` or `--!>` before
//!   that end. An allowed tag at the start of a line (after an indent of 0 to
//!   3) is a problem: cmark-family renderers, GitHub's among them, may start
//!   an HTML block there that hides the lines after it. A blank line inside
//!   an HTML block holds no HTML and is not reported. The hidden ranges are
//!   only code blocks and allowed comments, so a candidate inside other raw
//!   HTML counts.
//! - **D9, look-alikes.** A line outside every hidden range that, with its
//!   leading spaces, tabs, `>` and list markers removed, starts with
//!   `Row verdict:`, and that is not a column-0 candidate, is a problem.
//! - **D10, line endings.** A lone CR is a problem at the LF/CRLF line that
//!   holds it, once per line however many CRs it holds, and a leading
//!   byte-order mark is a problem. A candidate right after the BOM is not at
//!   column 0, so it does not count.
//! - **D11, reporting.** A record's problems are reported once per citing
//!   row, in line order, before the verdict problem, with the BOM problem
//!   first. On one line the order is raw HTML, lone CR, malformed; a
//!   look-alike never shares a line with a malformed line.
//! - **D14, closed inline comments (second Copilot review of PR #14):
//!   withdrawn after verification round 3.** D14 hid a line whose whole
//!   content lay inside one closed inline comment. Round 3 showed that this
//!   hides look-alikes that GitHub shows: a comment whose text ends in `-`,
//!   a GFM table cell, a GFM footnote and an image description. The
//!   maintainer chose route W: a closed inline comment is allowed (D8) but
//!   hides nothing, so a look-alike inside it is a D9 problem, as at
//!   `ee4ef31`. A column-0 candidate is unaffected either way: it starts a
//!   list item and so ends the paragraph that holds the comment.
//! - **Well-formed ids (decision 6 of the fix round).** A counted candidate
//!   whose id part is not a well-formed row id (`<SACM23|LIB>-<AREA>-<NNN>`,
//!   as `docs/matrix/README.md` rule 2 and the matrix gate define it) is
//!   malformed.
//!
//! Where an existing test's expectation changes under D8 to D10, its comment
//! says "slice 0007 D8: strict records" (or D9, D10).
//!
//! The gate is driven through `xtask::matrix::check` against an in-memory
//! [`Repo`], as in `gate_hardening.rs`. Every expectation is an exact,
//! ordered list of problems. The problem texts are built by the helpers below
//! and pinned once by `lib_tool_005_the_problem_texts_stay_as_they_are` and
//! `lib_tool_005_the_strict_record_problem_texts_are_exact`.
//!
//! On a raw-HTML problem's line: `pulldown-cmark` 0.13 emits one `Html` event
//! for each line of an HTML block, and with CRLF two (the text and the LF).
//! It also emits one for a blank line inside a type 1 to 5 block; that line
//! holds no HTML and gets no problem.
//!
//! # The `CommonMark` 0.31.2 rules these tests rely on
//!
//! Section numbers are those of the 0.31.2 specification; "Example n" is its
//! numbered example.
//! - §3.2 and Appendix A: block structure is decided before inline structure.
//! - §4.4: an indented code block is lines indented 4 or more columns; a tab
//!   counts to the next multiple of 4 (§2.2). A column-0 line ends it.
//! - §4.5: a fence opener is 0 to 3 spaces of indent and 3 or more backticks
//!   or tildes. A backtick opener's info string may not contain a backtick
//!   (Example 145); a tilde opener's may (Example 146). A closer is the same
//!   character, a run at least as long, 0 to 3 spaces of indent, and then only
//!   spaces or tabs; any other line is content (Example 147). The content of
//!   an opener indented N spaces loses up to N spaces of indent, so a less
//!   indented line is still content (Examples 132, 133). A fence may
//!   interrupt a paragraph (Example 140). An unclosed fence runs to the end of
//!   its container, or of the document.
//! - §4.6: HTML blocks of types 1 to 5 (`<pre>`, `<script>`, `<style>`,
//!   `<textarea>`; `<!--`; `<?`; `<!` and a letter; `<![CDATA[`) end at the
//!   line that holds their end marker, blank lines included (Examples 179,
//!   180). Types 6 (a known block tag such as `div`, `details`, `summary`,
//!   opening or closing) and 7 (any other complete tag alone on its line) end
//!   at a blank line, so a `</div>` line does not end them (Examples 186,
//!   189). Types 1 to 6 may interrupt a paragraph; type 7 may not (Examples
//!   185, 187). An HTML block may be indented up to 3 spaces.
//! - §5.1: a block quote continues on a line without `>` only by paragraph
//!   laziness; a fence inside it ends with it (Examples 128, 237).
//! - §5.2 and §5.3: a list item continues on a line indented at least its
//!   content indent (marker width plus the spaces after it), or by paragraph
//!   laziness. A fence or HTML block in the item ends with the item, and its
//!   closer is judged relative to the content indent (Examples 318, 324). A
//!   line starting `- ` and a non-blank character starts a bullet list item,
//!   which may interrupt a paragraph, so it is never paragraph continuation
//!   text and never lazy.
//!
//! Two consequences shape the tests. A candidate starts a list item at the
//! top level, so it can only be hidden by a code block or HTML block that is
//! open at the top level: a column-0 line always ends a list item's or block
//! quote's fence. And a candidate always interrupts a paragraph, so inline
//! HTML can never span one. The list-item, block-quote, indented-code and
//! inline-HTML tests therefore pin that those constructs end where `CommonMark`
//! ends them and hide nothing after.
//!
//! Fixture source text is written on one line with `\n` escapes on purpose: a
//! line of this file that began with `#[test]` inside a string literal would
//! be picked up by the real repository's orphan scan. Every verdict line in a
//! fixture names [`ROW`] or [`OTHER`], both defined by the fixture's matrix.
//!
//! This test target sorts after `function_scan`, and none of its tests scan
//! the real repository (see `docs/quality/mutants-triage.md`, slice 0005).

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use xtask::matrix::{
    Matrix, RENDERED_PATH, Repo, check, ignored_traced_tests, load, render, traced_tests,
};

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

/// An in-memory repository: relative path to file text.
#[derive(Debug, Default)]
struct FakeRepo {
    files: BTreeMap<String, String>,
}

impl FakeRepo {
    fn with(mut self, path: &str, text: &str) -> Self {
        self.files.insert(path.into(), text.into());
        self
    }

    /// `(file, name)` for every name `find` returns for a file, read as
    /// Python when its extension is `.py`.
    fn scan(&self, find: fn(&str, bool) -> Vec<String>) -> Vec<(String, String)> {
        let mut out = Vec::new();
        for (path, text) in &self.files {
            let python = Path::new(path)
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("py"));
            for name in find(text, python) {
                out.push((path.clone(), name));
            }
        }
        out
    }
}

impl Repo for FakeRepo {
    fn exists(&self, rel: &str) -> bool {
        self.files.contains_key(rel)
    }

    fn read(&self, rel: &str) -> Option<String> {
        self.files.get(rel).cloned()
    }

    fn traced_functions(&self) -> Vec<(String, String)> {
        self.scan(traced_tests)
    }

    fn ignored_functions(&self) -> Vec<(String, String)> {
        self.scan(ignored_traced_tests)
    }
}

const HEADER: &str = r#"
schema_version = 1
[[milestone]]
id = "M0"
title = "Foundation"
goal = "Build."
[[area]]
code = "TOOL"
prefix = "LIB"
title = "Tooling"
"#;

/// The row most tests are about.
const ROW: &str = "LIB-TOOL-001";
/// A second row, for records shared between rows.
const OTHER: &str = "LIB-TOOL-002";
/// The verification record the verified rows cite.
const RECORD: &str = "docs/verification/v.md";

/// `LIB-TOOL-001` -> `lib_tool_001`.
fn snake(id: &str) -> String {
    id.to_ascii_lowercase().replace('-', "_")
}

/// The traced test cited for row `id`.
fn test_name(id: &str) -> String {
    format!("{}_x", snake(id))
}

/// A verified row `id` with all of its evidence, citing [`RECORD`].
fn verified_row(id: &str) -> String {
    format!(
        "\n[[row]]\nid = \"{id}\"\narea = \"TOOL\"\nmilestone = \"M0\"\ntitle = \"Title\"\n\
         requirement = \"Requirement.\"\nsource = \"Source\"\nkind = \"tooling\"\n\
         status = \"verified\"\nacceptance = [\"a\"]\ncode = [\"src/lib.rs\"]\n\
         tests = [\"crates/x/tests/t.rs::{}\"]\nverification = \"{RECORD}\"\n",
        test_name(id)
    )
}

/// The problems the gate reports for the verified rows `ids`, which all cite
/// [`RECORD`], when the record reads `record`. The rendered page is in sync
/// and every other citation resolves, so only the record can give problems.
fn problems_for(ids: &[&str], record: &str) -> Vec<String> {
    let rows: String = ids.iter().map(|id| verified_row(id)).collect();
    let m: Matrix = load(&format!("{HEADER}{rows}")).expect("LIB-TOOL-005: test matrix parses");
    let mut tests = String::new();
    for id in ids {
        let _ = write!(tests, "#[test]\nfn {}() {{}}\n", test_name(id));
    }
    let repo = FakeRepo::default()
        .with("src/lib.rs", "")
        .with("crates/x/tests/t.rs", &tests)
        .with(RECORD, record)
        .with(RENDERED_PATH, &render(&m));
    check(&m, &repo)
}

/// The problems for the verified row [`ROW`] whose record reads `record`.
fn record_problems(record: &str) -> Vec<String> {
    problems_for(&[ROW], record)
}

/// The problems for the verified rows [`ROW`] and [`OTHER`], which share
/// [`RECORD`].
fn shared_record_problems(record: &str) -> Vec<String> {
    problems_for(&[ROW, OTHER], record)
}

// The exact problems the gate reports today (`check_record` in
// `xtask/src/matrix.rs`). Slice 0007 does not change them (D3).

/// [`ROW`] has no verdict line in [`RECORD`].
fn no_verdict() -> String {
    format!("{ROW}: verification record {RECORD} has no `- Row verdict: {ROW}: PASS` line")
}

/// [`ROW`]'s one verdict line in [`RECORD`] says FAIL.
fn fail() -> String {
    format!("{ROW}: verification record {RECORD} gives the verdict FAIL, not PASS")
}

/// [`ROW`] has `n` verdict lines in [`RECORD`].
fn count(n: usize) -> String {
    format!(
        "{ROW}: verification record {RECORD} has {n} verdict lines for this row; expected \
         exactly one"
    )
}

/// Line `n` of [`RECORD`] is malformed, reported for the citing row `id`.
fn malformed(id: &str, n: usize) -> String {
    format!(
        "{id}: verification record {RECORD}:{n}: malformed verdict line; expected \
         `- Row verdict: <ROW-ID>: <PASS|FAIL|OPEN>`"
    )
}

// The strict-record problems of slice 0007 D8 to D10, whose texts the brief
// fixes.

/// The D8 problem, after `<record>:<line>: `.
const RAW_HTML: &str = "raw HTML other than a closed comment or <br>, <ul>, <li> is not allowed";
/// The D9 problem, after `<record>:<line>: `.
const LOOK_ALIKE: &str = "a verdict-like line is not at column 0";
/// The D10 lone-CR problem, after `<record>:<line>: `.
const LONE_CR: &str = "lone carriage return; use LF or CRLF line endings";

/// `what` at line `n` of [`RECORD`], reported for the citing row `id`.
fn at_line(id: &str, n: usize, what: &str) -> String {
    format!("{id}: verification record {RECORD}:{n}: {what}")
}

/// D8: line `n` holds raw HTML that is not allowed, reported for [`ROW`].
fn html(n: usize) -> String {
    at_line(ROW, n, RAW_HTML)
}

/// [`html`] for each of `lines`, in order.
fn html_at(lines: &[usize]) -> Vec<String> {
    lines.iter().map(|&n| html(n)).collect()
}

/// D9: line `n` is a verdict-like line not at column 0, reported for [`ROW`].
fn look_alike(n: usize) -> String {
    at_line(ROW, n, LOOK_ALIKE)
}

/// D10: line `n` holds a lone CR, reported for [`ROW`].
fn lone_cr(n: usize) -> String {
    at_line(ROW, n, LONE_CR)
}

/// D10: [`RECORD`] starts with a byte-order mark, reported for the row `id`.
fn bom(id: &str) -> String {
    format!("{id}: verification record {RECORD}: starts with a byte-order mark")
}

/// The row a problem is reported for, and where in the record it is: 0 for
/// the BOM, the line for a problem at a line, and `usize::MAX` for the
/// verdict problem. Used to read the lines of raw-HTML problems.
fn problem_key(problem: &str) -> (&str, usize) {
    let (row, rest) = problem.split_once(": ").unwrap_or((problem, ""));
    let rest = rest
        .strip_prefix("verification record ")
        .and_then(|r| r.strip_prefix(RECORD))
        .unwrap_or(rest);
    let line = rest.strip_prefix(':').and_then(|r| {
        let digits = r.len() - r.trim_start_matches(|c: char| c.is_ascii_digit()).len();
        r[..digits].parse().ok()
    });
    match line {
        Some(n) => (row, n),
        None if rest == ": starts with a byte-order mark" => (row, 0),
        None => (row, usize::MAX),
    }
}

/// [`ROW`]'s verdict line saying PASS.
const PASS: &str = "- Row verdict: LIB-TOOL-001: PASS";
/// [`ROW`]'s verdict line saying FAIL.
const FAIL: &str = "- Row verdict: LIB-TOOL-001: FAIL";

/// `template` with `{PASS}` and `{FAIL}` replaced by [`PASS`] and [`FAIL`].
fn fill(template: &str) -> String {
    template.replace("{PASS}", PASS).replace("{FAIL}", FAIL)
}

/// `text` with every `\n` line ending made `\r\n`.
fn crlf(text: &str) -> String {
    text.replace('\n', "\r\n")
}

/// Each case whose record, its template [`fill`]ed, does not give [`ROW`]
/// exactly the problems `expected`, with its `why`, the record and what the
/// gate gave. Each test lists every failing case, not only the first.
fn mismatches(cases: &[(&str, &str)], expected: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for (template, why) in cases {
        let record = fill(template);
        let p = record_problems(&record);
        if p != expected {
            out.push(format!(
                "{why}: the record {record:?} gave {p:#?}, expected {expected:#?}"
            ));
        }
    }
    out
}

/// A record template, the exact problems [`ROW`] gets from it, and why.
type Case<'a> = (&'a str, Vec<String>, &'a str);

/// Each of `cases` whose record, its template [`fill`]ed, with LF and with
/// CRLF line endings, does not give [`ROW`] exactly its expected problems.
fn case_mismatches(cases: &[Case<'_>]) -> Vec<String> {
    let mut out = Vec::new();
    for (template, expected, why) in cases {
        out.extend(mismatches_lf_and_crlf(&[(*template, *why)], expected));
    }
    out
}

/// `head` followed by `tail`.
fn then(mut head: Vec<String>, tail: &[String]) -> Vec<String> {
    head.extend_from_slice(tail);
    head
}

/// [`mismatches`] for `cases` and for each case with CRLF line endings.
fn mismatches_lf_and_crlf(cases: &[(&str, &str)], expected: &[String]) -> Vec<String> {
    let crlf_cases: Vec<(String, String)> = cases
        .iter()
        .map(|(t, why)| (crlf(t), format!("{why} (CRLF)")))
        .collect();
    let crlf_refs: Vec<(&str, &str)> = crlf_cases
        .iter()
        .map(|(t, why)| (t.as_str(), why.as_str()))
        .collect();
    let mut out = mismatches(cases, expected);
    out.extend(mismatches(&crlf_refs, expected));
    out
}

// ---------------------------------------------------------------------------
// LIB-TOOL-005: the problem texts stay as they are
// ---------------------------------------------------------------------------

#[test]
fn lib_tool_005_the_problem_texts_stay_as_they_are() {
    for (record, expected, what) in [
        (
            "# V\n\nNothing was judged.\n".to_owned(),
            vec![no_verdict()],
            "no verdict line",
        ),
        (fill("{FAIL}\n"), vec![fail()], "one FAIL line"),
        (fill("{PASS}\n\n{PASS}\n"), vec![count(2)], "two PASS lines"),
        (
            fill("# V\n- Row verdict: x\n{PASS}\n"),
            vec![malformed(ROW, 2)],
            "a malformed line 2 beside a PASS line",
        ),
        (
            "- Row verdict: x\n".to_owned(),
            vec![malformed(ROW, 1), no_verdict()],
            "a lone malformed line",
        ),
    ] {
        assert_eq!(
            record_problems(&record),
            expected,
            "LIB-TOOL-005: {what} gives exactly today's problem texts, in today's order; the \
             record is {record:?}"
        );
    }
    let shared = fill("{PASS}\n- Row verdict: LIB-TOOL-002: PASS\n- Row verdict: x\n");
    assert_eq!(
        shared_record_problems(&shared),
        [malformed(ROW, 3), malformed(OTHER, 3)],
        "LIB-TOOL-005: a malformed line in a shared record is reported once per citing row \
         (D5), in row order; the record is {shared:?}"
    );
}

// ---------------------------------------------------------------------------
// LIB-TOOL-005, T7: the P-005b probes
// ---------------------------------------------------------------------------

/// P-005b probe P1: an unclosed fence in a bullet item ends with the item.
const P1: &str = "- example:\n  ```\n  log\n- Row verdict: LIB-TOOL-001: FAIL\n\n```\n- Row verdict: LIB-TOOL-001: PASS\n```\n";
/// P-005b probe P1b: the same in an ordered item, whose content indent is 3.
/// It has the form of the real record `0001-m0-tooling-verification-round4.md`.
const P1B: &str = "1. Probe:\n\n   ```\n   log\n\n- Row verdict: LIB-TOOL-001: FAIL\n\nExample:\n\n```\n- Row verdict: LIB-TOOL-001: PASS\n```\n";
/// P-005b probe P6: a closer indented 5 columns closes a fence in an item with
/// content indent 2.
const P6: &str = "- a\n  ```\n  x\n     ```\n- Row verdict: LIB-TOOL-001: FAIL\n\n```\n- Row verdict: LIB-TOOL-001: PASS\n```\n";
/// P-005b probe P8: a fence in a nested item, then a fence in the outer item.
const P8: &str = "- a\n  - b\n    ```\n    x\n  ```\n- Row verdict: LIB-TOOL-001: FAIL\n\n```\n- Row verdict: LIB-TOOL-001: PASS\n```\n";
/// P-005b probe P9: the item's fence ends with the item, so the column-0
/// ``` opens a top-level fence around the PASS line.
const P9: &str = "- a\n  ```\n  x\n```\n- Row verdict: LIB-TOOL-001: PASS\n```\n";

// P1, P1b, P6 and P8 (§5.2, §4.5): the column-0 FAIL line is not indented to
// the item's content indent and is not paragraph text, so it ends the item and
// the fence in it, and counts. The column-0 ``` after it ends the list and
// opens a top-level fence, which hides the PASS example.
#[test]
fn lib_tool_005_probes_p1_p1b_p6_and_p8_give_exactly_the_fail_problem() {
    let failures = mismatches_lf_and_crlf(
        &[
            (
                P1,
                "P1: an unclosed fence in a bullet item ends with the item",
            ),
            (
                P1B,
                "P1b: an unclosed fence in an ordered item (content indent 3) ends with the item",
            ),
            (
                P6,
                "P6: a closer indented 5 columns is indented 3 relative to the item, so it closes",
            ),
            (P8, "P8: the outer item's fence ends with the outer item"),
        ],
        &[fail()],
    );
    assert!(
        failures.is_empty(),
        "LIB-TOOL-005: the FAIL line counts and the fenced PASS example does not, so the row \
         gets exactly the FAIL problem:\n{failures:#?}"
    );
}

#[test]
fn lib_tool_005_probe_p9_gives_exactly_the_no_verdict_problem() {
    let failures = mismatches_lf_and_crlf(
        &[(
            P9,
            "P9: the column-0 ``` after the item opens a top-level fence around the PASS line",
        )],
        &[no_verdict()],
    );
    assert!(
        failures.is_empty(),
        "LIB-TOOL-005: the PASS line is inside a fence, so the row has no verdict line:\n\
         {failures:#?}"
    );
}

// ---------------------------------------------------------------------------
// LIB-TOOL-005, T8: controls that pass the row
// ---------------------------------------------------------------------------

#[test]
fn lib_tool_005_a_closed_list_item_fence_then_pass_passes_the_row() {
    // Every real record passing is pinned by the real-matrix tests in
    // `gate_hardening.rs`; these are the synthetic controls.
    let failures = mismatches_lf_and_crlf(
        &[
            (
                "- example:\n  ```\n  log\n  ```\n{PASS}\n",
                "a closed fence in a bullet item, then PASS",
            ),
            (
                "- example:\n  ```\n  log\n\n  more\n  ```\n{PASS}\n",
                "a closed fence in a bullet item with a blank line inside, then PASS",
            ),
            (
                "1. Probe:\n\n   ```\n   log\n   ```\n\n{PASS}\n",
                "a closed fence in an ordered item (the form of 0001-round4), then PASS",
            ),
            (
                "1. Probe:\n\n   ```\n   log\n\n   more\n   ```\n\n{PASS}\n",
                "a closed fence in an ordered item with a blank line inside, then PASS",
            ),
            (
                "  ```\n  log\n  ```\n{PASS}\n",
                "a top-level fence indented 2, its content indented exactly 2, then PASS",
            ),
            (
                "- a\n    ```\n    log\n    ```\n{PASS}\n",
                "a fence indented 2 past the item's content indent, its content and closer \
                 indented the same, then PASS",
            ),
        ],
        &[],
    );
    assert!(
        failures.is_empty(),
        "LIB-TOOL-005: the fence is closed, so the one PASS line after it passes the row:\n\
         {failures:#?}"
    );
}

// §4.5 and §5.2: inside an item, a closer may have 0 to 3 spaces of indent
// beyond the item's content indent, whatever the opener's indent was.
#[test]
fn lib_tool_005_a_list_item_fence_closer_is_judged_relative_to_the_item() {
    let failures = mismatches_lf_and_crlf(
        &[
            (
                "- a\n     ```\n     x\n  ```\n{PASS}\n",
                "an opener 3 past the content indent is closed by a closer 0 past it",
            ),
            (
                "- a\n  ```\n  x\n     ```\n{PASS}\n",
                "a closer 3 past the content indent closes",
            ),
            (
                "1. a\n\n   ```\n   x\n      ```\n{PASS}\n",
                "in an ordered item, a closer 3 past the content indent 3 closes",
            ),
        ],
        &[],
    );
    assert!(
        failures.is_empty(),
        "LIB-TOOL-005: the item's fence closes, so the PASS line after it passes the row:\n\
         {failures:#?}"
    );
}

// ---------------------------------------------------------------------------
// LIB-TOOL-005, item 3: opener and closer lines
// ---------------------------------------------------------------------------

#[test]
fn lib_tool_005_a_verdict_look_alike_in_an_opener_info_string_is_not_a_verdict_line() {
    // §4.5: an info string without a backtick is allowed after a backtick
    // fence, and any info string after a tilde fence. The opener line does not
    // start with `- Row verdict:`, so it is not a candidate; the one PASS line
    // after the closed fence is the verdict.
    let failures = mismatches_lf_and_crlf(
        &[
            (
                "```- Row verdict: LIB-TOOL-001: PASS\ncode\n```\n{PASS}\n",
                "a PASS look-alike as a backtick opener's info string",
            ),
            (
                "``` - Row verdict: LIB-TOOL-001: FAIL\n{PASS}\n```\n{PASS}\n",
                "a FAIL look-alike as the info string, the PASS inside, a PASS after",
            ),
            (
                "```- Row verdict: x\ncode\n```\n{PASS}\n",
                "a malformed look-alike as the info string",
            ),
            (
                "~~~- Row verdict: LIB-TOOL-001: FAIL\ncode\n~~~\n{PASS}\n",
                "a FAIL look-alike as a tilde opener's info string",
            ),
            (
                "- a\n  ```- Row verdict: x\n  code\n  ```\n{PASS}\n",
                "a malformed look-alike as the info string of a fence in a list item",
            ),
        ],
        &[],
    );
    assert!(
        failures.is_empty(),
        "LIB-TOOL-005: an opener line is never a verdict line and never malformed:\n\
         {failures:#?}"
    );
}

#[test]
fn lib_tool_005_a_closer_look_alike_with_text_after_it_does_not_close_the_fence() {
    // §4.5, Example 147: a fence line with anything but spaces or tabs after
    // the run is content, so the fence stays open and hides the next line.
    let failures = mismatches_lf_and_crlf(
        &[
            (
                "```\ncode\n``` - Row verdict: LIB-TOOL-001: PASS\n{PASS}\n",
                "a PASS look-alike after a ``` run; the fence never closes",
            ),
            (
                "```\ncode\n```- Row verdict: x\n{PASS}\n```\n",
                "a malformed look-alike glued to a ``` run; the next ``` closes",
            ),
            (
                "~~~\ncode\n~~~ - Row verdict: LIB-TOOL-001: PASS\n{PASS}\n~~~\n",
                "a PASS look-alike after a ~~~ run",
            ),
        ],
        &[no_verdict()],
    );
    assert!(
        failures.is_empty(),
        "LIB-TOOL-005: the look-alike line is content, so the PASS line after it is hidden \
         and the row has no verdict line:\n{failures:#?}"
    );
}

#[test]
fn lib_tool_005_the_line_after_a_closer_counts() {
    let mut failures = mismatches_lf_and_crlf(
        &[
            ("````\ncode\n````\n{PASS}\n", "PASS after a ```` closer"),
            ("~~~\ncode\n~~~\n{PASS}\n", "PASS after a ~~~ closer"),
            ("```\n```\n{PASS}\n", "PASS after an empty fence"),
            (
                "```\ncode\n``` \t\n{PASS}\n",
                "PASS after a closer with trailing blanks",
            ),
            (
                "- a\n  ```\n  x\n  ```\n{PASS}\n",
                "PASS after a list item's closer",
            ),
        ],
        &[],
    );
    failures.extend(mismatches_lf_and_crlf(
        &[
            ("```\n{PASS}\n```\n{FAIL}\n", "FAIL after a ``` closer"),
            (
                "~~~~\n{PASS}\n~~~~~\n{FAIL}\n",
                "FAIL after a longer ~~~~~ closer",
            ),
            (
                "- a\n  ```\n  x\n  ```\n{FAIL}\n",
                "FAIL after a list item's closer",
            ),
        ],
        &[fail()],
    ));
    failures.extend(mismatches_lf_and_crlf(
        &[(
            "```\n{PASS}\n```\n- Row verdict: x\n",
            "a malformed line 4 after a ``` closer",
        )],
        &[malformed(ROW, 4), no_verdict()],
    ));
    assert!(
        failures.is_empty(),
        "LIB-TOOL-005: the line right after a closer is outside the fence and counts:\n\
         {failures:#?}"
    );
}

// ---------------------------------------------------------------------------
// LIB-TOOL-005, F5: a closer followed by U+00A0
// ---------------------------------------------------------------------------

#[test]
fn lib_tool_005_a_closer_followed_by_a_no_break_space_does_not_close_the_fence() {
    // §4.5: a closer "may be followed only by spaces or tabs". U+00A0 is
    // neither, so the line is content and the fence stays open.
    let mut failures = mismatches_lf_and_crlf(
        &[
            (
                "```\ncode\n```\u{A0}\n{PASS}\n",
                "F5: ``` and U+00A0 is content, so the PASS line is inside the fence",
            ),
            (
                "~~~\ncode\n~~~\u{A0}\n{PASS}\n",
                "~~~ and U+00A0 is content, so the PASS line is inside the fence",
            ),
        ],
        &[no_verdict()],
    );
    failures.extend(mismatches_lf_and_crlf(
        &[(
            "```\ncode\n```\u{A0}\n{PASS}\n```\n{FAIL}\n",
            "the real closer comes later, so only the FAIL after it counts",
        )],
        &[fail()],
    ));
    assert!(
        failures.is_empty(),
        "LIB-TOOL-005: a run followed by U+00A0 does not close the fence:\n{failures:#?}"
    );
}

// ---------------------------------------------------------------------------
// LIB-TOOL-005: a tab after a closer (pulldown-cmark 0.13 deviates from §4.5)
// ---------------------------------------------------------------------------

#[test]
fn lib_tool_005_a_tab_after_a_closer_closes_the_fence_so_the_fail_after_it_counts() {
    // §4.5: a closing fence "may be followed only by spaces or tabs". So each
    // closer below closes the first fence; the FAIL line after it is a
    // top-level list item and counts; the blank line and the column-0 ```
    // end that list and open a second fence, which hides the PASS line.
    // `gate_hardening.rs` pins the same closers followed by a PASS line;
    // this shape also pins that the closed fence does not swallow a FAIL and
    // that the next fence is an opener, not a closer, so a fence left open by
    // a rejected tab cannot pass the row.
    let failures = mismatches_lf_and_crlf(
        &[
            (
                "```\nx\n```\t\n{FAIL}\n\n```\n{PASS}\n```\n",
                "``` and a tab closes",
            ),
            (
                "~~~\nx\n~~~\t\n{FAIL}\n\n~~~\n{PASS}\n~~~\n",
                "~~~ and a tab closes",
            ),
            (
                "```\nx\n``` \t\n{FAIL}\n\n```\n{PASS}\n```\n",
                "``` then a space then a tab closes",
            ),
            (
                "```\nx\n```\t \n{FAIL}\n\n```\n{PASS}\n```\n",
                "``` then a tab then a space closes",
            ),
            // Verification 0007 P-7 (hand mutant M15): only the first of two
            // trailing tabs made a space leaves "``` \t", which
            // pulldown-cmark 0.13 does not take as a closer.
            (
                "```\nx\n```\t\t\n{FAIL}\n\n```\n{PASS}\n```\n",
                "``` then two tabs closes (M15)",
            ),
        ],
        &[fail()],
    );
    assert!(
        failures.is_empty(),
        "LIB-TOOL-005: a closer followed by spaces and tabs closes the fence, so the FAIL \
         line after it is the row's one verdict line:\n{failures:#?}"
    );
}

#[test]
fn lib_tool_005_a_lone_cr_ends_the_closer_line_before_text() {
    // §2.1: a line ending is LF, CR not followed by LF, or CRLF. So in
    // "```\t\rtext\n" the lone CR ends the line "```\t", which closes the
    // fence (§4.5), and "text" is a paragraph on a line of its own. The FAIL
    // line interrupts that paragraph as a list item (§5.2), the blank line and
    // the column-0 ``` end the list and open a second fence, and that fence
    // hides the PASS line. The row's one verdict is FAIL.
    //
    // The gate's own line splitting (`str::lines`) splits only on `\n`, so to
    // the gate "```\t\rtext" is one line. It does not start with
    // `- Row verdict:`, so it is not a candidate, and the FAIL line, after the
    // next `\n`, is a candidate at its own byte offset either way. That is why
    // the CR is followed by "text" and not by the FAIL line: a candidate glued
    // after a lone CR is never one to the gate, and could not show what the
    // parser makes of the CR. Which candidate counts depends only on the
    // hidden ranges, and those depend on the tab before the lone CR being
    // read as trailing, which it is only if `\r` ends the line.
    //
    // Slice 0007 D10: strict records. The lone CR on line 3 is now also a
    // problem, reported at line 3 (the LF/CRLF line that holds it) before
    // the verdict problem. The FAIL verdict is still judged as before.
    let failures = mismatches_lf_and_crlf(
        &[
            (
                "```\nx\n```\t\rtext\n{FAIL}\n\n```\n{PASS}\n```\n",
                "``` and a tab, then a lone CR and text",
            ),
            (
                "~~~\nx\n~~~\t\rtext\n{FAIL}\n\n~~~\n{PASS}\n~~~\n",
                "~~~ and a tab, then a lone CR and text",
            ),
        ],
        &[lone_cr(3), fail()],
    );
    assert!(
        failures.is_empty(),
        "LIB-TOOL-005: a lone CR ends the closer line, so the tab before it is trailing, the \
         fence closes and the FAIL line counts; the lone CR is reported at line 3 (D10):\n\
         {failures:#?}"
    );
}

#[test]
fn lib_tool_005_a_tab_that_is_not_trailing_keeps_its_meaning() {
    // §2.2 and §4.4: a leading tab reaches column 4, so each first line below
    // is an indented code block, whatever follows the tab; a tab inside the
    // content is content. Only a tab in the trailing run of a line may be
    // read as a space. Were the leading tab a space, "``` " and "```  info"
    // would be fence openers indented 1 space (§4.5) and would hide the PASS
    // line; as code, they end at the column-0 PASS line, which counts.
    let failures = mismatches_lf_and_crlf(
        &[
            (
                "\tco\tde\n{PASS}\n",
                "a tab-indented code line with a tab inside its content",
            ),
            (
                "\t```\t\n{PASS}\n",
                "a tab-indented ``` with a trailing tab: only the trailing tab is a blank",
            ),
            (
                "\t```\tinfo\n{PASS}\n",
                "a tab-indented ``` with a tab before more text",
            ),
        ],
        &[],
    );
    assert!(
        failures.is_empty(),
        "LIB-TOOL-005: a tab that is not trailing keeps its CommonMark meaning, so the \
         indented code block ends at the column-0 PASS line:\n{failures:#?}"
    );
}

// ---------------------------------------------------------------------------
// LIB-TOOL-005: nesting and containers
// ---------------------------------------------------------------------------

#[test]
fn lib_tool_005_a_fence_in_a_nested_list_item_ends_with_its_item() {
    // §5.2: a column-0 line ends every open item, and with them the fences in
    // them. A line indented less than the inner item's content indent but at
    // least the outer one's belongs to the outer item.
    let mut failures = mismatches_lf_and_crlf(
        &[
            (
                "- a\n  - b\n    ```\n    x\n{PASS}\n",
                "an unclosed fence in a nested item, then a column-0 PASS",
            ),
            (
                "- a\n  - b\n    ```\n    x\n  ```\n{PASS}\n",
                "the inner fence ends with the inner item; the ``` in the outer item opens a \
                 fence that ends with the outer item",
            ),
            (
                "- a\n  - b\n\n    ```\n    x\n    ```\n\n  ```\n  y\n{PASS}\n",
                "a closed fence in the inner item, an unclosed one in the outer item",
            ),
            (
                "- a\n  1. b\n     ```\n     x\n  ```\n{PASS}\n",
                "an ordered item (content indent 5) nested in a bullet item",
            ),
        ],
        &[],
    );
    failures.extend(mismatches_lf_and_crlf(
        &[(
            "- a\n  - b\n    ```\n    x\n{FAIL}\n\n```\n{PASS}\n```\n",
            "the nested fence ends at the FAIL line; the top-level fence hides the PASS",
        )],
        &[fail()],
    ));
    assert!(
        failures.is_empty(),
        "LIB-TOOL-005: a fence in a nested list item ends with its item:\n{failures:#?}"
    );
}

#[test]
fn lib_tool_005_a_fence_in_a_block_quote_ends_with_the_quote() {
    // §5.1, Examples 128 and 237: a line without `>` continues a block quote
    // only as lazy paragraph text. A fence is not a paragraph, so a column-0
    // line ends the quote and its fence; a `> ` line is never a candidate.
    let mut failures = mismatches_lf_and_crlf(
        &[
            (
                "> ```\n> code\n{PASS}\n",
                "a column-0 PASS right after a quoted, unclosed fence",
            ),
            (
                "> ```\n> code\n\n{PASS}\n",
                "a column-0 PASS after a blank line ends the quote",
            ),
            (
                "> ```\n> {PASS}\n> ```\n{PASS}\n",
                "a quoted PASS inside a quoted fence, then a column-0 PASS",
            ),
            (
                "> ```\n> code\n{PASS}\n```\n",
                "the column-0 PASS counts, and the ``` after it opens a top-level fence",
            ),
            (
                "> - a\n>   ```\n>   x\n{PASS}\n",
                "a fence in a list item in a block quote",
            ),
        ],
        &[],
    );
    failures.extend(mismatches_lf_and_crlf(
        &[(
            "> ```\n> code\n```\n{PASS}\n",
            "the column-0 ``` ends the quote and opens a top-level fence around the PASS",
        )],
        &[no_verdict()],
    ));
    assert!(
        failures.is_empty(),
        "LIB-TOOL-005: a fence in a block quote ends with the quote:\n{failures:#?}"
    );
}

#[test]
fn lib_tool_005_an_indented_code_block_hides_only_its_own_lines() {
    // §4.4: a line indented 4 spaces is code (and not a candidate); a column-0
    // line ends the block.
    let mut failures = mismatches_lf_and_crlf(
        &[
            ("    {PASS}\n", "a PASS line indented 4 spaces"),
            (
                "Text.\n\n    {PASS}\n    - Row verdict: x\n",
                "a PASS line and a malformed line in an indented code block",
            ),
        ],
        &[no_verdict()],
    );
    failures.extend(mismatches_lf_and_crlf(
        &[
            (
                "    code\n{PASS}\n",
                "a PASS line right after an indented code block",
            ),
            (
                "    code\n\n{PASS}\n",
                "a PASS line after an indented code block and a blank",
            ),
            (
                "    code\n\n    more\n{PASS}\n",
                "a PASS line after an indented code block with a blank line inside",
            ),
            (
                "    ```\n    {PASS}\n{PASS}\n",
                "an indented code block holding fence-like text, then a PASS line",
            ),
        ],
        &[],
    ));
    assert!(
        failures.is_empty(),
        "LIB-TOOL-005: an indented code block hides its own lines and nothing after:\n\
         {failures:#?}"
    );
}

#[test]
fn lib_tool_005_a_tab_indented_code_block_hides_only_its_own_lines() {
    // §2.2 and §4.4: a leading tab reaches column 4, so the line is code.
    let mut failures = mismatches_lf_and_crlf(
        &[
            ("\t{PASS}\n", "a tab-indented PASS line"),
            (
                "\t{PASS}\n\t- Row verdict: x\n",
                "a tab-indented PASS and malformed line",
            ),
        ],
        &[no_verdict()],
    );
    failures.extend(mismatches_lf_and_crlf(
        &[
            (
                "\tcode\n{PASS}\n",
                "a PASS line right after a tab-indented code block",
            ),
            (
                "\t- Row verdict: x\n{PASS}\n",
                "a tab-indented malformed look-alike, then a PASS line",
            ),
            (
                "\t```\n\tcode\n{PASS}\n\t```\n",
                "a tab-indented code block holding fence-like text, then a PASS line",
            ),
        ],
        &[],
    ));
    assert!(
        failures.is_empty(),
        "LIB-TOOL-005: a tab-indented code block hides its own lines and nothing after:\n\
         {failures:#?}"
    );
}

// ---------------------------------------------------------------------------
// LIB-TOOL-005, criterion 7: core CommonMark through the parser
// ---------------------------------------------------------------------------

#[test]
fn lib_tool_005_extension_syntax_does_not_hide_a_verdict_line() {
    // With `Options::empty()` there are no metadata blocks. In core CommonMark
    // `---` is a thematic break (§4.1) and `+++` is paragraph text, so the
    // PASS line between them is a list item and counts. A parser with the
    // YAML or TOML metadata-block extension would hide it.
    let failures = mismatches_lf_and_crlf(
        &[
            (
                "---\n{PASS}\n---\n",
                "a PASS line between `---` lines at the start",
            ),
            (
                "+++\n{PASS}\n+++\n",
                "a PASS line between `+++` lines at the start",
            ),
        ],
        &[],
    );
    assert!(
        failures.is_empty(),
        "LIB-TOOL-005: the parser runs with no extensions, so metadata-block syntax hides \
         nothing:\n{failures:#?}"
    );
}

#[test]
fn lib_tool_005_hidden_lines_are_decided_only_by_pulldown_cmark_with_no_extensions() {
    let root = xtask::repo_root();
    let read = |rel: &str| {
        std::fs::read_to_string(root.join(rel))
            .unwrap_or_else(|e| panic!("LIB-TOOL-005: read {rel}: {e}"))
    };
    let source = read("xtask/src/matrix.rs");
    assert!(
        !source.contains("fn fence("),
        "LIB-TOOL-005: the hand-written fence model (`fn fence`) is removed from \
         xtask/src/matrix.rs (D4)"
    );
    assert!(
        source.contains("pulldown_cmark"),
        "LIB-TOOL-005: xtask/src/matrix.rs decides hidden lines through `pulldown_cmark` (D1)"
    );
    assert!(
        !source.contains("ENABLE_") && !source.contains("Options::all"),
        "LIB-TOOL-005: the parser runs with `Options::empty()`: no extension is enabled (D1)"
    );

    let manifest: toml::Table = read("xtask/Cargo.toml")
        .parse()
        .expect("LIB-TOOL-005: xtask/Cargo.toml parses");
    let dep = manifest
        .get("dependencies")
        .and_then(|d| d.get("pulldown-cmark"))
        .and_then(toml::Value::as_table);
    let version = dep
        .and_then(|d| d.get("version"))
        .and_then(toml::Value::as_str);
    let defaults = dep
        .and_then(|d| d.get("default-features"))
        .and_then(toml::Value::as_bool);
    assert!(
        version.is_some_and(|v| v == "0.13" || v.starts_with("0.13.")) && defaults == Some(false),
        "LIB-TOOL-005: xtask depends on pulldown-cmark 0.13 with `default-features = false` \
         under [dependencies] (D1), got {dep:?}"
    );
    for other in ["crates/sacm/Cargo.toml", "crates/sacm-cli/Cargo.toml"] {
        assert!(
            !read(other).contains("pulldown-cmark"),
            "LIB-TOOL-005: only xtask depends on pulldown-cmark (ADR 0008), not {other}"
        );
    }
}

// ---------------------------------------------------------------------------
// LIB-TOOL-005: malformed lines and D5 under the parser
// ---------------------------------------------------------------------------

/// Line 13 is a visible malformed line; the malformed look-alikes on lines 4
/// (a closed HTML comment) and 7 (a fence) are hidden; line 14 is the PASS.
/// Lines 9 to 11 are a `<div>` block that the blank line 12 ends. Under
/// slice 0007 D8 (strict records) it is raw HTML on each of its lines and
/// hides nothing, so the malformed candidate on its line 10 counts.
const MIXED: &str = "# Verification\n\n<!-- note\n- Row verdict: a\n-->\n```\n- Row verdict: b\n```\n<div>\n- Row verdict: c\n</div>\n\n- Row verdict: LIB-TOOL-001: pass\n- Row verdict: LIB-TOOL-001: PASS\n";

#[test]
fn lib_tool_005_a_malformed_line_outside_every_hidden_range_is_reported_with_its_line_number() {
    // Slice 0007 D8: strict records. The `<div>` block was hidden; now its
    // lines 9 to 11 are raw-HTML problems, and its line 10 is also a visible
    // malformed line.
    let expected = [
        html(9),
        html(10),
        malformed(ROW, 10),
        html(11),
        malformed(ROW, 13),
    ];
    // On line 10 the raw-HTML problem comes before the malformed one (D11).
    for record in [MIXED.to_owned(), crlf(MIXED)] {
        assert_eq!(
            record_problems(&record),
            expected,
            "LIB-TOOL-005: the visible malformed lines 10 and 13 are reported, by their 1-based \
             line numbers, beside the raw HTML of lines 9 to 11, and the PASS on line 14 stands; \
             the record is {record:?}"
        );
    }
    // The column-0 line ends the item and its fence (§5.2), so the malformed
    // line 4 is visible.
    let ended = fill("- a\n  ```\n  x\n- Row verdict: x\n{PASS}\n");
    for record in [ended.clone(), crlf(&ended)] {
        assert_eq!(
            record_problems(&record),
            [malformed(ROW, 4)],
            "LIB-TOOL-005: a malformed line that ends a list item's fence is reported as line \
             4; the record is {record:?}"
        );
    }
}

#[test]
fn lib_tool_005_a_malformed_line_inside_a_code_block_is_not_reported() {
    let failures = mismatches_lf_and_crlf(
        &[
            (
                "```\n- Row verdict: x\n```\n{PASS}\n",
                "inside a closed fence",
            ),
            (
                "{PASS}\n```\n- Row verdict: x\n",
                "inside an unclosed fence",
            ),
            (
                "  ```\n- Row verdict: x\n  ```\n{PASS}\n",
                "less indented than the fence's opener, so still content (§4.5)",
            ),
            (
                "- a\n  ```\n  x\n```\n- Row verdict: x\n```\n{PASS}\n",
                "inside the top-level fence that follows a list item (the P9 form)",
            ),
            (
                "> ```\n> - Row verdict: x\n```\n- Row verdict: x\n```\n{PASS}\n",
                "inside the top-level fence that follows a block quote",
            ),
        ],
        &[],
    );
    assert!(
        failures.is_empty(),
        "LIB-TOOL-005: a malformed line inside a code block is not reported, and the PASS line \
         outside passes the row:\n{failures:#?}"
    );
}

#[test]
fn lib_tool_005_a_shared_record_reports_each_visible_malformed_line_once_per_citing_row() {
    // D5 of slice 0004, under the parser: line 4 is inside a comment, line 6
    // is visible.
    let record = fill(
        "{PASS}\n- Row verdict: LIB-TOOL-002: PASS\n<!--\n- Row verdict: x\n-->\n- Row verdict: y\n",
    );
    for record in [record.clone(), crlf(&record)] {
        assert_eq!(
            shared_record_problems(&record),
            [malformed(ROW, 6), malformed(OTHER, 6)],
            "LIB-TOOL-005: the visible malformed line 6 is reported once for each citing row, \
             and the hidden line 4 never; the record is {record:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// LIB-TOOL-015: HTML comments and HTML blocks
// ---------------------------------------------------------------------------

#[test]
fn lib_tool_015_a_verdict_line_inside_an_html_comment_does_not_count() {
    // §4.6 type 2: a block starting `<!--` ends at the line holding `-->`,
    // blank lines included (Example 179), or at the end of the document. It
    // may interrupt a paragraph and be indented up to 3 spaces. Each block
    // below is one closed comment, so D8 allows it and it hides its lines.
    let mut failures = mismatches_lf_and_crlf(
        &[
            (
                "<!--\n{PASS}\n-->\n",
                "a PASS line in a comment spanning lines",
            ),
            (
                "<!-- note\n\n{PASS}\n\n-->\n",
                "a comment with blank lines inside",
            ),
            (
                "<!--\n{PASS} -->\n",
                "a PASS line on the comment's closing line",
            ),
            ("   <!--\n{PASS}\n-->\n", "a comment indented 3 spaces"),
            (
                "Text.\n<!--\n{PASS}\n-->\n",
                "a comment that interrupts a paragraph",
            ),
            (
                "<!-- one -->\n<!--\n{PASS}\n-->\n",
                "a second comment after a one-line one",
            ),
        ],
        &[no_verdict()],
    );
    failures.extend(mismatches_lf_and_crlf(
        &[
            (
                "{PASS}\n<!--\n{PASS}\n-->\n<!--\n{FAIL}\n-->\n",
                "one visible PASS, then a commented PASS and a commented FAIL",
            ),
            (
                "<!--\n{FAIL}\n\n{PASS}\n-->\n{PASS}\n",
                "a FAIL and a PASS in a comment, then one visible PASS",
            ),
        ],
        &[],
    ));
    // Slice 0007 D8: strict records. A comment with no `-->` is not closed,
    // so it is raw HTML on each of its lines and hides nothing: the PASS on
    // line 2 counts.
    failures.extend(mismatches_lf_and_crlf(
        &[("<!--\n{PASS}\n", "a PASS line in an unclosed comment")],
        &html_at(&[1, 2]),
    ));
    assert!(
        failures.is_empty(),
        "LIB-TOOL-015: a line inside a closed HTML comment is neither a verdict line nor \
         malformed; an unclosed comment is raw HTML (D8):\n{failures:#?}"
    );
}

#[test]
fn lib_tool_015_an_html_block_of_any_type_is_raw_html_on_each_of_its_lines_and_hides_nothing() {
    // Formerly `lib_tool_015_a_verdict_line_inside_an_html_block_does_not_count`.
    // Slice 0007 D8: strict records. An HTML block (§4.6) is raw HTML on each
    // of its lines that holds HTML, and it hides nothing: the PASS line inside
    // it counts. A blank line inside a type 1 to 5 block holds no HTML and is
    // not reported (decision (b) of the fix round).
    let failures = case_mismatches(&html_block_cases());
    assert!(
        failures.is_empty(),
        "LIB-TOOL-015: an HTML block is raw HTML on each of its lines, and a verdict line inside \
         it counts (D8):\n{failures:#?}"
    );
}

/// HTML blocks of types 1 and 3 to 7 (§4.6), each with a verdict line
/// inside, and the problems each gives under slice 0007 D8.
fn html_block_cases() -> Vec<Case<'static>> {
    vec![
        (
            "<div>\n{PASS}\n</div>\n",
            html_at(&[1, 2, 3]),
            "type 6: between <div> and </div>",
        ),
        (
            "<div class=\"note\">\n{PASS}\n",
            html_at(&[1, 2]),
            "type 6: after an unclosed <div> with attributes",
        ),
        (
            "<DIV>\n{PASS}\n",
            html_at(&[1, 2]),
            "type 6: tag names are case-insensitive (Example 152)",
        ),
        (
            "<details>\n{PASS}\n</details>\n",
            html_at(&[1, 2, 3]),
            "type 6: after <details>",
        ),
        (
            "<details>\n<summary>Example</summary>\n{PASS}\n</details>\n",
            html_at(&[1, 2, 3, 4]),
            "type 6: after <details> and <summary>",
        ),
        (
            "</div>\n{PASS}\n",
            html_at(&[1, 2]),
            "type 6: a closing tag starts a block (Example 151)",
        ),
        (
            "<div>\n{PASS}\n</div>\n{PASS}\n",
            then(html_at(&[1, 2, 3, 4]), &[count(2)]),
            "type 6: </div> does not end the block; only a blank line does (Example 186)",
        ),
        (
            "   <div>\n{PASS}\n",
            html_at(&[1, 2]),
            "type 6: indented 3 spaces",
        ),
        (
            "Text.\n<div>\n{PASS}\n",
            html_at(&[2, 3]),
            "type 6: interrupting a paragraph (Example 185)",
        ),
        (
            "<custom-tag>\n{PASS}\n</custom-tag>\n",
            html_at(&[1, 2, 3]),
            "type 7: <custom-tag>",
        ),
        (
            "<custom-tag data-x=\"1\">\n{PASS}\n",
            html_at(&[1, 2]),
            "type 7: an open tag with an attribute",
        ),
        (
            "</custom-tag>\n{PASS}\n",
            html_at(&[1, 2]),
            "type 7: a closing tag",
        ),
    ]
    .into_iter()
    .chain(html_block_cases_of_types_1_to_5())
    .collect()
}

/// [`html_block_cases`] for types 1 to 5, which end at their end marker.
fn html_block_cases_of_types_1_to_5() -> Vec<Case<'static>> {
    vec![
        (
            "<pre>\n\n{PASS}\n</pre>\n",
            html_at(&[1, 3, 4]),
            "type 1: <pre> runs past a blank line to </pre>; the blank line holds no HTML",
        ),
        (
            "<pre>\n  \t\n{PASS}\n</pre>\n",
            html_at(&[1, 3, 4]),
            "type 1: a line of only spaces and a tab inside <pre> holds no HTML",
        ),
        (
            "<script>\n{PASS}\n</script>\n",
            html_at(&[1, 2, 3]),
            "type 1: <script>",
        ),
        (
            "<style>\n\n{PASS}\n</style>\n",
            html_at(&[1, 3, 4]),
            "type 1: <style> runs past a blank line",
        ),
        (
            "<textarea>\n{PASS}\n</textarea>\n",
            html_at(&[1, 2, 3]),
            "type 1: <textarea>",
        ),
        (
            "<?note\n\n{PASS}\n?>\n",
            html_at(&[1, 3, 4]),
            "type 3: a processing instruction (Example 180)",
        ),
        (
            "<!DOCTYPE note\n{PASS}\n>\n",
            html_at(&[1, 2, 3]),
            "type 4: a declaration",
        ),
        (
            "<![CDATA[\n\n{PASS}\n]]>\n",
            html_at(&[1, 3, 4]),
            "type 5: a CDATA section",
        ),
        (
            "<div>\n{FAIL}\n</div>\n\n{PASS}\n",
            then(html_at(&[1, 2, 3]), &[count(2)]),
            "a FAIL line in a <div> block, then a PASS line after the blank line",
        ),
    ]
}

#[test]
fn lib_tool_015_a_malformed_candidate_is_hidden_by_a_closed_comment_but_not_by_other_html() {
    // Formerly `lib_tool_015_a_malformed_candidate_inside_html_is_not_malformed`.
    // Slice 0007 D8: strict records. Only a closed comment hides a candidate;
    // inside other raw HTML the malformed line is visible and reported, on
    // the same line as the raw-HTML problem.
    let cases: Vec<Case<'_>> = vec![
        (
            "<!--\n- Row verdict: x\n-->\n{PASS}\n",
            vec![],
            "inside a comment",
        ),
        (
            "<!--\n- Row verdict:\n{PASS} -->\n{PASS}\n",
            vec![],
            "on a comment's lines and closing line",
        ),
        (
            "<div>\n- Row verdict: LIB-TOOL-001: pass\n</div>\n\n{PASS}\n",
            vec![html(1), html(2), malformed(ROW, 2), html(3)],
            "inside a <div> block",
        ),
        (
            "<custom-tag>\n- Row verdict: x\n\n{PASS}\n",
            vec![html(1), html(2), malformed(ROW, 2)],
            "inside a type 7 block",
        ),
        (
            "<pre>\n- Row verdict: x\n</pre>\n{PASS}\n",
            vec![html(1), html(2), malformed(ROW, 2), html(3)],
            "inside a <pre> block",
        ),
    ];
    let failures = case_mismatches(&cases);
    assert!(
        failures.is_empty(),
        "LIB-TOOL-015: a malformed candidate inside a closed comment is not reported; inside \
         other raw HTML it is (D8):\n{failures:#?}"
    );
}

// ---------------------------------------------------------------------------
// LIB-TOOL-015: the seeds P2 and P7
// ---------------------------------------------------------------------------

/// Seed P2: a line `` ```a`b ``, then `` ``` ``, then a PASS line, then `` ``` ``.
const P2: &str = "```a`b\n```\n- Row verdict: LIB-TOOL-001: PASS\n```\n";

#[test]
fn lib_tool_015_seed_p2_a_backtick_in_the_info_string_makes_the_next_fence_line_the_opener() {
    // §4.5, Example 145: a backtick opener's info string may not hold a
    // backtick, so ```a`b is paragraph text. The ``` after it interrupts the
    // paragraph (Example 140) and opens the fence.
    let mut failures = mismatches_lf_and_crlf(
        &[
            (
                P2,
                "P2: the PASS line is inside the fence the second line opens",
            ),
            (
                "``` a ` b\n```\n{PASS}\n```\n",
                "a backtick between spaces in the info string",
            ),
            (
                "````x`\n````\n{PASS}\n````\n",
                "a four-backtick run with a backtick after",
            ),
            (
                "   ```a`b\n```\n{PASS}\n```\n",
                "the non-opener indented 3 spaces",
            ),
        ],
        &[no_verdict()],
    );
    failures.extend(mismatches_lf_and_crlf(
        &[
            ("```a`b\n{PASS}\n", "the non-opener hides nothing after it"),
            (
                "```a`b\n```\n{FAIL}\n```\n{PASS}\n",
                "the FAIL is inside the fence the second line opens; the PASS is after it",
            ),
            (
                "- a\n  ```a`b\n  ```\n  x\n{PASS}\n",
                "in a list item, the second line opens a fence that ends with the item",
            ),
        ],
        &[],
    ));
    assert!(
        failures.is_empty(),
        "LIB-TOOL-015: a backtick fence line whose info string holds a backtick is not an \
         opener:\n{failures:#?}"
    );
}

/// Seed P7 as the criterion states its outcome: `<div>`, `` ``` ``, `</div>`, a
/// blank line, a FAIL line, then a fenced PASS.
const P7: &str = "<div>\n```\n</div>\n\n- Row verdict: LIB-TOOL-001: FAIL\n```\n- Row verdict: LIB-TOOL-001: PASS\n```\n";

/// Seed P7 with no blank line after `</div>`.
const P7_NO_BLANK: &str = "<div>\n```\n</div>\n- Row verdict: LIB-TOOL-001: FAIL\n```\n- Row verdict: LIB-TOOL-001: PASS\n```\n";

#[test]
fn lib_tool_015_seed_p7_a_fence_line_inside_an_html_block_opens_no_fence() {
    // §4.6: the ``` line is inside the <div> block (type 6), which only a
    // blank line ends. With the blank line, the FAIL line counts, and the ```
    // after it opens a top-level fence around the PASS line.
    //
    // Slice 0007 D8: strict records. The <div> block is now raw HTML on its
    // lines 1 to 3, beside the FAIL problem.
    let mut failures = mismatches_lf_and_crlf(
        &[
            (P7, "P7: the FAIL counts and the fenced PASS does not"),
            (
                "<div>\n```\n</div>\n\n{FAIL}\n\n```\n{PASS}\n```\n",
                "P7 with a blank line before the fence",
            ),
        ],
        &then(html_at(&[1, 2, 3]), &[fail()]),
    );
    // Without the blank line, `</div>` does not end the block (Example 186):
    // the whole record is one HTML block. Slice 0007 D8: it is raw HTML on
    // each of its 7 lines and hides nothing, so the FAIL on line 4 and the
    // PASS on line 6 both count.
    failures.extend(mismatches_lf_and_crlf(
        &[(
            P7_NO_BLANK,
            "P7 without a blank line: the whole record after <div> is one HTML block",
        )],
        &then(html_at(&[1, 2, 3, 4, 5, 6, 7]), &[count(2)]),
    ));
    assert!(
        failures.is_empty(),
        "LIB-TOOL-015: a fence line inside an HTML block is HTML and opens no fence, and the \
         block is raw HTML (D8):\n{failures:#?}"
    );
}

// ---------------------------------------------------------------------------
// LIB-TOOL-015: controls that pass the row
// ---------------------------------------------------------------------------

#[test]
fn lib_tool_015_a_pass_line_after_a_closed_html_comment_counts() {
    let mut failures = mismatches_lf_and_crlf(
        &[
            ("<!-- note -->\n{PASS}\n", "after a one-line comment"),
            (
                "<!--\nnote\n-->\n{PASS}\n",
                "after a comment spanning lines",
            ),
            (
                "<!-- a\n\nb -->\n{PASS}\n",
                "after a comment with a blank line inside",
            ),
            (
                "Text <!-- note\n{PASS}\n-->\n",
                "a `<!--` in a paragraph: the PASS line interrupts the paragraph, so no \
                 comment spans it (§3.2, §6.6)",
            ),
        ],
        &[],
    );
    // Slice 0007 D8: strict records. Text after `-->` in the same block makes
    // the block more than one closed comment, so it is raw HTML; the PASS
    // after it still counts.
    failures.extend(mismatches_lf_and_crlf(
        &[(
            "<!-- note --> text\n{PASS}\n",
            "after a closing line with text after `-->`",
        )],
        &[html(1)],
    ));
    assert!(
        failures.is_empty(),
        "LIB-TOOL-015: the comment is closed, so the PASS line after it passes the row:\n\
         {failures:#?}"
    );
}

#[test]
fn lib_tool_015_a_pass_line_after_an_html_block_that_has_ended_counts() {
    // Slice 0007 D8: strict records. Each record holds raw HTML, now a
    // problem on each of its lines; the PASS line after it still counts, so
    // there is no verdict problem.
    let cases: Vec<Case<'_>> = vec![
        (
            "<div>\nx\n</div>\n\n{PASS}\n",
            html_at(&[1, 2, 3]),
            "after a <div> block that a blank line ends",
        ),
        (
            "<div>\n\n{PASS}\n\n</div>\n",
            html_at(&[1, 5]),
            "after an open <div> line that a blank line ends",
        ),
        (
            "<p>x</p>\n\n{PASS}\n",
            html_at(&[1]),
            "after a one-line <p> block and a blank line",
        ),
        (
            "<custom-tag>\n\n{PASS}\n",
            html_at(&[1]),
            "after a type 7 block that a blank line ends",
        ),
        (
            "<pre>\nx\n</pre>\n{PASS}\n",
            html_at(&[1, 2, 3]),
            "after a <pre> block that </pre> ends",
        ),
        (
            "<?note ?>\n{PASS}\n",
            html_at(&[1]),
            "after a processing instruction that `?>` ends",
        ),
        (
            "Text\n<custom-tag>\n{PASS}\n",
            html_at(&[2]),
            "a type 7 tag cannot interrupt a paragraph, so it is inline (Example 187)",
        ),
        (
            "- a\n\n  <div>\n{PASS}\n",
            html_at(&[3]),
            "a <div> block in a list item ends with the item",
        ),
        (
            "> <div>\n{PASS}\n",
            html_at(&[1]),
            "a <div> block in a block quote ends with the quote",
        ),
    ];
    let failures = case_mismatches(&cases);
    assert!(
        failures.is_empty(),
        "LIB-TOOL-015: the HTML block has ended, so the PASS line after it counts; the block is \
         raw HTML (D8):\n{failures:#?}"
    );
}

#[test]
fn lib_tool_015_a_pass_line_after_a_closed_fence_with_the_other_fence_character_in_its_info_string_counts()
 {
    // §4.5, Example 146: a tilde fence's info string may hold backticks and
    // tildes; a backtick fence's may hold tildes. Each opens a fence, so the
    // FAIL inside it is hidden and the PASS after the closer counts.
    let failures = mismatches_lf_and_crlf(
        &[
            (
                "~~~ a`b\n{FAIL}\n~~~\n{PASS}\n",
                "a tilde fence with a backtick in its info string",
            ),
            (
                "~~~ ``` ~~~\n{FAIL}\n~~~\n{PASS}\n",
                "a tilde fence with ``` and ~~~ in its info",
            ),
            (
                "```a~b\n{FAIL}\n```\n{PASS}\n",
                "a backtick fence with a tilde in its info string",
            ),
        ],
        &[],
    );
    assert!(
        failures.is_empty(),
        "LIB-TOOL-015: the fence opens and closes, so only the PASS line after it counts:\n\
         {failures:#?}"
    );
}

// ---------------------------------------------------------------------------
// LIB-TOOL-015, the former D6 records under slice 0007 D8
// ---------------------------------------------------------------------------
//
// D6 (superseded) hid everything from an unclosed `<!--` in an HTML event to
// the next `-->` in raw HTML. Under D8 (strict records) an unclosed comment,
// and any HTML block or inline tag that holds one, is raw HTML, a problem at
// its line, and hides nothing. Each record of the D6 tests is kept, with its
// expectation updated: the raw-HTML problems, then the verdict the visible
// candidates give. "Slice 0007 D8: strict records" applies to every test of
// this section.
//
// The shapes, by CommonMark 0.31.2:
// - `<!-- a --> <!--`: a type 2 block of one line, since the line holds
//   `-->` (§4.6). It is not one closed comment.
// - `<div><!--`: a type 6 block, which the blank line ends.
// - a `<!--` block in a list item or block quote: it ends with its
//   container, and the next column-0 line is not lazy (§5.1, §5.2).
// A `-->` line right after a list item is lazy paragraph text (§5.2); after
// a blank line it is a paragraph of its own. `<!-- end -->` at column 0 is a
// closed comment block, which D8 allows.

#[test]
fn lib_tool_015_an_unclosed_comment_in_an_html_event_is_raw_html_and_hides_nothing() {
    // Formerly `lib_tool_015_an_unclosed_comment_in_an_html_event_hides_the_pass_line_after_it`.
    let cases: Vec<Case<'_>> = vec![
        (
            "<!-- a --> <!--\n{PASS}\n-->\n",
            html_at(&[1]),
            "a second, unclosed comment after a closed one on a type 2 line",
        ),
        (
            "<div><!--\n\n{PASS}\n\n-->\n",
            html_at(&[1]),
            "an unclosed comment in a <div> block that a blank line ends",
        ),
        (
            "- a\n\n  <!--\n{PASS}\n-->\n",
            html_at(&[3]),
            "an unclosed comment block in a list item",
        ),
        (
            "> <!--\n{PASS}\n-->\n",
            html_at(&[1]),
            "an unclosed comment block in a block quote",
        ),
    ];
    let failures = case_mismatches(&cases);
    assert!(
        failures.is_empty(),
        "LIB-TOOL-015: the unclosed comment is raw HTML at its line and hides nothing, so the \
         PASS line after it counts (D8):\n{failures:#?}"
    );
}

#[test]
fn lib_tool_015_an_unclosed_comment_hides_neither_the_pass_line_nor_a_fail_after_its_closer() {
    // Formerly `lib_tool_015_an_unclosed_comment_hides_the_pass_line_but_not_a_fail_after_its_closer`.
    let cases: Vec<Case<'_>> = vec![
        (
            "<!-- a --> <!--\n{PASS}\n<!-- end -->\n{FAIL}\n",
            then(html_at(&[1]), &[count(2)]),
            "after a type 2 line with a second, unclosed comment",
        ),
        (
            "<div><!--\n\n{PASS}\n\n</div> -->\n\n{FAIL}\n",
            then(html_at(&[1, 5]), &[count(2)]),
            "after a <div> block holding an unclosed comment",
        ),
        (
            "- a\n\n  <!--\n{PASS}\n<!-- end -->\n{FAIL}\n",
            then(html_at(&[3]), &[count(2)]),
            "after an unclosed comment block in a list item",
        ),
        (
            "> <!--\n{PASS}\n<!-- end -->\n{FAIL}\n",
            then(html_at(&[1]), &[count(2)]),
            "after an unclosed comment block in a block quote",
        ),
    ];
    let failures = case_mismatches(&cases);
    assert!(
        failures.is_empty(),
        "LIB-TOOL-015: the unclosed comment hides nothing, so both the PASS and the FAIL count \
         (D8):\n{failures:#?}"
    );
}

#[test]
fn lib_tool_015_an_unclosed_comment_opens_no_region_up_to_the_next_closer() {
    // Formerly `lib_tool_015_an_unclosed_comment_region_ends_at_the_end_of_the_next_closer`.
    let cases: Vec<Case<'_>> = vec![
        (
            "<!-- a --> <!--\n{FAIL}\n<!-- end -->\n{PASS}\n",
            then(html_at(&[1]), &[count(2)]),
            "a PASS line right after the `-->` line",
        ),
        (
            "- a\n\n  <!--\n{FAIL}\n<!-- end -->\n{PASS}\n",
            then(html_at(&[3]), &[count(2)]),
            "a PASS line after the `-->` that follows a list item's comment",
        ),
        (
            "> <!--\n{FAIL}\n<!-- end -->\n\n{PASS}\n",
            then(html_at(&[1]), &[count(2)]),
            "a PASS line after the `-->` and a blank line",
        ),
        (
            "<div><!--\n\n{FAIL}\n<p>x --> y\n\n{PASS}\n",
            then(html_at(&[1, 4]), &[count(2)]),
            "a `-->` in the middle of a type 6 line",
        ),
        (
            "<!-- a --> <!--\n{FAIL}\n<!-- end -->\n{PASS}\n<div><!--\n\n{FAIL}\n<!-- end -->\n",
            then(html_at(&[1, 5]), &[count(3)]),
            "two unclosed comments, each before a closed comment block",
        ),
    ];
    let failures = case_mismatches(&cases);
    assert!(
        failures.is_empty(),
        "LIB-TOOL-015: an unclosed comment hides nothing before the next `-->`, so every \
         verdict line counts (D8):\n{failures:#?}"
    );
}

#[test]
fn lib_tool_015_an_unclosed_comment_with_no_closer_anywhere_hides_nothing_after_it() {
    // Formerly `lib_tool_015_an_unclosed_comment_with_no_closer_anywhere_hides_to_the_end_of_the_record`.
    let cases: Vec<Case<'_>> = vec![
        (
            "<!-- a --> <!--\n{PASS}\n\n{PASS}\n",
            then(html_at(&[1]), &[count(2)]),
            "after a type 2 line with an unclosed comment",
        ),
        (
            "> <!--\n{PASS}\n",
            html_at(&[1]),
            "after an unclosed comment in a block quote",
        ),
        (
            "<div><!--\n\n{PASS}\n\nText.\n\n{FAIL}\n",
            then(html_at(&[1]), &[count(2)]),
            "after a <div> block holding an unclosed comment, past paragraphs",
        ),
        (
            "- a\n\n  <!--\n{PASS}\n",
            html_at(&[3]),
            "after an unclosed comment in a list item",
        ),
    ];
    let failures = case_mismatches(&cases);
    assert!(
        failures.is_empty(),
        "LIB-TOOL-015: with no `-->` after it, an unclosed comment still hides nothing (D8):\n\
         {failures:#?}"
    );
}

#[test]
fn lib_tool_015_an_unclosed_comment_in_inline_html_is_raw_html_and_opens_no_region() {
    // Formerly `lib_tool_015_an_unclosed_comment_in_inline_html_opens_a_hidden_region`.
    // §6.6: `<span title="<!--">` is one open tag, an `InlineHtml` event that
    // is not on the allow-list.
    let failures = mismatches_lf_and_crlf(
        &[(
            "Text <span title=\"<!--\"> x\n{PASS}\n",
            "a `<!--` inside an inline tag's attribute value",
        )],
        &[html(1)],
    );
    assert!(
        failures.is_empty(),
        "LIB-TOOL-015: the inline tag is raw HTML and hides nothing (D8):\n{failures:#?}"
    );
}

#[test]
fn lib_tool_015_a_closed_comment_in_an_html_block_hides_nothing_past_its_closer() {
    // Each record also holds a stray `-->` after the PASS line. It is text.
    // A comment inside a `<div>` block is part of a block that is more than
    // one closed comment, so that block is raw HTML (D8); a block that is one
    // closed comment is allowed.
    let cases: Vec<Case<'_>> = vec![
        (
            "<div><!-- a -->\n\n{PASS}\n\n-->\n",
            html_at(&[1]),
            "a closed comment in a <div> block",
        ),
        (
            "<div><!-- a --> <!-- b -->\n\n{PASS}\n\n-->\n",
            html_at(&[1]),
            "two closed comments in a <div> block",
        ),
        (
            "<!-- a -->\n{PASS}\n-->\n",
            vec![],
            "a one-line comment block",
        ),
        (
            "<!--\nnote\n-->\n{PASS}\n-->\n",
            vec![],
            "a comment block spanning lines ends at its own `-->`",
        ),
    ];
    let failures = case_mismatches(&cases);
    assert!(
        failures.is_empty(),
        "LIB-TOOL-015: a closed comment hides nothing past its `-->`:\n{failures:#?}"
    );
}

#[test]
fn lib_tool_015_a_comment_opener_outside_every_html_event_opens_no_region() {
    // A code span (§6.1), a code block (§4.4, §4.5), a backslash escape (§2.4)
    // and paragraph text that is not a complete comment (§6.6) are not HTML
    // events, so they are neither raw HTML (D8) nor hidden.
    let failures = mismatches_lf_and_crlf(
        &[
            ("Text `<!--` here\n{PASS}\n-->\n", "a `<!--` in a code span"),
            (
                "`<!--`\n{PASS}\n-->\n",
                "a code span at the start of a line",
            ),
            (
                "The gate reads `<!--` here.\n\n{PASS}\n",
                "a `<!--` in a code span, with no `-->` anywhere",
            ),
            (
                "```\n<!--\n```\n{PASS}\n-->\n",
                "a `<!--` in a fenced code block",
            ),
            (
                "```html\n<div><!--\n```\n{PASS}\n",
                "a `<!--` in a fenced code block, with no `-->` anywhere",
            ),
            (
                "    <!--\n{PASS}\n-->\n",
                "a `<!--` in an indented code block",
            ),
            (
                "Text \\<!-- note\n{PASS}\n-->\n",
                "a backslash-escaped `<!--`",
            ),
            (
                "Text <!-- note\n{PASS}\n-->\n",
                "a `<!--` in a paragraph that ends before any `-->`",
            ),
        ],
        &[],
    );
    assert!(
        failures.is_empty(),
        "LIB-TOOL-015: a `<!--` outside every HTML event is no problem and hides nothing:\n\
         {failures:#?}"
    );
}

#[test]
fn lib_tool_015_a_pass_line_inside_details_after_a_blank_line_counts() {
    // §4.6: a blank line ends the <details> block, so the PASS line is a list
    // item inside the rendered <details> element and counts. Slice 0007 D8:
    // strict records. The <details>, <summary> and </details> lines are raw
    // HTML.
    let cases: Vec<Case<'_>> = vec![
        (
            "<details>\n<summary>Example</summary>\n\n{PASS}\n\n</details>\n",
            html_at(&[1, 2, 6]),
            "a PASS line inside <details> with a <summary>",
        ),
        (
            "<details>\n\n{PASS}\n\n</details>\n",
            html_at(&[1, 5]),
            "a PASS line inside <details>",
        ),
    ];
    let failures = case_mismatches(&cases);
    assert!(
        failures.is_empty(),
        "LIB-TOOL-015: a PASS line inside <details> after a blank line counts, and the tags \
         are raw HTML (D8):\n{failures:#?}"
    );
}

// `<!-->` and `<!--->` are complete comments (CommonMark 0.31.2 §6.6,
// Example 626), and so is `<!---->`. A line that starts with `<!--` is a type
// 2 HTML block, and since the line holds `-->` the block is that line only
// (§4.6). D8: such a block starts with `<!--`, ends with `-->` (here
// overlapping the opener) and holds no earlier `-->`, so it is one closed
// comment and allowed. In a `<div>` block it is raw HTML like the rest of the
// block.

#[test]
fn lib_tool_015_a_short_comment_is_closed_by_a_closer_that_overlaps_its_opener() {
    let cases: Vec<Case<'_>> = vec![
        (
            "<!-->\n{PASS}\n",
            vec![],
            "`<!-->` as a one-line type 2 block",
        ),
        (
            "<!--->\n{PASS}\n",
            vec![],
            "`<!--->` as a one-line type 2 block",
        ),
        (
            "Text <!--> x\n{PASS}\n",
            vec![],
            "`<!-->` as inline HTML (Example 626)",
        ),
        (
            "Text <!---> x\n{PASS}\n",
            vec![],
            "`<!--->` as inline HTML (Example 626)",
        ),
        (
            "<div><!-->\n\n{PASS}\n",
            html_at(&[1]),
            "`<!-->` in a <div> block",
        ),
        (
            "<div><!--->\n\n{PASS}\n",
            html_at(&[1]),
            "`<!--->` in a <div> block",
        ),
    ];
    let failures = case_mismatches(&cases);
    assert!(
        failures.is_empty(),
        "LIB-TOOL-015: `<!-->` and `<!--->` are closed comments, so the PASS line after them \
         passes the row:\n{failures:#?}"
    );
}

#[test]
fn lib_tool_015_an_empty_comment_of_four_dashes_is_closed() {
    let cases: Vec<Case<'_>> = vec![
        (
            "<!---->\n{PASS}\n",
            vec![],
            "`<!---->` as a one-line type 2 block",
        ),
        (
            "Text <!----> x\n{PASS}\n",
            vec![],
            "`<!---->` as inline HTML",
        ),
        (
            "<div><!---->\n\n{PASS}\n",
            html_at(&[1]),
            "`<!---->` in a <div> block",
        ),
    ];
    let failures = case_mismatches(&cases);
    assert!(
        failures.is_empty(),
        "LIB-TOOL-015: `<!---->` is a closed comment, so the PASS line after it passes the row:\n\
         {failures:#?}"
    );
}

#[test]
fn lib_tool_015_a_comment_opener_followed_by_a_lone_dash_is_not_a_closed_comment() {
    // Formerly `lib_tool_015_a_comment_opener_followed_by_a_lone_dash_hides_to_the_end_of_the_record`.
    // `<!-- -` holds no `-->`: the opener's own `--` does not close it. A type
    // 2 block with no `-->` runs to the end of the document (§4.6).
    let cases: Vec<Case<'_>> = vec![
        (
            "<!-- -\n{PASS}\n",
            html_at(&[1, 2]),
            "a type 2 block with no `-->` runs to the end and is raw HTML on each line",
        ),
        (
            "<div><!-- -\n\n{PASS}\n",
            html_at(&[1]),
            "in a <div> block that a blank line ends",
        ),
        (
            "> <!-- -\n{PASS}\n",
            html_at(&[1]),
            "in a block quote that the PASS line ends",
        ),
    ];
    let failures = case_mismatches(&cases);
    assert!(
        failures.is_empty(),
        "LIB-TOOL-015: `<!-- -` is not closed, so it is raw HTML and the PASS line counts \
         (D8):\n{failures:#?}"
    );
}

#[test]
fn lib_tool_015_a_closer_in_a_code_span_does_not_close_an_unclosed_comment() {
    // Formerly `lib_tool_015_a_closer_in_a_code_span_does_not_end_an_unclosed_comment_region`.
    let cases: Vec<Case<'_>> = vec![
        (
            "<div><!--\n\n`-->`\n\n{PASS}\n",
            html_at(&[1]),
            "the Copilot record: a code span `-->` in its own paragraph",
        ),
        (
            "<div><!--\n\nThe closer `-->` is text.\n\n{PASS}\n",
            html_at(&[1]),
            "a code span `-->` in the middle of a paragraph",
        ),
        (
            "<!-- a --> <!--\n`` --> ``\n{PASS}\n",
            html_at(&[1]),
            "a double-backtick code span on the line after a type 2 line",
        ),
        (
            "> <!--\n{FAIL}\n`-->`\n{PASS}\n",
            then(html_at(&[1]), &[count(2)]),
            "a code span `-->` in a lazy line after a block quote's comment",
        ),
        (
            "- a\n\n  <!--\n{FAIL}\n`-->`\n{PASS}\n",
            then(html_at(&[3]), &[count(2)]),
            "a code span `-->` in a lazy line after a list item's comment",
        ),
    ];
    let failures = case_mismatches(&cases);
    assert!(
        failures.is_empty(),
        "LIB-TOOL-015: the unclosed comment is raw HTML and a code span `-->` changes nothing, \
         so every verdict line counts (D8):\n{failures:#?}"
    );
}

#[test]
fn lib_tool_015_a_fail_line_after_a_raw_html_closer_that_follows_a_code_span_closer_counts() {
    let cases: Vec<Case<'_>> = vec![
        (
            "<div><!--\n\n`-->`\n\n{PASS}\n\n<!-- end -->\n{FAIL}\n",
            then(html_at(&[1]), &[count(2)]),
            "the Copilot record, then a comment block and a FAIL line",
        ),
        (
            "> <!--\n`-->`\n{PASS}\n<pre>--></pre>\n{FAIL}\n",
            then(html_at(&[1, 4]), &[count(2)]),
            "a code span `-->`, a PASS line, a type 1 line holding `-->`, a FAIL line",
        ),
    ];
    let failures = case_mismatches(&cases);
    assert!(
        failures.is_empty(),
        "LIB-TOOL-015: nothing is hidden, so the FAIL line and the PASS line both count \
         (D8):\n{failures:#?}"
    );
}

#[test]
fn lib_tool_015_a_closer_in_a_fenced_code_block_does_not_close_an_unclosed_comment() {
    // Formerly `lib_tool_015_a_closer_in_a_fenced_code_block_does_not_end_an_unclosed_comment_region`.
    let cases: Vec<Case<'_>> = vec![
        (
            "<div><!--\n\n```\n-->\n```\n{PASS}\n",
            html_at(&[1]),
            "a `-->` line in a backtick fence",
        ),
        (
            "<div><!--\n\n~~~\n-->\n~~~\n{PASS}\n",
            html_at(&[1]),
            "a `-->` line in a tilde fence",
        ),
        (
            "<!-- a --> <!--\n```text\nx --> y\n```\n{PASS}\n",
            html_at(&[1]),
            "a `-->` in the middle of a line in a backtick fence with an info string",
        ),
        (
            "> <!--\n~~~\n-->\n~~~\n{PASS}\n",
            html_at(&[1]),
            "a tilde fence after a block quote's comment",
        ),
    ];
    let failures = case_mismatches(&cases);
    assert!(
        failures.is_empty(),
        "LIB-TOOL-015: the unclosed comment is raw HTML and hides nothing, so the PASS line after \
         the fence counts (D8):\n{failures:#?}"
    );
}

#[test]
fn lib_tool_015_a_closer_in_an_indented_code_block_does_not_close_an_unclosed_comment() {
    // Formerly `lib_tool_015_a_closer_in_an_indented_code_block_does_not_end_an_unclosed_comment_region`.
    // §4.4: a column-0 line ends the indented code block.
    let cases: Vec<Case<'_>> = vec![
        (
            "<div><!--\n\n    -->\n\n{PASS}\n",
            html_at(&[1]),
            "a `-->` indented 4 spaces after a blank line",
        ),
        (
            "<!-- a --> <!--\n    -->\n{PASS}\n",
            html_at(&[1]),
            "a `-->` indented 4 spaces right after a type 2 line",
        ),
        (
            "<div><!--\n\n\t-->\n\n{PASS}\n",
            html_at(&[1]),
            "a `-->` indented by a tab",
        ),
    ];
    let failures = case_mismatches(&cases);
    assert!(
        failures.is_empty(),
        "LIB-TOOL-015: the unclosed comment is raw HTML and hides nothing, so the PASS line after \
         the code block counts (D8):\n{failures:#?}"
    );
}

#[test]
fn lib_tool_015_a_closer_in_paragraph_text_does_not_close_an_unclosed_comment() {
    // Formerly `lib_tool_015_a_closer_in_paragraph_text_does_not_end_an_unclosed_comment_region`.
    let cases: Vec<Case<'_>> = vec![
        (
            "<div><!--\n\n-->\n\n{PASS}\n",
            html_at(&[1]),
            "a `-->` line that is a paragraph of its own",
        ),
        (
            "<div><!--\n\nText --> here.\n\n{PASS}\n",
            html_at(&[1]),
            "a `-->` in the middle of a paragraph",
        ),
        (
            "<!-- a --> <!--\n{FAIL}\n-->\n{PASS}\n",
            then(html_at(&[1]), &[count(2)]),
            "a `-->` line that is lazy text of a list item",
        ),
        (
            "> <!--\n{FAIL}\n-->\n\n{PASS}\n",
            then(html_at(&[1]), &[count(2)]),
            "a lazy `-->` line, then a blank line",
        ),
        (
            "<div><!--\n\n*-->*\n\n{PASS}\n",
            html_at(&[1]),
            "a `-->` in emphasis",
        ),
        (
            "<div><!--\n\nSee <http://x/-->.\n\n{PASS}\n",
            html_at(&[1]),
            "an autolink (§6.5) ending `--`, which is a link, not raw HTML",
        ),
    ];
    let failures = case_mismatches(&cases);
    assert!(
        failures.is_empty(),
        "LIB-TOOL-015: the unclosed comment is raw HTML and hides nothing (D8):\n{failures:#?}"
    );
}

#[test]
fn lib_tool_015_a_closer_in_a_later_html_block_is_raw_html_and_hides_nothing() {
    // Formerly `lib_tool_015_a_closer_in_a_later_html_block_ends_an_unclosed_comment_region`.
    // Types 1 to 5 may interrupt the FAIL line's paragraph and end on their
    // own line (§4.6); types 6 and 7 end only at a blank line. Each later
    // block is raw HTML too.
    let cases: Vec<Case<'_>> = vec![
        (
            "<div><!--\n\n{FAIL}\n<pre>--></pre>\n{PASS}\n",
            then(html_at(&[1, 4]), &[count(2)]),
            "type 1: a `-->` inside <pre> on one line",
        ),
        (
            "<!-- a --> <!--\n{FAIL}\n<?x -->?>\n{PASS}\n",
            then(html_at(&[1, 3]), &[count(2)]),
            "type 3: a `-->` inside a processing instruction",
        ),
        (
            "> <!--\n{FAIL}\n<!X -->\n{PASS}\n",
            then(html_at(&[1, 3]), &[count(2)]),
            "type 4: a `-->` inside a declaration",
        ),
        (
            "<div><!--\n\n{FAIL}\n<![CDATA[-->]]>\n{PASS}\n",
            then(html_at(&[1, 4]), &[count(2)]),
            "type 5: a `-->` inside a CDATA section",
        ),
        (
            "<div><!--\n\n{FAIL}\n<p>`-->`\n\n{PASS}\n",
            then(html_at(&[1, 4]), &[count(2)]),
            "type 6: a backticked `-->` in a <p> line, then a blank line",
        ),
        (
            "- a\n\n  <!--\n{FAIL}\n\n<x a=-->\n\n{PASS}\n",
            then(html_at(&[3, 6]), &[count(2)]),
            "type 7: an open tag whose unquoted attribute value ends `--`",
        ),
    ];
    let failures = case_mismatches(&cases);
    assert!(
        failures.is_empty(),
        "LIB-TOOL-015: both HTML blocks are raw HTML and nothing is hidden, so the FAIL and the \
         PASS both count (D8):\n{failures:#?}"
    );
}

#[test]
fn lib_tool_015_a_closer_in_later_inline_html_is_raw_html_unless_a_closed_comment() {
    // Formerly `lib_tool_015_a_closer_in_later_inline_html_ends_an_unclosed_comment_region`.
    // Each `-->` lies in an inline HTML event (§6.6), and the PASS line on the
    // next line interrupts that paragraph (§5.2). A closed inline comment is
    // allowed; any other inline HTML is raw HTML.
    let cases: Vec<Case<'_>> = vec![
        (
            "<div><!--\n\n{FAIL}\n\nText <span title=\"-->\"> x\n{PASS}\n",
            then(html_at(&[1, 5]), &[count(2)]),
            "a `-->` in a quoted attribute value",
        ),
        (
            "<!-- a --> <!--\n{FAIL}\n\nText <x a=--> y\n{PASS}\n",
            then(html_at(&[1, 4]), &[count(2)]),
            "a `-->` where an unquoted attribute value meets the tag's `>`",
        ),
        (
            "> <!--\n{FAIL}\n\nText <!-- end --> y\n{PASS}\n",
            then(html_at(&[1]), &[count(2)]),
            "a closed inline comment, which is allowed",
        ),
        (
            "<div><!--\n\n{FAIL}\n\nText <?x --> ?> y\n{PASS}\n",
            then(html_at(&[1, 5]), &[count(2)]),
            "a `-->` inside an inline processing instruction",
        ),
        (
            "- a\n\n  <!--\n{FAIL}\nand <span title=\"-->\"> x\n{PASS}\n",
            then(html_at(&[3, 5]), &[count(2)]),
            "inline HTML in a lazy line of a list item",
        ),
    ];
    let failures = case_mismatches(&cases);
    assert!(
        failures.is_empty(),
        "LIB-TOOL-015: nothing is hidden, so the FAIL and the PASS both count; inline HTML other \
         than a closed comment is raw HTML (D8):\n{failures:#?}"
    );
}

// ---------------------------------------------------------------------------
// LIB-TOOL-015, slice 0007 D8: strict HTML
// ---------------------------------------------------------------------------

#[test]
fn lib_tool_015_disallowed_html_gives_exactly_the_raw_html_problem_once_per_line() {
    // D8: each `Html` or `InlineHtml` event that is not allowed is a problem,
    // reported once per line that holds one, at the event's first line. HTML
    // blocks of types 1 and 3 to 7 are pinned by
    // `lib_tool_015_an_html_block_of_any_type_is_raw_html_on_each_of_its_lines_and_hides_nothing`.
    let cases: Vec<Case<'_>> = vec![
        // An unclosed comment.
        (
            "{PASS}\n\n<!-- note\n",
            html_at(&[3]),
            "an unclosed comment at the end of the record",
        ),
        (
            "<!--\n{FAIL}\n",
            then(html_at(&[1, 2]), &[fail()]),
            "an unclosed comment hides nothing: the FAIL in it counts",
        ),
        // Text after `-->` in the same block.
        (
            "<!-- note --> text\n{PASS}\n",
            html_at(&[1]),
            "text after the closer, on the closer's line",
        ),
        (
            "<!--\nnote\n--> text\n{PASS}\n",
            html_at(&[1, 2, 3]),
            "text after the closer of a comment spanning lines",
        ),
        (
            "<!-- a --> <!-- b -->\n{PASS}\n",
            html_at(&[1]),
            "two comments in one block: a `-->` before the end",
        ),
        (
            "<!-- a --> b -->\n{PASS}\n",
            html_at(&[1]),
            "a `-->` before the block's last `-->`",
        ),
        // `--!>`.
        (
            "<!-- a --!> b -->\n{PASS}\n",
            html_at(&[1]),
            "`--!>` before the closer of a one-line comment",
        ),
        (
            "<!--\n--!>\n{PASS}\n-->\n",
            html_at(&[1, 2, 3, 4]),
            "`--!>` in a comment spanning lines: not closed, so the PASS in it counts",
        ),
        (
            "Text <!-- a --!> b --> c\n{PASS}\n",
            html_at(&[1]),
            "`--!>` in an inline comment",
        ),
        // Inline tags outside the allow-list.
        (
            "Text <span>x</span> y\n{PASS}\n",
            html_at(&[1]),
            "two inline tags on one line: once per line",
        ),
        (
            "Text <b>x</b> and <i>y</i>\n{PASS}\n",
            html_at(&[1]),
            "four inline tags on one line: once per line",
        ),
        (
            "Text\nmore <img src=\"x\"> y\n{PASS}\n",
            html_at(&[2]),
            "an inline tag on the paragraph's second line",
        ),
        (
            "Text <span\ntitle=\"x\"> y\n{PASS}\n",
            html_at(&[1]),
            "an inline tag across two lines: the event's first line",
        ),
        (
            "| a | <ol><li>x</li></ol> |\n{PASS}\n",
            html_at(&[1]),
            "<ol> beside allowed <li> tags",
        ),
        (
            "| a <br clear=\"all\"> b |\n{PASS}\n",
            html_at(&[1]),
            "<br> with an attribute is not one of the allowed forms",
        ),
        (
            "Text </br> x\n{PASS}\n",
            html_at(&[1]),
            "</br> is not on the allow-list",
        ),
        (
            "Text <ul class=\"x\"> y\n{PASS}\n",
            html_at(&[1]),
            "<ul> with an attribute is not one of the allowed forms",
        ),
        ("Text <hr> y\n{PASS}\n", html_at(&[1]), "inline <hr>"),
    ];
    let failures = case_mismatches(&cases);
    assert!(
        failures.is_empty(),
        "LIB-TOOL-015: disallowed raw HTML gives exactly the raw-HTML problem, once per line \
         that holds it (D8):\n{failures:#?}"
    );
}

#[test]
fn lib_tool_015_closed_comments_and_the_allowed_inline_tags_are_not_problems() {
    // D8 allows a block that is one closed comment, an inline closed comment,
    // and the inline tags <br>, <br/>, <br />, <ul>, </ul>, <li>, </li> in any
    // case. The table rows have the form the real records use (for example
    // `0001-m0-tooling-verification-round6.md`). Candidates inside allowed
    // comments are hidden, look-alikes included.
    let cases: Vec<Case<'_>> = vec![
        (
            "<!-- note -->\n{PASS}\n",
            vec![],
            "a one-line comment block",
        ),
        (
            "<!--\nnote\n\nmore\n-->\n{PASS}\n",
            vec![],
            "a comment block spanning lines, with a blank line inside",
        ),
        (
            "   <!-- note -->  \n{PASS}\n",
            vec![],
            "an indented comment block with trailing blanks",
        ),
        (
            "- a\n\n  <!--\n  note\n  -->\n{PASS}\n",
            vec![],
            "a comment block in a list item",
        ),
        (
            "Text <!-- note --> more\n{PASS}\n",
            vec![],
            "an inline comment",
        ),
        (
            "Text <!-- a\nb --> c\n{PASS}\n",
            vec![],
            "an inline comment spanning lines",
        ),
        (
            "| LIB-TOOL-001 | All tests:<br>\u{2022} a<br>\u{2022} b |\n{PASS}\n",
            vec![],
            "<br> in a table row, as the real records use it",
        ),
        (
            "| a | b<br/>c<br />d |\n{PASS}\n",
            vec![],
            "<br/> and <br />",
        ),
        (
            "| a | <ul><li>x</li><li>y</li></ul> |\n{PASS}\n",
            vec![],
            "<ul>, <li>, </li> and </ul> in a table row",
        ),
        (
            "| A | x<BR>y<Br/>z<bR />w |\n{PASS}\n",
            vec![],
            "<br> forms in any case",
        ),
        (
            "| A | <UL><Li>x</lI></Ul> |\n{PASS}\n",
            vec![],
            "<ul> and <li> in any case",
        ),
        (
            "Text with a <br> break\n{PASS}\n",
            vec![],
            "<br> in a paragraph",
        ),
        (
            "<!--\n{FAIL}\n-->\n{PASS}\n",
            vec![],
            "a FAIL inside a closed comment is hidden",
        ),
        (
            "<!--\n{PASS}\n-->\n",
            vec![no_verdict()],
            "a PASS inside a closed comment is hidden",
        ),
        (
            "<!--\n{FAIL}\n\n  {FAIL}\n> {FAIL}\n-->\n{PASS}\n",
            vec![],
            "a candidate and look-alikes inside a closed comment are hidden",
        ),
        (
            "- a\n\n  <!--\n  {FAIL}\n  -->\n{PASS}\n",
            vec![],
            "a look-alike inside a comment in a list item is hidden",
        ),
    ];
    let failures = case_mismatches(&cases);
    assert!(
        failures.is_empty(),
        "LIB-TOOL-015: closed comments and the allowed inline tags are no problem, and a closed \
         comment hides what is inside it (D8):\n{failures:#?}"
    );
}

#[test]
fn lib_tool_015_the_verifiers_f3_and_f4b_probes_give_the_raw_html_problem_and_never_pass() {
    // Verification 0007 round 1. F3: D6 over-hid, giving a false PASS. F4(b):
    // text in an HTML block that a reader sees. Each probe now gives the
    // raw-HTML problem, and every verdict line in it counts.
    let cases: Vec<Case<'_>> = vec![
        (
            "{PASS}\n\n<!-- a --> <!--\n\n<x a=--!>\n\n{FAIL}\n",
            then(html_at(&[3, 5]), &[count(2)]),
            "F3 b1: `--!>` in a type 7 block after an unclosed comment",
        ),
        (
            "{PASS}\n\n<div><!--\n\n<p>--!>\n\n{FAIL}\n",
            then(html_at(&[3, 5]), &[count(2)]),
            "F3 b2: `--!>` in a <p> block",
        ),
        (
            "{PASS}\n\n<div><!--\n\nText <x a=--!> y\n\n{FAIL}\n",
            then(html_at(&[3, 5]), &[count(2)]),
            "F3 b3: `--!>` in an inline tag",
        ),
        (
            "{PASS}\n\nText <span title=\"<!--\"> x\n\n{FAIL}\n",
            then(html_at(&[3]), &[count(2)]),
            "F3 c1: `<!--` in an inline attribute value",
        ),
        (
            "{PASS}\n\n<div title=\"<!--\">\n\n{FAIL}\n",
            then(html_at(&[3]), &[count(2)]),
            "F3 c2: `<!--` in a block tag's attribute value",
        ),
        (
            "{PASS}\n\n<script>\n<!--\n</script>\n\n{FAIL}\n",
            then(html_at(&[3, 4, 5]), &[count(2)]),
            "F3 f1: `<!--` in <script>",
        ),
        (
            "{PASS}\n\n<style>\n<!--\n</style>\n\n{FAIL}\n",
            then(html_at(&[3, 4, 5]), &[count(2)]),
            "F3 f2: `<!--` in <style>",
        ),
        (
            "{PASS}\n\n<textarea>\n<!--\n</textarea>\n\n{FAIL}\n",
            then(html_at(&[3, 4, 5]), &[count(2)]),
            "F3 f3: `<!--` in <textarea>",
        ),
        ("<title>\n\n{PASS}\n", html_at(&[1]), "F4(b) d1: <title>"),
        (
            "<iframe>\n\n{PASS}\n\n</iframe>\n",
            html_at(&[1, 5]),
            "F4(b) d2: <iframe>",
        ),
        (
            "<div title=\"\n\n{PASS}\n",
            html_at(&[1]),
            "F4(b) d3: an unclosed attribute value",
        ),
        (
            "<div hidden>\n\n{PASS}\n\n</div>\n",
            html_at(&[1, 5]),
            "F4(b) d4: <div hidden>",
        ),
        (
            "<noframes>\n\n{PASS}\n",
            html_at(&[1]),
            "F4(b) d5: <noframes>",
        ),
        (
            "<div>\n{FAIL}\n</div>\n",
            then(html_at(&[1, 2, 3]), &[fail()]),
            "F4(b): a FAIL in a <div> block, which a reader sees",
        ),
    ];
    let failures = case_mismatches(&cases);
    assert!(
        failures.is_empty(),
        "LIB-TOOL-015: each F3 and F4(b) probe gives the raw-HTML problem and does not pass \
         (D8):\n{failures:#?}"
    );
}

#[test]
fn lib_tool_015_the_verifiers_f1_probes_give_the_raw_html_problem_and_never_pass() {
    // Verification 0007 round 1, F1: pulldown-cmark 0.13.6 ends a type 1 HTML
    // block only at the matching lowercase end tag, where CommonMark 0.31.2
    // §4.6 ends it at a line holding any of `</pre>`, `</script>`,
    // `</style>`, `</textarea>`, in any case. Under D8 the block hides
    // nothing, so the deviation can only add raw-HTML problems on lines that
    // pulldown-cmark keeps in the block. Each probe must give the raw-HTML
    // problem on every line CommonMark puts in the block, more raw-HTML
    // problems only after it, and then exactly the verdict problems the
    // visible verdict lines give.
    let probes: [(&str, &[usize], Vec<String>, &str); 7] = [
        (
            "{PASS}\n\n<pre>\n</textarea>\n{FAIL}\n",
            &[3, 4],
            vec![count(2)],
            "g1: <pre> ended by </textarea>",
        ),
        (
            "<style>\n</script>\n{PASS}\n",
            &[1, 2],
            vec![],
            "g2: <style> ended by </script>",
        ),
        (
            "{PASS}\n\n<script>\nx\n</style>\n\n{FAIL}\n",
            &[3, 4, 5],
            vec![count(2)],
            "g3: <script> ended by </style>",
        ),
        (
            "{PASS}\n\n<pre>x</script>\n{FAIL}\n",
            &[3],
            vec![count(2)],
            "g4: <pre> ended on its own line by </script>",
        ),
        (
            "{PASS}\n\n<textarea>\n\n</pre>\n{FAIL}\n",
            &[3, 5],
            vec![count(2)],
            "g5: <textarea> ended by </pre> past a blank line",
        ),
        (
            "{PASS}\n\n<pre>\n</PRE>\n{FAIL}\n",
            &[3, 4],
            vec![count(2)],
            "g6 (the F1 example): <pre> ended by </PRE>",
        ),
        (
            "<pre>\n</PRE>\n{PASS}\n",
            &[1, 2],
            vec![],
            "the F1 reverse case: <pre> ended by </PRE>, then a PASS",
        ),
    ];
    let mut failures = Vec::new();
    for (template, block, verdict, why) in &probes {
        for record in [fill(template), crlf(&fill(template))] {
            let got = record_problems(&record);
            let html_lines: Vec<usize> = got
                .iter()
                .map_while(|p| {
                    p.strip_suffix(&format!(": {RAW_HTML}"))
                        .map(|p| problem_key(&format!("{p}: x")).1)
                })
                .collect();
            let rest = &got[html_lines.len()..];
            let ordered = html_lines.windows(2).all(|w| w[0] < w[1]);
            let covers = block.iter().all(|n| html_lines.contains(n));
            let after = html_lines.iter().all(|n| *n >= block[0]);
            if got.is_empty() || !ordered || !covers || !after || rest != verdict.as_slice() {
                failures.push(format!(
                    "{why}: the record {record:?} gave {got:#?}; expected raw HTML on lines \
                     {block:?} (and perhaps later lines), then {verdict:#?}"
                ));
            }
        }
    }
    // §4.6: `</pre>` alone is not a type 1 start. After a paragraph line it
    // is a type 7 tag, which cannot interrupt a paragraph, so it is inline
    // HTML in a lazy or continuation line; at the start it is a type 7 block,
    // which the PASS line continues.
    failures.extend(case_mismatches(&[
        (
            "- a\n</pre>\n{PASS}\n",
            html_at(&[2]),
            "h1: a lazy </pre> line after a list item",
        ),
        (
            "x\n</pre>\n{PASS}\n",
            html_at(&[2]),
            "h2: </pre> continuing a paragraph",
        ),
        (
            "</pre>\n{PASS}\n",
            html_at(&[1, 2]),
            "h3: </pre> starting a type 7 block",
        ),
    ]));
    assert!(
        failures.is_empty(),
        "LIB-TOOL-015: each F1 probe gives the raw-HTML problem and does not pass (D8):\n\
         {failures:#?}"
    );
}

// ---------------------------------------------------------------------------
// LIB-TOOL-005, slice 0007 D8 to D11: strict records
// ---------------------------------------------------------------------------

#[test]
fn lib_tool_005_the_strict_record_problem_texts_are_exact() {
    // The texts of D8, D9 and D10, written out in full once.
    for (record, expected, what) in [
        (
            fill("<div>\n\n{PASS}\n"),
            "LIB-TOOL-001: verification record docs/verification/v.md:1: raw HTML other than a \
             closed comment or <br>, <ul>, <li> is not allowed",
            "D8, raw HTML on line 1",
        ),
        (
            fill(" {PASS}\n\n{PASS}\n"),
            "LIB-TOOL-001: verification record docs/verification/v.md:1: a verdict-like line is \
             not at column 0",
            "D9, a look-alike on line 1",
        ),
        (
            fill("{PASS}\n\nx\ry\n"),
            "LIB-TOOL-001: verification record docs/verification/v.md:3: lone carriage return; \
             use LF or CRLF line endings",
            "D10, a lone CR on line 3",
        ),
        (
            fill("\u{feff}# V\n\n{PASS}\n"),
            "LIB-TOOL-001: verification record docs/verification/v.md: starts with a byte-order \
             mark",
            "D10, a leading BOM",
        ),
    ] {
        assert_eq!(
            record_problems(&record),
            [expected],
            "LIB-TOOL-005: {what} gives exactly the problem text of the slice 0007 brief; the \
             record is {record:?}"
        );
    }
}

#[test]
fn lib_tool_005_a_visible_verdict_like_line_not_at_column_0_is_a_problem() {
    // D9: a line outside every hidden range that, with its leading spaces,
    // tabs, `>` and list markers removed, starts with `Row verdict:`, and that
    // is not a column-0 candidate. The column-0 PASS still counts.
    let failures = case_mismatches(&look_alike_cases());
    assert!(
        failures.is_empty(),
        "LIB-TOOL-005: a visible verdict-like line that is not at column 0 is a problem at its \
         line (D9):\n{failures:#?}"
    );
}

/// Visible verdict-like lines that are not at column 0, and the problems
/// each record gives under slice 0007 D9.
fn look_alike_cases() -> Vec<Case<'static>> {
    vec![
        (
            " {FAIL}\n\n{PASS}\n",
            vec![look_alike(1)],
            "indented 1 space",
        ),
        (
            "  {FAIL}\n\n{PASS}\n",
            vec![look_alike(1)],
            "indented 2 spaces",
        ),
        (
            "   {FAIL}\n\n{PASS}\n",
            vec![look_alike(1)],
            "indented 3 spaces",
        ),
        (
            "- a\n  {FAIL}\n{PASS}\n",
            vec![look_alike(2)],
            "a nested bullet item",
        ),
        (
            "1. a\n   {FAIL}\n{PASS}\n",
            vec![look_alike(2)],
            "a bullet item nested in an ordered item",
        ),
        (
            "* Row verdict: LIB-TOOL-001: FAIL\n\n{PASS}\n",
            vec![look_alike(1)],
            "a `*` marker",
        ),
        (
            "+ Row verdict: LIB-TOOL-001: FAIL\n\n{PASS}\n",
            vec![look_alike(1)],
            "a `+` marker",
        ),
        (
            "1. Row verdict: LIB-TOOL-001: FAIL\n\n{PASS}\n",
            vec![look_alike(1)],
            "a `1.` marker",
        ),
        (
            "1) Row verdict: LIB-TOOL-001: FAIL\n\n{PASS}\n",
            vec![look_alike(1)],
            "a `1)` marker",
        ),
        (
            "Row verdict: LIB-TOOL-001: FAIL\n\n{PASS}\n",
            vec![look_alike(1)],
            "no marker at all",
        ),
        ("> {FAIL}\n\n{PASS}\n", vec![look_alike(1)], "block-quoted"),
        (
            ">{FAIL}\n\n{PASS}\n",
            vec![look_alike(1)],
            "block-quoted with no space",
        ),
        (
            "> > {FAIL}\n\n{PASS}\n",
            vec![look_alike(1)],
            "in a nested block quote",
        ),
        (
            "> * Row verdict: x\n\n{PASS}\n",
            vec![look_alike(1)],
            "a malformed look-alike in a block quote's list",
        ),
        (
            "Text\n\t{FAIL}\n\n{PASS}\n",
            vec![look_alike(2)],
            "tab-indented paragraph continuation text (an indented code block cannot \
             interrupt a paragraph, §4.4)",
        ),
        (
            "- a\n\t{FAIL}\n{PASS}\n",
            vec![look_alike(2)],
            "a tab-indented nested item",
        ),
        (
            "  {PASS}\n",
            vec![look_alike(1), no_verdict()],
            "an indented PASS is no verdict line",
        ),
    ]
    .into_iter()
    .chain(f4a_and_multi_digit_marker_cases())
    .collect()
}

/// More of [`look_alike_cases`]: the verifier's F4(a) probes, and ordered
/// markers of several digits. An ordered marker has 1 to 9 digits (§5.2);
/// those cases kill the mutant that strips only one digit (H1).
fn f4a_and_multi_digit_marker_cases() -> Vec<Case<'static>> {
    vec![
        // Verification 0007 round 1, F4(a): indented or nested verdict items
        // that a reader sees.
        (
            "{PASS}\n\n  {FAIL}\n",
            vec![look_alike(3)],
            "F4(a): a FAIL nested in the PASS item",
        ),
        (
            "- Probe:\n  {FAIL}\n\n{PASS}\n",
            vec![look_alike(2)],
            "F4(a): a FAIL nested in a bullet item",
        ),
        (
            "1. Probe:\n\n   {FAIL}\n\n{PASS}\n",
            vec![look_alike(3)],
            "F4(a): a FAIL nested in an ordered item",
        ),
        (
            "10. Row verdict: LIB-TOOL-001: FAIL\n\n{PASS}\n",
            vec![look_alike(1)],
            "a two-digit `10.` marker",
        ),
        (
            "123) Row verdict: LIB-TOOL-001: FAIL\n\n{PASS}\n",
            vec![look_alike(1)],
            "a three-digit `123)` marker",
        ),
        (
            "- a\n  10. Row verdict: x\n{PASS}\n",
            vec![look_alike(2)],
            "a nested two-digit marker",
        ),
    ]
}

#[test]
fn lib_tool_005_a_verdict_like_line_in_a_code_block_or_closed_comment_is_not_a_problem() {
    // D9 looks only at lines outside every hidden range. An indented code
    // block may start after its indent in the parser's offsets; the line is
    // in the block all the same.
    let cases: Vec<Case<'_>> = vec![
        (
            "```\n  {FAIL}\n> {FAIL}\n* Row verdict: x\nRow verdict: x\n```\n{PASS}\n",
            vec![],
            "look-alikes in a fenced code block",
        ),
        (
            "~~~\n1. Row verdict: LIB-TOOL-001: FAIL\n~~~\n{PASS}\n",
            vec![],
            "a look-alike in a tilde fence",
        ),
        (
            "    {FAIL}\n\n{PASS}\n",
            vec![],
            "the first line of an indented code block",
        ),
        (
            "Text.\n\n     {FAIL}\n\n{PASS}\n",
            vec![],
            "a line indented 5 spaces after a blank line",
        ),
        ("\t{FAIL}\n\n{PASS}\n", vec![], "a tab-indented code line"),
        (
            "- a\n\n      {FAIL}\n{PASS}\n",
            vec![],
            "an indented code block in a list item",
        ),
        (
            "> ```\n> {FAIL}\n> ```\n{PASS}\n",
            vec![],
            "a fence in a block quote",
        ),
        (
            "<!--\n  {FAIL}\n> {FAIL}\n-->\n{PASS}\n",
            vec![],
            "look-alikes in a closed comment",
        ),
    ];
    let failures = case_mismatches(&cases);
    assert!(
        failures.is_empty(),
        "LIB-TOOL-005: a look-alike inside a code block or a closed comment is not a problem \
         (D9):\n{failures:#?}"
    );
}

#[test]
fn lib_tool_005_a_column_0_candidate_is_never_a_verdict_like_problem() {
    let cases: Vec<Case<'_>> = vec![
        ("{PASS}\n", vec![], "a PASS line"),
        ("{PASS} \t\n", vec![], "a PASS line with trailing blanks"),
        ("{FAIL}\n", vec![fail()], "a FAIL line"),
        (
            "- Row verdict: x\n{PASS}\n",
            vec![malformed(ROW, 1)],
            "a malformed candidate",
        ),
        (
            "- Row verdict: LIB-TOOL-001 PASS\n",
            vec![malformed(ROW, 1), no_verdict()],
            "no colon before the verdict: malformed, not a look-alike",
        ),
        (
            "- Row verdict:LIB-TOOL-001: PASS\n",
            vec![malformed(ROW, 1), no_verdict()],
            "no space after the prefix: malformed, not a look-alike",
        ),
        (
            "- Row verdict: LIB-TOOL-002: PASS\n{PASS}\n",
            vec![],
            "a verdict line for another row",
        ),
        (
            "Text\n{PASS}\n",
            vec![],
            "a PASS line interrupting a paragraph",
        ),
        (
            "<div>\n{PASS}\n",
            html_at(&[1, 2]),
            "a candidate inside raw HTML is raw HTML, not a look-alike",
        ),
    ];
    let failures = case_mismatches(&cases);
    assert!(
        failures.is_empty(),
        "LIB-TOOL-005: a column-0 candidate is a verdict line or malformed, never a \
         verdict-like problem (D9):\n{failures:#?}"
    );
}

#[test]
fn lib_tool_005_a_lone_cr_is_a_problem_at_the_line_that_holds_it() {
    // D10: `<line>` is the line, by LF/CRLF splitting, that holds the CR.
    // Every case also runs with each LF made CRLF; the lone CR stays lone.
    let cases: Vec<Case<'_>> = vec![
        (
            "{PASS}\n\ntext\rmore\n",
            vec![lone_cr(3)],
            "a lone CR inside line 3",
        ),
        (
            "# V\rText\n\n{PASS}\n",
            vec![lone_cr(1)],
            "a lone CR inside line 1",
        ),
        (
            "{PASS}\n\nend\r",
            vec![lone_cr(3)],
            "a lone CR at the end of the record",
        ),
        (
            "a\rb\n{PASS}\nc\rd\n",
            vec![lone_cr(1), lone_cr(3)],
            "lone CRs on lines 1 and 3, in line order",
        ),
        (
            "a\r\rb\rc\n{PASS}\n",
            vec![lone_cr(1)],
            "three lone CRs on one line give one problem (decision 4)",
        ),
        (
            "\r{PASS}\n",
            vec![lone_cr(1), no_verdict()],
            "a PASS after a lone CR is not at the start of its LF/CRLF line",
        ),
    ];
    let failures = case_mismatches(&cases);
    assert!(
        failures.is_empty(),
        "LIB-TOOL-005: a lone CR is a problem at the LF/CRLF line that holds it (D10):\n\
         {failures:#?}"
    );
}

#[test]
fn lib_tool_005_the_verifiers_f2_probes_never_pass() {
    // Verification 0007 round 1, F2: a lone CR, or a leading BOM, hid a FAIL.
    let cases: Vec<Case<'_>> = vec![
        (
            "{PASS}\n\ntext\r{FAIL}\n",
            vec![lone_cr(3)],
            "a1: a FAIL after text and a lone CR",
        ),
        (
            "{PASS}\n\n\r{FAIL}\n",
            vec![lone_cr(3)],
            "a2: a FAIL after a lone CR on a blank line",
        ),
        (
            "text\r{PASS}\n",
            vec![lone_cr(1), no_verdict()],
            "a3: the only PASS after a lone CR",
        ),
    ];
    let mut failures = case_mismatches(&cases);
    // a4 holds three lone CRs on its one LF/CRLF line: one problem for the
    // line (decision 4 of the fix round).
    failures.extend(mismatches(
        &[("# V\r\r{PASS}\r", "a4: every line ends in a lone CR")],
        &[lone_cr(1), no_verdict()],
    ));
    // e1 and e2: a candidate right after the leading BOM is not at column 0,
    // so it does not count (decision 5 of the fix round). In e1 the FAIL is
    // hidden from the verdict that way, but the BOM problem refuses the record.
    failures.extend(case_mismatches(&[
        (
            "\u{feff}{FAIL}\n\n{PASS}\n",
            vec![bom(ROW)],
            "e1: a BOM before a FAIL line",
        ),
        (
            "\u{feff}{PASS}\n",
            vec![bom(ROW), no_verdict()],
            "e2: a BOM before the only PASS line",
        ),
    ]));
    assert!(
        failures.is_empty(),
        "LIB-TOOL-005: no F2 probe passes the row (D10):\n{failures:#?}"
    );
}

#[test]
fn lib_tool_005_crlf_line_endings_are_not_a_problem() {
    let failures = mismatches(
        &[
            ("{PASS}\r\n", "one CRLF line"),
            (
                "# V\r\n\r\n{PASS}\r\n",
                "a heading, a blank line and a PASS",
            ),
            ("```\r\n{FAIL}\r\n```\r\n{PASS}\r\n", "a fenced FAIL"),
            ("<!--\r\n{FAIL}\r\n-->\r\n{PASS}\r\n", "a commented FAIL"),
            ("| a<br>b |\r\n{PASS}\r\n", "an allowed inline tag"),
            ("{PASS}", "no line ending at all"),
        ],
        &[],
    );
    assert!(
        failures.is_empty(),
        "LIB-TOOL-005: CRLF line endings are no problem (D10):\n{failures:#?}"
    );
}

#[test]
fn lib_tool_005_a_leading_byte_order_mark_is_a_problem() {
    let mut failures = mismatches_lf_and_crlf(
        &[
            ("\u{feff}# V\n\n{PASS}\n", "a BOM before a heading"),
            ("\u{feff}\n{PASS}\n", "a BOM alone on line 1"),
        ],
        &[bom(ROW)],
    );
    failures.extend(mismatches_lf_and_crlf(
        &[(
            "# V \u{feff}\n\n{PASS}\n",
            "a U+FEFF that does not lead the record",
        )],
        &[],
    ));
    // Decision 5 of the fix round: a candidate right after the leading BOM is
    // not at column 0, so it does not count, and it is not a look-alike.
    failures.extend(mismatches_lf_and_crlf(
        &[("\u{feff}{PASS}\n", "the only PASS line right after the BOM")],
        &[bom(ROW), no_verdict()],
    ));
    failures.extend(mismatches_lf_and_crlf(
        &[(
            "\u{feff}{PASS}\n{PASS}\n",
            "a PASS right after the BOM, then a PASS on line 2",
        )],
        &[bom(ROW)],
    ));
    assert!(
        failures.is_empty(),
        "LIB-TOOL-005: a leading byte-order mark is a problem, and only a leading one (D10):\n\
         {failures:#?}"
    );
}

/// A record with one problem of each kind on its own line: a BOM, a
/// look-alike on line 3, raw HTML on line 4, a lone CR on line 6, a malformed
/// line 7, and a FAIL line 8 for [`ROW`].
const ONE_OF_EACH: &str = "\u{feff}# V\n\n  {PASS}\n<div>\n\nx\ry\n- Row verdict: x\n{FAIL}\n";

#[test]
fn lib_tool_005_strict_problems_come_in_line_order_before_the_verdict_problem_bom_first() {
    // D11. Line 3 is an indented item (D9); `<div>` closes the list as a type
    // 6 block (§4.6), which the blank line 5 ends (D8); line 6 holds a lone CR
    // (D10); line 7 is malformed; line 8 is the FAIL.
    let expected = [
        bom(ROW),
        look_alike(3),
        html(4),
        lone_cr(6),
        malformed(ROW, 7),
        fail(),
    ];
    for record in [fill(ONE_OF_EACH), crlf(&fill(ONE_OF_EACH))] {
        assert_eq!(
            record_problems(&record),
            expected,
            "LIB-TOOL-005: the BOM problem first, then the problems at lines in line order, then \
             the verdict problem (D11); the record is {record:?}"
        );
    }
}

#[test]
fn lib_tool_005_a_shared_record_reports_each_strict_problem_once_per_citing_row() {
    // D11 with D5 of slice 0004: two rows cite the record. Each gets every
    // problem once, in the same order, and its own verdict problem; [`OTHER`]
    // has no verdict line.
    let mut expected = Vec::new();
    for id in [ROW, OTHER] {
        expected.extend([
            bom(id),
            at_line(id, 3, LOOK_ALIKE),
            at_line(id, 4, RAW_HTML),
            at_line(id, 6, LONE_CR),
            malformed(id, 7),
        ]);
        expected.push(if id == ROW {
            fail()
        } else {
            format!(
                "{OTHER}: verification record {RECORD} has no `- Row verdict: {OTHER}: PASS` line"
            )
        });
    }
    for record in [fill(ONE_OF_EACH), crlf(&fill(ONE_OF_EACH))] {
        assert_eq!(
            shared_record_problems(&record),
            expected,
            "LIB-TOOL-005: each strict problem is reported once per citing row, in row order \
             (D11); the record is {record:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// Slice 0007 fix round 1, the lead's decisions (a) and (c), 4 to 6
// ---------------------------------------------------------------------------

#[test]
fn lib_tool_015_an_allowed_tag_at_the_start_of_a_line_is_raw_html() {
    // Decision (a): an allowed inline tag counts as allowed only when
    // non-whitespace text comes before it on its line. At the start of a
    // line, after an indent of 0 to 3, it is raw HTML: there CommonMark reads
    // a lazy or continuation line as inline HTML, but cmark-family renderers
    // may start a type 7 HTML block that hides the lines after it.
    let cases: Vec<Case<'_>> = vec![
        (
            "- note\n<br>\n{PASS}\n",
            html_at(&[2]),
            "a lazy <br> line after a list item",
        ),
        (
            "> note\n<br>\n{PASS}\n",
            html_at(&[2]),
            "a lazy <br> line after a block quote",
        ),
        (
            "Text\n<br>\n{PASS}\n",
            html_at(&[2]),
            "a <br> line continuing a paragraph",
        ),
        (
            "Text\n   <BR/> more\n{PASS}\n",
            html_at(&[2]),
            "<BR/> after an indent of 3, then text",
        ),
        (
            "<br>\n{PASS}\n",
            html_at(&[1, 2]),
            "a line that is only <br> starts a type 7 block, which the PASS line continues",
        ),
        (
            "Text\n<ul><li>x</li></ul> y\n{PASS}\n",
            html_at(&[2, 3]),
            "<ul> at the start of a line starts a type 6 block",
        ),
        (
            "Text\n</li> x\n{PASS}\n",
            html_at(&[2, 3]),
            "</li> at the start of a line starts a type 6 block",
        ),
        // Controls: text before the tag on its line.
        ("| a<br>b |\n{PASS}\n", vec![], "<br> inside a table row"),
        (
            "| <ul><li>x</li></ul> |\n{PASS}\n",
            vec![],
            "<ul> after the row's `|`",
        ),
        ("Text <br>\n{PASS}\n", vec![], "<br> at the end of a line"),
        ("a<br>\n{PASS}\n", vec![], "<br> right after text"),
        (
            "Text\nx <BR/> y\n{PASS}\n",
            vec![],
            "<BR/> after text on a continuation line",
        ),
    ];
    let failures = case_mismatches(&cases);
    assert!(
        failures.is_empty(),
        "LIB-TOOL-015: an allowed tag at the start of a line is raw HTML; after text on its line \
         it is allowed (decision (a)):\n{failures:#?}"
    );
}

#[test]
fn lib_tool_005_problems_on_one_line_come_as_raw_html_then_lone_cr_then_malformed() {
    // Decision (c) of the fix round, refining D11.
    let cases: Vec<Case<'_>> = vec![
        (
            "<div>\n- Row verdict: x\ry\n\n{PASS}\n",
            vec![html(1), html(2), lone_cr(2), malformed(ROW, 2)],
            "raw HTML, a lone CR and a malformed candidate on line 2",
        ),
        (
            "<div>\n- Row verdict: x\n\n{PASS}\n",
            vec![html(1), html(2), malformed(ROW, 2)],
            "raw HTML and a malformed candidate on line 2",
        ),
        (
            "- Row verdict: x\ry\n{PASS}\n",
            vec![lone_cr(1), malformed(ROW, 1)],
            "a lone CR and a malformed candidate on line 1",
        ),
        (
            "Text <span>\r{PASS}\n",
            vec![html(1), lone_cr(1), no_verdict()],
            "raw HTML and a lone CR on line 1; the PASS after the CR is no candidate",
        ),
    ];
    let failures = case_mismatches(&cases);
    assert!(
        failures.is_empty(),
        "LIB-TOOL-005: on one line, raw HTML comes first, then the lone CR, then the malformed \
         line (decision (c)):\n{failures:#?}"
    );
}

#[test]
fn lib_tool_005_a_verdict_line_whose_id_is_not_a_well_formed_row_id_is_malformed() {
    // Decision 6 of the fix round. A well-formed row id is
    // `<SACM23|LIB>-<AREA>-<NNN>`: one of the matrix's prefixes, an area of
    // uppercase ASCII letters and three digits (`docs/matrix/README.md` rule
    // 2, as the matrix gate checks row ids). Before this, a line such as
    // `- Row verdict:  LIB-TOOL-001: PASS` was read as a verdict for the id
    // ` LIB-TOOL-001` and ignored.
    let ids = [
        (" LIB-TOOL-001", "two spaces after `Row verdict:`"),
        ("lib-tool-001", "a lowercase id"),
        ("LIB-tool-001", "a lowercase area"),
        ("LIB-TOOL-0011", "four digits"),
        ("LIB-TOOL-00", "two digits"),
        ("LIB-TOOL-001x", "a letter after the digits"),
        ("LIB-TOOL-001-2", "a fourth segment"),
        ("LIB-TOOL-001 ", "a space before the colon"),
        ("LIBTOOL001", "no dashes"),
        ("XLIB-TOOL-001", "a prefix the matrix does not define"),
    ];
    let mut failures = Vec::new();
    for (id, why) in ids {
        let line = format!("- Row verdict: {id}: PASS");
        let beside = format!("{{PASS}}\n{line}\n");
        failures.extend(mismatches_lf_and_crlf(
            &[(beside.as_str(), why)],
            &[malformed(ROW, 2)],
        ));
        let alone = format!("{line}\n");
        failures.extend(mismatches_lf_and_crlf(
            &[(alone.as_str(), why)],
            &[malformed(ROW, 1), no_verdict()],
        ));
    }
    // Controls: well-formed ids of other rows are verdict lines that do not
    // count for [`ROW`].
    failures.extend(mismatches_lf_and_crlf(
        &[
            (
                "{PASS}\n- Row verdict: LIB-TOOL-002: PASS\n",
                "a well-formed id of another LIB row",
            ),
            (
                "{PASS}\n- Row verdict: SACM23-BASE-001: OPEN\n",
                "a well-formed SACM23 id",
            ),
        ],
        &[],
    ));
    assert!(
        failures.is_empty(),
        "LIB-TOOL-005: a candidate whose id part is not a well-formed row id is malformed \
         (decision 6):\n{failures:#?}"
    );
}

#[test]
fn lib_tool_005_a_look_alike_is_reported_last_on_its_line() {
    // D13(c): on a line that also holds raw HTML or a lone CR, the look-alike
    // problem comes after them. A look-alike never shares a line with a
    // malformed line, since a malformed line is a column-0 candidate.
    let cases: Vec<Case<'_>> = vec![
        (
            " - Row verdict: x\ry\n\n{PASS}\n",
            vec![lone_cr(1), look_alike(1)],
            "a lone CR, then the look-alike, on line 1",
        ),
        (
            " - Row verdict: x <span>y</span>\n\n{PASS}\n",
            vec![html(1), look_alike(1)],
            "inline raw HTML, then the look-alike, on line 1",
        ),
        (
            " - Row verdict: x <span>\ry\n\n{PASS}\n",
            vec![html(1), lone_cr(1), look_alike(1)],
            "raw HTML, a lone CR, then the look-alike, on line 1",
        ),
        (
            "<div>\n  - Row verdict: x\n\n{PASS}\n",
            vec![html(1), html(2), look_alike(2)],
            "a look-alike inside an HTML block: the block's raw HTML first",
        ),
    ];
    let failures = case_mismatches(&cases);
    assert!(
        failures.is_empty(),
        "LIB-TOOL-005: the look-alike problem comes last on its line (D13(c)):\n{failures:#?}"
    );
}

// ---------------------------------------------------------------------------
// Slice 0007 D14, withdrawn: closed inline comments hide nothing
// ---------------------------------------------------------------------------
//
// Slice 0007 round 3, D14 withdrawn. D14 (from the second Copilot review of
// PR #14) hid a line whose whole content lay inside one closed inline
// comment. Verification round 3 found records where such a line is visible
// on GitHub: the comment's text ends in `-`, the line is a GFM table cell or
// footnote, or the comment is an image description. The maintainer chose
// route W: a closed inline comment is allowed (D8) but hides nothing, so a
// look-alike inside it is a D9 problem, as at `ee4ef31` (round 2). That is
// fail-safe and matches strict records (ADR 0008 amendment 1).
//
// A closed comment inside a paragraph is one `InlineHtml` event, from the
// `<` of `<!--` to the `>` of `-->`. Block structure comes first (§3.2), so a
// line inside it is paragraph continuation text. A line that starts a block
// quote or a list item at an indent of 0 to 3 ends the paragraph, and then
// no comment spans it (see
// `lib_tool_005_a_line_that_ends_the_paragraph_is_never_inside_an_inline_comment`).
// A `>` or a list marker after an indent of 4 or more is continuation text,
// since an indented code block cannot interrupt a paragraph (§4.4). So a
// column-0 candidate never lies inside an inline comment, and withdrawing
// D14 changes only which look-alikes are reported.

#[test]
fn lib_tool_015_a_look_alike_inside_a_closed_inline_comment_is_reported() {
    // Formerly `lib_tool_015_a_line_wholly_inside_a_closed_inline_comment_is_hidden`.
    // Slice 0007 round 3, D14 withdrawn: each record is the one that test
    // used, and every look-alike in it, inside the comment or not, is now a
    // D9 problem. The comment itself is allowed (D8), so there is no other
    // problem, and the PASS line counts.
    let cases: Vec<Case<'_>> = vec![
        (
            "Text <!-- note\nRow verdict: LIB-TOOL-001: FAIL\n--> more\n{PASS}\n",
            vec![look_alike(2)],
            "the Copilot record: line 2 lies wholly inside the comment",
        ),
        (
            "Text <!-- note\nRow verdict: LIB-TOOL-001: FAIL\nplain\n Row verdict: x\n   \
             Row verdict: LIB-TOOL-002: PASS\n--> more\n{PASS}\n",
            vec![look_alike(2), look_alike(4), look_alike(5)],
            "a comment over five lines with three look-alikes",
        ),
        (
            "  > Text <!-- note\n  > Row verdict: LIB-TOOL-001: FAIL\n  > -->\n{PASS}\n",
            vec![look_alike(2)],
            "a line written `  > Row verdict:` in an indented block quote",
        ),
        (
            "> Text <!-- note\nRow verdict: LIB-TOOL-001: FAIL\n> -->\n\n{PASS}\n",
            vec![look_alike(2)],
            "a lazy continuation line of a block quote",
        ),
        (
            "Text <!-- note\n    * Row verdict: LIB-TOOL-001: FAIL\n\t> Row verdict: x\n-->\n{PASS}\n",
            vec![look_alike(2), look_alike(3)],
            "lines `    * Row verdict:` and `\\t> Row verdict:`: indented 4 columns, they are \
             continuation text, not a list item or block quote",
        ),
        (
            "- Text <!-- note\n  Row verdict: LIB-TOOL-001: FAIL\n  -->\n{PASS}\n",
            vec![look_alike(2)],
            "a line in a list item",
        ),
        (
            "Text <!-- note\n Row verdict: x\n   Row verdict: LIB-TOOL-001 PASS\n    {FAIL}\n-->\n{PASS}\n",
            vec![look_alike(2), look_alike(3), look_alike(4)],
            "malformed-looking lines indented 1, 3 and 4 spaces, the last a whole candidate",
        ),
        (
            "Text <!-- a\nRow verdict: LIB-TOOL-001: FAIL\n--> and <!-- b\n Row verdict: x\n-->\n{PASS}\n",
            vec![look_alike(2), look_alike(4)],
            "two comments in one paragraph, each with one look-alike",
        ),
        (
            "Text <!-- note\nRow verdict: x\n-->\nRow verdict: LIB-TOOL-001: FAIL\n{PASS}\n",
            vec![look_alike(2), look_alike(4)],
            "a look-alike inside the comment, then one after the closer",
        ),
    ];
    let failures = case_mismatches(&cases);
    assert!(
        failures.is_empty(),
        "LIB-TOOL-015: a closed inline comment hides nothing (D14 withdrawn, slice 0007 round 3), \
         so a look-alike inside it is a problem (D9):\n{failures:#?}"
    );
}

#[test]
fn lib_tool_015_a_look_alike_that_ends_with_the_inline_comment_closer_is_reported() {
    // Formerly `lib_tool_015_a_line_that_ends_with_the_inline_comment_closer_is_hidden`.
    // Slice 0007 round 3, D14 withdrawn: the line's content ends with the
    // comment's `-->` (with CRLF, the CR follows it), and it is still a
    // look-alike. The comment is allowed (D8), so there is no other problem.
    let cases: Vec<Case<'_>> = vec![
        (
            "Text <!-- note\nRow verdict: LIB-TOOL-001: FAIL -->\n{PASS}\n",
            vec![look_alike(2)],
            "a look-alike that ends with the closer",
        ),
        (
            "Text <!-- note\nmore\n  Row verdict: x-->\n{PASS}\n",
            vec![look_alike(3)],
            "an indented malformed look-alike that ends with the closer",
        ),
        (
            "> Text <!-- note\n> Row verdict: LIB-TOOL-001: FAIL -->\n\n{PASS}\n",
            vec![look_alike(2)],
            "a block-quoted look-alike that ends with the closer",
        ),
    ];
    let failures = case_mismatches(&cases);
    assert!(
        failures.is_empty(),
        "LIB-TOOL-015: a look-alike whose content ends with the inline comment's `-->` is a \
         problem, with LF and with CRLF (D14 withdrawn, slice 0007 round 3; D9):\n{failures:#?}"
    );
}

// Slice 0007 round 3, D14 withdrawn: the verifier's probes (a) to (d). Each
// record, with the PASS line added, passed while D14 was in and was refused
// at `ee4ef31`; on GitHub a reader sees `Row verdict: LIB-TOOL-001: FAIL` in
// each. Under core `CommonMark` 0.31.2 every comment below is one closed
// inline comment (§6.6: the text after `<!--` holds no `-->`), so it is
// allowed (D8) and the only problem is the look-alike (D9). Each case runs
// with LF and with CRLF, and none passes.

#[test]
fn lib_tool_005_a_look_alike_in_a_comment_whose_text_ends_in_a_hyphen_is_reported() {
    // Probe (a). `--->` closes the comment in `CommonMark` 0.31.2: its text is
    // everything up to the last `-->`, here ending in `-`. GitHub's cmark-gfm
    // and markdown-it read the same source as text. Either way the
    // look-alike is visible text to some renderer, and with D14 withdrawn it
    // is a problem. Each form is written in a paragraph, a block quote and a
    // list item.
    let cases: Vec<Case<'_>> = vec![
        (
            "Text <!-- a\nRow verdict: LIB-TOOL-001: FAIL\n--->\n{PASS}\n",
            vec![look_alike(2)],
            "probe (a): `--->` on its own line, in a paragraph",
        ),
        (
            "Text <!-- a\nRow verdict: LIB-TOOL-001: FAIL --->\n{PASS}\n",
            vec![look_alike(2)],
            "probe (a): `--->` ending the look-alike's line, in a paragraph",
        ),
        (
            "Text <!-- a\nRow verdict: LIB-TOOL-001: FAIL\nx--->\n{PASS}\n",
            vec![look_alike(2)],
            "probe (a): `x--->` on its own line, in a paragraph",
        ),
        (
            "> Text <!-- a\n> Row verdict: LIB-TOOL-001: FAIL\n> --->\n\n{PASS}\n",
            vec![look_alike(2)],
            "probe (a): `--->` on its own line, in a block quote",
        ),
        (
            "> Text <!-- a\n> Row verdict: LIB-TOOL-001: FAIL --->\n\n{PASS}\n",
            vec![look_alike(2)],
            "probe (a): `--->` ending the look-alike's line, in a block quote",
        ),
        (
            "> Text <!-- a\n> Row verdict: LIB-TOOL-001: FAIL\n> x--->\n\n{PASS}\n",
            vec![look_alike(2)],
            "probe (a): `x--->` on its own line, in a block quote",
        ),
        (
            "- Text <!-- a\n  Row verdict: LIB-TOOL-001: FAIL\n  --->\n{PASS}\n",
            vec![look_alike(2)],
            "probe (a): `--->` on its own line, in a list item",
        ),
        (
            "- Text <!-- a\n  Row verdict: LIB-TOOL-001: FAIL --->\n{PASS}\n",
            vec![look_alike(2)],
            "probe (a): `--->` ending the look-alike's line, in a list item",
        ),
        (
            "- Text <!-- a\n  Row verdict: LIB-TOOL-001: FAIL\n  x--->\n{PASS}\n",
            vec![look_alike(2)],
            "probe (a): `x--->` on its own line, in a list item",
        ),
    ];
    let failures = case_mismatches(&cases);
    assert!(
        failures.is_empty(),
        "LIB-TOOL-005: a look-alike inside a comment whose text ends in `-` is a problem and the \
         record never passes (verification 0007 round 3, probe (a); D14 withdrawn):\n\
         {failures:#?}"
    );
}

#[test]
fn lib_tool_005_a_look_alike_in_a_table_cell_inside_a_closed_inline_comment_is_reported() {
    // Probe (b). Under core `CommonMark` a delimiter row such as `--- | ---`
    // is paragraph continuation text: the `|` makes it neither a setext
    // underline (§4.3) nor a thematic break (§4.1). So the comment spans it
    // and is closed. Under GFM the lines become a table, and the look-alike
    // a cell GitHub shows. Every form here leaves `Row verdict:` at the start
    // of its line once D9's markers are stripped. A header row with a
    // leading `|` does not: see the test report.
    let cases: Vec<Case<'_>> = vec![
        (
            "Text <!-- a\nRow verdict: LIB-TOOL-001: FAIL | x\n--- | ---\n-->\n{PASS}\n",
            vec![look_alike(2)],
            "probe (b): the verifier's table, without outer pipes",
        ),
        (
            "Text <!-- a\nRow verdict: LIB-TOOL-001: FAIL | x |\n| --- | --- |\n-->\n{PASS}\n",
            vec![look_alike(2)],
            "probe (b): a header row with a trailing pipe and a delimiter row with outer pipes",
        ),
        (
            "Text <!-- a\nRow verdict: LIB-TOOL-001: FAIL |\n--- |\n-->\n{PASS}\n",
            vec![look_alike(2)],
            "probe (b): a single-column table",
        ),
        (
            "Text <!-- a\nh | x\n--- | ---\nRow verdict: LIB-TOOL-001: FAIL | x\n-->\n{PASS}\n",
            vec![look_alike(4)],
            "probe (b): the look-alike in the body row",
        ),
        (
            "Text <!-- a\n| h | x |\n| --- | --- |\nRow verdict: LIB-TOOL-001: FAIL | x |\n-->\n{PASS}\n",
            vec![look_alike(4)],
            "probe (b): the look-alike in the body row of a table with outer pipes",
        ),
        (
            "> Text <!-- a\n> Row verdict: LIB-TOOL-001: FAIL | x\n> --- | ---\n> -->\n\n{PASS}\n",
            vec![look_alike(2)],
            "probe (b): the table inside a block quote",
        ),
        (
            "- Text <!-- a\n  Row verdict: LIB-TOOL-001: FAIL | x\n  --- | ---\n  -->\n{PASS}\n",
            vec![look_alike(2)],
            "probe (b): the table inside a list item",
        ),
    ];
    let failures = case_mismatches(&cases);
    assert!(
        failures.is_empty(),
        "LIB-TOOL-005: a look-alike that GFM renders as a table cell inside a closed inline \
         comment is a problem and the record never passes (verification 0007 round 3, probe \
         (b); D14 withdrawn):\n{failures:#?}"
    );
}

#[test]
fn lib_tool_005_a_look_alike_continuing_a_footnote_inside_a_closed_inline_comment_is_reported() {
    // Probe (c), the part D9 can see. GitHub shows a GFM footnote
    // definition's text in the footnotes section, and an indented line after
    // the definition continues it. Under core `CommonMark` a link reference
    // definition cannot interrupt a paragraph (§4.7), so `[^1]: note` and
    // the indented line after it are continuation text inside the comment.
    // The indented line starts with `Row verdict:` once D9 strips its
    // spaces.
    //
    // The verifier's own probe, `[^1]: Row verdict: …`, does not start with
    // `Row verdict:` after D9's markers, so D9 does not report it, with or
    // without D14. It is not asserted here: that class is LIB-TOOL-018's.
    let cases: Vec<Case<'_>> = vec![
        (
            "Text <!-- a\n[^1]: note\n    Row verdict: LIB-TOOL-001: FAIL\n-->\n\n[^1]\n{PASS}\n",
            vec![look_alike(3)],
            "probe (c): a look-alike continuing a footnote definition",
        ),
        (
            "> Text <!-- a\n> [^1]: note\n>     Row verdict: LIB-TOOL-001: FAIL\n> -->\n\n{PASS}\n",
            vec![look_alike(3)],
            "probe (c): the same in a block quote",
        ),
    ];
    let failures = case_mismatches(&cases);
    assert!(
        failures.is_empty(),
        "LIB-TOOL-005: a look-alike that GFM shows in a footnote, inside a closed inline \
         comment, is a problem and the record never passes (verification 0007 round 3, probe \
         (c); D14 withdrawn):\n{failures:#?}"
    );
}

#[test]
fn lib_tool_005_a_look_alike_in_an_image_description_is_reported() {
    // Probe (d). Raw HTML binds more tightly than link brackets (§6.3), so
    // the comment is a closed inline comment inside the image description.
    // A renderer turns the description into alt text, which is shown when
    // the image fails to load and is read by screen readers.
    let cases: Vec<Case<'_>> = vec![
        (
            "Text ![a <!-- a\nRow verdict: LIB-TOOL-001: FAIL\n-->](x.png)\n{PASS}\n",
            vec![look_alike(2)],
            "probe (d): an inline image",
        ),
        (
            "Text ![a <!-- a\nRow verdict: LIB-TOOL-001: FAIL\n-->][img]\n\n[img]: x.png\n{PASS}\n",
            vec![look_alike(2)],
            "probe (d): a full reference image",
        ),
        (
            "> Text ![a <!-- a\n> Row verdict: LIB-TOOL-001: FAIL\n> -->](x.png)\n\n{PASS}\n",
            vec![look_alike(2)],
            "probe (d): an inline image in a block quote",
        ),
    ];
    let failures = case_mismatches(&cases);
    assert!(
        failures.is_empty(),
        "LIB-TOOL-005: a look-alike in an image description is a problem and the record never \
         passes (verification 0007 round 3, probe (d); D14 withdrawn):\n{failures:#?}"
    );
}

#[test]
fn lib_tool_005_a_look_alike_on_a_boundary_line_of_an_inline_comment_is_reported() {
    // A closed inline comment hides nothing (slice 0007 round 3, D14
    // withdrawn), and a boundary line also holds text a reader sees in every
    // renderer. These controls were written when D14 hid the lines wholly
    // inside a comment; they pin the boundary lines whose visible part is a
    // look-alike under D9:
    // - text after the closer: D9 strips `-` and `>`, so `--> Row verdict:`
    //   is `Row verdict:` once stripped, and the reader sees that text;
    // - text before the opener, at the start of the line;
    // - text between the closer of one comment and the opener of the next:
    //   the line's two ends lie in comments, but not in one comment.
    // A line such as `Text <!-- c --> Row verdict: …` is not tested: with or
    // without the comment, the line does not start with `Row verdict:` once
    // stripped, so no rule makes it a look-alike.
    let cases: Vec<Case<'_>> = vec![
        (
            "Text <!-- note\nhidden\n--> Row verdict: LIB-TOOL-001: FAIL\n{PASS}\n",
            vec![look_alike(3)],
            "a look-alike after the closer on its line",
        ),
        (
            "Text <!-- note\nhidden\n-->Row verdict: x\n{PASS}\n",
            vec![look_alike(3)],
            "a malformed look-alike right after the closer",
        ),
        (
            "Text <!-- note\nhidden\n--> {FAIL}\n{PASS}\n",
            vec![look_alike(3)],
            "a whole candidate after the closer",
        ),
        (
            "> Text <!-- note\n> hidden\n> --> Row verdict: LIB-TOOL-001: FAIL\n\n{PASS}\n",
            vec![look_alike(3)],
            "a look-alike after the closer in a block quote",
        ),
        (
            "Row verdict: LIB-TOOL-001: FAIL <!-- note\nhidden\n-->\n{PASS}\n",
            vec![look_alike(1)],
            "a look-alike before the opener on its line",
        ),
        (
            "> Row verdict: x <!-- note\n> hidden\n> -->\n\n{PASS}\n",
            vec![look_alike(1)],
            "a block-quoted look-alike before the opener on its line",
        ),
        (
            "Text\nRow verdict: LIB-TOOL-001: FAIL <!-- c -->\n{PASS}\n",
            vec![look_alike(2)],
            "a look-alike before a one-line comment",
        ),
        (
            "Text <!-- a\n--> Row verdict: LIB-TOOL-001: FAIL <!-- b\n-->\n{PASS}\n",
            vec![look_alike(2)],
            "a look-alike between two comments: its ends lie in different comments",
        ),
    ];
    let failures = case_mismatches(&cases);
    assert!(
        failures.is_empty(),
        "LIB-TOOL-005: a boundary line of an inline comment is not hidden, so a \
         look-alike in its visible text is a problem (D9):\n{failures:#?}"
    );
}

#[test]
fn lib_tool_005_a_look_alike_outside_a_closed_inline_comment_is_reported() {
    // Controls: a look-alike in the paragraph that holds a closed inline
    // comment, or before or after it, is a problem. They were written when
    // D14 hid the comment's own lines; with D14 withdrawn (slice 0007 round
    // 3) the comment hides nothing at all, and these expectations stand.
    let cases: Vec<Case<'_>> = vec![
        (
            "Text <!-- note\nhidden\n-->\nRow verdict: LIB-TOOL-001: FAIL\n{PASS}\n",
            vec![look_alike(4)],
            "a look-alike on its own line after the closer, in the same paragraph",
        ),
        (
            "Text <!-- note\nhidden\n--> more\n   Row verdict: x\n{PASS}\n",
            vec![look_alike(4)],
            "an indented look-alike after the closer's line, in the same paragraph",
        ),
        (
            "Text <!-- note\nhidden\n-->\n\n  {FAIL}\n\n{PASS}\n",
            vec![look_alike(5)],
            "a look-alike in a later block",
        ),
        (
            "Row verdict: LIB-TOOL-001: FAIL\nText <!-- note\nhidden\n-->\n{PASS}\n",
            vec![look_alike(1)],
            "a look-alike before the opener's line, in the same paragraph",
        ),
        (
            "> {FAIL}\n\nText <!-- note\nhidden\n-->\n{PASS}\n",
            vec![look_alike(1)],
            "a look-alike in an earlier block",
        ),
    ];
    let failures = case_mismatches(&cases);
    assert!(
        failures.is_empty(),
        "LIB-TOOL-005: a look-alike outside every closed inline comment is a problem (D9):\n\
         {failures:#?}"
    );
}

#[test]
fn lib_tool_005_a_line_that_ends_the_paragraph_is_never_inside_an_inline_comment() {
    // Controls: a block quote or list item that starts at an indent of 0 to 3
    // interrupts the paragraph (§5.1, §5.2). The `<!--` before it is then not
    // closed in its paragraph, so it is text, and no comment spans the
    // look-alike. A reader sees it.
    let cases: Vec<Case<'_>> = vec![
        (
            "Text <!-- note\n  > Row verdict: LIB-TOOL-001: FAIL\n-->\n{PASS}\n",
            vec![look_alike(2)],
            "`  > Row verdict:` starts a block quote",
        ),
        (
            "Text <!-- note\n* Row verdict: LIB-TOOL-001: FAIL\n-->\n{PASS}\n",
            vec![look_alike(2)],
            "`* Row verdict:` starts a list item",
        ),
        (
            "Text <!-- note\n   + Row verdict: x\n-->\n{PASS}\n",
            vec![look_alike(2)],
            "`   + Row verdict:` starts a list item",
        ),
        (
            "> Text <!-- note\n> > Row verdict: x\n> -->\n\n{PASS}\n",
            vec![look_alike(2)],
            "`> > Row verdict:` starts a block quote nested in the outer one, which ends the \
             outer quote's paragraph (§3.2, §5.1)",
        ),
    ];
    let failures = case_mismatches(&cases);
    assert!(
        failures.is_empty(),
        "LIB-TOOL-005: a look-alike that starts a block quote or list item ends the paragraph, \
         so it lies inside no inline comment and is a problem (D9):\n{failures:#?}"
    );
}

#[test]
fn lib_tool_015_inline_html_other_than_a_closed_comment_hides_nothing() {
    // Controls: no inline HTML hides a line. A closed comment is allowed but
    // hides nothing (slice 0007 round 3, D14 withdrawn; see
    // `lib_tool_015_a_look_alike_inside_a_closed_inline_comment_is_reported`).
    // Other inline HTML over several lines is raw HTML at its first line
    // (D8), as before, and the look-alike inside it is still a problem.
    let cases: Vec<Case<'_>> = vec![
        (
            "Text <span title=\"a\nRow verdict: LIB-TOOL-001: FAIL\n\">\n{PASS}\n",
            vec![html(1), look_alike(2)],
            "an open tag whose attribute value spans lines",
        ),
        (
            "Text <!-- a --!>\nRow verdict: LIB-TOOL-001: FAIL\n-->\n{PASS}\n",
            vec![html(1), look_alike(2)],
            "a comment holding `--!>`, which a browser ends there (not a closed comment, D8)",
        ),
        (
            "Text <?x\nRow verdict: LIB-TOOL-001: FAIL\n?>\n{PASS}\n",
            vec![html(1), look_alike(2)],
            "a processing instruction",
        ),
        (
            "Text <![CDATA[\nRow verdict: LIB-TOOL-001: FAIL\n]]>\n{PASS}\n",
            vec![html(1), look_alike(2)],
            "a CDATA section",
        ),
        (
            "Text <!X\nRow verdict: LIB-TOOL-001: FAIL\nx>\n{PASS}\n",
            vec![html(1), look_alike(2)],
            "a declaration (its `>` is not at the start of a line, where it would start a \
             block quote)",
        ),
        (
            "Text <!-- note\nRow verdict: LIB-TOOL-001: FAIL\n\n{PASS}\n",
            vec![look_alike(2)],
            "an opener not closed in its paragraph is text and hides nothing",
        ),
    ];
    let failures = case_mismatches(&cases);
    assert!(
        failures.is_empty(),
        "LIB-TOOL-015: inline HTML other than a closed comment hides no line (D8):\n\
         {failures:#?}"
    );
}
