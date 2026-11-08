//! Differential test of which `- Row verdict:` lines count (slice 0007 D12).
//!
//! LIB-TOOL-005, strict-records criterion: "A differential test against an
//! independent `CommonMark` implementation (markdown-rs) over at least 20,000
//! generated records shows no unadjudicated disagreement in which candidates
//! count."
//!
//! - **Generator.** A seeded, deterministic generator (xorshift64*, no `rand`
//!   crate) builds [`RECORDS`] records from the D12 grammar: fences with
//!   indents, tabs, trailing blanks and info strings; bullet, ordered and
//!   empty list items and block quotes; HTML of types 1 to 7 and comments, in
//!   any case; blank, paragraph, setext, thematic and link reference definition
//!   lines; column-0 PASS, FAIL, OPEN, other-row and malformed candidates, and
//!   verdict look-alikes; LF, CRLF and lone CR endings, and a leading BOM.
//!   Half of the records are *plain*: they hold no HTML but closed comments and
//!   the allowed inline tags, no look-alike, no lone CR and no BOM, so the
//!   strict gate is expected to accept most of them.
//! - **Reference.** markdown-rs (`markdown` 1.0, default constructs, which
//!   are `CommonMark`, with raw HTML passed through) renders a *twin* of each
//!   record, in which candidate `i` reads `- CANDnnnnn`, to HTML. A column-0
//!   candidate is visible when its marker is rendered as list-item text: a
//!   candidate inside a code block or an HTML node is rendered verbatim, its
//!   `- ` included. This is "not inside a code node and not inside an html
//!   node", read from the renderer's output. The HTML compiler is used, not
//!   mdast, because `to_mdast` in markdown-rs 1.0.0 panics on
//!   `"1. ```\n- b\n"`, the shape of probe P1b and of the real record
//!   `0001-m0-tooling-verification-round4.md`. From the visible candidates,
//!   whose kind the generator knows, the test computes the problems a single
//!   verified row gets: one malformed problem per visible malformed
//!   candidate, in line order, then the verdict problem (none, FAIL or OPEN, a
//!   count, or no verdict). Line numbers are LF/CRLF lines, as the gate counts
//!   them (D3, D10).
//! - **Adjudications.** Where markdown-rs departs from `CommonMark` 0.31.2,
//!   the reference reads a spec-neutral form of the twin (CRLF made LF,
//!   §2.1), or the record class is named in [`ADJUDICATIONS`] with the spec
//!   section, and counted rather than compared.
//! - **Assertion.** A record the gate refuses as not strict (it reports a D8,
//!   D9 or D10 problem) is counted and skipped. For every other record, the
//!   gate's problems equal the expected ones. Every disagreement is either
//!   fixed in the gate or matches a named adjudication in [`ADJUDICATIONS`].
//!   The test also requires that enough strict records were compared, and
//!   that they cover every verdict outcome, so it cannot pass by refusing
//!   everything.
//!
//! This test target sorts after `function_scan`, and none of its tests scan
//! the real repository (see `docs/quality/mutants-triage.md`, slice 0005).
//! Every verdict line in a generated record names [`ROW`] or [`OTHER`],
//! both defined by the fixture's matrix.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use markdown::{Options, to_html_with_options};
use xtask::matrix::{
    Matrix, RENDERED_PATH, Repo, check, ignored_traced_tests, load, render, traced_tests,
};

// ---------------------------------------------------------------------------
// Harness: the gate for one verified row whose record is generated
// ---------------------------------------------------------------------------

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

/// The verified row every record is checked for.
const ROW: &str = "LIB-TOOL-001";
/// A second row: its verdict lines are well formed but do not count for [`ROW`].
const OTHER: &str = "LIB-TOOL-002";
/// The verification record [`ROW`] cites.
const RECORD: &str = "docs/verification/v.md";
/// The test [`ROW`] cites.
const TEST_FILE: &str = "crates/x/tests/t.rs";

/// The number of generated records (D12: at least 20,000). The environment
/// variable `VERDICT_DIFFERENTIAL_RECORDS` may raise it for a longer run.
const RECORDS: usize = 20_000;
/// The generator's seed. Changing it changes every record. The environment
/// variable `VERDICT_DIFFERENTIAL_SEED` (decimal) may replace it.
const SEED: u64 = 0x0007_D12D_1FF5_EED5;

/// `name` from the environment as a number, or `default`.
fn from_env<T: std::str::FromStr>(name: &str, default: T) -> T {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// An in-memory repository holding one record.
#[derive(Debug)]
struct OneRecord<'a> {
    files: &'a BTreeMap<String, String>,
    record: &'a str,
}

impl Repo for OneRecord<'_> {
    fn exists(&self, rel: &str) -> bool {
        rel == RECORD || self.files.contains_key(rel)
    }

    fn read(&self, rel: &str) -> Option<String> {
        if rel == RECORD {
            Some(self.record.to_owned())
        } else {
            self.files.get(rel).cloned()
        }
    }

    fn traced_functions(&self) -> Vec<(String, String)> {
        let text = self.files.get(TEST_FILE).map_or("", String::as_str);
        traced_tests(text, false)
            .into_iter()
            .map(|name| (TEST_FILE.to_owned(), name))
            .collect()
    }

    fn ignored_functions(&self) -> Vec<(String, String)> {
        let text = self.files.get(TEST_FILE).map_or("", String::as_str);
        ignored_traced_tests(text, false)
            .into_iter()
            .map(|name| (TEST_FILE.to_owned(), name))
            .collect()
    }
}

