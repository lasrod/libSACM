//! Repository automation for libSACM, run as `cargo xtask <command>`.
//!
//! This binary decodes its arguments, hands them to [`xtask::run`] with the
//! repository root and standard output, and maps the result to an exit
//! status. The root is the `XTASK_ROOT` environment variable when it is set,
//! and [`xtask::repo_root`] otherwise. Every command, argument form and
//! message lives in the `xtask` library; see its crate documentation.
// Reporting a failure on stderr is the binary's job.
#![allow(clippy::print_stderr)]

use std::io;
use std::path::PathBuf;
use std::process::ExitCode;

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
    let root = std::env::var_os("XTASK_ROOT").map_or_else(xtask::repo_root, PathBuf::from);
    match xtask::run(&args, &root, &mut io::stdout().lock()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("xtask: {message}");
            ExitCode::FAILURE
        }
    }
}
