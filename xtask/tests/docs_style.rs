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
            in_test: tests.iter().any(|(from, to)| start > *from && start < *to),
            in_trait_impl: impls
                .iter()
                .filter(|(from, to, _)| start > *from && start < *to)
                .max_by_key(|(from, _, _)| *from)
                .is_some_and(|(_, _, is_trait)| *is_trait),
        });
    }
    blocks
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

/// The files LIB-DOC-012 checks, each with its path from the repository root
/// and its text, in a stable order.
fn item_doc_sources() -> Vec<(String, String)> {
    let src = repository_root().join("crates/sacm/src");
    let mut files = Vec::new();
    for module in ITEM_DOC_MODULES {
        let file = src.join(format!("{module}.rs"));
        assert!(
            file.is_file(),
            "LIB-DOC-012: {} must exist",
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

#[test]
fn lib_doc_012_no_paragraph_or_list_item_in_the_item_docs_runs_over_four_lines() {
    // Controls: only the docs of public items, outside test modules, count.
    check_public_item_controls();

    let sources = item_doc_sources();
    let blocks: usize = sources
        .iter()
        .map(|(_, text)| public_doc_blocks(text).len())
        .sum();
    assert!(
        blocks > 100,
        "LIB-DOC-012: the public item docs must be found in the `///` blocks of the \
         {ITEM_DOC_MODULES:?} modules; found {blocks} blocks"
    );
    let problems: Vec<String> = sources
        .iter()
        .flat_map(|(rel, text)| long_item_doc_problems(rel, text))
        .collect();
    assert!(
        problems.is_empty(),
        "LIB-DOC-012: no prose paragraph or list item in the `///` docs of the \
         {ITEM_DOC_MODULES:?} modules may run over {MAX_LINES} display lines of \
         {DISPLAY_WIDTH} characters (docs/style.md); found {}: {problems:#?}",
        problems.len()
    );
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

    let sources = item_doc_sources();
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
        "LIB-DOC-012: the item summaries must be found in the `///` blocks of the \
         {ITEM_DOC_MODULES:?} modules; found {summaries}"
    );
    let problems: Vec<String> = sources
        .iter()
        .flat_map(|(rel, text)| long_summary_problems(rel, text))
        .collect();
    assert!(
        problems.is_empty(),
        "LIB-DOC-012: no item summary (the first paragraph of an item's `///` docs) in the \
         {ITEM_DOC_MODULES:?} modules may run over {MAX_SUMMARY_LINES} display lines of \
         {DISPLAY_WIDTH} characters (docs/style.md); found {}: {problems:#?}",
        problems.len()
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
