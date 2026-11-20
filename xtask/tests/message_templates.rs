//! Repository checks for the message templates (LIB-API-017, acceptance 6;
//! LIB-API-018, acceptances 4 and 7; ADR 0030 amendment 3).
//!
//! The crate's templates are the entries of the `templates!` table in
//! `crates/sacm/src/diagnostic/template.rs`, one per entry, each on its own
//! lines and ending with `;`:
//!
//! ```text
//! MULT_001_MISSING = MULTIPLICITY_VIOLATION, Diagnostic, "SACM-MULT-001/missing",
//!     [Text("class"), Text("id"), Text("role"), Text("type"), Text("multiplicity")],
//!     "{class} {id:?} has no {role} ({role}:{type}[{multiplicity}])";
//! ```
//!
//! The const name, the code constant, the carrier, the id, the arguments with
//! their kinds, and the English pattern, a plain literal whose only escapes
//! are `\"` and `\\`. The carrier is `Diagnostic`, `Read("<Variant>")` for a
//! `ReadError` variant, `Write("<Variant>")` for a `WriteError` variant, or
//! `Unread`, which stands for `ReadError::Unsupported`: LIB-API-011's check
//! (a) reports the word `Unsupported` anywhere in `crates/sacm/src` outside
//! `xmi/refusal.rs` and the declaration in `xmi.rs`, a string literal
//! included, so the table cannot name that variant by its name.
//!
//! Three checks:
//!
//! - (a) The `## Message templates` table of `docs/sacm/diagnostics.md`,
//!   `| Template | Carrier | Arguments | English |`, lists exactly the
//!   crate's templates, sorted by id: per template its id in a code span, its
//!   carrier (`Diagnostic`, `ReadError::<Variant>` or
//!   `WriteError::<Variant>`), its arguments as `` `name` `` for Text and
//!   `` `name` (count) `` for Count, separated by `, ` (`—` for none), and its
//!   English in a code span, with `|` written `\|`.
//! - (b) Every template const is named, outside comments, literals and
//!   `#[cfg(test)]` items, in another file of `crates/sacm/src`: no template
//!   is left unused.
//! - (c) The crate builds every `Diagnostic` from a template (LIB-API-018,
//!   acceptance 4): outside comments, literals and `#[cfg(test)]` items, no
//!   file of `crates/sacm/src` but `diagnostic.rs` names `Diagnostic::new(`
//!   or `Diagnostic::create(`, and `diagnostic.rs` has no `fn create(`. Each
//!   occurrence is one `[built-without-template]` problem.
//!
//! As in `interim_refusals.rs`, each check is a function that returns the
//! problems it finds, each starting with a stable code in brackets. The
//! checks run on the real files, which must pass, and on synthetic inputs,
//! each of which must give its codes. The fixture source text is written as
//! single-line string literals with `\n` escapes (see `scanner.rs`).

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

const ROW: &str = "LIB-API-017";

/// The source directory of the `sacm` crate, relative to the repository root.
const SACM_SRC: &str = "crates/sacm/src";

/// The template table, relative to [`SACM_SRC`].
const TEMPLATES: &str = "diagnostic/template.rs";

/// The catalog, relative to the repository root.
const CATALOG: &str = "docs/sacm/diagnostics.md";

/// The heading of the catalog's template table.
const HEADING: &str = "## Message templates";

/// The number of refusal templates: the 103 of slice 0063 (acceptance 4),
/// less `SACM-XMI-014/expression-lang-string`, which slice 0065 removes
/// (SACM23-BASE-009), and the four of unread kinds and ends, which slice
/// 0067 removes (ADR 0035 amendment 1); and the eleven slice 0072 adds, less
/// `SACM-XMI-002/no-namespace`, which it removes (LIB-SEC-003,
/// LIB-COMPAT-004, LIB-COMPAT-016; ADR 0041): Read 75, Unread 27, Write 7;
/// and slice 0073a removes the seven Unread templates of SACM-XMI-014 and
/// adds `SACM-XMI-001/xmi-namespace-root` (Read) and
/// `SACM-XMI-003/wrapper-element` (Unread) (SACM23-XMI-019, ADR 0042): Read
/// 76, Unread 21, Write 7.
const REFUSAL_TEMPLATES: usize = 104;

/// The row of the diagnostic templates (slice 0064).
const ROW_018: &str = "LIB-API-018";

/// The number of diagnostic templates (LIB-API-018, acceptance 7): 38 from
/// validation and 71 from the reader (slice 0067 removes the four kept forms
/// of unread kinds and ends; slice 0068 adds eight to validation,
/// SACM23-VAL-013, -014, -015 and -023; slice 0072 adds three to the
/// reader, `SACM-XMI-008/other-revision`, `/occurence` and
/// `SACM-XMI-002/sacm-like`, LIB-COMPAT-004); slice 0073a removes the eight
/// kept forms of SACM-XMI-014 and adds seven: `SACM-XMI-003/wrapper-element-kept`,
/// the four `SACM-XMI-003/proxy-*` forms, `SACM-REF-001/metadata-dangling`
/// and `SACM-ID-002/metadata-id-not-an-ncname` (SACM23-XMI-019,
/// SACM23-XMI-030; ADR 0042).
const DIAGNOSTIC_TEMPLATES: usize = 108;

/// The file of `Diagnostic` itself, relative to [`SACM_SRC`], where a
/// diagnostic is built without a template only by `test-support`'s `new`.
const DIAGNOSTIC_SOURCE: &str = "diagnostic.rs";

fn repository_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap_or_else(|| panic!("{ROW}: setup: no repository root"))
}

fn read(rel: &str) -> String {
    fs::read_to_string(repository_root().join(rel))
        .unwrap_or_else(|e| panic!("{ROW}: setup: read {rel}: {e}"))
}

