//! Checks of the documentation style guide (LIB-DOC-009) and of the matrix
//! link in the entry documents (LIB-DOC-006).
//!
//! - **Short prose.** No prose paragraph and no list item in the entry
//!   documents (the crate root, the three READMEs, docs/architecture.md and
//!   docs/style.md) runs over four display lines of 100 characters. A list
//!   item counts its marker line and its continuation lines, not its nested
//!   items. Tables, code, headings, block quotes, link reference definitions,
//!   HTML and badges are exempt.
//! - **No diagram in rustdoc.** No Mermaid fence and no `aquamarine` in the
//!   library sources: rustdoc does not render them.
//! - **The skill links the guide.** The `rust-library-craft` skill links
//!   docs/style.md, which exists.
//! - **The matrix link.** The crate root and the three READMEs link the
//!   functionality matrix for current status.
//! - **Item docs.** The `///` docs of public items keep the four-line rule
//!   and a summary of at most two lines: `model`, `compare`, `diagnostic`
//!   and `validation` (LIB-DOC-012), `edit` (LIB-DOC-014), `xmi`
//!   (LIB-DOC-015).
//! - **Examples.** Every public type and fallible public function written
//!   out in the library has a runnable example, and clippy pedantic
//!   enforces `# Errors` and `# Panics` (LIB-DOC-002).
//! - **The guide and the primer.** `crates/sacm/GUIDE.md` (LIB-DOC-003) and
//!   `PRIMER.md` (LIB-DOC-005) keep the four-line rule, run as doctests,
//!   ship with the crate and are linked from the crate root; the guide or
//!   the crate README names each example (LIB-DOC-004).
//!
//! The paragraph classifier is a function over a document's lines; its rules
//! are pinned by synthetic snippets. Nothing on disk is changed.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::Block::{ListItem, Paragraph};

fn repository_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repository root")
}

/// The text of `rel`, or `None` if it does not exist.
fn read_optional(rel: &str) -> Option<String> {
    match fs::read_to_string(repository_root().join(rel)) {
        Ok(text) => Some(text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => panic!("read {rel}: {e}"),
    }
}

fn read(rel: &str) -> String {
    read_optional(rel).unwrap_or_else(|| panic!("read {rel}: not found"))
}

/// The crate root, whose `//!` block is the crate's front page.
const CRATE_ROOT: &str = "crates/sacm/src/lib.rs";

/// The style guide.
const STYLE_GUIDE: &str = "docs/style.md";

/// The skill that summarises the style guide.
const SKILL: &str = ".claude/skills/rust-library-craft/SKILL.md";

/// The Markdown entry documents the paragraph rule applies to, besides the
/// crate root.
const ENTRY_MARKDOWN: [&str; 5] = [
    "README.md",
    "crates/sacm/README.md",
    "crates/sacm-cli/README.md",
    "docs/architecture.md",
    STYLE_GUIDE,
];

/// The READMEs that must link the functionality matrix.
const READMES: [&str; 3] = [
    "README.md",
    "crates/sacm/README.md",
    "crates/sacm-cli/README.md",
];

/// The longest prose paragraph or list item allowed, in display lines.
const MAX_LINES: usize = 4;

/// The characters of one display line.
const DISPLAY_WIDTH: usize = 100;

/// The source directories whose `//!` blocks are module docs (LIB-DOC-011).
const MODULE_SOURCES: [&str; 2] = ["crates/sacm/src", "crates/sacm-cli/src"];

/// The docs/sacm references the paragraph rule applies to whole (LIB-DOC-013).
const SACM_REFERENCES: [&str; 3] = [
    "docs/sacm/diagnostics.md",
    "docs/sacm/compliance-points.md",
    "docs/sacm/decisions.md",
];

/// The generated metamodel inventory; only its introduction is checked.
const INVENTORY: &str = "docs/sacm/metamodel-inventory.md";

/// The functionality matrix, from the repository root.
const MATRIX: &str = "docs/matrix/functionality-matrix.md";

/// The repository on GitHub; a link to a file is `<this>/blob/<ref>/<path>`.
const GITHUB: &str = "https://github.com/lasrod/libSACM/blob/";

// ---------------------------------------------------------------------------
// Reading the documents
// ---------------------------------------------------------------------------

/// The `//!` lines of a Rust source, each with its 1-based line number and
/// its text without the `//!` prefix and the one space after it.
fn inner_doc_lines(source: &str) -> Vec<(usize, String)> {
    source
        .lines()
        .enumerate()
        .filter_map(|(i, line)| {
            let rest = line.trim_start().strip_prefix("//!")?;
            let text = rest.strip_prefix(' ').unwrap_or(rest);
            Some((i + 1, text.to_owned()))
        })
        .collect()
}

/// The lines of a Markdown document, each with its 1-based line number.
fn markdown_lines(text: &str) -> Vec<(usize, String)> {
    text.lines()
        .enumerate()
        .map(|(i, line)| (i + 1, line.to_owned()))
        .collect()
}

/// The text of numbered lines, joined as one Markdown document.
fn joined(lines: &[(usize, String)]) -> String {
    lines
        .iter()
        .map(|(_, line)| line.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

// ---------------------------------------------------------------------------
// The paragraph and list-item classifier
// ---------------------------------------------------------------------------

/// What a line of a document is, for the paragraph and list-item rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LineKind {
    /// Empty or only whitespace.
    Blank,
    /// A line of a prose paragraph.
    Prose,
    /// A text line of the list item with this number, counted in document
    /// order: its marker line or one of its continuation lines, not a line of
    /// a nested item.
    Item(usize),
    /// Exempt, in a list item or not: code, a heading, a table row, a block
    /// quote, a link reference definition, HTML or a badge.
    Other,
}

/// A block of text the rule measures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Block {
    /// A prose paragraph.
    Paragraph,
    /// A list item, without its nested items.
    ListItem,
}

/// A block over the limit: the line number of its first line, its length in
/// display lines and its kind.
type Long = (usize, usize, Block);

/// The display lines of a line: its characters (Unicode scalar values) after
/// its indentation, per [`DISPLAY_WIDTH`], rounded up, and at least one.
fn display_lines(line: &str) -> usize {
    line.trim_start()
        .chars()
        .count()
        .div_ceil(DISPLAY_WIDTH)
        .max(1)
}

/// The indentation of `line` in columns, a tab counting four.
fn indent(line: &str) -> usize {
    line.chars()
        .take_while(|c| *c == ' ' || *c == '\t')
        .map(|c| if c == '\t' { 4 } else { 1 })
        .sum()
}

/// The fence `trimmed` opens or closes, as its character and length: a run of
/// three or more backticks or tildes.
fn fence(trimmed: &str) -> Option<(char, usize)> {
    let first = trimmed.chars().next()?;
    if first != '`' && first != '~' {
        return None;
    }
    let length = trimmed.chars().take_while(|c| *c == first).count();
    (length >= 3).then_some((first, length))
}

/// Whether `trimmed` closes the fence `open`: the same character, at least as
/// long, and nothing after it.
fn closes(trimmed: &str, open: (char, usize)) -> bool {
    fence(trimmed).is_some_and(|(c, n)| {
        c == open.0 && n >= open.1 && trimmed.trim_start_matches(c).trim().is_empty()
    })
}

/// Whether `trimmed` starts a list item: `- `, `* `, `+ `, or digits followed
/// by `. ` or `) `.
fn is_list_item(trimmed: &str) -> bool {
    if ["- ", "* ", "+ "].iter().any(|m| trimmed.starts_with(m)) {
        return true;
    }
    let digits = trimmed.chars().take_while(char::is_ascii_digit).count();
    digits > 0 && {
        let rest = &trimmed[digits..];
        rest.starts_with(". ") || rest.starts_with(") ")
    }
}

/// Whether `trimmed` is an ATX heading: one to six `#`, then a space or the end.
fn is_heading(trimmed: &str) -> bool {
    let hashes = trimmed.chars().take_while(|c| *c == '#').count();
    (1..=6).contains(&hashes) && trimmed[hashes..].chars().next().is_none_or(|c| c == ' ')
}

/// Whether `trimmed` is a link reference definition: `[label]: destination`.
fn is_link_reference_definition(trimmed: &str) -> bool {
    trimmed.starts_with('[')
        && trimmed
            .find("]:")
            .is_some_and(|end| end > 1 && !trimmed[1..end].contains(']'))
}

/// Whether `trimmed` is a line of HTML: a tag or a comment, not an autolink.
fn is_html(trimmed: &str) -> bool {
    let Some(rest) = trimmed.strip_prefix('<') else {
        return false;
    };
    match rest.chars().next() {
        Some('/' | '!' | '?') => true,
        Some(c) if c.is_ascii_alphabetic() => {
            let name = rest
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '-')
                .count();
            !rest[name..].starts_with(':')
        }
        _ => false,
    }
}

/// Whether `trimmed` is a badge line: an image, or a linked image.
fn is_badge(trimmed: &str) -> bool {
    trimmed.starts_with("![") || trimmed.starts_with("[![")
}

/// The kind of each of `lines`, in order.
fn classify(lines: &[String]) -> Vec<LineKind> {
    let mut kinds = Vec::with_capacity(lines.len());
    let mut open_fence: Option<(char, usize)> = None;
    let mut in_html_comment = false;
    // The open list items, outermost first: each its marker's indentation
    // and its number. The indentations strictly increase.
    let mut open_items: Vec<(usize, usize)> = Vec::new();
    let mut item_count = 0;
    for line in lines {
        let trimmed = line.trim();
        if let Some(open) = open_fence {
            if closes(trimmed, open) {
                open_fence = None;
            }
            kinds.push(LineKind::Other);
            continue;
        }
        if in_html_comment {
            if trimmed.contains("-->") {
                in_html_comment = false;
            }
            kinds.push(LineKind::Other);
            continue;
        }
        if trimmed.is_empty() {
            kinds.push(LineKind::Blank);
            continue;
        }
        let column = indent(line);
        let item = if is_list_item(trimmed) {
            // A new item closes the open items at its indentation or deeper.
            open_items.retain(|(marker, _)| *marker < column);
            open_items.push((column, item_count));
            item_count += 1;
            Some(item_count - 1)
        } else if open_items.first().is_some_and(|(outer, _)| column > *outer) {
            // A continuation line belongs to the innermost open item it is
            // indented under, and closes the items nested deeper.
            open_items.retain(|(marker, _)| *marker < column);
            open_items.last().map(|(_, number)| *number)
        } else {
            open_items.clear();
            None
        };
        if let Some(opened) = fence(trimmed) {
            open_fence = Some(opened);
        }
        if trimmed.starts_with("<!--") && !trimmed.contains("-->") {
            in_html_comment = true;
        }
        let exempt = open_fence.is_some()
            || is_heading(trimmed)
            || trimmed.starts_with('|')
            || trimmed.starts_with('>')
            || is_link_reference_definition(trimmed)
            || is_html(trimmed)
            || is_badge(trimmed);
        kinds.push(match (exempt, item) {
            (true, _) => LineKind::Other,
            (false, Some(number)) => LineKind::Item(number),
            (false, None) => LineKind::Prose,
        });
    }
    kinds
}

/// The prose paragraphs and list items of `lines` longer than `max` display
/// lines, in document order.
fn long_blocks(lines: &[(usize, String)], max: usize) -> Vec<Long> {
    let texts: Vec<String> = lines.iter().map(|(_, text)| text.clone()).collect();
    let kinds = classify(&texts);
    let mut long = Vec::new();
    // The paragraph in progress: its first line number and its display lines.
    let mut paragraph: Option<(usize, usize)> = None;
    // Each list item's first line number and display lines, by its number.
    let mut items: BTreeMap<usize, (usize, usize)> = BTreeMap::new();
    for ((number, text), kind) in lines.iter().zip(&kinds) {
        if *kind == LineKind::Prose {
            let weight = display_lines(text);
            paragraph =
                Some(paragraph.map_or((*number, weight), |(first, total)| (first, total + weight)));
            continue;
        }
        if let Some((first, total)) = paragraph.take() {
            if total > max {
                long.push((first, total, Paragraph));
            }
        }
        if let LineKind::Item(item) = kind {
            items.entry(*item).or_insert((*number, 0)).1 += display_lines(text);
        }
    }
    if let Some((first, total)) = paragraph {
        if total > max {
            long.push((first, total, Paragraph));
        }
    }
    long.extend(
        items
            .into_values()
            .filter(|(_, total)| *total > max)
            .map(|(first, total)| (first, total, ListItem)),
    );
    long.sort_by_key(|(first, _, _)| *first);
    long
}

/// The long blocks of a snippet, numbered from line 1.
fn long_in(snippet: &str) -> Vec<Long> {
    long_blocks(&markdown_lines(snippet), MAX_LINES)
}