/// The matrix with [`ROW`] verified, and the other files of its repository,
/// built once.
struct Gate {
    matrix: Matrix,
    files: BTreeMap<String, String>,
}

impl Gate {
    fn new() -> Self {
        let row = format!(
            "\n[[row]]\nid = \"{ROW}\"\narea = \"TOOL\"\nmilestone = \"M0\"\ntitle = \"Title\"\n\
             requirement = \"Requirement.\"\nsource = \"Source\"\nkind = \"tooling\"\n\
             status = \"verified\"\nacceptance = [\"a\"]\ncode = [\"src/lib.rs\"]\n\
             tests = [\"{TEST_FILE}::lib_tool_001_x\"]\nverification = \"{RECORD}\"\n"
        );
        // [`OTHER`] is defined, so its verdict lines name a row of the matrix;
        // it is planned, so the gate reads no record for it.
        let other = format!(
            "\n[[row]]\nid = \"{OTHER}\"\narea = \"TOOL\"\nmilestone = \"M0\"\ntitle = \"Other\"\n\
             requirement = \"Requirement.\"\nsource = \"Source\"\nkind = \"tooling\"\n\
             status = \"planned\"\n"
        );
        let matrix =
            load(&format!("{HEADER}{row}{other}")).expect("LIB-TOOL-005: test matrix parses");
        let mut files = BTreeMap::new();
        files.insert("src/lib.rs".to_owned(), String::new());
        files.insert(
            TEST_FILE.to_owned(),
            "#[test]\nfn lib_tool_001_x() {}\n".to_owned(),
        );
        files.insert(RENDERED_PATH.to_owned(), render(&matrix));
        Self { matrix, files }
    }

    /// The problems the gate reports when [`ROW`]'s record reads `record`.
    fn problems(&self, record: &str) -> Vec<String> {
        check(
            &self.matrix,
            &OneRecord {
                files: &self.files,
                record,
            },
        )
    }
}

/// Whether `problem` is one of the strict-record problems of D8, D9 and D10,
/// whose texts are fixed in the slice 0007 brief.
fn is_strict_problem(problem: &str) -> bool {
    let Some(rest) = problem.strip_prefix(&format!("{ROW}: verification record {RECORD}")) else {
        return false;
    };
    if rest == ": starts with a byte-order mark" {
        return true;
    }
    let Some(rest) = rest.strip_prefix(':') else {
        return false;
    };
    let digits = rest.len() - rest.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    digits > 0
        && matches!(
            &rest[digits..],
            ": raw HTML other than a closed comment or <br>, <ul>, <li> is not allowed"
                | ": a verdict-like line is not at column 0"
                | ": lone carriage return; use LF or CRLF line endings"
        )
}

// ---------------------------------------------------------------------------
// The expected problems, from markdown-rs
// ---------------------------------------------------------------------------

/// What a generated column-0 candidate says, known by construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Pass,
    Fail,
    Open,
    /// A well-formed verdict line for another row, or for a similar id.
    Other,
    Malformed,
}

/// The markdown-rs options: `CommonMark` (the default constructs), with raw
/// HTML passed through as a renderer shows it.
fn reference_options() -> Options {
    let mut options = Options::default();
    options.compile.allow_dangerous_html = true;
    options.compile.allow_dangerous_protocol = true;
    options
}

/// The marker that stands for candidate `i` in the twin record.
fn marker(i: usize) -> String {
    format!("CAND{i:05}")
}

/// Whether markdown-rs shows candidate `i` of the twin record as text: its
/// marker is rendered as list-item text, so its `- ` was read as a list
/// marker. A candidate inside a code block or raw HTML is rendered verbatim,
/// so its marker follows `- `.
fn shown(html: &str, i: usize) -> bool {
    let marker = marker(i);
    let at = html.find(&marker).unwrap_or_else(|| {
        panic!(
            "LIB-TOOL-005: markdown-rs renders every candidate marker, but not {marker} in {html:?}"
        )
    });
    !html[..at].ends_with("- ")
}

