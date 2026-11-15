//! Repository checks for the gated list of interim refusals (LIB-API-011,
//! ADR 0033).
//!
//! Every `ReadError::Unsupported` of the `sacm` crate comes from one
//! crate-private list, `crates/sacm/src/xmi/refusal.rs`, whose entries are
//! labelled `Interim("ROW")`, `Tolerance("ROW")`, `Limit` or `Defect`. Three
//! checks keep that list honest:
//!
//! - (a) outside comments and `#[cfg(test)]` items, the token `Unsupported`
//!   appears in `crates/sacm/src` only in `xmi.rs` (where `ReadError` is
//!   declared) and in `xmi/refusal.rs`;
//! - (b) every `Interim("ROW")` names a matrix row that exists, is not
//!   `verified`, and is in the `depends_on` closure of each of SACM23-CP-001
//!   to SACM23-CP-004, so no compliance point can be verified while the
//!   reader still refuses conforming content it is to read;
//! - (c) every `Tolerance("ROW")` names a row that exists and is not
//!   `verified`.
//!
//! As in `repo_checks.rs`, each check is a function that returns the problems
//! it finds, and every problem starts with a stable code in brackets, so a
//! test can assert which check reported a mutation. Each check is applied to
//! the real files, which must pass, and to in-memory mutations of them, each
//! of which must give its own code. Nothing on disk is changed.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

const ROW: &str = "LIB-API-011";

/// The source directory of the `sacm` crate, relative to the repository root.
const SACM_SRC: &str = "crates/sacm/src";

/// The one list of refusals, relative to [`SACM_SRC`].
const LIST: &str = "xmi/refusal.rs";

/// The files, relative to [`SACM_SRC`], that may name `Unsupported`: the
/// declaration of `ReadError` and the list.
const ALLOWED: [&str; 2] = ["xmi.rs", LIST];

/// The compliance points whose `depends_on` closure must hold every
/// `Interim` row.
const COMPLIANCE_POINTS: [&str; 4] = [
    "SACM23-CP-001",
    "SACM23-CP-002",
    "SACM23-CP-003",
    "SACM23-CP-004",
];

const MATRIX: &str = "docs/matrix/functionality-matrix.toml";

fn repository_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repository root")
}

fn read(rel: &str) -> String {
    fs::read_to_string(repository_root().join(rel))
        .unwrap_or_else(|e| panic!("{ROW}: read {rel}: {e}"))
}

fn matrix() -> toml::Table {
    read(MATRIX)
        .parse()
        .unwrap_or_else(|e| panic!("{ROW}: parse {MATRIX}: {e}"))
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

/// Asserts that `problems` are exactly `count` problems, all with `code`.
fn assert_only(what: &str, problems: &[String], code: &str, count: usize) {
    assert_eq!(
        problem_codes(problems),
        vec![code.to_owned(); count],
        "{ROW}: {what}: expected {count} [{code}], got {problems:#?}"
    );
}

// ---------------------------------------------------------------------------
// A small Rust lexer: comments, literals and `#[cfg(test)]` items
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
/// comment blanked, the second with every literal blanked too. Line breaks
/// are kept, so offsets and line numbers agree with `source`.
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
    String::from_utf8(code).expect("blanking keeps UTF-8")
}

