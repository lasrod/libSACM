//! Check that every mutation exclusion excludes exactly its triaged mutants
//! (LIB-QA-010).
//!
//! `.cargo/mutants.toml` may exclude mutants with the string entries of three
//! keys: `exclude_re`, `exclude_globs` and `skip_calls`. Each excluded mutant
//! must have a row in `docs/quality/mutants-triage.md` whose Excluded cell
//! names the entry that excludes it (LIB-QA-007). [`check_exclusions`] shows
//! this from cargo-mutants' own listings, so an entry that is broader than its
//! triage rows, or that has stopped matching, is caught. It compares three
//! kinds of listing:
//!
//! - **(a)** with the configuration;
//! - **(b)** with `--no-config`;
//! - **(`c_e`)** for each entry `e`, with the configuration without `e`.
//!
//! A mutant in (b) but not in (a) is *excluded*. The mutants in `c_e` but not
//! in (a) are `e`'s *own*: the ones that only `e` excludes. Each entry's own
//! mutants must be exactly the mutants of the triage rows that name it.
//!
//! [`check_exclusions`] takes the lister as a parameter, so it can be tested
//! without cargo-mutants; [`cmd_exclusions`] runs it with
//! `cargo mutants --workspace --list` on the repository, as
//! `cargo xtask mutants exclusions`.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::Command;

use crate::matrix::escape_controls;
use crate::output_error;

/// The cargo-mutants configuration, relative to the repository root.
const CONFIG_PATH: &str = ".cargo/mutants.toml";

/// The triage document, relative to the repository root.
const TRIAGE_PATH: &str = "docs/quality/mutants-triage.md";

/// Where [`cmd_exclusions`] writes the configuration copies it lists with,
/// relative to the repository root.
const COPIES_DIR: &str = "target/xtask-mutants";

/// The configuration keys whose string entries exclude mutants.
const EXCLUSION_KEYS: [&str; 3] = ["exclude_re", "exclude_globs", "skip_calls"];

/// The header of the triage table, cell by cell.
const TRIAGE_HEADER: [&str; 5] = ["Mutant", "Disposition", "Row", "Reason", "Excluded"];

/// The dispositions a triage row may have.
const TRIAGE_DISPOSITIONS: [&str; 5] = [
    "killed",
    "equivalent",
    "accepted A",
    "accepted B",
    "accepted C",
];

/// What an Excluded cell holds when the mutant is not excluded.
const NOT_EXCLUDED: [&str; 4] = ["", "—", "-", "no"];

/// The counts [`check_exclusions`] returns, and `cargo xtask mutants
/// exclusions` prints, when every exclusion excludes exactly its triaged
/// mutants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExclusionSummary {
    /// Exclusion entries checked: the strings of `exclude_re`,
    /// `exclude_globs` and `skip_calls`.
    pub entries: usize,
    /// Mutants in the `--no-config` listing but not in the configured one.
    pub excluded: usize,
    /// Mutants in the configured listing.
    pub listed: usize,
}

/// A lister for [`check_exclusions`]: the mutant names cargo-mutants lists,
/// one per element, or the tool's error.
///
/// `list(Some(text))` lists with a configuration file holding `text`;
/// `list(None)` lists with `--no-config`.
pub type Lister<'a> = dyn FnMut(Option<&str>) -> Result<Vec<String>, String> + 'a;

