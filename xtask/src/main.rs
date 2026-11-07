//! Repository automation for libSACM, run as `cargo xtask <command>`.
//!
//! This binary decodes its arguments, hands them to [`xtask::run_from`] with
//! the repository root, where that root came from and standard output, and
//! maps the result to an exit status. The root is the `XTASK_ROOT`
//! environment variable when it is set ([`xtask::RootSource::Environment`]),
//! and [`xtask::repo_root`] otherwise ([`xtask::RootSource::Build`]). An
//! `XTASK_ROOT` that is set but empty is an error: the binary prints
//! `xtask: XTASK_ROOT is set but empty` on stderr, runs no command and fails.
//! Every command, argument form and message lives in the `xtask` library; see
//! its crate documentation.
// Reporting a failure on stderr is the binary's job.
#![allow(clippy::print_stderr)]

use std::io;
use std::path::PathBuf;
use std::process::ExitCode;

use xtask::RootSource;

fn main() -> ExitCode {
    let mut args = Vec::new();
    for arg in std::env::args_os().skip(1) {
        match arg.into_string() {
            Ok(arg) => args.push(arg),
            Err(raw) => {
                // The lossy form is only shown, never used as an argument.
                eprintln!(
                    "xtask: argument is not valid Unicode: {}",
                    raw.to_string_lossy()
                );
                return ExitCode::FAILURE;
            }
        }
    }
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let (root, source) = match std::env::var_os("XTASK_ROOT") {
        None => (xtask::repo_root(), RootSource::Build),
        Some(root) if root.is_empty() => {
            // An empty root would silently mean the current directory.
            eprintln!("xtask: XTASK_ROOT is set but empty");
            return ExitCode::FAILURE;
        }
        Some(root) => (PathBuf::from(root), RootSource::Environment),
    };
    match xtask::run_from(&args, &root, source, &mut io::stdout().lock()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("xtask: {message}");
            ExitCode::FAILURE
        }
    }
}