/// The long blocks of the document `rel`, of numbered `lines`, as
/// `rel:line (N lines[, list item])` problems.
fn long_block_problems(rel: &str, lines: &[(usize, String)]) -> Vec<String> {
    long_blocks(lines, MAX_LINES)
        .into_iter()
        .map(|(line, length, block)| match block {
            Paragraph => format!("{rel}:{line} ({length} lines)"),
            ListItem => format!("{rel}:{line} ({length} lines, list item)"),
        })
        .collect()
}

/// The path of `path` from the repository root, with `/` separators.
fn repository_relative(path: &Path) -> String {
    path.strip_prefix(repository_root())
        .unwrap_or(path)
        .display()
        .to_string()
        .replace('\\', "/")
}

// ---------------------------------------------------------------------------
// LIB-DOC-009: no prose paragraph or list item over four lines
// ---------------------------------------------------------------------------

#[test]
fn lib_doc_009_no_paragraph_or_list_item_in_the_entry_docs_runs_over_four_lines() {
    let mut problems = Vec::new();
    let mut documents = vec![(CRATE_ROOT, inner_doc_lines(&read(CRATE_ROOT)))];
    for rel in ENTRY_MARKDOWN {
        match read_optional(rel) {
            Some(text) => documents.push((rel, markdown_lines(&text))),
            None => problems.push(format!("{rel}: missing")),
        }
    }
    assert!(
        documents[0].1.len() > 10,
        "LIB-DOC-009: {CRATE_ROOT} must hold the crate's `//!` documentation"
    );
    for (rel, lines) in &documents {
        for (line, length, block) in long_blocks(lines, MAX_LINES) {
            problems.push(match block {
                Paragraph => format!("{rel}:{line} ({length} lines)"),
                ListItem => format!("{rel}:{line} ({length} lines, list item)"),
            });
        }
    }
    assert!(
        problems.is_empty(),
        "LIB-DOC-009: no prose paragraph or list item in the entry documents may run over \
         {MAX_LINES} display lines of {DISPLAY_WIDTH} characters (docs/style.md). Found: {problems:#?}"
    );
}

#[test]
fn lib_doc_009_the_rule_skips_tables_code_and_markup() {
    let snippet = "\
# A heading

- A list item that runs
  over a continuation line.
  1. A nested item
     with its continuation,
     and a third line.

  A second paragraph of the item,
  indented under it.
* A star item
+ A plus item
1. A numbered item
2) Another

| A | table |
|---|---|
| with | rows |
| and | more |
| and | more |

```rust
let prose_like = 1;
like a paragraph
inside a fence
of six lines
over the limit
```

~~~~text
```
a shorter fence inside a longer one
does not close it
and these lines
are still code
```
~~~~

> A quote
> of five
> lines
> is not
> prose.

[a]: https://example.org/a
[b]: https://example.org/b
[c]: https://example.org/c
[d]: https://example.org/d
[e]: https://example.org/e

<p>
<b>HTML</b>
</p>
<!-- a comment
that runs
over five
lines and
is hidden -->
<br>