/// Check that each exclusion entry of the cargo-mutants configuration
/// `config` excludes exactly the mutants that the triage document `triage`
/// assigns to it, using the listings of `list`.
///
/// The entries are the strings of the arrays `exclude_re`, `exclude_globs`
/// and `skip_calls`, in that order. `list` is called with:
///
/// - (a) `Some(config)`, the configuration text exactly as given;
/// - (b) `None`, for `--no-config`;
/// - (`c_e`) for each entry `e`, once, the configuration without `e`: the
///   TOML table with that one string removed from its array (an array left
///   empty stays, as an empty array), written by `toml`. Comments and
///   formatting are not kept; every other key and entry is.
///
/// The triage table is read by the LIB-QA-007 rules: exactly one table
/// headed `| Mutant | Disposition | Row | Reason | Excluded |`, then a
/// separator row, then one row per mutant up to the first line that does not
/// start with `|`. A `|` in a cell is written `\|`, and a code span around a
/// cell is not part of it. The Excluded cell is `—`, `-`, `no` or empty if
/// the mutant is not excluded, and otherwise `<key>: <entry>`, with `<key>`
/// one of the three exclusion keys and `<entry>` the entry's exact string.
///
/// Each problem is a `[code] text` string naming the mutant or the entry:
///
/// - `[exclusion-adds]`: a mutant in (a) but not in (b);
/// - `[exclusion-untriaged]`: a mutant in (b) but not in (a) that no triage
///   row assigns to an entry of the configuration;
/// - `[exclusion-mismatch]`: for an entry `e`, a mutant in `c_e` but not in
///   (a) that no triage row assigns to `e` (extra), or a mutant a triage row
///   assigns to `e` that is not (missing). Each is reported once per entry;
/// - `[exclusion-empty]`: an entry whose removal restores no mutant, for
///   example because another entry also excludes everything it does;
/// - `[mutants-tool]`: a listing failed; the problem holds `list`'s error;
/// - `[config-parse]`: `config` is not TOML;
/// - `[config-exclusion]`: an exclusion key is not an array of strings;
/// - `[triage-format]`: the triage table is malformed.
///
/// A configuration or triage table that cannot be read is reported before
/// anything is listed, and nothing is listed. When (a) or (b) fails, no
/// (`c_e`) is listed; when a (`c_e`) fails, that entry alone is not
/// compared. Otherwise every comparison is made and every problem is
/// reported. The summary and the problems depend only on the sets of names
/// listed, not on the order of a listing.
///
/// ```
/// use xtask::mutants::{ExclusionSummary, check_exclusions};
///
/// const RUN: &str = "src/lib.rs:2:5: replace run -> bool with true";
/// const HELP: &str = "src/lib.rs:9:5: replace help -> bool with true";
///
/// let config = "exclude_re = ['^src/lib\\.rs:\\d+:\\d+: replace help -> bool with true$']\n";
/// let triage = "\
/// | Mutant | Disposition | Row | Reason | Excluded |
/// |---|---|---|---|---|
/// | `src/lib.rs:9:5: replace help -> bool with true` | equivalent | LIB-TOOL-001 | No input tells it apart. | `exclude_re: ^src/lib\\.rs:\\d+:\\d+: replace help -> bool with true$` |
/// ";
/// // A stand-in for cargo-mutants: a configuration holding the regex leaves out HELP.
/// let mut list = |config: Option<&str>| -> Result<Vec<String>, String> {
///     let excludes_help = config.is_some_and(|text| text.contains("replace help"));
///     let all = [RUN, HELP].map(str::to_owned);
///     Ok(all.into_iter().filter(|m| !(excludes_help && m == HELP)).collect())
/// };
/// let summary = check_exclusions(config, triage, &mut list).map_err(|p| p.join("\n"))?;
/// assert_eq!(summary, ExclusionSummary { entries: 1, excluded: 1, listed: 1 });
/// # Ok::<(), String>(())
/// ```
///
/// # Errors
///
/// Returns every problem found, sorted, when any exclusion does not exclude
/// exactly its triaged mutants, a listing fails, or an input cannot be read.
pub fn check_exclusions(
    config: &str,
    triage: &str,
    list: &mut Lister<'_>,
) -> Result<ExclusionSummary, Vec<String>> {
    let mut problems = Vec::new();
    let (table, entries) = match toml::from_str::<toml::Table>(config) {
        Ok(table) => {
            let entries = exclusion_entries(&table, &mut problems);
            (Some(table), entries)
        }
        Err(error) => {
            problems.push(problem("config-parse", parse_error(config, &error)));
            (None, Vec::new())
        }
    };
    let assignments = triage_assignments(triage, &mut problems);
    let Some(table) = table.filter(|_| problems.is_empty()) else {
        return Err(sorted(problems));
    };

    let configured = list(Some(config));
    let unconfigured = list(None);
    let (configured, unconfigured) = match (configured, unconfigured) {
        (Ok(configured), Ok(unconfigured)) => (names(configured), names(unconfigured)),
        (configured, unconfigured) => {
            if let Err(error) = configured {
                problems.push(problem(
                    "mutants-tool",
                    format!("listing (a), with the configuration, failed: {error}"),
                ));
            }
            if let Err(error) = unconfigured {
                problems.push(problem(
                    "mutants-tool",
                    format!("listing (b), with --no-config, failed: {error}"),
                ));
            }
            return Err(sorted(problems));
        }
    };

    for added in configured.difference(&unconfigured) {
        problems.push(problem(
            "exclusion-adds",
            format!("`{added}` is listed with the configuration but not with --no-config"),
        ));
    }
    let excluded: BTreeSet<&String> = unconfigured.difference(&configured).collect();
    let configured_entries: BTreeSet<(&str, &str)> =
        entries.iter().map(|e| (e.key, e.value.as_str())).collect();
    let triaged: BTreeSet<&str> = assignments
        .iter()
        .filter(|a| configured_entries.contains(&(a.key.as_str(), a.entry.as_str())))
        .map(|a| a.mutant.as_str())
        .collect();
    for mutant in &excluded {
        if !triaged.contains(mutant.as_str()) {
            problems.push(problem(
                "exclusion-untriaged",
                format!(
                    "`{mutant}` is excluded, but no triage row assigns it to an entry of the configuration"
                ),
            ));
        }
    }

    for entry in &entries {
        let listing = match listing_without(&table, entry, list) {
            Ok(listing) => listing,
            Err(fault) => {
                problems.push(fault);
                continue;
            }
        };
        let own: BTreeSet<&str> = listing
            .difference(&configured)
            .map(String::as_str)
            .collect();
        entry_problems(entry, &own, &assignments, &mut problems);
    }

    if problems.is_empty() {
        Ok(ExclusionSummary {
            entries: entries.len(),
            excluded: excluded.len(),
            listed: configured.len(),
        })
    } else {
        Err(sorted(problems))
    }
}