/// The problems [`ROW`] gets from `record` when exactly the column-0
/// candidates that markdown-rs shows as text count. `twin` is `record` with
/// candidate `i` (which starts at byte `candidates[i].0` of `record`) replaced
/// by `- ` and [`marker`]`(i)`; the replacement keeps every block boundary,
/// since a `- ` line with plain text after it is a list item whatever the text.
///
/// `None` when markdown-rs panics on the twin: markdown-rs 1.0.0 has a
/// defect in its HTML compiler, which on some documents panics at
/// `to_html.rs:197` with "at least one buffer should exist" (its buffer stack
/// is popped empty). Such a record has no reference view, so the test counts
/// it as an adjudicated exclusion and bounds how many there may be.
fn expected_problems(
    record: &str,
    twin: &str,
    candidates: &[(usize, Kind)],
) -> Option<Vec<String>> {
    // markdown-rs 1.0 misreads a list item that starts with a blank line when
    // its lines end in CRLF: `"*\r\n  ```\r\n- PASS"` puts the fence outside
    // the item and hides the PASS, where the LF form is right. `CommonMark`
    // 0.31.2 §2.1 makes CRLF and LF the same line ending, so the reference
    // reads the twin with each CRLF made LF. A lone CR stays.
    let twin = twin.replace("\r\n", "\n");
    let options = reference_options();
    // `Options` holds MDX callbacks that are not `RefUnwindSafe`; the
    // reference keeps no state across calls, so nothing is observed after a
    // panic.
    let html = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        to_html_with_options(&twin, &options)
    }))
    .ok()?
    .expect("LIB-TOOL-005: markdown-rs renders every CommonMark document");
    let mut out = Vec::new();
    let mut verdicts = Vec::new();
    for (i, &(at, kind)) in candidates.iter().enumerate() {
        if !shown(&html, i) {
            continue;
        }
        match kind {
            Kind::Pass => verdicts.push("PASS"),
            Kind::Fail => verdicts.push("FAIL"),
            Kind::Open => verdicts.push("OPEN"),
            Kind::Other => {}
            Kind::Malformed => {
                // `at` is the start of a line, so a char boundary.
                let line = record.get(..at).unwrap_or_default().matches('\n').count() + 1;
                out.push(format!(
                    "{ROW}: verification record {RECORD}:{line}: malformed verdict line; \
                     expected `- Row verdict: <ROW-ID>: <PASS|FAIL|OPEN>`"
                ));
            }
        }
    }
    match verdicts.as_slice() {
        ["PASS"] => {}
        [] => out.push(format!(
            "{ROW}: verification record {RECORD} has no `- Row verdict: {ROW}: PASS` line"
        )),
        [verdict] => out.push(format!(
            "{ROW}: verification record {RECORD} gives the verdict {verdict}, not PASS"
        )),
        many => out.push(format!(
            "{ROW}: verification record {RECORD} has {} verdict lines for this row; expected \
             exactly one",
            many.len()
        )),
    }
    Some(out)
}

/// Installs a panic hook that keeps quiet about panics inside markdown-rs,
/// which [`expected_problems`] catches and counts, and reports every other
/// panic (a panic in the gate is a failure) as the previous hook did.
fn quiet_reference_panics() {
    let previous = std::sync::Arc::new(std::panic::take_hook());
    std::panic::set_hook(Box::new(move |info| {
        let in_reference = info
            .location()
            .is_some_and(|l| l.file().replace('\\', "/").contains("/markdown-1."));
        if !in_reference {
            previous(info);
        }
    }));
}

/// The first record on which the reference panicked, shown once on stderr
/// (visible with `--nocapture`) and kept in the tally.
#[expect(
    clippy::print_stderr,
    reason = "the first reference panic is reported once, as the D12 brief asks"
)]
fn report_first_reference_panic(record: &str) {
    eprintln!("LIB-TOOL-005: markdown-rs 1.0 panicked on the reference twin of {record:?}");
}

/// The tally of a run, shown on stderr (visible with `--nocapture`) so that
/// a run on another seed reports its numbers even when it passes.
#[expect(
    clippy::print_stderr,
    reason = "the D12 brief asks for the refused and adjudicated counts of each run"
)]
fn report_tally(seed: u64, tally: &Tally) {
    eprintln!("LIB-TOOL-005: differential run, seed {seed}: {tally:?}");
}

// ---------------------------------------------------------------------------
// Adjudications: where markdown-rs and CommonMark 0.31.2 differ
// ---------------------------------------------------------------------------

/// A record class in which markdown-rs 1.0 departs from `CommonMark` 0.31.2, so
/// that its view is not the reference. Each names the spec section.
struct Adjudication {
    name: &'static str,
    /// Whether `record` is in the class.
    applies: fn(&str) -> bool,
}

/// Every adjudicated class. Records in one are counted, not compared.
const ADJUDICATIONS: &[Adjudication] = &[
    Adjudication {
        name: "markdown-rs 1.0 lets an indented code block, an empty list item, or a type 7 \
           tag inside a code span, interrupt a paragraph after a container; CommonMark 0.31.2 \
           \u{a7}4.4 (Example 113), \u{a7}5.3, \u{a7}4.6 (Example 187) and \u{a7}6.1",
        applies: interrupts_the_paragraph_after_a_container,
    },
    Adjudication {
        name: "markdown-rs 1.0 continues a list item or block quote lazily after a setext heading \
           in it; CommonMark 0.31.2 \u{a7}5.1 and \u{a7}5.2: only paragraph continuation text \
           is lazy, and a heading is not a paragraph",
        applies: continues_lazily_after_a_setext_heading,
    },
];

