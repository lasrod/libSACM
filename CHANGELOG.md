# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html). Before 1.0, a breaking
change bumps the minor version and is listed under **Breaking**.

## [Unreleased]

### Added

- Cargo workspace: `sacm` library, `sacm-cli` (`sacm version`), `xtask` automation.
- `sacm::VERSION` and `sacm::SACM_VERSION`.
- Functionality matrix with its gate (`cargo xtask matrix check|render|next|status`).
- Hash-pinned fetching of the OMG SACM 2.3 references (`cargo xtask fetch-spec`).
- CI: rustfmt, clippy, tests on Linux/Windows/macOS, MSRV 1.85, rustdoc, cargo-deny,
  matrix gate, agent-hook tests.
- Agent roles, skills and write-scope hook for the engineering loop.
- CI hardening: third-party actions are pinned by commit SHA, with a reviewed table of
  the inputs each may receive (`docs/quality/ci-actions.md`). The CI gate tests read
  the parsed workflow, so a gate that is commented out, conditional, allowed to fail,
  or weakened by its environment, OS list, triggers or other steps fails them.
- Mutation testing with cargo-mutants: a PR job mutates the diff, and a weekly sharded
  full run also runs on every Rust PR. Configuration is in `.cargo/mutants.toml`, and
  every survivor's disposition is in `docs/quality/mutants-triage.md`.
- Mutation exclusions checked against cargo-mutants' own listings
  (`cargo xtask mutants exclusions`, and the CI job `mutants-exclusions`). Each
  `exclude_re`, `exclude_globs` and `skip_calls` entry must exclude exactly the mutants
  the triage document assigns to it.
- Mutation testing runs the tests under cargo-nextest, with a `mutants` profile
  (`.config/nextest.toml`) that kills every running test at the first failure (ADR
  0009). Whether a mutant is caught no longer depends on the order of test-target
  names. A local mutants run needs cargo-nextest 0.9.111 or later. Doctests do not run
  under mutation; they still run in the normal gate.
- A stricter matrix gate. A verified row needs exactly one `- Row verdict: <ROW-ID>: PASS`
  line in its verification record. Cited paths must be relative paths inside the
  repository, and area codes must be uppercase ASCII letters. A failing
  `matrix check` returns every problem in its error, one per line, in a deterministic
  order. `XTASK_ROOT` points the `xtask` binary at another tree.
- Clearer matrix reporting. Under `XTASK_ROOT`, `matrix check` names the root it
  checked: the first line of its report ends with ` (root: <root>)`. An `XTASK_ROOT`
  that is set but empty is an error. An error from reading or parsing the matrix is one
  line, `docs/matrix/functionality-matrix.toml:<line>:<column>: <message>` or
  `read <path>: <error>`, with control characters escaped.
- A precise traced-test scanner for the matrix gate.
  - It skips Rust and Python comments and literals: block comments that nest, raw
    strings, char literals, and triple-quoted and prefixed Python strings. Test-like
    text inside them is not a test.
  - It recognises `#[test_case]`, `#[rstest]` and other attributes whose path ends in
    `test`, and Python `async def test_`.
  - It reads names as tokens, as Rust and Python do, and judges a whole attribute path
    or decorator name. So `# [test]`, `#[tokio :: test]` (also with comments around
    the `::` or split across lines), `@ unittest . skip`, and a tab or several spaces
    after `fn`, `def`, `async def` or `class` are the plain forms. `#[test :: helper]`
    and `@unittest.skip .helper` are not, and neither are `self.fn` and `r#fn`.
  - An ignored or skipped traced test (`#[ignore]`, `#[cfg_attr(<pred>, ignore)]`, or
    a Python skip decorator on the test or on its class) is not evidence. The gate
    reports it.
  - The gate also reports a traced test under a `src` directory.
  - Only build output is skipped: a `target` directory with a `Cargo.toml` directly
    beside it. Any other `target` directory, such as a `src/target/` module, is
    scanned.
  - A `path::function` test citation must name a scanned test file: a `.rs` or `.py`
    file under `crates/`, `xtask/` or `.claude/hooks/`, outside build output. Any other
    file is the row problem `test file <path> is outside the scanned test files`.
    `doctest:` citations may still name any file.
  - API: `xtask::matrix::ignored_traced_tests`, and the required trait method
    `Repo::ignored_functions`.
- The matrix gate decides which `- Row verdict:` lines of a verification record count
  with a CommonMark parser, `pulldown-cmark` 0.13 (ADR 0008).
  - It parses the record as core CommonMark 0.31.2, with no extensions.
  - A line inside a code block (fenced or indented) or inside an HTML block that is one
    closed comment does not count, and it is not malformed. Nothing else hides a line.
    The parser decides fences inside list items and block quotes, and a backtick fence
    "opener" whose info string holds a backtick.
  - The hand-written fence model is removed. The verdict format is unchanged.
- Verification records are strict (ADR 0008 amendment 1). The gate reports anything a
  renderer could show differently, rather than modelling it:
  - raw HTML other than a closed comment and the inline tags `<br>`, `<br/>`, `<br />`,
    `<ul>`, `</ul>`, `<li>`, `</li>` (in any case, and only after other text on their
    line), once per line;
  - a visible verdict-like line that is not at column 0, such as an indented, nested or
    block-quoted `- Row verdict:` item, or a `*`, `+` or `1.` item;
  - a lone carriage return, once per line, and a leading byte-order mark.
  - A `- Row verdict:` line whose id is not a well-formed row id is now malformed.
  - Problems come in a fixed order: the byte-order mark, then by line (raw HTML, lone
    CR, malformed or verdict-like), then the verdict problem, once per citing row.