/// The problems of `entry`, whose own mutants (in its `c_e` but not in (a))
/// are `own`: `[exclusion-empty]` if it has none, and `[exclusion-mismatch]`
/// for each own mutant no triage row assigns to it and each mutant a triage
/// row assigns to it that is not its own.
fn entry_problems(
    entry: &Entry,
    own: &BTreeSet<&str>,
    assignments: &[Assignment],
    problems: &mut Vec<String>,
) {
    let named = entry.named();
    if own.is_empty() {
        problems.push(problem(
            "exclusion-empty",
            format!("{named} excludes no mutant of its own: removing it restores none"),
        ));
    }
    let assigned: BTreeMap<&str, usize> = assignments
        .iter()
        .filter(|a| a.key == entry.key && a.entry == entry.value)
        .map(|a| (a.mutant.as_str(), a.line))
        .collect();
    for mutant in own {
        if !assigned.contains_key(mutant) {
            problems.push(problem(
                "exclusion-mismatch",
                format!("{named} excludes `{mutant}`, which no triage row assigns to it"),
            ));
        }
    }
    for (mutant, line) in &assigned {
        if !own.contains(mutant) {
            problems.push(problem(
                "exclusion-mismatch",
                format!(
                    "{named} does not exclude `{mutant}`, which triage line {line} assigns to it"
                ),
            ));
        }
    }
}

/// `cargo xtask mutants exclusions`: run [`check_exclusions`] on the
/// repository at `root`, listing with cargo-mutants.
///
/// Reads `.cargo/mutants.toml` and `docs/quality/mutants-triage.md` under
/// `root`, and lists with `cargo mutants --workspace --list --colors never`,
/// run from `root` with the cargo of the `CARGO` environment variable (else
/// `cargo` from `PATH`), with `NO_COLOR=1` and without `CLICOLOR_FORCE`, so
/// that no colour code reaches a listing. A listing line that still holds a
/// control character is a `[mutants-tool]` problem, never a mutant name.
/// A listing with a configuration adds `--config <file>`, naming a
/// copy of that configuration written under `target/xtask-mutants/`; the
/// `--no-config` listing adds `--no-config`. The directory is emptied first,
/// so it holds this run's copies only, `config-1.toml` being (a).
///
/// On success, writes `mutant exclusions OK: <entries> entries, <excluded>
/// excluded mutants, <listed> mutants listed` to `out`. On failure, writes
/// nothing.
///
/// # Errors
///
/// Returns an error when a file cannot be read, when writing to `out`
/// fails, or when [`check_exclusions`] finds problems. Then the first line is
/// `mutant exclusions: <n> problem(s)`, followed by each problem on a line of
/// its own, indented by two spaces, with control characters escaped. A
/// missing cargo-mutants, or a listing that exits non-zero, is a
/// `[mutants-tool]` problem naming the command and holding its stderr.
pub fn cmd_exclusions(root: &Path, out: &mut dyn Write) -> Result<(), String> {
    let program = std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
    exclusions_with(root, &program, out)
}

/// [`cmd_exclusions`] with `program` as cargo.
fn exclusions_with(root: &Path, program: &OsStr, out: &mut dyn Write) -> Result<(), String> {
    let config = read(root, CONFIG_PATH)?;
    let triage = read(root, TRIAGE_PATH)?;
    // Only this run's copies stay, numbered in the order they were listed.
    let copies_dir = root.join(COPIES_DIR);
    match fs::remove_dir_all(&copies_dir) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
            return Err(escape_controls(&format!(
                "remove {}: {e}",
                copies_dir.display()
            )));
        }
        _ => {}
    }
    let mut copies = 0_usize;
    let mut list = |config: Option<&str>| {
        copies += 1;
        cargo_mutants_list(root, program, config, copies)
    };
    match check_exclusions(&config, &triage, &mut list) {
        Ok(summary) => writeln!(
            out,
            "mutant exclusions OK: {} entries, {} excluded mutants, {} mutants listed",
            summary.entries, summary.excluded, summary.listed
        )
        .map_err(output_error),
        Err(problems) => {
            let mut report = format!("mutant exclusions: {} problem(s)", problems.len());
            for problem in &problems {
                report.push_str("\n  ");
                report.push_str(&escape_controls(problem));
            }
            Err(report)
        }
    }
}