/// The record class of the second adjudication. markdown-rs 1.0 renders
/// `"- a\n  ===\nb"`, `"- a\n  -\nb"` and `"> a\n> ===\nb"` with `b` inside
/// the item or quote, after the heading. In `CommonMark` 0.31.2 the heading
/// has closed the paragraph, so `b` is not paragraph continuation text and
/// not a lazy line: it closes the container (§5.1, §5.2). The container a
/// later fence opens in then differs, and so do the candidates it hides
/// (verification seeds 1 and 7: `"- Row verdict: …\n\t-\n``` x ```\n   ~~~\n…"`,
/// where `` ``` x `` is no fence, §4.5). The class: a setext underline (a run
/// of `=` or `-`, a lone `-` included) in a container, then a line that is
/// not in it and opens no container, fence or heading. It over-approximates.
fn continues_lazily_after_a_setext_heading(record: &str) -> bool {
    let lines: Vec<&str> = record
        .split('\n')
        .map(|l| l.strip_suffix('\r').unwrap_or(l))
        .collect();
    lines.windows(2).any(|pair| {
        let (underline, next) = (pair[0], pair[1]);
        // Only indent and `>` are removed: a lone `-` is the underline here.
        let content = underline
            .trim_start_matches([' ', '\t', '>'])
            .trim_end_matches([' ', '\t']);
        let in_container =
            underline.trim_start().starts_with('>') || indent_columns(underline) >= 1;
        let run = !content.is_empty()
            && (content.chars().all(|c| c == '=') || content.chars().all(|c| c == '-'));
        let lazy = !is_blank(next)
            && indent_columns(next) < 2
            && !next.trim_start().starts_with(['>', '#'])
            && !starts_container(next)
            && !is_fence_opener(next);
        in_container && run && lazy
    })
}

/// Whether `line` opens a code fence (§4.5): 0 to 3 spaces, then 3 or more
/// backticks and an info string with no backtick, or 3 or more tildes.
fn is_fence_opener(line: &str) -> bool {
    let rest = line.trim_start_matches(' ');
    if indent_columns(line) >= 4 {
        return false;
    }
    let ticks = rest.len() - rest.trim_start_matches('`').len();
    let tildes = rest.len() - rest.trim_start_matches('~').len();
    (ticks >= 3 && !rest[ticks..].contains('`')) || tildes >= 3
}

/// Whether `line` is an empty list item: 0 to 3 spaces, a bullet or an
/// ordered marker, and nothing but blanks after it.
fn is_empty_list_item(line: &str) -> bool {
    let rest = line.trim_start_matches(' ');
    indent_columns(line) < 4
        && !rest.starts_with('>')
        && starts_container(rest)
        && container_content(rest).trim_matches([' ', '\t']).is_empty()
}

/// The columns of indent of `line`, with tab stops of 4 (§2.2).
fn indent_columns(line: &str) -> usize {
    let mut column = 0;
    for c in line.chars() {
        match c {
            ' ' => column += 1,
            '\t' => column += 4 - column % 4,
            _ => break,
        }
    }
    column
}

/// Whether `line` holds only spaces and tabs.
fn is_blank(line: &str) -> bool {
    line.trim_matches([' ', '\t']).is_empty()
}

/// Whether `line`, after its indent, starts a block quote or a list item
/// (`>`, `-`, `*`, `+`, or 1 to 9 digits and `.` or `)`, then a blank or the
/// end of the line).
fn starts_container(line: &str) -> bool {
    let rest = line.trim_start_matches([' ', '\t']);
    if rest.starts_with('>') {
        return true;
    }
    let digits = rest.len() - rest.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    let marker = match rest.as_bytes().first() {
        Some(b'-' | b'*' | b'+') => 1,
        Some(b'0'..=b'9')
            if digits <= 9 && matches!(rest.as_bytes().get(digits), Some(b'.' | b')')) =>
        {
            digits + 1
        }
        _ => return false,
    };
    rest[marker..].is_empty() || rest[marker..].starts_with([' ', '\t'])
}

/// Whether `line` is paragraph text at indent 0 to 3: it starts with a
/// letter, `|`, or a backtick that is not a fence run.
fn is_paragraph_text(line: &str) -> bool {
    let rest = line.trim_start_matches(' ');
    indent_columns(line) < 4
        && !starts_container(line)
        && !rest.starts_with("```")
        && (rest.starts_with(|c: char| c.is_ascii_alphabetic()) || rest.starts_with(['|', '`']))
}

/// `line` with its indent, block-quote markers and list markers removed.
fn container_content(line: &str) -> &str {
    let mut rest = line;
    loop {
        let trimmed = rest.trim_start_matches([' ', '\t']);
        if let Some(after) = trimmed.strip_prefix('>') {
            rest = after;
        } else if starts_container(trimmed) {
            let digits = trimmed.len()
                - trimmed
                    .trim_start_matches(|c: char| c.is_ascii_digit())
                    .len();
            rest = &trimmed[digits.max(1)..];
            if digits > 0 {
                rest = &rest[1..];
            }
        } else {
            return trimmed;
        }
    }
}

/// Whether `line` starts, after 0 to 3 spaces, with `<` and a letter or `/`:
/// the start of an open or closing tag, as a type 7 HTML block would.
fn starts_tag(line: &str) -> bool {
    let tag = line.trim_start_matches(' ');
    indent_columns(line) < 4
        && tag.starts_with('<')
        && tag[1..].starts_with(|c: char| c.is_ascii_alphabetic() || c == '/')
}

