# 0002. Cargo workspace: library, CLI, xtask

- Status: accepted
- Date: 2026-11-07

## Context

The deliverable is a reusable library. A command-line tool is needed early for smoke
testing, interop checks and users who are not writing Rust. Repository automation (the
matrix gate, fetching the normative sources) has to run identically on Windows, Linux
and macOS.

## Decision

- One workspace with three members: `crates/sacm` (library, published as `sacm`),
  `crates/sacm-cli` (binary `sacm`, published as `sacm-cli`), and `xtask` (automation,
  `publish = false`), invoked as `cargo xtask <command>`.
- The library is a **single crate** with internal modules (`model`, `xmi`, `validation`,
  `diagnostic`, `edit`, `compare`). It is not split into several published crates until
  there is a concrete user need (e.g. a model-only build without XML); a split later is
  possible behind the same public paths.
- Shared metadata, lints and dependency versions live in the workspace manifest.
- `default-members` excludes `xtask`, so `cargo build`/`cargo test` at the root cover
  the published crates; CI tests xtask explicitly.

## Consequences

Users depend on one crate. Automation needs only Rust and `curl` — no Python, Make or
shell-specific scripts. (The agent write-guard hook is Python, because Claude Code hooks
run outside cargo; it is optional for human contributors.)

## Alternatives considered

- **Several published crates from the start** (`sacm-model`, `sacm-xmi`, ...): more
  versioning and documentation overhead before the API is stable.
- **Scripts instead of xtask**: Python/shell differ across platforms; the C++
  predecessor needed Python for its gates.