/// The text of the file at `path` under `root`.
fn read(root: &Path, path: &str) -> Result<String, String> {
    let full = root.join(path);
    fs::read_to_string(&full).map_err(|e| escape_controls(&format!("read {}: {e}", full.display())))
}

/// List the workspace's mutants with cargo-mutants, run from `root` with
/// `program`: with `--no-config` for `None`, otherwise with `--config` naming
/// copy number `copy` of the configuration, written under [`COPIES_DIR`].
fn cargo_mutants_list(
    root: &Path,
    program: &OsStr,
    config: Option<&str>,
    copy: usize,
) -> Result<Vec<String>, String> {
    let copy = match config {
        None => None,
        Some(text) => {
            let dir = root.join(COPIES_DIR);
            fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
            let name = format!("config-{copy}.toml");
            let file = dir.join(&name);
            fs::write(&file, text).map_err(|e| format!("write {}: {e}", file.display()))?;
            Some(format!("{COPIES_DIR}/{name}"))
        }
    };
    run_listing(listing_command(root, program, copy.as_deref()))
}

/// The cargo-mutants command that lists the workspace's mutants: `program`
/// run from `root` as `mutants --workspace --list --colors never`, followed
/// by `--no-config` for `None` or `--config <copy>` for `Some(copy)`.
///
/// Colour is off whatever the environment says: `--colors never`, and
/// `NO_COLOR=1` with `CLICOLOR_FORCE` removed, because `CLICOLOR_FORCE`
/// overrides `--colors never` and CI sets `CARGO_TERM_COLOR=always`.
fn listing_command(root: &Path, program: &OsStr, copy: Option<&str>) -> Command {
    let mut command = Command::new(program);
    command
        .args(["mutants", "--workspace", "--list", "--colors", "never"])
        .current_dir(root)
        .env("NO_COLOR", "1")
        .env_remove("CLICOLOR_FORCE");
    match copy {
        None => command.arg("--no-config"),
        Some(copy) => command.args(["--config", copy]),
    };
    command
}

/// `command` as problems name it: the program and its arguments, in
/// backticks.
fn command_text(command: &Command) -> String {
    let mut text = Path::new(command.get_program()).display().to_string();
    for arg in command.get_args() {
        text.push(' ');
        text.push_str(&arg.to_string_lossy());
    }
    escape_controls(&format!("`{text}`"))
}

/// The listing `command` prints on standard output ([`listing_lines`]); an
/// error naming the command if it cannot be run, exits non-zero (with its
/// stderr), or prints something that is not UTF-8.
fn run_listing(mut command: Command) -> Result<Vec<String>, String> {
    let text = command_text(&command);
    let output = command
        .output()
        .map_err(|e| format!("could not run {text}: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "{text} failed ({}); stderr: {}",
            output.status,
            stderr.trim()
        ));
    }
    let stdout = String::from_utf8(output.stdout)
        .map_err(|e| format!("{text} printed a listing that is not UTF-8: {e}"))?;
    listing_lines(&text, &stdout)
}

/// The mutant names in the standard output `stdout` of `command`: its
/// non-empty lines. A line holding a control character, such as the ESC of
/// a colour code or a lone CR, is an error naming the command: such a line
/// is never compared as a name, nor silently cleaned.
fn listing_lines(command: &str, stdout: &str) -> Result<Vec<String>, String> {
    let mut names = Vec::new();
    for (index, line) in stdout.lines().enumerate() {
        if line.chars().any(char::is_control) {
            return Err(format!(
                "{command} printed line {} with a control character, which is no mutant name: `{}`",
                index + 1,
                escape_controls(line)
            ));
        }
        if !line.trim().is_empty() {
            names.push(line.to_owned());
        }
    }
    Ok(names)
}

/// One exclusion entry of the configuration: string `index` of the array
/// `key`.
#[derive(Debug)]
struct Entry {
    key: &'static str,
    index: usize,
    value: String,
}

impl Entry {
    /// The entry as problems name it.
    fn named(&self) -> String {
        format!("`{}` entry `{}`", self.key, self.value)
    }
}