/// Every `.rs` file under [`SACM_SRC`], by its path relative to it with `/`
/// separators, with its text.
fn sacm_sources() -> BTreeMap<String, String> {
    fn walk(dir: &Path, prefix: &str, out: &mut BTreeMap<String, String>) {
        let entries =
            fs::read_dir(dir).unwrap_or_else(|e| panic!("{ROW}: list {}: {e}", dir.display()));
        for entry in entries {
            let entry = entry.unwrap_or_else(|e| panic!("{ROW}: list {}: {e}", dir.display()));
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            let rel = if prefix.is_empty() {
                name
            } else {
                format!("{prefix}/{name}")
            };
            if path.is_dir() {
                walk(&path, &rel, out);
            } else if Path::new(&rel)
                .extension()
                .is_some_and(|extension| extension == "rs")
            {
                let text = fs::read_to_string(&path)
                    .unwrap_or_else(|e| panic!("{ROW}: read {}: {e}", path.display()));
                out.insert(rel, text);
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(&repository_root().join(SACM_SRC), "", &mut out);
    out
}

/// A problem: `[code] text`.
fn problem(code: &str, text: impl std::fmt::Display) -> String {
    format!("[{code}] {text}")
}

/// The codes of `problems`, sorted, with repeats. A problem without a
/// `[code] ` prefix shows as `<uncoded>`, so it can never match a code.
fn problem_codes(problems: &[String]) -> Vec<String> {
    let mut codes: Vec<String> = problems
        .iter()
        .map(|p| {
            p.strip_prefix('[')
                .and_then(|rest| rest.split_once("] "))
                .map_or_else(|| "<uncoded>".to_owned(), |(code, _)| code.to_owned())
        })
        .collect();
    codes.sort();
    codes
}

/// Asserts that `problems` are exactly `codes`, in any order.
fn assert_codes(what: &str, problems: &[String], codes: &[&str]) {
    let mut expected: Vec<String> = codes.iter().map(|&code| code.to_owned()).collect();
    expected.sort();
    assert_eq!(
        problem_codes(problems),
        expected,
        "{ROW}: {what}: expected {codes:?}, got {problems:#?}"
    );
}

// ---------------------------------------------------------------------------
// A small Rust lexer: comments, literals and `#[cfg(test)]` items (as in
// `interim_refusals.rs`)
// ---------------------------------------------------------------------------

fn is_ident_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// Blanks `bytes[from..to]` in `out`, keeping line breaks.
fn blank(out: &mut [u8], from: usize, to: usize) {
    for byte in &mut out[from..to] {
        if *byte != b'\n' {
            *byte = b' ';
        }
    }
}

/// The end of the string literal whose opening `"` is at `open`: the index
/// after its closing `"`.
fn string_end(bytes: &[u8], open: usize) -> usize {
    let mut i = open + 1;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2,
            b'"' => return i + 1,
            _ => i += 1,
        }
    }
    bytes.len()
}

/// The end of the raw string literal whose `#`s or `"` start at `from` (just
/// after the `r`), or `None` if no raw string starts there.
fn raw_string_end(bytes: &[u8], from: usize) -> Option<usize> {
    let hashes = bytes[from..].iter().take_while(|&&b| b == b'#').count();
    let open = from + hashes;
    if bytes.get(open) != Some(&b'"') {
        return None;
    }
    let mut close = Vec::with_capacity(hashes + 1);
    close.push(b'"');
    close.extend(std::iter::repeat_n(b'#', hashes));
    let body = open + 1;
    let end = bytes[body..]
        .windows(close.len())
        .position(|window| window == close.as_slice())
        .map_or(bytes.len(), |at| body + at + close.len());
    Some(end)
}

/// The end of the char literal whose `'` is at `open`, or `None` if it opens
/// a lifetime or a label.
fn char_end(bytes: &[u8], open: usize) -> Option<usize> {
    let next = *bytes.get(open + 1)?;
    if next == b'\\' {
        let close = bytes[open + 2..].iter().position(|&b| b == b'\'')?;
        return Some(open + 2 + close + 1);
    }
    let width = match next {
        0x00..=0x7F => 1,
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        _ => 4,
    };
    (bytes.get(open + 1 + width) == Some(&b'\'')).then_some(open + 2 + width)
}

/// Two copies of `source`, byte for byte as long: the first with every
/// comment blanked, the second with every literal blanked too.
fn masks(source: &str) -> (Vec<u8>, Vec<u8>) {
    let bytes = source.as_bytes();
    let mut code = bytes.to_vec();
    let mut skeleton = bytes.to_vec();
    let mut i = 0;
    while i < bytes.len() {
        let previous_is_ident = i > 0 && is_ident_byte(bytes[i - 1]);
        let rest = &bytes[i..];
        if rest.starts_with(b"//") {
            let end = rest
                .iter()
                .position(|&b| b == b'\n')
                .map_or(bytes.len(), |at| i + at);
            blank(&mut code, i, end);
            blank(&mut skeleton, i, end);
            i = end;
        } else if rest.starts_with(b"/*") {
            let mut depth = 0usize;
            let mut j = i;
            while j < bytes.len() {
                if bytes[j..].starts_with(b"/*") {
                    depth += 1;
                    j += 2;
                } else if bytes[j..].starts_with(b"*/") {
                    depth -= 1;
                    j += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    j += 1;
                }
            }
            blank(&mut code, i, j);
            blank(&mut skeleton, i, j);
            i = j;
        } else if !previous_is_ident && (rest.starts_with(b"r") || rest.starts_with(b"br")) {
            let after_r = if rest.starts_with(b"br") {
                i + 2
            } else {
                i + 1
            };
            if let Some(end) = raw_string_end(bytes, after_r) {
                blank(&mut skeleton, i, end);
                i = end;
            } else {
                i += 1;
            }
        } else if bytes[i] == b'"' {
            let end = string_end(bytes, i);
            blank(&mut skeleton, i, end);
            i = end;
        } else if bytes[i] == b'\'' {
            if let Some(end) = char_end(bytes, i) {
                blank(&mut skeleton, i, end);
                i = end;
            } else {
                i += 1;
            }
        } else {
            i += 1;
        }
    }
    (code, skeleton)
}

/// The end of the item that starts at `from` in `skeleton`: after the first
/// `;` outside brackets, or after the `}` that closes the first `{` outside
/// brackets.
fn item_end(skeleton: &[u8], from: usize) -> usize {
    let mut depth = 0usize;
    for (offset, &byte) in skeleton[from..].iter().enumerate() {
        match byte {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' => depth = depth.saturating_sub(1),
            b'}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return from + offset + 1;
                }
            }
            b';' if depth == 0 => return from + offset + 1,
            _ => {}
        }
    }
    skeleton.len()
}

