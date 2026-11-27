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