[![CI](https://example.org/ci.svg)](https://example.org/ci)
[![Licence](https://example.org/l.svg)](https://example.org/l)
![MSRV](https://example.org/m.svg)
[![Docs](https://example.org/d.svg)](https://example.org/d)
[![Crate](https://example.org/c.svg)](https://example.org/c)
";
    assert_eq!(
        long_in(snippet),
        [],
        "LIB-DOC-009: list items within the limit, tables, code fences, block quotes, \
         link reference definitions, HTML and badges are not reported"
    );
    let shorter_close = "````\n```\na\nb\nc\nd\ne\n````\n";
    assert_eq!(
        long_in(shorter_close),
        [],
        "LIB-DOC-009: a backtick fence shorter than the opening fence does not close it"
    );
    let info_close = "```\n```rust\na\nb\nc\nd\ne\n```\n";
    assert_eq!(
        long_in(info_close),
        [],
        "LIB-DOC-009: a fence line with an info string does not close the open fence"
    );
}

#[test]
fn lib_doc_009_the_paragraph_rule_reports_a_prose_paragraph_of_five_lines() {
    let five = "Intro.\n\nOne,\ntwo,\nthree,\nfour,\nfive.\n";
    assert_eq!(
        long_in(five),
        [(3, 5, Paragraph)],
        "LIB-DOC-009: a prose paragraph of five lines is reported at its first line"
    );
    let four = "One,\ntwo,\nthree,\nfour.\n";
    assert_eq!(
        long_in(four),
        [],
        "LIB-DOC-009: a prose paragraph of four lines is within the limit"
    );
    let split = "One,\ntwo,\nthree.\n\nFour,\nfive,\nsix.\n";
    assert_eq!(
        long_in(split),
        [],
        "LIB-DOC-009: a blank line ends a paragraph"
    );
    let by_heading = "One,\ntwo,\nthree.\n## Heading\nFour,\nfive,\nsix.\n";
    assert_eq!(
        long_in(by_heading),
        [],
        "LIB-DOC-009: a heading ends a paragraph"
    );
    let at_end = "One,\ntwo,\nthree,\nfour,\nfive,\nsix.";
    assert_eq!(
        long_in(at_end),
        [(1, 6, Paragraph)],
        "LIB-DOC-009: a paragraph at the end of the document, without a final newline, is reported"
    );
    let with_link = "One [link](https://example.org),\n**two**,\n`three`,\n*four*,\nfive.\n";
    assert_eq!(
        long_in(with_link),
        [(1, 5, Paragraph)],
        "LIB-DOC-009: inline links, emphasis and code spans are prose"
    );
}

#[test]
fn lib_doc_009_the_paragraph_rule_counts_prose_after_a_list_or_a_fence() {
    let after_list = "- An item\n  continued.\n\nOne,\ntwo,\nthree,\nfour,\nfive.\n";
    assert_eq!(
        long_in(after_list),
        [(4, 5, Paragraph)],
        "LIB-DOC-009: an unindented paragraph after a list is prose"
    );
    let after_fence = "```\ncode\n```\nOne,\ntwo,\nthree,\nfour,\nfive.\n";
    assert_eq!(
        long_in(after_fence),
        [(4, 5, Paragraph)],
        "LIB-DOC-009: the lines after a closed fence are prose again"
    );
    let two_backticks = "``x`` starts,\ntwo,\nthree,\nfour,\nfive.\n";
    assert_eq!(
        long_in(two_backticks),
        [(1, 5, Paragraph)],
        "LIB-DOC-009: two backticks open a code span, not a fence, so the lines are prose"
    );
    let after_comment = "<!-- a\ncomment -->\nOne,\ntwo,\nthree,\nfour,\nfive.\n";
    assert_eq!(
        long_in(after_comment),
        [(3, 5, Paragraph)],
        "LIB-DOC-009: the lines after a closed HTML comment are prose again"
    );
    let lazy = "- An item\nOne,\ntwo,\nthree,\nfour,\nfive.\n";
    assert_eq!(
        long_in(lazy),
        [(2, 5, Paragraph)],
        "LIB-DOC-009: unindented lines after a list item are not indented under it, so they are prose"
    );
    let autolink = "<https://example.org>\none,\ntwo,\nthree,\nfour.\n";
    assert_eq!(
        long_in(autolink),
        [(1, 5, Paragraph)],
        "LIB-DOC-009: an autolink is prose, not HTML"
    );
    let hashtag = "#hashtag\none,\ntwo,\nthree,\nfour.\n";
    assert_eq!(
        long_in(hashtag),
        [(1, 5, Paragraph)],
        "LIB-DOC-009: `#` without a space is not a heading"
    );
}

#[test]
fn lib_doc_009_the_rule_measures_list_items_and_display_lines() {
    let x = |n: usize| "x".repeat(n);

    // A list item counts its marker line and its continuation lines.
    let five = "- one,\n  two,\n  three,\n  four,\n  five.\n";
    assert_eq!(
        long_in(five),
        [(1, 5, ListItem)],
        "LIB-DOC-009: a list item of five lines is reported at its marker line"
    );
    let nested = "- one,\n  two,\n  - n1,\n    n2,\n    n3.\n  three,\n  four.\n";
    assert_eq!(
        long_in(nested),
        [],
        "LIB-DOC-009: a four-line item with a three-line nested item is within the limit; \
         the nested item's lines are its own"
    );
    let deep = "- top.\n  - one,\n    two,\n    three,\n    four,\n    five.\n";
    assert_eq!(
        long_in(deep),
        [(2, 5, ListItem)],
        "LIB-DOC-009: a nested item of five lines is reported at its own marker line"
    );
    let siblings = "1. one,\n   two,\n   three.\n2. four,\n   five.\n";
    assert_eq!(
        long_in(siblings),
        [],
        "LIB-DOC-009: each sibling item counts only its own lines"
    );
    let paragraphs = "- one,\n  two.\n\n  three,\n  four,\n  five.\n";
    assert_eq!(
        long_in(paragraphs),
        [(1, 5, ListItem)],
        "LIB-DOC-009: an item's paragraphs, across a blank line, count together"
    );
    let with_code = "- one,\n  ```\n  a\n  b\n  c\n  d\n  ```\n  two.\n";
    assert_eq!(
        long_in(with_code),
        [],
        "LIB-DOC-009: a code fence in an item is exempt from its count"
    );

    // Each prose or list-item line counts as ceil(characters / 100) lines.
    assert_eq!(
        long_in(&x(450)),
        [(1, 5, Paragraph)],
        "LIB-DOC-009: a one-line paragraph of 450 characters counts as five lines"
    );
    assert_eq!(
        long_in(&x(400)),
        [],
        "LIB-DOC-009: a one-line paragraph of 400 characters counts as four lines"
    );
    assert_eq!(
        long_in(&x(401)),
        [(1, 5, Paragraph)],
        "LIB-DOC-009: a one-line paragraph of 401 characters counts as five lines"
    );
    assert_eq!(
        long_in(&"é".repeat(400)),
        [],
        "LIB-DOC-009: characters are counted as Unicode scalar values, not bytes"
    );
    assert_eq!(
        long_in(&format!("{}{}", " ".repeat(8), x(400))),
        [],
        "LIB-DOC-009: indentation is not counted"
    );
    assert_eq!(
        long_in(&format!("one,\n{}\nfour.\n", x(250))),
        [(1, 5, Paragraph)],
        "LIB-DOC-009: a paragraph sums the display lines of its lines"
    );
    assert_eq!(
        long_in(&format!("- {}\n  {}\n", x(298), x(100))),
        [],
        "LIB-DOC-009: an item of 300 and 100 characters counts as four lines"
    );
    assert_eq!(
        long_in(&format!("- {}\n  {}\n", x(298), x(101))),
        [(1, 5, ListItem)],
        "LIB-DOC-009: an item sums the display lines of its lines"
    );

    // Exempt lines are not measured, however long.
    for exempt in [
        format!("| {} |", x(600)),
        format!("# {}", x(600)),
        format!("[a]: https://example.org/{}", x(600)),
        format!("<p>{}</p>", x(600)),
        format!("```\n{}\n```", x(600)),
    ] {
        assert_eq!(
            long_in(&exempt),
            [],
            "LIB-DOC-009: a long table row, heading, link reference definition, HTML or code \
             line is exempt"
        );
    }
}

#[test]
fn lib_doc_009_the_crate_root_block_is_read_without_its_prefix() {
    let source = "\
//! Line one,
//! line two.
//!
//! ```
//! # use hidden;
//! let code = 1;
//! ```
//!
//!   - An indented item
//!     continued.
//! One,
//! two,
//! three,
//! four,
//! five.

// A plain comment is not documentation.
pub mod a;
";
    let lines = inner_doc_lines(source);
    assert_eq!(
        lines.first(),
        Some(&(1, "Line one,".to_owned())),
        "LIB-DOC-009: a `//!` line is read without its prefix and the space after it"
    );
    assert_eq!(
        lines.get(2),
        Some(&(3, String::new())),
        "LIB-DOC-009: a bare `//!` is a blank line"
    );
    assert_eq!(
        lines.get(8),
        Some(&(9, "  - An indented item".to_owned())),
        "LIB-DOC-009: the indentation after the prefix's space is kept"
    );
    assert_eq!(
        lines.len(),
        15,
        "LIB-DOC-009: only the `//!` lines are documentation"
    );
    assert_eq!(
        long_blocks(&lines, MAX_LINES),
        [(11, 5, Paragraph)],
        "LIB-DOC-009: a long paragraph of the crate root is reported at its source line; \
         the doctest is code"
    );
}

// ---------------------------------------------------------------------------
// LIB-DOC-011: no prose paragraph or list item over four lines in module docs
// ---------------------------------------------------------------------------

#[test]
fn lib_doc_011_no_paragraph_or_list_item_in_the_module_docs_runs_over_four_lines() {
    // Control: a module's `//!` block is read as the crate root's is, and
    // a long paragraph and a long list item in it are reported at their
    // source lines.
    let module = "\
//! The module.
//!
//! One,
//! two,
//! three,
//! four,
//! five.
//!
//! - one,
//!   two,
//!   three,
//!   four,
//!   five.

/// An item's docs are not the module's.
pub fn f() {}
";
    assert_eq!(
        long_block_problems("m.rs", &inner_doc_lines(module)),
        ["m.rs:3 (5 lines)", "m.rs:9 (5 lines, list item)"],
        "LIB-DOC-011: a module's long paragraph and long list item are reported as \
         `file:line (N lines[, list item])`"
    );

    let mut files = Vec::new();
    for dir in MODULE_SOURCES {
        let before = files.len();
        rust_files(&repository_root().join(dir), &mut files);
        assert!(
            files.len() > before,
            "LIB-DOC-011: the sources under {dir} must be found"
        );
    }
    let mut problems = Vec::new();
    let mut documented = 0;
    for path in &files {
        let rel = repository_relative(path);
        if rel == CRATE_ROOT {
            // LIB-DOC-009 checks the crate root.
            continue;
        }
        let text =
            fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        let lines = inner_doc_lines(&text);
        if !lines.is_empty() {
            documented += 1;
        }
        problems.extend(long_block_problems(&rel, &lines));
    }
    assert!(
        documented > 10,
        "LIB-DOC-011: the module docs must be found in the `//!` blocks under \
         {MODULE_SOURCES:?}; found {documented} files with one"
    );
    assert!(
        problems.is_empty(),
        "LIB-DOC-011: no prose paragraph or list item in a module's `//!` docs may run over \
         {MAX_LINES} display lines of {DISPLAY_WIDTH} characters (docs/style.md). Found: {problems:#?}"
    );
}

// ---------------------------------------------------------------------------
// LIB-DOC-013: no prose paragraph or list item over four lines in docs/sacm
// ---------------------------------------------------------------------------

/// The introduction of a Markdown document: its numbered lines before its
/// first line that starts with `## `, or `None` if it has no such line.
fn introduction(text: &str) -> Option<Vec<(usize, String)>> {
    let lines = markdown_lines(text);
    let end = lines.iter().position(|(_, line)| line.starts_with("## "))?;
    Some(lines[..end].to_vec())
}

#[test]
fn lib_doc_013_no_paragraph_or_list_item_in_the_sacm_references_runs_over_four_lines() {
    // Controls: the introduction ends at the first `## ` heading, not at a
    // deeper one, and its lines keep their numbers.
    let generated = "# Title\n\nIntro.\n### Deeper\nStill intro.\n## Section\nOne,\ntwo,\nthree,\nfour,\nfive.\n## Next\n";
    let intro = introduction(generated);
    assert_eq!(
        intro.as_ref().map(Vec::len),
        Some(5),
        "LIB-DOC-013: the introduction is the text before the first line starting with `## `"
    );
    assert_eq!(
        intro
            .as_deref()
            .map(|lines| long_block_problems("i.md", lines)),
        Some(Vec::new()),
        "LIB-DOC-013: a long paragraph after the introduction is not the introduction's"
    );
    let long_intro = "# Title\n\nOne,\ntwo,\nthree,\nfour,\nfive.\n## Section\n";
    assert_eq!(
        introduction(long_intro)
            .as_deref()
            .map(|lines| long_block_problems("i.md", lines)),
        Some(vec!["i.md:3 (5 lines)".to_owned()]),
        "LIB-DOC-013: a long paragraph in the introduction is reported at its line"
    );
    assert_eq!(
        introduction("# Title\n\nNo section.\n"),
        None,
        "LIB-DOC-013: a document without a `## ` heading has no introduction to check"
    );

    let mut problems = Vec::new();
    for rel in SACM_REFERENCES {
        match read_optional(rel) {
            Some(text) => problems.extend(long_block_problems(rel, &markdown_lines(&text))),
            None => problems.push(format!("{rel}: missing")),
        }
    }
    match read_optional(INVENTORY).map(|text| introduction(&text)) {
        Some(Some(lines)) => problems.extend(long_block_problems(INVENTORY, &lines)),
        Some(None) => problems.push(format!(
            "{INVENTORY}: no `## ` heading ends its introduction"
        )),
        None => problems.push(format!("{INVENTORY}: missing")),
    }
    assert!(
        problems.is_empty(),
        "LIB-DOC-013: no prose paragraph or list item in {SACM_REFERENCES:?}, or in the \
         introduction of {INVENTORY}, may run over {MAX_LINES} display lines of \
         {DISPLAY_WIDTH} characters (docs/style.md). Found: {problems:#?}"
    );
}

// ---------------------------------------------------------------------------
// LIB-DOC-012: item docs of model, compare, diagnostic and validation
// ---------------------------------------------------------------------------

/// The modules of `crates/sacm/src` whose `///` docs LIB-DOC-012 checks:
/// each `<module>.rs` and every `.rs` file under `<module>/`.
const ITEM_DOC_MODULES: [&str; 4] = ["model", "compare", "diagnostic", "validation"];

/// The longest item summary allowed, in display lines.
const MAX_SUMMARY_LINES: usize = 2;

/// One item's `///` docs, as [`doc_blocks`] reads them.
struct DocBlock {
    /// The doc lines, each with its 1-based number and its text.
    lines: Vec<(usize, String)>,
    /// The first line after the block that is not blank, a comment or an
    /// attribute, trimmed: the documented item's. Empty at the end of the file.
    item: String,
    /// Whether the block is inside a `#[cfg(test)]` module.
    in_test: bool,
    /// Whether the innermost `impl` block around the block is a trait impl,
    /// `impl … for … {`.
    in_trait_impl: bool,
}

/// The text of a `///` line, at any indentation, without the `///` and the
/// one space after it; `None` for any other line, `////` included.
fn doc_text(line: &str) -> Option<String> {
    let rest = line
        .trim_start()
        .strip_prefix("///")
        .filter(|rest| !rest.starts_with('/'))?;
    Some(rest.strip_prefix(' ').unwrap_or(rest).to_owned())
}

/// Whether `line` starts an outer attribute, `#[…]`.
fn is_attribute(line: &str) -> bool {
    line.trim_start().starts_with("#[")
}

/// The index of the line after the attribute that starts at `lines[start]`:
/// the attribute ends on the line where its brackets balance.
fn attribute_end(lines: &[&str], start: usize) -> usize {
    let mut depth = 0_i64;
    for (i, line) in lines.iter().enumerate().skip(start) {
        for c in line.chars() {
            match c {
                '[' => depth += 1,
                ']' => depth -= 1,
                _ => {}
            }
        }
        if depth <= 0 {
            return i + 1;
        }
    }
    lines.len()
}

/// The `#[cfg(test)]` modules of `lines`, as index ranges: from the
/// `#[cfg(test)]` line to the `}` at the indentation of the `mod … {` line
/// after it (the sources are rustfmt-formatted).
fn test_modules(lines: &[&str]) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if line.trim() != "#[cfg(test)]" {
            continue;
        }
        let mut header = i + 1;
        while header < lines.len() && is_attribute(lines[header]) {
            header = attribute_end(lines, header);
        }
        let Some(text) = lines.get(header) else {
            continue;
        };
        let trimmed = text.trim();
        if !(trimmed.ends_with('{') && trimmed.split_whitespace().any(|word| word == "mod")) {
            continue;
        }
        let close = format!("{}}}", &text[..text.len() - text.trim_start().len()]);
        let end = lines[header + 1..]
            .iter()
            .position(|line| *line == close)
            .map_or(lines.len(), |k| header + 1 + k);
        ranges.push((i, end));
    }
    ranges
}

/// The `impl` blocks of `lines`, as index ranges, each with whether it is a
/// trait impl: from the header line, `impl…` at indentation N, to the first
/// line after it that is exactly N spaces and `}` (the sources are
/// rustfmt-formatted). A header with ` for ` in it is a trait impl's; an
/// `impl … {}` on one line holds nothing.
fn impl_blocks(lines: &[&str]) -> Vec<(usize, usize, bool)> {
    let mut ranges = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if !(trimmed.starts_with("impl ") || trimmed.starts_with("impl<"))
            || trimmed.ends_with("{}")
            || trimmed.ends_with(';')
        {
            continue;
        }
        let close = format!("{}}}", &line[..line.len() - line.trim_start().len()]);
        let end = lines[i + 1..]
            .iter()
            .position(|line| *line == close)
            .map_or(lines.len(), |k| i + 1 + k);
        ranges.push((i, end, trimmed.contains(" for ")));
    }
    ranges
}

/// The `///` blocks of a Rust source. A block is a run of `///` lines, at any
/// indentation, and the attribute lines between them (`#[doc(alias = …)]`):
/// it ends at the first line that is neither. A bare `///` is a blank line;
/// `////` is a plain comment; `//!` and `#[doc = …]` are not read.
fn doc_blocks(source: &str) -> Vec<DocBlock> {
    let lines: Vec<&str> = source.lines().collect();
    let tests = test_modules(&lines);
    let impls = impl_blocks(&lines);
    let mut blocks = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        if doc_text(lines[i]).is_none() {
            i += 1;
            continue;
        }
        let start = i;
        let mut block = Vec::new();
        while i < lines.len() {
            if let Some(text) = doc_text(lines[i]) {
                block.push((i + 1, text));
                i += 1;
            } else if is_attribute(lines[i]) {
                i = attribute_end(&lines, i);
            } else {
                break;
            }
        }
        let mut next = i;
        while next < lines.len() {
            let trimmed = lines[next].trim();
            if is_attribute(trimmed) {
                next = attribute_end(&lines, next);
            } else if trimmed.is_empty() || trimmed.starts_with("//") {
                next += 1;
            } else {
                break;
            }
        }
        blocks.push(DocBlock {
            lines: block,
            item: lines.get(next).map_or("", |line| line.trim()).to_owned(),
            in_test: within(&tests, start),
            in_trait_impl: in_trait_impl(&impls, start),
        });
    }
    blocks
}

/// Whether the line at `index` is strictly inside one of `ranges`, the index
/// ranges [`test_modules`] or [`macro_bodies`] gives.
fn within(ranges: &[(usize, usize)], index: usize) -> bool {
    ranges.iter().any(|(from, to)| index > *from && index < *to)
}

/// Whether the innermost of the `impl` blocks `impls` around the line at
/// `index` is a trait impl.
fn in_trait_impl(impls: &[(usize, usize, bool)], index: usize) -> bool {
    impls
        .iter()
        .filter(|(from, to, _)| index > *from && index < *to)
        .max_by_key(|(from, _, _)| *from)
        .is_some_and(|(_, _, is_trait)| *is_trait)
}

/// The lines of every `///` block of a Rust source, public or not.
fn outer_doc_blocks(source: &str) -> Vec<Vec<(usize, String)>> {
    doc_blocks(source)
        .into_iter()
        .map(|block| block.lines)
        .collect()
}

/// Whether the item line `item` is clearly not public: restricted
/// (`pub(crate)`, `pub(super)`, `pub(in …)`) or an item keyword without
/// `pub`. Enum variants, fields and `impl` blocks count; the parent's
/// visibility is not resolved. The items of trait impls are left to
/// [`counts`].
fn is_non_public_item(item: &str) -> bool {
    const RESTRICTED: [&str; 3] = ["pub(crate)", "pub(super)", "pub(in"];
    const PRIVATE: [&str; 11] = [
        "fn ",
        "const fn ",
        "unsafe fn ",
        "struct ",
        "enum ",
        "trait ",
        "type ",
        "const ",
        "static ",
        "mod ",
        "macro_rules!",
    ];
    RESTRICTED
        .iter()
        .chain(PRIVATE.iter())
        .any(|prefix| item.starts_with(prefix))
}

/// Whether LIB-DOC-012 checks `block`: outside `#[cfg(test)]` modules, the
/// docs of a `fn`, `type` or `const` of a trait impl, which rustdoc shows,
/// and of any item [`is_non_public_item`] does not rule out.
fn counts(block: &DocBlock) -> bool {
    const TRAIT_IMPL_ITEMS: [&str; 3] = ["fn ", "type ", "const "];
    if block.in_test {
        return false;
    }
    if block.in_trait_impl
        && TRAIT_IMPL_ITEMS
            .iter()
            .any(|prefix| block.item.starts_with(prefix))
    {
        return true;
    }
    !is_non_public_item(&block.item)
}

/// The lines of the `///` blocks of a Rust source that LIB-DOC-012 checks:
/// those of public items and of the items of trait impls, outside
/// `#[cfg(test)]` modules, per [`counts`].
fn public_doc_blocks(source: &str) -> Vec<Vec<(usize, String)>> {
    doc_blocks(source)
        .into_iter()
        .filter(counts)
        .map(|block| block.lines)
        .collect()
}

/// Controls of [`public_doc_blocks`]: which items' docs count.
fn check_public_item_controls() {
    let source = "\
/// Crate-private.
pub(crate) fn a() {}
/// Private.
fn b() {}
/// Public.
pub fn c() {}
/// A public enum.
pub enum E {
    /// A variant.
    V,
    /// A variant with fields.
    W { x: u8 },
}
/// Restricted to the parent, behind an attribute and a comment.
#[cfg_attr(
    feature = \"serde\",
    derive(Debug)
)]
// A plain comment.
pub(super) struct S;
#[cfg(test)]
mod tests {
    /// A test helper, public within its module.
    pub fn h() {}
}
/// After the test module.
pub fn d() {}
impl fmt::Display for E {
    /// Writes it.
    fn fmt(..) {}
}
impl S {
    /// A private method of an inherent impl.
    fn p() {}
    /// A public method of an inherent impl.
    pub fn q() {}
}
/// Private, behind a blank line and an attribute.

#[inline]
fn r() {}
";
    let counted: Vec<usize> = public_doc_blocks(source)
        .iter()
        .filter_map(|block| block.first().map(|(line, _)| *line))
        .collect();
    for (line, what, counts) in [
        (1, "a `pub(crate) fn`", false),
        (3, "a private `fn`", false),
        (5, "a `pub fn`", true),
        (7, "a `pub enum`", true),
        (9, "a unit enum variant", true),
        (11, "a struct enum variant", true),
        (
            14,
            "a `pub(super)` item behind an attribute and a comment",
            false,
        ),
        (23, "a block inside `#[cfg(test)] mod tests { … }`", false),
        (26, "a `pub fn` after the test module", true),
        (29, "a `fn` of a trait impl, `impl … for … {`", true),
        (33, "a private `fn` of an inherent impl", false),
        (35, "a `pub fn` of an inherent impl", true),
        (
            38,
            "a private `fn` behind a blank line and an attribute",
            false,
        ),
    ] {
        assert_eq!(
            counted.contains(&line),
            counts,
            "LIB-DOC-012: the docs of {what} (line {line}) must {}be checked; checked: {counted:?}",
            if counts { "" } else { "not " }
        );
    }
}

/// Controls of [`doc_blocks`]: an attribute between `///` lines does not end
/// the block, so the item has one block and one summary.
fn check_doc_attribute_controls() {
    let source = "\
/// The summary.
#[doc(alias = \"x\")]
///
/// One,
/// two,
/// three.
#[must_use]
pub fn f() {}
";
    let blocks = public_doc_blocks(source);
    assert_eq!(
        blocks.iter().map(Vec::len).collect::<Vec<_>>(),
        [5],
        "LIB-DOC-012: a `#[doc(alias = \"x\")]` between two `///` lines gives one block of \
         five doc lines; found {blocks:?}"
    );
    assert_eq!(
        blocks
            .iter()
            .filter_map(|block| summary(block))
            .collect::<Vec<_>>(),
        [(1, 1)],
        "LIB-DOC-012: the joined block has one summary, its first paragraph"
    );
    assert_eq!(
        long_summary_problems("m.rs", source),
        Vec::<String>::new(),
        "LIB-DOC-012: the paragraph after the attribute is not a second summary"
    );
}

/// The summary of an item's doc block: its first paragraph, if the block
/// opens with prose, as the paragraph's first line number and its display
/// lines. A block that opens with a heading, a list, a table or code has none.
fn summary(block: &[(usize, String)]) -> Option<(usize, usize)> {
    let texts: Vec<String> = block.iter().map(|(_, text)| text.clone()).collect();
    let kinds = classify(&texts);
    let start = kinds.iter().position(|kind| *kind != LineKind::Blank)?;
    if kinds[start] != LineKind::Prose {
        return None;
    }
    let total = block[start..]
        .iter()
        .zip(&kinds[start..])
        .take_while(|(_, kind)| **kind == LineKind::Prose)
        .map(|((_, text), _)| display_lines(text))
        .sum();
    Some((block[start].0, total))
}

/// The long paragraphs and list items of the public `///` blocks of
/// `source`, the file `rel`, as `rel:line (N lines[, list item])` problems.
fn long_item_doc_problems(rel: &str, source: &str) -> Vec<String> {
    public_doc_blocks(source)
        .iter()
        .flat_map(|block| long_block_problems(rel, block))
        .collect()
}

/// The summaries over [`MAX_SUMMARY_LINES`] of the public `///` blocks of
/// `source`, the file `rel`, as `rel:line (N lines, summary)` problems.
fn long_summary_problems(rel: &str, source: &str) -> Vec<String> {
    public_doc_blocks(source)
        .iter()
        .filter_map(|block| summary(block))
        .filter(|(_, length)| *length > MAX_SUMMARY_LINES)
        .map(|(line, length)| format!("{rel}:{line} ({length} lines, summary)"))
        .collect()
}

/// The files of the `modules` of `crates/sacm/src` that the row `row`
/// checks, each `<module>.rs` and every `.rs` file under `<module>/`, each
/// with its path from the repository root and its text, in a stable order.
fn item_doc_sources(row: &str, modules: &[&str]) -> Vec<(String, String)> {
    let src = repository_root().join("crates/sacm/src");
    let mut files = Vec::new();
    for module in modules {
        let file = src.join(format!("{module}.rs"));
        assert!(
            file.is_file(),
            "{row}: {} must exist",
            repository_relative(&file)
        );
        files.push(file);
        let dir = src.join(module);
        if dir.is_dir() {
            rust_files(&dir, &mut files);
        }
    }
    files
        .iter()
        .map(|path| {
            let text =
                fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
            (repository_relative(path), text)
        })
        .collect()
}

#[test]
fn lib_doc_012_the_item_doc_reader_splits_blocks_and_finds_summaries() {
    // Two items separated by code are two blocks.
    let two_items = "\
/// The first item.
pub fn a() {}
/// The second item.
///
/// Its details.
pub fn b() {}
//! Not an item's docs.
//// Not docs either.
";
    let blocks = outer_doc_blocks(two_items);
    assert_eq!(
        blocks,
        [
            vec![(1, "The first item.".to_owned())],
            vec![
                (3, "The second item.".to_owned()),
                (4, String::new()),
                (5, "Its details.".to_owned()),
            ],
        ],
        "LIB-DOC-012: two items separated by code are two blocks, read without the `/// ` \
         prefix, a bare `///` blank; `//!` and `////` lines are not item docs"
    );
    let split =
        "/// One,\n/// two,\n/// three.\npub struct S;\n/// Four,\n/// five.\npub struct T;\n";
    assert_eq!(
        long_item_doc_problems("m.rs", split),
        Vec::<String>::new(),
        "LIB-DOC-012: the lines of two blocks are not counted together"
    );

    // An indented `///` (an impl method) is read, at its source line.
    let method = "\
impl S {
    /// One,
    /// two,
    /// three,
    /// four,
    /// five.
    pub fn f(&self) {}
}
";
    assert_eq!(
        long_item_doc_problems("m.rs", method),
        ["m.rs:2 (5 lines)"],
        "LIB-DOC-012: an indented `///` block is read, and a long paragraph in it is reported \
         at its source line"
    );
    assert_eq!(
        outer_doc_blocks("    ///   - nested\n")
            .first()
            .and_then(|b| b.first()),
        Some(&(1, "  - nested".to_owned())),
        "LIB-DOC-012: the indentation after the prefix's space is kept"
    );

    // A list item over four lines is reported as a list item.
    let item = "/// The item.\n///\n/// - one,\n///   two,\n///   three,\n///   four,\n///   five.\npub fn f() {}\n";
    assert_eq!(
        long_item_doc_problems("m.rs", item),
        ["m.rs:3 (5 lines, list item)"],
        "LIB-DOC-012: a long list item is reported as `file:line (N lines, list item)`"
    );

    // A 2-line summary followed by a long list item reports only the list item.
    let summary_then_item = "\
/// One,
/// two.
///
/// - one,
///   two,
///   three,
///   four,
///   five.
pub fn f() {}
";
    assert_eq!(
        long_summary_problems("m.rs", summary_then_item),
        Vec::<String>::new(),
        "LIB-DOC-012: a summary of two display lines is within the limit"
    );
    assert_eq!(
        long_item_doc_problems("m.rs", summary_then_item),
        ["m.rs:4 (5 lines, list item)"],
        "LIB-DOC-012: after a two-line summary only the long list item is reported"
    );
    let summary_then_list = "/// One,\n/// two.\n/// - three,\n/// - four.\npub fn f() {}\n";
    assert_eq!(
        long_summary_problems("m.rs", summary_then_list),
        Vec::<String>::new(),
        "LIB-DOC-012: a list right after the summary is not part of it"
    );
}

/// The four-line rule over the public `///` docs of `modules`, for the row
/// `row`: more than 100 public blocks are found, and no prose paragraph or
/// list item in them runs over [`MAX_LINES`].
fn assert_no_long_item_docs(row: &str, modules: &[&str]) {
    let sources = item_doc_sources(row, modules);
    let blocks: usize = sources
        .iter()
        .map(|(_, text)| public_doc_blocks(text).len())
        .sum();
    assert!(
        blocks > 100,
        "{row}: the public item docs must be found in the `///` blocks of the \
         {modules:?} modules; found {blocks} blocks"
    );
    let problems: Vec<String> = sources
        .iter()
        .flat_map(|(rel, text)| long_item_doc_problems(rel, text))
        .collect();
    assert!(
        problems.is_empty(),
        "{row}: no prose paragraph or list item in the `///` docs of the \
         {modules:?} modules may run over {MAX_LINES} display lines of \
         {DISPLAY_WIDTH} characters (docs/style.md); found {}: {problems:#?}",
        problems.len()
    );
}

/// The two-line summary rule over the public `///` docs of `modules`, for
/// the row `row`: more than 100 summaries are found, and none runs over
/// [`MAX_SUMMARY_LINES`].
fn assert_no_long_summaries(row: &str, modules: &[&str]) {
    let sources = item_doc_sources(row, modules);
    let summaries: usize = sources
        .iter()
        .map(|(_, text)| {
            public_doc_blocks(text)
                .iter()
                .filter(|block| summary(block).is_some())
                .count()
        })
        .sum();
    assert!(
        summaries > 100,
        "{row}: the item summaries must be found in the `///` blocks of the \
         {modules:?} modules; found {summaries}"
    );
    let problems: Vec<String> = sources
        .iter()
        .flat_map(|(rel, text)| long_summary_problems(rel, text))
        .collect();
    assert!(
        problems.is_empty(),
        "{row}: no item summary (the first paragraph of an item's `///` docs) in the \
         {modules:?} modules may run over {MAX_SUMMARY_LINES} display lines of \
         {DISPLAY_WIDTH} characters (docs/style.md); found {}: {problems:#?}",
        problems.len()
    );
}

#[test]
fn lib_doc_012_no_paragraph_or_list_item_in_the_item_docs_runs_over_four_lines() {
    // Controls: only the docs of public items, outside test modules, count.
    check_public_item_controls();

    assert_no_long_item_docs("LIB-DOC-012", &ITEM_DOC_MODULES);
}

#[test]
fn lib_doc_012_no_item_summary_runs_over_two_lines() {
    // Controls: a 3-line summary fails, counted in display lines from its
    // first line.
    let three = "/// One,\n/// two,\n/// three.\n///\n/// Details.\npub fn f() {}\n";
    assert_eq!(
        long_summary_problems("m.rs", three),
        ["m.rs:1 (3 lines, summary)"],
        "LIB-DOC-012: a summary of three display lines is reported as `file:line (N lines, summary)`"
    );
    let wide = format!("/// {}\npub fn f() {{}}\n", "x".repeat(201));
    assert_eq!(
        long_summary_problems("m.rs", &wide),
        ["m.rs:1 (3 lines, summary)"],
        "LIB-DOC-012: a summary line of 201 characters counts as three display lines"
    );
    let indented = "    ///\n    /// One,\n    /// two,\n    /// three.\n    pub fn f() {}\n";
    assert_eq!(
        long_summary_problems("m.rs", indented),
        ["m.rs:2 (3 lines, summary)"],
        "LIB-DOC-012: the summary is the first paragraph after leading blank lines, reported \
         at its first line"
    );

    // Controls: a block that opens with a heading, a list or code has no
    // summary.
    for (opening, block) in [
        (
            "a heading",
            "/// # Examples\n///\n/// One,\n/// two,\n/// three.\npub fn f() {}\n",
        ),
        (
            "a list",
            "/// - One,\n///   two,\n///   three.\npub fn f() {}\n",
        ),
        (
            "code",
            "/// ```rust,ignore\n/// let x = 1;\n/// ```\n/// One,\n/// two,\n/// three.\npub fn f() {}\n",
        ),
    ] {
        assert_eq!(
            outer_doc_blocks(block).first().and_then(|b| summary(b)),
            None,
            "LIB-DOC-012: a block that opens with {opening} has no summary"
        );
        assert_eq!(
            long_summary_problems("m.rs", block),
            Vec::<String>::new(),
            "LIB-DOC-012: a block that opens with {opening} is not a summary failure"
        );
    }

    // Controls: an attribute between `///` lines does not split a block.
    check_doc_attribute_controls();

    assert_no_long_summaries("LIB-DOC-012", &ITEM_DOC_MODULES);
}

// ---------------------------------------------------------------------------
// LIB-DOC-014 and LIB-DOC-015: item docs of edit and xmi
// ---------------------------------------------------------------------------

/// The module whose `///` docs LIB-DOC-014 checks, as LIB-DOC-012 checks its
/// modules: `edit.rs` and every `.rs` file under `edit/`.
const EDIT_MODULES: [&str; 1] = ["edit"];

/// The module whose `///` docs LIB-DOC-015 checks, as LIB-DOC-012 checks its
/// modules: `xmi.rs` and every `.rs` file under `xmi/`.
const XMI_MODULES: [&str; 1] = ["xmi"];

/// Whether `sources`, as [`item_doc_sources`] gives them, hold the file
/// `crates/sacm/src/<module>.rs` and a file under `crates/sacm/src/<module>/`.
fn covers_module_and_directory(sources: &[(String, String)], module: &str) -> bool {
    let file = format!("crates/sacm/src/{module}.rs");
    let dir = format!("crates/sacm/src/{module}/");
    sources.iter().any(|(rel, _)| *rel == file)
        && sources.iter().any(|(rel, _)| rel.starts_with(&dir))
}

#[test]
fn lib_doc_014_no_paragraph_or_list_item_in_the_edit_item_docs_runs_over_four_lines() {
    // Control: the module list reaches edit.rs and the files under edit/.
    assert!(
        covers_module_and_directory(&item_doc_sources("LIB-DOC-014", &EDIT_MODULES), "edit"),
        "LIB-DOC-014: crates/sacm/src/edit.rs and the files under crates/sacm/src/edit/ must \
         be checked"
    );
    assert_no_long_item_docs("LIB-DOC-014", &EDIT_MODULES);
}

#[test]
fn lib_doc_014_no_edit_item_summary_runs_over_two_lines() {
    assert_no_long_summaries("LIB-DOC-014", &EDIT_MODULES);
}

#[test]
fn lib_doc_015_no_paragraph_or_list_item_in_the_xmi_item_docs_runs_over_four_lines() {
    // Control: the module list reaches xmi.rs and the files under xmi/.
    assert!(
        covers_module_and_directory(&item_doc_sources("LIB-DOC-015", &XMI_MODULES), "xmi"),
        "LIB-DOC-015: crates/sacm/src/xmi.rs and the files under crates/sacm/src/xmi/ must \
         be checked"
    );
    assert_no_long_item_docs("LIB-DOC-015", &XMI_MODULES);
}

#[test]
fn lib_doc_015_no_xmi_item_summary_runs_over_two_lines() {
    assert_no_long_summaries("LIB-DOC-015", &XMI_MODULES);
}

// ---------------------------------------------------------------------------
// LIB-DOC-002: a runnable example on every public type and fallible function
// ---------------------------------------------------------------------------

/// One item of a Rust source, as [`source_items`] reads it.
struct SourceItem {
    /// The 1-based number of the item's first line.
    line: usize,
    /// The item's lines from its first up to its `{` or `;` (outside
    /// parentheses and brackets), joined by spaces.
    signature: String,
    /// The item's `///` docs, each line with its number and its text.
    docs: Vec<(usize, String)>,
    /// Whether one of the item's own attributes gates it on the
    /// `test-support` feature: `#[cfg(feature = "test-support")]`, or the
    /// `#[cfg_attr(docsrs, doc(cfg(feature = "test-support")))]` that marks
    /// an item public only under it.
    test_support: bool,
    /// Whether the public filter of LIB-DOC-012, [`counts`], counts the
    /// item, and it is outside every `macro_rules!` body.
    public: bool,
    /// The header of the innermost `impl` block around the item, trimmed and
    /// without its `{`; empty outside one.
    owner: String,
}

/// The `macro_rules!` bodies of `lines`, as index ranges: from the
/// `macro_rules! … {` line to the `}` at its indentation (the sources are
/// rustfmt-formatted).
fn macro_bodies(lines: &[&str]) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if !(trimmed.starts_with("macro_rules!") && trimmed.ends_with('{')) {
            continue;
        }
        let close = format!("{}}}", &line[..line.len() - line.trim_start().len()]);
        let end = lines[i + 1..]
            .iter()
            .position(|line| *line == close)
            .map_or(lines.len(), |k| i + 1 + k);
        ranges.push((i, end));
    }
    ranges
}

