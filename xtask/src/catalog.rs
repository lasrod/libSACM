//! The machine-readable diagnostics catalog, `cargo xtask catalog`
//! (LIB-DIAG-006; ADR 0011 amendment 5).
//!
//! [`generate`] builds `crates/sacm/src/diagnostic/catalog.json`, which the
//! crate ships as `sacm::diagnostic::CATALOG_JSON`, from three files of the
//! repository:
//!
//! - the `templates!` entries of `crates/sacm/src/diagnostic/template.rs`;
//! - the `pub const NAME: Code = Code::from_static("TEXT");` constants and
//!   the `serde(rename = "…")` names of `pub enum Specification` in
//!   `crates/sacm/src/diagnostic.rs`;
//! - the Severity cell of each code's live row in `docs/sacm/diagnostics.md`.
//!
//! A test of the crate fails when the checked-in file is not what
//! [`generate`] gives.

use std::fs;
use std::io::Write;
use std::path::Path;

use serde::Serialize;

/// The file `cargo xtask catalog` writes, relative to the repository root.
pub const OUTPUT: &str = "crates/sacm/src/diagnostic/catalog.json";

/// The template table.
const TEMPLATE_RS: &str = "crates/sacm/src/diagnostic/template.rs";

/// The code constants and the specifications.
const DIAGNOSTIC_RS: &str = "crates/sacm/src/diagnostic.rs";

/// The diagnostics catalog, with the Severity of each code.
const CATALOG_MD: &str = "docs/sacm/diagnostics.md";

/// The version of the JSON shape: it changes only on a breaking change
/// (ADR 0030 amendment 1).
const FORMAT: u32 = 1;

/// The severity words of a Severity cell and their JSON names, in the order
/// the catalog lists them.
const SEVERITIES: [(&str, &str); 3] =
    [("Error", "error"), ("Warning", "warning"), ("Info", "info")];

/// The whole catalog, in the order its keys are written.
#[derive(Serialize)]
struct Catalog {
    format: u32,
    specifications: Vec<String>,
    codes: Vec<CodeEntry>,
    templates: Vec<TemplateEntry>,
}

/// One code and the severities it is reported with.
#[derive(Serialize)]
struct CodeEntry {
    code: String,
    severities: Vec<&'static str>,
}

/// One message template.
#[derive(Serialize)]
struct TemplateEntry {
    id: String,
    code: String,
    carrier: String,
    arguments: Vec<ArgumentEntry>,
    english: String,
}

/// One argument of a template: its name and `text`, `count` or `prose`.
#[derive(Serialize)]
struct ArgumentEntry {
    name: String,
    kind: &'static str,
}

/// The catalog of the repository at `root`, as the JSON text of
/// `crates/sacm/src/diagnostic/catalog.json`.
///
/// - `format` is 1; `specifications` the `serde(rename)` names of
///   `Specification`, in order.
/// - `codes`, sorted by their text: each `Code` constant with the severities
///   its live row's Severity cell names (`Error`, `Warning`, `Info`, listed
///   error, warning, info); a cell of exactly `—` lists none.
/// - `templates`, sorted by id: each `templates!` entry with its code, its
///   carrier (`Diagnostic`, `ReadError::<Variant>`, `WriteError::<Variant>`,
///   and `ReadError::Unsupported` for `Unread`), its arguments and its
///   English pattern, with `\"` and `\\` read as `"` and `\`.
///
/// The text is `serde_json`'s pretty form and a line feed.
///
/// ```
/// let json = xtask::catalog::generate(&xtask::repo_root())?;
/// assert!(json.starts_with("{\n  \"format\": 1,"));
/// # Ok::<(), String>(())
/// ```
///
/// # Errors
///
/// A message when an input cannot be read, or one line per problem:
/// - `[severity] <code>: "<cell>" names no severity`;
/// - `[no-row] <code>`: no live row names the constant;
/// - `[malformed-entry]`: a `templates!` entry that does not parse;
/// - `[unknown-code]`: an entry names no `Code` constant;
/// - `[no-specification]`: `Specification` has no `serde(rename)` name.
pub fn generate(root: &Path) -> Result<String, String> {
    render(&build(root)?)
}

