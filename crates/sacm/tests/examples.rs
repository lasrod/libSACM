//! Tests of the example programs in `crates/sacm/examples/` (LIB-DOC-004).
//!
//! The examples are the four common tasks, each with a `//!` doc comment
//! saying what it shows, and each runs without arguments and exits 0. The
//! run test builds them with the cargo these tests run under.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The example programs, by name: read and validate a file; build and save a
/// case; edit and save in place; translate diagnostics with the catalog.
const EXAMPLES: [&str; 4] = [
    "read_and_validate",
    "build_and_save",
    "edit_in_place",
    "translate_diagnostics",
];

/// The directory of the `sacm` crate.
fn crate_dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// The workspace root, two levels above the crate.
fn workspace_root() -> &'static Path {
    crate_dir()
        .parent()
        .and_then(Path::parent)
        .expect("the sacm crate lies at crates/sacm under the workspace root")
}

/// The examples directory.
fn examples_dir() -> PathBuf {
    crate_dir().join("examples")
}

/// The example targets cargo finds in `dir`: each `<name>.rs`, and each
/// directory `<name>/` holding a `main.rs`. Empty if `dir` does not exist.
fn example_targets(dir: &Path) -> BTreeSet<String> {
    let Ok(entries) = fs::read_dir(dir) else {
        return BTreeSet::new();
    };
    entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            let name = path.file_name()?.to_str()?.to_owned();
            if path.is_dir() {
                path.join("main.rs").is_file().then_some(format!("{name}/"))
            } else {
                Path::new(&name)
                    .extension()
                    .is_some_and(|ext| ext == "rs")
                    .then_some(name)
            }
        })
        .collect()
}

#[test]
fn lib_doc_004_the_examples_are_the_four_common_tasks() {
    let dir = examples_dir();
    let found = example_targets(&dir);
    let expected: BTreeSet<String> = EXAMPLES.iter().map(|name| format!("{name}.rs")).collect();
    assert_eq!(
        found,
        expected,
        "LIB-DOC-004: {} must hold exactly the four example programs {expected:?}",
        dir.display()
    );

    let mut undocumented = Vec::new();
    for name in &expected {
        let path = dir.join(name);
        let text = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("LIB-DOC-004: read {}: {e}", path.display()));
        let first = text.lines().find(|line| !line.trim().is_empty());
        if !first.is_some_and(|line| line.trim_start().starts_with("//!")) {
            undocumented.push(name.clone());
        }
    }
    assert!(
        undocumented.is_empty(),
        "LIB-DOC-004: each example must start with a `//!` doc comment saying what it shows; \
         these do not: {undocumented:?}"
    );
}

#[test]
fn lib_doc_004_each_example_runs_without_arguments_and_exits_zero() {
    let mut problems = Vec::new();
    for name in EXAMPLES {
        let source = examples_dir().join(format!("{name}.rs"));
        if !source.is_file() {
            problems.push(format!("{name}: {} is missing", source.display()));
            continue;
        }
        let output = Command::new(env!("CARGO"))
            .current_dir(workspace_root())
            .args([
                "run",
                "--quiet",
                "--package",
                "sacm",
                "--all-features",
                "--example",
                name,
            ])
            .output();
        match output {
            Err(e) => problems.push(format!("{name}: cargo did not start: {e}")),
            Ok(output) if !output.status.success() => problems.push(format!(
                "{name}: exited with {}; stderr:\n{}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
            )),
            Ok(output) if output.stdout.iter().all(u8::is_ascii_whitespace) => {
                problems.push(format!(
                    "{name}: printed nothing on stdout; stderr:\n{}",
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
            Ok(_) => {}
        }
    }
    assert!(
        problems.is_empty(),
        "LIB-DOC-004: each example must run without arguments, print what it did and exit 0; \
         problems:\n{}",
        problems.join("\n")
    );
}