/// The exclusion entries of `config`, key by key in [`EXCLUSION_KEYS`]
/// order; `[config-exclusion]` for a key that is not an array of strings.
fn exclusion_entries(config: &toml::Table, problems: &mut Vec<String>) -> Vec<Entry> {
    let mut entries = Vec::new();
    for key in EXCLUSION_KEYS {
        match config.get(key) {
            None => {}
            Some(toml::Value::Array(items)) if items.iter().all(toml::Value::is_str) => {
                entries.extend(items.iter().enumerate().filter_map(|(index, item)| {
                    item.as_str().map(|value| Entry {
                        key,
                        index,
                        value: value.to_owned(),
                    })
                }));
            }
            Some(other) => problems.push(problem(
                "config-exclusion",
                format!("`{key}` must be an array of strings, found `{other}`"),
            )),
        }
    }
    entries
}

/// The listing (`c_e`) of the configuration `config` without `entry`, as a
/// set; a `[config-parse]` or `[mutants-tool]` problem if it cannot be had.
fn listing_without(
    config: &toml::Table,
    entry: &Entry,
    list: &mut Lister<'_>,
) -> Result<BTreeSet<String>, String> {
    let mut without = config.clone();
    if let Some(toml::Value::Array(items)) = without.get_mut(entry.key) {
        *items = items
            .iter()
            .enumerate()
            .filter(|(index, _)| *index != entry.index)
            .map(|(_, item)| item.clone())
            .collect();
    }
    let named = entry.named();
    let text = toml::to_string(&without).map_err(|e| {
        problem(
            "config-parse",
            format!("the configuration without {named} cannot be written as TOML: {e}"),
        )
    })?;
    list(Some(&text)).map(names).map_err(|error| {
        problem(
            "mutants-tool",
            format!("listing (c), without {named}, failed: {error}"),
        )
    })
}

/// The mutant names of a listing, as a set.
fn names(listing: Vec<String>) -> BTreeSet<String> {
    listing.into_iter().collect()
}

/// A problem string: `[code] text`.
fn problem(code: &str, text: impl std::fmt::Display) -> String {
    format!("[{code}] {text}")
}

/// `problems`, sorted.
fn sorted(mut problems: Vec<String>) -> Vec<String> {
    problems.sort();
    problems
}

/// Why `text` is not TOML, on one line: the position of the fault (line and
/// column, from 1) and the parser's message.
fn parse_error(text: &str, error: &toml::de::Error) -> String {
    let message = error.message().trim().replace('\n', " ");
    let Some(span) = error.span() else {
        return format!("the configuration is not TOML: {message}");
    };
    let before = text.get(..span.start).unwrap_or(text);
    let line = before.matches('\n').count() + 1;
    let column = before.rsplit('\n').next().map_or(0, |l| l.chars().count()) + 1;
    format!("the configuration is not TOML: line {line}, column {column}: {message}")
}

/// A triage row that assigns a mutant to an exclusion entry.
#[derive(Debug)]
struct Assignment {
    /// The row's line number in the triage document, from 1.
    line: usize,
    mutant: String,
    key: String,
    entry: String,
}

/// The triage table's rows whose Excluded cell names an entry;
/// `[triage-format]` for anything malformed (the LIB-QA-007 rules, see
/// [`check_exclusions`]).
fn triage_assignments(triage: &str, problems: &mut Vec<String>) -> Vec<Assignment> {
    let lines: Vec<&str> = triage.lines().collect();
    let headers: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| {
            line.trim_start().starts_with('|') && table_cells(line) == TRIAGE_HEADER
        })
        .map(|(index, _)| index)
        .collect();
    let [header] = headers.as_slice() else {
        problems.push(problem(
            "triage-format",
            format!(
                "the triage document must have exactly one table headed `| {} |`, found {}",
                TRIAGE_HEADER.join(" | "),
                headers.len()
            ),
        ));
        return Vec::new();
    };
    let separator = lines.get(header + 1).map(|line| table_cells(line));
    if !separator.is_some_and(|cells| {
        cells.len() == TRIAGE_HEADER.len()
            && cells
                .iter()
                .all(|c| !c.is_empty() && c.chars().all(|ch| ch == '-' || ch == ':'))
    }) {
        problems.push(problem(
            "triage-format",
            "the triage table has no separator row",
        ));
    }
    let mut assignments = Vec::new();
    for (index, line) in lines.iter().enumerate().skip(header + 2) {
        if !line.trim_start().starts_with('|') {
            break;
        }
        let number = index + 1;
        let cells = table_cells(line);
        let [mutant, disposition, _, _, excluded] = cells.as_slice() else {
            problems.push(problem(
                "triage-format",
                format!(
                    "triage line {number} must have 5 cells, found {}",
                    cells.len()
                ),
            ));
            continue;
        };
        if !TRIAGE_DISPOSITIONS.contains(&disposition.as_str()) {
            problems.push(problem(
                "triage-format",
                format!(
                    "triage line {number}: unknown disposition `{disposition}`; allowed: {TRIAGE_DISPOSITIONS:?}"
                ),
            ));
        }
        let excluded = code_span_text(excluded);
        if NOT_EXCLUDED.contains(&excluded) {
            continue;
        }
        match excluded
            .split_once(": ")
            .filter(|(key, _)| EXCLUSION_KEYS.contains(key))
        {
            Some((key, entry)) => assignments.push(Assignment {
                line: number,
                mutant: code_span_text(mutant).to_owned(),
                key: key.to_owned(),
                entry: entry.to_owned(),
            }),
            None => problems.push(problem(
                "triage-format",
                format!(
                    "triage line {number}: the Excluded cell must be `—` or `<key>: <entry>` with a key in {EXCLUSION_KEYS:?}, found `{excluded}`"
                ),
            )),
        }
    }
    assignments
}