/// `cargo xtask catalog`: writes [`generate`]'s text to [`OUTPUT`] under
/// `root` and reports how many codes and templates it holds.
///
/// # Errors
///
/// As [`generate`], and when the file or `out` cannot be written.
pub fn cmd_catalog(root: &Path, out: &mut dyn Write) -> Result<(), String> {
    let catalog = build(root)?;
    let text = render(&catalog)?;
    let path = root.join(OUTPUT);
    fs::write(&path, text).map_err(|error| format!("write {}: {error}", path.display()))?;
    writeln!(
        out,
        "wrote {OUTPUT}: {} codes, {} templates",
        catalog.codes.len(),
        catalog.templates.len()
    )
    .map_err(crate::output_error)
}

/// The JSON text of `catalog`.
fn render(catalog: &Catalog) -> Result<String, String> {
    serde_json::to_string_pretty(catalog)
        .map(|text| text + "\n")
        .map_err(|error| format!("serialize the catalog: {error}"))
}

/// The text of the file `rel` under `root`.
fn read(root: &Path, rel: &str) -> Result<String, String> {
    let path = root.join(rel);
    fs::read_to_string(&path).map_err(|error| format!("read {}: {error}", path.display()))
}

/// The catalog of the three inputs under `root`, or every problem found.
fn build(root: &Path) -> Result<Catalog, String> {
    let template_rs = read(root, TEMPLATE_RS)?;
    let diagnostic_rs = read(root, DIAGNOSTIC_RS)?;
    let catalog_md = read(root, CATALOG_MD)?;
    let mut problems = Vec::new();

    let specifications = specifications(&diagnostic_rs);
    if specifications.is_empty() {
        problems.push(format!(
            "[no-specification] {DIAGNOSTIC_RS}: `pub enum Specification` has no serde(rename = \"…\")"
        ));
    }

    let constants = code_constants(&diagnostic_rs);
    let mut codes = Vec::new();
    for (name, code) in &constants {
        match live_row_severity(&catalog_md, name, code) {
            None => problems.push(format!(
                "[no-row] {code}: no row of {CATALOG_MD} for {code} names `Code::{name}`"
            )),
            Some(cell) => match severities(code, cell) {
                Ok(severities) => codes.push(CodeEntry {
                    code: code.clone(),
                    severities,
                }),
                Err(problem) => problems.push(problem),
            },
        }
    }
    codes.sort_by(|a, b| a.code.cmp(&b.code));

    let entries = source_templates(&template_rs).unwrap_or_else(|problem| {
        problems.push(problem);
        Vec::new()
    });
    let mut templates = Vec::new();
    for entry in entries {
        match constants.iter().find(|(name, _)| *name == entry.constant) {
            Some((_, code)) => templates.push(TemplateEntry {
                id: entry.id,
                code: code.clone(),
                carrier: entry.carrier,
                arguments: entry.arguments,
                english: entry.english,
            }),
            None => problems.push(format!(
                "[unknown-code] {}: {TEMPLATE_RS} names `Code::{}`, which {DIAGNOSTIC_RS} does not define",
                entry.id, entry.constant
            )),
        }
    }
    templates.sort_by(|a, b| a.id.cmp(&b.id));

    if problems.is_empty() {
        Ok(Catalog {
            format: FORMAT,
            specifications,
            codes,
            templates,
        })
    } else {
        Err(problems.join("\n"))
    }
}

/// Each `pub const NAME: Code = Code::from_static("TEXT");` of `source`, as
/// `(NAME, TEXT)`, in source order.
fn code_constants(source: &str) -> Vec<(String, String)> {
    source
        .lines()
        .filter_map(|line| {
            let rest = line.trim().strip_prefix("pub const ")?;
            let (name, rest) = rest.split_once(": Code = Code::from_static(\"")?;
            let text = rest.strip_suffix("\");")?;
            let is_name = !name.is_empty() && name.bytes().all(is_ident_byte);
            is_name.then(|| (name.to_owned(), text.to_owned()))
        })
        .collect()
}

/// The `serde(rename = "…")` names inside `pub enum Specification { … }` of
/// `source`, in order.
fn specifications(source: &str) -> Vec<String> {
    const RENAME: &str = "serde(rename = \"";
    source
        .lines()
        .skip_while(|line| line.trim() != "pub enum Specification {")
        .skip(1)
        .take_while(|line| line.trim() != "}")
        .filter_map(|line| {
            let (_, rest) = line.split_once(RENAME)?;
            let (name, _) = rest.split_once('"')?;
            Some(name.to_owned())
        })
        .collect()
}