/// The signature of the item that starts at `lines[start]`: its text up to
/// its first `{` or `;` outside parentheses and brackets, its lines joined by
/// spaces.
fn signature(lines: &[&str], start: usize) -> String {
    let mut text = String::new();
    let mut depth = 0_i64;
    for line in &lines[start..] {
        for c in line.trim().chars() {
            match c {
                '(' | '[' => depth += 1,
                ')' | ']' => depth -= 1,
                '{' | ';' if depth <= 0 => return text.trim().to_owned(),
                _ => {}
            }
            text.push(c);
        }
        text.push(' ');
    }
    text.trim().to_owned()
}

/// The items of a Rust source, each with the `///` docs and the attributes
/// before it. An item's header is the run of `///` lines, attributes, blank
/// lines and comments before its first line; the item is the first line after
/// it that is none of these.
fn source_items(source: &str) -> Vec<SourceItem> {
    let lines: Vec<&str> = source.lines().collect();
    let tests = test_modules(&lines);
    let impls = impl_blocks(&lines);
    let macros = macro_bodies(&lines);
    let mut items = Vec::new();
    let mut docs = Vec::new();
    let mut test_support = false;
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();
        if let Some(text) = doc_text(line) {
            docs.push((i + 1, text));
            i += 1;
            continue;
        }
        if is_attribute(line) {
            let end = attribute_end(&lines, i);
            test_support |= lines[i..end]
                .join(" ")
                .contains("cfg(feature = \"test-support\")");
            i = end;
            continue;
        }
        if trimmed.is_empty() || trimmed.starts_with("//") {
            i += 1;
            continue;
        }
        let block = DocBlock {
            lines: std::mem::take(&mut docs),
            item: trimmed.to_owned(),
            in_test: within(&tests, i),
            in_trait_impl: in_trait_impl(&impls, i),
        };
        let owner = impls
            .iter()
            .filter(|(from, to, _)| i > *from && i < *to)
            .max_by_key(|(from, _, _)| *from)
            .map_or(String::new(), |(from, _, _)| {
                lines[*from].trim().trim_end_matches('{').trim().to_owned()
            });
        items.push(SourceItem {
            line: i + 1,
            signature: signature(&lines, i),
            public: counts(&block) && !within(&macros, i),
            docs: block.lines,
            test_support: std::mem::take(&mut test_support),
            owner,
        });
        i += 1;
    }
    items
}