/// The 1-based line of byte `offset` of `text`.
fn line_of(text: &str, offset: usize) -> usize {
    text[..offset].matches('\n').count() + 1
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

// ---------------------------------------------------------------------------
// The checks
// ---------------------------------------------------------------------------

/// Check (a): `Unsupported`, outside comments and `#[cfg(test)]` items, only
/// in the [`ALLOWED`] files, and the list exists.
fn unsupported_problems(files: &BTreeMap<String, String>) -> Vec<String> {
    let mut problems = Vec::new();
    if !files.contains_key(LIST) {
        problems.push(problem(
            "no-refusal-list",
            format!("{SACM_SRC}/{LIST} does not exist"),
        ));
    }
    for (path, text) in files {
        if ALLOWED.contains(&path.as_str()) {
            continue;
        }
        let code = code_outside_tests(text);
        for at in word_offsets(&code, "Unsupported") {
            problems.push(problem(
                "unsupported-outside-list",
                format!(
                    "{SACM_SRC}/{path}:{} names Unsupported outside {LIST}",
                    line_of(text, at)
                ),
            ));
        }
    }
    problems
}

/// The rows of every `label("ROW")` in `list`, outside comments and
/// `#[cfg(test)]` items, in order, with their lines.
fn labelled_rows(list: &str, label: &str) -> Vec<(String, usize)> {
    let code = code_outside_tests(list);
    let opening = format!("{label}(\"");
    word_offsets(&code, label)
        .into_iter()
        .filter(|&at| code[at..].starts_with(&opening))
        .map(|at| {
            let from = at + opening.len();
            let row = code[from..].split('"').next().unwrap_or("").to_owned();
            (row, line_of(list, at))
        })
        .collect()
}

/// The matrix rows by id: their status and their `depends_on`.
fn matrix_rows(matrix: &toml::Table) -> BTreeMap<String, (String, Vec<String>)> {
    let rows = matrix
        .get("row")
        .and_then(toml::Value::as_array)
        .unwrap_or_else(|| panic!("{ROW}: the matrix has no [[row]] array"));
    rows.iter()
        .filter_map(|row| {
            let id = row.get("id")?.as_str()?.to_owned();
            let status = row
                .get("status")
                .and_then(toml::Value::as_str)
                .unwrap_or("")
                .to_owned();
            let depends_on = row
                .get("depends_on")
                .and_then(toml::Value::as_array)
                .map(|ids| {
                    ids.iter()
                        .filter_map(toml::Value::as_str)
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default();
            Some((id, (status, depends_on)))
        })
        .collect()
}

/// Every row `id` depends on, directly or not.
fn closure(rows: &BTreeMap<String, (String, Vec<String>)>, id: &str) -> BTreeSet<String> {
    let mut seen = BTreeSet::new();
    let mut stack = vec![id.to_owned()];
    while let Some(next) = stack.pop() {
        if let Some((_, depends_on)) = rows.get(&next) {
            for dependency in depends_on {
                if seen.insert(dependency.clone()) {
                    stack.push(dependency.clone());
                }
            }
        }
    }
    seen
}

/// Check (b): every `Interim` row exists, is not verified, and is in the
/// closure of each compliance point.
fn interim_problems(list: &str, matrix: &toml::Table) -> Vec<String> {
    let rows = matrix_rows(matrix);
    let mut problems = Vec::new();
    let interim = labelled_rows(list, "Interim");
    if interim.is_empty() {
        problems.push(problem(
            "no-interim-rows",
            format!("{LIST} names no Interim(\"ROW\")"),
        ));
    }
    let mut closures = Vec::new();
    for point in COMPLIANCE_POINTS {
        if rows.contains_key(point) {
            closures.push((point, closure(&rows, point)));
        } else {
            problems.push(problem(
                "compliance-point-missing",
                format!("the matrix has no row {point}"),
            ));
        }
    }
    for (row, line) in interim {
        let Some((status, _)) = rows.get(&row) else {
            problems.push(problem(
                "interim-row-unknown",
                format!("{LIST}:{line}: Interim({row:?}) names no matrix row"),
            ));
            continue;
        };
        if status == "verified" {
            problems.push(problem(
                "interim-row-verified",
                format!("{LIST}:{line}: Interim({row:?}) names a verified row"),
            ));
        }
        for (point, depends) in &closures {
            if !depends.contains(&row) {
                problems.push(problem(
                    "interim-row-outside-compliance-point",
                    format!(
                        "{LIST}:{line}: Interim({row:?}) is not in the depends_on closure of {point}"
                    ),
                ));
            }
        }
    }
    problems
}

/// Check (c): every `Tolerance` row exists and is not verified.
fn tolerance_problems(list: &str, matrix: &toml::Table) -> Vec<String> {
    let rows = matrix_rows(matrix);
    let mut problems = Vec::new();
    let tolerance = labelled_rows(list, "Tolerance");
    if tolerance.is_empty() {
        problems.push(problem(
            "no-tolerance-rows",
            format!("{LIST} names no Tolerance(\"ROW\")"),
        ));
    }
    for (row, line) in tolerance {
        match rows.get(&row) {
            None => problems.push(problem(
                "tolerance-row-unknown",
                format!("{LIST}:{line}: Tolerance({row:?}) names no matrix row"),
            )),
            Some((status, _)) if status == "verified" => problems.push(problem(
                "tolerance-row-verified",
                format!("{LIST}:{line}: Tolerance({row:?}) names a verified row"),
            )),
            Some(_) => {}
        }
    }
    problems
}

// ---------------------------------------------------------------------------
// Mutation helpers
// ---------------------------------------------------------------------------

fn list_text() -> String {
    read(&format!("{SACM_SRC}/{LIST}"))
}

/// `files` with `text` added as `path`, which must be new.
fn with_file(files: &BTreeMap<String, String>, path: &str, text: &str) -> BTreeMap<String, String> {
    let mut files = files.clone();
    assert!(
        files.insert(path.to_owned(), text.to_owned()).is_none(),
        "{ROW}: setup: {path} exists already"
    );
    files
}

/// The first row `label("ROW")` of the real list names.
fn first_row(list: &str, label: &str) -> String {
    labelled_rows(list, label).into_iter().next().map_or_else(
        || panic!("{ROW}: setup: {LIST} names no {label}(\"ROW\")"),
        |(row, _)| row,
    )
}

/// `matrix` with the row `id`'s `key` set to `value`.
fn with_row_value(matrix: &toml::Table, id: &str, key: &str, value: toml::Value) -> toml::Table {
    let mut matrix = matrix.clone();
    let rows = matrix
        .get_mut("row")
        .and_then(toml::Value::as_array_mut)
        .expect("the matrix has a [[row]] array");
    let row = rows
        .iter_mut()
        .find(|row| row.get("id").and_then(toml::Value::as_str) == Some(id))
        .unwrap_or_else(|| panic!("{ROW}: setup: the matrix has no row {id}"))
        .as_table_mut()
        .expect("a row is a table");
    row.insert(key.to_owned(), value);
    matrix
}

/// `matrix` without the row `id`.
fn without_row(matrix: &toml::Table, id: &str) -> toml::Table {
    let mut matrix = matrix.clone();
    let rows = matrix
        .get_mut("row")
        .and_then(toml::Value::as_array_mut)
        .expect("the matrix has a [[row]] array");
    let before = rows.len();
    rows.retain(|row| row.get("id").and_then(toml::Value::as_str) != Some(id));
    assert_eq!(
        rows.len() + 1,
        before,
        "{ROW}: setup: no row {id} to remove"
    );
    matrix
}

/// `text` with every `from` replaced by `to`; at least one must be there.
fn replace_all(text: &str, from: &str, to: &str) -> String {
    assert!(
        text.contains(from),
        "{ROW}: setup: mutation target `{from}` is not in the text"
    );
    text.replace(from, to)
}

// ---------------------------------------------------------------------------
// (a) `Unsupported` is built only in the list
// ---------------------------------------------------------------------------

#[test]
fn lib_api_011_unsupported_is_named_only_in_xmi_rs_and_the_refusal_list() {
    let problems = unsupported_problems(&sacm_sources());
    assert!(
        problems.is_empty(),
        "{ROW}: every ReadError::Unsupported comes from {LIST}: {problems:#?}"
    );
}

#[test]
fn lib_api_011_unsupported_check_reports_a_missing_list() {
    let mut files = sacm_sources();
    assert!(
        files.remove(LIST).is_some(),
        "{ROW}: setup: {SACM_SRC}/{LIST} does not exist"
    );
    assert_only(
        "the list removed",
        &unsupported_problems(&files),
        "no-refusal-list",
        1,
    );
}

#[test]
fn lib_api_011_unsupported_check_reports_a_construction_outside_the_list() {
    let files = sacm_sources();
    let built = "fn refuse() -> crate::xmi::ReadError {\n    \
                 crate::xmi::ReadError::Unsupported { location: None, reason: String::new() }\n}\n";
    assert_only(
        "Unsupported built in a new module",
        &unsupported_problems(&with_file(&files, "model/extra.rs", built)),
        "unsupported-outside-list",
        1,
    );
    let matched =
        "fn is_refusal(e: &ReadError) -> bool { matches!(e, ReadError::Unsupported { .. }) }\n";
    assert_only(
        "Unsupported matched in another xmi module",
        &unsupported_problems(&with_file(&files, "xmi/extra.rs", matched)),
        "unsupported-outside-list",
        1,
    );
    let used =
        "use crate::xmi::ReadError::Unsupported;\nfn a() {}\nfn b() { let _ = Unsupported; }\n";
    assert_only(
        "Unsupported imported and used: one problem per occurrence",
        &unsupported_problems(&with_file(&files, "edit/extra.rs", used)),
        "unsupported-outside-list",
        2,
    );
}

#[test]
fn lib_api_011_unsupported_check_ignores_comments_tests_and_other_words() {
    let files = sacm_sources();
    let ignored = concat!(
        "//! Unsupported in a module doc.\n",
        "/// Unsupported in an item doc.\n",
        "fn a() {} // Unsupported after code\n",
        "/* Unsupported /* nested Unsupported */ still a comment Unsupported */\n",
        "fn b() -> usize { let unsupported = 1; let NotUnsupported = 2; Unsupported_2 + unsupported }\n",
        "#[cfg(test)]\n",
        "mod tests {\n",
        "    const S: &str = \"}\";\n",
        "    const C: char = '}';\n",
        "    fn f<'a>(x: &'a str) -> &'a str { let _ = ReadError::Unsupported { reason: x }; x }\n",
        "}\n",
        "#[cfg(test)]\n",
        "fn g() { let _ = Unsupported; }\n",
    );
    let problems = unsupported_problems(&with_file(&files, "model/extra.rs", ignored));
    assert!(
        problems.is_empty(),
        "{ROW}: comments, #[cfg(test)] items and other words are not uses: {problems:#?}"
    );
    let allowed = with_file(
        &files,
        "model/unrelated.rs",
        "// nothing here names it\nfn c() {}\n",
    );
    assert!(
        unsupported_problems(&allowed).is_empty(),
        "{ROW}: a file that does not name it gives no problem"
    );
}

#[test]
fn lib_api_011_unsupported_check_is_not_fooled_by_literals_or_short_test_items() {
    let files = sacm_sources();
    let cases = [
        (
            "after a string holding //",
            "fn a() { let url = \"http://example.invalid\"; let _ = Unsupported; }\n",
        ),
        (
            "after a string holding /*",
            "fn a() { let s = \"/*\"; let _ = Unsupported; }\n",
        ),
        (
            "after a raw string holding a quote and //",
            "fn a() { let s = r#\"a \" // b\"#; let _ = Unsupported; }\n",
        ),
        (
            "after a #[cfg(test)] item that ends at ;",
            "#[cfg(test)]\nuse std::fmt;\nfn a() { let _ = Unsupported; }\n",
        ),
        (
            "after a #[cfg(test)] block holding a brace in a string",
            "#[cfg(test)]\nmod t { const S: &str = \"{\"; }\nfn a() { let _ = Unsupported; }\n",
        ),
        (
            "after a lifetime",
            "fn a<'a>(x: &'a str) -> &'a str { let _ = Unsupported; x }\n",
        ),
        (
            "in a string literal",
            "fn a() -> &'static str { \"Unsupported\" }\n",
        ),
    ];
    for (what, text) in cases {
        assert_only(
            what,
            &unsupported_problems(&with_file(&files, "model/extra.rs", text)),
            "unsupported-outside-list",
            1,
        );
    }
}

#[test]
fn lib_api_011_unsupported_check_allows_xmi_rs_and_the_list() {
    let mut files = sacm_sources();
    for path in ALLOWED {
        let text = files
            .get_mut(path)
            .unwrap_or_else(|| panic!("{ROW}: setup: {SACM_SRC}/{path} does not exist"));
        text.push_str("\nfn extra() { let _ = ReadError::Unsupported { location: None, reason: String::new() }; }\n");
    }
    let problems = unsupported_problems(&files);
    assert!(
        problems.is_empty(),
        "{ROW}: xmi.rs and {LIST} may name Unsupported: {problems:#?}"
    );
}

// ---------------------------------------------------------------------------
// (b) every Interim row is open and gates every compliance point
// ---------------------------------------------------------------------------

#[test]
fn lib_api_011_every_interim_row_exists_is_open_and_gates_each_compliance_point() {
    let problems = interim_problems(&list_text(), &matrix());
    assert!(
        problems.is_empty(),
        "{ROW}: the Interim rows of {LIST}: {problems:#?}"
    );
}

#[test]
fn lib_api_011_interim_check_reports_an_unknown_row() {
    let list = list_text();
    let row = first_row(&list, "Interim");
    let mutated = replace_all(
        &list,
        &format!("Interim(\"{row}\")"),
        "Interim(\"SACM23-NOPE-999\")",
    );
    let problems = interim_problems(&mutated, &matrix());
    assert!(
        !problems.is_empty()
            && problem_codes(&problems)
                .iter()
                .all(|c| c == "interim-row-unknown"),
        "{ROW}: an Interim row that does not exist: {problems:#?}"
    );
}

#[test]
fn lib_api_011_interim_check_reports_a_verified_row() {
    let list = list_text();
    let row = first_row(&list, "Interim");
    let uses = labelled_rows(&list, "Interim")
        .iter()
        .filter(|(found, _)| *found == row)
        .count();
    let verified = with_row_value(&matrix(), &row, "status", toml::Value::from("verified"));
    assert_only(
        "an Interim row marked verified",
        &interim_problems(&list, &verified),
        "interim-row-verified",
        uses,
    );
}

#[test]
fn lib_api_011_interim_check_reports_a_row_outside_a_compliance_point() {
    let list = list_text();
    let count = labelled_rows(&list, "Interim").len();
    let emptied = with_row_value(
        &matrix(),
        "SACM23-CP-004",
        "depends_on",
        toml::Value::Array(Vec::new()),
    );
    let problems = interim_problems(&list, &emptied);
    assert!(
        problem_codes(&problems)
            .iter()
            .all(|c| c == "interim-row-outside-compliance-point"),
        "{ROW}: SACM23-CP-004 with no dependencies: {problems:#?}"
    );
    assert_eq!(
        problems
            .iter()
            .filter(|p| p.contains("SACM23-CP-004"))
            .count(),
        count,
        "{ROW}: each Interim entry is reported as outside SACM23-CP-004: {problems:#?}"
    );
}

#[test]
fn lib_api_011_interim_check_reports_a_missing_compliance_point() {
    let problems = interim_problems(&list_text(), &without_row(&matrix(), "SACM23-CP-003"));
    assert_only(
        "SACM23-CP-003 removed",
        &problems,
        "compliance-point-missing",
        1,
    );
}

#[test]
fn lib_api_011_interim_check_reports_a_list_with_no_interim_rows() {
    let list = replace_all(&list_text(), "Interim(\"", "Interim(r\"");
    assert_only(
        "no Interim(\"ROW\") left",
        &interim_problems(&list, &matrix()),
        "no-interim-rows",
        1,
    );
}

#[test]
fn lib_api_011_interim_check_ignores_rows_in_comments_and_tests() {
    let mut list = list_text();
    list.push_str(concat!(
        "\n// Interim(\"SACM23-NOPE-998\")\n",
        "/// Interim(\"SACM23-NOPE-997\")\n",
        "#[cfg(test)]\n",
        "mod extra_tests { const X: Refusal = Refusal::Interim(\"SACM23-NOPE-996\"); }\n",
    ));
    let problems = interim_problems(&list, &matrix());
    assert!(
        problems.is_empty(),
        "{ROW}: rows in comments and #[cfg(test)] items are not entries: {problems:#?}"
    );
}

// ---------------------------------------------------------------------------
// (c) every Tolerance row is open
// ---------------------------------------------------------------------------

#[test]
fn lib_api_011_every_tolerance_row_exists_and_is_open() {
    let problems = tolerance_problems(&list_text(), &matrix());
    assert!(
        problems.is_empty(),
        "{ROW}: the Tolerance rows of {LIST}: {problems:#?}"
    );
}

#[test]
fn lib_api_011_tolerance_check_reports_an_unknown_row() {
    let list = list_text();
    let row = first_row(&list, "Tolerance");
    let mutated = replace_all(
        &list,
        &format!("Tolerance(\"{row}\")"),
        "Tolerance(\"LIB-NOPE-999\")",
    );
    let problems = tolerance_problems(&mutated, &matrix());
    assert!(
        !problems.is_empty()
            && problem_codes(&problems)
                .iter()
                .all(|c| c == "tolerance-row-unknown"),
        "{ROW}: a Tolerance row that does not exist: {problems:#?}"
    );
}

#[test]
fn lib_api_011_tolerance_check_reports_a_verified_row() {
    let list = list_text();
    let row = first_row(&list, "Tolerance");
    let uses = labelled_rows(&list, "Tolerance")
        .iter()
        .filter(|(found, _)| *found == row)
        .count();
    let verified = with_row_value(&matrix(), &row, "status", toml::Value::from("verified"));
    assert_only(
        "a Tolerance row marked verified",
        &tolerance_problems(&list, &verified),
        "tolerance-row-verified",
        uses,
    );
}

#[test]
fn lib_api_011_tolerance_check_reports_a_list_with_no_tolerance_rows() {
    let list = replace_all(&list_text(), "Tolerance(\"", "Tolerance(r\"");
    assert_only(
        "no Tolerance(\"ROW\") left",
        &tolerance_problems(&list, &matrix()),
        "no-tolerance-rows",
        1,
    );
}

#[test]
fn lib_api_011_tolerance_rows_are_not_checked_against_the_compliance_points() {
    // A Tolerance row reads non-conforming input, so no compliance point
    // waits for it; only Interim rows gate them.
    let list = list_text();
    let emptied = COMPLIANCE_POINTS.iter().fold(matrix(), |matrix, point| {
        with_row_value(&matrix, point, "depends_on", toml::Value::Array(Vec::new()))
    });
    let problems = tolerance_problems(&list, &emptied);
    assert!(
        problems.is_empty(),
        "{ROW}: the Tolerance check does not depend on the compliance points: {problems:#?}"
    );
}

// ---------------------------------------------------------------------------
// The compliance points name the two rows the maintainer added (2026-11-15)
// ---------------------------------------------------------------------------

#[test]
fn lib_api_011_each_compliance_point_depends_directly_on_xmi_027_and_compat_004() {
    let rows = matrix_rows(&matrix());
    for point in COMPLIANCE_POINTS {
        let Some((_, depends_on)) = rows.get(point) else {
            panic!("{ROW}: the matrix has no row {point}");
        };
        for row in ["SACM23-XMI-027", "LIB-COMPAT-004"] {
            assert!(
                depends_on.iter().any(|d| d == row),
                "{ROW}: {point} lists {row} in its depends_on (the maintainer, 2026-11-15)"
            );
        }
    }
}