/// The Severity cell of the live row of `code` in `catalog`: the first table
/// row whose first cell is `code` and which names `` `Code::NAME` ``.
fn live_row_severity<'c>(catalog: &'c str, name: &str, code: &str) -> Option<&'c str> {
    let constant = format!("`Code::{name}`");
    catalog.lines().find_map(|line| {
        let mut cells = line.trim().strip_prefix('|')?.split('|');
        let first = cells.next()?.trim();
        let severity = cells.next()?.trim();
        (first == code && line.contains(&constant)).then_some(severity)
    })
}

/// The severities the Severity `cell` of `code` names: each of `Error`,
/// `Warning` and `Info` it holds as a word, in that order; none for a cell of
/// exactly `—`.
fn severities(code: &str, cell: &str) -> Result<Vec<&'static str>, String> {
    if cell == "—" {
        return Ok(Vec::new());
    }
    let words: Vec<&str> = cell.split(|c: char| !c.is_alphanumeric()).collect();
    let named: Vec<&'static str> = SEVERITIES
        .iter()
        .filter(|(word, _)| words.contains(word))
        .map(|&(_, severity)| severity)
        .collect();
    if named.is_empty() {
        Err(format!("[severity] {code}: {cell:?} names no severity"))
    } else {
        Ok(named)
    }
}

/// Whether `byte` may stand in a Rust identifier of the inputs.
fn is_ident_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

// ---------------------------------------------------------------------------
// The templates! table
// ---------------------------------------------------------------------------

/// One entry of the `templates!` table.
struct SourceTemplate {
    /// The `Code` constant it names.
    constant: String,
    /// `Diagnostic`, `ReadError::<Variant>` or `WriteError::<Variant>`.
    carrier: String,
    id: String,
    arguments: Vec<ArgumentEntry>,
    english: String,
}

/// A token of the `templates!` body.
#[derive(Debug, PartialEq, Eq)]
enum Token {
    /// A name, such as `SEC_001_DOCTYPE` or `Text`.
    Ident(String),
    /// A string literal, unescaped.
    Str(String),
    /// One of `=`, `,`, `(`, `)`, `[`, `]` and `;`.
    Punct(char),
}

/// The entries of the `templates!` table of `source`, the text of
/// `template.rs`, in source order.
fn source_templates(source: &str) -> Result<Vec<SourceTemplate>, String> {
    const OPEN: &str = "templates! {";
    let start = if source.starts_with(OPEN) {
        0
    } else {
        source
            .find(&format!("\n{OPEN}"))
            .map(|at| at + 1)
            .ok_or_else(|| format!("[malformed-entry] {TEMPLATE_RS} has no `{OPEN}` table"))?
    };
    let body = &source[start + OPEN.len()..];
    let tokens = tokens(body)?;
    let mut cursor = Cursor {
        tokens: &tokens,
        at: 0,
    };
    let mut entries = Vec::new();
    while !cursor.is_done() {
        match cursor.entry() {
            Ok(entry) => entries.push(entry),
            Err(what) => {
                let after = entries
                    .last()
                    .map_or("the start of the table", |last| last.id.as_str());
                return Err(format!(
                    "[malformed-entry] {TEMPLATE_RS}: {what}, in the entry after {after}"
                ));
            }
        }
    }
    Ok(entries)
}

/// The tokens of `body`, the text after `templates! {`, up to its closing
/// `}`. Line comments are skipped; a string literal may escape only `"` and
/// `\`.
fn tokens(body: &str) -> Result<Vec<Token>, String> {
    let malformed = |what: &str| format!("[malformed-entry] {TEMPLATE_RS}: {what}");
    let mut tokens = Vec::new();
    let mut chars = body.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '}' => return Ok(tokens),
            c if c.is_whitespace() => {}
            '/' if chars.peek() == Some(&'/') => {
                for skipped in chars.by_ref() {
                    if skipped == '\n' {
                        break;
                    }
                }
            }
            '"' => {
                let mut text = String::new();
                loop {
                    match chars.next() {
                        None => return Err(malformed("a string literal is not closed")),
                        Some('"') => break,
                        Some('\\') => match chars.next() {
                            Some('"') => text.push('"'),
                            Some('\\') => text.push('\\'),
                            other => {
                                return Err(malformed(&format!(
                                    "a string literal escapes {other:?}, not '\"' or '\\\\'"
                                )));
                            }
                        },
                        Some(c) => text.push(c),
                    }
                }
                tokens.push(Token::Str(text));
            }
            '=' | ',' | '(' | ')' | '[' | ']' | ';' => tokens.push(Token::Punct(c)),
            c if c.is_ascii_alphanumeric() || c == '_' => {
                let mut name = String::from(c);
                while let Some(&next) = chars.peek() {
                    if !(next.is_ascii_alphanumeric() || next == '_') {
                        break;
                    }
                    name.push(next);
                    chars.next();
                }
                tokens.push(Token::Ident(name));
            }
            other => return Err(malformed(&format!("unexpected {other:?}"))),
        }
    }
    Err(malformed("the templates! table is not closed"))
}