/// `source` with comments and every `#[cfg(test)]` item blanked; string
/// literals are kept.
fn code_outside_tests(source: &str) -> String {
    const CFG_TEST: &[u8] = b"#[cfg(test)]";
    let (mut code, skeleton) = masks(source);
    let mut i = 0;
    while let Some(at) = skeleton[i..]
        .windows(CFG_TEST.len())
        .position(|window| window == CFG_TEST)
    {
        let start = i + at;
        let end = item_end(&skeleton, start + CFG_TEST.len());
        blank(&mut code, start, end);
        i = end;
    }
    String::from_utf8(code).unwrap_or_else(|_| panic!("{ROW}: blanking keeps UTF-8"))
}

/// The offsets of `token` in `code` as a whole word.
fn word_offsets(code: &str, token: &str) -> Vec<usize> {
    let bytes = code.as_bytes();
    code.match_indices(token)
        .map(|(at, _)| at)
        .filter(|&at| {
            let before = at.checked_sub(1).map(|b| bytes[b]);
            let after = bytes.get(at + token.len()).copied();
            !before.is_some_and(is_ident_byte) && !after.is_some_and(is_ident_byte)
        })
        .collect()
}

/// The index after the bracket that closes the one at `open` in `skeleton`.
fn closing(skeleton: &[u8], open: usize) -> usize {
    let mut depth = 0usize;
    for (offset, &byte) in skeleton[open..].iter().enumerate() {
        match byte {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return open + offset + 1;
                }
            }
            _ => {}
        }
    }
    skeleton.len()
}

// ---------------------------------------------------------------------------
// Templates, as the source and the catalog give them
// ---------------------------------------------------------------------------

/// An argument's kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Text,
    Count,
}

/// One template.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Template {
    /// The const name, in the source; empty for a catalog row.
    name: String,
    /// The id, `<CODE>/<form>`.
    id: String,
    /// `Diagnostic`, `ReadError::<Variant>` or `WriteError::<Variant>`.
    carrier: String,
    arguments: Vec<(String, Kind)>,
    english: String,
}

/// `text`, a Rust string literal with its quotes, unescaped. Only `\"` and
/// `\\` are allowed; `None` for anything else.
fn unquote(text: &str) -> Option<String> {
    let body = text.trim().strip_prefix('"')?.strip_suffix('"')?;
    let mut out = String::new();
    let mut chars = body.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => match chars.next()? {
                '"' => out.push('"'),
                '\\' => out.push('\\'),
                _ => return None,
            },
            '"' => return None,
            c => out.push(c),
        }
    }
    Some(out)
}

/// `text` split at the commas outside brackets and literals, each part
/// trimmed. `skeleton` is `text` with its literals blanked.
fn top_level_parts<'t>(text: &'t str, skeleton: &[u8]) -> Vec<&'t str> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut start = 0;
    for (at, &byte) in skeleton.iter().enumerate() {
        match byte {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth = depth.saturating_sub(1),
            b',' if depth == 0 => {
                parts.push(text[start..at].trim());
                start = at + 1;
            }
            _ => {}
        }
    }
    let last = text[start..].trim();
    if !last.is_empty() {
        parts.push(last);
    }
    parts
}

/// The carrier a source entry's carrier token names.
fn carrier_of(token: &str) -> Option<String> {
    if token == "Diagnostic" {
        return Some("Diagnostic".to_owned());
    }
    if token == "Unread" {
        return Some("ReadError::Unsupported".to_owned());
    }
    for (prefix, owner) in [("Read(", "ReadError"), ("Write(", "WriteError")] {
        if let Some(inner) = token.strip_prefix(prefix).and_then(|t| t.strip_suffix(')')) {
            let variant = unquote(inner)?;
            let is_name = !variant.is_empty() && variant.bytes().all(is_ident_byte);
            return is_name.then(|| format!("{owner}::{variant}"));
        }
    }
    None
}

/// The arguments of a source entry, `[Text("a"), Count("b")]`.
fn source_arguments(token: &str) -> Option<Vec<(String, Kind)>> {
    let inner = token.strip_prefix('[')?.strip_suffix(']')?;
    let (_, skeleton) = masks(inner);
    top_level_parts(inner, &skeleton)
        .into_iter()
        .map(|part| {
            let (kind, rest) = if let Some(rest) = part.strip_prefix("Text(") {
                (Kind::Text, rest)
            } else {
                (Kind::Count, part.strip_prefix("Count(")?)
            };
            Some((unquote(rest.strip_suffix(')')?)?, kind))
        })
        .collect()
}

/// The template of one source entry, from its parts.
fn source_entry(
    name: &str,
    carrier: &str,
    id: &str,
    arguments: &str,
    english: &str,
) -> Option<Template> {
    Some(Template {
        name: name.to_owned(),
        id: unquote(id)?,
        carrier: carrier_of(carrier)?,
        arguments: source_arguments(arguments)?,
        english: unquote(english)?,
    })
}