/// Whether the info string of a fence, the text after its backticks or
/// tildes, makes it a doctest rustdoc runs: empty, or only the tokens
/// `rust`, `should_panic`, `test_harness`, `standalone_crate` and
/// `edition…`. `ignore`, `no_run`, `compile_fail`, another language and any
/// unknown token make it not runnable.
fn is_runnable_info(info: &str) -> bool {
    info.split(|c: char| c == ',' || c.is_whitespace())
        .filter(|token| !token.is_empty())
        .all(|token| {
            matches!(
                token,
                "rust" | "should_panic" | "test_harness" | "standalone_crate"
            ) || token.starts_with("edition")
        })
}

/// Whether the docs `docs` hold an `# Examples` heading followed, in its
/// section (up to the next level-1 heading), by a runnable code fence. A
/// heading inside a code fence is code.
fn has_runnable_example(docs: &[(usize, String)]) -> bool {
    let mut open: Option<(char, usize)> = None;
    let mut in_examples = false;
    for (_, text) in docs {
        let trimmed = text.trim();
        if let Some(fence_open) = open {
            if closes(trimmed, fence_open) {
                open = None;
            }
            continue;
        }
        if let Some(opened) = fence(trimmed) {
            open = Some(opened);
            if in_examples && is_runnable_info(trimmed.trim_start_matches(opened.0)) {
                return true;
            }
            continue;
        }
        if is_heading(trimmed) && trimmed.chars().take_while(|c| *c == '#').count() == 1 {
            in_examples = trimmed == "# Examples";
        }
    }
    false
}

/// Whether the item line `item` declares a public type: `pub struct`,
/// `pub enum` or `pub trait`.
fn is_public_type(item: &str) -> bool {
    ["pub struct ", "pub enum ", "pub trait "]
        .iter()
        .any(|prefix| item.starts_with(prefix))
}

/// Whether the item line `item` declares a `pub fn` (`pub const fn` too).
fn is_public_fn(item: &str) -> bool {
    ["pub fn ", "pub const fn "]
        .iter()
        .any(|prefix| item.starts_with(prefix))
}

/// The return type of the function signature `signature`: the text after
/// its `->` outside parentheses and angle brackets, up to a `where`; `None`
/// without one.
fn return_type(signature: &str) -> Option<&str> {
    let bytes = signature.as_bytes();
    let mut parens = 0_i64;
    let mut angles = 0_i64;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'-' if bytes.get(i + 1) == Some(&b'>') => {
                if parens == 0 && angles == 0 {
                    let rest = &signature[i + 2..];
                    let end = rest.find(" where ").unwrap_or(rest.len());
                    return Some(rest[..end].trim());
                }
                i += 2;
                continue;
            }
            b'(' | b'[' => parens += 1,
            b')' | b']' => parens -= 1,
            b'<' => angles += 1,
            b'>' => angles -= 1,
            _ => {}
        }
        i += 1;
    }
    None
}

/// Whether the function signature `signature` returns `Result<…>`, by any
/// path (`io::Result<…>` too); not an `Option<Result<…>>` or an iterator of
/// results.
fn returns_result(signature: &str) -> bool {
    return_type(signature).is_some_and(|ret| {
        ret.find('<')
            .is_some_and(|open| ret[..open].trim().rsplit("::").next() == Some("Result"))
    })
}

/// A short name of `item` for a problem: its signature up to its first `(`,
/// `<` or `{`, with its `impl` block's header if it has one.
fn item_label(item: &SourceItem) -> String {
    let head = item
        .signature
        .split(['(', '<', '{'])
        .next()
        .unwrap_or_default()
        .trim();
    if item.owner.is_empty() {
        head.to_owned()
    } else {
        format!("{head} (in `{}`)", item.owner)
    }
}

/// The public types of `source`, the file `rel`, without a runnable
/// example, as `rel:line label` problems. Test-support items are exempt:
/// they are outside the stable API (ADR 0045 D7).
fn type_example_problems(rel: &str, source: &str) -> Vec<String> {
    source_items(source)
        .iter()
        .filter(|item| item.public && !item.test_support && is_public_type(&item.signature))
        .filter(|item| !has_runnable_example(&item.docs))
        .map(|item| format!("{rel}:{} {}", item.line, item_label(item)))
        .collect()
}

/// The fallible public functions of `source`, the file `rel`, as
/// [`source_items`] reads them: each `pub fn` the public filter counts,
/// outside test-support (ADR 0045 D7), whose signature returns `Result<…>`.
fn fallible_functions(source: &str) -> Vec<SourceItem> {
    source_items(source)
        .into_iter()
        .filter(|item| {
            item.public
                && !item.test_support
                && is_public_fn(&item.signature)
                && returns_result(&item.signature)
        })
        .collect()
}

/// The fallible public functions of `source`, the file `rel`, without a
/// runnable example, as `rel:line label` problems.
fn fallible_example_problems(rel: &str, source: &str) -> Vec<String> {
    fallible_functions(source)
        .iter()
        .filter(|item| !has_runnable_example(&item.docs))
        .map(|item| format!("{rel}:{} {}", item.line, item_label(item)))
        .collect()
}

