//! Acceptance tests for LIB-QA-010: every exclusion in `.cargo/mutants.toml`
//! excludes exactly the mutants that `docs/quality/mutants-triage.md` assigns
//! to it, and nothing else.
//!
//! The comparison is `xtask::mutants::check_exclusions` (slice 0010, D1), a
//! function over the configuration text, the triage text and a lister. These
//! tests drive it with a fake lister, so they need neither cargo-mutants nor
//! the real repository: nothing here reads a file.
//!
//! # The fake lister
//!
//! [`Fake`] models what `cargo mutants --workspace --list` prints for a
//! configuration. It holds the eleven mutants of [`UNIVERSE`], and parses the
//! configuration it is given with `toml`:
//!
//! - each `exclude_re` entry removes the names it matches. The fake reads the
//!   regex form LIB-QA-007 accepts: `^`, escaped literal text, `\d+`, `$`;
//! - each `exclude_globs` entry removes the mutants of the file it names;
//! - each `skip_calls` entry removes the mutants inside a call to it;
//! - `skip_calls_defaults`, true unless the configuration says `false`,
//!   removes the mutants inside a call to `with_capacity`;
//! - `examine_globs`, if present, keeps only the mutants of the files it
//!   names. It is not an exclusion entry, so it excludes mutants that no
//!   entry accounts for.
//!
//! `None` (`--no-config`) is the empty configuration, and so is a text that
//! does not parse. Every call is recorded, so a test can check that the
//! function asks for (a) the configuration, (b) `--no-config` and (c) one
//! copy per entry without that entry. A fake can also fail the listings a
//! predicate picks, as cargo-mutants does when it is missing or exits
//! non-zero.
//!
//! # What the rules give for an overlapping pair
//!
//! Each rule of acceptance item 2 is applied on its own, as written. When two
//! entries both exclude a mutant `m`, the configuration without either one
//! still excludes `m`, so `m` is in neither entry's own set (`c_e` minus a).
//! The entry left with nothing of its own is `[exclusion-empty]`. And the
//! triage row of `m` names one of the two entries, whose own set lacks `m`:
//! that is `[exclusion-mismatch]` for `m` and that entry. So an overlapping
//! pair gives exactly one `[exclusion-empty]` and one `[exclusion-mismatch]`
//! (two `[exclusion-empty]` when both entries exclude the same mutants and
//! nothing else).
//!
//! A mismatched mutant is reported once for each entry it is mismatched
//! with. A mutant that an entry excludes but no triage row assigns to any
//! entry is both `[exclusion-untriaged]` and, for that entry, an extra
//! `[exclusion-mismatch]`.

use xtask::mutants::{ExclusionSummary, check_exclusions};

const QA10: &str = "LIB-QA-010";

// ---------------------------------------------------------------------------
// The modelled workspace
// ---------------------------------------------------------------------------

/// A mutant of the fake listing: its name (one line of `--list`), the file
/// it is in, and the function whose call it sits inside, if any.
struct Mutant {
    name: &'static str,
    file: &'static str,
    call: Option<&'static str>,
}

const REPO_ROOT: &str =
    "xtask/src/lib.rs:20:5: replace repo_root -> PathBuf with Default::default()";
const RUN_FROM: &str = "xtask/src/lib.rs:31:9: replace == with != in run_from";
const EXAMPLE_TRUE: &str = "xtask/src/fetch.rs:10:5: replace example -> bool with true";
const EXAMPLE_FALSE: &str = "xtask/src/fetch.rs:10:5: replace example -> bool with false";
const FETCH_OR: &str = "xtask/src/fetch.rs:44:17: replace || with && in fetch_into";
const MAIN_DEFAULT: &str =
    "xtask/src/main.rs:12:5: replace main -> ExitCode with Default::default()";
const MAIN_NOT: &str = "xtask/src/main.rs:18:20: delete ! in main";
const LOAD_MINUS: &str = "xtask/src/matrix.rs:100:30: replace + with - in load";
const LOAD_TIMES: &str = "xtask/src/matrix.rs:100:30: replace + with * in load";
const READY_TRUE: &str = "xtask/src/matrix.rs:200:5: replace ready -> bool with true";
const RENDER_CAPACITY: &str = "xtask/src/matrix.rs:300:40: replace * with + in render";

/// A mutant name in no listing: the line of a mutant that has moved.
const MOVED_MUTANT: &str = "xtask/src/main.rs:99:5: replace gone -> bool with true";

const LIB_RS: &str = "xtask/src/lib.rs";
const FETCH_RS: &str = "xtask/src/fetch.rs";
const MAIN_RS: &str = "xtask/src/main.rs";
const MATRIX_RS: &str = "xtask/src/matrix.rs";

const UNIVERSE: [Mutant; 11] = [
    Mutant {
        name: REPO_ROOT,
        file: LIB_RS,
        call: None,
    },
    Mutant {
        name: RUN_FROM,
        file: LIB_RS,
        call: None,
    },
    Mutant {
        name: EXAMPLE_TRUE,
        file: FETCH_RS,
        call: None,
    },
    Mutant {
        name: EXAMPLE_FALSE,
        file: FETCH_RS,
        call: None,
    },
    Mutant {
        name: FETCH_OR,
        file: FETCH_RS,
        call: None,
    },
    Mutant {
        name: MAIN_DEFAULT,
        file: MAIN_RS,
        call: None,
    },
    Mutant {
        name: MAIN_NOT,
        file: MAIN_RS,
        call: None,
    },
    Mutant {
        name: LOAD_MINUS,
        file: MATRIX_RS,
        call: Some(EXAMPLE_CALL),
    },
    Mutant {
        name: LOAD_TIMES,
        file: MATRIX_RS,
        call: Some(EXAMPLE_CALL),
    },
    Mutant {
        name: READY_TRUE,
        file: MATRIX_RS,
        call: None,
    },
    Mutant {
        name: RENDER_CAPACITY,
        file: MATRIX_RS,
        call: Some("with_capacity"),
    },
];

/// Mutants in the `--no-config` listing: all but [`RENDER_CAPACITY`], which
/// cargo-mutants' default `skip_calls_defaults = true` skips.
const NO_CONFIG_LISTED: usize = 10;

// ---------------------------------------------------------------------------
// Exclusion entries
// ---------------------------------------------------------------------------