/// The record class of the first adjudication. markdown-rs 1.0 lets a line
/// interrupt a paragraph that `CommonMark` 0.31.2 continues. In
/// `"- a\n\nText.\n    code"` and `"- ```\nText.\n    code"`, the line
/// `Text.` closes the container and opens a paragraph; markdown-rs makes the
/// next line, indented 4 columns, an indented code block, which cannot
/// interrupt a paragraph (§4.4, Example 113). It does the same with an empty
/// list item such as `*`, which cannot interrupt a paragraph either (§5.3),
/// so a fence after it opens in that item instead of at the top level
/// (verification seed 31337). From the paragraph's second line on,
/// markdown-rs is right.
///
/// markdown-rs does the same with a type 7 tag on that line, and applies no
/// laziness against one (`"- a\n<br>\n- PASS"` hides the PASS, where
/// `CommonMark` keeps `<br>` inline, §4.6 Example 187). Under the lead's
/// decision (a) of fix round 1 an allowed tag at the start of a line is raw
/// HTML, so every such record is refused and that part of the class needs
/// no exclusion.
///
/// One case of the tag shape is not refused: the tag inside a code span
/// (§6.1). In `"> - a\n``` x\n<br>\n```\n- PASS"` (verification seeds 42
/// and 7) the lines after the item's first line are lazy, and the code span
/// that `` ``` `` opens runs past `<br>`, so `CommonMark` and pulldown-cmark
/// read `<br>` as code-span text, no HTML at all. markdown-rs starts a type 7
/// HTML block at the lazy `<br>` line and hides the PASS. The class includes
/// it: after a container, a line starting with a tag, in a paragraph that
/// already holds a backtick.
///
/// The test below over-approximates both shapes. It cannot hide a false PASS
/// of the gate: an indented code block hides no column-0 candidate; a tag
/// that the gate reads as raw HTML is refused under D8; and a tag in a code
/// span hides nothing.
fn interrupts_the_paragraph_after_a_container(record: &str) -> bool {
    let lines: Vec<&str> = record
        .split('\n')
        .map(|l| l.strip_suffix('\r').unwrap_or(l))
        .collect();
    // Whether the current line continues a paragraph that a container line
    // opened.
    let mut container_paragraph = false;
    // Whether the last non-blank line was in a container.
    let mut in_container = false;
    // Whether a container line came before.
    let mut container_seen = false;
    // Whether the paragraph so far (since the last blank line) holds a
    // backtick, which may open a code span.
    let mut backtick = false;
    for i in 0..lines.len() {
        let line = lines[i];
        let next = lines.get(i + 1).copied().unwrap_or("");
        let opens = starts_container(line);
        container_seen |= opens;
        backtick = !is_blank(line) && (backtick || line.contains('`'));
        // The tag shape: a tag line that may sit in a code span.
        if container_seen && backtick && starts_tag(next) {
            return true;
        }
        let previous = i.checked_sub(1).map(|p| lines[p]);
        let after_container = previous.is_some_and(|p| {
            (is_blank(p) && in_container)
                || (starts_container(p) && !is_paragraph_text(container_content(p)))
        });
        container_paragraph = if opens {
            is_paragraph_text(container_content(line))
        } else {
            container_paragraph && is_paragraph_text(line)
        };
        // The second line of the paragraph a closing container leaves,
        // indented 4 columns or an empty list item (§5.3: an empty item
        // cannot interrupt a paragraph; verification seed 31337).
        if !opens
            && !container_paragraph
            && after_container
            && is_paragraph_text(line)
            && !is_blank(next)
            && (indent_columns(next) >= 4 || is_empty_list_item(next))
        {
            return true;
        }
        if !is_blank(line) {
            in_container = opens || indent_columns(line) >= 2 || container_paragraph;
        }
    }
    false
}

// ---------------------------------------------------------------------------
// The generator
// ---------------------------------------------------------------------------

/// xorshift64*: small, seeded and the same on every platform.
#[derive(Debug)]
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed | 1)
    }

    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// A number in `0..n`, for `n > 0`.
    fn below(&mut self, n: usize) -> usize {
        let n64 = u64::try_from(n).unwrap_or(u64::MAX);
        usize::try_from((self.next() >> 11) % n64).unwrap_or(0)
    }

    /// True `percent` times in 100.
    fn chance(&mut self, percent: usize) -> bool {
        self.below(100) < percent
    }

    fn pick<'a>(&mut self, items: &[&'a str]) -> &'a str {
        items[self.below(items.len())]
    }

    /// `text` with each ASCII letter's case flipped at random, for HTML.
    fn any_case(&mut self, text: &str) -> String {
        text.chars()
            .map(|c| {
                if c.is_ascii_alphabetic() && self.chance(30) {
                    if c.is_ascii_lowercase() {
                        c.to_ascii_uppercase()
                    } else {
                        c.to_ascii_lowercase()
                    }
                } else {
                    c
                }
            })
            .collect()
    }
}

