//! Repository automation for libSACM, run as `cargo xtask <command>`.
//!
//! The binary in `src/main.rs` only parses arguments; everything it runs lives
//! in this library so that the acceptance tests in `xtask/tests/` exercise the
//! same code through a public API.
//!
//! - [`matrix`]: the functionality-matrix gate (`cargo xtask matrix check`,
//!   `render`, `next`, `status`).
//! - [`fetch`]: hash-pinned download of the OMG SACM 2.3 reference artifacts
//!   (`cargo xtask fetch-spec`).
//!
//! ```text
//! cargo xtask matrix check        Gate: validate the functionality matrix against the repo
//! cargo xtask matrix render       Regenerate docs/matrix/functionality-matrix.md
//! cargo xtask matrix next [-n N]  List the next ready rows for the slice loop
//! cargo xtask matrix status       Summary of row statuses per milestone
//! cargo xtask fetch-spec [--dest DIR]
//!                                 Download the pinned OMG SACM 2.3 reference artifacts
//! ```
//!
//! This crate is repository tooling, not part of the published library: it is
//! `publish = false` and its API serves the binary and its tests only.
// The `cmd_*` functions are the command-line front end; printing is their job.
#![allow(clippy::print_stdout, clippy::print_stderr)]

pub mod fetch;
pub mod matrix;

use std::path::{Path, PathBuf};

/// The repository root: the parent of the `xtask` crate directory.
///
/// Resolved from the crate's manifest directory at compile time, so it does
/// not depend on the current working directory.
///
/// # Panics
///
/// Never in this repository: the `xtask` crate always lives one level below
/// the repository root, so its manifest directory has a parent.
#[must_use]
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives one level below the repository root")
        .to_path_buf()
}