/// A position in the tokens of the `templates!` body.
struct Cursor<'t> {
    tokens: &'t [Token],
    at: usize,
}

impl Cursor<'_> {
    fn is_done(&self) -> bool {
        self.at >= self.tokens.len()
    }

    fn next(&mut self) -> Option<&Token> {
        let token = self.tokens.get(self.at);
        self.at += 1;
        token
    }

    fn ident(&mut self, what: &str) -> Result<String, String> {
        match self.next() {
            Some(Token::Ident(name)) => Ok(name.clone()),
            other => Err(format!("expected {what}, found {other:?}")),
        }
    }

    fn string(&mut self, what: &str) -> Result<String, String> {
        match self.next() {
            Some(Token::Str(text)) => Ok(text.clone()),
            other => Err(format!("expected {what}, found {other:?}")),
        }
    }

    fn punct(&mut self, expected: char) -> Result<(), String> {
        match self.next() {
            Some(Token::Punct(c)) if *c == expected => Ok(()),
            other => Err(format!("expected {expected:?}, found {other:?}")),
        }
    }

    fn peek_punct(&self, expected: char) -> bool {
        self.tokens.get(self.at) == Some(&Token::Punct(expected))
    }

    /// `NAME = CODE, Carrier, "id", [arguments], "English";`
    fn entry(&mut self) -> Result<SourceTemplate, String> {
        self.ident("a template name")?;
        self.punct('=')?;
        let constant = self.ident("a Code constant")?;
        self.punct(',')?;
        let carrier = self.carrier()?;
        self.punct(',')?;
        let id = self.string("the template id")?;
        self.punct(',')?;
        let arguments = self.arguments()?;
        self.punct(',')?;
        let english = self.string("the English pattern")?;
        self.punct(';')?;
        Ok(SourceTemplate {
            constant,
            carrier,
            id,
            arguments,
            english,
        })
    }

    /// `Diagnostic`, `Unread`, `Read("<Variant>")` or `Write("<Variant>")`,
    /// as the carrier the catalog names.
    fn carrier(&mut self) -> Result<String, String> {
        let token = self.ident("a carrier")?;
        let owner = match token.as_str() {
            "Diagnostic" => return Ok("Diagnostic".to_owned()),
            "Unread" => return Ok("ReadError::Unsupported".to_owned()),
            "Read" => "ReadError",
            "Write" => "WriteError",
            other => return Err(format!("the carrier {other:?} is not one of the table's")),
        };
        self.punct('(')?;
        let variant = self.string("a variant name")?;
        self.punct(')')?;
        if variant.is_empty() || !variant.bytes().all(is_ident_byte) {
            return Err(format!("the variant {variant:?} is not a name"));
        }
        Ok(format!("{owner}::{variant}"))
    }

    /// `[Text("a"), Count("b"), Prose("c")]`, possibly empty.
    fn arguments(&mut self) -> Result<Vec<ArgumentEntry>, String> {
        self.punct('[')?;
        let mut arguments = Vec::new();
        if self.peek_punct(']') {
            self.punct(']')?;
            return Ok(arguments);
        }
        loop {
            let kind = match self.ident("an argument kind")?.as_str() {
                "Text" => "text",
                "Count" => "count",
                "Prose" => "prose",
                other => {
                    return Err(format!(
                        "the argument kind {other:?} is not one of the table's"
                    ));
                }
            };
            self.punct('(')?;
            let name = self.string("an argument name")?;
            self.punct(')')?;
            arguments.push(ArgumentEntry { name, kind });
            if self.peek_punct(']') {
                self.punct(']')?;
                return Ok(arguments);
            }
            self.punct(',')?;
        }
    }
}