/// Matches [`EXAMPLE_TRUE`] only.
const RE_EXAMPLE: &str = r"^xtask/src/fetch\.rs:\d+:\d+: replace example -> bool with true$";
/// Matches [`EXAMPLE_TRUE`] only, at its exact position.
const RE_EXAMPLE_AT: &str = r"^xtask/src/fetch\.rs:10:5: replace example -> bool with true$";
/// Matches [`FETCH_OR`] only; its `|` are escaped.
const RE_FETCH_OR: &str = r"^xtask/src/fetch\.rs:\d+:\d+: replace \|\| with && in fetch_into$";
/// Matches [`MAIN_NOT`] only, which [`GLOB_MAIN`] also excludes.
const RE_MAIN_NOT: &str = r"^xtask/src/main\.rs:\d+:\d+: delete ! in main$";
/// Matches nothing.
const RE_GONE: &str = r"^xtask/src/fetch\.rs:\d+:\d+: replace gone -> bool with true$";
/// Excludes [`MAIN_DEFAULT`] and [`MAIN_NOT`].
const GLOB_MAIN: &str = "xtask/src/main.rs";
/// Excludes [`LOAD_MINUS`] and [`LOAD_TIMES`].
const EXAMPLE_CALL: &str = "example_call";
/// Excludes nothing: no mutant is inside a call to it.
const NEVER_CALLED: &str = "never_called";

/// A configuration with the LIB-QA-007 keys and no exclusions.
const CONFIG_BASE: &str = "# cargo-mutants configuration (LIB-QA-007).

# A lint that `-D warnings` denies must not turn a mutant unviable (D5).
cap_lints = true

# Build and test with every feature, as CI does (D6).
all_features = true

# A mutant must be killed by its own package's tests, not another package's (D8).
test_workspace = false
";

/// One TOML array of literal strings.
fn array(key: &str, entries: &[&str]) -> String {
    let items: Vec<String> = entries.iter().map(|e| format!("'{e}'")).collect();
    format!("{key} = [{}]\n", items.join(", "))
}

/// [`CONFIG_BASE`] with these exclusion arrays (an empty slice: no key).
fn config(re: &[&str], globs: &[&str], calls: &[&str]) -> String {
    let mut text = CONFIG_BASE.to_owned();
    for (key, entries) in [
        ("exclude_re", re),
        ("exclude_globs", globs),
        ("skip_calls", calls),
    ] {
        if !entries.is_empty() {
            text.push_str(&array(key, entries));
        }
    }
    text
}

/// The correct configuration: one entry of each kind, each excluding exactly
/// what [`correct_rows`] assigns to it.
fn correct_config() -> String {
    config(&[RE_EXAMPLE], &[GLOB_MAIN], &[EXAMPLE_CALL])
}

// ---------------------------------------------------------------------------
// Triage documents
// ---------------------------------------------------------------------------

const TRIAGE_HEAD: &str = "# Mutation-testing triage

- **Configuration in force:** `skip_calls_defaults = true` is the cargo-mutants default.

| Mutant | Disposition | Row | Reason | Excluded |
|---|---|---|---|---|
";

/// A killed mutant: not excluded, as most triage rows are.
fn killed(mutant: &str) -> String {
    format!(
        "| `{}` | killed | LIB-TOOL-001 | A test kills it. | — |\n",
        cell(mutant)
    )
}

/// A triage row excluding `mutant` with the `key` entry `entry`.
fn excluded(mutant: &str, key: &str, entry: &str) -> String {
    format!(
        "| `{}` | equivalent | LIB-TOOL-001 | Example: no reachable input tells it apart. | `{key}: {}` |\n",
        cell(mutant),
        cell(entry)
    )
}

/// `text` as a table cell: `|` written `\|`.
fn cell(text: &str) -> String {
    text.replace('|', "\\|")
}

fn re_row(mutant: &str, entry: &str) -> String {
    excluded(mutant, "exclude_re", entry)
}

fn glob_row(mutant: &str, entry: &str) -> String {
    excluded(mutant, "exclude_globs", entry)
}

fn call_row(mutant: &str, entry: &str) -> String {
    excluded(mutant, "skip_calls", entry)
}

/// [`TRIAGE_HEAD`] followed by `rows`. The first row is always a killed
/// mutant that is listed, and the second a killed mutant that no longer
/// exists, as in the real document.
fn triage(rows: &[String]) -> String {
    let mut text = TRIAGE_HEAD.to_owned();
    text.push_str(&killed(REPO_ROOT));
    text.push_str(&killed(
        "xtask/src/matrix.rs:460:35: replace == with != in defined_functions",
    ));
    for row in rows {
        text.push_str(row);
    }
    text.push_str("\n## Known items\n\nNothing else is read.\n");
    text
}

/// The triage rows of [`correct_config`].
fn correct_rows() -> Vec<String> {
    vec![
        re_row(EXAMPLE_TRUE, RE_EXAMPLE),
        glob_row(MAIN_DEFAULT, GLOB_MAIN),
        glob_row(MAIN_NOT, GLOB_MAIN),
        call_row(LOAD_MINUS, EXAMPLE_CALL),
        call_row(LOAD_TIMES, EXAMPLE_CALL),
    ]
}

/// [`correct_rows`] with the row of `mutant` replaced by `rows`.
fn correct_rows_with(mutant: &str, rows: &[String]) -> Vec<String> {
    let mark = format!("| `{}` |", cell(mutant));
    let mut out: Vec<String> = correct_rows()
        .into_iter()
        .filter(|row| !row.starts_with(&mark))
        .collect();
    assert_eq!(
        out.len() + 1,
        correct_rows().len(),
        "{QA10}: precondition: `{mutant}` has exactly one row in the correct triage"
    );
    out.extend_from_slice(rows);
    out
}

// ---------------------------------------------------------------------------
// The fake lister
// ---------------------------------------------------------------------------

/// Which listings fail, and with what error.
type Failure = (Box<dyn Fn(Option<&str>) -> bool>, String);

/// A fake `cargo mutants --workspace --list` (see the module docs).
#[derive(Default)]
struct Fake {
    calls: Vec<Option<String>>,
    fail: Option<Failure>,
    reversed: bool,
}

impl Fake {
    fn new() -> Self {
        Self::default()
    }

    /// A fake whose listings print the mutants in reverse order.
    fn reversed() -> Self {
        Self {
            reversed: true,
            ..Self::default()
        }
    }

    /// A fake whose listings fail with `error` wherever `fails` holds.
    fn failing(fails: impl Fn(Option<&str>) -> bool + 'static, error: &str) -> Self {
        Self {
            fail: Some((Box::new(fails), error.to_owned())),
            ..Self::default()
        }
    }