/// The templates of `source`, the text of `template.rs`, from its
/// `templates!` table, with the problems of entries that do not parse.
fn source_templates(source: &str) -> (Vec<Template>, Vec<String>) {
    let mut problems = Vec::new();
    let (code, skeleton) = masks(source);
    let code = String::from_utf8_lossy(&code).into_owned();
    let skeleton_text = String::from_utf8_lossy(&skeleton).into_owned();
    let Some(at) = skeleton_text.find("templates!") else {
        problems.push(problem(
            "no-templates-table",
            format!("{SACM_SRC}/{TEMPLATES} has no templates! table"),
        ));
        return (Vec::new(), problems);
    };
    let Some(open) = skeleton_text[at..].find(['{', '(', '[']).map(|o| at + o) else {
        problems.push(problem("no-templates-table", "templates! has no body"));
        return (Vec::new(), problems);
    };
    let close = closing(&skeleton, open);
    let body_start = open + 1;
    let body_end = close.saturating_sub(1).max(body_start);
    let body = &code[body_start..body_end];
    let body_skeleton = &skeleton[body_start..body_end];
    let mut templates = Vec::new();
    let mut start = 0;
    let mut depth = 0usize;
    let mut entries = Vec::new();
    for (offset, &byte) in body_skeleton.iter().enumerate() {
        match byte {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth = depth.saturating_sub(1),
            b';' if depth == 0 => {
                entries.push((start, offset));
                start = offset + 1;
            }
            _ => {}
        }
    }
    if !body[start..].trim().is_empty() {
        problems.push(problem(
            "malformed-entry",
            format!("an entry does not end with ';': {:?}", body[start..].trim()),
        ));
    }
    for (from, to) in entries {
        let entry = &body[from..to];
        let entry_skeleton = &body_skeleton[from..to];
        let Some(eq) = entry_skeleton.iter().position(|&b| b == b'=') else {
            problems.push(problem(
                "malformed-entry",
                format!("no '=': {:?}", entry.trim()),
            ));
            continue;
        };
        let name = entry[..eq].trim().to_owned();
        let rest = &entry[eq + 1..];
        let parts = top_level_parts(rest, &entry_skeleton[eq + 1..]);
        let parsed = match parts.as_slice() {
            [_code, carrier, id, arguments, english] => {
                source_entry(&name, carrier, id, arguments, english)
            }
            _ => None,
        };
        match parsed {
            Some(template) if !name.is_empty() && name.bytes().all(is_ident_byte) => {
                templates.push(template);
            }
            _ => problems.push(problem(
                "malformed-entry",
                format!("{name}: not `NAME = CODE, Carrier, \"id\", [arguments], \"English\"`"),
            )),
        }
    }
    (templates, problems)
}

/// The cells of the table row `line`, split at the `|`s that are not
/// written `\|`, each trimmed, with `\|` read as `|`.
fn cells(line: &str) -> Vec<String> {
    let inner = line.trim().trim_start_matches('|');
    let inner = inner.strip_suffix('|').unwrap_or(inner);
    let mut out = Vec::new();
    let mut cell = String::new();
    let mut chars = inner.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&'|') => {
                cell.push('|');
                chars.next();
            }
            '|' => out.push(std::mem::take(&mut cell).trim().to_owned()),
            c => cell.push(c),
        }
    }
    out.push(cell.trim().to_owned());
    out
}

/// `cell` without one pair of enclosing backticks, if it has them.
fn code_span(cell: &str) -> Option<&str> {
    cell.strip_prefix('`')?.strip_suffix('`')
}

/// The arguments of a catalog row: `` `a`, `b` (count) ``, or `—`.
fn catalog_arguments(cell: &str) -> Option<Vec<(String, Kind)>> {
    if matches!(cell, "" | "—" | "-") {
        return Some(Vec::new());
    }
    cell.split(", ")
        .map(|item| {
            let (name, kind) = match item.strip_suffix(" (count)") {
                Some(name) => (name, Kind::Count),
                None => (item, Kind::Text),
            };
            Some((code_span(name)?.to_owned(), kind))
        })
        .collect()
}

/// The template of one catalog row, from its cells.
fn catalog_row(id: &str, carrier: &str, arguments: &str, english: &str) -> Option<Template> {
    Some(Template {
        name: String::new(),
        id: code_span(id)?.to_owned(),
        carrier: code_span(carrier).unwrap_or(carrier).to_owned(),
        arguments: catalog_arguments(arguments)?,
        english: code_span(english)?.to_owned(),
    })
}

/// The rows of the catalog's `## Message templates` table, in order, with
/// the problems of rows that do not parse.
fn catalog_templates(catalog: &str) -> (Vec<Template>, Vec<String>) {
    let mut problems = Vec::new();
    let Some(start) = catalog.lines().position(|line| line.trim_end() == HEADING) else {
        problems.push(problem(
            "no-table",
            format!("{CATALOG} has no {HEADING:?} section"),
        ));
        return (Vec::new(), problems);
    };
    let mut rows = Vec::new();
    for line in catalog
        .lines()
        .skip(start + 1)
        .take_while(|line| !line.starts_with("## "))
        .filter(|line| line.trim_start().starts_with('|'))
    {
        let cells = cells(line);
        let is_header = cells.first().is_some_and(|cell| cell == "Template");
        let is_rule = cells
            .iter()
            .all(|cell| !cell.is_empty() && cell.chars().all(|c| matches!(c, '-' | ':')));
        if is_header || is_rule {
            continue;
        }
        let parsed = match cells.as_slice() {
            [id, carrier, arguments, english] => catalog_row(id, carrier, arguments, english),
            _ => None,
        };
        match parsed {
            Some(row) => rows.push(row),
            None => problems.push(problem("malformed-row", format!("{line:?}"))),
        }
    }
    (rows, problems)
}

/// The problems of the arguments `listed` for `id` against the crate's
/// `declared`: one per missing or extra name, one per kind that differs,
/// and one for the same names in another order.
fn argument_problems(
    id: &str,
    declared: &[(String, Kind)],
    listed: &[(String, Kind)],
) -> Vec<String> {
    let mut problems = Vec::new();
    let names = |list: &[(String, Kind)]| -> Vec<String> {
        list.iter().map(|(name, _)| name.clone()).collect()
    };
    let declared_names = names(declared);
    let listed_names = names(listed);
    for name in &declared_names {
        if !listed_names.contains(name) {
            problems.push(problem(
                "missing-argument",
                format!("{id}: the catalog does not list the argument {name:?}"),
            ));
        }
    }
    for name in &listed_names {
        if !declared_names.contains(name) {
            problems.push(problem(
                "extra-argument",
                format!("{id}: the catalog lists the argument {name:?}, which the crate does not declare"),
            ));
        }
    }
    for (name, kind) in declared {
        if let Some((_, other)) = listed.iter().find(|(listed, _)| listed == name) {
            if other != kind {
                problems.push(problem(
                    "argument-kind",
                    format!("{id}: {name:?} is {kind:?} in the crate and {other:?} in the catalog"),
                ));
            }
        }
    }
    let common = |a: &[String], b: &[String]| -> Vec<String> {
        a.iter().filter(|name| b.contains(name)).cloned().collect()
    };
    if common(&declared_names, &listed_names) != common(&listed_names, &declared_names) {
        problems.push(problem(
            "argument-order",
            format!(
                "{id}: the catalog lists {listed_names:?}, the crate declares {declared_names:?}"
            ),
        ));
    }
    problems
}