/// Column-0 candidates, by kind.
const CANDIDATES: &[(&str, Kind)] = &[
    ("- Row verdict: LIB-TOOL-001: PASS", Kind::Pass),
    ("- Row verdict: LIB-TOOL-001: PASS", Kind::Pass),
    ("- Row verdict: LIB-TOOL-001: PASS", Kind::Pass),
    ("- Row verdict: LIB-TOOL-001: FAIL", Kind::Fail),
    ("- Row verdict: LIB-TOOL-001: FAIL", Kind::Fail),
    ("- Row verdict: LIB-TOOL-001: OPEN", Kind::Open),
    ("- Row verdict: LIB-TOOL-002: PASS", Kind::Other),
    ("- Row verdict: LIB-TOOL-002: OPEN", Kind::Other),
    ("- Row verdict: x", Kind::Malformed),
    ("- Row verdict: LIB-TOOL-001: pass", Kind::Malformed),
    ("- Row verdict:", Kind::Malformed),
    ("- Row verdict:LIB-TOOL-001: PASS", Kind::Malformed),
    ("- Row verdict: LIB-TOOL-001:  PASS", Kind::Malformed),
    ("- Row verdict: LIB-TOOL-001: PASS.", Kind::Malformed),
    // Ids that are not well-formed row ids (decision 6 of fix round 1).
    ("- Row verdict:  LIB-TOOL-001: PASS", Kind::Malformed),
    ("- Row verdict: lib-tool-001: PASS", Kind::Malformed),
    ("- Row verdict: LIB-TOOL-0011: PASS", Kind::Malformed),
];

/// Trailing blanks; the gate removes spaces and tabs before matching.
const TRAILING: &[&str] = &["", "", "", "", " ", "\t", " \t", "\t\t", "  "];

/// Indents for non-candidate lines.
const INDENTS: &[&str] = &[
    "", "", "", "", "", " ", "  ", "   ", "    ", "     ", "      ", "\t", "  \t", " \t",
];

/// Fence lines (§4.5).
const FENCES: &[&str] = &[
    "```",
    "```",
    "~~~",
    "~~~",
    "````",
    "~~~~",
    "`````",
    "``` info",
    "```rust",
    "```a`b",
    "~~~ a`b",
    "``` x ```",
    "~~~ ``` ~~~",
    "```a~b",
    "``",
    "~~",
];

/// Trailing text after a fence line, including U+00A0 (not a blank).
const FENCE_TRAILING: &[&str] = &[
    "", "", "", "", " ", "\t", " \t", "\t ", "\t\t", "\u{a0}", " x",
];

/// Container starts (§5.1 to §5.3).
const CONTAINERS: &[&str] = &[
    "- a", "1. a", "* b", "+ c", "10) x", "2. b", "-", "1.", "*", "- ```", "1. ```", "- ~~~",
    "- - x", "> x", ">", "> ```", "> - a", "> > ```", "> > x", ">\t```", "- a\t", "1) a",
];

/// Leaf lines: blank, paragraph, setext, thematic, heading, link reference
/// definitions, code spans.
const LEAVES: &[&str] = &[
    "",
    "",
    "",
    "",
    " ",
    "\t",
    "x",
    "text",
    "Text.",
    "***",
    "---",
    "===",
    "- - -",
    "# h",
    "## h",
    "[a]: /u",
    "[a]: /u \"t\"",
    "| a |",
    "| a | b |",
    "Text `code` here",
    "`-->`",
    "`<!--`",
    "x --> y",
];

/// Raw HTML a strict record may hold: closed comments and the allowed inline
/// tags, as real records use them in table rows.
const ALLOWED_HTML: &[&str] = &[
    "<!-- c -->",
    "<!-- c -->  ",
    "<!-->",
    "<!--->",
    "<!---->",
    "Text <!-- c --> t",
    "a<br>b",
    "| a<br>b |",
    "| A <br/> b |",
    "| a<br />b |",
    "| <ul><li>x</li></ul> |",
    "x <br> y <br> z",
    // At the start of a line an allowed tag is raw HTML (decision (a) of fix
    // round 1), so these refuse the record; they keep the class in which
    // markdown-rs reads a lazy `<br>` as an HTML block exercised.
    "<br>",
    "<BR/>",
];

/// Raw HTML a strict record may not hold: block types 1 to 7, unclosed or
/// trailed comments, `--!>`, and inline tags outside the allow-list.
const OTHER_HTML: &[&str] = &[
    "<pre>",
    "</pre>",
    "<pre>x</pre>",
    "<pre>x</script>",
    "<script>",
    "</script>",
    "<style>",
    "</style>",
    "<textarea>",
    "</textarea>",
    "<!--",
    "-->",
    "<!-- c --> x",
    "<!-- a --> <!--",
    "<!-- a --!> b -->",
    "--!>",
    "<?x",
    "?>",
    "<?x ?>",
    "<!X",
    ">",
    "<!DOCTYPE x>",
    "<![CDATA[",
    "]]>",
    "<div>",
    "</div>",
    "<details>",
    "<summary>s</summary>",
    "<p>x</p>",
    "<ul>",
    "<li>",
    "<title>",
    "<custom-tag>",
    "</custom-tag>",
    "<x a=1>",
    "<a href=\"x\">",
    "<br>",
    "Text <span>",
    "Text <span title=\"<!--\"> x",
    "t <b>x</b> t",
];

