//! Repository automation for libSACM, run as `cargo xtask <command>`.
//!
//! ```text
//! cargo xtask matrix check        Gate: validate the functionality matrix against the repo
//! cargo xtask matrix render       Regenerate docs/matrix/functionality-matrix.md
//! cargo xtask matrix next [-n N]  List the next ready rows for the slice loop
//! cargo xtask matrix status       Summary of row statuses per milestone
//! cargo xtask fetch-spec [--dest DIR]
//!                                 Download the pinned OMG SACM 2.3 reference artifacts
//! ```
#![allow(clippy::print_stdout, clippy::print_stderr)]

mod fetch;
mod matrix;
#[cfg(test)]
mod repo_checks;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str = "\
usage: cargo xtask <command>

commands:
  matrix check            validate the functionality matrix against the repository
  matrix render           regenerate docs/matrix/functionality-matrix.md
  matrix next [-n N]      list the next N ready rows (default 10)
  matrix status           summarize row statuses per milestone
  fetch-spec [--dest DIR] download the pinned OMG SACM 2.3 reference artifacts
                          (default DIR: third_party/sacm-2.3)";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let root = repo_root();
    let result = match args.as_slice() {
        ["matrix", "check"] => matrix::cmd_check(&root),
        ["matrix", "render"] => matrix::cmd_render(&root),
        ["matrix", "next"] => matrix::cmd_next(&root, 10),
        ["matrix", "next", "-n" | "--limit", n] => match n.parse() {
            Ok(n) => matrix::cmd_next(&root, n),
            Err(_) => Err(format!("invalid row count: {n}")),
        },
        ["matrix", "status"] => matrix::cmd_status(&root),
        ["fetch-spec"] => fetch::cmd_fetch(&root.join(fetch::DEFAULT_DEST)),
        ["fetch-spec", "--dest", dest] => fetch::cmd_fetch(&root.join(dest)),
        ["help" | "--help" | "-h"] | [] => {
            println!("{USAGE}");
            Ok(())
        }
        _ => Err(format!(
            "unrecognized arguments: {}\n\n{USAGE}",
            args.join(" ")
        )),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("xtask: {message}");
            ExitCode::FAILURE
        }
    }
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives one level below the repository root")
        .to_path_buf()
}
