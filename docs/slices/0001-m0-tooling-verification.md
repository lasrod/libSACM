---
slice: 0001
name: m0-tooling-verification
state: stopped
step: verification
rows: [LIB-TOOL-001, LIB-TOOL-002, LIB-TOOL-003, LIB-API-001, LIB-QA-001]
mode: local
---

# Slice 0001 — Verify the locally verifiable M0 foundation rows

## Scope

Independent verification of the foundation tooling and the first API row, all
implemented in the initial commit (`4b4a8e9`): the functionality-matrix gate, the
hash-pinned reference fetcher, the agent write-scope hook (including the live probe its
acceptance criteria require), and the version constants.

| Row | Title | Status at start |
|---|---|---|
| LIB-TOOL-001 | Functionality-matrix gate | implemented |
| LIB-TOOL-002 | Hash-pinned fetching of the normative SACM 2.3 sources | implemented |
| LIB-TOOL-003 | Agent write scopes are enforced by a hook | implemented |
| LIB-API-001 | Library and standard versions are exposed | implemented |

## Out of scope

- LIB-QA-001 (CI): its acceptance requires a green GitHub run, which requires pushing —
  not authorized for this slice. LIB-QA-002 and LIB-QA-003 depend on it.
- LIB-CLI-001: depends on LIB-API-001 being verified first; next slice.

## Normative basis

None (tooling and API rows; sources are ADRs 0003, 0004, 0006 and D-VERSION).
Research, design, test and implementation steps do not apply — the rows were
implemented test-first in the initial commit.

## Design

No change to the `sacm` public API.

### Fix round 1 (after verification 0001 FAIL), per ADR 0006 amendment 1

**xtask becomes a library plus a thin binary.** Its acceptance tests move from inline
`#[cfg(test)]` modules to `xtask/tests/`. `xtask/src/main.rs` only parses arguments and
calls `xtask::matrix::cmd_*` / `xtask::fetch::cmd_fetch`.

```rust
// xtask/src/lib.rs
pub mod fetch;
pub mod matrix;
pub fn repo_root() -> std::path::PathBuf;

// xtask::matrix — everything currently pub(crate) becomes pub:
pub const MATRIX_PATH: &str; pub const RENDERED_PATH: &str;
pub struct Matrix { pub schema_version, pub milestones, pub areas, pub rows }   // + Milestone, Area, Row
pub enum Prefix { Sacm23, Lib }  pub enum Status { .. }  pub enum Kind { .. }
impl Status { pub fn as_str(self) -> &'static str; }
pub fn load(text: &str) -> Result<Matrix, String>;
pub trait Repo { fn exists(&self, rel: &str) -> bool; fn read(&self, rel: &str) -> Option<String>;
                 fn traced_functions(&self) -> Vec<(String, String)>; }
pub struct FsRepo; impl FsRepo { pub fn new(root: &Path) -> Self; }   impl Repo for FsRepo
pub fn check(matrix: &Matrix, repo: &dyn Repo) -> Vec<String>;
pub fn render(matrix: &Matrix) -> String;
pub fn ready(matrix: &Matrix) -> Vec<&Row>;
pub fn traced_tests(text: &str, python: bool) -> Vec<String>;
pub fn cmd_check/cmd_render/cmd_status(root: &Path) -> Result<(), String>;
pub fn cmd_next(root: &Path, limit: usize) -> Result<(), String>;

// xtask::fetch
pub const DEFAULT_DEST: &str;
pub struct Artifact { pub file_name, pub url, pub sha256, pub what: &'static str }
pub const ARTIFACTS: &[Artifact];
#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub enum Outcome { AlreadyPresent, Fetched }
pub fn sha256_hex(bytes: &[u8]) -> String;   // lowercase hex
/// For each artifact: existing file with the pinned hash -> AlreadyPresent (download not called);
/// existing file with another hash -> Err, file left untouched; otherwise download(url, dest/<name>.part),
/// on download error or hash mismatch delete the .part and Err; on match rename into place -> Fetched.
pub fn fetch_into(dest: &Path, artifacts: &[Artifact],
                  download: &mut dyn FnMut(&str, &Path) -> Result<(), String>) -> Result<Vec<Outcome>, String>;
pub fn curl_download(url: &str, out: &Path) -> Result<(), String>;
pub fn cmd_fetch(dest: &Path) -> Result<(), String>;   // fetch_into(dest, ARTIFACTS, &mut curl_download) + output
```