/// Check (a): the catalog's table lists exactly the templates of `source`,
/// sorted by id.
fn catalog_problems(source: Option<&str>, catalog: &str) -> Vec<String> {
    let mut problems = Vec::new();
    let Some(source) = source else {
        problems.push(problem(
            "no-template-file",
            format!("{SACM_SRC}/{TEMPLATES} does not exist"),
        ));
        return problems;
    };
    let (declared, source_problems) = source_templates(source);
    let (listed, catalog_problems) = catalog_templates(catalog);
    problems.extend(source_problems);
    problems.extend(catalog_problems);
    let mut seen = BTreeSet::new();
    for template in &declared {
        if !seen.insert(template.id.clone()) {
            problems.push(problem(
                "duplicate-template",
                format!("{SACM_SRC}/{TEMPLATES} declares {} twice", template.id),
            ));
        }
    }
    let mut seen = BTreeSet::new();
    for row in &listed {
        if !seen.insert(row.id.clone()) {
            problems.push(problem(
                "duplicate-row",
                format!("{CATALOG} lists {} twice", row.id),
            ));
        }
    }
    let ids: Vec<&str> = listed.iter().map(|row| row.id.as_str()).collect();
    let mut sorted = ids.clone();
    sorted.sort_unstable();
    if ids != sorted {
        problems.push(problem(
            "row-order",
            format!("the rows of {HEADING:?} are not sorted by id"),
        ));
    }
    for template in &declared {
        let Some(row) = listed.iter().find(|row| row.id == template.id) else {
            problems.push(problem(
                "missing-row",
                format!("{CATALOG} has no row for {}", template.id),
            ));
            continue;
        };
        if row.carrier != template.carrier {
            problems.push(problem(
                "carrier",
                format!(
                    "{}: the catalog's carrier is {:?}, the crate's {:?}",
                    template.id, row.carrier, template.carrier
                ),
            ));
        }
        problems.extend(argument_problems(
            &template.id,
            &template.arguments,
            &row.arguments,
        ));
        if row.english != template.english {
            problems.push(problem(
                "english",
                format!(
                    "{}: the catalog's English {:?} is not the crate's {:?}",
                    template.id, row.english, template.english
                ),
            ));
        }
    }
    for row in &listed {
        if !declared.iter().any(|template| template.id == row.id) {
            problems.push(problem(
                "extra-row",
                format!(
                    "{CATALOG} lists {}, which the crate does not declare",
                    row.id
                ),
            ));
        }
    }
    problems
}

/// Check (b): every template const of `files`' template table is named,
/// outside comments, literals and `#[cfg(test)]` items, in another file.
fn unused_problems(files: &BTreeMap<String, String>) -> Vec<String> {
    let Some(source) = files.get(TEMPLATES) else {
        return vec![problem(
            "no-template-file",
            format!("{SACM_SRC}/{TEMPLATES} does not exist"),
        )];
    };
    let (declared, mut problems) = source_templates(source);
    let others: Vec<String> = files
        .iter()
        .filter(|(path, _)| path.as_str() != TEMPLATES)
        .map(|(_, text)| {
            let code = code_outside_tests(text);
            let (_, skeleton) = masks(&code);
            String::from_utf8_lossy(&skeleton).into_owned()
        })
        .collect();
    for template in &declared {
        let used = others
            .iter()
            .any(|skeleton| !word_offsets(skeleton, &template.name).is_empty());
        if !used {
            problems.push(problem(
                "unused-template",
                format!(
                    "{} ({}) is named in no other file of {SACM_SRC}",
                    template.name, template.id
                ),
            ));
        }
    }
    problems
}

/// Whether the byte before `at` in `code` ends an identifier or a path, so
/// that a token found at `at` is part of a longer name.
fn follows_a_name(code: &[u8], at: usize) -> bool {
    at.checked_sub(1)
        .map(|before| code[before])
        .is_some_and(|byte| is_ident_byte(byte) || byte == b':')
}

/// Check (c): every `Diagnostic` of `files` is built from a template. In
/// each file but [`DIAGNOSTIC_SOURCE`], outside comments, literals and
/// `#[cfg(test)]` items, each `Diagnostic::new(` and `Diagnostic::create(`
/// is a problem; in [`DIAGNOSTIC_SOURCE`], each `fn create(`.
fn construction_problems(files: &BTreeMap<String, String>) -> Vec<String> {
    let mut problems = Vec::new();
    for (path, text) in files {
        let code = code_outside_tests(text);
        let (_, skeleton) = masks(&code);
        let tokens: &[&str] = if path == DIAGNOSTIC_SOURCE {
            &["fn create("]
        } else {
            &["Diagnostic::new(", "Diagnostic::create("]
        };
        for token in tokens {
            let found = skeleton
                .windows(token.len())
                .enumerate()
                .filter(|(at, window)| {
                    *window == token.as_bytes() && !follows_a_name(&skeleton, *at)
                })
                .count();
            for _ in 0..found {
                problems.push(problem(
                    "built-without-template",
                    format!(
                        "{SACM_SRC}/{path} has `{token}`: build a diagnostic from its template"
                    ),
                ));
            }
        }
    }
    problems
}

// ---------------------------------------------------------------------------
// The real files
// ---------------------------------------------------------------------------

#[test]
fn lib_api_017_the_catalog_lists_exactly_the_templates_of_template_rs() {
    let files = sacm_sources();
    let problems = catalog_problems(files.get(TEMPLATES).map(String::as_str), &read(CATALOG));
    assert!(
        problems.is_empty(),
        "{ROW}: {CATALOG} and {SACM_SRC}/{TEMPLATES} disagree: {problems:#?}"
    );
}

#[test]
fn lib_api_017_every_template_const_is_named_in_another_file_of_the_crate() {
    let problems = unused_problems(&sacm_sources());
    assert!(
        problems.is_empty(),
        "{ROW}: unused templates: {problems:#?}"
    );
}

#[test]
fn lib_api_017_template_rs_declares_the_104_refusal_templates() {
    let files = sacm_sources();
    let Some(source) = files.get(TEMPLATES) else {
        panic!("{ROW}: {SACM_SRC}/{TEMPLATES} does not exist");
    };
    let (declared, problems) = source_templates(source);
    assert!(problems.is_empty(), "{ROW}: {problems:#?}");
    let refusals: Vec<&Template> = declared
        .iter()
        .filter(|template| template.carrier != "Diagnostic")
        .collect();
    assert_eq!(
        refusals.len(),
        REFUSAL_TEMPLATES,
        "{ROW}: {SACM_SRC}/{TEMPLATES} declares {} refusal templates, not {REFUSAL_TEMPLATES}",
        refusals.len()
    );
}

