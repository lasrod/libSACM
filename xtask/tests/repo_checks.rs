//! Regression tests for the repository's policy files: the CI gate list
//! (LIB-QA-001), the MSRV declaration (LIB-QA-002, LIB-DOC-007), the
//! dependency policy in `deny.toml` (LIB-QA-003), the cargo-mutants
//! configuration and its triage (LIB-QA-007), the nextest settings that make
//! mutation outcomes independent of test order (LIB-QA-012, ADR 0009), the
//! git-ignore rules (LIB-TOOL-002, LIB-QA-007), the ADR index and the
//! links to ADRs (LIB-DOC-008), the wasm32 build and the platform-API
//! lint (LIB-QA-017), and the CI gates that let a documentation-only push
//! skip the code-only jobs (LIB-QA-018).
//!
//! Workflow YAML structure is otherwise not pinned here: branch protection
//! on `main` and review guard it (slice 0012). The LIB-QA-018 gates are
//! pinned line by line, and its detection script is run on a scratch git
//! repository with the platform's bash and git.
//!
//! Each check is a function over a file's text or parsed TOML that returns the
//! problems it finds. Every problem starts with a stable code in brackets
//! (`[unreviewed-key] ...`), so a test can assert *which* check reported a
//! mutation, not only that something did. Each check is applied to the real
//! file, which must pass, and to in-memory mutations of a reference text, each
//! of which must give its codes. The mutations never touch the files on disk.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use xtask::fetch::DEFAULT_DEST;

fn repository_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repository root")
}

fn read(rel: &str) -> String {
    fs::read_to_string(repository_root().join(rel)).unwrap_or_else(|e| panic!("read {rel}: {e}"))
}

/// The text of `rel`, or `None` if it does not exist.
fn read_optional(rel: &str) -> Option<String> {
    match fs::read_to_string(repository_root().join(rel)) {
        Ok(text) => Some(text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => panic!("read {rel}: {e}"),
    }
}

fn toml_table(rel: &str) -> toml::Table {
    read(rel)
        .parse()
        .unwrap_or_else(|e| panic!("parse {rel}: {e}"))
}

fn workspace_msrv() -> String {
    toml_table("Cargo.toml")["workspace"]["package"]["rust-version"]
        .as_str()
        .expect("workspace.package.rust-version is a string")
        .to_owned()
}

/// A problem: `[code] text`.
fn problem(code: &str, text: impl std::fmt::Display) -> String {
    format!("[{code}] {text}")
}

fn has_code(problems: &[String], code: &str) -> bool {
    let tag = format!("[{code}] ");
    problems.iter().any(|p| p.starts_with(&tag))
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

fn require(problems: &mut Vec<String>, ok: bool, code: &str, text: &str) {
    if !ok {
        problems.push(problem(code, text));
    }
}

fn lookup<'a>(table: &'a toml::Table, path: &[&str]) -> Option<&'a toml::Value> {
    let (first, rest) = path.split_first()?;
    rest.iter()
        .try_fold(table.get(*first)?, |value, key| value.get(*key))
}

/// A TOML value written inline, e.g. `toml_value("[1, 2]")`.
fn toml_value(inline: &str) -> toml::Value {
    let table: toml::Table = format!("v = {inline}")
        .parse()
        .unwrap_or_else(|e| panic!("parse `{inline}`: {e}"));
    table["v"].clone()
}

// ---------------------------------------------------------------------------
// Mutation helpers: in-memory edits of a text, line by line.
// ---------------------------------------------------------------------------

