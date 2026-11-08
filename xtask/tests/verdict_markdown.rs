//! Acceptance tests for which `- Row verdict:` lines count (slice 0007).
//!
//! - LIB-TOOL-005: a verdict-line candidate is a source line that starts at
//!   column 0 with `- Row verdict:`. It counts only when `CommonMark` 0.31.2
//!   puts it outside every code block, HTML block and inline HTML, as the
//!   parser of ADR 0008 (`pulldown-cmark` with `Options::empty()`) decides.
//!   This closes P-005b of `docs/verification/0004-m0-gate-hardening-round2.md`:
//!   the probes P1, P1b, P6, P8 and P9 (T7), the controls (T8), item 3 and F5.
//! - LIB-TOOL-015: lines inside HTML comments and HTML blocks do not count,
//!   and a backtick fence "opener" whose info string holds a backtick is not
//!   an opener (seeds P2 and P7). Under slice 0007 D6, a `<!--` inside an
//!   HTML event that no `-->` in that event closes also hides everything up
//!   to the end of the next `-->` that lies in an HTML event, or to the end
//!   of the record. A `-->` in text, a code span or a code block is rendered
//!   escaped and ends nothing (D6 as amended after the Copilot review of
//!   PR #14).
//!
//! The gate is driven through `xtask::matrix::check` against an in-memory
//! [`Repo`], as in `gate_hardening.rs`. Every expectation is an exact list of
//! problems. The problem texts are the ones the gate produces today (slice
//! 0007 keeps them), built by the helpers below and pinned once by
//! `lib_tool_005_the_problem_texts_stay_as_they_are`.
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
            out.push(format!("{why}: the record {record:?} gave {p:#?}"));
        }
    }
    out
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
        &[fail()],
    );
    assert!(
        failures.is_empty(),
        "LIB-TOOL-005: a lone CR ends the closer line, so the tab before it is trailing, the \
         fence closes and the FAIL line counts:\n{failures:#?}"
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
/// (an HTML comment), 7 (a fence) and 10 (an HTML block that the blank line
/// 12 ends) are hidden; line 14 is the PASS.
const MIXED: &str = "# Verification\n\n<!-- note\n- Row verdict: a\n-->\n```\n- Row verdict: b\n```\n<div>\n- Row verdict: c\n</div>\n\n- Row verdict: LIB-TOOL-001: pass\n- Row verdict: LIB-TOOL-001: PASS\n";

#[test]
fn lib_tool_005_a_malformed_line_outside_every_hidden_range_is_reported_with_its_line_number() {
    for record in [MIXED.to_owned(), crlf(MIXED)] {
        assert_eq!(
            record_problems(&record),
            [malformed(ROW, 13)],
            "LIB-TOOL-005: only the visible malformed line 13 is reported, by its 1-based line \
             number, and the PASS on line 14 stands; the record is {record:?}"
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
    // may interrupt a paragraph and be indented up to 3 spaces.
    let mut failures = mismatches_lf_and_crlf(
        &[
            (
                "<!--\n{PASS}\n-->\n",
                "a PASS line in a comment spanning lines",
            ),
            ("<!--\n{PASS}\n", "a PASS line in an unclosed comment"),
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
    assert!(
        failures.is_empty(),
        "LIB-TOOL-015: a line inside an HTML comment is neither a verdict line nor malformed:\n\
         {failures:#?}"
    );
}

/// Records whose only PASS line is inside an HTML block (`CommonMark` §4.6),
/// so the row has no verdict line.
const HIDDEN_BY_HTML_BLOCK: &[(&str, &str)] = &[
    (
        "<div>\n{PASS}\n</div>\n",
        "type 6: between <div> and </div>",
    ),
    (
        "<div class=\"note\">\n{PASS}\n",
        "type 6: after an unclosed <div> with attributes",
    ),
    (
        "<DIV>\n{PASS}\n",
        "type 6: tag names are case-insensitive (Example 152)",
    ),
    ("<details>\n{PASS}\n</details>\n", "type 6: after <details>"),
    (
        "<details>\n<summary>Example</summary>\n{PASS}\n</details>\n",
        "type 6: after <details> and <summary>",
    ),
    (
        "</div>\n{PASS}\n",
        "type 6: a closing tag starts a block (Example 151)",
    ),
    (
        "<div>\n{PASS}\n</div>\n{PASS}\n",
        "type 6: </div> does not end the block; only a blank line does (Example 186)",
    ),
    ("   <div>\n{PASS}\n", "type 6: indented 3 spaces"),
    (
        "Text.\n<div>\n{PASS}\n",
        "type 6: interrupting a paragraph (Example 185)",
    ),
    (
        "<custom-tag>\n{PASS}\n</custom-tag>\n",
        "type 7: <custom-tag>",
    ),
    (
        "<custom-tag data-x=\"1\">\n{PASS}\n",
        "type 7: an open tag with an attribute",
    ),
    ("</custom-tag>\n{PASS}\n", "type 7: a closing tag"),
    (
        "<pre>\n\n{PASS}\n</pre>\n",
        "type 1: <pre> runs past a blank line to </pre>",
    ),
    ("<script>\n{PASS}\n</script>\n", "type 1: <script>"),
    (
        "<style>\n\n{PASS}\n</style>\n",
        "type 1: <style> runs past a blank line",
    ),
    ("<textarea>\n{PASS}\n</textarea>\n", "type 1: <textarea>"),
    (
        "<?note\n\n{PASS}\n?>\n",
        "type 3: a processing instruction (Example 180)",
    ),
    ("<!DOCTYPE note\n{PASS}\n>\n", "type 4: a declaration"),
    ("<![CDATA[\n\n{PASS}\n]]>\n", "type 5: a CDATA section"),
];

#[test]
fn lib_tool_015_a_verdict_line_inside_an_html_block_does_not_count() {
    let mut failures = mismatches_lf_and_crlf(HIDDEN_BY_HTML_BLOCK, &[no_verdict()]);
    failures.extend(mismatches_lf_and_crlf(
        &[(
            "<div>\n{FAIL}\n</div>\n\n{PASS}\n",
            "a FAIL line in a <div> block, then a PASS line after the blank line",
        )],
        &[],
    ));
    assert!(
        failures.is_empty(),
        "LIB-TOOL-015: a line inside an HTML block is neither a verdict line nor malformed:\n\
         {failures:#?}"
    );
}

#[test]
fn lib_tool_015_a_malformed_candidate_inside_html_is_not_malformed() {
    let failures = mismatches_lf_and_crlf(
        &[
            ("<!--\n- Row verdict: x\n-->\n{PASS}\n", "inside a comment"),
            (
                "<!--\n- Row verdict:\n{PASS} -->\n{PASS}\n",
                "on a comment's lines and closing line",
            ),
            (
                "<div>\n- Row verdict: LIB-TOOL-001: pass\n</div>\n\n{PASS}\n",
                "inside a <div> block",
            ),
            (
                "<custom-tag>\n- Row verdict: x\n\n{PASS}\n",
                "inside a type 7 block",
            ),
            (
                "<pre>\n- Row verdict: x\n</pre>\n{PASS}\n",
                "inside a <pre> block",
            ),
        ],
        &[],
    );
    assert!(
        failures.is_empty(),
        "LIB-TOOL-015: a candidate that is not well formed, inside HTML, is not reported as \
         malformed, and the PASS line outside passes the row:\n{failures:#?}"
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
    let mut failures = mismatches_lf_and_crlf(
        &[
            (P7, "P7: the FAIL counts and the fenced PASS does not"),
            (
                "<div>\n```\n</div>\n\n{FAIL}\n\n```\n{PASS}\n```\n",
                "P7 with a blank line before the fence",
            ),
        ],
        &[fail()],
    );
    // Without the blank line, `</div>` does not end the block (Example 186):
    // the FAIL line and everything after it are HTML, so there is no verdict
    // line at all. The row still cannot pass.
    failures.extend(mismatches_lf_and_crlf(
        &[(
            P7_NO_BLANK,
            "P7 without a blank line: the whole record after <div> is one HTML block",
        )],
        &[no_verdict()],
    ));
    assert!(
        failures.is_empty(),
        "LIB-TOOL-015: a fence line inside an HTML block is HTML and opens no fence:\n\
         {failures:#?}"
    );
}

// ---------------------------------------------------------------------------
// LIB-TOOL-015: controls that pass the row
// ---------------------------------------------------------------------------

#[test]
fn lib_tool_015_a_pass_line_after_a_closed_html_comment_counts() {
    let failures = mismatches_lf_and_crlf(
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
                "<!-- note --> text\n{PASS}\n",
                "after a closing line with text after `-->`",
            ),
            (
                "Text <!-- note\n{PASS}\n-->\n",
                "a `<!--` in a paragraph: the PASS line interrupts the paragraph, so no \
                 comment spans it (§3.2, §6.6)",
            ),
        ],
        &[],
    );
    assert!(
        failures.is_empty(),
        "LIB-TOOL-015: the comment is closed, so the PASS line after it passes the row:\n\
         {failures:#?}"
    );
}

#[test]
fn lib_tool_015_a_pass_line_after_an_html_block_that_has_ended_counts() {
    let failures = mismatches_lf_and_crlf(
        &[
            (
                "<div>\nx\n</div>\n\n{PASS}\n",
                "after a <div> block that a blank line ends",
            ),
            (
                "<div>\n\n{PASS}\n\n</div>\n",
                "after an open <div> line that a blank line ends",
            ),
            (
                "<p>x</p>\n\n{PASS}\n",
                "after a one-line <p> block and a blank line",
            ),
            (
                "<custom-tag>\n\n{PASS}\n",
                "after a type 7 block that a blank line ends",
            ),
            (
                "<pre>\nx\n</pre>\n{PASS}\n",
                "after a <pre> block that </pre> ends",
            ),
            (
                "<?note ?>\n{PASS}\n",
                "after a processing instruction that `?>` ends",
            ),
            (
                "Text\n<custom-tag>\n{PASS}\n",
                "a type 7 tag cannot interrupt a paragraph, so it is inline (Example 187)",
            ),
            (
                "- a\n\n  <div>\n{PASS}\n",
                "a <div> block in a list item ends with the item",
            ),
            (
                "> <div>\n{PASS}\n",
                "a <div> block in a block quote ends with the quote",
            ),
        ],
        &[],
    );
    assert!(
        failures.is_empty(),
        "LIB-TOOL-015: the HTML block has ended, so the PASS line after it passes the row:\n\
         {failures:#?}"
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
// LIB-TOOL-015, slice 0007 D6: an unclosed comment in HTML hides what follows
// ---------------------------------------------------------------------------
//
// CommonMark passes raw HTML through. A `<!--` that it does not close inside
// its own HTML range opens a comment in the rendered page, which hides
// everything up to the next `-->`. D6: a `<!--` inside an `Html` or
// `InlineHtml` event range that no `-->` closes within that range opens a
// hidden region. The region runs from the `<!--` to the end of the next `-->`
// that itself lies inside an `Html` or `InlineHtml` range, or to the end of
// the record. A `-->` in text, a code span or a code block is rendered
// escaped (`--&gt;`), so it closes nothing in the browser and ends no region.
// A `<!--` outside every HTML event (in a code span, a code block or plain
// text) opens nothing.
//
// In each probe below, CommonMark alone puts the PASS line outside every HTML
// range:
// - `<!-- a --> <!--`: the type 2 block is line 1 only, because line 1 holds
//   `-->` (§4.6). The second `<!--` on it is unclosed.
// - `<div><!--`: a type 6 block, which the blank line ends.
// - a `<!--` block in a list item or block quote: it ends with its container,
//   and the PASS line is not lazy (§5.1, §5.2).
// A `-->` line right after a list item is lazy paragraph text (§5.2); after a
// blank line it is a paragraph of its own. Either way it is text, so the
// trailing `-->` in these probes ends no region. The tests below that need a
// region to end close it with raw HTML: a one-line comment block
// `<!-- end -->` (type 2, which may interrupt a paragraph, §4.6) or a type 6
// line holding `-->`.

/// The four probes: the PASS line is inside an unclosed comment's region.
const UNCLOSED_COMMENT_PROBES: &[(&str, &str)] = &[
    (
        "<!-- a --> <!--\n{PASS}\n-->\n",
        "a second, unclosed comment after a closed one on a type 2 line",
    ),
    (
        "<div><!--\n\n{PASS}\n\n-->\n",
        "an unclosed comment in a <div> block that a blank line ends",
    ),
    (
        "- a\n\n  <!--\n{PASS}\n-->\n",
        "an unclosed comment block in a list item",
    ),
    (
        "> <!--\n{PASS}\n-->\n",
        "an unclosed comment block in a block quote",
    ),
];

#[test]
fn lib_tool_015_an_unclosed_comment_in_an_html_event_hides_the_pass_line_after_it() {
    let failures = mismatches_lf_and_crlf(UNCLOSED_COMMENT_PROBES, &[no_verdict()]);
    assert!(
        failures.is_empty(),
        "LIB-TOOL-015: the PASS line is inside the region of an unclosed `<!--` (D6), so the \
         row has no verdict line:\n{failures:#?}"
    );
}

#[test]
fn lib_tool_015_an_unclosed_comment_hides_the_pass_line_but_not_a_fail_after_its_closer() {
    let failures = mismatches_lf_and_crlf(
        &[
            (
                "<!-- a --> <!--\n{PASS}\n<!-- end -->\n{FAIL}\n",
                "after a type 2 line with a second, unclosed comment",
            ),
            (
                "<div><!--\n\n{PASS}\n\n</div> -->\n\n{FAIL}\n",
                "after a <div> block holding an unclosed comment",
            ),
            (
                "- a\n\n  <!--\n{PASS}\n<!-- end -->\n{FAIL}\n",
                "after an unclosed comment block in a list item",
            ),
            (
                "> <!--\n{PASS}\n<!-- end -->\n{FAIL}\n",
                "after an unclosed comment block in a block quote",
            ),
        ],
        &[fail()],
    );
    assert!(
        failures.is_empty(),
        "LIB-TOOL-015: the region hides the PASS line and ends at the `-->` in raw HTML, so \
         the visible FAIL line after it is the row's only verdict (D6):\n{failures:#?}"
    );
}

#[test]
fn lib_tool_015_an_unclosed_comment_region_ends_at_the_end_of_the_next_closer() {
    // The FAIL line is in the region; the PASS line after the `-->` is not.
    // A region that ended with its HTML event would leave the FAIL visible,
    // and the row would have two verdict lines.
    // A region ending at the start of `-->` is equivalent: no byte of `-->`
    // can be the first byte of a candidate, which starts `- R` (triaged).
    let failures = mismatches_lf_and_crlf(
        &[
            (
                "<!-- a --> <!--\n{FAIL}\n<!-- end -->\n{PASS}\n",
                "a PASS line right after the `-->` line",
            ),
            (
                "- a\n\n  <!--\n{FAIL}\n<!-- end -->\n{PASS}\n",
                "a PASS line after the `-->` that follows a list item's comment",
            ),
            (
                "> <!--\n{FAIL}\n<!-- end -->\n\n{PASS}\n",
                "a PASS line after the `-->` and a blank line",
            ),
            (
                "<div><!--\n\n{FAIL}\n<p>x --> y\n\n{PASS}\n",
                "a `-->` in the middle of a type 6 line; the PASS after the block counts",
            ),
            (
                "<!-- a --> <!--\n{FAIL}\n<!-- end -->\n{PASS}\n<div><!--\n\n{FAIL}\n<!-- end -->\n",
                "two regions, each ending at its own `-->`",
            ),
        ],
        &[],
    );
    assert!(
        failures.is_empty(),
        "LIB-TOOL-015: an unclosed comment's region ends at the end of the next `-->`, so the \
         PASS line after it passes the row (D6):\n{failures:#?}"
    );
}

#[test]
fn lib_tool_015_an_unclosed_comment_with_no_closer_anywhere_hides_to_the_end_of_the_record() {
    let failures = mismatches_lf_and_crlf(
        &[
            (
                "<!-- a --> <!--\n{PASS}\n\n{PASS}\n",
                "after a type 2 line with an unclosed comment",
            ),
            (
                "> <!--\n{PASS}\n",
                "after an unclosed comment in a block quote",
            ),
            (
                "<div><!--\n\n{PASS}\n\nText.\n\n{FAIL}\n",
                "after a <div> block holding an unclosed comment, past paragraphs",
            ),
            (
                "- a\n\n  <!--\n{PASS}\n",
                "after an unclosed comment in a list item",
            ),
        ],
        &[no_verdict()],
    );
    assert!(
        failures.is_empty(),
        "LIB-TOOL-015: with no `-->` after it, an unclosed comment's region runs to the end of \
         the record (D6):\n{failures:#?}"
    );
}

#[test]
fn lib_tool_015_an_unclosed_comment_in_inline_html_opens_a_hidden_region() {
    // §6.6: `<span title="<!--">` is one open tag, so it is an `InlineHtml`
    // event, and the `<!--` in it has no `-->` in that event. D6 covers
    // `InlineHtml` too, so the region runs to the end of the record. A browser
    // would not start a comment inside an attribute value: this is the
    // fail-safe side of D6, and it is pinned because it is the only way an
    // unclosed `<!--` reaches an `InlineHtml` event.
    let failures = mismatches_lf_and_crlf(
        &[(
            "Text <span title=\"<!--\"> x\n{PASS}\n",
            "a `<!--` inside an inline tag's attribute value",
        )],
        &[no_verdict()],
    );
    assert!(
        failures.is_empty(),
        "LIB-TOOL-015: an unclosed `<!--` in an inline HTML event opens a hidden region (D6):\n\
         {failures:#?}"
    );
}

#[test]
fn lib_tool_015_a_closed_comment_in_an_html_block_hides_nothing_past_its_closer() {
    // Each record also holds a stray `-->` after the PASS line. It is text,
    // so it ends no region (D6): a region opened by mistake would hide the
    // PASS whether or not it stopped there.
    let failures = mismatches_lf_and_crlf(
        &[
            (
                "<div><!-- a -->\n\n{PASS}\n\n-->\n",
                "a closed comment in a <div> block",
            ),
            (
                "<div><!-- a --> <!-- b -->\n\n{PASS}\n\n-->\n",
                "two closed comments in a <div> block",
            ),
            ("<!-- a -->\n{PASS}\n-->\n", "a one-line comment block"),
            (
                "<!--\nnote\n-->\n{PASS}\n-->\n",
                "a comment block spanning lines ends at its own `-->`",
            ),
        ],
        &[],
    );
    assert!(
        failures.is_empty(),
        "LIB-TOOL-015: a comment closed inside its HTML range opens no region past its `-->` \
         (D6):\n{failures:#?}"
    );
}

#[test]
fn lib_tool_015_a_comment_opener_outside_every_html_event_opens_no_region() {
    // D6 looks for `<!--` only inside HTML events. A code span (§6.1), a code
    // block (§4.4, §4.5), a backslash escape (§2.4) and paragraph text that is
    // not a complete comment (§6.6) are not HTML events.
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
        "LIB-TOOL-015: a `<!--` outside every HTML event hides nothing (D6):\n{failures:#?}"
    );
}

#[test]
fn lib_tool_015_a_pass_line_inside_details_after_a_blank_line_counts() {
    // §4.6: a blank line ends the <details> block, so the PASS line is a list
    // item inside the rendered <details> element. A reader can expand it, so
    // it counts (D6 hides only comments).
    let failures = mismatches_lf_and_crlf(
        &[
            (
                "<details>\n<summary>Example</summary>\n\n{PASS}\n\n</details>\n",
                "a PASS line inside <details> with a <summary>",
            ),
            (
                "<details>\n\n{PASS}\n\n</details>\n",
                "a PASS line inside <details>",
            ),
        ],
        &[],
    );
    assert!(
        failures.is_empty(),
        "LIB-TOOL-015: a PASS line inside <details> after a blank line passes the row:\n\
         {failures:#?}"
    );
}

// D6's closing rule: the search for the `-->` that closes a `<!--` starts at
// the opener's third byte, so `<!-->` and `<!--->` close themselves, as
// CommonMark 0.31.2 §6.6 (Example 626) and browsers treat them. A line that
// starts with `<!--` is a type 2 HTML block, and since the line holds `-->`
// the block is that line only (§4.6). None of these records holds another
// `-->`, so a comment D6 left open would hide everything after it.

#[test]
fn lib_tool_015_a_short_comment_is_closed_by_a_closer_that_overlaps_its_opener() {
    let failures = mismatches_lf_and_crlf(
        &[
            ("<!-->\n{PASS}\n", "`<!-->` as a one-line type 2 block"),
            ("<!--->\n{PASS}\n", "`<!--->` as a one-line type 2 block"),
            (
                "Text <!--> x\n{PASS}\n",
                "`<!-->` as inline HTML (Example 626)",
            ),
            (
                "Text <!---> x\n{PASS}\n",
                "`<!--->` as inline HTML (Example 626)",
            ),
            ("<div><!-->\n\n{PASS}\n", "`<!-->` in a <div> block"),
            ("<div><!--->\n\n{PASS}\n", "`<!--->` in a <div> block"),
        ],
        &[],
    );
    assert!(
        failures.is_empty(),
        "LIB-TOOL-015: `<!-->` and `<!--->` are complete comments, so they open no region and \
         the PASS line after them passes the row (D6):\n{failures:#?}"
    );
}

#[test]
fn lib_tool_015_an_empty_comment_of_four_dashes_is_closed() {
    let failures = mismatches_lf_and_crlf(
        &[
            ("<!---->\n{PASS}\n", "`<!---->` as a one-line type 2 block"),
            ("Text <!----> x\n{PASS}\n", "`<!---->` as inline HTML"),
            ("<div><!---->\n\n{PASS}\n", "`<!---->` in a <div> block"),
        ],
        &[],
    );
    assert!(
        failures.is_empty(),
        "LIB-TOOL-015: `<!---->` is a closed comment, so the PASS line after it passes the row \
         (D6):\n{failures:#?}"
    );
}

#[test]
fn lib_tool_015_a_comment_opener_followed_by_a_lone_dash_hides_to_the_end_of_the_record() {
    // `<!-- -` holds no `-->`: the opener's own `--` does not close it.
    let failures = mismatches_lf_and_crlf(
        &[
            (
                "<!-- -\n{PASS}\n",
                "a type 2 block with no `-->` runs to the end (§4.6)",
            ),
            (
                "<div><!-- -\n\n{PASS}\n",
                "in a <div> block that a blank line ends: the D6 region runs on",
            ),
            (
                "> <!-- -\n{PASS}\n",
                "in a block quote that the PASS line ends: the D6 region runs on",
            ),
        ],
        &[no_verdict()],
    );
    assert!(
        failures.is_empty(),
        "LIB-TOOL-015: `<!-- -` is not closed, so it hides the rest of the record and the row \
         has no verdict line (D6):\n{failures:#?}"
    );
}

// ---------------------------------------------------------------------------
// LIB-TOOL-015, slice 0007 D6 as amended: only a `-->` in raw HTML ends a region
// ---------------------------------------------------------------------------
//
// The Copilot review of PR #14 (`matrix.rs:1453`): CommonMark escapes a `-->`
// in text, in a code span (§6.1) and in a code block (§4.4, §4.5) as
// `--&gt;`, so the browser's comment stays open past it. Only a `-->` that
// lies inside an `Html` or `InlineHtml` range reaches the page as `-->`. In
// every record below, the unclosed `<!--` is one of the D6 probes: the second
// comment on a type 2 line, a `<div>` block that a blank line ends, or a
// comment block in a block quote or list item. A `-->` at column 0 after a
// list item is lazy paragraph text (§5.2), and a type 2 block in a container
// ends with the container, so a `-->` line after it is not in that block.

#[test]
fn lib_tool_015_a_closer_in_a_code_span_does_not_end_an_unclosed_comment_region() {
    let failures = mismatches_lf_and_crlf(
        &[
            (
                "<div><!--\n\n`-->`\n\n{PASS}\n",
                "the Copilot record: a code span `-->` in its own paragraph",
            ),
            (
                "<div><!--\n\nThe closer `-->` is text.\n\n{PASS}\n",
                "a code span `-->` in the middle of a paragraph",
            ),
            (
                "<!-- a --> <!--\n`` --> ``\n{PASS}\n",
                "a double-backtick code span on the line after a type 2 line",
            ),
            (
                "> <!--\n{FAIL}\n`-->`\n{PASS}\n",
                "a code span `-->` in a lazy line after a block quote's comment",
            ),
            (
                "- a\n\n  <!--\n{FAIL}\n`-->`\n{PASS}\n",
                "a code span `-->` in a lazy line after a list item's comment",
            ),
        ],
        &[no_verdict()],
    );
    assert!(
        failures.is_empty(),
        "LIB-TOOL-015: a `-->` in a code span is rendered `--&gt;` and ends no region, so the \
         PASS line after it is hidden and the row has no verdict line (D6):\n{failures:#?}"
    );
}

#[test]
fn lib_tool_015_a_fail_line_after_a_raw_html_closer_that_follows_a_code_span_closer_counts() {
    // The region runs past the code span `-->` and the PASS line to the `-->`
    // in raw HTML. The FAIL line after that is the row's only verdict line.
    let failures = mismatches_lf_and_crlf(
        &[
            (
                "<div><!--\n\n`-->`\n\n{PASS}\n\n<!-- end -->\n{FAIL}\n",
                "the Copilot record, then a comment block and a FAIL line",
            ),
            (
                "> <!--\n`-->`\n{PASS}\n<pre>--></pre>\n{FAIL}\n",
                "a code span `-->`, a PASS line, a type 1 line holding `-->`, a FAIL line",
            ),
        ],
        &[fail()],
    );
    assert!(
        failures.is_empty(),
        "LIB-TOOL-015: the region ends only at the `-->` in raw HTML, so the PASS line before \
         it is hidden and the FAIL line after it is the row's only verdict (D6):\n{failures:#?}"
    );
}

#[test]
fn lib_tool_015_a_closer_in_a_fenced_code_block_does_not_end_an_unclosed_comment_region() {
    let failures = mismatches_lf_and_crlf(
        &[
            (
                "<div><!--\n\n```\n-->\n```\n{PASS}\n",
                "a `-->` line in a backtick fence",
            ),
            (
                "<div><!--\n\n~~~\n-->\n~~~\n{PASS}\n",
                "a `-->` line in a tilde fence",
            ),
            (
                "<!-- a --> <!--\n```text\nx --> y\n```\n{PASS}\n",
                "a `-->` in the middle of a line in a backtick fence with an info string",
            ),
            (
                "> <!--\n~~~\n-->\n~~~\n{PASS}\n",
                "a tilde fence after a block quote's comment",
            ),
        ],
        &[no_verdict()],
    );
    assert!(
        failures.is_empty(),
        "LIB-TOOL-015: a `-->` in a fenced code block is rendered `--&gt;` and ends no region, \
         so the PASS line after the fence is hidden (D6):\n{failures:#?}"
    );
}

#[test]
fn lib_tool_015_a_closer_in_an_indented_code_block_does_not_end_an_unclosed_comment_region() {
    // §4.4: a column-0 line ends the indented code block, so the PASS line is
    // outside it; only the region can hide it.
    let failures = mismatches_lf_and_crlf(
        &[
            (
                "<div><!--\n\n    -->\n\n{PASS}\n",
                "a `-->` indented 4 spaces after a blank line",
            ),
            (
                "<!-- a --> <!--\n    -->\n{PASS}\n",
                "a `-->` indented 4 spaces right after a type 2 line",
            ),
            (
                "<div><!--\n\n\t-->\n\n{PASS}\n",
                "a `-->` indented by a tab",
            ),
        ],
        &[no_verdict()],
    );
    assert!(
        failures.is_empty(),
        "LIB-TOOL-015: a `-->` in an indented code block is rendered `--&gt;` and ends no \
         region, so the PASS line after the block is hidden (D6):\n{failures:#?}"
    );
}

#[test]
fn lib_tool_015_a_closer_in_paragraph_text_does_not_end_an_unclosed_comment_region() {
    let failures = mismatches_lf_and_crlf(
        &[
            (
                "<div><!--\n\n-->\n\n{PASS}\n",
                "a `-->` line that is a paragraph of its own",
            ),
            (
                "<div><!--\n\nText --> here.\n\n{PASS}\n",
                "a `-->` in the middle of a paragraph",
            ),
            (
                "<!-- a --> <!--\n{FAIL}\n-->\n{PASS}\n",
                "a `-->` line that is lazy text of a list item",
            ),
            (
                "> <!--\n{FAIL}\n-->\n\n{PASS}\n",
                "a lazy `-->` line, then a blank line",
            ),
            ("<div><!--\n\n*-->*\n\n{PASS}\n", "a `-->` in emphasis"),
            (
                "<div><!--\n\nSee <http://x/-->.\n\n{PASS}\n",
                "an autolink (§6.5) ending `--`, rendered with no `-->`",
            ),
        ],
        &[no_verdict()],
    );
    assert!(
        failures.is_empty(),
        "LIB-TOOL-015: a `-->` in paragraph text is rendered `--&gt;` and ends no region, so \
         the PASS line after it is hidden (D6):\n{failures:#?}"
    );
}

#[test]
fn lib_tool_015_a_closer_in_a_later_html_block_ends_an_unclosed_comment_region() {
    // None of these records holds inline HTML. Each closing `-->` lies in an
    // HTML block that holds no `<!--` of its own. Types 1 to 5 may interrupt
    // the FAIL line's paragraph and end on their own line (§4.6), so the PASS
    // line right after counts. Types 6 and 7 end only at a blank line. In a
    // type 6 block, backticks are raw HTML, not a code span.
    let failures = mismatches_lf_and_crlf(
        &[
            (
                "<div><!--\n\n{FAIL}\n<pre>--></pre>\n{PASS}\n",
                "type 1: a `-->` inside <pre> on one line",
            ),
            (
                "<!-- a --> <!--\n{FAIL}\n<?x -->?>\n{PASS}\n",
                "type 3: a `-->` inside a processing instruction",
            ),
            (
                "> <!--\n{FAIL}\n<!X -->\n{PASS}\n",
                "type 4: a `-->` inside a declaration",
            ),
            (
                "<div><!--\n\n{FAIL}\n<![CDATA[-->]]>\n{PASS}\n",
                "type 5: a `-->` inside a CDATA section",
            ),
            (
                "<div><!--\n\n{FAIL}\n<p>`-->`\n\n{PASS}\n",
                "type 6: a backticked `-->` in a <p> line, then a blank line",
            ),
            (
                "- a\n\n  <!--\n{FAIL}\n\n<x a=-->\n\n{PASS}\n",
                "type 7: an open tag whose unquoted attribute value ends `--`",
            ),
        ],
        &[],
    );
    assert!(
        failures.is_empty(),
        "LIB-TOOL-015: a `-->` inside a later HTML block ends the region, so the PASS line \
         after it passes the row (D6):\n{failures:#?}"
    );
}

#[test]
fn lib_tool_015_a_closer_in_later_inline_html_ends_an_unclosed_comment_region() {
    // None of these records holds an HTML block after the unclosed `<!--`.
    // Each closing `-->` lies in an inline HTML event (§6.6), and the PASS
    // line on the next line interrupts that paragraph (§5.2), so it counts.
    let failures = mismatches_lf_and_crlf(
        &[
            (
                "<div><!--\n\n{FAIL}\n\nText <span title=\"-->\"> x\n{PASS}\n",
                "a `-->` in a quoted attribute value",
            ),
            (
                "<!-- a --> <!--\n{FAIL}\n\nText <x a=--> y\n{PASS}\n",
                "a `-->` where an unquoted attribute value meets the tag's `>`",
            ),
            (
                "> <!--\n{FAIL}\n\nText <!-- end --> y\n{PASS}\n",
                "a closed inline comment",
            ),
            (
                "<div><!--\n\n{FAIL}\n\nText <?x --> ?> y\n{PASS}\n",
                "a `-->` inside an inline processing instruction",
            ),
            (
                "- a\n\n  <!--\n{FAIL}\nand <span title=\"-->\"> x\n{PASS}\n",
                "inline HTML in a lazy line of a list item",
            ),
        ],
        &[],
    );
    assert!(
        failures.is_empty(),
        "LIB-TOOL-015: a `-->` inside later inline HTML ends the region, so the PASS line \
         after it passes the row (D6):\n{failures:#?}"
    );
}