#[test]
fn lib_api_018_template_rs_declares_the_108_diagnostic_templates() {
    let files = sacm_sources();
    let Some(source) = files.get(TEMPLATES) else {
        panic!("{ROW_018}: {SACM_SRC}/{TEMPLATES} does not exist");
    };
    let (declared, problems) = source_templates(source);
    assert!(problems.is_empty(), "{ROW_018}: {problems:#?}");
    let diagnostics: Vec<&Template> = declared
        .iter()
        .filter(|template| template.carrier == "Diagnostic")
        .collect();
    assert_eq!(
        diagnostics.len(),
        DIAGNOSTIC_TEMPLATES,
        "{ROW_018}: {SACM_SRC}/{TEMPLATES} declares {} diagnostic templates, not \
         {DIAGNOSTIC_TEMPLATES}",
        diagnostics.len()
    );
    assert_eq!(
        declared.len(),
        REFUSAL_TEMPLATES + DIAGNOSTIC_TEMPLATES,
        "{ROW_018}: {SACM_SRC}/{TEMPLATES} declares {} templates in all",
        declared.len()
    );
}

#[test]
fn lib_api_018_the_crate_builds_diagnostics_only_from_templates() {
    let files = sacm_sources();
    assert!(
        files.contains_key(DIAGNOSTIC_SOURCE),
        "{ROW_018}: setup: {SACM_SRC}/{DIAGNOSTIC_SOURCE} does not exist"
    );
    let problems = construction_problems(&files);
    assert!(
        problems.is_empty(),
        "{ROW_018}: diagnostics built without a template: {problems:#?}"
    );
}

// ---------------------------------------------------------------------------
// Synthetic inputs
// ---------------------------------------------------------------------------

/// `files` holding `diagnostic` as [`DIAGNOSTIC_SOURCE`] and `user` as one
/// other file.
fn construction_files(diagnostic: &str, user: &str) -> BTreeMap<String, String> {
    let mut files = BTreeMap::new();
    files.insert(DIAGNOSTIC_SOURCE.to_owned(), diagnostic.to_owned());
    files.insert("validation/names.rs".to_owned(), user.to_owned());
    files
}

/// A `diagnostic.rs` whose only constructors are `test-support`'s `new` and
/// the crate-private `emit`.
const DIAGNOSTIC_RS: &str = concat!(
    "impl Diagnostic {\n",
    "    pub fn new(code: Code, message: String) -> Self { Self { code, message, details: None } }\n",
    "    pub(crate) fn emit(template: &'static Template, arguments: Vec<Argument>) -> Self { todo() }\n",
    "}\n",
);

/// A file that builds its diagnostic from a template.
const EMITS: &str = "fn f() -> Diagnostic { Diagnostic::emit(&template::MULT_001_MISSING, Severity::Error, ROW, vec![]) }\n";

#[test]
fn lib_api_018_construction_check_reports_a_diagnostic_built_without_a_template() {
    let check =
        |diagnostic: &str, user: &str| construction_problems(&construction_files(diagnostic, user));
    assert_codes(
        "diagnostics built from templates",
        &check(DIAGNOSTIC_RS, EMITS),
        &[],
    );
    for (what, user) in [
        (
            "Diagnostic::create",
            "fn f() -> Diagnostic { Diagnostic::create(Code::REF_DANGLING, Severity::Error, ROW, m) }\n",
        ),
        (
            "Diagnostic::new",
            "fn f() -> Diagnostic { Diagnostic::new(Code::REF_DANGLING, Severity::Error, ROW, m) }\n",
        ),
        (
            "Diagnostic::create on lines of its own",
            "fn f() -> Diagnostic {\n    let d =\n        Diagnostic::create(\n            code, severity, ROW, m,\n        );\n    d\n}\n",
        ),
    ] {
        assert_codes(
            what,
            &check(DIAGNOSTIC_RS, &format!("{EMITS}{user}")),
            &["built-without-template"],
        );
    }
    assert_codes(
        "two in one file",
        &check(
            DIAGNOSTIC_RS,
            "fn f() { Diagnostic::new(a); Diagnostic::create(b); }\n",
        ),
        &["built-without-template", "built-without-template"],
    );
    for (what, user) in [
        (
            "in a comment",
            "// Diagnostic::create(code, severity, ROW, m)\n/* Diagnostic::new( */\n",
        ),
        (
            "in a literal",
            "fn f() -> &'static str { \"Diagnostic::create(\" }\n",
        ),
        (
            "in a test",
            "#[cfg(test)]\nmod tests { fn f() { let _ = Diagnostic::new(a); } }\n",
        ),
        (
            "as part of another name",
            "fn f() { MyDiagnostic::new(a); Diagnostic::newer(b); }\n",
        ),
    ] {
        assert_codes(
            &format!("named only {what}"),
            &check(DIAGNOSTIC_RS, &format!("{EMITS}{user}")),
            &[],
        );
    }
    // In diagnostic.rs, `fn create` is reported, and `Diagnostic::new` is not.
    let with_create = format!(
        "{DIAGNOSTIC_RS}impl Diagnostic {{\n    pub(crate) fn create(code: Code) -> Self {{ todo() }}\n}}\n"
    );
    assert_codes(
        "fn create in diagnostic.rs",
        &check(&with_create, EMITS),
        &["built-without-template"],
    );
    assert_codes(
        "Diagnostic::new in diagnostic.rs",
        &check(
            &format!("{DIAGNOSTIC_RS}fn g() {{ let _ = Diagnostic::new(c, m); }}\n"),
            EMITS,
        ),
        &[],
    );
    assert_codes(
        "fn create in a test of diagnostic.rs",
        &check(
            &format!("{DIAGNOSTIC_RS}#[cfg(test)]\nmod tests {{ fn create(x: u8) {{}} }}\n"),
            EMITS,
        ),
        &[],
    );
    assert_codes(
        "fn create in another file",
        &check(
            DIAGNOSTIC_RS,
            &format!("{EMITS}fn create(code: Code) -> Plan {{ todo() }}\n"),
        ),
        &[],
    );
}