Behaviour changes in the gate: a test citation must resolve to a function that
`traced_tests` recognises as a test (not merely `fn name` anywhere in the file), and
`traced_tests` recognises a test attribute on the same line as `fn`.

**Write guard (lead-implemented, maintainer-reviewed).** New pure functions in
`agent_write_guard.py` for the tests to target:

```python
def parse_scopes(text: str) -> dict | None
    # None unless: a JSON object whose "agents" is an object mapping each name to an object
    # with "allow": list[str] and optional "deny": list[str]. load_scopes() uses it.
def guarded_decide(payload: dict, scopes: dict | None, project_dir: str) -> dict | None
    # decide(), except that any exception denies when the call is a subagent write
    # (tool in WRITE_TOOLS and agent_type set); main() uses it.
```

`.claude/settings.json`: the hook command runs exactly one interpreter. No `a || b`
retry, which re-ran the guard with stdin already consumed.

## Tests

Existing: 14 `lib_tool_001_*` and 2 `lib_tool_002_*` xtask unit tests, 11
`test_lib_tool_003_*` hook tests, `lib_api_001_reports_library_and_sacm_versions` plus
doctests.

## Log

- 2026-11-07 preflight: initial structure approved and committed (`4b4a8e9`); references fetched.
- 2026-11-07 verification: live hook probes (test-engineer out-of-scope write denied by hook; read-only role had no Write tool). sacm-verifier: LIB-API-001 PASS → verified; LIB-TOOL-001/002/003 FAIL (test adequacy, hook fail-open paths) — see docs/verification/0001-m0-tooling-verification.md.
- 2026-11-07 stopped: fix round blocked — xtask/ and .claude/hooks/ are outside every agent write scope; maintainer decision needed on tooling ownership.
- 2026-11-07 resumed: ADR 0006 amendment 1 accepted (a74ca9c), main pushed for CI. Fix round 1: tests.
- 2026-11-07 tests: test engineer wrote 45 matrix + 8 fetch + 4 repo-check Rust tests and 18 new hook tests; red confirmed (xtask lib missing; parse_scopes/guarded_decide missing).
- 2026-11-07 implementation: implementer split xtask into lib + bin, citations must be tests, injectable fetch; lead implemented guard fail-closed changes and single-interpreter hook command. Full gate green.
- 2026-11-07 LIB-QA-001 added to the slice: first GitHub CI run green on a74ca9c (https://github.com/lasrod/libSACM/actions/runs/42338058640).
- 2026-11-07 verification round 2 (after fix round 1).
- 2026-11-07 verification round 2: LIB-TOOL-002 and LIB-QA-001 PASS -> verified; LIB-TOOL-001 and LIB-TOOL-003 FAIL a second time (untested FsRepo/cmd_check; fail-closed tests only use RuntimeError; live probe predates new hook command). See docs/verification/0001-m0-tooling-verification-round2.md.
- 2026-11-07 stopped: second FAIL (skill limit) and the guard diff awaits maintainer review; nothing committed since a74ca9c.
- 2026-11-07 maintainer approved the guard diff and a fix round 2. Fix round 1 committed.
- 2026-11-07 fix round 2: test engineer added 12 gate tests (filesystem scan, cmd_check, id/name/notes clauses) and 10 guard tests (every exception type, malformed payloads, real hook command). No production code changed.
- 2026-11-07 verification round 3: round-2 survivors all killed. LIB-TOOL-001 FAIL on a new Medium (binary exit code untested, mutation R28). LIB-TOOL-003 passes everything except the fresh-session live probe. See docs/verification/0001-m0-tooling-verification-round3.md.
- 2026-11-07 stopped. To resume in a NEW session: (1) live probe per the round-3 record's conditions, for LIB-TOOL-003; (2) with maintainer authorization, fix round 3: one lib_tool_001_ binary exit-code test (plus the Low one-liners R21/R23), then round-4 verification of LIB-TOOL-001.