/// Every `.rs` file of `crates/sacm/src`, with its path from the repository
/// root and its text, in a stable order.
fn library_sources() -> Vec<(String, String)> {
    let mut files = Vec::new();
    rust_files(&repository_root().join("crates/sacm/src"), &mut files);
    files
        .iter()
        .map(|path| {
            let text =
                fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
            (repository_relative(path), text)
        })
        .collect()
}

#[test]
fn lib_doc_002_the_type_example_rule_needs_a_runnable_examples_section() {
    let runnable = "\
/// A type.
///
/// # Examples
///
/// ```
/// let x = 1;
/// ```
pub struct A;
";
    assert_eq!(
        type_example_problems("m.rs", runnable),
        Vec::<String>::new(),
        "LIB-DOC-002: a type with an `# Examples` section and a plain fence has a runnable example"
    );
    for (what, source) in [
        ("without docs", "pub struct A;\n"),
        (
            "without an `# Examples` section",
            "/// A type.\npub struct A;\n",
        ),
        (
            "with a `rust,ignore` example",
            "/// A type.\n///\n/// # Examples\n///\n/// ```rust,ignore\n/// let x = 1;\n/// ```\npub struct A;\n",
        ),
        (
            "with a `no_run` example",
            "/// A type.\n///\n/// # Examples\n///\n/// ```no_run\n/// let x = 1;\n/// ```\npub struct A;\n",
        ),
        (
            "with a `text` fence as its example",
            "/// A type.\n///\n/// # Examples\n///\n/// ```text\n/// x\n/// ```\npub struct A;\n",
        ),
        (
            "with its only fence under another heading",
            "/// A type.\n///\n/// # Examples\n///\n/// See below.\n///\n/// # Notes\n///\n/// ```\n/// let x = 1;\n/// ```\npub struct A;\n",
        ),
        (
            "with `# Examples` only inside a code fence",
            "/// A type.\n///\n/// ```\n/// # Examples\n/// ```\n///\n/// ```\n/// let x = 1;\n/// ```\npub struct A;\n",
        ),
    ] {
        assert_eq!(
            type_example_problems("m.rs", source),
            [format!("m.rs:{} pub struct A", source.lines().count())],
            "LIB-DOC-002: a type {what} has no runnable example and must be reported"
        );
    }
    for (what, item) in [
        ("a `pub enum`", "pub enum E {"),
        ("a `pub trait`", "pub trait T {"),
    ] {
        assert_eq!(
            type_example_problems("m.rs", &format!("/// A type.\n{item}\n}}\n")).len(),
            1,
            "LIB-DOC-002: {what} without an example must be reported"
        );
    }
    let deeper = "/// A type.\n///\n/// # Examples\n///\n/// ## Reading\n///\n/// ```should_panic\n/// panic!();\n/// ```\npub struct A;\n";
    assert_eq!(
        type_example_problems("m.rs", deeper),
        Vec::<String>::new(),
        "LIB-DOC-002: a `should_panic` fence under a deeper heading of `# Examples` runs"
    );
}

#[test]
fn lib_doc_002_the_type_example_rule_skips_test_support_private_test_and_macro_items() {
    let source = "\
/// Behind the feature.
#[cfg(feature = \"test-support\")]
pub struct Gated;
// Public only under the feature, marked for docs.rs.
/// Why a seam refused.
#[derive(Debug)]
#[cfg_attr(docsrs, doc(cfg(feature = \"test-support\")))]
pub enum SeamError {}
/// Crate-private.
pub(crate) struct Private;
#[cfg(test)]
mod tests {
    pub struct Helper;
}
macro_rules! operation {
    ($name:ident) => {
        pub struct $name;
    };
}
/// Not exempt: the feature is negated.
#[cfg(not(feature = \"test-support\"))]
pub struct Ungated;
";
    assert_eq!(
        type_example_problems("m.rs", source),
        ["m.rs:22 pub struct Ungated"],
        "LIB-DOC-002: test-support types (ADR 0045 D7), crate-private types, types in \
         `#[cfg(test)]` modules and `macro_rules!` bodies are not checked; a type gated on \
         `not(feature = \"test-support\")` is"
    );
}

#[test]
fn lib_doc_002_the_fallible_function_rule_finds_result_returning_functions() {
    let source = "\
/// Fails.
pub fn f() -> Result<(), E> {}
/// Fails, over several lines.
pub fn apply(
    document: &mut Document,
    operation: impl Into<Operation>,
) -> Result<Mutation, EditError> {
}
/// By another path.
pub fn g(x: [u8; 4]) -> io::Result<()>;
/// With a closure bound and a where clause.
pub fn h<F: Fn(u8) -> bool>(f: F) -> Result<u8, E>
where
    F: Copy,
{
}
impl<'d> Batch<'d> {
    /// A method.
    pub fn apply(&mut self, operation: impl Into<Operation>) -> Result<Mutation, BatchError> {
    }
}
";
    let found: Vec<usize> = fallible_functions(source)
        .iter()
        .map(|item| item.line)
        .collect();
    assert_eq!(
        found,
        [2, 4, 10, 12, 19],
        "LIB-DOC-002: `pub fn f() -> Result<(), E>`, a multi-line signature returning `Result`, \
         `io::Result`, a signature with a `Fn() -> bool` bound and a where clause, and a method \
         are fallible"
    );
    assert_eq!(
        fallible_example_problems("m.rs", source),
        [
            "m.rs:2 pub fn f",
            "m.rs:4 pub fn apply",
            "m.rs:10 pub fn g",
            "m.rs:12 pub fn h",
            "m.rs:19 pub fn apply (in `impl<'d> Batch<'d>`)",
        ],
        "LIB-DOC-002: a fallible function without an example is reported at its first line"
    );

    let not_fallible = "\
/// Returns no result.
pub fn a() -> Option<Result<(), E>> {}
/// Returns an iterator of results.
pub fn b() -> impl Iterator<Item = Result<u8, E>> {}
/// Takes a result.
pub fn c(r: Result<u8, E>) -> u8 {}
/// Writes it.
pub fn d() -> fmt::Result {}
/// Crate-private.
pub(crate) fn e() -> Result<(), E> {}
/// Behind the feature.
#[cfg(feature = \"test-support\")]
pub fn s() -> Result<(), E> {}
impl FromStr for X {
    fn from_str(s: &str) -> Result<Self, E> {}
}
";
    assert_eq!(
        fallible_example_problems("m.rs", not_fallible),
        Vec::<String>::new(),
        "LIB-DOC-002: an `Option<Result>`, an iterator of results, a `Result` parameter, \
         `fmt::Result` (no `<`), a crate-private function, a test-support function and a trait \
         impl's `fn` are not checked"
    );

    let with_example = "\
/// Fails.
///
/// # Errors
///
/// Always.
///
/// # Examples
///
/// ```
/// # fn main() {}
/// ```
pub fn f() -> Result<(), E> {}
";
    assert_eq!(
        fallible_example_problems("m.rs", with_example),
        Vec::<String>::new(),
        "LIB-DOC-002: a fallible function with a runnable example is not reported"
    );
}

/// Asserts that more than `min` `what` are found under crates/sacm/src, and
/// that `problems` is empty.
fn assert_examples(what: &str, found: usize, min: usize, problems: &[String]) {
    assert!(
        found > min,
        "LIB-DOC-002: the {what} must be found under crates/sacm/src; found {found}"
    );
    assert!(
        problems.is_empty(),
        "LIB-DOC-002: every {what} must have an `# Examples` section with a runnable code \
         fence (not `ignore` or `no_run`); {} of {found} have none: {problems:#?}",
        problems.len()
    );
}

/// The public structs, enums and traits written out in crates/sacm/src, as
/// [`source_items`] reads them, must each have a runnable example.
///
/// The types the operation macros (`macro_rules!` bodies in `edit/`)
/// generate are not written out, so a source scan cannot see them: the
/// verifier checks those in the rendered docs. Test-support types are exempt
/// (ADR 0045 D7).
#[test]
fn lib_doc_002_every_public_type_has_a_runnable_example() {
    let sources = library_sources();
    let found: usize = sources
        .iter()
        .map(|(_, text)| {
            source_items(text)
                .iter()
                .filter(|item| item.public && !item.test_support && is_public_type(&item.signature))
                .count()
        })
        .sum();
    let problems: Vec<String> = sources
        .iter()
        .flat_map(|(rel, text)| type_example_problems(rel, text))
        .collect();
    assert_examples("public type", found, 50, &problems);
}

/// Every `pub fn` in crates/sacm/src that returns `Result<…>`, as
/// [`fallible_functions`] finds them, must have a runnable example.
/// Test-support functions are exempt (ADR 0045 D7).
#[test]
fn lib_doc_002_every_fallible_public_function_has_a_runnable_example() {
    let sources = library_sources();
    let found: usize = sources
        .iter()
        .map(|(_, text)| fallible_functions(text).len())
        .sum();
    let problems: Vec<String> = sources
        .iter()
        .flat_map(|(rel, text)| fallible_example_problems(rel, text))
        .collect();
    assert_examples("fallible public function", found, 5, &problems);
}

/// The clippy lints that require the `# Errors` and `# Panics` sections,
/// and the group that holds them.
const SECTION_LINTS: [&str; 3] = ["missing_errors_doc", "missing_panics_doc", "pedantic"];

/// Whether the level `value` of a lint in a Cargo.toml, a string or a
/// `{ level = … }` table, is `warn`, `deny` or `forbid`.
fn is_enforced(value: Option<&toml::Value>) -> bool {
    let level = match value {
        Some(toml::Value::String(level)) => Some(level.as_str()),
        Some(toml::Value::Table(table)) => table.get("level").and_then(toml::Value::as_str),
        _ => None,
    };
    matches!(level, Some("warn" | "deny" | "forbid"))
}

/// The problems of the `# Errors` and `# Panics` enforcement: the workspace
/// manifest `workspace` must set clippy `pedantic` to warn or deny; no crate
/// manifest of `manifests` may allow it or the two section lints, and each
/// must inherit the workspace lints; no attribute of `sources` may `allow`
/// or `expect` them.
fn section_lint_problems(
    workspace: &str,
    manifests: &[(String, String)],
    sources: &[(String, String)],
) -> Vec<String> {
    let mut problems = Vec::new();
    match workspace.parse::<toml::Table>() {
        Ok(table) => {
            let clippy = table
                .get("workspace")
                .and_then(|w| w.get("lints"))
                .and_then(|l| l.get("clippy"));
            if !is_enforced(clippy.and_then(|c| c.get("pedantic"))) {
                problems.push(
                    "Cargo.toml: [workspace.lints.clippy] must set `pedantic` to warn or deny"
                        .to_owned(),
                );
            }
            for lint in &SECTION_LINTS[..2] {
                if let Some(value) = clippy.and_then(|c| c.get(*lint)) {
                    if !is_enforced(Some(value)) {
                        problems.push(format!("Cargo.toml: [workspace.lints.clippy] sets {lint}"));
                    }
                }
            }
        }
        Err(e) => problems.push(format!("Cargo.toml: {e}")),
    }
    for (rel, text) in manifests {
        match text.parse::<toml::Table>() {
            Ok(table) => {
                let lints = table.get("lints");
                if lints
                    .and_then(|l| l.get("workspace"))
                    .and_then(toml::Value::as_bool)
                    != Some(true)
                {
                    problems.push(format!("{rel}: [lints] must set `workspace = true`"));
                }
                for lint in SECTION_LINTS {
                    if let Some(value) = lints
                        .and_then(|l| l.get("clippy"))
                        .and_then(|c| c.get(lint))
                    {
                        if !is_enforced(Some(value)) {
                            problems.push(format!("{rel}: [lints.clippy] sets {lint}"));
                        }
                    }
                }
            }
            Err(e) => problems.push(format!("{rel}: {e}")),
        }
    }
    for (rel, text) in sources {
        let lines: Vec<&str> = text.lines().collect();
        let mut i = 0;
        while i < lines.len() {
            let trimmed = lines[i].trim_start();
            if !(trimmed.starts_with("#[") || trimmed.starts_with("#![")) {
                i += 1;
                continue;
            }
            let end = attribute_end(&lines, i);
            let attribute = lines[i..end].join(" ");
            let suppresses = attribute.contains("allow(") || attribute.contains("expect(");
            for lint in SECTION_LINTS {
                if suppresses && attribute.contains(&format!("clippy::{lint}")) {
                    problems.push(format!("{rel}:{} allows clippy::{lint}", i + 1));
                }
            }
            i = end.max(i + 1);
        }
    }
    problems
}