/// A template table of three templates, in the fixed entry syntax.
const SOURCE: &str = concat!(
    "//! The template table.\n",
    "templates! {\n",
    "    MULT_001_MISSING = MULTIPLICITY_VIOLATION, Diagnostic, \"SACM-MULT-001/missing\",\n",
    "        [Text(\"class\"), Text(\"id\"), Text(\"role\")],\n",
    "        \"{class} {id:?} has no {role}\";\n",
    "    XML_001_UNPAIRED_SURROGATE = XML_MALFORMED, Read(\"Malformed\"), \"SACM-XML-001/unpaired-surrogate\",\n",
    "        [Count(\"unit\")],\n",
    "        \"the UTF-16 code unit {unit:04X} is an \\\"unpaired\\\" surrogate\";\n",
    "    XMI_014_UNREAD_ROOT = XMI_UNREAD_CONTENT, Unread, \"SACM-XMI-014/unread-root\",\n",
    "        [Text(\"element\"), Text(\"row\")],\n",
    "        \"the root element {element:?} is not read in this release\";\n",
    "    XMI_001_NO_ROOT_TO_WRITE = XMI_INVALID_ROOT, Write(\"NoRoot\"), \"SACM-XMI-001/no-root-to-write\",\n",
    "        [],\n",
    "        \"the document has no root to write\";\n",
    "}\n",
);

/// A catalog whose table matches [`SOURCE`].
const CATALOG_TEXT: &str = concat!(
    "# Diagnostics\n",
    "\n",
    "| SACM-XMI-014 | Warning | Content not read yet. `Code::XMI_UNREAD_CONTENT` |\n",
    "\n",
    "## Message templates\n",
    "\n",
    "Each template, sorted by id.\n",
    "\n",
    "| Template | Carrier | Arguments | English |\n",
    "|---|---|---|---|\n",
    "| `SACM-MULT-001/missing` | Diagnostic | `class`, `id`, `role` | `{class} {id:?} has no {role}` |\n",
    "| `SACM-XMI-001/no-root-to-write` | `WriteError::NoRoot` | — | `the document has no root to write` |\n",
    "| `SACM-XMI-014/unread-root` | `ReadError::Unsupported` | `element`, `row` | `the root element {element:?} is not read in this release` |\n",
    "| `SACM-XML-001/unpaired-surrogate` | `ReadError::Malformed` | `unit` (count) | `the UTF-16 code unit {unit:04X} is an \"unpaired\" surrogate` |\n",
    "\n",
    "## After\n",
    "\n",
    "| `SACM-ZZZ-001/not-a-row` | Diagnostic | — | `outside the section` |\n",
);

/// [`CATALOG_TEXT`] with its first `from` replaced by `to`.
fn catalog_with(from: &str, to: &str) -> String {
    let changed = CATALOG_TEXT.replacen(from, to, 1);
    assert_ne!(
        changed, CATALOG_TEXT,
        "{ROW}: setup: {from:?} is not in the catalog"
    );
    changed
}

/// [`SOURCE`] with its first `from` replaced by `to`.
fn source_with(from: &str, to: &str) -> String {
    let changed = SOURCE.replacen(from, to, 1);
    assert_ne!(
        changed, SOURCE,
        "{ROW}: setup: {from:?} is not in the source"
    );
    changed
}

#[test]
fn lib_api_017_catalog_check_finds_no_problem_in_a_matching_table() {
    assert_codes(
        "a matching table",
        &catalog_problems(Some(SOURCE), CATALOG_TEXT),
        &[],
    );
    let (declared, problems) = source_templates(SOURCE);
    assert!(problems.is_empty(), "{ROW}: setup: {problems:#?}");
    assert_eq!(
        declared
            .iter()
            .map(|template| (template.name.as_str(), template.carrier.as_str()))
            .collect::<Vec<_>>(),
        [
            ("MULT_001_MISSING", "Diagnostic"),
            ("XML_001_UNPAIRED_SURROGATE", "ReadError::Malformed"),
            ("XMI_014_UNREAD_ROOT", "ReadError::Unsupported"),
            ("XMI_001_NO_ROOT_TO_WRITE", "WriteError::NoRoot"),
        ],
        "{ROW}: the entries and their carriers"
    );
    assert_eq!(
        declared[1].english, "the UTF-16 code unit {unit:04X} is an \"unpaired\" surrogate",
        "{ROW}: \\\" in the pattern is a quote"
    );
}

#[test]
fn lib_api_017_catalog_check_reports_a_missing_extra_or_reordered_argument() {
    let row = "`class`, `id`, `role`";
    assert_codes(
        "a missing argument",
        &catalog_problems(Some(SOURCE), &catalog_with(row, "`class`, `id`")),
        &["missing-argument"],
    );
    assert_codes(
        "an extra argument",
        &catalog_problems(
            Some(SOURCE),
            &catalog_with(row, "`class`, `id`, `role`, `type`"),
        ),
        &["extra-argument"],
    );
    assert_codes(
        "a reordered argument",
        &catalog_problems(Some(SOURCE), &catalog_with(row, "`id`, `class`, `role`")),
        &["argument-order"],
    );
    assert_codes(
        "an argument of another kind",
        &catalog_problems(Some(SOURCE), &catalog_with("`unit` (count)", "`unit`")),
        &["argument-kind"],
    );
    assert_codes(
        "a missing argument in the crate",
        &catalog_problems(
            Some(&source_with(
                "[Text(\"element\"), Text(\"row\")]",
                "[Text(\"element\")]",
            )),
            CATALOG_TEXT,
        ),
        &["extra-argument"],
    );
}