/// Lines that look like verdict lines but do not start at column 0.
const LOOK_ALIKE_PREFIXES: &[&str] = &[
    " ", "  ", "   ", "\t", "    ", "> ", ">", "> > ", "  - ", "\t- ",
];
const LOOK_ALIKE_MARKERS: &[&str] = &[
    "* Row verdict: LIB-TOOL-001: FAIL",
    "+ Row verdict: LIB-TOOL-001: PASS",
    "1. Row verdict: LIB-TOOL-001: FAIL",
    "1) Row verdict: LIB-TOOL-001: PASS",
    "Row verdict: LIB-TOOL-001: FAIL",
    "> * Row verdict: x",
];

/// A generated line. A column-0 candidate also has its kind and the trailing
/// blanks after its text, which its twin keeps.
#[derive(Debug)]
struct Line {
    text: String,
    candidate: Option<(Kind, &'static str)>,
}

impl Line {
    fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            candidate: None,
        }
    }
}

/// A generated record: its lines, and whether it was generated as plain.
#[derive(Debug)]
struct Generated {
    lines: Vec<Line>,
    plain: bool,
}

impl Rng {
    fn candidate(&mut self) -> Line {
        let (text, kind) = CANDIDATES[self.below(CANDIDATES.len())];
        let trailing = self.pick(TRAILING);
        Line {
            text: format!("{text}{trailing}"),
            candidate: Some((kind, trailing)),
        }
    }

    fn line(&mut self, plain: bool, out: &mut Vec<Line>) {
        let roll = self.below(100);
        let indent = self.pick(INDENTS);
        match roll {
            0..=21 => {
                let line = self.candidate();
                out.push(line);
            }
            22..=39 => {
                let fence = self.pick(FENCES);
                let trailing = self.pick(FENCE_TRAILING);
                out.push(Line::plain(format!("{indent}{fence}{trailing}")));
            }
            40..=51 => {
                let container = self.pick(CONTAINERS);
                out.push(Line::plain(format!("{indent}{container}")));
            }
            52..=59 if plain || self.chance(40) => {
                // A closed comment block, often around a candidate.
                out.push(Line::plain("<!--"));
                if self.chance(60) {
                    let line = self.candidate();
                    out.push(line);
                } else {
                    let leaf = self.pick(LEAVES);
                    out.push(Line::plain(leaf));
                }
                if self.chance(30) {
                    out.push(Line::plain(""));
                }
                let closer = self.pick(&["-->", "x -->", "-->  "]);
                out.push(Line::plain(closer));
            }
            52..=59 => {
                let html = self.pick(OTHER_HTML);
                let html = self.any_case(html);
                out.push(Line::plain(format!("{indent}{html}")));
            }
            60..=67 => {
                let html = self.pick(ALLOWED_HTML);
                let html = self.any_case(html);
                out.push(Line::plain(format!("{indent}{html}")));
            }
            68..=72 if !plain => {
                let text = if self.chance(50) {
                    let prefix = self.pick(LOOK_ALIKE_PREFIXES);
                    let line = self.candidate();
                    format!("{prefix}{}", line.text)
                } else {
                    self.pick(LOOK_ALIKE_MARKERS).to_owned()
                };
                out.push(Line::plain(text));
            }
            _ => {
                let leaf = self.pick(LEAVES);
                let trailing = self.pick(TRAILING);
                let indent = if self.chance(70) { "" } else { indent };
                out.push(Line::plain(format!("{indent}{leaf}{trailing}")));
            }
        }
    }

    fn record(&mut self) -> Generated {
        let plain = self.chance(50);
        let mut lines = Vec::new();
        let n = 1 + self.below(10);
        while lines.len() < n {
            self.line(plain, &mut lines);
        }
        Generated { lines, plain }
    }
}

/// A record ready to check.
#[derive(Debug)]
struct Assembled {
    /// The record text.
    record: String,
    /// The record with each candidate's text replaced by `- ` and its
    /// [`marker`], for the reference.
    twin: String,
    /// The byte offset in `record` and the kind of each column-0 candidate.
    candidates: Vec<(usize, Kind)>,
}

/// The record text of `generated`. Plain records end lines with LF or CRLF;
/// the others may also use a lone CR and start with a BOM.
fn assemble(rng: &mut Rng, generated: &Generated) -> Assembled {
    let style = rng.below(10);
    let bom = !generated.plain && rng.chance(4);
    let mut record = String::new();
    let mut twin = String::new();
    if bom {
        record.push('\u{feff}');
        twin.push('\u{feff}');
    }
    let mut candidates = Vec::new();
    for (i, line) in generated.lines.iter().enumerate() {
        record.push_str(&line.text);
        match line.candidate {
            Some((kind, trailing)) => {
                candidates.push((record.len() - line.text.len(), kind));
                let _ = write!(twin, "- {}{trailing}", marker(candidates.len() - 1));
            }
            None => twin.push_str(&line.text),
        }
        let last = i + 1 == generated.lines.len();
        let ending = match style {
            0..=5 => "\n",
            6..=7 => "\r\n",
            _ if generated.plain => "\r\n",
            _ => rng.pick(&["\n", "\r\n", "\r", "\r"]),
        };
        if !(last && rng.chance(5)) {
            record.push_str(ending);
            twin.push_str(ending);
        }
    }
    Assembled {
        record,
        twin,
        candidates,
    }
}

// ---------------------------------------------------------------------------
// The test
// ---------------------------------------------------------------------------