    fn list(&mut self, config: Option<&str>) -> Result<Vec<String>, String> {
        self.calls.push(config.map(str::to_owned));
        if let Some((fails, error)) = &self.fail {
            if fails(config) {
                return Err(error.clone());
            }
        }
        let table: toml::Table = config.and_then(|t| t.parse().ok()).unwrap_or_default();
        let strings = |key: &str| -> Option<Vec<String>> {
            table.get(key).and_then(toml::Value::as_array).map(|items| {
                items
                    .iter()
                    .filter_map(toml::Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
        };
        let regexes = strings("exclude_re").unwrap_or_default();
        let globs = strings("exclude_globs").unwrap_or_default();
        let calls = strings("skip_calls").unwrap_or_default();
        let examined = strings("examine_globs");
        let defaults = table
            .get("skip_calls_defaults")
            .and_then(toml::Value::as_bool)
            .unwrap_or(true);
        let mut names: Vec<String> = UNIVERSE
            .iter()
            .filter(|m| !(defaults && m.call == Some("with_capacity")))
            .filter(|m| !regexes.iter().any(|re| regex_matches(re, m.name)))
            .filter(|m| !globs.iter().any(|glob| glob == m.file))
            .filter(|m| !m.call.is_some_and(|c| calls.iter().any(|s| s == c)))
            .filter(|m| {
                examined
                    .as_ref()
                    .is_none_or(|files| files.iter().any(|f| f == m.file))
            })
            .map(|m| m.name.to_owned())
            .collect();
        if self.reversed {
            names.reverse();
        }
        Ok(names)
    }
}

/// One element of the regex form LIB-QA-007 accepts.
enum Token {
    Char(char),
    Digits,
}

/// Whether the regex `pattern`, in the form `^`, escaped literal text and
/// `\d+`, `$`, matches all of `name`. Any other regex matches nothing.
fn regex_matches(pattern: &str, name: &str) -> bool {
    let Some(body) = pattern
        .strip_prefix('^')
        .and_then(|rest| rest.strip_suffix('$'))
    else {
        return false;
    };
    let mut tokens = Vec::new();
    let mut chars = body.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => match chars.next() {
                Some('d') if chars.next() == Some('+') => tokens.push(Token::Digits),
                Some(p) if p.is_ascii_punctuation() => tokens.push(Token::Char(p)),
                _ => return false,
            },
            '.' | '+' | '*' | '?' | '(' | ')' | '|' | '[' | ']' | '{' | '}' | '^' | '$' => {
                return false;
            }
            c => tokens.push(Token::Char(c)),
        }
    }
    let text: Vec<char> = name.chars().collect();
    tokens_match(&tokens, &text)
}

fn tokens_match(tokens: &[Token], text: &[char]) -> bool {
    match tokens.split_first() {
        None => text.is_empty(),
        Some((Token::Char(c), rest)) => text.first() == Some(c) && tokens_match(rest, &text[1..]),
        Some((Token::Digits, rest)) => {
            let digits = text.iter().take_while(|c| c.is_ascii_digit()).count();
            (1..=digits).any(|n| tokens_match(rest, &text[n..]))
        }
    }
}

/// Whether `config` parses and holds `entry` in one of its exclusion arrays.
fn has_entry(config: &str, entry: &str) -> bool {
    let Ok(table) = config.parse::<toml::Table>() else {
        return false;
    };
    ["exclude_re", "exclude_globs", "skip_calls"]
        .iter()
        .any(|key| {
            table
                .get(*key)
                .and_then(toml::Value::as_array)
                .is_some_and(|items| items.iter().any(|v| v.as_str() == Some(entry)))
        })
}

#[test]
fn lib_qa_010_the_fake_lister_models_cargo_mutants() {
    let mut fake = Fake::new();
    let listed = |fake: &mut Fake, config: Option<&str>| fake.list(config).expect("lists");
    assert_eq!(
        listed(&mut fake, None).len(),
        NO_CONFIG_LISTED,
        "{QA10}: precondition: --no-config lists every mutant but the default skip"
    );
    for (what, text, gone) in [
        (
            "exclude_re",
            config(&[RE_EXAMPLE], &[], &[]),
            vec![EXAMPLE_TRUE],
        ),
        (
            "exclude_re with escaped `|`",
            config(&[RE_FETCH_OR], &[], &[]),
            vec![FETCH_OR],
        ),
        (
            "exclude_re at a position",
            config(&[RE_EXAMPLE_AT], &[], &[]),
            vec![EXAMPLE_TRUE],
        ),
        (
            "exclude_re matching nothing",
            config(&[RE_GONE], &[], &[]),
            vec![],
        ),
        (
            "exclude_globs",
            config(&[], &[GLOB_MAIN], &[]),
            vec![MAIN_DEFAULT, MAIN_NOT],
        ),
        (
            "skip_calls",
            config(&[], &[], &[EXAMPLE_CALL]),
            vec![LOAD_MINUS, LOAD_TIMES],
        ),
        (
            "skip_calls never called",
            config(&[], &[], &[NEVER_CALLED]),
            vec![],
        ),
    ] {
        let names = listed(&mut fake, Some(&text));
        let mut missing: Vec<&str> = UNIVERSE
            .iter()
            .map(|m| m.name)
            .filter(|n| *n != RENDER_CAPACITY && !names.iter().any(|l| l == n))
            .collect();
        missing.sort_unstable();
        let mut gone = gone;
        gone.sort_unstable();
        assert_eq!(
            missing, gone,
            "{QA10}: precondition: the fake's {what} excludes exactly these"
        );
    }
}

// ---------------------------------------------------------------------------
// Assertions
// ---------------------------------------------------------------------------

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

fn check(config: &str, triage: &str, fake: &mut Fake) -> Result<ExclusionSummary, Vec<String>> {
    check_exclusions(config, triage, &mut |c| fake.list(c))
}

/// Assert that `result` passes with `summary`.
fn assert_passes(
    what: &str,
    result: Result<ExclusionSummary, Vec<String>>,
    summary: &ExclusionSummary,
) {
    match result {
        Ok(got) => assert_eq!(
            &got, summary,
            "{QA10}: {what} passes with the counts {summary:?}"
        ),
        Err(problems) => {
            panic!("{QA10}: {what} must pass with {summary:?}; it gave {problems:#?}")
        }
    }
}

/// Assert that `result` fails with exactly the problem codes `codes` (each
/// as many times as named), sorted; return the problems.
fn assert_fails(
    what: &str,
    result: Result<ExclusionSummary, Vec<String>>,
    codes: &[&str],
) -> Vec<String> {
    let problems = match result {
        Ok(summary) => {
            panic!("{QA10}: {what} must give exactly {codes:?}; it passed with {summary:?}")
        }
        Err(problems) => problems,
    };
    let mut want = codes.to_vec();
    want.sort_unstable();
    assert_eq!(
        problem_codes(&problems),
        want,
        "{QA10}: {what} must give exactly {want:?}; it gave {problems:#?}"
    );
    let mut sorted = problems.clone();
    sorted.sort();
    assert_eq!(
        problems, sorted,
        "{QA10}: {what}: the problems are reported sorted"
    );
    problems
}

/// Assert that one of `problems` has `code` and contains every `needle`.
fn assert_named(what: &str, problems: &[String], code: &str, needles: &[&str]) {
    let tag = format!("[{code}] ");
    assert!(
        problems
            .iter()
            .any(|p| p.starts_with(&tag) && needles.iter().all(|n| p.contains(n))),
        "{QA10}: {what}: a `[{code}]` problem must name {needles:?}; the problems are {problems:#?}"
    );
}

/// Check `config` and `triage` with a fresh fake; the problems, which must
/// have exactly `codes`.
fn fails_with(what: &str, config: &str, triage: &str, codes: &[&str]) -> Vec<String> {
    assert_fails(what, check(config, triage, &mut Fake::new()), codes)
}

/// As [`fails_with`], for unreadable input: the function must report it
/// without listing anything, so the fake must record no call at all.
fn fails_unlisted(what: &str, config: &str, triage: &str, codes: &[&str]) -> Vec<String> {
    let mut fake = Fake::new();
    let problems = assert_fails(what, check(config, triage, &mut fake), codes);
    assert!(
        fake.calls.is_empty(),
        "{QA10}: {what}: unreadable input is reported without listing anything; it listed {:#?}",
        fake.calls
    );
    problems
}

// ---------------------------------------------------------------------------
// Passing configurations
// ---------------------------------------------------------------------------

/// Acceptance item 4: a configuration with no exclusions passes when the
/// listings with and without it are equal, whatever killed rows the triage
/// table holds. Only (a) and (b) are listed.
#[test]
fn lib_qa_010_a_configuration_without_exclusions_passes() {
    let summary = ExclusionSummary {
        entries: 0,
        excluded: 0,
        listed: NO_CONFIG_LISTED,
    };
    let empty_table = format!("{TRIAGE_HEAD}\nNo rows yet.\n");
    for (what, triage) in [
        ("no exclusions, killed rows", triage(&[])),
        ("no exclusions, a table with no rows", empty_table),
    ] {
        let mut fake = Fake::new();
        assert_passes(what, check(CONFIG_BASE, &triage, &mut fake), &summary);
        let mut calls = fake.calls.clone();
        calls.sort();
        assert_eq!(
            calls,
            vec![None, Some(CONFIG_BASE.to_owned())],
            "{QA10}: {what}: with no entries, only (a) the configuration and (b) --no-config are listed"
        );
    }
}

/// Acceptance items 2 and 4: one `exclude_re`, one `exclude_globs` and one
/// `skip_calls` entry, each excluding exactly the mutants of its triage rows,
/// pass with the right counts, whatever order the listings come in.
#[test]
fn lib_qa_010_a_correct_configuration_passes_with_its_counts() {
    let summary = ExclusionSummary {
        entries: 3,
        excluded: 5,
        listed: NO_CONFIG_LISTED - 5,
    };
    let config = correct_config();
    let triage = triage(&correct_rows());
    for (what, mut fake) in [
        ("the correct configuration", Fake::new()),
        (
            "the correct configuration, listings reversed",
            Fake::reversed(),
        ),
    ] {
        assert_passes(what, check(&config, &triage, &mut fake), &summary);
        assert_eq!(
            fake.calls.len(),
            2 + summary.entries,
            "{QA10}: {what}: (a), (b) and one (c) per entry are listed; calls: {:#?}",
            fake.calls
        );
    }
}

/// Acceptance item 4 (D5): the triage table is read by the LIB-QA-007 rules.
/// `\|` in a cell is `|`, a code span around a cell is not part of it, and
/// `—`, `-`, `no` and an empty cell mean not excluded.
#[test]
fn lib_qa_010_triage_cells_are_read_by_the_lib_qa_007_rules() {
    // Written by hand, not by `cell`: the regex `\|\|` is `\\|\\|` in a cell.
    let or_row = r"| `xtask/src/fetch.rs:44:17: replace \|\| with && in fetch_into` | equivalent | LIB-TOOL-002 | Example. | `exclude_re: ^xtask/src/fetch\.rs:\d+:\d+: replace \\|\\| with && in fetch_into$` |
";
    let or_summary = ExclusionSummary {
        entries: 1,
        excluded: 1,
        listed: NO_CONFIG_LISTED - 1,
    };
    assert_passes(
        "a mutant and a regex holding `|`",
        check(
            &config(&[RE_FETCH_OR], &[], &[]),
            &triage(&[or_row.to_owned()]),
            &mut Fake::new(),
        ),
        &or_summary,
    );
    let correct = ExclusionSummary {
        entries: 3,
        excluded: 5,
        listed: NO_CONFIG_LISTED - 5,
    };
    let bare_mutant = format!(
        "| {MAIN_NOT} | accepted A | LIB-TOOL-001 | Example. | `exclude_globs: {GLOB_MAIN}` |\n"
    );
    let bare_excluded = format!(
        "| `{MAIN_NOT}` | accepted A | LIB-TOOL-001 | Example. | exclude_globs: {GLOB_MAIN} |\n"
    );
    let not_excluded = [
        format!("| `{RUN_FROM}` | killed | LIB-TOOL-001 | A test. | - |\n"),
        format!("| `{READY_TRUE}` | killed | LIB-TOOL-001 | A test. | no |\n"),
        format!("| `{EXAMPLE_FALSE}` | killed | LIB-TOOL-001 | A test. |  |\n"),
    ];
    let mut with_not_excluded = correct_rows();
    with_not_excluded.extend_from_slice(&not_excluded);
    for (what, rows) in [
        (
            "a Mutant cell without a code span",
            correct_rows_with(MAIN_NOT, &[bare_mutant]),
        ),
        (
            "an Excluded cell without a code span",
            correct_rows_with(MAIN_NOT, &[bare_excluded]),
        ),
        ("`-`, `no` and an empty Excluded cell", with_not_excluded),
    ] {
        assert_passes(
            what,
            check(&correct_config(), &triage(&rows), &mut Fake::new()),
            &correct,
        );
    }
}

// ---------------------------------------------------------------------------
// Each problem code
// ---------------------------------------------------------------------------

/// Acceptance item 2: a mutant in (a) but not in (b) is `[exclusion-adds]`,
/// naming the mutant. `skip_calls_defaults = false` lets the configuration
/// list the `with_capacity` mutants that `--no-config` skips.
#[test]
fn lib_qa_010_a_mutant_only_the_configuration_lists_is_exclusion_adds() {
    for (what, base, rows) in [
        (
            "skip_calls_defaults = false, no exclusions",
            CONFIG_BASE.to_owned(),
            Vec::new(),
        ),
        (
            "skip_calls_defaults = false, with exclusions",
            correct_config(),
            correct_rows(),
        ),
    ] {
        let config = format!("{base}skip_calls_defaults = false\n");
        let problems = fails_with(what, &config, &triage(&rows), &["exclusion-adds"]);
        assert_named(what, &problems, "exclusion-adds", &[RENDER_CAPACITY]);
    }
}

/// Acceptance item 2: a mutant in (b) but not in (a) that no triage row
/// assigns to an entry is `[exclusion-untriaged]`, naming the mutant, whether
/// no entry or one entry excludes it. In the second case it is also an extra
/// `[exclusion-mismatch]` for that entry.
#[test]
fn lib_qa_010_an_excluded_mutant_without_a_triage_row_is_exclusion_untriaged() {
    let what = "a non-entry key (examine_globs) that leaves out main.rs";
    let config = format!(
        "{CONFIG_BASE}{}",
        array("examine_globs", &[LIB_RS, FETCH_RS, MATRIX_RS])
    );
    let problems = fails_with(
        what,
        &config,
        &triage(&[]),
        &["exclusion-untriaged", "exclusion-untriaged"],
    );
    assert_named(what, &problems, "exclusion-untriaged", &[MAIN_DEFAULT]);
    assert_named(what, &problems, "exclusion-untriaged", &[MAIN_NOT]);

    let dash_row = format!("| `{MAIN_NOT}` | accepted A | LIB-TOOL-001 | Example. | — |\n");
    for (what, rows) in [
        (
            "the row of a mutant the glob excludes removed",
            correct_rows_with(MAIN_NOT, &[]),
        ),
        (
            "the row of a mutant the glob excludes marked `—`",
            correct_rows_with(MAIN_NOT, std::slice::from_ref(&dash_row)),
        ),
        (
            "the row of a mutant the glob excludes marked `—`, but triaged as excluded",
            correct_rows_with(MAIN_NOT, &[dash_row.replace("accepted A", "equivalent")]),
        ),
    ] {
        let problems = fails_with(
            what,
            &correct_config(),
            &triage(&rows),
            &["exclusion-mismatch", "exclusion-untriaged"],
        );
        assert_named(what, &problems, "exclusion-untriaged", &[MAIN_NOT]);
        assert_named(
            what,
            &problems,
            "exclusion-mismatch",
            &[MAIN_NOT, GLOB_MAIN],
        );
    }
}

/// Acceptance item 2: an entry that excludes a mutant its triage rows do not
/// name is `[exclusion-mismatch]` for that mutant and entry. Assigned to the
/// wrong entry, the mutant is extra for the entry that excludes it and
/// missing for the one its row names: one problem for each.
#[test]
fn lib_qa_010_an_entry_excluding_a_mutant_it_is_not_assigned_is_exclusion_mismatch() {
    let what = "a mutant the glob excludes, assigned to the skip_calls entry";
    let rows = correct_rows_with(MAIN_NOT, &[call_row(MAIN_NOT, EXAMPLE_CALL)]);
    let problems = fails_with(
        what,
        &correct_config(),
        &triage(&rows),
        &["exclusion-mismatch", "exclusion-mismatch"],
    );
    assert_named(
        what,
        &problems,
        "exclusion-mismatch",
        &[MAIN_NOT, EXAMPLE_CALL],
    );
    let naming_call = problems.iter().filter(|p| p.contains(EXAMPLE_CALL)).count();
    assert_eq!(
        naming_call, 1,
        "{QA10}: {what}: one mismatch names the skip_calls entry (missing), the other the glob (extra): {problems:#?}"
    );

    let what = "a mutant the regex excludes, assigned to the glob";
    let rows = correct_rows_with(EXAMPLE_TRUE, &[glob_row(EXAMPLE_TRUE, GLOB_MAIN)]);
    let problems = fails_with(
        what,
        &correct_config(),
        &triage(&rows),
        &["exclusion-mismatch", "exclusion-mismatch"],
    );
    assert_named(
        what,
        &problems,
        "exclusion-mismatch",
        &[EXAMPLE_TRUE, RE_EXAMPLE],
    );
    let naming_re = problems.iter().filter(|p| p.contains(RE_EXAMPLE)).count();
    assert_eq!(
        naming_re, 1,
        "{QA10}: {what}: one mismatch names the regex (extra), the other the glob (missing): {problems:#?}"
    );
}

/// Acceptance item 2: a mutant a triage row assigns to an entry that the
/// entry does not exclude is `[exclusion-mismatch]`, naming the mutant and
/// the entry: a listed mutant outside the entry (for a `skip_calls` entry,
/// a regex and a glob), and a mutant that is no longer listed at all.
#[test]
fn lib_qa_010_a_triaged_mutant_its_entry_does_not_exclude_is_exclusion_mismatch() {
    for (what, extra, mutant, entry) in [
        (
            "a mutant outside the call, assigned to the skip_calls entry",
            call_row(READY_TRUE, EXAMPLE_CALL),
            READY_TRUE,
            EXAMPLE_CALL,
        ),
        (
            "a mutant the regex does not match, assigned to it",
            re_row(EXAMPLE_FALSE, RE_EXAMPLE),
            EXAMPLE_FALSE,
            RE_EXAMPLE,
        ),
        (
            "a mutant of another file, assigned to the glob",
            glob_row(RUN_FROM, GLOB_MAIN),
            RUN_FROM,
            GLOB_MAIN,
        ),
        (
            "a moved mutant no listing has, assigned to the glob",
            glob_row(MOVED_MUTANT, GLOB_MAIN),
            MOVED_MUTANT,
            GLOB_MAIN,
        ),
    ] {
        let mut rows = correct_rows();
        rows.push(extra);
        let problems = fails_with(
            what,
            &correct_config(),
            &triage(&rows),
            &["exclusion-mismatch"],
        );
        assert_named(what, &problems, "exclusion-mismatch", &[mutant, entry]);
    }
}

/// Acceptance item 2: an entry whose removal restores no mutant is
/// `[exclusion-empty]`, naming the entry: a `skip_calls` entry for a function
/// no mutant is inside, and a regex that matches nothing.
#[test]
fn lib_qa_010_an_entry_that_excludes_nothing_is_exclusion_empty() {
    for (what, config, entry) in [
        (
            "a skip_calls entry never called",
            config(&[RE_EXAMPLE], &[GLOB_MAIN], &[EXAMPLE_CALL, NEVER_CALLED]),
            NEVER_CALLED,
        ),
        (
            "an exclude_re entry matching nothing",
            config(&[RE_EXAMPLE, RE_GONE], &[GLOB_MAIN], &[EXAMPLE_CALL]),
            RE_GONE,
        ),
    ] {
        let problems = fails_with(
            what,
            &config,
            &triage(&correct_rows()),
            &["exclusion-empty"],
        );
        assert_named(what, &problems, "exclusion-empty", &[entry]);
    }
}

/// Acceptance items 2 and 4: an overlapping pair of entries. The entry with
/// nothing of its own is `[exclusion-empty]`, and the triage row of the
/// shared mutant names an entry whose removal does not restore it, which is
/// `[exclusion-mismatch]` (see the module docs).
#[test]
fn lib_qa_010_an_overlapping_pair_is_exclusion_empty_for_the_entry_with_nothing_of_its_own() {
    let overlap = config(&[RE_EXAMPLE, RE_MAIN_NOT], &[GLOB_MAIN], &[EXAMPLE_CALL]);

    let what = "a regex inside the glob, the shared mutant assigned to the regex";
    let rows = correct_rows_with(MAIN_NOT, &[re_row(MAIN_NOT, RE_MAIN_NOT)]);
    let problems = fails_with(
        what,
        &overlap,
        &triage(&rows),
        &["exclusion-empty", "exclusion-mismatch"],
    );
    assert_named(what, &problems, "exclusion-empty", &[RE_MAIN_NOT]);
    assert_named(
        what,
        &problems,
        "exclusion-mismatch",
        &[MAIN_NOT, RE_MAIN_NOT],
    );

    let what = "a regex inside the glob, the shared mutant assigned to the glob";
    let problems = fails_with(
        what,
        &overlap,
        &triage(&correct_rows()),
        &["exclusion-empty", "exclusion-mismatch"],
    );
    assert_named(what, &problems, "exclusion-empty", &[RE_MAIN_NOT]);
    assert_named(
        what,
        &problems,
        "exclusion-mismatch",
        &[MAIN_NOT, GLOB_MAIN],
    );

    let what = "two regexes for the same mutant";
    let twins = config(&[RE_EXAMPLE, RE_EXAMPLE_AT], &[GLOB_MAIN], &[EXAMPLE_CALL]);
    let problems = fails_with(
        what,
        &twins,
        &triage(&correct_rows()),
        &["exclusion-empty", "exclusion-empty", "exclusion-mismatch"],
    );
    assert_named(what, &problems, "exclusion-empty", &[RE_EXAMPLE]);
    assert_named(what, &problems, "exclusion-empty", &[RE_EXAMPLE_AT]);
    assert_named(
        what,
        &problems,
        "exclusion-mismatch",
        &[EXAMPLE_TRUE, RE_EXAMPLE],
    );
}

/// Acceptance item 3 (D1): a listing that fails is `[mutants-tool]`, and the
/// problem holds the tool's error, for (a), (b) and each (`c_e`). The fixture
/// is otherwise correct, so this is the only problem whether or not the
/// other listings are compared.
#[test]
fn lib_qa_010_a_failed_listing_is_mutants_tool_with_its_error() {
    let config = correct_config();
    let triage = triage(&correct_rows());
    let configured = config.clone();
    let mut fakes: Vec<(String, Fake, String)> = vec![
        (
            "(a), the configured listing".to_owned(),
            Fake::failing(
                move |c| c == Some(configured.as_str()),
                "error: cargo-mutants failed on (a): no such file",
            ),
            "error: cargo-mutants failed on (a): no such file".to_owned(),
        ),
        (
            "(b), the --no-config listing".to_owned(),
            Fake::failing(
                |c: Option<&str>| c.is_none(),
                "error: unexpected argument '--no-config'",
            ),
            "error: unexpected argument '--no-config'".to_owned(),
        ),
    ];
    for entry in [RE_EXAMPLE, GLOB_MAIN, EXAMPLE_CALL] {
        let error = format!("error: failed to list mutants without `{entry}`: exit status 1");
        let entry = entry.to_owned();
        fakes.push((
            format!("(c) without `{entry}`"),
            Fake::failing(
                move |c| c.is_some_and(|text| !has_entry(text, &entry)),
                &error,
            ),
            error,
        ));
    }
    for (what, mut fake, error) in fakes {
        let problems = assert_fails(&what, check(&config, &triage, &mut fake), &["mutants-tool"]);
        assert_named(&what, &problems, "mutants-tool", &[&error]);
    }
}

/// D1: a configuration that is not TOML is `[config-parse]`, and nothing
/// else, and nothing is listed (verification 0010 F8).
#[test]
fn lib_qa_010_an_unparsable_configuration_is_config_parse() {
    for (what, config) in [
        ("a key without a value", "cap_lints =\n".to_owned()),
        (
            "a duplicate key",
            format!("{CONFIG_BASE}cap_lints = true\n"),
        ),
        (
            "an unterminated table header",
            format!("{CONFIG_BASE}[unterminated\n"),
        ),
        ("JSON", "{ \"cap_lints\": true }".to_owned()),
    ] {
        fails_unlisted(what, &config, &triage(&[]), &["config-parse"]);
    }
}

/// D1 (lead's decision): an exclusion key whose value is not an array of
/// strings is `[config-exclusion]`, the LIB-QA-007 code, naming the key, and
/// nothing else. Nothing is listed, neither (c) nor (a) and (b)
/// (verification 0010 F8). An array holding a string and a non-string is not
/// an array of strings either (F4: kills `any(is_str)` for `all(is_str)`).
#[test]
fn lib_qa_010_an_exclusion_key_that_is_not_a_string_array_is_config_exclusion() {
    let mixed = format!("['{RE_EXAMPLE}', 1]");
    for (what, key, value) in [
        ("exclude_re a string", "exclude_re", "\"x\""),
        ("exclude_globs an array of integers", "exclude_globs", "[1]"),
        ("skip_calls a table", "skip_calls", "{}"),
        (
            "exclude_re a valid regex and an integer",
            "exclude_re",
            mixed.as_str(),
        ),
    ] {
        let config = format!("{CONFIG_BASE}{key} = {value}\n");
        let problems = fails_unlisted(what, &config, &triage(&[]), &["config-exclusion"]);
        assert_named(what, &problems, "config-exclusion", &[key]);
    }
}

/// D1 and D5: a triage table the LIB-QA-007 rules cannot read is
/// `[triage-format]`, the only problem, and nothing is listed
/// (verification 0010 F8).
#[test]
fn lib_qa_010_a_malformed_triage_table_is_triage_format() {
    let reference = triage(&[]);
    let edit = |from: &str, to: &str| {
        assert_eq!(
            reference.matches(from).count(),
            1,
            "{QA10}: precondition: `{from}` occurs once in the triage fixture"
        );
        reference.replacen(from, to, 1)
    };
    let header = "| Mutant | Disposition | Row | Reason | Excluded |\n";
    let separator = "|---|---|---|---|---|\n";
    let killed_cells = format!("{REPO_ROOT}` | killed | LIB-TOOL-001 | A test kills it. | — |");
    let row_edit = |cells: &str| edit(&killed_cells, &format!("{REPO_ROOT}` {cells}"));
    let controls = [
        ("no triage table", "# Mutation-testing triage\n".to_owned()),
        (
            "a column removed from the header",
            edit("| Reason | Excluded |", "| Excluded |"),
        ),
        (
            "a column renamed in the header",
            edit("| Reason | Excluded |", "| Reason | Exclusion |"),
        ),
        (
            "the columns reordered",
            edit("| Row | Reason |", "| Reason | Row |"),
        ),
        (
            "two tables with the header",
            format!("{reference}\n{header}{separator}"),
        ),
        ("no separator row", edit(separator, "")),
        (
            "a row with four cells",
            row_edit("| killed | LIB-TOOL-001 | — |"),
        ),
        (
            "an unknown disposition",
            row_edit("| unlikely | LIB-TOOL-001 | A test kills it. | — |"),
        ),
        (
            "an Excluded cell that is neither `—` nor `<key>: <entry>`",
            row_edit("| killed | LIB-TOOL-001 | A test kills it. | maybe |"),
        ),
        (
            "an Excluded cell with an unknown key",
            row_edit("| killed | LIB-TOOL-001 | A test kills it. | `skip: example_call` |"),
        ),
        (
            "an Excluded cell without the space after the key",
            row_edit("| killed | LIB-TOOL-001 | A test kills it. | `skip_calls:example_call` |"),
        ),
    ];
    assert_passes(
        "the reference triage table",
        check(CONFIG_BASE, &reference, &mut Fake::new()),
        &ExclusionSummary {
            entries: 0,
            excluded: 0,
            listed: NO_CONFIG_LISTED,
        },
    );
    for (what, triage) in controls {
        assert_ne!(
            triage, reference,
            "{QA10}: precondition: `{what}` changes the triage fixture"
        );
        fails_unlisted(what, CONFIG_BASE, &triage, &["triage-format"]);
    }
}

/// D5 (verification 0010 F5): the table ends at the first line that does not
/// start with `|`. Row-shaped lines after a blank line that follows it are
/// not read: an excluded row there assigns nothing (else the `skip_calls`
/// entry would miss a mutant), and a 4-cell row there is no
/// `[triage-format]`.
#[test]
fn lib_qa_010_rows_after_the_end_of_the_triage_table_are_not_read() {
    let after_table = format!(
        "\n{}| `{EXAMPLE_FALSE}` | killed | LIB-TOOL-001 | — |\n",
        call_row(READY_TRUE, EXAMPLE_CALL)
    );
    let reference = triage(&correct_rows());
    let mark = "\n## Known items";
    assert_eq!(
        reference.matches(mark).count(),
        1,
        "{QA10}: precondition: the triage fixture has one `{mark}` after its table"
    );
    let triage = reference.replacen(mark, &format!("{after_table}{mark}"), 1);
    assert!(
        triage.contains(&format!("|\n\n| `{READY_TRUE}`")),
        "{QA10}: precondition: a blank line separates the table from the later rows:\n{triage}"
    );
    assert_passes(
        "rows after the table",
        check(&correct_config(), &triage, &mut Fake::new()),
        &ExclusionSummary {
            entries: 3,
            excluded: 5,
            listed: NO_CONFIG_LISTED - 5,
        },
    );
}

// ---------------------------------------------------------------------------
// Several problems, and the listings asked for
// ---------------------------------------------------------------------------

/// Acceptance item 3: every problem is reported, sorted, and in the same
/// order on every run, whatever order the listings come in.
#[test]
fn lib_qa_010_every_problem_is_reported_sorted_and_deterministically() {
    let config = format!(
        "{}skip_calls_defaults = false\n",
        config(&[RE_EXAMPLE], &[GLOB_MAIN], &[EXAMPLE_CALL, NEVER_CALLED])
    );
    let mut rows = correct_rows_with(MAIN_NOT, &[]);
    rows.push(call_row(READY_TRUE, EXAMPLE_CALL));
    let triage = triage(&rows);
    let codes = [
        "exclusion-adds",
        "exclusion-empty",
        "exclusion-mismatch",
        "exclusion-mismatch",
        "exclusion-untriaged",
    ];
    let what = "five problems at once";
    let first = assert_fails(what, check(&config, &triage, &mut Fake::new()), &codes);
    assert_named(what, &first, "exclusion-adds", &[RENDER_CAPACITY]);
    assert_named(what, &first, "exclusion-empty", &[NEVER_CALLED]);
    assert_named(what, &first, "exclusion-mismatch", &[MAIN_NOT, GLOB_MAIN]);
    assert_named(
        what,
        &first,
        "exclusion-mismatch",
        &[READY_TRUE, EXAMPLE_CALL],
    );
    assert_named(what, &first, "exclusion-untriaged", &[MAIN_NOT]);
    let again = assert_fails(what, check(&config, &triage, &mut Fake::new()), &codes);
    assert_eq!(
        first, again,
        "{QA10}: the same inputs give the same problems in the same order"
    );
    let reversed = assert_fails(what, check(&config, &triage, &mut Fake::reversed()), &codes);
    assert_eq!(
        first, reversed,
        "{QA10}: the order of a listing does not change the problems or their order"
    );
}

/// Acceptance item 1 (D1): the function lists (a) the configuration text as
/// given, (b) `--no-config`, and (c) for each entry `e`, once, the
/// configuration without `e`: every other key and entry kept, `e` removed
/// from its array (an array left empty stays, as an empty array).
#[test]
fn lib_qa_010_each_entry_is_listed_without_only_itself() {
    let config = format!(
        "# Every exclusion here is triaged.
cap_lints = true
all_features = true
test_workspace = false
skip_calls_defaults = true
timeout_multiplier = 3.0

exclude_re = [
  '{RE_EXAMPLE}',
  '{RE_FETCH_OR}',
]
exclude_globs = [\"{GLOB_MAIN}\"]
skip_calls = [\"{EXAMPLE_CALL}\"]
"
    );
    let mut rows = correct_rows();
    rows.push(re_row(FETCH_OR, RE_FETCH_OR));
    let mut fake = Fake::new();
    assert_passes(
        "four entries over three keys",
        check(&config, &triage(&rows), &mut fake),
        &ExclusionSummary {
            entries: 4,
            excluded: 6,
            listed: NO_CONFIG_LISTED - 6,
        },
    );

    let calls = &fake.calls;
    let count = |want: Option<&str>| calls.iter().filter(|c| c.as_deref() == want).count();
    assert_eq!(
        count(Some(&config)),
        1,
        "{QA10}: (a) lists the configuration text exactly as given, once; calls: {calls:#?}"
    );
    assert_eq!(
        count(None),
        1,
        "{QA10}: (b) lists with --no-config, once; calls: {calls:#?}"
    );
    let original: toml::Table = config.parse().expect("the fixture parses");
    let copies: Vec<toml::Table> = calls
        .iter()
        .flatten()
        .filter(|c| c.as_str() != config.as_str())
        .map(|c| {
            c.parse()
                .unwrap_or_else(|e| panic!("{QA10}: each (c) configuration is TOML: {e}\n{c}"))
        })
        .collect();
    assert_eq!(
        copies.len(),
        4,
        "{QA10}: (c) lists one configuration per entry and nothing else; calls: {calls:#?}"
    );
    for (key, entry) in [
        ("exclude_re", RE_EXAMPLE),
        ("exclude_re", RE_FETCH_OR),
        ("exclude_globs", GLOB_MAIN),
        ("skip_calls", EXAMPLE_CALL),
    ] {
        let mut without = original.clone();
        if let Some(toml::Value::Array(items)) = without.get_mut(key) {
            items.retain(|v| v.as_str() != Some(entry));
        }
        let matching = copies.iter().filter(|copy| **copy == without).count();
        assert_eq!(
            matching, 1,
            "{QA10}: exactly one (c) configuration is the original without the `{key}` entry `{entry}`, keeping cap_lints, all_features, test_workspace and every other key and entry; the (c) configurations: {copies:#?}"
        );
    }
}

// ---------------------------------------------------------------------------
// Verification 0010, round 1: F2, F3 and F6
// ---------------------------------------------------------------------------

/// Acceptance items 1 and 2 (F2): the same string twice under one key is two
/// entries. Each copy without one of them still holds the other, so each is
/// `[exclusion-empty]`; the triage row names both, and neither restores its
/// mutant, so each is also `[exclusion-mismatch]`. Kills removing the entry
/// by value (both copies) instead of by its index.
#[test]
fn lib_qa_010_a_string_twice_under_one_key_is_two_empty_entries() {
    let what = "the same exclude_re twice";
    let config = config(&[RE_EXAMPLE, RE_EXAMPLE], &[], &[]);
    let mut fake = Fake::new();
    let problems = assert_fails(
        what,
        check(
            &config,
            &triage(&[re_row(EXAMPLE_TRUE, RE_EXAMPLE)]),
            &mut fake,
        ),
        &[
            "exclusion-empty",
            "exclusion-empty",
            "exclusion-mismatch",
            "exclusion-mismatch",
        ],
    );
    assert_named(what, &problems, "exclusion-empty", &[RE_EXAMPLE]);
    assert_named(
        what,
        &problems,
        "exclusion-mismatch",
        &[EXAMPLE_TRUE, RE_EXAMPLE],
    );
    let copies: Vec<toml::Table> = fake
        .calls
        .iter()
        .flatten()
        .filter(|c| c.as_str() != config.as_str())
        .map(|c| c.parse().expect("each (c) configuration is TOML"))
        .collect();
    let mut kept = config.parse::<toml::Table>().expect("the fixture parses");
    if let Some(toml::Value::Array(items)) = kept.get_mut("exclude_re") {
        items.truncate(1);
    }
    assert_eq!(
        copies,
        vec![kept.clone(), kept],
        "{QA10}: {what}: each (c) configuration drops one of the two strings and keeps the other"
    );
}

/// Acceptance item 3 (F3): a failed (c) listing does not stop the others.
/// The first entry's (c) fails, and a later `skip_calls` entry that excludes
/// nothing is still reported; every (c) is listed. Kills `break` for
/// `continue` after a failed (c).
#[test]
fn lib_qa_010_a_failed_entry_listing_does_not_stop_the_later_entries() {
    let what = "the first entry's (c) fails, a later entry is empty";
    let config = config(&[RE_EXAMPLE], &[GLOB_MAIN], &[EXAMPLE_CALL, NEVER_CALLED]);
    let error = "error: listing without the regex failed: exit status 1";
    let mut fake = Fake::failing(|c| c.is_some_and(|t| !has_entry(t, RE_EXAMPLE)), error);
    let problems = assert_fails(
        what,
        check(&config, &triage(&correct_rows()), &mut fake),
        &["exclusion-empty", "mutants-tool"],
    );
    assert_named(what, &problems, "mutants-tool", &[error]);
    assert_named(what, &problems, "exclusion-empty", &[NEVER_CALLED]);
    assert_eq!(
        fake.calls.len(),
        2 + 4,
        "{QA10}: {what}: (a), (b) and all four (c) are listed; calls: {:#?}",
        fake.calls
    );
}

/// Acceptance item 2 (F6): a triage row counts only if the entry it names is
/// in the configuration. A mutant the glob excludes whose only row names an
/// `exclude_re` the configuration lacks is `[exclusion-untriaged]`, and an
/// extra `[exclusion-mismatch]` for the glob.
#[test]
fn lib_qa_010_a_row_naming_an_entry_the_configuration_lacks_does_not_triage() {
    let what = "a glob-excluded mutant whose row names a missing exclude_re";
    let rows = correct_rows_with(MAIN_NOT, &[re_row(MAIN_NOT, RE_MAIN_NOT)]);
    let problems = fails_with(
        what,
        &correct_config(),
        &triage(&rows),
        &["exclusion-mismatch", "exclusion-untriaged"],
    );
    assert_named(what, &problems, "exclusion-untriaged", &[MAIN_NOT]);
    assert_named(
        what,
        &problems,
        "exclusion-mismatch",
        &[MAIN_NOT, GLOB_MAIN],
    );
}