#[test]
fn lib_api_017_catalog_check_reports_a_different_english_or_carrier() {
    assert_codes(
        "a different English",
        &catalog_problems(
            Some(SOURCE),
            &catalog_with(
                "`{class} {id:?} has no {role}`",
                "`{class} {id} has no {role}`",
            ),
        ),
        &["english"],
    );
    assert_codes(
        "an unescaped quote read as written",
        &catalog_problems(
            Some(SOURCE),
            &catalog_with("an \"unpaired\" surrogate", "an unpaired surrogate"),
        ),
        &["english"],
    );
    assert_codes(
        "a different carrier",
        &catalog_problems(
            Some(SOURCE),
            &catalog_with("`ReadError::Malformed`", "`ReadError::InvalidRoot`"),
        ),
        &["carrier"],
    );
    assert_codes(
        "Unread is ReadError::Unsupported",
        &catalog_problems(
            Some(SOURCE),
            &catalog_with("`ReadError::Unsupported`", "`ReadError::Malformed`"),
        ),
        &["carrier"],
    );
}

#[test]
fn lib_api_017_catalog_check_reports_a_missing_or_extra_row() {
    let row = "| `SACM-MULT-001/missing` | Diagnostic | `class`, `id`, `role` | `{class} {id:?} has no {role}` |\n";
    assert_codes(
        "a missing row",
        &catalog_problems(Some(SOURCE), &catalog_with(row, "")),
        &["missing-row"],
    );
    assert_codes(
        "an extra row",
        &catalog_problems(
            Some(SOURCE),
            &catalog_with(
                row,
                &format!("{row}| `SACM-MULT-001/too-many` | Diagnostic | — | `too many` |\n"),
            ),
        ),
        &["extra-row"],
    );
    assert_codes(
        "a row twice",
        &catalog_problems(Some(SOURCE), &catalog_with(row, &format!("{row}{row}"))),
        &["duplicate-row"],
    );
    // The first row moved after the last.
    let last = "an \"unpaired\" surrogate` |\n";
    let moved = catalog_with(row, "").replacen(last, &format!("{last}{row}"), 1);
    assert_ne!(
        moved,
        catalog_with(row, ""),
        "{ROW}: setup: the last row is not found"
    );
    assert_codes(
        "rows out of order",
        &catalog_problems(Some(SOURCE), &moved),
        &["row-order"],
    );
    assert_codes(
        "no section",
        &catalog_problems(
            Some(SOURCE),
            &catalog_with("## Message templates\n", "## Templates\n"),
        ),
        &[
            "no-table",
            "missing-row",
            "missing-row",
            "missing-row",
            "missing-row",
        ],
    );
    assert_codes(
        "no template file",
        &catalog_problems(None, CATALOG_TEXT),
        &["no-template-file"],
    );
}

#[test]
fn lib_api_017_catalog_check_reports_malformed_entries_and_rows() {
    assert_codes(
        "an entry without its arguments",
        &catalog_problems(Some(&source_with("        [],\n", "")), CATALOG_TEXT),
        &["malformed-entry", "extra-row"],
    );
    assert_codes(
        "an escape other than \\\" in a pattern",
        &catalog_problems(
            Some(&source_with("has no {role}\";", "has no {role}\\n\";")),
            CATALOG_TEXT,
        ),
        &["malformed-entry", "extra-row"],
    );
    assert_codes(
        "a template declared twice",
        &catalog_problems(
            Some(&source_with(
                "    XMI_001_NO_ROOT_TO_WRITE =",
                "    XMI_001_NO_ROOT_TWICE = XMI_INVALID_ROOT, Write(\"NoRoot\"), \"SACM-XMI-001/no-root-to-write\",\n        [],\n        \"the document has no root to write\";\n    XMI_001_NO_ROOT_TO_WRITE =",
            )),
            CATALOG_TEXT,
        ),
        &["duplicate-template"],
    );
    assert_codes(
        "a row with three cells",
        &catalog_problems(
            Some(SOURCE),
            &catalog_with(" | `{class} {id:?} has no {role}` |", " |"),
        ),
        &["malformed-row", "missing-row"],
    );
    assert_codes(
        "an English outside a code span",
        &catalog_problems(
            Some(SOURCE),
            &catalog_with(
                "`{class} {id:?} has no {role}`",
                "{class} {id:?} has no {role}",
            ),
        ),
        &["malformed-row", "missing-row"],
    );
}

/// `files` holding [`SOURCE`] as the template table, and `user` as one other
/// file.
fn files_with(user: &str) -> BTreeMap<String, String> {
    let mut files = BTreeMap::new();
    files.insert(TEMPLATES.to_owned(), SOURCE.to_owned());
    files.insert("xmi/user.rs".to_owned(), user.to_owned());
    files
}

/// A file that names every template of [`SOURCE`] but `MULT_001_MISSING`.
const USES: &str = concat!(
    "fn a() { let _ = (template::XML_001_UNPAIRED_SURROGATE, XMI_014_UNREAD_ROOT); }\n",
    "fn b() { let _ = &XMI_001_NO_ROOT_TO_WRITE; }\n",
);

#[test]
fn lib_api_017_unused_check_reports_a_template_named_in_no_other_file() {
    assert_codes(
        "every template named",
        &unused_problems(&files_with(&format!(
            "{USES}fn c() {{ let _ = MULT_001_MISSING; }}\n"
        ))),
        &[],
    );
    assert_codes(
        "one template named nowhere",
        &unused_problems(&files_with(USES)),
        &["unused-template"],
    );
    for (what, text) in [
        (
            "in a comment",
            "// MULT_001_MISSING\n/* MULT_001_MISSING */\n",
        ),
        (
            "in a literal",
            "fn c() -> &'static str { \"MULT_001_MISSING\" }\n",
        ),
        (
            "in a test",
            "#[cfg(test)]\nmod tests { fn c() { let _ = super::MULT_001_MISSING; } }\n",
        ),
        (
            "as part of another word",
            "fn c() { let _ = MULT_001_MISSING_TOO; }\n",
        ),
    ] {
        assert_codes(
            &format!("named only {what}"),
            &unused_problems(&files_with(&format!("{USES}{text}"))),
            &["unused-template"],
        );
    }
    // Named only in the table itself.
    let mut files = files_with(USES);
    if let Some(text) = files.get_mut(TEMPLATES) {
        text.push_str("fn uses() { let _ = MULT_001_MISSING; }\n");
    }
    assert_codes(
        "named only in template.rs",
        &unused_problems(&files),
        &["unused-template"],
    );
    let mut files = files_with(USES);
    files.remove(TEMPLATES);
    assert_codes(
        "no template file",
        &unused_problems(&files),
        &["no-template-file"],
    );
}