#[test]
fn lib_doc_002_the_section_lint_check_finds_an_allow() {
    let workspace = "[workspace.lints.clippy]\npedantic = { level = \"warn\", priority = -1 }\n";
    let manifest = || {
        vec![(
            "crates/a/Cargo.toml".to_owned(),
            "[lints]\nworkspace = true\n".to_owned(),
        )]
    };
    assert_eq!(
        section_lint_problems(workspace, &manifest(), &[]),
        Vec::<String>::new(),
        "LIB-DOC-002: pedantic at warn, inherited, with no allow, passes"
    );
    assert_eq!(
        section_lint_problems(
            "[workspace.lints.clippy]\npedantic = \"deny\"\n",
            &manifest(),
            &[]
        ),
        Vec::<String>::new(),
        "LIB-DOC-002: pedantic at deny passes"
    );
    for (what, workspace) in [
        (
            "without pedantic",
            "[workspace.lints.clippy]\ntodo = \"warn\"\n",
        ),
        (
            "with pedantic allowed",
            "[workspace.lints.clippy]\npedantic = \"allow\"\n",
        ),
        (
            "with missing_errors_doc allowed",
            "[workspace.lints.clippy]\npedantic = \"warn\"\nmissing_errors_doc = \"allow\"\n",
        ),
    ] {
        assert_eq!(
            section_lint_problems(workspace, &manifest(), &[]).len(),
            1,
            "LIB-DOC-002: a workspace manifest {what} must be reported"
        );
    }
    let allowing = vec![(
        "crates/a/Cargo.toml".to_owned(),
        "[lints.clippy]\nmissing_panics_doc = \"allow\"\n".to_owned(),
    )];
    assert_eq!(
        section_lint_problems(workspace, &allowing, &[]).len(),
        2,
        "LIB-DOC-002: a crate manifest that allows missing_panics_doc, and does not inherit the \
         workspace lints, must be reported twice"
    );
    let sources = vec![(
        "crates/a/src/lib.rs".to_owned(),
        "#![allow(clippy::missing_errors_doc)]\n#[expect(\n    clippy::missing_panics_doc,\n    reason = \"x\"\n)]\nfn f() {}\n#[cfg_attr(test, allow(clippy::pedantic))]\nmod m;\n#[warn(clippy::missing_errors_doc)]\nfn g() {}\n"
            .to_owned(),
    )];
    assert_eq!(
        section_lint_problems(workspace, &manifest(), &sources),
        [
            "crates/a/src/lib.rs:1 allows clippy::missing_errors_doc",
            "crates/a/src/lib.rs:2 allows clippy::missing_panics_doc",
            "crates/a/src/lib.rs:7 allows clippy::pedantic",
        ],
        "LIB-DOC-002: an inner `allow`, a multi-line `expect` and a `cfg_attr` `allow` are \
         reported; a `warn` is not"
    );
}

/// The files with extension `ext` under `dir`, recursively, in a stable order.
fn files_with_extension(dir: &Path, ext: &str, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("read {}: {e}", dir.display()))
        .map(|entry| entry.expect("a directory entry is readable").path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            files_with_extension(&path, ext, out);
        } else if path.extension().is_some_and(|e| e == ext) {
            out.push(path);
        }
    }
}

#[test]
fn lib_doc_002_errors_and_panics_sections_are_enforced_by_clippy() {
    let crates = repository_root().join("crates");
    let read_all = |paths: &[PathBuf]| -> Vec<(String, String)> {
        paths
            .iter()
            .map(|path| {
                let text = fs::read_to_string(path)
                    .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
                (repository_relative(path), text)
            })
            .collect()
    };
    let mut rust = Vec::new();
    files_with_extension(&crates, "rs", &mut rust);
    let mut tomls = Vec::new();
    files_with_extension(&crates, "toml", &mut tomls);
    let manifests: Vec<(String, String)> = read_all(&tomls)
        .into_iter()
        .filter(|(rel, _)| rel.ends_with("/Cargo.toml"))
        .collect();
    assert!(
        manifests.len() >= 2 && rust.len() > 10,
        "LIB-DOC-002: the crate manifests and sources under crates/ must be found; found {} \
         manifests and {} sources",
        manifests.len(),
        rust.len()
    );
    let problems = section_lint_problems(&read("Cargo.toml"), &manifests, &read_all(&rust));
    assert!(
        problems.is_empty(),
        "LIB-DOC-002: clippy pedantic must enforce the `# Errors` and `# Panics` sections \
         (missing_errors_doc, missing_panics_doc): pedantic at warn or deny in \
         [workspace.lints.clippy], inherited by every crate, and never allowed under crates/; \
         found {problems:#?}"
    );
}

// ---------------------------------------------------------------------------
// LIB-DOC-009: no diagram in rustdoc
// ---------------------------------------------------------------------------

/// Whether `line` opens a Mermaid fence: three or more backticks or tildes,
/// then `mermaid` as the info string.
fn opens_mermaid_fence(line: &str) -> bool {
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '`' || c == '~' {
            let run = chars[i..].iter().take_while(|d| **d == c).count();
            if run >= 3 {
                let info: String = chars[i + run..].iter().collect();
                if info
                    .trim_start()
                    .to_ascii_lowercase()
                    .starts_with("mermaid")
                {
                    return true;
                }
            }
            i += run;
        } else {
            i += 1;
        }
    }
    false
}

/// The diagrams in a Rust source, as `line: text` problems.
fn diagrams(source: &str) -> Vec<String> {
    source
        .lines()
        .enumerate()
        .filter(|(_, line)| {
            opens_mermaid_fence(line) || line.to_ascii_lowercase().contains("aquamarine")
        })
        .map(|(i, line)| format!("{}: {}", i + 1, line.trim()))
        .collect()
}