fn indent(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

fn leading(line: &str) -> &str {
    &line[..indent(line)]
}

/// Replace the one line of `text` whose trimmed form is `target` by the lines
/// `edit` returns for it. Panics if there is not exactly one such line, so a
/// mutation can never silently be a no-op.
fn edit_line(text: &str, target: &str, edit: impl FnOnce(&str) -> Vec<String>) -> String {
    let hits = text.lines().filter(|line| line.trim() == target).count();
    assert_eq!(
        hits, 1,
        "mutation target `{target}` must occur exactly once"
    );
    let mut edit = Some(edit);
    let mut out = String::new();
    for line in text.lines() {
        if line.trim() == target {
            let edit = edit.take().expect("one target");
            for new in edit(line) {
                out.push_str(&new);
                out.push('\n');
            }
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

/// `text` with the one line equal (trimmed) to `target` replaced by
/// `replacement`, keeping its indentation.
fn replace_line(text: &str, target: &str, replacement: &str) -> String {
    edit_line(text, target, |line| {
        vec![format!("{}{replacement}", leading(line))]
    })
}

/// `text` with the one line equal (trimmed) to `target` commented out.
fn comment_line(text: &str, target: &str) -> String {
    edit_line(text, target, |line| {
        vec![format!("{}# {}", leading(line), line.trim())]
    })
}

/// `text` with the one line equal (trimmed) to `target` deleted.
fn delete_line(text: &str, target: &str) -> String {
    edit_line(text, target, |_| Vec::new())
}

/// `text` with `extra` inserted after the line equal (trimmed) to `target`,
/// indented `deeper` spaces more than it.
fn insert_after(text: &str, target: &str, deeper: usize, extra: &[&str]) -> String {
    edit_line(text, target, |line| {
        let pad = format!("{}{}", leading(line), " ".repeat(deeper));
        std::iter::once(line.to_owned())
            .chain(extra.iter().map(|e| format!("{pad}{e}")))
            .collect()
    })
}

/// `text` with its one occurrence of `from` replaced by `to`.
fn replace_once(text: &str, from: &str, to: &str) -> String {
    assert_eq!(
        text.matches(from).count(),
        1,
        "mutation target `{from}` must occur exactly once"
    );
    text.replacen(from, to, 1)
}

// ---------------------------------------------------------------------------
// LIB-QA-001
// ---------------------------------------------------------------------------

#[test]
fn lib_qa_001_ci_runs_every_gate() {
    let ci = read(".github/workflows/ci.yml");
    for gate in [
        "cargo fmt --all -- --check",
        "cargo clippy --workspace --all-targets --all-features -- -D warnings",
        "cargo test --workspace --all-features",
        "cargo test -p xtask",
        "cargo doc --workspace --no-deps --all-features",
        "RUSTDOCFLAGS: -D warnings",
        "cargo xtask matrix check",
        "EmbarkStudios/cargo-deny-action",
        "python -m unittest discover -s .claude/hooks",
    ] {
        assert!(ci.contains(gate), "LIB-QA-001: ci.yml must run `{gate}`");
    }
    for os in ["ubuntu-latest", "windows-latest", "macos-latest"] {
        assert!(ci.contains(os), "LIB-QA-001: tests must run on {os}");
    }
    for trigger in ["push:", "pull_request:"] {
        assert!(
            ci.contains(trigger),
            "LIB-QA-001: CI must trigger on {trigger}"
        );
    }
}

// ---------------------------------------------------------------------------
// LIB-QA-017: the wasm32 build and the platform-API lint
// ---------------------------------------------------------------------------

/// The target the library must build for.
const WASM_TARGET: &str = "wasm32-unknown-unknown";

/// The two wasm builds CI must run, each as a whole command.
const WASM_BUILDS: [&str; 2] = [
    "cargo build -p sacm --target wasm32-unknown-unknown",
    "cargo build -p sacm --target wasm32-unknown-unknown --all-features",
];

/// The command a workflow line runs: the line without its indent, a `- `
/// list marker and a `run:` key. A line of a `run: |` block is its own
/// command.
fn ci_command(line: &str) -> &str {
    let line = line.trim();
    let line = line.strip_prefix("- ").unwrap_or(line);
    line.strip_prefix("run:").unwrap_or(line).trim()
}

/// Whether a workflow line installs the wasm target: a toolchain action's
/// `targets:` input naming it, or `rustup target add` naming it.
fn installs_wasm_target(line: &str) -> bool {
    let line = line.trim();
    let line = line.strip_prefix("- ").unwrap_or(line);
    (line.starts_with("targets:") || line.contains("rustup target add"))
        && line.contains(WASM_TARGET)
}

/// LIB-QA-017: what is missing from `ci.yml` for the wasm builds.
fn wasm_ci_problems(ci: &str) -> Vec<String> {
    let mut problems = Vec::new();
    for build in WASM_BUILDS {
        // Whole commands, not substrings: the default build is a prefix of
        // the `--all-features` one.
        require(
            &mut problems,
            ci.lines().any(|line| ci_command(line) == build),
            "wasm-build",
            &format!("ci.yml must run `{build}`"),
        );
    }
    require(
        &mut problems,
        ci.lines().any(installs_wasm_target),
        "wasm-target",
        &format!("ci.yml must install the `{WASM_TARGET}` target (`targets: {WASM_TARGET}`)"),
    );
    // "Any failure fails the build."
    require(
        &mut problems,
        !ci.lines()
            .any(|line| line.trim().starts_with("continue-on-error:")),
        "wasm-continue-on-error",
        "ci.yml must not let a job's failure pass (`continue-on-error`)",
    );
    problems
}

/// `ci` without its one line that runs `command` (compared whole).
fn without_command(ci: &str, command: &str) -> String {
    let kept: Vec<&str> = ci
        .lines()
        .filter(|line| ci_command(line) != command)
        .collect();
    assert_eq!(
        ci.lines().count() - kept.len(),
        1,
        "LIB-QA-017: mutation target `{command}` must be one line of ci.yml"
    );
    kept.join("\n")
}

#[test]
fn lib_qa_017_ci_builds_sacm_for_wasm32() {
    let problems = wasm_ci_problems(&read(".github/workflows/ci.yml"));
    assert!(
        problems.is_empty(),
        "LIB-QA-017: ci.yml does not build sacm for {WASM_TARGET}: {problems:#?}"
    );
}

#[test]
fn lib_qa_017_wasm_ci_check_reports_a_removed_build_or_target() {
    let ci = read(".github/workflows/ci.yml");
    assert!(
        wasm_ci_problems(&ci).is_empty(),
        "LIB-QA-017: the mutations need a ci.yml that passes the check"
    );
    for build in WASM_BUILDS {
        let mutated = without_command(&ci, build);
        assert_eq!(
            problem_codes(&wasm_ci_problems(&mutated)),
            ["wasm-build"],
            "LIB-QA-017: removing `{build}` from ci.yml is not reported"
        );
    }
    let without_target: Vec<&str> = ci
        .lines()
        .filter(|line| !installs_wasm_target(line))
        .collect();
    assert_eq!(
        problem_codes(&wasm_ci_problems(&without_target.join("\n"))),
        ["wasm-target"],
        "LIB-QA-017: removing the {WASM_TARGET} target install from ci.yml is not reported"
    );
    let tolerated = format!("{ci}\n    continue-on-error: true\n");
    assert_eq!(
        problem_codes(&wasm_ci_problems(&tolerated)),
        ["wasm-continue-on-error"],
        "LIB-QA-017: a job with `continue-on-error: true` is not reported"
    );
}

/// The platform APIs that compile for `wasm32-unknown-unknown` but fail at
/// run time, as `clippy.toml` `disallowed-methods` paths: every free function
/// of `std::fs` (Rust 1.85), the listed thread, process and environment
/// functions, and the `std::path::Path` methods that touch the file system
/// (its lexical methods, such as `join` and `parent`, stay allowed).
const DISALLOWED_METHODS: [&str; 43] = [
    "std::fs::canonicalize",
    "std::fs::copy",
    "std::fs::create_dir",
    "std::fs::create_dir_all",
    "std::fs::exists",
    "std::fs::hard_link",
    "std::fs::metadata",
    "std::fs::read",
    "std::fs::read_dir",
    "std::fs::read_link",
    "std::fs::read_to_string",
    "std::fs::remove_dir",
    "std::fs::remove_dir_all",
    "std::fs::remove_file",
    "std::fs::rename",
    "std::fs::set_permissions",
    "std::fs::soft_link",
    "std::fs::symlink_metadata",
    "std::fs::write",
    "std::thread::spawn",
    "std::thread::sleep",
    "std::thread::scope",
    "std::process::exit",
    "std::process::abort",
    "std::env::var",
    "std::env::var_os",
    "std::env::vars",
    "std::env::vars_os",
    "std::env::args",
    "std::env::args_os",
    "std::env::current_dir",
    "std::env::set_current_dir",
    "std::env::temp_dir",
    "std::path::Path::exists",
    "std::path::Path::try_exists",
    "std::path::Path::is_file",
    "std::path::Path::is_dir",
    "std::path::Path::is_symlink",
    "std::path::Path::metadata",
    "std::path::Path::symlink_metadata",
    "std::path::Path::canonicalize",
    "std::path::Path::read_link",
    "std::path::Path::read_dir",
];

/// The platform types, as `clippy.toml` `disallowed-types` paths: the
/// `std::fs` handles, the thread builder, the process command, the system
/// clocks and the `std::net` sockets.
const DISALLOWED_TYPES: [&str; 11] = [
    "std::fs::File",
    "std::fs::OpenOptions",
    "std::fs::DirEntry",
    "std::fs::ReadDir",
    "std::thread::Builder",
    "std::process::Command",
    "std::time::Instant",
    "std::time::SystemTime",
    "std::net::TcpListener",
    "std::net::TcpStream",
    "std::net::UdpSocket",
];

/// The `clippy.toml` arrays and the paths each must list.
const DISALLOWED: [(&str, &[&str]); 2] = [
    ("disallowed-methods", &DISALLOWED_METHODS),
    ("disallowed-types", &DISALLOWED_TYPES),
];

/// LIB-QA-017: what is missing from `clippy.toml` for the platform-API lint.
/// Every entry must be `{ path = "...", reason = "..." }` with a non-empty
/// reason, and every listed path must be there.
fn platform_api_problems(clippy: &toml::Table) -> Vec<String> {
    let mut problems = Vec::new();
    for (key, expected) in DISALLOWED {
        let Some(entries) = clippy.get(key).and_then(toml::Value::as_array) else {
            problems.push(problem(
                "platform-api-missing",
                format!("clippy.toml has no `{key}` array"),
            ));
            continue;
        };
        let mut paths = BTreeSet::new();
        for entry in entries {
            let path = entry.get("path").and_then(toml::Value::as_str);
            let reason = entry.get("reason").and_then(toml::Value::as_str);
            if let Some(path) = path {
                paths.insert(path);
            }
            require(
                &mut problems,
                path.is_some() && reason.is_some_and(|reason| !reason.trim().is_empty()),
                "platform-api-reason",
                &format!("clippy.toml `{key}` entry {entry} has no path or no reason"),
            );
        }
        for path in expected.iter().filter(|path| !paths.contains(*path)) {
            problems.push(problem(
                "platform-api-missing",
                format!("clippy.toml `{key}` does not list `{path}`"),
            ));
        }
    }
    problems
}

/// The crate attribute that denies the two lints in `sacm`'s non-test code,
/// without white space.
const DENY_PLATFORM_APIS: &str =
    "#![cfg_attr(not(test),deny(clippy::disallowed_methods,clippy::disallowed_types))]";

/// LIB-QA-017: whether `lib_rs` holds the deny attribute, compared without
/// white space and outside `//` comments.
fn platform_deny_problems(lib_rs: &str) -> Vec<String> {
    let code: String = lib_rs
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .flat_map(str::chars)
        .filter(|c| !c.is_whitespace())
        .collect();
    let mut problems = Vec::new();
    require(
        &mut problems,
        code.contains(DENY_PLATFORM_APIS),
        "platform-lint-deny",
        "crates/sacm/src/lib.rs must have `#![cfg_attr(not(test), deny(clippy::disallowed_methods, clippy::disallowed_types))]`",
    );
    problems
}

/// LIB-QA-017: whether the root `Cargo.toml` allows the two lints
/// workspace-wide, as `"allow"` or `{ level = "allow", ... }`.
fn platform_allow_problems(cargo: &toml::Table) -> Vec<String> {
    let mut problems = Vec::new();
    for lint in ["disallowed_methods", "disallowed_types"] {
        let level = lookup(cargo, &["workspace", "lints", "clippy", lint]).and_then(|value| {
            value
                .as_str()
                .or_else(|| value.get("level").and_then(toml::Value::as_str))
        });
        require(
            &mut problems,
            level == Some("allow"),
            "platform-lint-allow",
            &format!("Cargo.toml [workspace.lints.clippy] must set `{lint} = \"allow\"`"),
        );
    }
    problems
}

#[test]
fn lib_qa_017_clippy_toml_disallows_the_platform_apis_with_reasons() {
    let problems = platform_api_problems(&toml_table("clippy.toml"));
    assert!(
        problems.is_empty(),
        "LIB-QA-017: clippy.toml does not disallow the platform APIs: {problems:#?}"
    );
}

#[test]
fn lib_qa_017_sacm_denies_the_platform_lints_outside_tests() {
    let mut problems = platform_deny_problems(&read("crates/sacm/src/lib.rs"));
    problems.extend(platform_allow_problems(&toml_table("Cargo.toml")));
    assert!(
        problems.is_empty(),
        "LIB-QA-017: the platform lints are not allowed workspace-wide and denied in sacm: {problems:#?}"
    );
}

#[test]
fn lib_qa_017_clippy_toml_check_reports_a_removed_path_or_reason() {
    let clippy = toml_table("clippy.toml");
    assert!(
        platform_api_problems(&clippy).is_empty(),
        "LIB-QA-017: the mutations need a clippy.toml that passes the check"
    );
    for (key, expected) in DISALLOWED {
        for path in expected {
            let mut mutated = clippy.clone();
            if let Some(entries) = mutated.get_mut(key).and_then(toml::Value::as_array_mut) {
                entries
                    .retain(|entry| entry.get("path").and_then(toml::Value::as_str) != Some(path));
            }
            assert_eq!(
                problem_codes(&platform_api_problems(&mutated)),
                ["platform-api-missing"],
                "LIB-QA-017: removing `{path}` from clippy.toml `{key}` is not reported"
            );
        }
        for bad in [
            format!("{{ path = \"{}\", reason = \"\" }}", expected[0]),
            format!("{{ path = \"{}\", reason = \"  \" }}", expected[0]),
            format!("{{ path = \"{}\" }}", expected[0]),
            format!("\"{}\"", expected[0]),
        ] {
            let mut mutated = clippy.clone();
            if let Some(entries) = mutated.get_mut(key).and_then(toml::Value::as_array_mut) {
                entries.push(toml_value(&bad));
            }
            assert_eq!(
                problem_codes(&platform_api_problems(&mutated)),
                ["platform-api-reason"],
                "LIB-QA-017: the clippy.toml `{key}` entry {bad} without a reason is not reported"
            );
        }
    }
}

#[test]
fn lib_qa_017_deny_and_allow_checks_report_a_removed_attribute_or_level() {
    let lib_rs = read("crates/sacm/src/lib.rs");
    assert!(
        platform_deny_problems(&lib_rs).is_empty(),
        "LIB-QA-017: the mutations need a lib.rs that passes the check"
    );
    let attribute = lib_rs
        .lines()
        .find(|line| {
            let compact: String = line.chars().filter(|c| !c.is_whitespace()).collect();
            compact == DENY_PLATFORM_APIS
        })
        .map(str::trim)
        .expect("LIB-QA-017: the deny attribute is one line of lib.rs");
    for mutated in [
        delete_line(&lib_rs, attribute),
        edit_line(&lib_rs, attribute, |line| {
            vec![format!("{}// {}", leading(line), line.trim())]
        }),
        replace_line(
            &lib_rs,
            attribute,
            "#![cfg_attr(not(test), deny(clippy::disallowed_methods))]",
        ),
        replace_line(
            &lib_rs,
            attribute,
            "#![deny(clippy::disallowed_methods, clippy::disallowed_types)]",
        ),
    ] {
        assert_eq!(
            problem_codes(&platform_deny_problems(&mutated)),
            ["platform-lint-deny"],
            "LIB-QA-017: a removed or changed deny attribute in lib.rs is not reported"
        );
    }

    let cargo = toml_table("Cargo.toml");
    assert!(
        platform_allow_problems(&cargo).is_empty(),
        "LIB-QA-017: the mutations need a Cargo.toml that passes the check"
    );
    for lint in ["disallowed_methods", "disallowed_types"] {
        for level in [None, Some("warn"), Some("deny")] {
            let mut mutated = cargo.clone();
            let clippy_lints = mutated
                .get_mut("workspace")
                .and_then(|workspace| workspace.get_mut("lints"))
                .and_then(|lints| lints.get_mut("clippy"))
                .and_then(toml::Value::as_table_mut)
                .expect("LIB-QA-017: Cargo.toml has [workspace.lints.clippy]");
            match level {
                None => {
                    clippy_lints.remove(lint);
                }
                Some(level) => {
                    clippy_lints.insert(lint.to_owned(), toml::Value::String(level.to_owned()));
                }
            }
            assert_eq!(
                problem_codes(&platform_allow_problems(&mutated)),
                ["platform-lint-allow"],
                "LIB-QA-017: `{lint}` at {level:?} in [workspace.lints.clippy] is not reported"
            );
        }
    }
}

/// The text of every `.rs` file under `crates/sacm/src`, by its path
/// relative to the repository root.
fn sacm_sources() -> BTreeMap<String, String> {
    fn walk(dir: &Path, out: &mut BTreeMap<String, String>) {
        let mut entries: Vec<PathBuf> = fs::read_dir(dir)
            .unwrap_or_else(|e| panic!("read {}: {e}", dir.display()))
            .map(|entry| entry.expect("a directory entry").path())
            .collect();
        entries.sort();
        for path in entries {
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                let rel = path
                    .strip_prefix(repository_root())
                    .expect("under the repository root")
                    .to_string_lossy()
                    .replace('\\', "/");
                let text = fs::read_to_string(&path)
                    .unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
                out.insert(rel, text);
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(&repository_root().join("crates/sacm/src"), &mut out);
    out
}

/// LIB-QA-017: every `allow(...)` or `expect(...)` in `sacm`'s source, an
/// attribute or inside a `cfg_attr`, that names `clippy::disallowed_methods`
/// or `clippy::disallowed_types`, which would undo the crate's deny for an
/// item. Lines are read without `//` comments and white space, so an
/// attribute over several lines is found.
fn inner_allow_problems(files: &BTreeMap<String, String>) -> Vec<String> {
    let mut problems = Vec::new();
    for (path, text) in files {
        let code: String = text
            .lines()
            .map(|line| line.find("//").map_or(line, |at| &line[..at]))
            .flat_map(str::chars)
            .filter(|c| !c.is_whitespace())
            .collect();
        let bytes = code.as_bytes();
        for level in ["allow(", "expect("] {
            for (at, _) in code.match_indices(level) {
                let preceded_by_ident = at
                    .checked_sub(1)
                    .is_some_and(|b| bytes[b].is_ascii_alphanumeric() || bytes[b] == b'_');
                if preceded_by_ident {
                    continue;
                }
                let open = at + level.len() - 1;
                let mut depth = 0usize;
                let close = bytes[open..]
                    .iter()
                    .position(|&byte| {
                        match byte {
                            b'(' => depth += 1,
                            b')' => depth -= 1,
                            _ => {}
                        }
                        depth == 0
                    })
                    .map_or(bytes.len(), |offset| open + offset);
                let inside = &code[open..close];
                for lint in ["clippy::disallowed_methods", "clippy::disallowed_types"] {
                    let named = inside.match_indices(lint).any(|(i, _)| {
                        !inside[i + lint.len()..]
                            .starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_')
                    });
                    if named {
                        problems.push(problem(
                            "platform-lint-inner-allow",
                            format!("{path} has `{level}{lint}...)`, which undoes the deny"),
                        ));
                    }
                }
            }
        }
    }
    problems
}

#[test]
fn lib_qa_017_sacm_source_has_no_inner_allow_of_the_platform_lints() {
    let files = sacm_sources();
    assert!(
        files.contains_key("crates/sacm/src/lib.rs") && files.len() > 10,
        "LIB-QA-017: setup: the walk did not find sacm's source: {:?}",
        files.keys().collect::<Vec<_>>()
    );
    let problems = inner_allow_problems(&files);
    assert!(
        problems.is_empty(),
        "LIB-QA-017: sacm's source allows a platform lint the crate denies: {problems:#?}"
    );
    let with = |extra: &str| {
        let mut mutated = files.clone();
        mutated
            .get_mut("crates/sacm/src/lib.rs")
            .expect("LIB-QA-017: lib.rs")
            .push_str(extra);
        mutated
    };
    for (what, extra, count) in [
        (
            "an item allow",
            "\n#[allow(clippy::disallowed_methods)]\nfn f() {}\n",
            1,
        ),
        (
            "an inner expect with a reason",
            "\nmod m {\n    #![expect(clippy::disallowed_types, reason = \"x\")]\n}\n",
            1,
        ),
        (
            "an allow in a cfg_attr, over several lines, with another lint",
            "\n#[cfg_attr(\n    target_arch = \"wasm32\",\n    allow(\n        dead_code,\n        \
             clippy::disallowed_methods\n    )\n)]\nfn f() {}\n",
            1,
        ),
        (
            "both lints in one allow",
            "\n#[allow(clippy::disallowed_types, clippy::disallowed_methods)]\nfn f() {}\n",
            2,
        ),
    ] {
        assert_eq!(
            problem_codes(&inner_allow_problems(&with(extra))),
            vec!["platform-lint-inner-allow".to_owned(); count],
            "LIB-QA-017: {what} is not reported"
        );
    }
    for (what, extra) in [
        (
            "a comment",
            "\n// #[allow(clippy::disallowed_methods)]\nfn f() {}\n",
        ),
        (
            "another lint",
            "\n#[allow(clippy::disallowed_script_idents)]\nfn f() {}\n",
        ),
        (
            "a deny",
            "\n#[deny(clippy::disallowed_methods)]\nfn f() {}\n",
        ),
        (
            "a longer lint name",
            "\n#[allow(clippy::disallowed_methods_x)]\nfn f() {}\n",
        ),
    ] {
        assert!(
            inner_allow_problems(&with(extra)).is_empty(),
            "LIB-QA-017: {what} is reported as an inner allow"
        );
    }
}

// ---------------------------------------------------------------------------
// LIB-QA-018: documentation-only pushes skip the code-only CI jobs
// ---------------------------------------------------------------------------

/// The `changes` job's output, read by every gate.
const CHANGES_OUTPUT: &str = "code: ${{ steps.diff.outputs.code }}";

/// The detection script's rule: only a file outside `docs/` makes `code`
/// true. Any other pattern widens or narrows what counts as documentation.
const DOCS_RULE: &str = "if ! echo \"$files\" | grep -qv '^docs/'; then code=false; fi";

/// The detection script's fail-open default.
const CHANGES_DEFAULT: &str = "code=true";

/// The detection script's output.
const CHANGES_WRITE: &str = "echo \"code=$code\" >> \"$GITHUB_OUTPUT\"";

/// The `changes` job's permission to read workflow runs, which the
/// `previous` step's query needs.
const CHANGES_ACTIONS_READ: &str = "actions: read";

/// The `previous` step's query: the successful runs of this workflow on the
/// previous commit, from the same event type as this run, so that a
/// successful run of another event does not count.
const PREVIOUS_QUERY: &str =
    "actions/workflows/ci.yml/runs?head_sha=$BEFORE&status=success&event=$EVENT";

/// The `previous` step's input the event filter reads.
const PREVIOUS_EVENT: &str = "EVENT: ${{ github.event_name }}";

/// The `previous` step's fail-closed answer, and its output.
const PREVIOUS_DEFAULT: &str = "ok=false";
const PREVIOUS_WRITE: &str = "echo \"ok=$ok\" >> \"$GITHUB_OUTPUT\"";

/// The `diff` step reads the `previous` step's answer.
const PREVIOUS_INPUT: &str = "PREVIOUS_OK: ${{ steps.previous.outputs.ok }}";

/// The ranges of the latest push, which need CI to have succeeded on the
/// commit before them: a pull request's new commits, and a push.
const SYNCHRONIZE_RANGE: &str = "if [ \"$EVENT\" = pull_request ] && [ \"$ACTION\" = synchronize ]; then range=\"$BEFORE $AFTER\"; needs_previous=true";
const PUSH_RANGE: &str =
    "elif [ \"$EVENT\" = push ]; then range=\"$BEFORE $AFTER\"; needs_previous=true";

/// A docs-only result that needs the previous commit's CI runs everything
/// unless that CI succeeded.
const PREVIOUS_OVERRIDE: &str = "if [ \"$code\" = false ] && [ \"$needs_previous\" = true ] && [ \"${PREVIOUS_OK:-}\" != true ]; then";

/// The `test` job runs even when `changes` failed.
const TEST_JOB_GATE: &str = "if: ${{ !cancelled() }}";

/// Each `test` step: skipped only on an explicit `false`, and never on
/// Ubuntu.
const TEST_STEP_GATE: &str =
    "if: needs.changes.outputs.code != 'false' || matrix.os == 'ubuntu-latest'";

const MSRV_GATE: &str = "if: ${{ !cancelled() && needs.changes.outputs.code != 'false' }}";

const MUTANTS_SHARD_GATE: &str = "if: ${{ !cancelled() && github.event_name == 'pull_request' && needs.changes.outputs.code != 'false' }}";

/// The `mutants (PR diff)` aggregator: it runs whatever the shards did, and
/// passes when every shard passed, or when `changes` succeeded with `false`
/// and the shards were skipped.
const MUTANTS_GATE: &str = "if: always() && github.event_name == 'pull_request'";
const MUTANTS_NEEDS: &str = "needs: [changes, mutants-shard]";
const MUTANTS_RUN: &str = "run: test \"$RESULT\" = success || { test \"$CHANGES\" = success && test \"$CODE\" = false && test \"$RESULT\" = skipped; }";
const MUTANTS_ENV: [&str; 3] = [
    "RESULT: ${{ needs.mutants-shard.result }}",
    "CHANGES: ${{ needs.changes.result }}",
    "CODE: ${{ needs.changes.outputs.code }}",
];

/// The jobs that may read `changes`; every other job always runs.
const CHANGES_JOBS: [&str; 5] = ["changes", "test", "msrv", "mutants-shard", "mutants"];

/// The jobs of a workflow, in order: each key two spaces deep under `jobs:`,
/// with the lines of its body. Comment lines are dropped; blank lines kept.
fn workflow_jobs(ci: &str) -> Vec<(String, Vec<&str>)> {
    let mut jobs: Vec<(String, Vec<&str>)> = Vec::new();
    let mut in_jobs = false;
    for line in ci.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            continue;
        }
        if !trimmed.is_empty() && indent(line) == 0 {
            in_jobs = trimmed == "jobs:";
            continue;
        }
        if !in_jobs {
            continue;
        }
        if indent(line) == 2 && !trimmed.contains(' ') {
            if let Some(name) = trimmed.strip_suffix(':') {
                jobs.push((name.to_owned(), Vec::new()));
                continue;
            }
        }
        if let Some((_, body)) = jobs.last_mut() {
            body.push(line);
        }
    }
    jobs
}

/// The body of the job `name`, empty when there is none.
fn job<'a>(jobs: &'a [(String, Vec<&'a str>)], name: &str) -> &'a [&'a str] {
    jobs.iter()
        .find(|(job, _)| job == name)
        .map_or(&[], |(_, body)| body.as_slice())
}

/// Whether the job `body` has the job-level line (four spaces deep) `key`.
fn has_job_line(body: &[&str], key: &str) -> bool {
    body.iter()
        .any(|line| indent(line) == 4 && line.trim() == key)
}

/// A step line without its indent and `- ` list marker.
fn step_line(line: &str) -> &str {
    let line = line.trim();
    line.strip_prefix("- ").unwrap_or(line)
}

/// The steps of the job `body`: each `- ` item six spaces deep under its
/// `steps:`, with its lines.
fn job_steps<'a>(body: &[&'a str]) -> Vec<Vec<&'a str>> {
    let mut steps: Vec<Vec<&str>> = Vec::new();
    let mut in_steps = false;
    for &line in body {
        if line.trim().is_empty() {
            continue;
        }
        if indent(line) <= 4 {
            in_steps = line.trim() == "steps:";
            continue;
        }
        if !in_steps {
            continue;
        }
        if indent(line) == 6 && line.trim_start().starts_with("- ") {
            steps.push(vec![line]);
        } else if let Some(step) = steps.last_mut() {
            step.push(line);
        }
    }
    steps
}

/// The `diff` step's script: the detection script.
fn detection_script(ci: &str) -> Option<String> {
    changes_step_script(ci, "diff")
}

/// The `previous` step's script: whether CI succeeded on the previous commit.
fn previous_script(ci: &str) -> Option<String> {
    changes_step_script(ci, "previous")
}

/// The lines of the `changes` job's step `id`, or none.
fn changes_step<'a>(jobs: &'a [(String, Vec<&'a str>)], id: &str) -> Vec<&'a str> {
    let id_line = format!("id: {id}");
    job_steps(job(jobs, "changes"))
        .into_iter()
        .find(|step| step.iter().any(|line| step_line(line) == id_line))
        .unwrap_or_default()
}

/// The `run: |` script of the `changes` job's step `id`, dedented, or `None`
/// when there is none.
fn changes_step_script(ci: &str, id: &str) -> Option<String> {
    let jobs = workflow_jobs(ci);
    let step = changes_step(&jobs, id);
    let start = step.iter().position(|line| step_line(line) == "run: |")?;
    let depth = indent(step[start]);
    let body: Vec<&str> = step[start + 1..]
        .iter()
        .copied()
        .take_while(|line| line.trim().is_empty() || indent(line) > depth)
        .collect();
    let margin = body
        .iter()
        .filter(|line| !line.trim().is_empty())
        .map(|line| indent(line))
        .min()?;
    let mut script = String::new();
    for line in body {
        script.push_str(line.get(margin..).unwrap_or(""));
        script.push('\n');
    }
    Some(script)
}

/// LIB-QA-018: what is wrong with `ci.yml`'s `changes` job and its
/// detection script.
fn changes_job_problems(ci: &str, jobs: &[(String, Vec<&str>)]) -> Vec<String> {
    let mut problems = Vec::new();
    let changes = job(jobs, "changes");
    let diff_step = job_steps(changes)
        .into_iter()
        .any(|step| step.iter().any(|line| step_line(line) == "id: diff"));
    require(
        &mut problems,
        changes.iter().any(|line| line.trim() == "outputs:")
            && changes.iter().any(|line| line.trim() == CHANGES_OUTPUT)
            && changes.iter().any(|line| line.trim() == "fetch-depth: 0")
            && diff_step,
        "changes-job",
        &format!(
            "ci.yml must have a `changes` job with the output `{CHANGES_OUTPUT}`, a `diff` step \
             and `fetch-depth: 0`"
        ),
    );
    let script = detection_script(ci).unwrap_or_default();
    let script: Vec<&str> = script.lines().map(str::trim).collect();
    let default_at = script.iter().position(|line| *line == CHANGES_DEFAULT);
    let first_false = script.iter().position(|line| line.contains("code=false"));
    require(
        &mut problems,
        default_at.is_some_and(|at| first_false.is_none_or(|first| at < first))
            && script.contains(&CHANGES_WRITE),
        "changes-default",
        &format!(
            "the detection script must start from `{CHANGES_DEFAULT}` (fail open) and write \
             `{CHANGES_WRITE}`"
        ),
    );
    let greps: Vec<&str> = script
        .iter()
        .copied()
        .filter(|line| line.contains("grep"))
        .collect();
    require(
        &mut problems,
        greps == [DOCS_RULE],
        "changes-rule",
        &format!("the detection script's only rule must be `{DOCS_RULE}`"),
    );
    previous_commit_problems(ci, jobs, &script, &mut problems);
    problems
}

/// LIB-QA-018: what is wrong with the check that CI succeeded on the
/// previous commit before a docs-only latest push skips anything.
fn previous_commit_problems(
    ci: &str,
    jobs: &[(String, Vec<&str>)],
    script: &[&str],
    problems: &mut Vec<String>,
) {
    let changes = job(jobs, "changes");
    let permissions = changes
        .iter()
        .position(|line| indent(line) == 4 && line.trim() == "permissions:");
    require(
        problems,
        permissions.is_some_and(|at| {
            changes[at + 1..]
                .iter()
                .take_while(|line| indent(line) > 4)
                .any(|line| line.trim() == CHANGES_ACTIONS_READ)
        }),
        "changes-permissions",
        &format!("the `changes` job's `permissions:` must have `{CHANGES_ACTIONS_READ}`"),
    );
    let previous = changes_step(jobs, "previous");
    let previous_script = previous_script(ci).unwrap_or_default();
    let previous_lines: Vec<&str> = previous_script.lines().map(str::trim).collect();
    require(
        problems,
        previous
            .iter()
            .any(|line| step_line(line) == "BEFORE: ${{ github.event.before }}")
            && previous
                .iter()
                .any(|line| step_line(line) == PREVIOUS_EVENT)
            && previous_script.contains(PREVIOUS_QUERY)
            && previous_lines.first() == Some(&PREVIOUS_DEFAULT)
            && previous_lines.contains(&PREVIOUS_WRITE),
        "changes-previous",
        &format!(
            "the `changes` job must have a `previous` step that reads `BEFORE` and \
             `{PREVIOUS_EVENT}`, starts from `{PREVIOUS_DEFAULT}`, queries `{PREVIOUS_QUERY}` \
             and writes `{PREVIOUS_WRITE}`"
        ),
    );
    require(
        problems,
        changes_step(jobs, "diff")
            .iter()
            .any(|line| step_line(line) == PREVIOUS_INPUT),
        "changes-previous-input",
        &format!("the `diff` step must read `{PREVIOUS_INPUT}`"),
    );
    let at = |line: &str| script.iter().position(|l| *l == line);
    let override_at = at(PREVIOUS_OVERRIDE);
    let sets_code = override_at.is_some_and(|start| {
        script[start + 1..]
            .iter()
            .take_while(|line| **line != "fi")
            .any(|line| *line == CHANGES_DEFAULT)
    });
    require(
        problems,
        at(SYNCHRONIZE_RANGE).is_some()
            && at(PUSH_RANGE).is_some()
            && sets_code
            && override_at > at(DOCS_RULE)
            && at(CHANGES_WRITE).is_none_or(|write| override_at < Some(write)),
        "changes-override",
        &format!(
            "the detection script must set `needs_previous=true` for a `synchronize` and a \
             push range, and after the rule apply `{PREVIOUS_OVERRIDE}` with `{CHANGES_DEFAULT}`"
        ),
    );
}

/// LIB-QA-018: what is wrong with `ci.yml`'s documentation-only gates.
fn docs_only_ci_problems(ci: &str) -> Vec<String> {
    let jobs = workflow_jobs(ci);
    let mut problems = changes_job_problems(ci, &jobs);

    let test = job(&jobs, "test");
    require(
        &mut problems,
        has_job_line(test, "needs: changes") && has_job_line(test, TEST_JOB_GATE),
        "test-job-gate",
        &format!("the `test` job must have `needs: changes` and `{TEST_JOB_GATE}`"),
    );
    let steps = job_steps(test);
    require(
        &mut problems,
        !steps.is_empty()
            && steps
                .iter()
                .all(|step| step.iter().any(|line| step_line(line) == TEST_STEP_GATE)),
        "test-step-gate",
        &format!("every step of the `test` job must have `{TEST_STEP_GATE}`"),
    );

    for (name, gate, code) in [
        ("msrv", MSRV_GATE, "msrv-gate"),
        ("mutants-shard", MUTANTS_SHARD_GATE, "mutants-shard-gate"),
    ] {
        let body = job(&jobs, name);
        require(
            &mut problems,
            has_job_line(body, "needs: changes") && has_job_line(body, gate),
            code,
            &format!("the `{name}` job must have `needs: changes` and `{gate}`"),
        );
    }

    let mutants = job(&jobs, "mutants");
    require(
        &mut problems,
        has_job_line(mutants, MUTANTS_NEEDS)
            && has_job_line(mutants, MUTANTS_GATE)
            && MUTANTS_ENV
                .iter()
                .chain([&MUTANTS_RUN])
                .all(|key| mutants.iter().any(|line| step_line(line) == *key)),
        "mutants-gate",
        &format!(
            "the `mutants` job must have `{MUTANTS_NEEDS}`, `{MUTANTS_GATE}`, the env \
             {MUTANTS_ENV:?} and `{MUTANTS_RUN}`"
        ),
    );

    for (name, body) in &jobs {
        if CHANGES_JOBS.contains(&name.as_str()) {
            continue;
        }
        let reads_changes = body.iter().any(|line| {
            line.contains("needs.changes")
                || (indent(line) == 4
                    && line.trim().starts_with("needs:")
                    && line.contains("changes"))
        });
        require(
            &mut problems,
            !reads_changes,
            "other-job-gated",
            &format!("the `{name}` job must not depend on `changes`: it always runs"),
        );
    }
    problems
}

/// `text` with the `n`th (0-based) line equal (trimmed) to `target`
/// replaced by `replacement`, keeping its indentation, or deleted when
/// `replacement` is `None`. Panics if there is no such line.
fn edit_nth_line(text: &str, target: &str, n: usize, replacement: Option<&str>) -> String {
    let hits = text.lines().filter(|line| line.trim() == target).count();
    assert!(
        n < hits,
        "mutation target `{target}` occurs {hits} times, not more than {n}"
    );
    let mut seen = 0;
    let mut out = String::new();
    for line in text.lines() {
        if line.trim() == target {
            seen += 1;
            if seen == n + 1 {
                if let Some(replacement) = replacement {
                    out.push_str(leading(line));
                    out.push_str(replacement);
                    out.push('\n');
                }
                continue;
            }
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// `ci.yml` with LF line ends, so that multi-line mutations match on every
/// platform.
fn ci_text() -> String {
    read(".github/workflows/ci.yml").replace("\r\n", "\n")
}

#[test]
fn lib_qa_018_ci_skips_only_the_code_only_jobs_on_a_docs_only_push() {
    let problems = docs_only_ci_problems(&ci_text());
    assert!(
        problems.is_empty(),
        "LIB-QA-018: ci.yml's documentation-only gates are wrong: {problems:#?}"
    );
}

#[test]
fn lib_qa_018_the_detection_script_is_found_with_its_rule_and_default() {
    let Some(script) = detection_script(&ci_text()) else {
        panic!("LIB-QA-018: ci.yml's `changes` job has no `run: |` script in its `diff` step");
    };
    let lines: Vec<&str> = script.lines().map(str::trim).collect();
    for line in [CHANGES_DEFAULT, DOCS_RULE, CHANGES_WRITE] {
        assert!(
            lines.contains(&line),
            "LIB-QA-018: the detection script has no line `{line}`:\n{script}"
        );
    }
}

/// A mutation of `ci.yml`: what it does, the mutated text, and the one
/// problem code it must give.
type Mutation = (String, String, &'static str);

/// Mutations of the `changes` job and its detection script.
fn changes_mutations(ci: &str) -> Vec<Mutation> {
    let mut mutations: Vec<Mutation> = vec![
        // The `changes` job.
        (
            "the `code` output removed".to_owned(),
            delete_line(ci, CHANGES_OUTPUT),
            "changes-job",
        ),
        (
            "the `changes` job's full history removed".to_owned(),
            edit_nth_line(ci, "fetch-depth: 0", 0, None),
            "changes-job",
        ),
        (
            "the default made `false` (fail closed)".to_owned(),
            edit_nth_line(ci, CHANGES_DEFAULT, 0, Some("code=false")),
            "changes-default",
        ),
        (
            "the output no longer written".to_owned(),
            delete_line(ci, CHANGES_WRITE),
            "changes-default",
        ),
        (
            "a second rule added".to_owned(),
            insert_after(
                ci,
                DOCS_RULE,
                0,
                &["if ! echo \"$files\" | grep -qv '^crates/'; then code=false; fi"],
            ),
            "changes-rule",
        ),
    ];
    mutations.extend(previous_commit_mutations(ci));
    for widened in [
        "'^docs'",
        "'docs/'",
        "'^d'",
        "'^[a-z]*/'",
        "'^docs/\\|^src/'",
    ] {
        mutations.push((
            format!("the rule widened to {widened}"),
            replace_line(
                ci,
                DOCS_RULE,
                &format!("if ! echo \"$files\" | grep -qv {widened}; then code=false; fi"),
            ),
            "changes-rule",
        ));
    }
    mutations
}

/// `ci` without the lines from the one starting (trimmed) with `first` up to
/// and including the one equal (trimmed) to `last`.
fn without_block(ci: &str, first: &str, last: &str) -> String {
    let start = ci
        .lines()
        .position(|line| line.trim().starts_with(first))
        .unwrap_or_else(|| panic!("mutation target `{first}` is not in ci.yml"));
    let length = ci
        .lines()
        .skip(start)
        .position(|line| line.trim() == last)
        .unwrap_or_else(|| panic!("mutation target `{last}` does not follow `{first}`"));
    let mut out = String::new();
    for (n, line) in ci.lines().enumerate() {
        if n < start || n > start + length {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

/// Mutations of the check that CI succeeded on the previous commit.
fn previous_commit_mutations(ci: &str) -> Vec<Mutation> {
    vec![
        (
            "`actions: read` dropped".to_owned(),
            delete_line(ci, CHANGES_ACTIONS_READ),
            "changes-permissions",
        ),
        (
            "the `previous` step removed".to_owned(),
            without_block(
                ci,
                "- id: previous",
                "echo \"ok=$ok\" >> \"$GITHUB_OUTPUT\"",
            ),
            "changes-previous",
        ),
        (
            "the `status=success` filter removed".to_owned(),
            replace_once(ci, "&status=success", ""),
            "changes-previous",
        ),
        (
            "the `event` filter removed, so a run of any event counts".to_owned(),
            replace_once(ci, "&event=$EVENT", ""),
            "changes-previous",
        ),
        (
            "the `event` filter on a fixed event".to_owned(),
            replace_once(ci, "&event=$EVENT", "&event=push"),
            "changes-previous",
        ),
        (
            "the `previous` step not reading the event name".to_owned(),
            edit_nth_line(ci, PREVIOUS_EVENT, 0, None),
            "changes-previous",
        ),
        (
            "the previous commit assumed green".to_owned(),
            replace_line(ci, PREVIOUS_DEFAULT, "ok=true"),
            "changes-previous",
        ),
        (
            "the `diff` step not reading the answer".to_owned(),
            delete_line(ci, PREVIOUS_INPUT),
            "changes-previous-input",
        ),
        (
            "the `needs_previous` override removed".to_owned(),
            without_block(ci, PREVIOUS_OVERRIDE, "fi"),
            "changes-override",
        ),
        (
            "the override failing open no longer (`= false`)".to_owned(),
            replace_line(
                ci,
                PREVIOUS_OVERRIDE,
                &PREVIOUS_OVERRIDE.replace("!= true", "= false"),
            ),
            "changes-override",
        ),
        (
            "`needs_previous=false` for synchronize".to_owned(),
            replace_line(
                ci,
                SYNCHRONIZE_RANGE,
                &SYNCHRONIZE_RANGE.replace("needs_previous=true", "needs_previous=false"),
            ),
            "changes-override",
        ),
        (
            "`needs_previous=false` for a push".to_owned(),
            replace_line(
                ci,
                PUSH_RANGE,
                &PUSH_RANGE.replace("needs_previous=true", "needs_previous=false"),
            ),
            "changes-override",
        ),
    ]
}

/// Mutations of the `test` job's gates, one set per step gate.
fn test_job_mutations(ci: &str) -> Vec<Mutation> {
    let step_gates = ci
        .lines()
        .filter(|line| line.trim() == TEST_STEP_GATE)
        .count();
    let last_test_step = format!(
        "      - run: cargo test -p xtask
        {TEST_STEP_GATE}
"
    );
    let mut mutations: Vec<Mutation> = vec![
        // The `test` job.
        (
            "the `test` job's `!cancelled()` removed".to_owned(),
            delete_line(ci, TEST_JOB_GATE),
            "test-job-gate",
        ),
        (
            "the `test` job gated as a whole (skipping Ubuntu)".to_owned(),
            replace_line(
                ci,
                TEST_JOB_GATE,
                "if: ${{ !cancelled() && needs.changes.outputs.code != 'false' }}",
            ),
            "test-job-gate",
        ),
        (
            "the `test` job's `needs: changes` removed".to_owned(),
            edit_nth_line(ci, "needs: changes", 0, None),
            "test-job-gate",
        ),
        (
            "an ungated step added to the `test` job".to_owned(),
            replace_once(
                ci,
                &last_test_step,
                &format!("{last_test_step}      - run: cargo test --doc\n"),
            ),
            "test-step-gate",
        ),
    ];
    for n in 0..step_gates {
        for (what, replacement) in [
            ("removed", None),
            (
                "inverted to `== 'true'`",
                Some("if: needs.changes.outputs.code == 'true' || matrix.os == 'ubuntu-latest'"),
            ),
            (
                "without the Ubuntu exception",
                Some("if: needs.changes.outputs.code != 'false'"),
            ),
        ] {
            mutations.push((
                format!("test step {n}'s gate {what}"),
                edit_nth_line(ci, TEST_STEP_GATE, n, replacement),
                "test-step-gate",
            ));
        }
    }
    mutations
}

/// Mutations of the `msrv`, `mutants-shard` and `mutants` gates, and a
/// gate on a job that must always run.
fn other_job_mutations(ci: &str) -> Vec<Mutation> {
    vec![
        // MSRV and the mutant shards.
        (
            "the `msrv` gate removed".to_owned(),
            delete_line(ci, MSRV_GATE),
            "msrv-gate",
        ),
        (
            "the `msrv` gate inverted to `== 'true'`".to_owned(),
            replace_line(
                ci,
                MSRV_GATE,
                "if: ${{ !cancelled() && needs.changes.outputs.code == 'true' }}",
            ),
            "msrv-gate",
        ),
        (
            "the `msrv` gate without `!cancelled()` (fail closed)".to_owned(),
            replace_line(
                ci,
                MSRV_GATE,
                "if: ${{ needs.changes.outputs.code != 'false' }}",
            ),
            "msrv-gate",
        ),
        (
            "the `msrv` job's `needs: changes` removed".to_owned(),
            edit_nth_line(ci, "needs: changes", 1, None),
            "msrv-gate",
        ),
        (
            "the `mutants-shard` gate removed".to_owned(),
            delete_line(ci, MUTANTS_SHARD_GATE),
            "mutants-shard-gate",
        ),
        (
            "the `mutants-shard` gate inverted to `== 'true'`".to_owned(),
            replace_line(
                ci,
                MUTANTS_SHARD_GATE,
                "if: ${{ !cancelled() && github.event_name == 'pull_request' && needs.changes.outputs.code == 'true' }}",
            ),
            "mutants-shard-gate",
        ),
        (
            "the `mutants-shard` job's `needs: changes` removed".to_owned(),
            edit_nth_line(ci, "needs: changes", 2, None),
            "mutants-shard-gate",
        ),
        // The aggregator.
        (
            "the aggregator passing any skipped shard".to_owned(),
            replace_line(
                ci,
                MUTANTS_RUN,
                "run: test \"$RESULT\" = success || test \"$RESULT\" = skipped",
            ),
            "mutants-gate",
        ),
        (
            "the aggregator not reading `code`".to_owned(),
            delete_line(ci, MUTANTS_ENV[2]),
            "mutants-gate",
        ),
        (
            "the aggregator not needing `changes`".to_owned(),
            replace_line(ci, MUTANTS_NEEDS, "needs: mutants-shard"),
            "mutants-gate",
        ),
        (
            "the aggregator not run when a shard is skipped".to_owned(),
            replace_line(ci, MUTANTS_GATE, "if: github.event_name == 'pull_request'"),
            "mutants-gate",
        ),
        // Every other job still runs.
        (
            "the rustdoc job gated".to_owned(),
            insert_after(
                ci,
                "name: rustdoc",
                0,
                &[
                    "needs: changes",
                    "if: needs.changes.outputs.code != 'false'",
                ],
            ),
            "other-job-gated",
        ),
    ]
}

#[test]
fn lib_qa_018_ci_check_reports_a_removed_inverted_or_widened_gate() {
    let ci = ci_text();
    assert!(
        docs_only_ci_problems(&ci).is_empty(),
        "LIB-QA-018: the mutations need a ci.yml that passes the check"
    );
    assert_eq!(
        ci.lines()
            .filter(|line| line.trim() == TEST_STEP_GATE)
            .count(),
        job_steps(job(&workflow_jobs(&ci), "test")).len(),
        "LIB-QA-018: setup: not one step gate per test step"
    );
    let mutations: Vec<Mutation> = changes_mutations(&ci)
        .into_iter()
        .chain(test_job_mutations(&ci))
        .chain(other_job_mutations(&ci))
        .collect();
    for (what, mutated, code) in &mutations {
        assert_eq!(
            problem_codes(&docs_only_ci_problems(mutated)),
            [*code],
            "LIB-QA-018: {what} is not reported as only `{code}`"
        );
    }

    // Without the `changes` job, its checks report it.
    let start = ci
        .find("\n  changes:\n")
        .expect("LIB-QA-018: setup: the changes job");
    let end = ci[start + 1..]
        .find("\n  fmt:\n")
        .map(|at| start + 1 + at)
        .expect("LIB-QA-018: setup: the job after changes");
    let without_changes = format!("{}{}", &ci[..start], &ci[end..]);
    let problems = docs_only_ci_problems(&without_changes);
    assert!(
        has_code(&problems, "changes-job")
            && has_code(&problems, "changes-default")
            && has_code(&problems, "changes-rule"),
        "LIB-QA-018: a removed `changes` job is not reported: {problems:#?}"
    );
    // Without the `diff` step, the job and its script are reported.
    let problems = docs_only_ci_problems(&replace_line(&ci, "- id: diff", "- id: detect"));
    assert!(
        has_code(&problems, "changes-job") && has_code(&problems, "changes-rule"),
        "LIB-QA-018: a renamed `diff` step is not reported: {problems:#?}"
    );
}

// --- The detection script, run on a scratch repository ------------------------------

/// Whether `program --version` runs and succeeds.
fn runs(program: &Path) -> bool {
    Command::new(program)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// A bash to run the workflow's scripts with, as GitHub's runners do. On
/// Windows, `bash` would resolve to System32's WSL launcher before `PATH`,
/// so a Git for Windows or MSYS2 bash is looked for on `PATH` and next to
/// `git`.
fn find_bash() -> Option<PathBuf> {
    if !cfg!(windows) {
        let bash = PathBuf::from("bash");
        return runs(&bash).then_some(bash);
    }
    let mut candidates = Vec::new();
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            let lower = dir.to_string_lossy().to_lowercase();
            if !lower.contains("system32") && !lower.contains("windowsapps") {
                candidates.push(dir.join("bash.exe"));
            }
        }
    }
    if let Ok(output) = Command::new("git").arg("--exec-path").output() {
        // <root>/mingw64/libexec/git-core, or <root>/ucrt64/libexec/git-core.
        let exec_path = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());
        if let Some(root) = exec_path.ancestors().nth(3) {
            candidates.push(root.join("bin").join("bash.exe"));
            candidates.push(root.join("usr").join("bin").join("bash.exe"));
        }
    }
    candidates
        .into_iter()
        .find(|candidate| candidate.is_file() && runs(candidate))
}

/// The bash to run the scripts with, or `None` to skip because bash or git
/// is missing on this machine. On CI, where both always exist, a missing
/// one fails the test instead.
fn script_runner(test: &str) -> Option<PathBuf> {
    let bash = find_bash();
    let git = runs(Path::new("git"));
    if let (Some(bash), true) = (&bash, git) {
        return Some(bash.clone());
    }
    let missing = if git { "bash" } else { "git" };
    assert!(
        std::env::var_os("CI").is_none(),
        "LIB-QA-018: {test}: {missing} is not available on CI"
    );
    skip(test, missing);
    None
}

/// Says that the test `test` is skipped because `missing` is not available.
#[expect(
    clippy::print_stderr,
    reason = "a skipped LIB-QA-018 check must be seen in the test output"
)]
fn skip(test: &str, missing: &str) {
    eprintln!("LIB-QA-018: {test}: skipped: {missing} is not available on this machine");
}

/// A path as bash and git read it on every platform: with `/` separators.
fn posix(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// A fresh git repository under the system temp directory, removed on drop.
/// Git runs with no global or system configuration.
struct ScratchRepo {
    root: PathBuf,
    repo: PathBuf,
    config: PathBuf,
}

impl ScratchRepo {
    fn new(test: &str) -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let root = std::env::temp_dir().join(format!(
            "xtask-lib-qa-018-{test}-{}-{nanos}",
            std::process::id()
        ));
        let repo = root.join("repo");
        fs::create_dir_all(&repo)
            .unwrap_or_else(|e| panic!("LIB-QA-018: create {}: {e}", repo.display()));
        let config = root.join("gitconfig");
        fs::write(&config, "")
            .unwrap_or_else(|e| panic!("LIB-QA-018: write {}: {e}", config.display()));
        let scratch = Self { root, repo, config };
        scratch.git(&["init", "--quiet"]);
        scratch
    }

    /// `program` run in the repository, with its git configuration.
    fn command(&self, program: &Path) -> Command {
        let mut command = Command::new(program);
        command
            .current_dir(&self.repo)
            .env("GIT_CONFIG_GLOBAL", posix(&self.config))
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_AUTHOR_NAME", "LIB-QA-018")
            .env("GIT_AUTHOR_EMAIL", "lib-qa-018@example.invalid")
            .env("GIT_COMMITTER_NAME", "LIB-QA-018")
            .env("GIT_COMMITTER_EMAIL", "lib-qa-018@example.invalid");
        command
    }

    /// Runs git with `args`, which must succeed, and returns its output.
    fn git(&self, args: &[&str]) -> String {
        let output = self
            .command(Path::new("git"))
            .args(["-c", "commit.gpgsign=false", "-c", "core.autocrlf=false"])
            .args(args)
            .output()
            .unwrap_or_else(|e| panic!("LIB-QA-018: run git {args:?}: {e}"));
        assert!(
            output.status.success(),
            "LIB-QA-018: git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    }

    /// Writes `text` to the file `rel` of the work tree.
    fn write(&self, rel: &str, text: &str) {
        let path = self.repo.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .unwrap_or_else(|e| panic!("LIB-QA-018: create {}: {e}", parent.display()));
        }
        fs::write(&path, text)
            .unwrap_or_else(|e| panic!("LIB-QA-018: write {}: {e}", path.display()));
    }

    /// Commits every change in the work tree and returns the commit's id.
    fn commit(&self, message: &str) -> String {
        self.git(&["add", "--all"]);
        self.git(&["commit", "--quiet", "-m", message]);
        self.git(&["rev-parse", "HEAD"])
    }

    /// Runs `script` with `bash` as a GitHub runner does (`bash -e`), with
    /// the event variables `event` (the others empty, and `PREVIOUS_OK` unset
    /// unless `event` gives it), and returns what it wrote to
    /// `$GITHUB_OUTPUT`.
    fn run_script(&self, bash: &Path, script: &str, event: &[(&str, &str)], n: usize) -> String {
        let script_file = self.root.join(format!("script-{n}.sh"));
        let output_file = self.root.join(format!("github-output-{n}"));
        fs::write(&script_file, script)
            .unwrap_or_else(|e| panic!("LIB-QA-018: write {}: {e}", script_file.display()));
        let mut command = self.command(bash);
        command
            .args(["--noprofile", "--norc", "-e"])
            .arg(posix(&script_file))
            .env("GITHUB_OUTPUT", posix(&output_file));
        for var in ["EVENT", "ACTION", "BEFORE", "AFTER", "BASE", "HEAD"] {
            let value = event
                .iter()
                .find(|(name, _)| *name == var)
                .map_or("", |(_, value)| *value);
            command.env(var, value);
        }
        command.env_remove("PREVIOUS_OK");
        for (name, value) in event {
            if *name == "PREVIOUS_OK" {
                command.env(name, value);
            }
        }
        let output = command
            .output()
            .unwrap_or_else(|e| panic!("LIB-QA-018: run {}: {e}", bash.display()));
        assert!(
            output.status.success(),
            "LIB-QA-018: the script failed for {event:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        fs::read_to_string(&output_file).unwrap_or_default()
    }
}

impl Drop for ScratchRepo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// Runs the detection script for `event` and returns `code`: the script
/// must write exactly one line, `code=true` or `code=false`.
fn detect(bash: &Path, repo: &ScratchRepo, script: &str, event: &[(&str, &str)], n: usize) -> bool {
    let written = repo.run_script(bash, script, event, n);
    match written.lines().collect::<Vec<_>>().as_slice() {
        ["code=true"] => true,
        ["code=false"] => false,
        _ => panic!("LIB-QA-018: for {event:?} the script wrote {written:?}, not one `code=` line"),
    }
}

/// The commits of the scratch history, in order.
struct History {
    /// README.md, src/lib.rs and docs/guide.md.
    base: String,
    /// docs/guide.md changed and docs/sub/new.md added.
    docs: String,
    /// src/lib.rs changed.
    code: String,
    /// docs.md added at the top: not under docs/.
    docs_file: String,
    /// crates/x/docs/notes.md added: a docs/ that is not the top one.
    nested_docs: String,
    /// docs/guide.md deleted.
    docs_deleted: String,
    /// src/lib.rs moved to docs/lib.rs.
    moved_into_docs: String,
}

fn history(repo: &ScratchRepo) -> History {
    repo.write("README.md", "readme\n");
    repo.write("src/lib.rs", "pub fn answer() -> u32 {\n    42\n}\n");
    repo.write("docs/guide.md", "# Guide\n");
    let base = repo.commit("base");
    repo.write("docs/guide.md", "# Guide\n\nMore.\n");
    repo.write("docs/sub/new.md", "# New\n");
    let docs = repo.commit("docs only");
    repo.write("src/lib.rs", "pub fn answer() -> u32 {\n    43\n}\n");
    let code = repo.commit("code");
    repo.write("docs.md", "not under docs/\n");
    let docs_file = repo.commit("a top-level docs.md");
    repo.write("crates/x/docs/notes.md", "nested\n");
    let nested_docs = repo.commit("a nested docs/");
    repo.git(&["rm", "--quiet", "docs/guide.md"]);
    let docs_deleted = repo.commit("a docs file deleted");
    repo.git(&["mv", "src/lib.rs", "docs/lib.rs"]);
    let moved_into_docs = repo.commit("code moved into docs/");
    History {
        base,
        docs,
        code,
        docs_file,
        nested_docs,
        docs_deleted,
        moved_into_docs,
    }
}

/// The event variables a detection script runs with, by name.
type Event = Vec<(&'static str, String)>;

/// A push event from `before` to `after`.
fn push_event(before: &str, after: &str) -> Event {
    vec![
        ("EVENT", "push".to_owned()),
        ("BEFORE", before.to_owned()),
        ("AFTER", after.to_owned()),
    ]
}

/// A pull request event: `action`, the pull request's `base` and `head`,
/// and the latest push's `before` and `after`.
fn pull_request_event(
    action: &str,
    (base, head): (&str, &str),
    (before, after): (&str, &str),
) -> Event {
    vec![
        ("EVENT", "pull_request".to_owned()),
        ("ACTION", action.to_owned()),
        ("BASE", base.to_owned()),
        ("HEAD", head.to_owned()),
        ("BEFORE", before.to_owned()),
        ("AFTER", after.to_owned()),
    ]
}

/// `event` with `PREVIOUS_OK` set to `ok`, the `previous` step's answer, or
/// unset when `ok` is `None`.
fn with_previous(mut event: Event, ok: Option<&str>) -> Event {
    if let Some(ok) = ok {
        event.push(("PREVIOUS_OK", ok.to_owned()));
    }
    event
}

/// `event` with CI known to have succeeded on the previous commit.
fn green(event: Event) -> Event {
    with_previous(event, Some("true"))
}

/// The detection script's cases on the scratch history `h`: what, the
/// event, and the `code` it must give.
fn detection_cases(h: &History) -> Vec<(&'static str, Event, bool)> {
    let unknown = "1234567890abcdef1234567890abcdef12345678";
    let zero = "0000000000000000000000000000000000000000";
    vec![
        // Only docs/ changed: the one case that skips. Each latest push here
        // follows a commit CI succeeded on (`PREVIOUS_OK=true`), so only its
        // paths decide.
        (
            "a push changing and adding docs/ files",
            green(push_event(&h.base, &h.docs)),
            false,
        ),
        (
            "a push deleting a docs/ file",
            green(push_event(&h.nested_docs, &h.docs_deleted)),
            false,
        ),
        (
            "a new pull request changing only docs/",
            pull_request_event("opened", (&h.base, &h.docs), ("", "")),
            false,
        ),
        (
            "a reopened pull request changing only docs/",
            pull_request_event("reopened", (&h.base, &h.docs), ("", "")),
            false,
        ),
        (
            "a pull request's latest push changing only docs/, after code",
            green(pull_request_event(
                "synchronize",
                (&h.docs, &h.code),
                (&h.base, &h.docs),
            )),
            false,
        ),
        // Anything else runs everything.
        (
            "a push changing src/",
            green(push_event(&h.docs, &h.code)),
            true,
        ),
        (
            "a push changing docs/ and src/",
            green(push_event(&h.base, &h.code)),
            true,
        ),
        (
            "a push adding a top-level docs.md",
            green(push_event(&h.code, &h.docs_file)),
            true,
        ),
        (
            "a push adding crates/x/docs/notes.md",
            green(push_event(&h.docs_file, &h.nested_docs)),
            true,
        ),
        (
            "a new pull request changing src/",
            pull_request_event("opened", (&h.docs, &h.code), ("", "")),
            true,
        ),
        (
            "a pull request's latest push changing src/, after docs",
            green(pull_request_event(
                "synchronize",
                (&h.base, &h.docs),
                (&h.docs, &h.code),
            )),
            true,
        ),
        ("an empty range", green(push_event(&h.docs, &h.docs)), true),
        ("a push with no range", green(push_event("", "")), true),
        (
            "a push creating a branch (before is all zeros)",
            green(push_event(zero, &h.docs)),
            true,
        ),
        (
            "a range from an unknown commit",
            green(push_event(unknown, &h.docs)),
            true,
        ),
        (
            "a range to an unknown commit",
            green(push_event(&h.base, unknown)),
            true,
        ),
        (
            "a manual run",
            green(vec![
                ("EVENT", "workflow_dispatch".to_owned()),
                ("BEFORE", h.base.clone()),
                ("AFTER", h.docs.clone()),
            ]),
            true,
        ),
    ]
}

#[test]
fn lib_qa_018_detection_script_is_false_only_for_a_docs_only_range() {
    let test = "detection";
    let Some(bash) = script_runner(test) else {
        return;
    };
    let Some(script) = detection_script(&ci_text()) else {
        panic!("LIB-QA-018: ci.yml's `changes` job has no detection script");
    };
    let repo = ScratchRepo::new(test);
    let h = history(&repo);
    let cases = detection_cases(&h);
    for (n, (what, event, expected)) in cases.iter().enumerate() {
        let event: Vec<(&str, &str)> = event.iter().map(|(k, v)| (*k, v.as_str())).collect();
        assert_eq!(
            detect(&bash, &repo, &script, &event, n),
            *expected,
            "LIB-QA-018: the detection script gives the wrong `code` for {what}"
        );
    }
}

#[test]
fn lib_qa_018_detection_cases_catch_a_widened_rule_or_a_closed_default() {
    // The scratch-repository cases bite: each mutated script gives the
    // wrong `code` for one of them.
    let test = "detection-mutants";
    let Some(bash) = script_runner(test) else {
        return;
    };
    let Some(script) = detection_script(&ci_text()) else {
        panic!("LIB-QA-018: ci.yml's `changes` job has no detection script");
    };
    let repo = ScratchRepo::new(test);
    let h = history(&repo);
    let unknown = "1234567890abcdef1234567890abcdef12345678";
    for (n, (what, mutated, event)) in [
        (
            "the rule widened to '^docs' reads docs.md as documentation",
            replace_once(&script, "'^docs/'", "'^docs'"),
            green(push_event(&h.code, &h.docs_file)),
        ),
        (
            "the rule unanchored reads crates/x/docs/ as documentation",
            replace_once(&script, "'^docs/'", "'docs/'"),
            green(push_event(&h.docs_file, &h.nested_docs)),
        ),
        (
            "the default made false fails closed on an unknown commit",
            replace_once(&script, "\ncode=true\nif [ -n", "\ncode=false\nif [ -n"),
            green(push_event(unknown, &h.docs)),
        ),
        (
            "without the override, a docs-only push after red CI skips",
            without_block(&script, PREVIOUS_OVERRIDE, "fi"),
            with_previous(push_event(&h.base, &h.docs), Some("false")),
        ),
        (
            "with `= false`, a docs-only push with no answer skips",
            replace_once(&script, "\" != true ]", "\" = false ]"),
            push_event(&h.base, &h.docs),
        ),
        (
            "with `needs_previous=false` for a push, a push after red CI skips",
            replace_once(
                &script,
                PUSH_RANGE,
                &PUSH_RANGE.replace("needs_previous=true", "needs_previous=false"),
            ),
            with_previous(push_event(&h.base, &h.docs), Some("false")),
        ),
        (
            "with `needs_previous=false` for synchronize, a latest push after red CI skips",
            replace_once(
                &script,
                SYNCHRONIZE_RANGE,
                &SYNCHRONIZE_RANGE.replace("needs_previous=true", "needs_previous=false"),
            ),
            with_previous(
                pull_request_event("synchronize", (&h.docs, &h.code), (&h.base, &h.docs)),
                Some("false"),
            ),
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let event: Vec<(&str, &str)> = event.iter().map(|(k, v)| (*k, v.as_str())).collect();
        assert!(
            !detect(&bash, &repo, &mutated, &event, n),
            "LIB-QA-018: the cases do not catch it when {what}"
        );
    }
}

#[test]
fn lib_qa_018_a_docs_only_latest_push_skips_only_after_ci_succeeded_on_the_previous_commit() {
    let test = "previous-ok";
    let Some(bash) = script_runner(test) else {
        return;
    };
    let Some(script) = detection_script(&ci_text()) else {
        panic!("LIB-QA-018: ci.yml's `changes` job has no detection script");
    };
    let repo = ScratchRepo::new(test);
    let h = history(&repo);
    let answers: [(&str, Option<&str>); 5] = [
        ("true", Some("true")),
        ("false", Some("false")),
        ("empty", Some("")),
        ("unset", None),
        ("`TRUE`", Some("TRUE")),
    ];
    let mut n = 0;
    // A docs-only latest push skips only when CI succeeded before it.
    for (what, event) in [
        ("a docs-only push", push_event(&h.base, &h.docs)),
        (
            "a pull request's docs-only latest push",
            pull_request_event("synchronize", (&h.docs, &h.code), (&h.base, &h.docs)),
        ),
    ] {
        for (answer, ok) in answers {
            n += 1;
            let event = with_previous(event.clone(), ok);
            let event: Vec<(&str, &str)> = event.iter().map(|(k, v)| (*k, v.as_str())).collect();
            assert_eq!(
                detect(&bash, &repo, &script, &event, n),
                ok != Some("true"),
                "LIB-QA-018: {what} with the previous commit's answer {answer} gives the wrong \
                 `code`"
            );
        }
    }
    // A new or reopened pull request's whole range is checked, so the
    // previous commit does not matter.
    for action in ["opened", "reopened"] {
        for (answer, ok) in answers {
            n += 1;
            let event = with_previous(
                pull_request_event(action, (&h.base, &h.docs), (&h.docs, &h.code)),
                ok,
            );
            let event: Vec<(&str, &str)> = event.iter().map(|(k, v)| (*k, v.as_str())).collect();
            assert!(
                !detect(&bash, &repo, &script, &event, n),
                "LIB-QA-018: an {action} docs-only pull request with the answer {answer} runs \
                 the code jobs"
            );
        }
    }
}

/// Runs the `previous` step's script with `bash` and a stub `gh` that
/// prints `answer` (or fails when it is `None`), and returns what the
/// script wrote to `$GITHUB_OUTPUT` and the arguments `gh` was given.
fn run_previous(
    bash: &Path,
    repo: &ScratchRepo,
    script: &str,
    before: &str,
    answer: Option<&str>,
    n: usize,
) -> (String, String) {
    let bin = repo.root.join(format!("bin-{n}"));
    fs::create_dir_all(&bin)
        .unwrap_or_else(|e| panic!("LIB-QA-018: create {}: {e}", bin.display()));
    let args_file = repo.root.join(format!("gh-args-{n}"));
    let body = match answer {
        Some(answer) => format!("printf '%s\n' '{answer}'\n"),
        None => "echo 'HTTP 403' >&2\nexit 1\n".to_owned(),
    };
    let gh = bin.join("gh");
    fs::write(
        &gh,
        format!(
            "#!/bin/sh\nprintf '%s\n' \"$@\" > '{}'\n{body}",
            posix(&args_file)
        ),
    )
    .unwrap_or_else(|e| panic!("LIB-QA-018: write {}: {e}", gh.display()));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&gh, fs::Permissions::from_mode(0o755))
            .unwrap_or_else(|e| panic!("LIB-QA-018: chmod {}: {e}", gh.display()));
    }
    let script_file = repo.root.join(format!("previous-{n}.sh"));
    let output_file = repo.root.join(format!("previous-output-{n}"));
    fs::write(&script_file, script)
        .unwrap_or_else(|e| panic!("LIB-QA-018: write {}: {e}", script_file.display()));
    let path = std::env::var_os("PATH").unwrap_or_default();
    let path =
        std::env::join_paths(std::iter::once(bin.clone()).chain(std::env::split_paths(&path)))
            .unwrap_or_else(|e| panic!("LIB-QA-018: join PATH: {e}"));
    let output = repo
        .command(bash)
        .args(["--noprofile", "--norc", "-e"])
        .arg(posix(&script_file))
        .env("PATH", path)
        .env("GITHUB_OUTPUT", posix(&output_file))
        .env("GH_TOKEN", "token")
        .env("REPO", "owner/repo")
        .env("BEFORE", before)
        .env("EVENT", "pull_request")
        .output()
        .unwrap_or_else(|e| panic!("LIB-QA-018: run {}: {e}", bash.display()));
    assert!(
        output.status.success(),
        "LIB-QA-018: the `previous` script failed for {answer:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    (
        fs::read_to_string(&output_file).unwrap_or_default(),
        fs::read_to_string(&args_file).unwrap_or_default(),
    )
}

#[test]
fn lib_qa_018_the_previous_step_says_ok_only_when_the_api_finds_a_successful_run() {
    let test = "previous-step";
    let Some(bash) = script_runner(test) else {
        return;
    };
    let Some(script) = previous_script(&ci_text()) else {
        panic!("LIB-QA-018: ci.yml's `changes` job has no `previous` step script");
    };
    let repo = ScratchRepo::new(test);
    let before = "1234567890abcdef1234567890abcdef12345678";
    for (n, (what, answer, ok)) in [
        ("one successful run", Some("1"), true),
        ("three successful runs", Some("3"), true),
        ("no successful run", Some("0"), false),
        ("an empty answer", Some(""), false),
        ("an answer that is no number", Some("null"), false),
        ("a failing lookup", None, false),
    ]
    .into_iter()
    .enumerate()
    {
        let (written, args) = run_previous(&bash, &repo, &script, before, answer, n);
        assert_eq!(
            written,
            format!("ok={ok}\n"),
            "LIB-QA-018: the `previous` step's answer for {what}"
        );
        let args: Vec<&str> = args.lines().collect();
        assert_eq!(
            args.get(..2),
            Some(
                [
                    "api",
                    format!(
                        "repos/owner/repo/actions/workflows/ci.yml/runs?head_sha={before}\
                         &status=success&event=pull_request"
                    )
                    .as_str()
                ]
                .as_slice()
            ),
            "LIB-QA-018: the `previous` step does not ask for the successful ci.yml runs of \
             this event type on the previous commit ({what})"
        );
    }
    // No previous commit (a new pull request): no lookup, and no answer.
    let (written, args) = run_previous(&bash, &repo, &script, "", Some("1"), 99);
    assert_eq!(
        (written.as_str(), args.as_str()),
        ("ok=false\n", ""),
        "LIB-QA-018: with no previous commit the `previous` step looks up or says ok"
    );
}

#[test]
fn lib_qa_018_detection_script_counts_code_moved_into_docs_as_a_code_change() {
    // `git diff --name-only` shows a rename by its new path only, so a file
    // moved out of the code into docs/ must not read as a docs-only push:
    // the code lost a file.
    let test = "rename";
    let Some(bash) = script_runner(test) else {
        return;
    };
    let Some(script) = detection_script(&ci_text()) else {
        panic!("LIB-QA-018: ci.yml's `changes` job has no detection script");
    };
    let repo = ScratchRepo::new(test);
    let h = history(&repo);
    let event = [
        ("EVENT", "push"),
        ("BEFORE", h.docs_deleted.as_str()),
        ("AFTER", h.moved_into_docs.as_str()),
        ("PREVIOUS_OK", "true"),
    ];
    assert!(
        detect(&bash, &repo, &script, &event, 0),
        "LIB-QA-018: moving src/lib.rs to docs/lib.rs reads as a docs-only push, so the code \
         jobs would be skipped"
    );
}

#[test]
fn lib_qa_018_mutants_check_passes_only_when_every_shard_passed_or_docs_only_skipped_them() {
    let test = "aggregator";
    let Some(bash) = script_runner(test) else {
        return;
    };
    let command = MUTANTS_RUN
        .strip_prefix("run: ")
        .expect("LIB-QA-018: the aggregator's `run:` line");
    for (result, changes, code, passes) in [
        ("success", "success", "true", true),
        ("success", "success", "false", true),
        ("success", "failure", "", true),
        ("skipped", "success", "false", true),
        ("skipped", "success", "true", false),
        ("skipped", "failure", "", false),
        ("skipped", "failure", "false", false),
        ("failure", "success", "false", false),
        ("failure", "success", "true", false),
        ("cancelled", "success", "false", false),
    ] {
        let status = Command::new(&bash)
            .args(["--noprofile", "--norc", "-e", "-c", command])
            .env("RESULT", result)
            .env("CHANGES", changes)
            .env("CODE", code)
            .status()
            .unwrap_or_else(|e| panic!("LIB-QA-018: run {}: {e}", bash.display()));
        assert_eq!(
            status.success(),
            passes,
            "LIB-QA-018: the `mutants (PR diff)` check with shards {result}, `changes` \
             {changes} and code {code:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// LIB-QA-002 and LIB-DOC-007: the MSRV
// ---------------------------------------------------------------------------

#[test]
fn lib_qa_002_msrv_is_consistent() {
    let msrv = workspace_msrv();
    let clippy_msrv = toml_table("clippy.toml")["msrv"]
        .as_str()
        .expect("clippy.toml msrv is a string")
        .to_owned();
    assert_eq!(
        clippy_msrv, msrv,
        "LIB-QA-002: clippy.toml msrv must equal rust-version"
    );
    let command = format!("cargo +{msrv} test -p sacm -p sacm-cli");
    assert!(
        read(".github/workflows/ci.yml").contains(&command),
        "LIB-QA-002: CI must test the published crates with the {msrv} toolchain (`{command}`)"
    );
    let adr = read("docs/adr/0007-msrv-and-edition.md");
    assert!(
        adr.contains(&msrv),
        "LIB-QA-002: ADR 0007 must state the MSRV {msrv}"
    );
}

/// LIB-DOC-007: README.md's MSRV badge and CONTRIBUTING.md's "Minimum
/// supported Rust version" section state the workspace `rust-version`.
#[test]
fn lib_doc_007_documented_msrv_matches_the_manifest() {
    let msrv = workspace_msrv();
    let readme = read("README.md");
    let badge = readme
        .split("MSRV-")
        .nth(1)
        .and_then(|rest| rest.split('-').next());
    assert_eq!(
        badge,
        Some(msrv.as_str()),
        "LIB-DOC-007: README.md's MSRV badge must state {msrv}"
    );
    let contributing = read("CONTRIBUTING.md");
    let stated = contributing
        .split("## Minimum supported Rust version")
        .nth(1)
        .and_then(|section| {
            section
                .split(|c: char| !c.is_ascii_digit() && c != '.')
                .find(|word| word.contains('.') && word.starts_with(|c: char| c.is_ascii_digit()))
        });
    assert_eq!(
        stated,
        Some(msrv.as_str()),
        "LIB-DOC-007: CONTRIBUTING.md's \"Minimum supported Rust version\" section must state {msrv} first"
    );
}

/// LIB-QA-002 (Q2-2): a member manifest inherits `rust-version` from the
/// workspace and declares none of its own.
fn check_inherits_rust_version(manifest: &str) -> Vec<String> {
    let table: toml::Table = match manifest.parse() {
        Ok(table) => table,
        Err(e) => return vec![format!("manifest does not parse: {e}")],
    };
    let inherited: toml::Table = "workspace = true".parse().expect("literal TOML");
    match table.get("package").and_then(|p| p.get("rust-version")) {
        Some(toml::Value::Table(t)) if *t == inherited => Vec::new(),
        Some(other) => vec![format!(
            "package.rust-version must be `{{ workspace = true }}`, found `{other}`"
        )],
        None => vec!["package.rust-version is missing; it must inherit the workspace MSRV".into()],
    }
}

/// The workspace members that are published (no `publish = false`), as
/// (path, manifest text).
fn published_members() -> Vec<(String, String)> {
    let workspace = toml_table("Cargo.toml");
    let members = workspace["workspace"]["members"]
        .as_array()
        .expect("workspace.members is an array");
    members
        .iter()
        .map(|m| m.as_str().expect("member path is a string"))
        .inspect(|m| {
            assert!(
                !m.contains('*'),
                "LIB-QA-002: member globs are not supported by this test: {m}"
            );
        })
        .map(|m| (m.to_owned(), read(&format!("{m}/Cargo.toml"))))
        .filter(|(_, manifest)| {
            let table: toml::Table = manifest.parse().expect("member manifest parses");
            lookup(&table, &["package", "publish"]).and_then(toml::Value::as_bool) != Some(false)
        })
        .collect()
}

#[test]
fn lib_qa_002_published_crates_inherit_rust_version() {
    let members = published_members();
    for required in ["crates/sacm", "crates/sacm-cli"] {
        assert!(
            members.iter().any(|(path, _)| path == required),
            "LIB-QA-002: {required} must be a published workspace member"
        );
    }
    for (path, manifest) in &members {
        let problems = check_inherits_rust_version(manifest);
        assert!(
            problems.is_empty(),
            "LIB-QA-002: {path} must inherit rust-version from the workspace: {problems:#?}"
        );
    }
}

/// The `rust-version` line of a member manifest, trimmed.
fn rust_version_line(manifest: &str) -> String {
    manifest
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with("rust-version"))
        .expect("the manifest has a rust-version line")
        .to_owned()
}

#[test]
fn lib_qa_002_rust_version_check_rejects_member_declarations() {
    let msrv = workspace_msrv();
    for path in ["crates/sacm", "crates/sacm-cli"] {
        let manifest = read(&format!("{path}/Cargo.toml"));
        assert!(
            check_inherits_rust_version(&manifest).is_empty(),
            "LIB-QA-002: {path} must pass before mutations are meaningful"
        );
        let target = rust_version_line(&manifest);
        let mutations = [
            (
                "literal rust-version equal to the workspace's",
                replace_line(&manifest, &target, &format!("rust-version = \"{msrv}\"")),
            ),
            (
                "literal older rust-version",
                replace_line(&manifest, &target, "rust-version = \"1.80\""),
            ),
            ("rust-version removed", delete_line(&manifest, &target)),
            (
                "workspace = false",
                replace_line(&manifest, &target, "rust-version.workspace = false"),
            ),
        ];
        for (name, mutated) in &mutations {
            assert!(
                !check_inherits_rust_version(mutated).is_empty(),
                "LIB-QA-002 ({path}): the check must report the mutation `{name}`; mutated text:\n{mutated}"
            );
        }
    }
}

#[test]
fn lib_qa_002_rust_version_check_accepts_inline_inheritance() {
    for path in ["crates/sacm", "crates/sacm-cli"] {
        let manifest = read(&format!("{path}/Cargo.toml"));
        let target = rust_version_line(&manifest);
        let inline = replace_line(&manifest, &target, "rust-version = { workspace = true }");
        let problems = check_inherits_rust_version(&inline);
        assert!(
            problems.is_empty(),
            "LIB-QA-002 ({path}): an inline `rust-version = {{ workspace = true }}` inherits the MSRV, but was reported: {problems:#?}"
        );
    }
}

// ---------------------------------------------------------------------------
// LIB-QA-003: the dependency policy in `deny.toml`
// ---------------------------------------------------------------------------

/// The permissive licences an MIT library may depend on. The lead-approved
/// set: the eight in `deny.toml` today plus six more.
const PERMISSIVE: [&str; 14] = [
    "MIT",
    "Apache-2.0",
    "Apache-2.0 WITH LLVM-exception",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "ISC",
    "Unicode-3.0",
    "Zlib",
    "0BSD",
    "MIT-0",
    "BSL-1.0",
    "CC0-1.0",
    "Unlicense",
    "Unicode-DFS-2016",
];

const CRATES_IO_INDEX: &str = "https://github.com/rust-lang/crates.io-index";

/// R3-3: the legacy top-level graph keys, none of which may be present.
const LEGACY_GRAPH_KEYS: [&str; 6] = [
    "targets",
    "exclude",
    "features",
    "all-features",
    "no-default-features",
    "exclude-dev",
];

/// R3-7: for each reviewed `deny.toml` section (dotted path; `""` is the top
/// level), the keys cargo-deny 0.20.2 accepts there, as listed by its own
/// `unexpected-keys` error. Any other key is reported, so a key a future
/// cargo-deny adds is reviewed before it can narrow the checks. `[bans]` and
/// `[output]` are not reviewed.
const DENY_SECTION_KEYS: [(&str, &[&str]); 7] = [
    (
        "",
        &[
            "advisories",
            "bans",
            "licenses",
            "sources",
            "graph",
            "targets",
            "exclude",
            "features",
            "all-features",
            "no-default-features",
            "exclude-dev",
            "output",
            "feature-depth",
        ],
    ),
    (
        "graph",
        &[
            "targets",
            "exclude",
            "features",
            "all-features",
            "no-default-features",
            "exclude-dev",
            "exclude-unpublished",
        ],
    ),
    (
        "advisories",
        &[
            "version",
            "db-path",
            "db-urls",
            "vulnerability",
            "notice",
            "unmaintained",
            "unsound",
            "yanked",
            "ignore",
            "severity-threshold",
            "git-fetch-with-cli",
            "disable-yank-checking",
            "maximum-db-staleness",
            "unused-ignored-advisory",
        ],
    ),
    (
        "licenses",
        &[
            "version",
            "private",
            "unlicensed",
            "allow-osi-fsf-free",
            "copyleft",
            "default",
            "confidence-threshold",
            "deny",
            "allow",
            "unused-allowed-license",
            "clarify",
            "exceptions",
            "unused-license-exception",
            "include-dev",
            "include-build",
        ],
    ),
    (
        "licenses.private",
        &["ignore", "ignore-sources", "registries"],
    ),
    (
        "sources",
        &[
            "unknown-registry",
            "unknown-git",
            "allow-registry",
            "allow-git",
            "allow-org",
            "private",
            "required-git-spec",
            "unused-allowed-source",
            "unused-allowed-org",
        ],
    ),
    ("sources.allow-org", &["github", "gitlab", "bitbucket"]),
];

fn is_absent_or_empty(value: Option<&toml::Value>) -> bool {
    value.is_none_or(|v| v.as_array().is_some_and(Vec::is_empty))
}

fn is_absent_or_false(value: Option<&toml::Value>) -> bool {
    value.is_none_or(|v| v.as_bool() == Some(false))
}

/// Licences in `value` (an array of strings) outside [`PERMISSIVE`].
fn non_permissive(value: Option<&toml::Value>, what: &str) -> Vec<String> {
    let Some(array) = value.and_then(toml::Value::as_array) else {
        return vec![problem(
            "licenses",
            format!("{what} must be an array of licence identifiers"),
        )];
    };
    array
        .iter()
        .filter(|l| l.as_str().is_none_or(|l| !PERMISSIVE.contains(&l)))
        .map(|l| {
            problem(
                "licenses",
                format!("{what} contains `{l}`, which is not in the permissive set"),
            )
        })
        .collect()
}

/// R3-7: every key of a reviewed section outside [`DENY_SECTION_KEYS`], as
/// `[unreviewed-key]`; a reviewed section that is not a table is `[shape]`.
fn unreviewed_key_problems(deny: &toml::Table) -> Vec<String> {
    let mut problems = Vec::new();
    for (section, keys) in DENY_SECTION_KEYS {
        let path: Vec<&str> = section.split('.').filter(|s| !s.is_empty()).collect();
        let table = if path.is_empty() {
            deny
        } else {
            match lookup(deny, &path) {
                None => continue,
                Some(toml::Value::Table(table)) => table,
                Some(other) => {
                    problems.push(problem(
                        "shape",
                        format!("deny.toml `[{section}]` must be a table, found `{other}`"),
                    ));
                    continue;
                }
            }
        };
        for key in table.keys() {
            if !keys.contains(&key.as_str()) {
                let at = if section.is_empty() {
                    "the top level".to_owned()
                } else {
                    format!("`[{section}]`")
                };
                problems.push(problem(
                    "unreviewed-key",
                    format!("deny.toml has the key `{key}` at {at}, which has not been reviewed"),
                ));
            }
        }
    }
    problems
}

/// Q3-1, Q3-2 and R3-6: the licence policy, applied to every crate.
fn licence_problems(deny: &toml::Table) -> Vec<String> {
    let mut problems = Vec::new();
    // Q3-1: licences.
    let allow = lookup(deny, &["licenses", "allow"]);
    require(
        &mut problems,
        allow
            .and_then(toml::Value::as_array)
            .is_some_and(|a| a.iter().any(|l| l.as_str() == Some("MIT"))),
        "licenses",
        "licenses.allow must allow MIT",
    );
    // Q3-2: exceptions.
    let exceptions = lookup(deny, &["licenses", "exceptions"]);
    let exception_problems: Vec<String> = match exceptions {
        None => Vec::new(),
        Some(toml::Value::Array(entries)) => entries
            .iter()
            .flat_map(|entry| non_permissive(entry.get("allow"), "a licenses.exceptions entry"))
            .collect(),
        Some(other) => vec![problem(
            "licenses",
            format!("licenses.exceptions must be an array, found `{other}`"),
        )],
    };
    // R3-6: the licence check covers every crate.
    require(
        &mut problems,
        is_absent_or_empty(lookup(deny, &["licenses", "private", "ignore-sources"])),
        "licenses",
        "licenses.private.ignore-sources must be absent or empty",
    );
    require(
        &mut problems,
        is_absent_or_empty(lookup(deny, &["licenses", "private", "registries"])),
        "licenses",
        "licenses.private.registries must be absent or empty",
    );
    // Round 3 F2: cargo-deny's default for `include-dev` is false, so an
    // absent key leaves dev-dependencies unchecked; it must be exactly `true`
    // (maintainer decision, option (a)). `include-build` defaults to true.
    require(
        &mut problems,
        matches!(
            lookup(deny, &["licenses", "include-dev"]),
            Some(toml::Value::Boolean(true))
        ),
        "licenses",
        "licenses.include-dev must be exactly true (cargo-deny's default is false)",
    );
    require(
        &mut problems,
        !matches!(
            lookup(deny, &["licenses", "include-build"]),
            Some(toml::Value::Boolean(false))
        ),
        "licenses",
        "licenses.include-build must not be false",
    );
    problems.extend(non_permissive(allow, "licenses.allow"));
    problems.extend(exception_problems);
    problems
}

/// Q3-2 and R3-3: no crate is excluded from the graph.
fn graph_problems(deny: &toml::Table) -> Vec<String> {
    let mut problems = Vec::new();
    require(
        &mut problems,
        is_absent_or_empty(lookup(deny, &["graph", "exclude"])),
        "graph",
        "graph.exclude must be absent or empty: no crate may be excluded from the checks",
    );
    require(
        &mut problems,
        is_absent_or_empty(lookup(deny, &["graph", "targets"])),
        "graph",
        "graph.targets must be absent or empty: no target may narrow the graph",
    );
    require(
        &mut problems,
        is_absent_or_false(lookup(deny, &["graph", "exclude-dev"])),
        "graph",
        "graph.exclude-dev must be absent or false",
    );
    require(
        &mut problems,
        is_absent_or_false(lookup(deny, &["graph", "exclude-unpublished"])),
        "graph",
        "graph.exclude-unpublished must be absent or false",
    );
    require(
        &mut problems,
        lookup(deny, &["graph", "all-features"]).and_then(toml::Value::as_bool) == Some(true),
        "graph",
        "graph.all-features must be true",
    );
    for key in LEGACY_GRAPH_KEYS {
        require(
            &mut problems,
            !deny.contains_key(key),
            "graph",
            &format!("the legacy top-level key `{key}` must be absent; it still narrows the graph"),
        );
    }
    problems
}

/// Q3-3 and R3-4: sources.
fn source_problems(deny: &toml::Table) -> Vec<String> {
    let mut problems = Vec::new();
    require(
        &mut problems,
        lookup(deny, &["sources", "unknown-git"]).and_then(toml::Value::as_str) == Some("deny"),
        "sources",
        "sources.unknown-git must be \"deny\"",
    );
    require(
        &mut problems,
        lookup(deny, &["sources", "unknown-registry"]).and_then(toml::Value::as_str)
            == Some("deny"),
        "sources",
        "sources.unknown-registry must be \"deny\"",
    );
    require(
        &mut problems,
        is_absent_or_empty(lookup(deny, &["sources", "allow-git"])),
        "sources",
        "sources.allow-git must be absent or empty",
    );
    require(
        &mut problems,
        lookup(deny, &["sources", "allow-org"]).is_none_or(|org| {
            org.as_table()
                .is_some_and(|t| t.values().all(|v| v.as_array().is_some_and(Vec::is_empty)))
        }),
        "sources",
        "sources.allow-org must be absent or have only empty lists",
    );
    require(
        &mut problems,
        lookup(deny, &["sources", "allow-registry"]).is_none_or(|r| {
            r.as_array()
                .is_some_and(|r| r.len() == 1 && r[0].as_str() == Some(CRATES_IO_INDEX))
        }),
        "sources",
        "sources.allow-registry must be absent or exactly the crates.io index",
    );
    require(
        &mut problems,
        is_absent_or_empty(lookup(deny, &["sources", "private"])),
        "sources",
        "sources.private must be absent or empty",
    );
    problems
}

/// Q3-4 and R3-5: advisories.
fn advisory_problems(deny: &toml::Table) -> Vec<String> {
    let mut problems = Vec::new();
    require(
        &mut problems,
        lookup(deny, &["advisories", "ignore"]).is_none_or(|ignore| {
            ignore.as_array().is_some_and(|entries| {
                entries.iter().all(|entry| {
                    entry
                        .get("reason")
                        .and_then(toml::Value::as_str)
                        .is_some_and(|reason| !reason.trim().is_empty())
                })
            })
        }),
        "advisories",
        "every advisories.ignore entry must be a table with a non-empty `reason`",
    );
    require(
        &mut problems,
        lookup(deny, &["advisories", "db-urls"]).is_none(),
        "advisories",
        "advisories.db-urls must be absent: no alternative advisory database",
    );
    require(
        &mut problems,
        lookup(deny, &["advisories", "yanked"]).and_then(toml::Value::as_str) == Some("deny"),
        "advisories",
        "advisories.yanked must be \"deny\"",
    );
    require(
        &mut problems,
        is_absent_or_false(lookup(deny, &["advisories", "disable-yank-checking"])),
        "advisories",
        "advisories.disable-yank-checking must be absent or false",
    );
    problems
}

/// LIB-QA-003 (Q3-1 to Q3-4, R3-3 to R3-7, and the original criteria): the
/// dependency policy in a parsed `deny.toml`.
fn check_deny_policy(deny: &toml::Table) -> Vec<String> {
    let mut problems = licence_problems(deny);
    problems.extend(graph_problems(deny));
    problems.extend(source_problems(deny));
    problems.extend(advisory_problems(deny));
    problems.extend(unreviewed_key_problems(deny));
    problems
}

#[test]
fn lib_qa_003_dependency_policy_is_enforced() {
    let deny = toml_table("deny.toml");
    let problems = check_deny_policy(&deny);
    assert!(
        problems.is_empty(),
        "LIB-QA-003: deny.toml must enforce the dependency policy: {problems:#?}"
    );
    let allowed: Vec<&str> = deny["licenses"]["allow"]
        .as_array()
        .expect("licenses.allow is an array")
        .iter()
        .filter_map(toml::Value::as_str)
        .collect();
    assert!(allowed.contains(&"MIT"), "LIB-QA-003: MIT must be allowed");
    for copyleft in ["GPL-2.0", "GPL-3.0", "AGPL-3.0", "LGPL-2.1", "LGPL-3.0"] {
        assert!(
            !allowed.iter().any(|l| l.starts_with(copyleft)),
            "LIB-QA-003: {copyleft} must not be allowed for an MIT library"
        );
    }
    assert_eq!(
        deny["sources"]["unknown-git"].as_str(),
        Some("deny"),
        "LIB-QA-003: git dependencies must be denied"
    );
    assert_eq!(
        deny["sources"]["unknown-registry"].as_str(),
        Some("deny"),
        "LIB-QA-003: registries other than crates.io must be denied"
    );
    assert_eq!(
        deny["advisories"]["yanked"].as_str(),
        Some("deny"),
        "LIB-QA-003: yanked crates must be denied"
    );
    assert_eq!(
        deny["licenses"]
            .get("include-dev")
            .and_then(toml::Value::as_bool),
        Some(true),
        "LIB-QA-003: dev-dependencies must be licence-checked (`licenses.include-dev = true`)"
    );
}

/// The table at the dotted `section` of `deny` (`""` is the top level),
/// created if absent.
fn section_mut<'a>(deny: &'a mut toml::Table, section: &str) -> &'a mut toml::Table {
    let mut table = deny;
    for name in section.split('.').filter(|s| !s.is_empty()) {
        table = table
            .entry(name)
            .or_insert_with(|| toml::Value::Table(toml::Table::new()))
            .as_table_mut()
            .expect("a deny.toml section is a table");
    }
    table
}

/// The real `deny.toml`, with `value` set at `section.key` (`section` dotted;
/// `""` is the top level).
fn deny_with(section: &str, key: &str, value: &str) -> toml::Table {
    let mut deny = toml_table("deny.toml");
    section_mut(&mut deny, section).insert(key.to_owned(), toml_value(value));
    deny
}

/// The real `deny.toml`, without `section.key`.
fn deny_without(section: &str, key: &str) -> toml::Table {
    let mut deny = toml_table("deny.toml");
    assert!(
        section_mut(&mut deny, section).remove(key).is_some(),
        "mutation target `{section}.{key}` must exist"
    );
    deny
}

/// The real `deny.toml`, with `licence` appended to `licenses.allow`.
fn deny_allowing(licence: &str) -> toml::Table {
    let mut deny = toml_table("deny.toml");
    deny["licenses"]
        .get_mut("allow")
        .and_then(toml::Value::as_array_mut)
        .expect("licenses.allow is an array")
        .push(licence.into());
    deny
}

fn assert_policy_reports(mutations: &[(String, toml::Table)]) {
    for (name, deny) in mutations {
        assert!(
            !check_deny_policy(deny).is_empty(),
            "LIB-QA-003: the policy check must report the mutation `{name}`"
        );
    }
}

/// Assert that the policy check reports each mutation `(name, section, key,
/// value)` of the real `deny.toml` with `code`.
fn assert_policy_reports_code(code: &str, mutations: &[(&str, &str, &str, &str)]) {
    assert!(
        check_deny_policy(&toml_table("deny.toml")).is_empty(),
        "LIB-QA-003: the real deny.toml must pass before mutations are meaningful"
    );
    for (name, section, key, value) in mutations {
        let problems = check_deny_policy(&deny_with(section, key, value));
        assert!(
            has_code(&problems, code),
            "LIB-QA-003: the policy check must report the mutation `{name}` as `[{code}]`; it reported {problems:#?}"
        );
    }
}

#[test]
fn lib_qa_003_licence_check_rejects_non_permissive_licences() {
    assert!(
        check_deny_policy(&toml_table("deny.toml")).is_empty(),
        "LIB-QA-003: the real deny.toml must pass before mutations are meaningful"
    );
    let mut mutations: Vec<(String, toml::Table)> = [
        "MPL-2.0",
        "LGPL-2.0-or-later",
        "GPL-1.0-or-later",
        "EUPL-1.2",
        "CC-BY-SA-4.0",
        "SSPL-1.0",
        "BUSL-1.1",
        "GPL-3.0-only",
        "AGPL-3.0-only",
    ]
    .iter()
    .map(|l| (format!("licenses.allow + {l}"), deny_allowing(l)))
    .collect();
    let mut without_mit = toml_table("deny.toml");
    without_mit["licenses"]
        .get_mut("allow")
        .and_then(toml::Value::as_array_mut)
        .expect("licenses.allow is an array")
        .retain(|l| l.as_str() != Some("MIT"));
    mutations.push(("MIT removed".into(), without_mit));
    for (name, value) in [
        (
            "exception allowing MPL-2.0",
            r#"[{ crate = "some-crate", allow = ["MPL-2.0"] }]"#,
        ),
        (
            "exception allowing GPL-3.0-only",
            r#"[{ name = "some-crate", allow = ["GPL-3.0-only"] }]"#,
        ),
        (
            "exception mixing permitted and copyleft",
            r#"[{ crate = "some-crate", allow = ["MIT", "LGPL-2.0-or-later"] }]"#,
        ),
        ("exception without allow", r#"[{ crate = "some-crate" }]"#),
    ] {
        mutations.push((name.into(), deny_with("licenses", "exceptions", value)));
    }
    mutations.push((
        "graph.exclude".into(),
        deny_with("graph", "exclude", r#"["some-crate"]"#),
    ));
    assert_policy_reports(&mutations);
}

#[test]
fn lib_qa_003_policy_check_rejects_known_bypasses() {
    let mutations: Vec<(String, toml::Table)> = [
        // Q3-3: sources.
        (
            "allow-git",
            "sources",
            "allow-git",
            r#"["https://github.com/someone/some-crate"]"#,
        ),
        (
            "extra registry",
            "sources",
            "allow-registry",
            r#"["https://github.com/rust-lang/crates.io-index", "https://example.com/index"]"#,
        ),
        (
            "registry replaced",
            "sources",
            "allow-registry",
            r#"["https://example.com/index"]"#,
        ),
        (
            "allow-org",
            "sources",
            "allow-org",
            r#"{ github = ["someone"] }"#,
        ),
        (
            "allow-org, one non-empty list",
            "sources",
            "allow-org",
            r#"{ github = [], gitlab = ["someone"] }"#,
        ),
        (
            "unknown-git = allow",
            "sources",
            "unknown-git",
            r#""allow""#,
        ),
        (
            "unknown-registry = warn",
            "sources",
            "unknown-registry",
            r#""warn""#,
        ),
        // Q3-4: advisories.
        (
            "advisories.ignore, bare id",
            "advisories",
            "ignore",
            r#"["RUSTSEC-2024-0001"]"#,
        ),
        (
            "advisories.ignore, no reason",
            "advisories",
            "ignore",
            r#"[{ id = "RUSTSEC-2024-0001" }]"#,
        ),
        (
            "advisories.ignore, blank reason",
            "advisories",
            "ignore",
            r#"[{ id = "RUSTSEC-2024-0001", reason = "  " }]"#,
        ),
        (
            "advisories.ignore, one of two unjustified",
            "advisories",
            "ignore",
            r#"[{ id = "RUSTSEC-2024-0001", reason = "not reachable" }, "RUSTSEC-2024-0002"]"#,
        ),
        (
            "advisories.db-urls",
            "advisories",
            "db-urls",
            r#"["https://example.com/advisory-db"]"#,
        ),
        ("yanked = warn", "advisories", "yanked", r#""warn""#),
        // Q3-2: graph.
        (
            "graph.exclude",
            "graph",
            "exclude",
            r#"["some-crate@1.0.0"]"#,
        ),
    ]
    .iter()
    .map(|(name, section, key, value)| ((*name).to_owned(), deny_with(section, key, value)))
    .collect();
    assert_policy_reports(&mutations);
}

#[test]
fn lib_qa_003_policy_check_rejects_graph_exclusions() {
    assert_policy_reports_code(
        "graph",
        &[
            (
                "graph.exclude = [\"clap\"]",
                "graph",
                "exclude",
                r#"["clap"]"#,
            ),
            (
                "graph.targets",
                "graph",
                "targets",
                r#"["x86_64-unknown-linux-gnu"]"#,
            ),
            (
                "graph.targets, table form",
                "graph",
                "targets",
                r#"[{ triple = "x86_64-unknown-linux-gnu" }]"#,
            ),
            ("graph.exclude-dev = true", "graph", "exclude-dev", "true"),
            (
                "graph.exclude-unpublished = true",
                "graph",
                "exclude-unpublished",
                "true",
            ),
            (
                "graph.all-features = false",
                "graph",
                "all-features",
                "false",
            ),
            (
                "top-level targets",
                "",
                "targets",
                r#"["x86_64-unknown-linux-gnu"]"#,
            ),
            (
                "top-level exclude = [\"clap\"]",
                "",
                "exclude",
                r#"["clap"]"#,
            ),
            ("top-level features", "", "features", r#"["some-feature"]"#),
            ("top-level all-features", "", "all-features", "true"),
            (
                "top-level no-default-features",
                "",
                "no-default-features",
                "true",
            ),
            ("top-level exclude-dev", "", "exclude-dev", "true"),
        ],
    );
    let mut no_graph = toml_table("deny.toml");
    no_graph.remove("graph");
    for (name, deny) in [
        (
            "graph.all-features removed",
            deny_without("graph", "all-features"),
        ),
        ("[graph] removed", no_graph),
    ] {
        let problems = check_deny_policy(&deny);
        assert!(
            has_code(&problems, "graph"),
            "LIB-QA-003: the policy check must report the mutation `{name}` as `[graph]`; it reported {problems:#?}"
        );
    }
}

#[test]
fn lib_qa_003_policy_check_rejects_private_sources() {
    assert_policy_reports_code(
        "sources",
        &[(
            "sources.private",
            "sources",
            "private",
            r#"["https://example.com/registry"]"#,
        )],
    );
}

#[test]
fn lib_qa_003_policy_check_rejects_disabled_yank_checking() {
    assert_policy_reports_code(
        "advisories",
        &[(
            "advisories.disable-yank-checking = true",
            "advisories",
            "disable-yank-checking",
            "true",
        )],
    );
}

#[test]
fn lib_qa_003_policy_check_rejects_narrowed_licence_check() {
    assert_policy_reports_code(
        "licenses",
        &[
            (
                "licenses.private.ignore-sources = [crates.io]",
                "licenses.private",
                "ignore-sources",
                r#"["https://github.com/rust-lang/crates.io-index"]"#,
            ),
            (
                "licenses.private.registries",
                "licenses.private",
                "registries",
                r#"["some-registry"]"#,
            ),
            (
                "licenses.include-dev = false",
                "licenses",
                "include-dev",
                "false",
            ),
            (
                "licenses.include-dev = \"true\", a string",
                "licenses",
                "include-dev",
                r#""true""#,
            ),
            (
                "licenses.include-build = false",
                "licenses",
                "include-build",
                "false",
            ),
        ],
    );
    // Round 3 F2: cargo-deny 0.20.2 does not licence-check dev-dependencies
    // unless `include-dev = true`, so removing the key narrows the check.
    let problems = check_deny_policy(&deny_without("licenses", "include-dev"));
    assert!(
        has_code(&problems, "licenses"),
        "LIB-QA-003: the mutation `licenses.include-dev removed` must be reported as `[licenses]`; it reported {problems:#?}"
    );
}

#[test]
fn lib_qa_003_policy_check_rejects_unreviewed_keys() {
    let mut in_every_section: Vec<(String, String)> = DENY_SECTION_KEYS
        .iter()
        .map(|(section, _)| ((*section).to_owned(), "future-key".to_owned()))
        .collect();
    // A key that exists in another section is not reviewed here.
    in_every_section.push(("sources".into(), "skip".into()));
    in_every_section.push(("graph".into(), "ignore".into()));
    for (section, key) in &in_every_section {
        let problems = check_deny_policy(&deny_with(section, key, "true"));
        assert!(
            has_code(&problems, "unreviewed-key"),
            "LIB-QA-003: the unreviewed key `{key}` in `[{section}]` must be reported as `[unreviewed-key]`; it reported {problems:#?}"
        );
    }
    for (name, section, key, value) in [
        (
            "[licenses.private] is not a table",
            "licenses",
            "private",
            "true",
        ),
        (
            "[sources.allow-org] is not a table",
            "sources",
            "allow-org",
            r#""github""#,
        ),
    ] {
        let problems = check_deny_policy(&deny_with(section, key, value));
        assert!(
            has_code(&problems, "shape"),
            "LIB-QA-003: the mutation `{name}` must be reported as `[shape]`; it reported {problems:#?}"
        );
    }
}

/// `(name, section, key, value)` settings of `deny.toml` within the approved
/// policy: the controls of round 1, the absent-or-empty and absent-or-false
/// values of R3-3 to R3-6, keys cargo-deny 0.20.2 accepts (R3-7), and the
/// unchecked `[bans]` and `[output]` sections. `licenses.include-dev` has one
/// approved value, `true` (round 3 F2); its absence is not approved.
const APPROVED_DENY_VALUES: &[(&str, &str, &str, &str)] = &[
    (
        "exception within the permissive set",
        "licenses",
        "exceptions",
        r#"[{ crate = "some-crate", allow = ["Unicode-DFS-2016"] }]"#,
    ),
    ("empty exceptions", "licenses", "exceptions", "[]"),
    ("empty graph.exclude", "graph", "exclude", "[]"),
    ("empty allow-git", "sources", "allow-git", "[]"),
    (
        "allow-org with empty lists",
        "sources",
        "allow-org",
        "{ github = [], gitlab = [], bitbucket = [] }",
    ),
    ("empty advisories.ignore", "advisories", "ignore", "[]"),
    (
        "justified advisory ignore",
        "advisories",
        "ignore",
        r#"[{ id = "RUSTSEC-2024-0001", reason = "the affected API is not used" }]"#,
    ),
    // R3-3 to R3-6: the absent-or-empty / absent-or-false values.
    ("empty graph.targets", "graph", "targets", "[]"),
    ("graph.exclude-dev = false", "graph", "exclude-dev", "false"),
    (
        "graph.exclude-unpublished = false",
        "graph",
        "exclude-unpublished",
        "false",
    ),
    ("empty sources.private", "sources", "private", "[]"),
    (
        "advisories.disable-yank-checking = false",
        "advisories",
        "disable-yank-checking",
        "false",
    ),
    (
        "empty licenses.private.ignore-sources",
        "licenses.private",
        "ignore-sources",
        "[]",
    ),
    (
        "empty licenses.private.registries",
        "licenses.private",
        "registries",
        "[]",
    ),
    // Round 3 F2: the only approved value, and the value in the real file.
    (
        "licenses.include-dev = true",
        "licenses",
        "include-dev",
        "true",
    ),
    // `include-build` may be true or absent (the real file), never false.
    (
        "licenses.include-build = true",
        "licenses",
        "include-build",
        "true",
    ),
    // R3-7: reviewed keys cargo-deny 0.20.2 accepts, and the unchecked
    // sections.
    (
        "advisories.unmaintained = \"workspace\"",
        "advisories",
        "unmaintained",
        r#""workspace""#,
    ),
    (
        "advisories.unused-ignored-advisory",
        "advisories",
        "unused-ignored-advisory",
        r#""allow""#,
    ),
    (
        "licenses.unused-allowed-license",
        "licenses",
        "unused-allowed-license",
        r#""allow""#,
    ),
    (
        "sources.required-git-spec",
        "sources",
        "required-git-spec",
        r#""rev""#,
    ),
    ("an unreviewed key in [bans]", "bans", "future-key", "true"),
    (
        "an unreviewed key in [output]",
        "output",
        "future-key",
        "true",
    ),
    ("output.feature-depth", "output", "feature-depth", "1"),
];

#[test]
fn lib_qa_003_policy_check_accepts_approved_configuration() {
    let mut approved: Vec<(String, toml::Table)> = PERMISSIVE
        .iter()
        .map(|l| (format!("licenses.allow + {l}"), deny_allowing(l)))
        .collect();
    for &(name, section, key, value) in APPROVED_DENY_VALUES {
        approved.push((name.into(), deny_with(section, key, value)));
    }
    let mut no_registry = toml_table("deny.toml");
    no_registry["sources"]
        .as_table_mut()
        .expect("sources is a table")
        .remove("allow-registry");
    approved.push(("allow-registry absent".into(), no_registry));
    for (name, deny) in &approved {
        let problems = check_deny_policy(deny);
        assert!(
            problems.is_empty(),
            "LIB-QA-003: `{name}` is within the approved policy, but was reported: {problems:#?}"
        );
    }
}

// ---------------------------------------------------------------------------
// LIB-TOOL-002
// ---------------------------------------------------------------------------

#[test]
fn lib_tool_002_reference_directory_is_git_ignored() {
    assert_eq!(
        DEFAULT_DEST, "third_party/sacm-2.3",
        "LIB-TOOL-002: the artifacts are fetched into third_party/sacm-2.3"
    );
    let gitignore = read(".gitignore");
    let lines: Vec<&str> = gitignore.lines().map(str::trim).collect();
    let rule = format!("/{DEFAULT_DEST}/");
    assert!(
        lines.contains(&rule.as_str()),
        "LIB-TOOL-002: .gitignore must contain the line `{rule}`"
    );
    assert!(
        !lines
            .iter()
            .any(|l| l.starts_with('!') && l.contains("third_party")),
        "LIB-TOOL-002: .gitignore must not re-include anything under third_party/"
    );
}

// ---------------------------------------------------------------------------
// LIB-QA-007: the cargo-mutants configuration and its triage
// ---------------------------------------------------------------------------
//
// The specification is docs/verification/0003-m0-mutation-testing-prescription.md
// (P4 and P5) and the row's criteria. Each test first shows that a *reference
// fixture* passes, then that every prescribed mutation of it is reported with
// its code and every control is accepted, and last that the real file passes.

const MUTANTS_CONFIG: &str = ".cargo/mutants.toml";
const MUTANTS_TRIAGE: &str = "docs/quality/mutants-triage.md";
const MATRIX_TOML: &str = "docs/matrix/functionality-matrix.toml";

/// D12: the `.gitignore` rule for cargo-mutants' output.
const MUTANTS_GITIGNORE_RULE: &str = "/mutants.out*";

/// A workspace member: its directory, manifest text and the `.rs` files
/// under its `src/`, as (path from the repository root, text).
#[derive(Clone, Debug)]
struct MemberPackage {
    dir: String,
    manifest: String,
    sources: Vec<(String, String)>,
}

/// The workspace as the P4 checks see it.
#[derive(Clone, Debug)]
struct WorkspaceSources {
    root_manifest: String,
    members: Vec<MemberPackage>,
}

impl WorkspaceSources {
    fn source_paths(&self) -> Vec<&str> {
        self.members
            .iter()
            .flat_map(|m| m.sources.iter().map(|(path, _)| path.as_str()))
            .collect()
    }

    /// This workspace with the text of the source `path` replaced by
    /// `edit(text)`. Panics if there is no such source.
    fn with_source(&self, path: &str, edit: impl FnOnce(&str) -> String) -> Self {
        let mut out = self.clone();
        let text = out
            .members
            .iter_mut()
            .flat_map(|m| m.sources.iter_mut())
            .find(|(p, _)| p == path)
            .map_or_else(
                || panic!("mutation target `{path}` is a workspace source"),
                |(_, text)| text,
            );
        *text = edit(text);
        out
    }

    /// This workspace with the manifest of member `dir` (`""`: the root
    /// manifest) replaced by `edit(text)`.
    fn with_manifest(&self, dir: &str, edit: impl FnOnce(&str) -> String) -> Self {
        let mut out = self.clone();
        let text = if dir.is_empty() {
            &mut out.root_manifest
        } else {
            out.members.iter_mut().find(|m| m.dir == dir).map_or_else(
                || panic!("mutation target `{dir}` is a workspace member"),
                |m| &mut m.manifest,
            )
        };
        *text = edit(text);
        out
    }
}

/// Every `.rs` file under the directory `rel`, recursively, sorted.
fn rust_sources(rel: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut pending = vec![rel.to_owned()];
    while let Some(dir) = pending.pop() {
        let entries = fs::read_dir(repository_root().join(&dir))
            .unwrap_or_else(|e| panic!("read directory {dir}: {e}"));
        for entry in entries {
            let entry = entry.unwrap_or_else(|e| panic!("read directory {dir}: {e}"));
            let name = entry.file_name().to_string_lossy().into_owned();
            let child = format!("{dir}/{name}");
            let path = entry.path();
            if path.is_dir() {
                pending.push(child);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                let text = read(&child);
                out.push((child, text));
            }
        }
    }
    out.sort();
    out
}

/// The real workspace: the root manifest and every member's manifest and
/// `src/` sources.
fn real_workspace() -> WorkspaceSources {
    let members = toml_table("Cargo.toml")["workspace"]["members"]
        .as_array()
        .expect("workspace.members is an array")
        .iter()
        .map(|m| m.as_str().expect("member path is a string").to_owned())
        .map(|dir| MemberPackage {
            manifest: read(&format!("{dir}/Cargo.toml")),
            sources: rust_sources(&format!("{dir}/src")),
            dir,
        })
        .collect();
    WorkspaceSources {
        root_manifest: read("Cargo.toml"),
        members,
    }
}

/// The row ids of the real matrix.
fn matrix_ids() -> Vec<String> {
    toml_table(MATRIX_TOML)["row"]
        .as_array()
        .expect("the matrix has rows")
        .iter()
        .filter_map(|row| row.get("id").and_then(toml::Value::as_str))
        .map(str::to_owned)
        .collect()
}

fn is_identifier_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// If a raw string literal (`r"..."`, `r#"..."#`, `br"..."`) starts at `at`,
/// the index just after it.
fn raw_string_end(chars: &[char], at: usize) -> Option<usize> {
    let prefix_ok = match at.checked_sub(1).map(|i| chars[i]) {
        None => true,
        Some('b') => at < 2 || !is_identifier_char(chars[at - 2]),
        Some(c) => !is_identifier_char(c),
    };
    if chars[at] != 'r' || !prefix_ok {
        return None;
    }
    let hashes = chars[at + 1..].iter().take_while(|&&c| c == '#').count();
    let open = at + 1 + hashes;
    if chars.get(open) != Some(&'"') {
        return None;
    }
    let mut i = open + 1;
    while i < chars.len() {
        if chars[i] == '"'
            && chars[i + 1..]
                .iter()
                .take(hashes)
                .filter(|&&c| c == '#')
                .count()
                == hashes
        {
            return Some(i + 1 + hashes);
        }
        i += 1;
    }
    Some(chars.len())
}

/// Rust source with comments and whitespace removed and every string and
/// character literal emptied (P4.9), so that `#[mutants :: skip]`,
/// `mutants/*c*/::skip` and a multi-line `cfg_attr` all read `mutants::skip`,
/// and a literal or comment that merely mentions it does not.
fn code_tokens(source: &str) -> String {
    let chars: Vec<char> = source.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        if c == '/' && next == Some('/') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
        } else if c == '/' && next == Some('*') {
            let mut depth = 0_usize;
            while i < chars.len() {
                if chars[i] == '/' && chars.get(i + 1) == Some(&'*') {
                    depth += 1;
                    i += 2;
                } else if chars[i] == '*' && chars.get(i + 1) == Some(&'/') {
                    depth -= 1;
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    i += 1;
                }
            }
        } else if let Some(end) = raw_string_end(&chars, i) {
            out.push_str("\"\"");
            i = end;
        } else if c == '"' {
            i += 1;
            while i < chars.len() && chars[i] != '"' {
                i += if chars[i] == '\\' { 2 } else { 1 };
            }
            i += 1;
            out.push_str("\"\"");
        } else if c == '\'' && next == Some('\\') {
            i += 3;
            while i < chars.len() && chars[i] != '\'' {
                i += 1;
            }
            i += 1;
            out.push_str("''");
        } else if c == '\'' && chars.get(i + 2) == Some(&'\'') {
            i += 3;
            out.push_str("''");
        } else {
            if !c.is_whitespace() {
                out.push(c);
            }
            i += 1;
        }
    }
    out
}

/// Whether a manifest depends on the `mutants` crate in any dependency table
/// (including `[workspace.dependencies]`, `[target.*.dependencies]` and a
/// renamed `package = "mutants"`).
fn depends_on_mutants(table: &toml::Table) -> bool {
    table.iter().any(|(key, value)| {
        let Some(inner) = value.as_table() else {
            return false;
        };
        let direct = key.ends_with("dependencies")
            && inner.iter().any(|(name, spec)| {
                name == "mutants"
                    || spec.get("package").and_then(toml::Value::as_str) == Some("mutants")
            });
        direct || depends_on_mutants(inner)
    })
}

/// `[source-skip]` (P4.9, D13): no source has a `mutants::skip` attribute
/// (cargo-mutants honours `cfg_attr(<anything>, mutants::skip)` without
/// evaluating the condition), and no manifest depends on `mutants`.
fn source_skip_problems(workspace: &WorkspaceSources) -> Vec<String> {
    let mut problems = Vec::new();
    for member in &workspace.members {
        for (path, text) in &member.sources {
            if code_tokens(text).contains("mutants::skip") {
                problems.push(problem(
                    "source-skip",
                    format!("{path} has a `mutants::skip` attribute; exclusions belong in {MUTANTS_CONFIG}"),
                ));
            }
        }
    }
    let manifests = std::iter::once(("Cargo.toml".to_owned(), &workspace.root_manifest)).chain(
        workspace
            .members
            .iter()
            .map(|m| (format!("{}/Cargo.toml", m.dir), &m.manifest)),
    );
    for (path, text) in manifests {
        match text.parse::<toml::Table>() {
            Ok(table) if depends_on_mutants(&table) => problems.push(problem(
                "source-skip",
                format!("{path} depends on the `mutants` crate"),
            )),
            Ok(_) => {}
            Err(e) => problems.push(problem(
                "source-skip",
                format!("{path} does not parse: {e}"),
            )),
        }
    }
    problems
}

/// The keys `.cargo/mutants.toml` may have (P4.2). Any other key, including
/// `examine_globs` and `examine_re`, is `[unreviewed-key]`; so is a key of
/// [`NEXTEST_MUTANTS_KEYS`] with any value but its reviewed one (ADR 0009).
const REVIEWED_MUTANTS_KEYS: [&str; 14] = [
    "test_tool",
    "additional_cargo_test_args",
    "cap_lints",
    "all_features",
    "test_workspace",
    "exclude_re",
    "exclude_globs",
    "skip_calls",
    "skip_calls_defaults",
    "timeout_multiplier",
    "minimum_test_timeout",
    "build_timeout_multiplier",
    "sharding",
    "error_values",
];

/// ADR 0009: the `.cargo/mutants.toml` keys that are reviewed with one value
/// only, as (key, the value written inline, LIB-QA-012's code for it lost).
/// LIB-QA-007 accepts each key absent or with exactly this value; LIB-QA-012
/// requires it.
const NEXTEST_MUTANTS_KEYS: [(&str, &str, &str); 2] = [
    ("test_tool", r#""nextest""#, "nextest-test-tool"),
    (
        "additional_cargo_test_args",
        r#"["--profile=mutants", "--no-tests=warn"]"#,
        "nextest-test-args",
    ),
];

/// The config keys whose entries exclude mutants, each of which the triage
/// document must list (P4.8).
const EXCLUSION_KEYS: [&str; 3] = ["exclude_re", "exclude_globs", "skip_calls"];

/// The strings of the array `key`, or `[config-exclusion]` if it is not an
/// array of strings.
fn config_strings(config: &toml::Table, key: &str, problems: &mut Vec<String>) -> Vec<String> {
    match config.get(key) {
        None => Vec::new(),
        Some(toml::Value::Array(items)) if items.iter().all(toml::Value::is_str) => items
            .iter()
            .filter_map(toml::Value::as_str)
            .map(str::to_owned)
            .collect(),
        Some(other) => {
            problems.push(problem(
                "config-exclusion",
                format!("`{key}` must be an array of strings, found `{other}`"),
            ));
            Vec::new()
        }
    }
}

/// `text` read as a literal in regex syntax: every metacharacter escaped
/// with `\`, and `\` only before ASCII punctuation (so no `\d`, `\w`, `\b`).
fn regex_literal(text: &str) -> Option<String> {
    let mut out = String::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => match chars.next() {
                Some(escaped) if escaped.is_ascii_punctuation() => out.push(escaped),
                _ => return None,
            },
            '.' | '+' | '*' | '?' | '(' | ')' | '|' | '[' | ']' | '{' | '}' | '^' | '$' => {
                return None;
            }
            _ => out.push(c),
        }
    }
    Some(out)
}

/// `text` without a leading mutant position: `\d+:\d+` or `<line>:<column>`.
fn strip_position(text: &str) -> Option<&str> {
    if let Some(rest) = text.strip_prefix(r"\d+:\d+") {
        return Some(rest);
    }
    let digits = |s: &str| s.len() - s.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    let line = digits(text);
    let rest = text[line..].strip_prefix(':')?;
    let column = digits(rest);
    (line > 0 && column > 0).then(|| &rest[column..])
}

/// P4.7: why an `exclude_re` entry does not match exactly one mutant name in
/// an existing source, or `None` if it is well formed. The form is `^` + the
/// escaped path + `:` + a position + `: ` + the escaped description + `$`.
fn exclude_re_fault(entry: &str, sources: &[&str]) -> Option<String> {
    let Some(body) = entry.strip_prefix('^') else {
        return Some("does not start with `^`".to_owned());
    };
    let Some(body) = body.strip_suffix('$') else {
        return Some("does not end with `$`".to_owned());
    };
    if (body.len() - body.trim_end_matches('\\').len()) % 2 == 1 {
        return Some("ends with an escaped `$`".to_owned());
    }
    let Some((path, rest)) = body.split_once(':') else {
        return Some("has no `<path>:` prefix".to_owned());
    };
    let Some(path) = regex_literal(path) else {
        return Some(format!(
            "has a path `{path}` that is not an escaped literal"
        ));
    };
    if !sources.contains(&path.as_str()) {
        return Some(format!("names `{path}`, which is not a workspace source"));
    }
    let Some(description) = strip_position(rest).and_then(|rest| rest.strip_prefix(": ")) else {
        return Some("has no `\\d+:\\d+: ` position after the path".to_owned());
    };
    match regex_literal(description) {
        None => Some(format!(
            "has a description `{description}` that is not an escaped literal"
        )),
        Some(description) if description.trim().is_empty() => {
            Some("has an empty description".to_owned())
        }
        Some(_) => None,
    }
}

/// P4.6 and P4.7: `[config-exclusion]` for every `exclude_globs` and
/// `exclude_re` entry that is broader than one file or one mutant.
fn exclusion_entry_problems(
    workspace: &WorkspaceSources,
    globs: &[String],
    regexes: &[String],
    problems: &mut Vec<String>,
) {
    let sources = workspace.source_paths();
    for glob in globs {
        if glob.contains(['*', '?', '[', ']', '{', '}', '!']) || !sources.contains(&glob.as_str()) {
            problems.push(problem(
                "config-exclusion",
                format!("`exclude_globs` entry `{glob}` must be the exact path of an existing source under a member's src/"),
            ));
        }
    }
    for member in &workspace.members {
        if !member.sources.is_empty() && member.sources.iter().all(|(path, _)| globs.contains(path))
        {
            problems.push(problem(
                "config-exclusion",
                format!("`exclude_globs` excludes every source of {}", member.dir),
            ));
        }
    }
    for entry in regexes {
        if let Some(fault) = exclude_re_fault(entry, &sources) {
            problems.push(problem(
                "config-exclusion",
                format!("`exclude_re` entry `{entry}` {fault}; it must match exactly one mutant"),
            ));
        }
    }
}

// The triage document, docs/quality/mutants-triage.md (P4.8, P8). The test
// reads two things from it; everything else in it is for people.
//
// 1. Exactly one Markdown table whose header row is exactly
//
//        | Mutant | Disposition | Row | Reason | Excluded |
//
//    followed by a separator row (`|---|---|---|---|---|`) and one row per
//    triaged mutant (every missed or timed-out mutant, P8.2), up to the first
//    line that does not start with `|`:
//
//    - Mutant: the mutant's exact name, usually in a code span.
//    - Disposition: `killed`, `equivalent`, `accepted A`, `accepted B` or
//      `accepted C` (the classes of P8.2).
//    - Row: the id of the matrix row that owns the code, e.g. `LIB-TOOL-001`.
//    - Reason: the argument; non-empty for an excluded mutant.
//    - Excluded: `—` (or `-`, `no`, empty) if the configuration does not
//      exclude it; otherwise a code span `<key>: <entry>`, where <key> is
//      `exclude_re`, `exclude_globs` or `skip_calls`, and <entry> is the
//      entry's string exactly as `.cargo/mutants.toml` has it. Only an
//      `equivalent` or `accepted` mutant may be excluded.
//
//    As in GitHub Markdown, a `|` inside a cell, code spans included, is
//    written `\|`. So the regex `\|\|` is written `\\|\\|`.
//
// 2. The `skip_calls_defaults` choice, as the text
//    `skip_calls_defaults = true` or `skip_calls_defaults = false`, matching
//    the configuration (absent there means `true`, cargo-mutants' default).
//
// The cross-check: every `exclude_re`, `exclude_globs` and `skip_calls` entry
// has a row whose Excluded cell names it, with a reason and a matrix row
// (`[exclusion-undocumented]`); every Excluded cell names an entry the
// configuration has (`[exclusion-stale]`); a malformed table is
// `[triage-format]`.

const TRIAGE_HEADER: [&str; 5] = ["Mutant", "Disposition", "Row", "Reason", "Excluded"];
const TRIAGE_DISPOSITIONS: [&str; 5] = [
    "killed",
    "equivalent",
    "accepted A",
    "accepted B",
    "accepted C",
];

/// One row of the triage table.
#[derive(Debug)]
struct TriageEntry {
    line: usize,
    disposition: String,
    row: String,
    reason: String,
    excluded: Option<(String, String)>,
}

/// The cells of a Markdown table row, split on `|` not preceded by `\`, with
/// `\|` read as `|`.
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
    if ticks == 0 || cell.len() < 2 * ticks || !cell.ends_with(&"`".repeat(ticks)) {
        return cell;
    }
    let inner = &cell[ticks..cell.len() - ticks];
    match inner.strip_prefix(' ').and_then(|i| i.strip_suffix(' ')) {
        Some(stripped) if !stripped.trim().is_empty() => stripped,
        _ => inner,
    }
}

/// The triage table's rows; `[triage-format]` for anything malformed.
fn triage_entries(triage: &str, problems: &mut Vec<String>) -> Vec<TriageEntry> {
    let lines: Vec<&str> = triage.lines().collect();
    let headers: Vec<usize> = (0..lines.len())
        .filter(|&i| {
            lines[i].trim_start().starts_with('|') && table_cells(lines[i]) == TRIAGE_HEADER
        })
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
    let separator = lines.get(header + 1).map(|l| table_cells(l));
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
    let mut entries = Vec::new();
    for (index, line) in lines.iter().enumerate().skip(header + 2) {
        if !line.trim_start().starts_with('|') {
            break;
        }
        let number = index + 1;
        let cells = table_cells(line);
        let [_, disposition, row, reason, excluded] = cells.as_slice() else {
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
            problems.push(problem("triage-format", format!("triage line {number}: unknown disposition `{disposition}`; allowed: {TRIAGE_DISPOSITIONS:?}")));
        }
        let excluded = code_span_text(excluded);
        let excluded = if ["", "—", "-", "no"].contains(&excluded) {
            None
        } else if let Some((key, entry)) = excluded
            .split_once(": ")
            .filter(|(key, _)| EXCLUSION_KEYS.contains(key))
        {
            Some((key.to_owned(), entry.to_owned()))
        } else {
            problems.push(problem("triage-format", format!("triage line {number}: the Excluded cell must be `—` or `<key>: <entry>` with a key in {EXCLUSION_KEYS:?}, found `{excluded}`")));
            None
        };
        entries.push(TriageEntry {
            line: number,
            disposition: disposition.clone(),
            row: row.clone(),
            reason: reason.clone(),
            excluded,
        });
    }
    entries
}

/// P4.8: every exclusion of the configuration is triaged with a reason and a
/// matrix row, and every triaged exclusion is in the configuration.
fn triage_cross_check(
    config: &toml::Table,
    exclusions: &[(String, String)],
    triage: &str,
    matrix_ids: &[String],
    problems: &mut Vec<String>,
) {
    let entries = triage_entries(triage, problems);
    for (key, entry) in exclusions {
        let documented = entries
            .iter()
            .find(|e| e.excluded.as_ref() == Some(&(key.clone(), entry.clone())));
        let fault = match documented {
            None => Some("has no triage entry".to_owned()),
            Some(e) if e.reason.trim().is_empty() => {
                Some(format!("has an empty reason (triage line {})", e.line))
            }
            Some(e) if !matrix_ids.contains(&e.row) => Some(format!(
                "cites `{}`, which is not a matrix row (triage line {})",
                e.row, e.line
            )),
            Some(e)
                if !e.disposition.starts_with("equivalent")
                    && !e.disposition.starts_with("accepted") =>
            {
                Some(format!(
                    "is triaged as `{}`, but only an equivalent or accepted mutant may be excluded (triage line {})",
                    e.disposition, e.line
                ))
            }
            Some(_) => None,
        };
        if let Some(fault) = fault {
            problems.push(problem(
                "exclusion-undocumented",
                format!("the `{key}` entry `{entry}` {fault}"),
            ));
        }
    }
    for e in &entries {
        if let Some((key, entry)) = &e.excluded {
            if !exclusions.contains(&(key.clone(), entry.clone())) {
                problems.push(problem(
                    "exclusion-stale",
                    format!("triage line {} says `{key}: {entry}` is excluded, but {MUTANTS_CONFIG} has no such entry", e.line),
                ));
            }
        }
    }
    let defaults = match config.get("skip_calls_defaults") {
        None => true,
        Some(toml::Value::Boolean(value)) => *value,
        Some(other) => {
            problems.push(problem(
                "config-exclusion",
                format!("`skip_calls_defaults` must be a boolean, found `{other}`"),
            ));
            true
        }
    };
    let stated = triage.contains(&format!("skip_calls_defaults = {defaults}"));
    let contradicted = triage.contains(&format!("skip_calls_defaults = {}", !defaults));
    if !stated || contradicted {
        problems.push(problem(
            "exclusion-undocumented",
            format!("the triage document must state `skip_calls_defaults = {defaults}`, the configuration's choice, and not the other value"),
        ));
    }
}

/// LIB-QA-007 P4: `.cargo/mutants.toml`, cross-checked with the triage
/// document, and the workspace's sources and manifests scanned for skips.
fn check_mutants_config(
    config: &str,
    workspace: &WorkspaceSources,
    triage: &str,
    matrix_ids: &[String],
) -> Vec<String> {
    let mut problems = source_skip_problems(workspace);
    let table: toml::Table = match config.parse() {
        Ok(table) => table,
        Err(e) => {
            problems.push(problem(
                "config-parse",
                format!("{MUTANTS_CONFIG} does not parse: {e}"),
            ));
            return problems;
        }
    };
    for (key, value) in &table {
        if !REVIEWED_MUTANTS_KEYS.contains(&key.as_str()) {
            problems.push(problem(
                "unreviewed-key",
                format!("{MUTANTS_CONFIG} has the key `{key}`, which has not been reviewed"),
            ));
        } else if let Some((_, reviewed, _)) = NEXTEST_MUTANTS_KEYS
            .iter()
            .find(|(fixed, _, _)| fixed == key)
        {
            if *value != toml_value(reviewed) {
                problems.push(problem(
                    "unreviewed-key",
                    format!("{MUTANTS_CONFIG} has `{key} = {value}`; only `{key} = {reviewed}` has been reviewed (ADR 0009)"),
                ));
            }
        }
    }
    let is_true = |key: &str| matches!(table.get(key), Some(toml::Value::Boolean(true)));
    require(
        &mut problems,
        is_true("cap_lints"),
        "config-cap-lints",
        "`cap_lints` must be exactly true (D5)",
    );
    require(
        &mut problems,
        is_true("all_features"),
        "config-features",
        "`all_features` must be exactly true (D6)",
    );
    require(
        &mut problems,
        matches!(
            table.get("test_workspace"),
            Some(toml::Value::Boolean(false))
        ),
        "config-test-scope",
        "`test_workspace` must be stated, exactly false (D8)",
    );
    let globs = config_strings(&table, "exclude_globs", &mut problems);
    let regexes = config_strings(&table, "exclude_re", &mut problems);
    let calls = config_strings(&table, "skip_calls", &mut problems);
    exclusion_entry_problems(workspace, &globs, &regexes, &mut problems);
    let exclusions: Vec<(String, String)> = [
        ("exclude_re", &regexes),
        ("exclude_globs", &globs),
        ("skip_calls", &calls),
    ]
    .into_iter()
    .flat_map(|(key, entries)| entries.iter().map(move |e| (key.to_owned(), e.clone())))
    .collect();
    triage_cross_check(&table, &exclusions, triage, matrix_ids, &mut problems);
    problems
}

/// LIB-QA-007 P5 (D12): `.gitignore` ignores `/mutants.out*` and re-includes
/// nothing. Each line has its trailing space characters (U+0020) stripped,
/// and nothing else: git strips only unescaped trailing spaces (gitignore(5),
/// dir.c `trim_trailing_spaces`), so a trailing tab, a backslash-escaped
/// trailing space and any leading whitespace stay part of the pattern.
/// Every line whose first character is `!` negates a pattern and is
/// reported, whatever it names: telling which paths a negation re-includes
/// would mean reimplementing git's matching, and D12 allows none. An
/// indented `!` or an escaped `\!` is a literal, not a negation.
fn check_mutants_gitignore(gitignore: &str) -> Vec<String> {
    let lines: Vec<&str> = gitignore.lines().map(|l| l.trim_end_matches(' ')).collect();
    let mut problems = Vec::new();
    require(
        &mut problems,
        lines.contains(&MUTANTS_GITIGNORE_RULE),
        "gitignore-mutants",
        "`.gitignore` must contain the line `/mutants.out*`",
    );
    for line in lines.iter().filter(|l| l.starts_with('!')) {
        problems.push(problem(
            "gitignore-mutants",
            format!("`.gitignore` line `{line}` negates a pattern; D12 re-includes nothing"),
        ));
    }
    problems
}

/// The reference `.cargo/mutants.toml` (P4; D5, D6, D8). With no exclusions,
/// which is what the configuration is to have until the baseline triage
/// excludes an equivalent or accepted mutant.
const CONFIG_FIXTURE: &str =
    "# cargo-mutants configuration (LIB-QA-007). Every exclusion here is listed,
# with its reason and owning row, in docs/quality/mutants-triage.md.

# A lint that `-D warnings` denies must not turn a mutant unviable (D5).
cap_lints = true

# Build and test with every feature, as CI does (D6).
all_features = true

# A mutant must be killed by its own package's tests, not another package's (D8).
test_workspace = false
";

/// ADR 0009 (LIB-QA-012 item 1): the test tool and its arguments, appended to
/// [`CONFIG_FIXTURE`].
const NEXTEST_MUTANTS_LINES: &str = "
# Run the tests under nextest, with the profile that stops every running test
# at the first failure (ADR 0009, LIB-QA-012).
test_tool = \"nextest\"
additional_cargo_test_args = [\"--profile=mutants\", \"--no-tests=warn\"]
";

/// An example of each kind of exclusion, in the form P4.6 and P4.7 accept.
/// (Not real triage: the mutant is made up.)
const CONFIG_EXCLUSIONS: &str = r#"
exclude_re = [
  '^xtask/src/fetch\.rs:\d+:\d+: replace example -> bool with true$',
]
exclude_globs = ["xtask/src/main.rs"]
skip_calls = ["example_call"]
"#;

/// The reference triage document's machine-read parts (P4.8; the format is
/// described above [`TRIAGE_HEADER`]).
const TRIAGE_FIXTURE: &str = "# Mutation-testing triage

`skip_calls_defaults = true`: cargo-mutants' default, so calls to `with_capacity` are not mutated.

| Mutant | Disposition | Row | Reason | Excluded |
|---|---|---|---|---|
| `xtask/src/lib.rs:20:5: replace repo_root -> PathBuf with Default::default()` | killed | LIB-TOOL-001 | A test cited under LIB-TOOL-001 asserts that the root holds `xtask/Cargo.toml`. | — |
";

/// The triage rows for [`CONFIG_EXCLUSIONS`].
const TRIAGE_EXCLUSIONS: &str = r"| `xtask/src/fetch.rs:10:5: replace example -> bool with true` | equivalent | LIB-TOOL-002 | Example: no reachable input tells it apart. | `exclude_re: ^xtask/src/fetch\.rs:\d+:\d+: replace example -> bool with true$` |
| every mutant in `xtask/src/main.rs` | accepted A | LIB-TOOL-001 | Example: compiled out on Linux. | `exclude_globs: xtask/src/main.rs` |
| every call to `example_call` | equivalent | LIB-TOOL-001 | Example: the call has no observable effect. | `skip_calls: example_call` |
";

fn config_with_exclusions() -> String {
    format!("{CONFIG_FIXTURE}{CONFIG_EXCLUSIONS}")
}

fn triage_with_exclusions() -> String {
    format!("{TRIAGE_FIXTURE}{TRIAGE_EXCLUSIONS}")
}

/// A configuration mutation: the config, triage and workspace the check
/// reads, and the codes it must report.
struct ConfigCase {
    name: String,
    config: String,
    triage: String,
    workspace: WorkspaceSources,
    codes: &'static [&'static str],
}

fn config_case(
    name: impl Into<String>,
    config: String,
    triage: String,
    codes: &'static [&'static str],
) -> ConfigCase {
    ConfigCase {
        name: name.into(),
        config,
        triage,
        workspace: real_workspace(),
        codes,
    }
}

/// `CONFIG_FIXTURE` with `lines` appended, and the reference triage.
fn config_plus(name: impl Into<String>, lines: &str, codes: &'static [&'static str]) -> ConfigCase {
    config_case(
        name,
        format!("{CONFIG_FIXTURE}{lines}\n"),
        TRIAGE_FIXTURE.to_owned(),
        codes,
    )
}

/// The real `.cargo/mutants.toml`, triage document, workspace and matrix
/// must pass P4. A missing file is `[config-missing]` or `[triage-missing]`.
fn assert_real_mutants_config() {
    let config = read_optional(MUTANTS_CONFIG);
    let triage = read_optional(MUTANTS_TRIAGE);
    let mut problems = Vec::new();
    require(
        &mut problems,
        config.is_some(),
        "config-missing",
        &format!("{MUTANTS_CONFIG} does not exist"),
    );
    require(
        &mut problems,
        triage.is_some(),
        "triage-missing",
        &format!("{MUTANTS_TRIAGE} does not exist"),
    );
    problems.extend(check_mutants_config(
        config.as_deref().unwrap_or_default(),
        &real_workspace(),
        triage.as_deref().unwrap_or_default(),
        &matrix_ids(),
    ));
    assert!(
        problems.is_empty(),
        "LIB-QA-007: {MUTANTS_CONFIG} must be the reviewed configuration, and agree with {MUTANTS_TRIAGE}: {problems:#?}"
    );
}

/// P4 applied three ways: the reference configurations pass, every case is
/// reported with its codes, and then the real configuration passes.
fn assert_config_cases(cases: &[ConfigCase]) {
    let ids = matrix_ids();
    let workspace = real_workspace();
    for (name, config, triage) in [
        (
            "the reference configuration",
            CONFIG_FIXTURE.to_owned(),
            TRIAGE_FIXTURE.to_owned(),
        ),
        (
            "the reference exclusions",
            config_with_exclusions(),
            triage_with_exclusions(),
        ),
    ] {
        let problems = check_mutants_config(&config, &workspace, &triage, &ids);
        assert!(
            problems.is_empty(),
            "LIB-QA-007: {name} must pass before its mutations are meaningful: {problems:#?}"
        );
    }
    for case in cases {
        assert!(
            !case.codes.is_empty(),
            "LIB-QA-007: the case `{}` names no expected code",
            case.name
        );
        let problems = check_mutants_config(&case.config, &case.workspace, &case.triage, &ids);
        for code in case.codes {
            assert!(
                has_code(&problems, code),
                "LIB-QA-007: the config check must report `{}` as `[{code}]`; it reported {problems:#?}; config:\n{}\ntriage:\n{}",
                case.name,
                case.config,
                case.triage
            );
        }
    }
    assert_real_mutants_config();
}

#[test]
fn lib_qa_007_mutants_config_is_reviewed() {
    assert_config_cases(&[]);
}

/// P4.2: keys outside [`REVIEWED_MUTANTS_KEYS`], or (ADR 0009) a key of
/// [`NEXTEST_MUTANTS_KEYS`] with another value, each with a value that would
/// narrow or redirect the run.
const UNREVIEWED_MUTANTS_KEYS: [(&str, &str); 13] = [
    ("additional_cargo_args", r#"["--release"]"#),
    ("additional_cargo_test_args", r#"["--", "--skip", "lib_"]"#),
    ("output", r#""elsewhere""#),
    ("profile", r#""mutants""#),
    ("test_tool", r#""cargo""#),
    ("test_package", r#"["sacm"]"#),
    ("copy_target", "true"),
    ("copy_vcs", "true"),
    ("gitignore", "false"),
    ("features", r#"["unstable"]"#),
    ("no_default_features", "true"),
    ("examine_globs", r#"["xtask/src/lib.rs"]"#),
    ("examine_re", r#"["matrix"]"#),
];

#[test]
fn lib_qa_007_mutants_config_check_rejects_parse_errors_and_unreviewed_keys() {
    let mut cases = vec![
        config_case(
            "a key without a value",
            "cap_lints =\n".to_owned(),
            TRIAGE_FIXTURE.to_owned(),
            &["config-parse"],
        ),
        config_plus("a duplicate key", "cap_lints = true", &["config-parse"]),
        config_plus(
            "an unterminated table header",
            "[unterminated",
            &["config-parse"],
        ),
        config_case(
            "not TOML at all",
            "{ \"cap_lints\": true }".to_owned(),
            TRIAGE_FIXTURE.to_owned(),
            &["config-parse"],
        ),
        config_plus("a future key", "future_key = true", &["unreviewed-key"]),
    ];
    for (key, value) in UNREVIEWED_MUTANTS_KEYS {
        cases.push(config_plus(
            format!("{key} = {value}"),
            &format!("{key} = {value}"),
            &["unreviewed-key"],
        ));
    }
    assert_config_cases(&cases);
}

/// ADR 0009: values of `test_tool` and `additional_cargo_test_args` other
/// than the reviewed ones, each of which is `[unreviewed-key]`.
const OTHER_NEXTEST_VALUES: [(&str, &str); 16] = [
    ("test_tool", r#""cargo""#),
    ("test_tool", r#""Nextest""#),
    ("test_tool", r#""nextest ""#),
    ("test_tool", r#"["nextest"]"#),
    ("test_tool", "true"),
    ("additional_cargo_test_args", "[]"),
    ("additional_cargo_test_args", r#"["--profile=mutants"]"#),
    ("additional_cargo_test_args", r#"["--no-tests=warn"]"#),
    (
        "additional_cargo_test_args",
        r#"["--profile=default", "--no-tests=warn"]"#,
    ),
    (
        "additional_cargo_test_args",
        r#"["--profile=mutants", "--no-tests=pass"]"#,
    ),
    (
        "additional_cargo_test_args",
        r#"["--no-tests=warn", "--profile=mutants"]"#,
    ),
    (
        "additional_cargo_test_args",
        r#"["--profile", "mutants", "--no-tests=warn"]"#,
    ),
    (
        "additional_cargo_test_args",
        r#"["--profile=mutants", "--no-tests=warn", "-E", "not test(lib_)"]"#,
    ),
    (
        "additional_cargo_test_args",
        r#"["--profile=mutants", "--no-tests=warn", "--no-fail-fast"]"#,
    ),
    (
        "additional_cargo_test_args",
        r#""--profile=mutants --no-tests=warn""#,
    ),
    ("additional_cargo_test_args", r#"["--", "--skip", "lib_"]"#),
];

/// ADR 0009, LIB-QA-012 item 4: the LIB-QA-007 config check accepts
/// `test_tool` and `additional_cargo_test_args` absent or with exactly their
/// reviewed values, and refuses every other value as `[unreviewed-key]`.
#[test]
fn lib_qa_007_mutants_config_check_accepts_only_the_reviewed_nextest_values() {
    let nextest = format!("{CONFIG_FIXTURE}{NEXTEST_MUTANTS_LINES}");
    let cases: Vec<ConfigCase> = OTHER_NEXTEST_VALUES
        .iter()
        .map(|(key, value)| {
            let reviewed = NEXTEST_MUTANTS_KEYS
                .iter()
                .find(|(fixed, _, _)| fixed == key)
                .map(|(_, reviewed, _)| format!("{key} = {reviewed}"))
                .expect("the key has a reviewed value");
            config_case(
                format!("{key} = {value}"),
                replace_once(&nextest, &reviewed, &format!("{key} = {value}")),
                TRIAGE_FIXTURE.to_owned(),
                &["unreviewed-key"],
            )
        })
        .collect();
    let ids = matrix_ids();
    let workspace = real_workspace();
    for case in &cases {
        let problems = check_mutants_config(&case.config, &workspace, &case.triage, &ids);
        assert_eq!(
            problem_codes(&problems),
            ["unreviewed-key"],
            "LIB-QA-007: `{}` must give exactly one `[unreviewed-key]`; it gave {problems:#?}",
            case.name
        );
        let key = case.name.split(" = ").next().unwrap_or_default();
        assert!(
            problems[0].contains(&format!("`{key} = ")),
            "LIB-QA-007: the `[unreviewed-key]` for `{}` must name the key and its value: {problems:#?}",
            case.name
        );
    }
    for (name, config) in [
        ("both keys with their reviewed values", nextest.clone()),
        (
            "the reviewed values as literal strings",
            nextest.replace(r#""nextest""#, "'nextest'").replace(
                r#"["--profile=mutants", "--no-tests=warn"]"#,
                "[\n  '--profile=mutants',\n  '--no-tests=warn',\n]",
            ),
        ),
        (
            "test_tool alone",
            replace_once(
                &nextest,
                "additional_cargo_test_args = [\"--profile=mutants\", \"--no-tests=warn\"]\n",
                "",
            ),
        ),
        ("neither key", CONFIG_FIXTURE.to_owned()),
    ] {
        let problems = check_mutants_config(&config, &workspace, TRIAGE_FIXTURE, &ids);
        assert!(
            problems.is_empty(),
            "LIB-QA-007: `{name}` is reviewed, but was reported: {problems:#?}; config:\n{config}"
        );
    }
    assert_config_cases(&cases);
}

#[test]
fn lib_qa_007_mutants_config_check_requires_cap_lints_features_and_test_scope() {
    let without = |line: &str| replace_once(CONFIG_FIXTURE, &format!("{line}\n"), "");
    let replaced = |line: &str, by: &str| replace_once(CONFIG_FIXTURE, line, by);
    let triage = || TRIAGE_FIXTURE.to_owned();
    assert_config_cases(&[
        config_case(
            "cap_lints absent",
            without("cap_lints = true"),
            triage(),
            &["config-cap-lints"],
        ),
        config_case(
            "cap_lints = false",
            replaced("cap_lints = true", "cap_lints = false"),
            triage(),
            &["config-cap-lints"],
        ),
        config_case(
            "cap_lints = \"true\"",
            replaced("cap_lints = true", "cap_lints = \"true\""),
            triage(),
            &["config-cap-lints"],
        ),
        config_case(
            "all_features absent",
            without("all_features = true"),
            triage(),
            &["config-features"],
        ),
        config_case(
            "all_features = false",
            replaced("all_features = true", "all_features = false"),
            triage(),
            &["config-features"],
        ),
        config_case(
            "test_workspace absent",
            without("test_workspace = false"),
            triage(),
            &["config-test-scope"],
        ),
        config_case(
            "test_workspace = true",
            replaced("test_workspace = false", "test_workspace = true"),
            triage(),
            &["config-test-scope"],
        ),
    ]);
}

#[test]
fn lib_qa_007_mutants_config_check_rejects_broad_exclude_globs() {
    let mut cases: Vec<ConfigCase> = [
        "xtask/**",
        "xtask/src/*.rs",
        "*.rs",
        "**",
        "src",
        "matrix.rs",
        "xtask/src/nonexistent.rs",
        "xtask/src/[lm]*.rs",
        "xtask/tests/repo_checks.rs",
    ]
    .iter()
    .map(|glob| {
        let triage = format!("{TRIAGE_FIXTURE}| every mutant in `{glob}` | accepted A | LIB-TOOL-001 | Example. | `exclude_globs: {glob}` |\n");
        config_case(
            format!("exclude_globs = [\"{glob}\"]"),
            format!("{CONFIG_FIXTURE}exclude_globs = [\"{glob}\"]\n"),
            triage,
            &["config-exclusion"],
        )
    })
    .collect();
    // Every source of a member, read from the real workspace, so that a new
    // file under its src/ cannot leave one out.
    let workspace = real_workspace();
    for dir in ["xtask", "crates/sacm-cli"] {
        let member_sources: Vec<String> = workspace
            .members
            .iter()
            .find(|member| member.dir == dir)
            .unwrap_or_else(|| panic!("LIB-QA-007: {dir} is a workspace member"))
            .sources
            .iter()
            .map(|(path, _)| format!("{path:?}"))
            .collect();
        assert!(
            member_sources.len() > 1,
            "LIB-QA-007: precondition: {dir} has several sources to exclude one by one: {member_sources:?}"
        );
        cases.push(config_plus(
            format!("every source of {dir}, one by one"),
            &format!("exclude_globs = [{}]", member_sources.join(", ")),
            &["config-exclusion"],
        ));
    }
    cases.push(config_plus(
        "exclude_globs as a string",
        r#"exclude_globs = "xtask/src/fetch.rs""#,
        &["config-exclusion"],
    ));
    assert_config_cases(&cases);
}

#[test]
fn lib_qa_007_mutants_config_check_rejects_broad_exclude_re() {
    let mut cases: Vec<ConfigCase> = [
        "",
        ".*",
        "xtask",
        "matrix",
        "replace .* with true",
        r"^xtask/src/matrix\.rs",
        r"^xtask/src/matrix\.rs:.*$",
        r"^xtask/src/nope\.rs:\d+:\d+: replace example -> bool with true$",
        r"^xtask/src/matrix.rs:\d+:\d+: replace example -> bool with true$",
        r"(?i)^xtask/src/matrix\.rs:\d+:\d+: replace example -> bool with true$",
        r"^xtask/src/matrix\.rs:\d+:\d+: replace .* with true$",
        r"^xtask/src/matrix\.rs:\d+:\d+: replace \w+ -> bool with true$",
        r"^xtask/src/matrix\.rs:\d+:\d+: $",
        r"^xtask/src/matrix\.rs:\d+:\d+: replace example -> bool with true\$",
        r"^xtask/src/matrix\.rs:\d+: replace example -> bool with true$",
        r"^xtask/src/matrix\.rs:\d+:\d+:replace example -> bool with true$",
        r"^xtask/src/matrix\.rs:\d+:\d+: replace example -> bool with true$|^.*$",
    ]
    .iter()
    .map(|entry| {
        let cell = entry.replace('|', "\\|");
        let triage = format!("{TRIAGE_FIXTURE}| `{cell}` | equivalent | LIB-TOOL-001 | Example. | `exclude_re: {cell}` |\n");
        config_case(
            format!("exclude_re = ['{entry}']"),
            format!("{CONFIG_FIXTURE}exclude_re = ['{entry}']\n"),
            triage,
            &["config-exclusion"],
        )
    })
    .collect();
    cases.push(config_plus(
        "exclude_re as a string",
        "exclude_re = 'matrix'",
        &["config-exclusion"],
    ));
    cases.push(config_plus(
        "skip_calls as a string",
        "skip_calls = 'example_call'",
        &["config-exclusion"],
    ));
    assert_config_cases(&cases);
}

/// P4.8: mutations of the reference exclusions and their triage rows.
fn triage_cases() -> Vec<ConfigCase> {
    let config = config_with_exclusions();
    let triage = triage_with_exclusions();
    let rows: Vec<&str> = TRIAGE_EXCLUSIONS.lines().collect();
    let triage_without = |row: &str| replace_once(&triage, &format!("{row}\n"), "");
    let triage_edit = |from: &str, to: &str| replace_once(&triage, from, to);
    let config_without = |text: &str| replace_once(&config, text, "");
    let undocumented: &'static [&'static str] = &["exclusion-undocumented"];
    vec![
        config_case(
            "the exclude_re row removed",
            config.clone(),
            triage_without(rows[0]),
            undocumented,
        ),
        config_case(
            "the exclude_globs row removed",
            config.clone(),
            triage_without(rows[1]),
            undocumented,
        ),
        config_case(
            "the skip_calls row removed",
            config.clone(),
            triage_without(rows[2]),
            undocumented,
        ),
        config_case(
            "an empty reason",
            config.clone(),
            triage_edit("| Example: no reachable input tells it apart. |", "|  |"),
            undocumented,
        ),
        config_case(
            "a row id not in the matrix",
            config.clone(),
            triage_edit(
                "| equivalent | LIB-TOOL-002 |",
                "| equivalent | LIB-NOPE-999 |",
            ),
            undocumented,
        ),
        config_case(
            "an excluded mutant triaged as killed",
            config.clone(),
            triage_edit("| equivalent | LIB-TOOL-002 |", "| killed | LIB-TOOL-002 |"),
            undocumented,
        ),
        config_case(
            "the exclude_re entry differs from its row",
            replace_once(
                &config,
                "replace example -> bool with true$",
                "replace example -> bool with false$",
            ),
            triage.clone(),
            &["exclusion-undocumented", "exclusion-stale"],
        ),
        config_case(
            "a triaged exclusion with no config entry",
            config_without("skip_calls = [\"example_call\"]\n"),
            triage.clone(),
            &["exclusion-stale"],
        ),
        config_case(
            "the skip_calls_defaults statement removed",
            config.clone(),
            triage_edit("`skip_calls_defaults = true`: ", ""),
            undocumented,
        ),
        config_case(
            "skip_calls_defaults stated false, default true",
            config.clone(),
            triage_edit(
                "`skip_calls_defaults = true`",
                "`skip_calls_defaults = false`",
            ),
            undocumented,
        ),
        config_case(
            "skip_calls_defaults = false, stated true",
            format!("{config}skip_calls_defaults = false\n"),
            triage.clone(),
            undocumented,
        ),
    ]
}

/// P4.8: a triage table the cross-check cannot read is `[triage-format]`.
fn triage_format_cases() -> Vec<ConfigCase> {
    let config = config_with_exclusions();
    let triage = triage_with_exclusions();
    let triage_edit = |from: &str, to: &str| replace_once(&triage, from, to);
    let format_code: &'static [&'static str] = &["triage-format"];
    vec![
        config_case(
            "no triage table",
            config.clone(),
            "# Mutation-testing triage\n".to_owned(),
            format_code,
        ),
        config_case(
            "the header changed",
            config.clone(),
            triage_edit("| Reason | Excluded |", "| Excluded |"),
            format_code,
        ),
        config_case(
            "a row with four cells",
            config.clone(),
            triage_edit(
                "| accepted A | LIB-TOOL-001 | Example: compiled out on Linux. |",
                "| accepted A | LIB-TOOL-001 |",
            ),
            format_code,
        ),
        config_case(
            "an unknown disposition",
            config.clone(),
            triage_edit("| accepted A |", "| unlikely |"),
            format_code,
        ),
        config_case(
            "an Excluded cell with an unknown key",
            config,
            triage_edit("`skip_calls: example_call`", "`skip: example_call`"),
            format_code,
        ),
    ]
}

#[test]
fn lib_qa_007_mutants_config_check_cross_checks_the_triage_document() {
    let mut cases = triage_cases();
    cases.extend(triage_format_cases());
    assert_config_cases(&cases);
}

/// P4.9: source-skip spellings, each appended to a workspace source.
const SOURCE_SKIPS: [&str; 13] = [
    "#[mutants::skip]\nfn skipped() {}\n",
    "#![mutants::skip]\n",
    "#[::mutants::skip]\nfn skipped() {}\n",
    "#[mutants :: skip]\nfn skipped() {}\n",
    "#[mutants/*c*/::skip]\nfn skipped() {}\n",
    "#[cfg_attr(test, mutants::skip)]\nfn skipped() {}\n",
    "#[cfg_attr(any(), mutants::skip)]\nfn skipped() {}\n",
    "#[cfg_attr(\n    test,\n    mutants\n        ::skip\n)]\nfn skipped() {}\n",
    "fn quote() -> char { '\"' }\n#[mutants::skip]\nfn skipped() {}\n",
    "const Q: char = '\\'';\n#[mutants::skip]\nfn skipped() {}\n",
    "const S: &str = \"/*\";\n#[mutants::skip]\nfn skipped() {}\n",
    "const R: &str = r#\"\" /*\"#;\n#[mutants::skip]\nfn skipped() {}\n",
    "fn f<'a>(s: &'a str) -> &'a str { s }\n#[mutants::skip]\nfn skipped() {}\n",
];

/// P4.9: text that mentions `mutants::skip` without being an attribute.
const SOURCE_NON_SKIPS: [&str; 6] = [
    "// #[mutants::skip]\n",
    "/// Not `#[mutants::skip]`.\nfn documented() {}\n",
    "/* #[mutants::skip] */\n",
    "/* nested /* #[mutants::skip] */ still a comment */\n",
    "const S: &str = \"#[mutants::skip]\";\n",
    "const R: &str = r#\"#[mutants::skip]\"#;\n",
];

#[test]
fn lib_qa_007_mutants_config_check_rejects_source_skips() {
    let workspace = real_workspace();
    let triage = || TRIAGE_FIXTURE.to_owned();
    let mut cases = Vec::new();
    for path in [
        "xtask/src/lib.rs",
        "crates/sacm-cli/src/main.rs",
        "crates/sacm/src/lib.rs",
    ] {
        for skip in SOURCE_SKIPS {
            cases.push(ConfigCase {
                name: format!("{skip:?} in {path}"),
                config: CONFIG_FIXTURE.to_owned(),
                triage: triage(),
                workspace: workspace.with_source(path, |text| format!("{text}\n{skip}")),
                codes: &["source-skip"],
            });
        }
    }
    let dependency = |name: &str, dir: &str, from: &str, to: &str| ConfigCase {
        name: name.to_owned(),
        config: CONFIG_FIXTURE.to_owned(),
        triage: triage(),
        workspace: workspace.with_manifest(dir, |text| replace_once(text, from, to)),
        codes: &["source-skip"],
    };
    // xtask's manifest may or may not have a `[dev-dependencies]` table, so
    // this control adds the dependency to the parsed manifest, creating the
    // table if needed; the edited manifest still parses, so the code comes
    // from the dependency and not from a parse failure.
    let dev_dependency = workspace.with_manifest("xtask", |text| {
        let mut manifest: toml::Table = text.parse().expect("xtask/Cargo.toml parses");
        manifest
            .entry("dev-dependencies")
            .or_insert_with(|| toml::Value::Table(toml::Table::new()))
            .as_table_mut()
            .expect("[dev-dependencies] is a table")
            .insert("mutants".to_owned(), toml_value("\"0.0.3\""));
        let edited = manifest.to_string();
        assert!(
            edited.parse::<toml::Table>().is_ok(),
            "LIB-QA-007: precondition: the edited xtask manifest parses:\n{edited}"
        );
        edited
    });
    cases.extend([
        ConfigCase {
            name: "xtask dev-depends on mutants".to_owned(),
            config: CONFIG_FIXTURE.to_owned(),
            triage: triage(),
            workspace: dev_dependency,
            codes: &["source-skip"],
        },
        dependency(
            "sacm-cli depends on mutants, renamed",
            "crates/sacm-cli",
            "[dependencies]\n",
            "[dependencies]\nskip = { package = \"mutants\", version = \"0.0.3\" }\n",
        ),
        dependency(
            "a workspace dependency on mutants",
            "",
            "[workspace.dependencies]\n",
            "[workspace.dependencies]\nmutants = \"0.0.3\"\n",
        ),
        dependency(
            "sacm depends on mutants for one target",
            "crates/sacm",
            "[lints]\n",
            "[target.'cfg(unix)'.dependencies]\nmutants = \"0.0.3\"\n\n[lints]\n",
        ),
        dependency(
            "sacm build-depends on mutants",
            "crates/sacm",
            "[lints]\n",
            "[build-dependencies]\nmutants = \"0.0.3\"\n\n[lints]\n",
        ),
    ]);
    for text in SOURCE_NON_SKIPS {
        let edited =
            workspace.with_source("xtask/src/lib.rs", |source| format!("{source}\n{text}"));
        let problems = source_skip_problems(&edited);
        assert!(
            problems.is_empty(),
            "LIB-QA-007: {text:?} only mentions `mutants::skip`, but was reported: {problems:#?}"
        );
    }
    let problems = source_skip_problems(&workspace);
    assert!(
        problems.is_empty(),
        "LIB-QA-007: the real sources must have no mutants::skip: {problems:#?}"
    );
    assert_config_cases(&cases);
}

#[test]
fn lib_qa_007_mutants_config_check_accepts_reviewed_configuration() {
    let ids = matrix_ids();
    let workspace = real_workspace();
    let literal_position = r"'^xtask/src/fetch\.rs:10:5: replace example -> bool with true$'";
    let pipes_config =
        r"exclude_re = ['^xtask/src/matrix\.rs:\d+:\d+: replace \|\| with && in looks_traced$']";
    let pipes_row = r"| `xtask/src/matrix.rs:492:5: replace \|\| with && in looks_traced` | equivalent | LIB-TOOL-001 | Example. | `exclude_re: ^xtask/src/matrix\.rs:\d+:\d+: replace \\|\\| with && in looks_traced$` |";
    let controls = [
        (
            "the reference configuration",
            CONFIG_FIXTURE.to_owned(),
            TRIAGE_FIXTURE.to_owned(),
        ),
        (
            "the reference exclusions",
            config_with_exclusions(),
            triage_with_exclusions(),
        ),
        (
            "every other reviewed key",
            format!(
                "{CONFIG_FIXTURE}timeout_multiplier = 2.0\nminimum_test_timeout = 60.0\nbuild_timeout_multiplier = 3.0\nsharding = \"round-robin\"\nerror_values = ['::anyhow::anyhow!(\"mutated\")']\nskip_calls_defaults = true\n"
            ),
            TRIAGE_FIXTURE.to_owned(),
        ),
        (
            "skip_calls_defaults = false, stated false",
            format!("{CONFIG_FIXTURE}skip_calls_defaults = false\n"),
            TRIAGE_FIXTURE.replace("skip_calls_defaults = true", "skip_calls_defaults = false"),
        ),
        (
            "empty exclusion lists",
            format!("{CONFIG_FIXTURE}exclude_re = []\nexclude_globs = []\nskip_calls = []\n"),
            TRIAGE_FIXTURE.to_owned(),
        ),
        (
            "an exclude_re with a literal position",
            config_with_exclusions().replace(
                r"'^xtask/src/fetch\.rs:\d+:\d+: replace example -> bool with true$'",
                literal_position,
            ),
            triage_with_exclusions().replace(
                r"`exclude_re: ^xtask/src/fetch\.rs:\d+:\d+:",
                r"`exclude_re: ^xtask/src/fetch\.rs:10:5:",
            ),
        ),
        (
            "an exclude_re with escaped pipes",
            format!("{CONFIG_FIXTURE}{pipes_config}\n"),
            format!("{TRIAGE_FIXTURE}{pipes_row}\n"),
        ),
        (
            "an exclude_globs entry leaving other sources",
            format!("{CONFIG_FIXTURE}exclude_globs = [\"xtask/src/fetch.rs\"]\n"),
            format!(
                "{TRIAGE_FIXTURE}| every mutant in `xtask/src/fetch.rs` | accepted B | LIB-TOOL-002 | Example. | `exclude_globs: xtask/src/fetch.rs` |\n"
            ),
        ),
        (
            "an excluded cell without a code span, and a not-excluded `-`",
            config_with_exclusions(),
            triage_with_exclusions()
                .replace("`skip_calls: example_call`", "skip_calls: example_call")
                .replace("| — |", "| - |"),
        ),
        (
            "the test tool and its arguments of ADR 0009",
            format!("{CONFIG_FIXTURE}{NEXTEST_MUTANTS_LINES}"),
            TRIAGE_FIXTURE.to_owned(),
        ),
    ];
    for (name, config, triage) in &controls {
        let problems = check_mutants_config(config, &workspace, triage, &ids);
        assert!(
            problems.is_empty(),
            "LIB-QA-007: `{name}` is a reviewed configuration, but was reported: {problems:#?}; config:\n{config}\ntriage:\n{triage}"
        );
    }
    assert_real_mutants_config();
}

/// The reference `.gitignore` rule for cargo-mutants' output (P5, D12).
const GITIGNORE_FIXTURE: &str = "/target\n\n# cargo-mutants output (LIB-QA-007).\n/mutants.out*\n";

#[test]
fn lib_qa_007_mutants_output_is_git_ignored() {
    let g = GITIGNORE_FIXTURE;
    let problems = check_mutants_gitignore(g);
    assert!(
        problems.is_empty(),
        "LIB-QA-007: the reference rule must pass: {problems:#?}"
    );
    for (name, text) in [
        ("rule removed", delete_line(g, MUTANTS_GITIGNORE_RULE)),
        (
            "rule commented out",
            comment_line(g, MUTANTS_GITIGNORE_RULE),
        ),
        (
            "rule without its *",
            replace_line(g, MUTANTS_GITIGNORE_RULE, "/mutants.out"),
        ),
        (
            "rule under another directory",
            replace_line(g, MUTANTS_GITIGNORE_RULE, "/target/mutants.out*"),
        ),
        ("a re-include", format!("{g}!/mutants.out/outcomes.json\n")),
        // A negation un-ignores `mutants.out` without naming it literally;
        // every `!` line is reported (D12 re-includes nothing).
        ("a re-include of everything", format!("{g}!*\n")),
        ("a re-include by wildcard", format!("{g}!/mutants.ou?\n")),
        // gitignore(5) strips trailing spaces only: an indented line is a
        // different pattern and does not ignore the root `mutants.out*`.
        (
            "rule indented",
            replace_line(g, MUTANTS_GITIGNORE_RULE, "  /mutants.out*"),
        ),
        (
            "rule indented, with trailing spaces",
            replace_line(g, MUTANTS_GITIGNORE_RULE, "  /mutants.out*  "),
        ),
        // git strips trailing spaces, not tabs (dir.c `trim_trailing_spaces`):
        // `/mutants.out*<TAB>` does not ignore `mutants.out`.
        (
            "rule with a trailing tab",
            replace_line(g, MUTANTS_GITIGNORE_RULE, "/mutants.out*\t"),
        ),
        // A backslash-escaped trailing space is kept by git, so the pattern
        // ends in a literal space and does not ignore `mutants.out`.
        (
            "rule with an escaped trailing space",
            replace_line(g, MUTANTS_GITIGNORE_RULE, "/mutants.out*\\ "),
        ),
    ] {
        let problems = check_mutants_gitignore(&text);
        assert!(
            has_code(&problems, "gitignore-mutants"),
            "LIB-QA-007: the mutation `{name}` must be reported as `[gitignore-mutants]`; it reported {problems:#?}; text:\n{text:?}"
        );
    }
    for (name, text) in [
        // gitignore(5) ignores unescaped trailing spaces.
        (
            "rule with trailing spaces",
            replace_line(g, MUTANTS_GITIGNORE_RULE, "/mutants.out*  "),
        ),
        // An indented `!` line is not a negation (gitignore(5): the prefix
        // `!` negates), so it re-includes nothing.
        (
            "an indented `!` line",
            format!("{g}  !/mutants.out/outcomes.json\n"),
        ),
        // gitignore(5): a backslash before the first `!` makes it literal.
        ("an escaped `\\!` line", format!("{g}\\!/mutants.out\n")),
        // git reads CRLF line endings; `str::lines` strips the `\r`.
        ("CRLF line endings", g.replace('\n', "\r\n")),
    ] {
        let problems = check_mutants_gitignore(&text);
        assert!(
            problems.is_empty(),
            "LIB-QA-007: `{name}` ignores `mutants.out*` and re-includes nothing, but was reported: {problems:#?}; text:\n{text:?}"
        );
    }
    let problems = check_mutants_gitignore(&read(".gitignore"));
    assert!(
        problems.is_empty(),
        "LIB-QA-007: .gitignore must ignore `/mutants.out*`: {problems:#?}"
    );
}

// ---------------------------------------------------------------------------
// LIB-QA-012: mutation outcomes do not depend on test-target order.
//
// ADR 0009: cargo-mutants runs the tests under nextest, with a profile that
// kills every running test at the first failure. The check reads the two
// files that make that so, and reports each part lost with its own code:
//
// - `.cargo/mutants.toml` (item 1): `test_tool` (`[nextest-test-tool]`) and
//   `additional_cargo_test_args` (`[nextest-test-args]`), each exactly its
//   reviewed value in [`NEXTEST_MUTANTS_KEYS`].
// - `.config/nextest.toml` (item 2): the version requirement
//   (`[nextest-version]`); the `mutants` profile (`[nextest-profile]`), its
//   `fail-fast` table (`[nextest-fail-fast]`), `max-fail = 1`
//   (`[nextest-max-fail]`) and `terminate = "immediate"`
//   (`[nextest-terminate]`); and no `fail-fast` in the `default` profile
//   (`[nextest-default-fail-fast]`). The `mutants` profile may also set
//   `default-filter`, to exactly [`NEXTEST_MUTANTS_DEFAULT_FILTER`]; any
//   other value is `[nextest-default-filter]`. Every other key is
//   `[nextest-unreviewed-key]`: every profile inherits the `default` one, so
//   a key such as `default-filter`, `retries`, `slow-timeout` or `overrides`
//   there, or in the `mutants` profile, changes which tests run or what
//   counts as a failure. Other profiles are not read: `mutants` does not
//   inherit them.
//
// A file missing is `[nextest-config-missing]`, and one that does not parse
// `[nextest-config-parse]`. Versions are compared as numbers, so 0.9.111 is
// above 0.9.99.
// ---------------------------------------------------------------------------

const QA12: &str = "LIB-QA-012";

const NEXTEST_CONFIG: &str = ".config/nextest.toml";

/// A release version `MAJOR.MINOR.PATCH`, compared as numbers.
type Version = (u64, u64, u64);

/// ADR 0009: the first nextest with `terminate = "immediate"`.
const NEXTEST_MINIMUM: Version = (0, 9, 111);

/// The one reviewed `default-filter` of the `mutants` profile: it leaves out
/// the test that runs each example with a nested `cargo run`, which is slow
/// per mutant and tests no library logic (slice 0091). Any other filter could
/// hide tests from cargo-mutants.
const NEXTEST_MUTANTS_DEFAULT_FILTER: &str =
    "not test(lib_doc_004_each_example_runs_without_arguments_and_exits_zero)";

/// The keys `.config/nextest.toml` may have at its top level.
const NEXTEST_TOP_KEYS: [&str; 2] = ["nextest-version", "profile"];

fn show_version((major, minor, patch): Version) -> String {
    format!("{major}.{minor}.{patch}")
}

/// `text` as a plain release version: three decimal numbers, none with a
/// sign or a leading zero, and no pre-release or build suffix.
fn plain_version(text: &str) -> Option<Version> {
    let number = |part: &str| {
        let digits = !part.is_empty() && part.chars().all(|c| c.is_ascii_digit());
        let canonical = part == "0" || !part.starts_with('0');
        if digits && canonical {
            part.parse::<u64>().ok()
        } else {
            None
        }
    };
    let mut parts = text.split('.');
    let version = (
        number(parts.next()?)?,
        number(parts.next()?)?,
        number(parts.next()?)?,
    );
    parts.next().is_none().then_some(version)
}

/// Item 1: `test_tool` and `additional_cargo_test_args` of
/// `.cargo/mutants.toml` (`None`: it does not exist).
fn nextest_mutants_key_problems(config: Option<&str>, problems: &mut Vec<String>) {
    let Some(config) = config else {
        problems.push(problem(
            "nextest-config-missing",
            format!("{MUTANTS_CONFIG} does not exist, so cargo-mutants does not run the tests under nextest"),
        ));
        return;
    };
    let table: toml::Table = match config.parse() {
        Ok(table) => table,
        Err(e) => {
            problems.push(problem(
                "nextest-config-parse",
                format!("{MUTANTS_CONFIG} does not parse: {e}"),
            ));
            return;
        }
    };
    for (key, reviewed, code) in NEXTEST_MUTANTS_KEYS {
        match table.get(key) {
            None => problems.push(problem(
                code,
                format!("{MUTANTS_CONFIG} must set `{key} = {reviewed}`; it does not set `{key}`"),
            )),
            Some(value) if *value != toml_value(reviewed) => problems.push(problem(
                code,
                format!("{MUTANTS_CONFIG} must set `{key} = {reviewed}` exactly, found `{key} = {value}`"),
            )),
            Some(_) => {}
        }
    }
}

/// Item 2: `nextest-version`, written `"X.Y.Z"` or `{ required = "X.Y.Z" }`
/// (with an optional `recommended`), at least [`NEXTEST_MINIMUM`].
fn nextest_version_problems(value: Option<&toml::Value>, problems: &mut Vec<String>) {
    let code = "nextest-version";
    let minimum = show_version(NEXTEST_MINIMUM);
    let Some(value) = value else {
        problems.push(problem(
            code,
            format!("{NEXTEST_CONFIG} must set `nextest-version = {{ required = \"{minimum}\" }}` or later; it does not set `nextest-version`"),
        ));
        return;
    };
    let required = match value {
        toml::Value::String(text) => Some(text.as_str()),
        toml::Value::Table(table) => {
            for key in table
                .keys()
                .filter(|key| !["required", "recommended"].contains(&key.as_str()))
            {
                problems.push(problem(
                    "nextest-unreviewed-key",
                    format!("{NEXTEST_CONFIG}'s `nextest-version` has the key `{key}`, which has not been reviewed"),
                ));
            }
            table.get("required").and_then(toml::Value::as_str)
        }
        _ => None,
    };
    match required.and_then(plain_version) {
        None => problems.push(problem(
            code,
            format!("{NEXTEST_CONFIG}'s `nextest-version` must require one version `X.Y.Z`, at least {minimum}; found `nextest-version = {value}`"),
        )),
        Some(version) if version < NEXTEST_MINIMUM => problems.push(problem(
            code,
            format!("{NEXTEST_CONFIG} requires nextest {}, below {minimum}, the first version with `terminate = \"immediate\"`", show_version(version)),
        )),
        Some(_) => {}
    }
}

/// Item 2: the `mutants` profile sets exactly `fail-fast = { max-fail = 1,
/// terminate = "immediate" }`, and no other key but a `default-filter` that
/// is exactly [`NEXTEST_MUTANTS_DEFAULT_FILTER`].
fn mutants_profile_problems(profile: &toml::Table, problems: &mut Vec<String>) {
    match profile.get("default-filter") {
        None => {}
        Some(value) if value.as_str() == Some(NEXTEST_MUTANTS_DEFAULT_FILTER) => {}
        Some(value) => problems.push(problem(
            "nextest-default-filter",
            format!("{NEXTEST_CONFIG}'s `[profile.mutants]` may set `default-filter` only to \"{NEXTEST_MUTANTS_DEFAULT_FILTER}\", found `default-filter = {value}`, which could hide tests from cargo-mutants"),
        )),
    }
    for key in profile
        .keys()
        .filter(|key| !["fail-fast", "default-filter"].contains(&key.as_str()))
    {
        problems.push(problem(
            "nextest-unreviewed-key",
            format!("{NEXTEST_CONFIG}'s `[profile.mutants]` has the key `{key}`, which has not been reviewed"),
        ));
    }
    let fail_fast = match profile.get("fail-fast") {
        Some(toml::Value::Table(fail_fast)) => fail_fast,
        other => {
            problems.push(problem(
                "nextest-fail-fast",
                format!(
                    "{NEXTEST_CONFIG}'s `[profile.mutants]` must set `fail-fast = {{ max-fail = 1, terminate = \"immediate\" }}`, found {}",
                    other.map_or_else(|| "no `fail-fast`".to_owned(), |v| format!("`fail-fast = {v}`"))
                ),
            ));
            return;
        }
    };
    for key in fail_fast
        .keys()
        .filter(|key| !["max-fail", "terminate"].contains(&key.as_str()))
    {
        problems.push(problem(
            "nextest-unreviewed-key",
            format!("{NEXTEST_CONFIG}'s `[profile.mutants]` fail-fast has the key `{key}`, which has not been reviewed"),
        ));
    }
    let shown = |key: &str| {
        fail_fast
            .get(key)
            .map_or_else(|| "none".to_owned(), |v| format!("`{key} = {v}`"))
    };
    if fail_fast.get("max-fail") != Some(&toml::Value::Integer(1)) {
        problems.push(problem(
            "nextest-max-fail",
            format!("{NEXTEST_CONFIG}'s `[profile.mutants]` fail-fast must have `max-fail = 1`, found {}", shown("max-fail")),
        ));
    }
    if fail_fast.get("terminate").and_then(toml::Value::as_str) != Some("immediate") {
        problems.push(problem(
            "nextest-terminate",
            format!("{NEXTEST_CONFIG}'s `[profile.mutants]` fail-fast must have `terminate = \"immediate\"`, found {}", shown("terminate")),
        ));
    }
}

/// Item 2: `.config/nextest.toml` (`None`: it does not exist).
fn nextest_config_problems(text: Option<&str>, problems: &mut Vec<String>) {
    let Some(text) = text else {
        problems.push(problem(
            "nextest-config-missing",
            format!("{NEXTEST_CONFIG} does not exist, so there is no `mutants` profile"),
        ));
        return;
    };
    let table: toml::Table = match text.parse() {
        Ok(table) => table,
        Err(e) => {
            problems.push(problem(
                "nextest-config-parse",
                format!("{NEXTEST_CONFIG} does not parse: {e}"),
            ));
            return;
        }
    };
    for key in table
        .keys()
        .filter(|key| !NEXTEST_TOP_KEYS.contains(&key.as_str()))
    {
        problems.push(problem(
            "nextest-unreviewed-key",
            format!("{NEXTEST_CONFIG} has the key `{key}`, which has not been reviewed"),
        ));
    }
    nextest_version_problems(table.get("nextest-version"), problems);
    let profiles = table.get("profile").and_then(toml::Value::as_table);
    match profiles
        .and_then(|profiles| profiles.get("mutants"))
        .and_then(toml::Value::as_table)
    {
        Some(profile) => mutants_profile_problems(profile, problems),
        None => problems.push(problem(
            "nextest-profile",
            format!("{NEXTEST_CONFIG} has no `[profile.mutants]` table"),
        )),
    }
    match profiles.and_then(|profiles| profiles.get("default")) {
        None => {}
        Some(toml::Value::Table(default)) => {
            for key in default.keys() {
                if key == "fail-fast" {
                    problems.push(problem(
                        "nextest-default-fail-fast",
                        format!("{NEXTEST_CONFIG}'s `[profile.default]` sets `fail-fast`; developers' `cargo nextest run` keeps nextest's own"),
                    ));
                } else {
                    problems.push(problem(
                        "nextest-unreviewed-key",
                        format!("{NEXTEST_CONFIG}'s `[profile.default]` has the key `{key}`, which every profile, `mutants` too, inherits; it has not been reviewed"),
                    ));
                }
            }
        }
        Some(other) => problems.push(problem(
            "nextest-unreviewed-key",
            format!("{NEXTEST_CONFIG}'s `profile.default` is not a table: `{other}`"),
        )),
    }
}

/// The two files LIB-QA-012 reads (`None`: the file does not exist).
#[derive(Clone, Debug, PartialEq, Eq)]
struct NextestSetup {
    mutants: Option<String>,
    nextest: Option<String>,
}

/// LIB-QA-012 items 1, 2 and 5: every part of ADR 0009's configuration.
fn check_nextest_setup(setup: &NextestSetup) -> Vec<String> {
    let mut problems = Vec::new();
    nextest_mutants_key_problems(setup.mutants.as_deref(), &mut problems);
    nextest_config_problems(setup.nextest.as_deref(), &mut problems);
    problems
}

/// The reference `.config/nextest.toml`.
const NEXTEST_CONFIG_FIXTURE: &str = "# nextest configuration (ADR 0009, LIB-QA-012).
nextest-version = { required = \"0.9.111\" }

# cargo-mutants runs the tests with this profile: at the first failure, every
# running test is killed.
[profile.mutants]
fail-fast = { max-fail = 1, terminate = \"immediate\" }
";

// Lines of `NEXTEST_CONFIG_FIXTURE` that controls edit.
const NEXTEST_VERSION_LINE: &str = "nextest-version = { required = \"0.9.111\" }";
const NEXTEST_PROFILE_LINE: &str = "[profile.mutants]";
const NEXTEST_FAIL_FAST_LINE: &str = "fail-fast = { max-fail = 1, terminate = \"immediate\" }";
const NEXTEST_PROFILE_BLOCK: &str =
    "[profile.mutants]\nfail-fast = { max-fail = 1, terminate = \"immediate\" }\n";

// Lines of the reference `.cargo/mutants.toml` that controls edit.
const TEST_TOOL_LINE: &str = "test_tool = \"nextest\"";
const TEST_ARGS_LINE: &str =
    "additional_cargo_test_args = [\"--profile=mutants\", \"--no-tests=warn\"]";

/// Problem codes LIB-QA-012 controls are expected to produce.
const QA12_ACCEPTED: &[&str] = &[];
const QA12_TEST_TOOL: &[&str] = &["nextest-test-tool"];
const QA12_TEST_ARGS: &[&str] = &["nextest-test-args"];
const QA12_PROFILE: &[&str] = &["nextest-profile"];
const QA12_FAIL_FAST: &[&str] = &["nextest-fail-fast"];
const QA12_MAX_FAIL: &[&str] = &["nextest-max-fail"];
const QA12_TERMINATE: &[&str] = &["nextest-terminate"];
const QA12_DEFAULT_FAIL_FAST: &[&str] = &["nextest-default-fail-fast"];
const QA12_UNREVIEWED: &[&str] = &["nextest-unreviewed-key"];
const QA12_DEFAULT_FILTER: &[&str] = &["nextest-default-filter"];
const QA12_VERSION: &[&str] = &["nextest-version"];
const QA12_CONFIG_MISSING: &[&str] = &["nextest-config-missing"];
const QA12_CONFIG_PARSE: &[&str] = &["nextest-config-parse"];

/// The reference setup: [`CONFIG_FIXTURE`] with the ADR 0009 keys, and
/// [`NEXTEST_CONFIG_FIXTURE`].
fn nextest_fixture() -> NextestSetup {
    NextestSetup {
        mutants: Some(format!("{CONFIG_FIXTURE}{NEXTEST_MUTANTS_LINES}")),
        nextest: Some(NEXTEST_CONFIG_FIXTURE.to_owned()),
    }
}

/// The reference setup with `.cargo/mutants.toml` replaced.
fn with_mutants_toml(text: Option<String>) -> NextestSetup {
    NextestSetup {
        mutants: text,
        ..nextest_fixture()
    }
}

/// The reference setup with `.config/nextest.toml` replaced.
fn with_nextest_toml(text: Option<String>) -> NextestSetup {
    NextestSetup {
        nextest: text,
        ..nextest_fixture()
    }
}

/// The reference `.cargo/mutants.toml` with the line `target` replaced.
fn mutants_toml_line(target: &str, replacement: &str) -> NextestSetup {
    let reference = nextest_fixture()
        .mutants
        .expect("the reference has a mutants.toml");
    with_mutants_toml(Some(replace_line(&reference, target, replacement)))
}

/// The reference `.config/nextest.toml` with the line `target` replaced.
fn nextest_toml_line(target: &str, replacement: &str) -> NextestSetup {
    with_nextest_toml(Some(replace_line(
        NEXTEST_CONFIG_FIXTURE,
        target,
        replacement,
    )))
}

/// The reference `.config/nextest.toml` with `lines` appended.
fn nextest_toml_plus(lines: &str) -> NextestSetup {
    with_nextest_toml(Some(format!("{NEXTEST_CONFIG_FIXTURE}\n{lines}\n")))
}

/// A LIB-QA-012 control: a setup and the exact codes it must give (none: it
/// is accepted).
struct NextestControl {
    name: String,
    setup: NextestSetup,
    codes: &'static [&'static str],
}

fn control(
    name: impl Into<String>,
    setup: NextestSetup,
    codes: &'static [&'static str],
) -> NextestControl {
    NextestControl {
        name: name.into(),
        setup,
        codes,
    }
}

/// Assert that the reference setup passes, and that each control, which
/// must differ from it, gives exactly its codes: no fewer, no others, each as
/// many times as named. A control's TOML files parse, unless it is about a
/// file that cannot be read, so that the check and not the parser rejects it.
fn assert_nextest_controls(controls: &[NextestControl]) {
    let reference = nextest_fixture();
    let problems = check_nextest_setup(&reference);
    assert!(
        problems.is_empty(),
        "{QA12}: the reference setup must pass before its controls mean anything: {problems:#?}"
    );
    for NextestControl { name, setup, codes } in controls {
        assert_ne!(
            setup, &reference,
            "{QA12}: precondition: the control `{name}` changes the reference"
        );
        let unreadable = codes
            .iter()
            .any(|code| matches!(*code, "nextest-config-missing" | "nextest-config-parse"));
        if !unreadable {
            for text in [&setup.mutants, &setup.nextest].into_iter().flatten() {
                assert!(
                    text.parse::<toml::Table>().is_ok(),
                    "{QA12}: precondition: the control `{name}` keeps its TOML parsable:\n{text}"
                );
            }
        }
        let mut want = codes.to_vec();
        want.sort_unstable();
        let problems = check_nextest_setup(setup);
        assert_eq!(
            problem_codes(&problems),
            want,
            "{QA12}: the control `{name}` must give exactly {want:?}; it gave {problems:#?}; setup:\n{setup:#?}"
        );
    }
}

/// Item 5: the real `.cargo/mutants.toml` and `.config/nextest.toml` keep
/// every part of ADR 0009's configuration.
#[test]
fn lib_qa_012_the_real_files_run_mutants_under_nextest() {
    let problems = check_nextest_setup(&NextestSetup {
        mutants: read_optional(MUTANTS_CONFIG),
        nextest: read_optional(NEXTEST_CONFIG),
    });
    assert!(
        problems.is_empty(),
        "{QA12}: {MUTANTS_CONFIG} and {NEXTEST_CONFIG} must run cargo-mutants under nextest with the `mutants` profile (ADR 0009): {problems:#?}"
    );
}

/// Items 1 and 5: `test_tool` and `additional_cargo_test_args` removed or
/// changed, each with its own code.
#[test]
fn lib_qa_012_each_mutants_toml_key_lost_is_named() {
    let reference = nextest_fixture()
        .mutants
        .expect("the reference has a mutants.toml");
    let mut controls = vec![
        control(
            "test_tool removed",
            with_mutants_toml(Some(delete_line(&reference, TEST_TOOL_LINE))),
            QA12_TEST_TOOL,
        ),
        control(
            "additional_cargo_test_args removed",
            with_mutants_toml(Some(delete_line(&reference, TEST_ARGS_LINE))),
            QA12_TEST_ARGS,
        ),
        control(
            "both removed",
            with_mutants_toml(Some(CONFIG_FIXTURE.to_owned())),
            &["nextest-test-tool", "nextest-test-args"],
        ),
        control(
            "test_tool commented out",
            with_mutants_toml(Some(comment_line(&reference, TEST_TOOL_LINE))),
            QA12_TEST_TOOL,
        ),
    ];
    for (key, value) in OTHER_NEXTEST_VALUES {
        let (line, code) = if key == "test_tool" {
            (TEST_TOOL_LINE, QA12_TEST_TOOL)
        } else {
            (TEST_ARGS_LINE, QA12_TEST_ARGS)
        };
        controls.push(control(
            format!("{key} = {value}"),
            mutants_toml_line(line, &format!("{key} = {value}")),
            code,
        ));
    }
    assert_nextest_controls(&controls);
}

/// Items 2 and 5: the `mutants` profile, its fail-fast, `max-fail` and
/// `terminate` removed or changed, each with its own code.
#[test]
fn lib_qa_012_each_part_of_the_mutants_profile_lost_is_named() {
    let fail_fast =
        |value: &str| nextest_toml_line(NEXTEST_FAIL_FAST_LINE, &format!("fail-fast = {value}"));
    let mut controls = vec![
        control(
            "[profile.mutants] removed",
            with_nextest_toml(Some(replace_once(
                NEXTEST_CONFIG_FIXTURE,
                NEXTEST_PROFILE_BLOCK,
                "",
            ))),
            QA12_PROFILE,
        ),
        control(
            "the profile renamed `mutant`",
            nextest_toml_line(NEXTEST_PROFILE_LINE, "[profile.mutant]"),
            QA12_PROFILE,
        ),
        control(
            "the profile renamed `Mutants`",
            nextest_toml_line(NEXTEST_PROFILE_LINE, "[profile.Mutants]"),
            QA12_PROFILE,
        ),
        control(
            "the profile as a string",
            with_nextest_toml(Some(
                replace_once(NEXTEST_CONFIG_FIXTURE, NEXTEST_PROFILE_BLOCK, "").replace(
                    NEXTEST_VERSION_LINE,
                    &format!("{NEXTEST_VERSION_LINE}\nprofile = \"mutants\""),
                ),
            )),
            QA12_PROFILE,
        ),
        control(
            "fail-fast removed",
            with_nextest_toml(Some(delete_line(
                NEXTEST_CONFIG_FIXTURE,
                NEXTEST_FAIL_FAST_LINE,
            ))),
            QA12_FAIL_FAST,
        ),
    ];
    for (value, codes) in [
        ("true", QA12_FAIL_FAST),
        ("false", QA12_FAIL_FAST),
        ("\"immediate\"", QA12_FAIL_FAST),
        ("1", QA12_FAIL_FAST),
        ("[1, \"immediate\"]", QA12_FAIL_FAST),
        ("{ terminate = \"immediate\" }", QA12_MAX_FAIL),
        ("{ max-fail = 2, terminate = \"immediate\" }", QA12_MAX_FAIL),
        ("{ max-fail = 0, terminate = \"immediate\" }", QA12_MAX_FAIL),
        (
            "{ max-fail = -1, terminate = \"immediate\" }",
            QA12_MAX_FAIL,
        ),
        (
            "{ max-fail = \"all\", terminate = \"immediate\" }",
            QA12_MAX_FAIL,
        ),
        (
            "{ max-fail = \"1\", terminate = \"immediate\" }",
            QA12_MAX_FAIL,
        ),
        (
            "{ max-fail = 1.0, terminate = \"immediate\" }",
            QA12_MAX_FAIL,
        ),
        ("{ max-fail = 1 }", QA12_TERMINATE),
        ("{ max-fail = 1, terminate = \"wait\" }", QA12_TERMINATE),
        (
            "{ max-fail = 1, terminate = \"Immediate\" }",
            QA12_TERMINATE,
        ),
        (
            "{ max-fail = 1, terminate = \"immediate \" }",
            QA12_TERMINATE,
        ),
        ("{ max-fail = 1, terminate = true }", QA12_TERMINATE),
        (
            "{ max-fail = 2, terminate = \"wait\" }",
            &["nextest-max-fail", "nextest-terminate"],
        ),
        ("{}", &["nextest-max-fail", "nextest-terminate"]),
        (
            "{ max-fail = 1, terminate = \"immediate\", retries = 1 }",
            QA12_UNREVIEWED,
        ),
    ] {
        controls.push(control(
            format!("fail-fast = {value}"),
            fail_fast(value),
            codes,
        ));
    }
    assert_nextest_controls(&controls);
}

/// Items 2 and 5: the `default` profile changing fail-fast is
/// `[nextest-default-fail-fast]`, whatever the value: developers' runs keep
/// nextest's own.
#[test]
fn lib_qa_012_the_default_profile_changing_fail_fast_is_named() {
    let mut controls = Vec::new();
    for value in [
        "false",
        "true",
        "{ max-fail = 1, terminate = \"immediate\" }",
        "{ max-fail = \"all\" }",
    ] {
        controls.push(control(
            format!("[profile.default] fail-fast = {value}"),
            nextest_toml_plus(&format!("[profile.default]\nfail-fast = {value}")),
            QA12_DEFAULT_FAIL_FAST,
        ));
    }
    controls.extend([
        control(
            "[profile.default.fail-fast] as a table",
            nextest_toml_plus("[profile.default.fail-fast]\nmax-fail = 2"),
            QA12_DEFAULT_FAIL_FAST,
        ),
        control(
            "[profile.default] fail-fast and retries",
            nextest_toml_plus("[profile.default]\nfail-fast = false\nretries = 2"),
            &["nextest-default-fail-fast", "nextest-unreviewed-key"],
        ),
    ]);
    assert_nextest_controls(&controls);
}

/// Item 2: any other key that the `mutants` profile sees (its own, or the
/// `default` profile's, which it inherits) is `[nextest-unreviewed-key]`.
/// The `mutants` profile's own `default-filter` is accepted with exactly its
/// reviewed value, and `[nextest-default-filter]` with any other.
#[test]
fn lib_qa_012_unreviewed_profile_keys_are_named() {
    let in_mutants = |line: &str| {
        with_nextest_toml(Some(insert_after(
            NEXTEST_CONFIG_FIXTURE,
            NEXTEST_FAIL_FAST_LINE,
            0,
            &[line],
        )))
    };
    assert_nextest_controls(&[
        control(
            "[profile.default] default-filter",
            nextest_toml_plus("[profile.default]\ndefault-filter = \"not test(lib_)\""),
            QA12_UNREVIEWED,
        ),
        control(
            "[profile.default] slow-timeout that kills tests",
            nextest_toml_plus(
                "[profile.default]\nslow-timeout = { period = \"1s\", terminate-after = 1 }",
            ),
            QA12_UNREVIEWED,
        ),
        control(
            "[profile.default] retries",
            nextest_toml_plus("[profile.default]\nretries = 3"),
            QA12_UNREVIEWED,
        ),
        control(
            "[[profile.default.overrides]]",
            nextest_toml_plus("[[profile.default.overrides]]\nfilter = \"all()\"\nretries = 3"),
            QA12_UNREVIEWED,
        ),
        control(
            "profile.default not a table",
            with_nextest_toml(Some(NEXTEST_CONFIG_FIXTURE.replace(
                NEXTEST_PROFILE_LINE,
                "[profile]\ndefault = 1\n\n[profile.mutants]",
            ))),
            QA12_UNREVIEWED,
        ),
        control(
            "[profile.mutants] retries",
            in_mutants("retries = 2"),
            QA12_UNREVIEWED,
        ),
        control(
            "[profile.mutants] the reviewed default-filter",
            in_mutants(&format!(
                "default-filter = \"{NEXTEST_MUTANTS_DEFAULT_FILTER}\""
            )),
            QA12_ACCEPTED,
        ),
        control(
            "[profile.mutants] default-filter",
            in_mutants("default-filter = \"not test(lib_)\""),
            QA12_DEFAULT_FILTER,
        ),
        control(
            "[profile.mutants] another default-filter",
            in_mutants("default-filter = \"not test(foo)\""),
            QA12_DEFAULT_FILTER,
        ),
        control(
            "[profile.mutants] the reviewed default-filter with a space added",
            in_mutants(&format!(
                "default-filter = \"{NEXTEST_MUTANTS_DEFAULT_FILTER} \""
            )),
            QA12_DEFAULT_FILTER,
        ),
        control(
            "[profile.mutants] default-filter not a string",
            in_mutants("default-filter = true"),
            QA12_DEFAULT_FILTER,
        ),
        control(
            "[profile.mutants] slow-timeout",
            in_mutants("slow-timeout = { period = \"1s\", terminate-after = 1 }"),
            QA12_UNREVIEWED,
        ),
        control(
            "[profile.mutants] test-threads",
            in_mutants("test-threads = 1"),
            QA12_UNREVIEWED,
        ),
        control(
            "a second nextest-version, inside [profile.mutants]",
            nextest_toml_plus(NEXTEST_VERSION_LINE),
            QA12_UNREVIEWED,
        ),
        control(
            "[profile.mutants] inherits",
            in_mutants("inherits = \"ci\""),
            QA12_UNREVIEWED,
        ),
        control(
            "[[profile.mutants.overrides]]",
            nextest_toml_plus("[[profile.mutants.overrides]]\nfilter = \"all()\"\nretries = 3"),
            QA12_UNREVIEWED,
        ),
    ]);
}

/// Item 2: any top-level key of `.config/nextest.toml` but `nextest-version`
/// and `profile`, or a key of `nextest-version` but `required` and
/// `recommended`, is `[nextest-unreviewed-key]`.
#[test]
fn lib_qa_012_unreviewed_top_level_nextest_keys_are_named() {
    let top_level = |line: &str| {
        with_nextest_toml(Some(insert_after(
            NEXTEST_CONFIG_FIXTURE,
            NEXTEST_VERSION_LINE,
            0,
            &[line],
        )))
    };
    assert_nextest_controls(&[
        control(
            "a top-level experimental",
            top_level("experimental = [\"setup-scripts\"]"),
            QA12_UNREVIEWED,
        ),
        control(
            "[scripts.setup]",
            nextest_toml_plus("[scripts.setup.prepare]\ncommand = \"true\""),
            QA12_UNREVIEWED,
        ),
        control(
            "[test-groups]",
            nextest_toml_plus("[test-groups.serial]\nmax-threads = 1"),
            QA12_UNREVIEWED,
        ),
        control(
            "[store]",
            nextest_toml_plus("[store]\ndir = \"elsewhere\""),
            QA12_UNREVIEWED,
        ),
        control(
            "an unknown key in nextest-version",
            nextest_toml_line(
                NEXTEST_VERSION_LINE,
                "nextest-version = { required = \"0.9.111\", minimum = \"0.9.111\" }",
            ),
            QA12_UNREVIEWED,
        ),
    ]);
}

/// Items 2 and 5: the version requirement missing, below 0.9.111 or not a
/// version is `[nextest-version]`. Versions compare as numbers: `0.9.99` is
/// below `0.9.111`, and `0.9.1000` above it.
#[test]
fn lib_qa_012_the_version_requirement_is_compared_as_numbers() {
    let required = |value: &str| {
        nextest_toml_line(
            NEXTEST_VERSION_LINE,
            &format!("nextest-version = {{ required = {value} }}"),
        )
    };
    let mut controls = vec![
        control(
            "nextest-version removed",
            with_nextest_toml(Some(delete_line(
                NEXTEST_CONFIG_FIXTURE,
                NEXTEST_VERSION_LINE,
            ))),
            QA12_VERSION,
        ),
        control(
            "nextest-version = 111",
            nextest_toml_line(NEXTEST_VERSION_LINE, "nextest-version = 111"),
            QA12_VERSION,
        ),
        control(
            "only a recommended version",
            nextest_toml_line(
                NEXTEST_VERSION_LINE,
                "nextest-version = { recommended = \"0.9.151\" }",
            ),
            QA12_VERSION,
        ),
        control(
            "a string form below 0.9.111",
            nextest_toml_line(NEXTEST_VERSION_LINE, "nextest-version = \"0.9.110\""),
            QA12_VERSION,
        ),
    ];
    for (value, codes) in [
        ("\"0.9.110\"", QA12_VERSION),
        ("\"0.9.99\"", QA12_VERSION),
        ("\"0.9.0\"", QA12_VERSION),
        ("\"0.8.999\"", QA12_VERSION),
        ("\"0.0.0\"", QA12_VERSION),
        ("\"0.9\"", QA12_VERSION),
        ("\"0.9.111.0\"", QA12_VERSION),
        ("\"v0.9.111\"", QA12_VERSION),
        ("\"0.9.111-rc.1\"", QA12_VERSION),
        ("\"0.9.111+build\"", QA12_VERSION),
        ("\"0.9.0111\"", QA12_VERSION),
        ("\" 0.9.111\"", QA12_VERSION),
        ("\"\"", QA12_VERSION),
        ("\"latest\"", QA12_VERSION),
        ("\"0.9.x\"", QA12_VERSION),
        ("\">=0.9.111\"", QA12_VERSION),
        ("111", QA12_VERSION),
        ("\"0.9.120\"", QA12_ACCEPTED),
        ("\"0.9.151\"", QA12_ACCEPTED),
        ("\"0.9.1000\"", QA12_ACCEPTED),
        ("\"0.10.0\"", QA12_ACCEPTED),
        ("\"1.0.0\"", QA12_ACCEPTED),
    ] {
        controls.push(control(
            format!("required = {value}"),
            required(value),
            codes,
        ));
    }
    assert_nextest_controls(&controls);
}

/// Item 5: a file missing or unreadable is named, never a skip.
#[test]
fn lib_qa_012_a_missing_or_unreadable_file_is_named() {
    let reference = nextest_fixture();
    assert_nextest_controls(&[
        control(
            ".cargo/mutants.toml missing",
            with_mutants_toml(None),
            QA12_CONFIG_MISSING,
        ),
        control(
            ".cargo/mutants.toml not TOML",
            with_mutants_toml(Some("{ \"test_tool\": \"nextest\" }".to_owned())),
            QA12_CONFIG_PARSE,
        ),
        control(
            ".cargo/mutants.toml with test_tool twice",
            with_mutants_toml(Some(format!(
                "{}{TEST_TOOL_LINE}\n",
                reference.mutants.clone().unwrap_or_default()
            ))),
            QA12_CONFIG_PARSE,
        ),
        control(
            ".config/nextest.toml missing",
            with_nextest_toml(None),
            QA12_CONFIG_MISSING,
        ),
        control(
            ".config/nextest.toml with an unterminated table header",
            nextest_toml_line(NEXTEST_PROFILE_LINE, "[profile.mutants"),
            QA12_CONFIG_PARSE,
        ),
        control(
            ".config/nextest.toml with nextest-version twice",
            with_nextest_toml(Some(insert_after(
                NEXTEST_CONFIG_FIXTURE,
                NEXTEST_VERSION_LINE,
                0,
                &[NEXTEST_VERSION_LINE],
            ))),
            QA12_CONFIG_PARSE,
        ),
        control(
            "both files missing",
            NextestSetup {
                mutants: None,
                nextest: None,
            },
            &["nextest-config-missing", "nextest-config-missing"],
        ),
    ]);
}

/// Items 1 and 2: the same setup written in other TOML forms, and other
/// profiles, which `mutants` does not inherit, are accepted.
#[test]
fn lib_qa_012_equivalent_forms_are_accepted() {
    let mutants = nextest_fixture()
        .mutants
        .expect("the reference has a mutants.toml");
    assert_nextest_controls(&[
        control(
            "mutants.toml values as literal strings, the array over lines",
            with_mutants_toml(Some(
                mutants
                    .replace(TEST_TOOL_LINE, "test_tool = 'nextest'")
                    .replace(
                        TEST_ARGS_LINE,
                        "additional_cargo_test_args = [\n  '--profile=mutants',\n  '--no-tests=warn',\n]",
                    ),
            )),
            QA12_ACCEPTED,
        ),
        control(
            "mutants.toml keys in another order",
            with_mutants_toml(Some(format!(
                "{TEST_ARGS_LINE}\n{}",
                delete_line(&mutants, TEST_ARGS_LINE)
            ))),
            QA12_ACCEPTED,
        ),
        control(
            "nextest-version as a string",
            nextest_toml_line(NEXTEST_VERSION_LINE, "nextest-version = \"0.9.111\""),
            QA12_ACCEPTED,
        ),
        control(
            "nextest-version with a recommended version",
            nextest_toml_line(
                NEXTEST_VERSION_LINE,
                "nextest-version = { required = \"0.9.111\", recommended = \"0.9.151\" }",
            ),
            QA12_ACCEPTED,
        ),
        control(
            "[profile.mutants.fail-fast] as a table",
            with_nextest_toml(Some(replace_once(
                NEXTEST_CONFIG_FIXTURE,
                NEXTEST_PROFILE_BLOCK,
                "[profile.mutants.fail-fast]\nmax-fail = 1\nterminate = \"immediate\"\n",
            ))),
            QA12_ACCEPTED,
        ),
        control(
            "dotted fail-fast keys",
            nextest_toml_line(
                NEXTEST_FAIL_FAST_LINE,
                "fail-fast.max-fail = 1\nfail-fast.terminate = \"immediate\"",
            ),
            QA12_ACCEPTED,
        ),
        control(
            "the whole file in dotted keys",
            with_nextest_toml(Some(
                "nextest-version.required = \"0.9.111\"\nprofile.mutants.fail-fast = { terminate = \"immediate\", max-fail = 1 }\n".to_owned(),
            )),
            QA12_ACCEPTED,
        ),
        control(
            "another profile, which `mutants` does not inherit",
            nextest_toml_plus("[profile.ci]\nretries = 2\nfail-fast = false"),
            QA12_ACCEPTED,
        ),
        control(
            "an empty [profile.default]",
            nextest_toml_plus("[profile.default]"),
            QA12_ACCEPTED,
        ),
        control(
            "comments",
            nextest_toml_plus("# Developers' runs keep nextest's defaults."),
            QA12_ACCEPTED,
        ),
    ]);
}

/// Item 4: the LIB-QA-007 config check accepts exactly ADR 0009's values.
/// The reference passes it, and every other value of the two keys is
/// LIB-QA-007's `[unreviewed-key]` and LIB-QA-012's code for that key.
#[test]
fn lib_qa_012_the_lib_qa_007_checks_accept_exactly_the_nextest_setup() {
    let ids = matrix_ids();
    let workspace = real_workspace();
    let config = |setup: &NextestSetup| {
        check_mutants_config(
            setup.mutants.as_deref().unwrap_or_default(),
            &workspace,
            TRIAGE_FIXTURE,
            &ids,
        )
    };
    let problems = config(&nextest_fixture());
    assert!(
        problems.is_empty(),
        "{QA12}: LIB-QA-007's config check must accept ADR 0009's setup: {problems:#?}"
    );
    for (key, value) in OTHER_NEXTEST_VALUES {
        let (line, code) = if key == "test_tool" {
            (TEST_TOOL_LINE, "nextest-test-tool")
        } else {
            (TEST_ARGS_LINE, "nextest-test-args")
        };
        let setup = mutants_toml_line(line, &format!("{key} = {value}"));
        let lib_qa_007 = config(&setup);
        let lib_qa_012 = check_nextest_setup(&setup);
        assert!(
            has_code(&lib_qa_007, "unreviewed-key") && problem_codes(&lib_qa_012) == [code],
            "{QA12}: `{key} = {value}` must be LIB-QA-007's `[unreviewed-key]` and LIB-QA-012's `[{code}]`; LIB-QA-007 gave {lib_qa_007:#?}, LIB-QA-012 gave {lib_qa_012:#?}"
        );
    }
}

// ---------------------------------------------------------------------------
// LIB-DOC-008: the ADR index and the links to ADRs
// ---------------------------------------------------------------------------
//
// The rest of the row's acceptance (what each ADR says) is checked by review.

const ADR_DIR: &str = "docs/adr";
const ADR_INDEX: &str = "docs/adr/README.md";
const DECISIONS: &str = "docs/sacm/decisions.md";

/// The ADR the mutations edit.
const ADR_0021: &str = "0021-xmi-reader-content.md";

/// An ADR file: its name in `docs/adr/` and its text.
type AdrFile = (String, String);

/// The number of an ADR file name `NNNN-*.md`, or `None` for any other name
/// (`README.md`, `template.md`).
fn adr_number(name: &str) -> Option<u32> {
    let digits = name.get(..4)?;
    let rest = &name[4..];
    let is_adr = digits.bytes().all(|b| b.is_ascii_digit())
        && rest.starts_with('-')
        && Path::new(rest).extension().is_some_and(|ext| ext == "md");
    if is_adr { digits.parse().ok() } else { None }
}

/// The names of the files in `docs/adr/`.
fn adr_dir_names() -> BTreeSet<String> {
    let entries = fs::read_dir(repository_root().join(ADR_DIR))
        .unwrap_or_else(|e| panic!("read directory {ADR_DIR}: {e}"));
    entries
        .map(|entry| entry.unwrap_or_else(|e| panic!("read directory {ADR_DIR}: {e}")))
        .filter(|entry| entry.path().is_file())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect()
}

/// The ADR files `docs/adr/NNNN-*.md`, sorted by name.
fn adr_files() -> Vec<AdrFile> {
    adr_dir_names()
        .into_iter()
        .filter(|name| adr_number(name).is_some())
        .map(|name| {
            let text = read(&format!("{ADR_DIR}/{name}"));
            (name, text)
        })
        .collect()
}

/// A status as the index check compares it: the first word of `text`,
/// lower-cased and without punctuation (`accepted`, `proposed`), or, for the
/// documented `superseded by NNNN`, those words with the successor's number
/// kept (a link `[NNNN](...)` gives its label). `None` if `text` states no
/// status, or `superseded` without `by NNNN`.
fn normal_status(text: &str) -> Option<String> {
    let word = |w: &str| -> String {
        w.chars()
            .filter(|c| c.is_alphanumeric() || *c == '-')
            .collect::<String>()
            .to_lowercase()
    };
    let mut words = text.split_whitespace();
    let first = word(words.next()?);
    if first != "superseded" {
        return (!first.is_empty()).then_some(first);
    }
    if words.next().map(word).as_deref() != Some("by") {
        return None;
    }
    let digits: String = words
        .next()?
        .trim_start_matches('[')
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    (digits.len() == 4).then(|| format!("superseded by {digits}"))
}

/// The status an ADR states: the [`normal_status`] of its first `- Status:`
/// line.
fn adr_status(text: &str) -> Option<String> {
    let rest = text
        .lines()
        .find_map(|line| line.trim().strip_prefix("- Status:"))?;
    normal_status(rest)
}

/// The source of the first table that the Markdown `text` renders, or `None`
/// if it renders none. A table in a code block or an HTML comment is not
/// rendered.
fn first_rendered_table(text: &str) -> Option<&str> {
    use pulldown_cmark::{Event, Options, Parser, Tag};
    Parser::new_ext(text, Options::ENABLE_TABLES)
        .into_offset_iter()
        .find_map(|(event, range)| {
            matches!(event, Event::Start(Tag::Table(_))).then(|| &text[range])
        })
}

/// Whether `cells` are a table's delimiter row (`|---|:--:|`).
fn is_delimiter_row(cells: &[String]) -> bool {
    cells
        .iter()
        .all(|cell| !cell.is_empty() && cell.chars().all(|c| matches!(c, '-' | ':')))
}

/// `(label, destination)` of a cell that is exactly one inline link
/// `[label](destination)`.
fn cell_link(cell: &str) -> Option<(&str, &str)> {
    let (label, rest) = cell.strip_prefix('[')?.split_once("](")?;
    Some((label, rest.strip_suffix(')')?))
}

/// A row of the ADR index: the ADR it links, its status cell, and that
/// cell's [`normal_status`].
struct IndexRow {
    number: u32,
    target: String,
    cell: String,
    status: Option<String>,
}

/// The rows of the first table the ADR index `index` renders, with a problem
/// in `problems` for each row that does not link an ADR by its number; or
/// `None`, with an `index-header` problem, if it renders no table with `ADR`
/// and `Status` columns.
fn index_rows(index: &str, problems: &mut Vec<String>) -> Option<Vec<IndexRow>> {
    let mut lines = first_rendered_table(index)
        .into_iter()
        .flat_map(str::lines)
        .filter(|line| !line.trim().is_empty())
        .map(table_cells);
    let Some(header) = lines.next() else {
        problems.push(problem("index-header", "the ADR index renders no table"));
        return None;
    };
    let column = |name: &str| header.iter().position(|cell| cell == name);
    let (Some(adr_column), Some(status_column)) = (column("ADR"), column("Status")) else {
        problems.push(problem(
            "index-header",
            format!("the index table needs `ADR` and `Status` columns; its header is {header:?}"),
        ));
        return None;
    };
    let mut rows: Vec<IndexRow> = Vec::new();
    for cells in lines.filter(|cells| !is_delimiter_row(cells)) {
        let link = cells
            .get(adr_column)
            .and_then(|cell| cell_link(cell))
            .and_then(|(label, target)| adr_number(target).map(|n| (label, target, n)));
        let Some((label, target, number)) = link else {
            problems.push(problem(
                "index-row",
                format!(
                    "the row `{}` does not link an ADR file `NNNN-*.md` in its ADR column",
                    cells.join(" | ")
                ),
            ));
            continue;
        };
        if target.get(..4) != Some(label) {
            problems.push(problem(
                "index-label",
                format!("the link to {target} is labelled `{label}`, not its number"),
            ));
        }
        let cell = cells.get(status_column).cloned().unwrap_or_default();
        rows.push(IndexRow {
            number,
            target: target.to_owned(),
            status: normal_status(&cell),
            cell,
        });
    }
    Some(rows)
}

/// LIB-DOC-008: the first table the ADR index `index` renders has one row per
/// file of `adrs`, linked to that file and labelled with its number, in number
/// order, whose status cell states the file's [`adr_status`]; and no other
/// rows. An ADR `superseded by NNNN` names an ADR file of `adrs`.
fn check_adr_index(index: &str, adrs: &[AdrFile]) -> Vec<String> {
    let mut problems = Vec::new();
    let Some(rows) = index_rows(index, &mut problems) else {
        return problems;
    };
    for pair in rows.windows(2) {
        if pair[1].number < pair[0].number {
            problems.push(problem(
                "index-order",
                format!("{} is listed after {}", pair[1].target, pair[0].target),
            ));
        }
    }
    let mut listed: BTreeMap<&str, Vec<&IndexRow>> = BTreeMap::new();
    for row in &rows {
        listed.entry(&row.target).or_default().push(row);
    }
    for (target, entries) in &listed {
        if entries.len() > 1 {
            problems.push(problem(
                "index-duplicate",
                format!("{target} is listed {} times", entries.len()),
            ));
        }
        if !adrs.iter().any(|(name, _)| name == target) {
            problems.push(problem(
                "index-dangling",
                format!("the index lists {target}, which does not exist"),
            ));
        }
    }
    for (name, text) in adrs {
        let Some(status) = adr_status(text) else {
            problems.push(problem(
                "adr-status",
                format!("{name} has no `- Status:` line with a status"),
            ));
            continue;
        };
        if let Some(successor) = status.strip_prefix("superseded by ") {
            let exists = adrs
                .iter()
                .any(|(other, _)| other.get(..4) == Some(successor));
            if !exists {
                problems.push(problem(
                    "status-successor",
                    format!("{name} is `{status}`, but there is no ADR {successor}"),
                ));
            }
        }
        let Some(entries) = listed.get(name.as_str()) else {
            problems.push(problem(
                "index-missing",
                format!("{name} is not listed in the index"),
            ));
            continue;
        };
        for row in entries
            .iter()
            .filter(|row| row.status.as_deref() != Some(status.as_str()))
        {
            problems.push(problem(
                "index-status",
                format!(
                    "the index gives {name} the status `{}`; the ADR says `{status}`",
                    row.cell
                ),
            ));
        }
    }
    problems
}

/// The one line of `text` that starts (trimmed) with `prefix`, trimmed.
/// Panics if there is not exactly one, so a mutation is never a no-op.
fn line_starting(text: &str, prefix: &str) -> String {
    let hits: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with(prefix))
        .collect();
    assert_eq!(
        hits.len(),
        1,
        "mutation target: exactly one line must start with `{prefix}`"
    );
    hits[0].to_owned()
}

/// The index row of ADR `number` (`"0021"`), trimmed.
fn index_row(index: &str, number: &str) -> String {
    line_starting(index, &format!("| [{number}]("))
}

/// `adrs` with the text of `name` replaced by `edit(text)`.
fn adrs_with(adrs: &[AdrFile], name: &str, edit: impl FnOnce(&str) -> String) -> Vec<AdrFile> {
    let mut out = adrs.to_vec();
    let text = out.iter_mut().find(|(n, _)| n == name).map_or_else(
        || panic!("mutation target `{name}` is an ADR file"),
        |(_, text)| text,
    );
    *text = edit(text);
    out
}

/// `adrs` with the `- Status:` line of `name` replaced by `status_line`.
fn adrs_with_status(adrs: &[AdrFile], name: &str, status_line: &str) -> Vec<AdrFile> {
    adrs_with(adrs, name, |text| {
        let line = line_starting(text, "- Status:");
        replace_line(text, &line, status_line)
    })
}

/// The last ADR row of the index table, whatever its number, so that
/// mutations appending a row do not change as ADRs are added.
fn last_index_row(index: &str) -> String {
    index
        .lines()
        .map(str::trim)
        .rfind(|line| line.starts_with("| ["))
        .expect("mutation target: the index table has rows")
        .to_owned()
}

/// The header line of the ADR index table.
const INDEX_HEADER: &str = "| ADR | Title | Status |";

/// `index` with its table between the lines `open` and `close` (a fence, an
/// HTML comment), so that it is no longer rendered as a table.
fn wrap_index_table(index: &str, open: &str, close: &str) -> String {
    let last_row = last_index_row(index);
    let opened = edit_line(index, INDEX_HEADER, |line| {
        vec![open.to_owned(), line.to_owned()]
    });
    edit_line(&opened, &last_row, |line| {
        vec![line.to_owned(), close.to_owned()]
    })
}

/// The successor ADR the superseded mutations add; a number no real ADR
/// will have soon, so the mutations do not change as ADRs are added.
const SUCCESSOR: &str = "9997-a-successor.md";

/// `index` and `adrs` with the ADR [`SUCCESSOR`] added (file and last index
/// row), and with ADR 0021 stating `status_line` and its index row the status
/// cell `cell`.
fn with_superseded(
    index: &str,
    adrs: &[AdrFile],
    status_line: &str,
    cell: &str,
) -> (String, Vec<AdrFile>) {
    let last_row = index_row(index, &adrs.last().expect("ADR files").0[..4]);
    let row_0021 = index_row(index, "0021");
    let index = insert_after(
        index,
        &last_row,
        0,
        &[&format!("| [9997]({SUCCESSOR}) | A successor | proposed |")],
    );
    let index = replace_line(
        &index,
        &row_0021,
        &row_0021.replace("| accepted |", &format!("| {cell} |")),
    );
    let mut adrs = adrs_with_status(adrs, ADR_0021, status_line);
    adrs.push((
        SUCCESSOR.to_owned(),
        "# 9997. A successor\n\n- Status: proposed\n".to_owned(),
    ));
    (index, adrs)
}

/// A mutation of the ADR index or the ADR files: its name, the index, the
/// ADR files, and the codes it must be reported with.
type IndexMutation = (&'static str, String, Vec<AdrFile>, &'static [&'static str]);

/// The mutations for the `superseded by NNNN` status and for an index whose
/// table does not render, each with exactly its codes.
fn adr_index_status_and_rendering_mutations(index: &str, adrs: &[AdrFile]) -> Vec<IndexMutation> {
    let superseded = |name: &'static str,
                      status_line: &str,
                      cell: &str,
                      codes: &'static [&'static str]|
     -> IndexMutation {
        let (index, adrs) = with_superseded(index, adrs, status_line, cell);
        (name, index, adrs, codes)
    };
    vec![
        superseded(
            "the index says superseded without the successor",
            "- Status: superseded by 9997",
            "superseded",
            &["index-status"],
        ),
        superseded(
            "the index names another successor",
            "- Status: superseded by 9997",
            "superseded by 9996",
            &["index-status"],
        ),
        superseded(
            "ADR 0021 superseded by an ADR that does not exist",
            "- Status: superseded by 9996",
            "superseded by 9996",
            &["status-successor"],
        ),
        superseded(
            "ADR 0021 says superseded without the successor",
            "- Status: superseded",
            "superseded",
            &["adr-status"],
        ),
        (
            "the index table in a fenced code block",
            wrap_index_table(index, "```text", "```"),
            adrs.to_vec(),
            &["index-header"],
        ),
        (
            "the index table in an HTML comment",
            wrap_index_table(index, "<!--", "-->"),
            adrs.to_vec(),
            &["index-header"],
        ),
    ]
}

/// The mutations of the real `index` and `adrs` that the index check must
/// report, each with exactly its codes.
fn adr_index_mutations(index: &str, adrs: &[AdrFile]) -> Vec<IndexMutation> {
    let row_0002 = index_row(index, "0002");
    let row_0003 = index_row(index, "0003");
    let row_0005 = index_row(index, "0005");
    let row_0021 = index_row(index, "0021");
    let last_row = last_index_row(index);
    let mut with_new_adr = adrs.to_vec();
    with_new_adr.push((
        "9998-a-new-decision.md".to_owned(),
        "# 9998. A new decision\n\n- Status: proposed\n".to_owned(),
    ));
    vec![
        (
            "the row of 0021 removed",
            delete_line(index, &row_0021),
            adrs.to_vec(),
            &["index-missing"],
        ),
        (
            "an ADR file without a row",
            index.to_owned(),
            with_new_adr,
            &["index-missing"],
        ),
        (
            "a row for a file that does not exist",
            insert_after(
                index,
                &last_row,
                0,
                &["| [9999](9999-not-an-adr.md) | Not an ADR | accepted |"],
            ),
            adrs.to_vec(),
            &["index-dangling"],
        ),
        (
            "0002 listed after 0003",
            insert_after(&delete_line(index, &row_0002), &row_0003, 0, &[&row_0002]),
            adrs.to_vec(),
            &["index-order"],
        ),
        (
            "the row of 0021 listed twice",
            insert_after(index, &row_0021, 0, &[&row_0021]),
            adrs.to_vec(),
            &["index-duplicate"],
        ),
        (
            "0010's link labelled 0011",
            replace_once(
                index,
                "[0010](0010-document-model.md)",
                "[0011](0010-document-model.md)",
            ),
            adrs.to_vec(),
            &["index-label"],
        ),
        (
            "0021's ADR cell not a link",
            replace_once(index, &format!("[0021]({ADR_0021})"), "0021"),
            adrs.to_vec(),
            &["index-missing", "index-row"],
        ),
        (
            "the index gives 0005 the status proposed",
            replace_line(
                index,
                &row_0005,
                &row_0005.replace("| accepted |", "| proposed |"),
            ),
            adrs.to_vec(),
            &["index-status"],
        ),
        (
            "ADR 0021 says proposed, the index accepted",
            index.to_owned(),
            adrs_with_status(adrs, ADR_0021, "- Status: proposed"),
            &["index-status"],
        ),
        (
            "ADR 0021 has no status line",
            index.to_owned(),
            adrs_with(adrs, ADR_0021, |text| {
                delete_line(text, &line_starting(text, "- Status:"))
            }),
            &["adr-status"],
        ),
        (
            "the index has no Status column",
            replace_once(index, "| ADR | Title | Status |", "| ADR | Title | State |"),
            adrs.to_vec(),
            &["index-header"],
        ),
    ]
}

#[test]
fn lib_doc_008_the_adr_index_lists_every_adr_with_its_status() {
    let index = read(ADR_INDEX);
    let adrs = adr_files();
    assert!(
        adrs.len() >= 21,
        "LIB-DOC-008: docs/adr/ must hold the ADR files; found {adrs:?}"
    );
    let problems = check_adr_index(&index, &adrs);
    assert!(
        problems.is_empty(),
        "LIB-DOC-008: {ADR_INDEX} must list every ADR, linked, in order, with its status: {problems:#?}"
    );

    let mut mutations = adr_index_mutations(&index, &adrs);
    mutations.extend(adr_index_status_and_rendering_mutations(&index, &adrs));
    for (name, index, adrs, codes) in &mutations {
        let problems = check_adr_index(index, adrs);
        assert_eq!(
            problem_codes(&problems),
            *codes,
            "LIB-DOC-008: the index check must report the mutation `{name}` as {codes:?}; it reported {problems:#?}"
        );
    }

    // Controls: the status is the first word, lower-cased, without punctuation.
    for status_line in [
        "- Status: Accepted, after review",
        "- Status: accepted (the maintainer, 2026-11-13)",
        "- Status: accepted. A longer note follows.",
    ] {
        let adrs = adrs_with_status(&adrs, ADR_0021, status_line);
        let problems = check_adr_index(&index, &adrs);
        assert!(
            problems.is_empty(),
            "LIB-DOC-008: `{status_line}` states the status `accepted`, but was reported: {problems:#?}"
        );
    }

    // Controls: the documented `superseded by NNNN`, stated alike in the ADR
    // and the index (the successor kept, a link giving its label).
    for (status_line, cell) in [
        ("- Status: superseded by 9997", "superseded by 9997"),
        (
            "- Status: Superseded by [9997](9997-a-successor.md).",
            "superseded by [9997](9997-a-successor.md)",
        ),
    ] {
        let (index, adrs) = with_superseded(&index, &adrs, status_line, cell);
        let problems = check_adr_index(&index, &adrs);
        assert!(
            problems.is_empty(),
            "LIB-DOC-008: `{status_line}` and the index cell `{cell}` state the same status, but were reported: {problems:#?}"
        );
    }

    // Control: the index is the first table that renders; a fenced table
    // before it, with another status, is not the index.
    let decoy = [
        "```text",
        INDEX_HEADER,
        "|---|---|---|",
        "| [0021](0021-xmi-reader-content.md) | Decoy | proposed |",
        "```",
        "",
        INDEX_HEADER,
    ]
    .map(str::to_owned);
    let with_decoy = edit_line(&index, INDEX_HEADER, |_| decoy.to_vec());
    let problems = check_adr_index(&with_decoy, &adrs);
    assert!(
        problems.is_empty(),
        "LIB-DOC-008: a fenced table before the index is not the index, but the check reported: {problems:#?}"
    );
}

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

/// LIB-DOC-008: every link of `text` (the file `doc`) that `adr_name` maps,
/// without its `#fragment`, to a file name in `docs/adr/` names a file of
/// `existing`. Fragments are not checked.
fn dangling_adr_links(
    doc: &str,
    text: &str,
    adr_name: fn(&str) -> Option<&str>,
    existing: &BTreeSet<String>,
) -> Vec<String> {
    link_destinations(text)
        .iter()
        .filter_map(|dest| {
            let path = dest.split_once('#').map_or(dest.as_str(), |(path, _)| path);
            adr_name(path)
                .filter(|name| !existing.contains(*name))
                .map(|_| {
                    problem(
                        "dangling-link",
                        format!("{doc} links `{dest}`, which is not a file in {ADR_DIR}/"),
                    )
                })
        })
        .collect()
}

/// LIB-DOC-008: every ADR file `NNNN-*.md` of `existing` is the destination
/// (without its `#fragment`) of a link that the index `text` renders, so an
/// index whose table does not render (a code block, an HTML comment) is
/// reported.
fn unlinked_adrs(text: &str, existing: &BTreeSet<String>) -> Vec<String> {
    let linked: BTreeSet<String> = link_destinations(text)
        .into_iter()
        .map(|dest| {
            dest.split_once('#')
                .map_or(dest.clone(), |(path, _)| path.to_owned())
        })
        .collect();
    existing
        .iter()
        .filter(|name| adr_number(name).is_some() && !linked.contains(*name))
        .map(|name| {
            problem(
                "index-unlinked",
                format!("{ADR_INDEX} renders no link to {name}"),
            )
        })
        .collect()
}

/// In `docs/sacm/decisions.md`: a link `../adr/<name>`.
fn decisions_adr_link(path: &str) -> Option<&str> {
    path.strip_prefix("../adr/")
}

/// In `docs/adr/README.md`: a link to an ADR file `NNNN-*.md`.
fn index_adr_link(path: &str) -> Option<&str> {
    adr_number(path).map(|_| path)
}

/// The mutations of the real ADR index `index` that the link check must
/// report, each with exactly its codes; `existing` are the files in
/// `docs/adr/`.
fn adr_index_link_mutations(
    index: &str,
    existing: &BTreeSet<String>,
) -> Vec<(&'static str, String, Vec<&'static str>)> {
    let adr_count = existing
        .iter()
        .filter(|name| adr_number(name).is_some())
        .count();
    let all_unlinked = vec!["index-unlinked"; adr_count];
    vec![
        (
            "a table link renamed",
            replace_once(index, "(0016-interchange-roots.md)", "(0016-roots.md)"),
            vec!["dangling-link", "index-unlinked"],
        ),
        (
            "a prose link to an ADR that does not exist",
            format!("{index}\nSee [ADR 9999](9999-not-an-adr.md).\n"),
            vec!["dangling-link"],
        ),
        (
            "the index table in a fenced code block",
            wrap_index_table(index, "```text", "```"),
            all_unlinked.clone(),
        ),
        (
            "the index table in an HTML comment",
            wrap_index_table(index, "<!--", "-->"),
            all_unlinked,
        ),
    ]
}

#[test]
fn lib_doc_008_every_adr_link_in_the_decisions_resolves() {
    let existing = adr_dir_names();
    let decisions = read(DECISIONS);
    let index = read(ADR_INDEX);
    let check_decisions =
        |text: &str| dangling_adr_links(DECISIONS, text, decisions_adr_link, &existing);
    let check_index = |text: &str| {
        let mut problems = dangling_adr_links(ADR_INDEX, text, index_adr_link, &existing);
        problems.extend(unlinked_adrs(text, &existing));
        problems
    };

    let in_decisions = link_destinations(&decisions)
        .iter()
        .filter(|dest| decisions_adr_link(dest).is_some())
        .count();
    assert!(
        in_decisions >= 5,
        "LIB-DOC-008: {DECISIONS} must point to the ADRs it rests on; found {in_decisions} links to ../adr/"
    );
    let problems = check_decisions(&decisions);
    assert!(
        problems.is_empty(),
        "LIB-DOC-008: every ../adr/ link in {DECISIONS} must name an existing file: {problems:#?}"
    );
    let problems = check_index(&index);
    assert!(
        problems.is_empty(),
        "LIB-DOC-008: every NNNN-*.md link in {ADR_INDEX} must name an existing file, and every ADR file must be linked: {problems:#?}"
    );

    let decisions_mutations = [
        (
            "an ADR link renamed",
            replace_once(
                &decisions,
                "../adr/0013-missing-name-stays-missing.md",
                "../adr/0013-missing-name.md",
            ),
        ),
        (
            "a link to an ADR that does not exist",
            format!("{decisions}\nSee [ADR 9999](../adr/9999-not-an-adr.md).\n"),
        ),
        (
            "a dangling link with a fragment",
            format!("{decisions}\nSee [ADR 9999](../adr/9999-not-an-adr.md#d1).\n"),
        ),
        (
            "a dangling reference link",
            format!("{decisions}\nSee [ADR 9999].\n\n[ADR 9999]: ../adr/9999-not-an-adr.md\n"),
        ),
    ];
    for (name, text) in &decisions_mutations {
        let problems = check_decisions(text);
        assert_eq!(
            problem_codes(&problems),
            ["dangling-link"],
            "LIB-DOC-008: the mutation `{name}` of {DECISIONS} must be reported once as `[dangling-link]`; it reported {problems:#?}"
        );
    }
    let index_mutations = adr_index_link_mutations(&index, &existing);
    for (name, text, codes) in &index_mutations {
        let problems = check_index(text);
        assert_eq!(
            problem_codes(&problems),
            *codes,
            "LIB-DOC-008: the mutation `{name}` of {ADR_INDEX} must be reported as {codes:?}; it reported {problems:#?}"
        );
    }

    // Controls: a fragment on an existing ADR, and text in a code span, which
    // is not a link.
    for (doc, problems) in [
        (
            DECISIONS,
            check_decisions(&format!(
                "{decisions}\nSee [ADR 0021](../adr/0021-xmi-reader-content.md#history).\n"
            )),
        ),
        (
            ADR_INDEX,
            check_index(&format!("{index}\nWrite `[9999](9999-not-an-adr.md)`.\n")),
        ),
    ] {
        assert!(
            problems.is_empty(),
            "LIB-DOC-008: a fragment on an existing ADR, or a code span in {doc}, is not a dangling link, but was reported: {problems:#?}"
        );
    }
}