/// The cells of a Markdown table row, split on `|` not preceded by `\`, with
/// `\|` read as `|` and each cell trimmed.
fn table_cells(line: &str) -> Vec<String> {
    let line = line.trim();
    let line = line.strip_prefix('|').unwrap_or(line);
    let line = match line.strip_suffix('|') {
        Some(rest) if !rest.ends_with('\\') => rest,
        _ => line,
    };
    let mut cells = vec![String::new()];
    let mut previous = None;
    for c in line.chars() {
        if c == '|' && previous != Some('\\') {
            cells.push(String::new());
        } else if let Some(cell) = cells.last_mut() {
            cell.push(c);
        }
        previous = Some(c);
    }
    cells
        .into_iter()
        .map(|cell| cell.replace("\\|", "|").trim().to_owned())
        .collect()
}

/// The text of a cell without a surrounding code span.
fn code_span_text(cell: &str) -> &str {
    let ticks = cell.len() - cell.trim_start_matches('`').len();
    let fence = "`".repeat(ticks);
    // A cell of backticks only has no closing fence after its opening one.
    let inner = cell
        .strip_prefix(fence.as_str())
        .and_then(|rest| rest.strip_suffix(fence.as_str()));
    let Some(inner) = inner.filter(|_| ticks > 0) else {
        return cell;
    };
    match inner.strip_prefix(' ').and_then(|i| i.strip_suffix(' ')) {
        Some(stripped) if !stripped.trim().is_empty() => stripped,
        _ => inner,
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::{OsStr, OsString};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use super::{
        COPIES_DIR, cargo_mutants_list, cmd_exclusions, code_span_text, command_text,
        exclusions_with, listing_command, listing_lines, parse_error, run_listing, table_cells,
        triage_assignments,
    };

    /// A program that does not exist.
    const NO_PROGRAM: &str = "xtask-no-such-program";

    /// A directory under the system temp directory, removed on drop.
    struct TempRoot(PathBuf);

    impl TempRoot {
        fn new(name: &str) -> Self {
            let path =
                std::env::temp_dir().join(format!("xtask-mutants-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("the temp root can be created");
            Self(path)
        }

        fn write(&self, path: &str, text: &str) {
            let full = self.0.join(path);
            if let Some(parent) = full.parent() {
                fs::create_dir_all(parent).expect("the parent can be created");
            }
            fs::write(full, text).expect("the file can be written");
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn cargo() -> OsString {
        std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"))
    }

    /// `cargo <arg>`, run from `root`.
    fn cargo_with(root: &Path, arg: &str) -> Command {
        let mut command = Command::new(cargo());
        command.arg(arg).current_dir(root);
        command
    }

    /// The listing flags every listing has.
    const LIST: [&str; 5] = ["mutants", "--workspace", "--list", "--colors", "never"];

    #[test]
    fn a_listing_command_is_colourless_and_runs_from_the_root() {
        let root = Path::new("some/root");
        for (copy, tail) in [
            (None, vec!["--no-config"]),
            (
                Some("target/xtask-mutants/config-2.toml"),
                vec!["--config", "target/xtask-mutants/config-2.toml"],
            ),
        ] {
            let command = listing_command(root, OsStr::new("my-cargo"), copy);
            assert_eq!(command.get_program(), OsStr::new("my-cargo"));
            let args: Vec<&OsStr> = command.get_args().collect();
            let want: Vec<&OsStr> = LIST.iter().chain(&tail).map(OsStr::new).collect();
            assert_eq!(args, want, "{copy:?}");
            let mut envs: Vec<(&OsStr, Option<&OsStr>)> = command.get_envs().collect();
            envs.sort();
            assert_eq!(
                envs,
                vec![
                    (OsStr::new("CLICOLOR_FORCE"), None),
                    (OsStr::new("NO_COLOR"), Some(OsStr::new("1"))),
                ],
                "{copy:?}"
            );
            assert_eq!(command.get_current_dir(), Some(root), "{copy:?}");
        }
    }

    #[test]
    fn a_listing_line_with_a_control_character_is_an_error() {
        let command = "`cargo mutants --workspace --list`";
        assert_eq!(
            listing_lines(command, "a.rs:1:1: replace x\n\nb.rs:2:2: delete !\r\n"),
            Ok(vec![
                "a.rs:1:1: replace x".to_owned(),
                "b.rs:2:2: delete !".to_owned()
            ])
        );
        for (stdout, line, shown) in [
            (
                "a.rs:1:1: x\n\u{1b}[32mb.rs:2:2: y\u{1b}[0m\n",
                2,
                r"\u{1b}[32mb.rs:2:2: y\u{1b}[0m",
            ),
            ("a.rs:1:1: x\rb.rs:2:2: y\n", 1, r"a.rs:1:1: x\rb.rs:2:2: y"),
            ("a.rs:1:1: x\r", 1, r"a.rs:1:1: x\r"),
        ] {
            assert_eq!(
                listing_lines(command, stdout),
                Err(format!(
                    "{command} printed line {line} with a control character, which is no mutant name: `{shown}`"
                )),
                "{stdout:?}"
            );
        }
    }

    #[test]
    fn a_listing_is_the_lines_the_program_prints() {
        let root = TempRoot::new("listing");
        let lines = run_listing(cargo_with(&root.0, "--version")).expect("cargo runs");
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert!(lines[0].starts_with("cargo "), "{lines:?}");
    }

    #[test]
    fn a_listing_whose_output_holds_a_colour_code_is_an_error() {
        let root = TempRoot::new("colour");
        let mut command = Command::new(cargo());
        command
            .args(["--color", "always", "--list"])
            .current_dir(&root.0);
        let text = command_text(&command);
        let error = run_listing(command).expect_err("cargo prints colour codes");
        assert!(
            error.starts_with(&format!("{text} printed line "))
                && error.contains("with a control character, which is no mutant name"),
            "{error}"
        );
    }

    #[test]
    fn a_listing_that_exits_non_zero_is_an_error_with_its_stderr() {
        let root = TempRoot::new("non-zero");
        let error = run_listing(cargo_with(&root.0, "no-such-subcommand-for-xtask-tests"))
            .expect_err("an unknown cargo command fails");
        assert!(
            error.contains(" no-such-subcommand-for-xtask-tests` failed (")
                && error.contains("; stderr: ")
                && error.contains("no such command"),
            "{error}"
        );
    }

    #[test]
    fn a_listing_names_the_command_it_could_not_run() {
        let root = TempRoot::new("no-program");
        let error = cargo_mutants_list(&root.0, OsStr::new(NO_PROGRAM), None, 1)
            .expect_err("the program does not exist");
        let command = format!(
            "could not run `{NO_PROGRAM} mutants --workspace --list --colors never --no-config`: "
        );
        assert!(error.starts_with(&command), "{error}");

        let error = cargo_mutants_list(&root.0, OsStr::new(NO_PROGRAM), Some("a = 1\n"), 3)
            .expect_err("the program does not exist");
        let command = format!(
            "could not run `{NO_PROGRAM} mutants --workspace --list --colors never --config {COPIES_DIR}/config-3.toml`: "
        );
        assert!(error.starts_with(&command), "{error}");
        let copy = root.0.join(COPIES_DIR).join("config-3.toml");
        assert_eq!(
            fs::read_to_string(copy).expect("the copy exists"),
            "a = 1\n"
        );
    }

    #[test]
    fn the_command_reports_each_failed_listing_and_keeps_only_its_own_copies() {
        let root = TempRoot::new("command");
        root.write(".cargo/mutants.toml", "cap_lints = true\n");
        root.write(
            "docs/quality/mutants-triage.md",
            "| Mutant | Disposition | Row | Reason | Excluded |\n|---|---|---|---|---|\n",
        );
        let copies = root.0.join(COPIES_DIR);
        for stale in [false, true] {
            if stale {
                root.write(&format!("{COPIES_DIR}/config-9.toml"), "stale = true\n");
            }
            let mut out = Vec::new();
            let error = exclusions_with(&root.0, OsStr::new(NO_PROGRAM), &mut out)
                .expect_err("no listing can be made");
            let lines: Vec<&str> = error.lines().collect();
            assert_eq!(lines.len(), 3, "{error}");
            assert_eq!(lines[0], "mutant exclusions: 2 problem(s)");
            let a = format!(
                "  [mutants-tool] listing (a), with the configuration, failed: could not run `{NO_PROGRAM} mutants --workspace --list --colors never --config {COPIES_DIR}/config-1.toml`: "
            );
            let b = format!(
                "  [mutants-tool] listing (b), with --no-config, failed: could not run `{NO_PROGRAM} mutants --workspace --list --colors never --no-config`: "
            );
            assert!(lines[1].starts_with(&a), "{error}");
            assert!(lines[2].starts_with(&b), "{error}");
            assert_eq!(out, Vec::<u8>::new(), "a failure writes nothing");
            assert!(!copies.join("config-9.toml").exists(), "stale copies go");
            assert_eq!(
                fs::read_to_string(copies.join("config-1.toml")).expect("(a) is copied"),
                "cap_lints = true\n"
            );
        }
    }

    #[test]
    fn the_command_reports_a_copies_directory_it_cannot_empty() {
        let root = TempRoot::new("blocked");
        root.write(".cargo/mutants.toml", "cap_lints = true\n");
        root.write("docs/quality/mutants-triage.md", "");
        root.write(COPIES_DIR, "a file where the copies go\n");
        let error = exclusions_with(&root.0, OsStr::new(NO_PROGRAM), &mut Vec::new())
            .expect_err("the copies directory is a file");
        assert!(error.starts_with("remove "), "{error}");
        assert!(!error.contains('\n'), "{error}");
    }

    #[test]
    fn the_command_reports_a_configuration_it_cannot_read() {
        let error = cmd_exclusions(Path::new("no-such-xtask-root"), &mut Vec::new())
            .expect_err("there is no configuration");
        assert!(error.starts_with("read no-such-xtask-root"), "{error}");
        assert!(error.contains("mutants.toml"), "{error}");
    }

    #[test]
    fn cells_split_on_unescaped_bars_only() {
        assert_eq!(
            table_cells(r"| `a \|\| b` | c |"),
            vec!["`a || b`".to_owned(), "c".to_owned()]
        );
        assert_eq!(table_cells("|---|:-:|"), vec!["---", ":-:"]);
        assert_eq!(table_cells(r"| a | b \|"), vec!["a", "b |"]);
    }

    #[test]
    fn a_separator_may_align_and_problems_name_their_line() {
        let triage = "Intro.\n\n| Mutant | Disposition | Row | Reason | Excluded |\n|:---|---:|:-:|---|---|\n| `m` | killed | LIB-TOOL-001 | A test. | maybe |\n| `n` | equivalent | LIB-TOOL-001 | None. | `skip_calls: f` |\n";
        let mut problems = Vec::new();
        let assignments = triage_assignments(triage, &mut problems);
        assert_eq!(
            problems,
            vec![
                "[triage-format] triage line 5: the Excluded cell must be `—` or `<key>: <entry>` with a key in [\"exclude_re\", \"exclude_globs\", \"skip_calls\"], found `maybe`"
                    .to_owned()
            ]
        );
        let [assignment] = assignments.as_slice() else {
            panic!("one row names an entry: {assignments:?}");
        };
        assert_eq!(
            (
                assignment.line,
                assignment.mutant.as_str(),
                assignment.key.as_str(),
                assignment.entry.as_str()
            ),
            (6, "n", "skip_calls", "f")
        );
    }

    #[test]
    fn code_spans_are_unwrapped() {
        assert_eq!(code_span_text("`x`"), "x");
        assert_eq!(code_span_text("`` `x` ``"), "`x`");
        assert_eq!(code_span_text("`"), "`");
        assert_eq!(code_span_text("``"), "``");
        assert_eq!(code_span_text("` `"), " ");
        assert_eq!(code_span_text("`  `"), "  ");
        assert_eq!(code_span_text("` x `"), "x");
        assert_eq!(code_span_text(" x "), " x ");
        assert_eq!(code_span_text("`x"), "`x");
        assert_eq!(code_span_text("plain"), "plain");
    }

    #[test]
    fn a_parse_error_names_its_line_and_column() {
        let text = "a = 1\nb =\n";
        let Err(error) = toml::from_str::<toml::Table>(text) else {
            panic!("the text is not TOML");
        };
        let message = parse_error(text, &error);
        assert!(
            message.starts_with("the configuration is not TOML: line 2, column 4: "),
            "{message}"
        );
        assert!(!message.contains('\n'), "{message}");
    }
}