/// The tallies of one run.
#[derive(Debug, Default)]
struct Tally {
    records: usize,
    plain: usize,
    refused: usize,
    refused_plain: usize,
    compared: usize,
    adjudicated: BTreeMap<&'static str, usize>,
    /// The compared records by expected outcome.
    outcomes: BTreeMap<&'static str, usize>,
    disagreements: usize,
    disagreements_plain: usize,
    /// Disagreements on records with a lone CR or a leading BOM (D10).
    disagreements_cr_or_bom: usize,
    /// Records on which markdown-rs panicked (see [`expected_problems`]).
    reference_panics: usize,
    /// The first of them.
    first_reference_panic: Option<String>,
}

/// Whether `record` holds a lone CR or starts with a BOM, for the tally.
fn has_lone_cr_or_bom(record: &str) -> bool {
    record.starts_with('\u{feff}') || record.replace("\r\n", "\n").contains('\r')
}

/// The name of the verdict outcome in `expected`, for the coverage tally.
fn outcome(expected: &[String]) -> &'static str {
    match expected.last() {
        None => "pass",
        Some(p) if p.ends_with("PASS` line") => "no verdict",
        Some(p) if p.contains("gives the verdict FAIL") => "FAIL",
        Some(p) if p.contains("gives the verdict OPEN") => "OPEN",
        Some(p) if p.contains("verdict lines for this row") => "count",
        Some(_) => "pass with a malformed line",
    }
}

#[test]
fn lib_tool_005_the_gate_agrees_with_markdown_rs_on_every_strict_generated_record() {
    let gate = Gate::new();
    let records = from_env("VERDICT_DIFFERENTIAL_RECORDS", RECORDS).max(RECORDS);
    let seed = from_env("VERDICT_DIFFERENTIAL_SEED", SEED);
    let mut rng = Rng::new(seed);
    let mut tally = Tally::default();
    // The first disagreements on records without a lone CR or BOM, then on
    // records with one.
    let mut shown = String::new();
    let mut shown_cr = String::new();
    quiet_reference_panics();
    for _ in 0..records {
        let generated = rng.record();
        let Assembled {
            record,
            twin,
            candidates,
        } = assemble(&mut rng, &generated);
        tally.records += 1;
        tally.plain += usize::from(generated.plain);

        let got = gate.problems(&record);
        if got.iter().any(|p| is_strict_problem(p)) {
            tally.refused += 1;
            tally.refused_plain += usize::from(generated.plain);
            continue;
        }
        if let Some(a) = ADJUDICATIONS.iter().find(|a| (a.applies)(&record)) {
            *tally.adjudicated.entry(a.name).or_default() += 1;
            continue;
        }
        let Some(expected) = expected_problems(&record, &twin, &candidates) else {
            tally.reference_panics += 1;
            if tally.first_reference_panic.is_none() {
                report_first_reference_panic(&record);
                tally.first_reference_panic = Some(record);
            }
            continue;
        };
        tally.compared += 1;
        *tally.outcomes.entry(outcome(&expected)).or_default() += 1;
        if got != expected {
            tally.disagreements += 1;
            tally.disagreements_plain += usize::from(generated.plain);
            let cr = has_lone_cr_or_bom(&record);
            tally.disagreements_cr_or_bom += usize::from(cr);
            let (out, n) = if cr {
                (&mut shown_cr, tally.disagreements_cr_or_bom)
            } else {
                (
                    &mut shown,
                    tally.disagreements - tally.disagreements_cr_or_bom,
                )
            };
            if n <= 8 {
                let _ = write!(
                    out,
                    "\n  record {record:?} ({})\n    gate:      {got:?}\n    reference: \
                     {expected:?}",
                    if generated.plain { "plain" } else { "mixed" }
                );
            }
        }
    }
    // Back to the default hook, so that a failing assertion below is shown.
    drop(std::panic::take_hook());
    report_tally(seed, &tally);

    assert!(
        tally.reference_panics * 200 <= tally.records,
        "LIB-TOOL-005: markdown-rs panics on at most 0.5% of the records, so the reference \
         still judges the rest; the first was {:?}: {tally:?}",
        tally.first_reference_panic
    );
    assert!(
        tally.records >= 20_000,
        "LIB-TOOL-005: the differential test checks at least 20,000 records (D12): {tally:?}"
    );
    assert_eq!(
        tally.disagreements, 0,
        "LIB-TOOL-005: for every record the gate accepts as strict, its problems equal those \
         computed from markdown-rs's view of which column-0 candidates are visible (D12). \
         Tally: {tally:?}. The first disagreements:{shown}\nThe first with a lone CR or a \
         BOM:{shown_cr}"
    );
    assert!(
        tally.compared * 4 >= tally.records,
        "LIB-TOOL-005: at least a quarter of the records are accepted as strict and compared, so \
         the test cannot pass by refusing them: {tally:?}"
    );
    for kind in [
        "pass",
        "no verdict",
        "FAIL",
        "OPEN",
        "count",
        "pass with a malformed line",
    ] {
        assert!(
            tally.outcomes.get(kind).copied().unwrap_or(0) >= 50,
            "LIB-TOOL-005: the compared records cover the outcome {kind:?} at least 50 times: \
             {tally:?}"
        );
    }
}
