//! Temporary repository trees shared by the `xtask` integration tests
//! (LIB-TOOL-001).
//!
//! Used by `matrix.rs` (the gate on a real tree) and `run.rs` (the command
//! dispatcher on a real tree).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use xtask::matrix::{MATRIX_PATH, RENDERED_PATH, load, render};

/// A fresh directory tree under the system temporary directory, unique per
/// test and removed on drop.
#[derive(Debug)]
pub(crate) struct TempTree(PathBuf);

impl TempTree {
    pub(crate) fn new(test: &str) -> Self {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "xtask-matrix-{test}-{}-{nanos}-{n}",
            std::process::id()
        ));
        fs::create_dir_all(&path)
            .unwrap_or_else(|e| panic!("LIB-TOOL-001: create {}: {e}", path.display()));
        Self(path)
    }

    pub(crate) fn root(&self) -> &Path {
        &self.0
    }

    /// Write `text` at the `/`-separated relative path `rel`, creating its
    /// parent directories.
    pub(crate) fn write(&self, rel: &str, text: &str) -> &Self {
        let path = self.0.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .unwrap_or_else(|e| panic!("LIB-TOOL-001: create {}: {e}", parent.display()));
        }
        fs::write(&path, text)
            .unwrap_or_else(|e| panic!("LIB-TOOL-001: write {}: {e}", path.display()));
        self
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// A matrix with one row citing a Rust test under `crates/` and a Python test
/// under `.claude/hooks/`.
pub(crate) const GATE_TOML: &str = r#"
schema_version = 1
[[milestone]]
id = "M0"
title = "Foundation"
goal = "Build."
[[area]]
code = "TOOL"
prefix = "LIB"
title = "Tooling"
[[row]]
id = "LIB-TOOL-001"
area = "TOOL"
milestone = "M0"
title = "Gate"
requirement = "The gate holds."
source = "ADR 0004"
kind = "tooling"
status = "tests-failing"
acceptance = ["a"]
tests = [
  "crates/demo/tests/gate.rs::lib_tool_001_cited",
  ".claude/hooks/test_gate.py::test_lib_tool_001_cited",
]
"#;

/// The smallest tree on which the gate passes: the matrix, its rendered page
/// and the cited test files.
pub(crate) fn consistent_tree(test: &str) -> TempTree {
    let matrix = load(GATE_TOML).expect("LIB-TOOL-001: the gate fixture parses");
    let tree = TempTree::new(test);
    tree.write(MATRIX_PATH, GATE_TOML)
        .write(RENDERED_PATH, &render(&matrix))
        .write(
            "crates/demo/tests/gate.rs",
            "#[test]\nfn lib_tool_001_cited() {}\n",
        )
        .write(
            ".claude/hooks/test_gate.py",
            "def test_lib_tool_001_cited(self):\n    pass\n",
        );
    tree
}