/// The `.rs` files under `dir`, recursively, in a stable order.
fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("read {}: {e}", dir.display()))
        .map(|entry| entry.expect("a directory entry is readable").path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn lib_doc_009_rustdoc_holds_no_diagram() {
    // Controls: the check finds what it looks for.
    for line in [
        "//! ```mermaid",
        "/// ~~~ Mermaid",
        "    /// ````mermaid",
        "#[cfg_attr(doc, aquamarine::aquamarine)]",
    ] {
        assert_eq!(
            diagrams(line).len(),
            1,
            "LIB-DOC-009: the diagram check must report `{line}`"
        );
    }
    for line in [
        "//! ```text",
        "/// A merman.",
        "//! ```",
        "/// Use `mermaid` here.",
    ] {
        assert!(
            diagrams(line).is_empty(),
            "LIB-DOC-009: `{line}` is not a diagram, but was reported"
        );
    }

    let crates = repository_root().join("crates");
    let mut files = Vec::new();
    let mut dirs: Vec<PathBuf> = fs::read_dir(&crates)
        .unwrap_or_else(|e| panic!("read {}: {e}", crates.display()))
        .map(|entry| {
            entry
                .expect("a directory entry is readable")
                .path()
                .join("src")
        })
        .filter(|src| src.is_dir())
        .collect();
    dirs.sort();
    for src in &dirs {
        rust_files(src, &mut files);
    }
    assert!(
        files.len() > 10,
        "LIB-DOC-009: the crates' sources must be found under crates/*/src; found {files:?}"
    );
    let mut problems = Vec::new();
    for path in &files {
        let text =
            fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
        let rel = path
            .strip_prefix(repository_root())
            .unwrap_or(path)
            .display()
            .to_string()
            .replace('\\', "/");
        problems.extend(diagrams(&text).into_iter().map(|p| format!("{rel}:{p}")));
    }
    assert!(
        problems.is_empty(),
        "LIB-DOC-009: rustdoc holds no diagram (no Mermaid fence, no aquamarine); found {problems:#?}"
    );
}

// ---------------------------------------------------------------------------
// Links
// ---------------------------------------------------------------------------

/// The destinations of the links in the Markdown `text`, as a renderer
/// resolves them (inline and reference links; not code spans).
fn link_destinations(text: &str) -> Vec<String> {
    use pulldown_cmark::{Event, Options, Parser, Tag};
    Parser::new_ext(text, Options::ENABLE_TABLES)
        .filter_map(|event| match event {
            Event::Start(Tag::Link { dest_url, .. }) => Some(dest_url.into_string()),
            _ => None,
        })
        .collect()
}

/// `dest` without its `#fragment`.
fn without_fragment(dest: &str) -> &str {
    dest.split_once('#').map_or(dest, |(path, _)| path)
}

/// The repository path a relative link `dest` in the file `doc` names, or
/// `None` for a URL, an absolute path or a path that leaves the repository.
fn resolve_relative(doc: &str, dest: &str) -> Option<String> {
    let path = without_fragment(dest);
    if path.is_empty() || path.contains("://") || path.starts_with('/') {
        return None;
    }
    let mut parts: Vec<String> = Path::new(doc)
        .parent()
        .into_iter()
        .flat_map(|p| p.components())
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    for component in Path::new(path).components() {
        match component {
            Component::Normal(part) => parts.push(part.to_string_lossy().into_owned()),
            Component::ParentDir => {
                parts.pop()?;
            }
            Component::CurDir => {}
            _ => return None,
        }
    }
    Some(parts.join("/"))
}

/// Whether `dest` is a GitHub URL of the repository file `target`, on any
/// branch or revision.
fn is_github_link_to(dest: &str, target: &str) -> bool {
    without_fragment(dest)
        .strip_prefix(GITHUB)
        .and_then(|rest| rest.split_once('/'))
        .is_some_and(|(revision, path)| !revision.is_empty() && path == target)
}

/// Whether the document `doc`, of Markdown `text`, links the repository file
/// `target`: by a GitHub URL, or, if `relative` is allowed, by a relative
/// path from `doc`'s directory.
fn links_to(doc: &str, text: &str, target: &str, relative: bool) -> bool {
    link_destinations(text).iter().any(|dest| {
        is_github_link_to(dest, target)
            || (relative && resolve_relative(doc, dest).as_deref() == Some(target))
    })
}

// ---------------------------------------------------------------------------
// LIB-DOC-009: the skill links the style guide
// ---------------------------------------------------------------------------

#[test]
fn lib_doc_009_the_skill_links_the_style_guide() {
    // Controls: the link check resolves relative links from the file.
    assert!(
        links_to(SKILL, "[guide](../../../docs/style.md)", STYLE_GUIDE, true),
        "LIB-DOC-009: a relative link from the skill to docs/style.md must be found"
    );
    assert!(
        !links_to(SKILL, "[guide](docs/style.md)", STYLE_GUIDE, true),
        "LIB-DOC-009: `docs/style.md` from the skill's directory names another file"
    );
    assert!(
        !links_to(SKILL, "`docs/style.md`", STYLE_GUIDE, true),
        "LIB-DOC-009: a code span is not a link"
    );

    let skill = read(SKILL);
    assert!(
        links_to(SKILL, &skill, STYLE_GUIDE, true),
        "LIB-DOC-009: {SKILL} must link {STYLE_GUIDE}"
    );
    assert!(
        read_optional(STYLE_GUIDE).is_some(),
        "LIB-DOC-009: {STYLE_GUIDE} must exist"
    );
}

// ---------------------------------------------------------------------------
// LIB-DOC-006: the entry documents link the functionality matrix
// ---------------------------------------------------------------------------

#[test]
fn lib_doc_006_the_crate_root_and_readmes_link_the_matrix() {
    // Controls: the matrix link is a GitHub URL on any revision, with or
    // without a fragment, or a relative path that resolves to the matrix.
    for (doc, text) in [
        (
            "crates/sacm/README.md",
            "[m](https://github.com/lasrod/libSACM/blob/main/docs/matrix/functionality-matrix.md)",
        ),
        (
            "crates/sacm/README.md",
            "[m]\n\n[m]: https://github.com/lasrod/libSACM/blob/v0.5.0/docs/matrix/functionality-matrix.md#m7",
        ),
        ("README.md", "[m](docs/matrix/functionality-matrix.md)"),
        (
            "crates/sacm/README.md",
            "[m](../../docs/matrix/functionality-matrix.md)",
        ),
    ] {
        assert!(
            links_to(doc, text, MATRIX, true),
            "LIB-DOC-006: `{text}` in {doc} links the matrix, but was not found"
        );
    }
    for (doc, text) in [
        (
            "crates/sacm/README.md",
            "[m](docs/matrix/functionality-matrix.md)",
        ),
        ("README.md", "`docs/matrix/functionality-matrix.md`"),
        (
            "README.md",
            "[m](https://github.com/lasrod/libSACM/blob/main/docs/matrix/functionality-matrix.toml)",
        ),
        (
            "README.md",
            "[m](https://github.com/someone/else/blob/main/docs/matrix/functionality-matrix.md)",
        ),
        (
            "README.md",
            "[m](https://github.com/lasrod/libSACM/blob//docs/matrix/functionality-matrix.md)",
        ),
    ] {
        assert!(
            !links_to(doc, text, MATRIX, true),
            "LIB-DOC-006: `{text}` in {doc} does not link the matrix, but was found"
        );
    }

    let mut missing = Vec::new();
    // Rustdoc resolves a relative link against the rendered page, not the
    // repository, so the crate root must use the GitHub URL.
    let crate_root = joined(&inner_doc_lines(&read(CRATE_ROOT)));
    if !links_to(CRATE_ROOT, &crate_root, MATRIX, false) {
        missing.push(CRATE_ROOT);
    }
    for readme in READMES {
        if !links_to(readme, &read(readme), MATRIX, true) {
            missing.push(readme);
        }
    }
    assert!(
        missing.is_empty(),
        "LIB-DOC-006: the crate root and each README must link the functionality matrix \
         ({MATRIX}) for current status; missing in {missing:?}"
    );
}

// ---------------------------------------------------------------------------
// LIB-DOC-003, LIB-DOC-005, LIB-DOC-004: the guide, the primer and the examples
// ---------------------------------------------------------------------------

/// The user guide (LIB-DOC-003), shipped with the crate.
const GUIDE: &str = "crates/sacm/GUIDE.md";

/// The SACM primer (LIB-DOC-005), shipped with the crate.
const PRIMER: &str = "crates/sacm/PRIMER.md";

/// The crate README, which may list the examples instead of the guide.
const SACM_README: &str = "crates/sacm/README.md";

/// The manifest of the `sacm` crate.
const SACM_MANIFEST: &str = "crates/sacm/Cargo.toml";

/// The example programs of `crates/sacm/examples/` (LIB-DOC-004), by name.
const EXAMPLES: [&str; 4] = [
    "read_and_validate",
    "build_and_save",
    "edit_in_place",
    "translate_diagnostics",
];

/// The info strings of the code fences of the Markdown `text`, each with the
/// 1-based line number of its opening fence.
fn fence_infos(text: &str) -> Vec<(usize, String)> {
    let mut infos = Vec::new();
    let mut open: Option<(char, usize)> = None;
    for (number, line) in markdown_lines(text) {
        let trimmed = line.trim();
        if let Some(fence_open) = open {
            if closes(trimmed, fence_open) {
                open = None;
            }
            continue;
        }
        if let Some(opened) = fence(trimmed) {
            open = Some(opened);
            infos.push((
                number,
                trimmed.trim_start_matches(opened.0).trim().to_owned(),
            ));
        }
    }
    infos
}

/// Whether a fence's info string keeps rustdoc from running it: an `ignore`
/// token (`ignore` or `ignore-<target>`) or `no_run`.
fn is_not_run(info: &str) -> bool {
    info.split(|c: char| c == ',' || c.is_whitespace())
        .any(|token| token.starts_with("ignore") || token == "no_run")
}

/// Whether the Rust `source` has a `#[cfg(doctest)]` item whose docs are
/// `#[doc = include_str!("../<file>")]`, the attribute and the `cfg` among
/// the same item's attributes and `///` lines.
fn includes_as_doctests(source: &str, file: &str) -> bool {
    let squeezed: Vec<String> = source
        .lines()
        .map(|line| line.chars().filter(|c| !c.is_whitespace()).collect())
        .collect();
    let include = format!("#[doc=include_str!(\"../{file}\")]");
    let of_item = |line: &&String| line.starts_with("#[") || line.starts_with("///");
    squeezed.iter().enumerate().any(|(i, line)| {
        *line == include
            && squeezed[..i]
                .iter()
                .rev()
                .take_while(of_item)
                .chain(squeezed[i + 1..].iter().take_while(of_item))
                .any(|line| line == "#[cfg(doctest)]")
    })
}

/// Whether the manifest `manifest` lists `file` in `package.include`.
fn manifest_includes(manifest: &str, file: &str) -> bool {
    manifest
        .parse::<toml::Table>()
        .ok()
        .and_then(|table| {
            table
                .get("package")
                .and_then(|package| package.get("include"))
                .and_then(toml::Value::as_array)
                .map(|include| include.iter().any(|entry| entry.as_str() == Some(file)))
        })
        .unwrap_or(false)
}

/// Whether the Markdown `text` links a file named `file` by a URL: rustdoc
/// resolves a relative link against the rendered page, not the repository.
fn links_by_url_to_file(text: &str, file: &str) -> bool {
    link_destinations(text).iter().any(|dest| {
        let path = without_fragment(dest);
        path.contains("://") && path.rsplit('/').next() == Some(file)
    })
}

/// The problems of the document `rel` against the four-line rule, or that it
/// is missing or holds next to nothing.
fn short_prose_problems(rel: &str) -> Vec<String> {
    match read_optional(rel) {
        None => vec![format!("{rel}: missing")],
        Some(text) => {
            let lines = markdown_lines(&text);
            let mut problems = long_block_problems(rel, &lines);
            if lines.len() <= 10 {
                problems.push(format!("{rel}: {} lines, next to nothing", lines.len()));
            }
            problems
        }
    }
}

/// The problems of running the Markdown document `rel`, named `file` beside
/// the manifest, as doctests and shipping it: the crate root's
/// `#[cfg(doctest)]` include, the manifest's `include`, no fence rustdoc
/// does not run and, if `needs_rust`, at least one runnable Rust fence.
fn doctest_problems(rel: &str, file: &str, needs_rust: bool) -> Vec<String> {
    let mut problems = Vec::new();
    if !includes_as_doctests(&read(CRATE_ROOT), file) {
        problems.push(format!(
            "{CRATE_ROOT}: no `#[cfg(doctest)]` item with `#[doc = include_str!(\"../{file}\")]`"
        ));
    }
    if !manifest_includes(&read(SACM_MANIFEST), file) {
        problems.push(format!(
            "{SACM_MANIFEST}: `package.include` does not list `{file}`"
        ));
    }
    match read_optional(rel) {
        None => problems.push(format!("{rel}: missing")),
        Some(text) => {
            let infos = fence_infos(&text);
            for (line, info) in &infos {
                if is_not_run(info) {
                    problems.push(format!("{rel}:{line}: fence `{info}` is not run"));
                }
            }
            if needs_rust && !infos.iter().any(|(_, info)| is_runnable_info(info)) {
                problems.push(format!("{rel}: no runnable Rust code fence"));
            }
        }
    }
    problems
}

#[test]
fn lib_doc_003_the_doctest_and_link_readers_find_what_they_look_for() {
    // Controls for the readers the guide and primer checks use.
    let markdown = "\
```rust
let a = 1;
```

- item

  ```rust,no_run
  ```

````text
```ignore
````

~~~
~~~
";
    let infos = fence_infos(markdown);
    assert_eq!(
        infos,
        vec![
            (1, "rust".to_owned()),
            (7, "rust,no_run".to_owned()),
            (10, "text".to_owned()),
            (14, String::new()),
        ],
        "LIB-DOC-003: the fence reader must find each fence's info string, not a fence \
         inside another"
    );
    for info in ["ignore", "rust,ignore", "ignore-wasm32", "rust,no_run"] {
        assert!(
            is_not_run(info),
            "LIB-DOC-003: a fence `{info}` is not run, but was taken as run"
        );
    }
    for info in ["", "rust", "text", "rust,should_panic"] {
        assert!(
            !is_not_run(info),
            "LIB-DOC-003: a fence `{info}` is run (or is not Rust), but was taken as not run"
        );
    }

    let source = "\
/// The guide.
#[cfg(doctest)]
#[doc = include_str!(\"../GUIDE.md\")]
struct GuideDoctests;

#[doc = include_str!(\"../PRIMER.md\")]
struct Primer;
";
    assert!(
        includes_as_doctests(source, "GUIDE.md"),
        "LIB-DOC-003: a `#[cfg(doctest)]` include of the guide must be found"
    );
    assert!(
        !includes_as_doctests(source, "PRIMER.md"),
        "LIB-DOC-005: an include without `#[cfg(doctest)]` on its item is not the doctest item"
    );
    assert!(
        !includes_as_doctests(
            "#[cfg(doctest)]\nstruct A;\n#[doc = include_str!(\"../GUIDE.md\")]\nstruct B;\n",
            "GUIDE.md"
        ),
        "LIB-DOC-003: a `#[cfg(doctest)]` on another item does not count"
    );

    assert!(
        manifest_includes(
            "[package]\ninclude = [\"src/**\", \"GUIDE.md\"]\n",
            "GUIDE.md"
        ),
        "LIB-DOC-003: a listed file must be found in `package.include`"
    );
    assert!(
        !manifest_includes("[package]\ninclude = [\"src/**\"]\n", "GUIDE.md"),
        "LIB-DOC-003: an unlisted file was found in `package.include`"
    );

    for text in [
        "[g](https://github.com/lasrod/libSACM/blob/main/crates/sacm/GUIDE.md)",
        "[g]\n\n[g]: https://docs.rs/crate/sacm/latest/source/GUIDE.md#editing",
    ] {
        assert!(
            links_by_url_to_file(text, "GUIDE.md"),
            "LIB-DOC-003: `{text}` links the guide, but was not found"
        );
    }
    for text in [
        "[g](GUIDE.md)",
        "`https://github.com/lasrod/libSACM/blob/main/crates/sacm/GUIDE.md`",
        "[g](https://github.com/lasrod/libSACM/blob/main/crates/sacm/MYGUIDE.md)",
    ] {
        assert!(
            !links_by_url_to_file(text, "GUIDE.md"),
            "LIB-DOC-003: `{text}` does not link the guide from rustdoc, but was found"
        );
    }
}

#[test]
fn lib_doc_003_no_paragraph_or_list_item_in_the_guide_runs_over_four_lines() {
    let problems = short_prose_problems(GUIDE);
    assert!(
        problems.is_empty(),
        "LIB-DOC-003: {GUIDE} must exist, and no prose paragraph or list item in it may run \
         over {MAX_LINES} display lines of {DISPLAY_WIDTH} characters (docs/style.md). \
         Found: {problems:#?}"
    );
}

#[test]
fn lib_doc_005_no_paragraph_or_list_item_in_the_primer_runs_over_four_lines() {
    let problems = short_prose_problems(PRIMER);
    assert!(
        problems.is_empty(),
        "LIB-DOC-005: {PRIMER} must exist, and no prose paragraph or list item in it may run \
         over {MAX_LINES} display lines of {DISPLAY_WIDTH} characters (docs/style.md). \
         Found: {problems:#?}"
    );
}

#[test]
fn lib_doc_003_the_guide_runs_as_doctests_and_ships_with_the_crate() {
    let problems = doctest_problems(GUIDE, "GUIDE.md", true);
    assert!(
        problems.is_empty(),
        "LIB-DOC-003: the guide's code must run as doctests (a `#[cfg(doctest)]` include in \
         the crate root, runnable Rust fences, none `ignore` or `no_run`) and the guide must \
         ship with the crate. Found: {problems:#?}"
    );
}

#[test]
fn lib_doc_005_the_primer_runs_as_doctests_and_ships_with_the_crate() {
    let problems = doctest_problems(PRIMER, "PRIMER.md", false);
    assert!(
        problems.is_empty(),
        "LIB-DOC-005: the primer's code, if any, must run as doctests (a `#[cfg(doctest)]` \
         include in the crate root, no fence `ignore` or `no_run`) and the primer must ship \
         with the crate. Found: {problems:#?}"
    );
}

#[test]
fn lib_doc_003_the_crate_root_links_the_guide_and_primer() {
    let crate_root = joined(&inner_doc_lines(&read(CRATE_ROOT)));
    let missing: Vec<&str> = ["GUIDE.md", "PRIMER.md"]
        .into_iter()
        .filter(|file| !links_by_url_to_file(&crate_root, file))
        .collect();
    assert!(
        missing.is_empty(),
        "LIB-DOC-003: the crate root's `//!` docs must link the guide and the primer by URL \
         (rustdoc resolves a relative link against the rendered page); missing {missing:?}"
    );
}

#[test]
fn lib_doc_004_the_guide_or_readme_lists_every_example() {
    let texts: Vec<String> = [GUIDE, SACM_README]
        .into_iter()
        .filter_map(read_optional)
        .collect();
    let missing: Vec<&str> = EXAMPLES
        .into_iter()
        .filter(|name| !texts.iter().any(|text| text.contains(name)))
        .collect();
    assert!(
        missing.is_empty(),
        "LIB-DOC-004: {GUIDE} or {SACM_README} must name each example of \
         crates/sacm/examples/; not named: {missing:?}"
    );
}
