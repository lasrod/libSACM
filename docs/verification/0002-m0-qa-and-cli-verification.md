# Verification 0002: m0-qa-and-cli-verification

- Date: 2026-11-07
- Verdict: **FAIL**.
  - LIB-QA-002: FAIL
  - LIB-QA-003: FAIL
  - LIB-CLI-001: FAIL
- Verifier: `sacm-verifier` (independent; read-only)
- Inspected: HEAD `b86187b` (the base of `slice/0002-m0-qa-and-cli-verification`); the working tree differed only by the untracked slice brief.
- CI evidence: [run 42354157482](https://github.com/lasrod/libSACM/actions/runs/42354157482), 10/10 jobs succeeded. The verifier read the MSRV job log (rustc 1.85.1) and the cargo-deny job log.
- Slice brief: [docs/slices/0002-m0-qa-and-cli-verification.md](../slices/0002-m0-qa-and-cli-verification.md)

Today's configuration and behaviour are correct. The evidence is not: each row's regression tests let through breakages that the row's own requirement rules out. Every surviving mutation below was run in a scratch copy, not just reasoned about.

## Rows left open, with the complete prescription (fix round 1)

### LIB-QA-002 (implemented)

| Item | Required |
|---|---|
| Q2-1 | Inside the MSRV job block of `ci.yml`, a line whose trimmed text is **exactly** `- run: cargo +{msrv} test -p sacm -p sacm-cli` (a whole-line match, never `contains`). The block has no `if:` and no `continue-on-error:`. Optionally, the installer line `toolchain: "{msrv}"` too. Must kill: the step commented out; `--no-run`; `\|\| true`; any appended arguments; a renamed package; the job deleted with the command left in a comment elsewhere; `continue-on-error: true`; `if: false`. |
| Q2-2 | `crates/sacm` and `crates/sacm-cli` each have `package.rust-version = { workspace = true }`. Must kill: either member declaring a literal `rust-version`. |

**Not held to:** MSRV mentions in the README, CONTRIBUTING or CHANGELOG; `--all-features` in the MSRV job; xtask at the MSRV; the `toolchain:` installer value.

### LIB-QA-003 (implemented)

| Item | Required |
|---|---|
| Q3-1 | `licenses.allow` is a subset of an explicit permissive allowlist kept in the test. The **lead-approved set** is the current eight (MIT, Apache-2.0, Apache-2.0 WITH LLVM-exception, BSD-2-Clause, BSD-3-Clause, ISC, Unicode-3.0, Zlib) plus 0BSD, MIT-0, BSL-1.0, CC0-1.0, Unlicense and Unicode-DFS-2016. Keep the "MIT is allowed" check. Must kill these added licences: MPL-2.0, LGPL-2.0-or-later, GPL-1.0-or-later, EUPL-1.2, CC-BY-SA-4.0, SSPL-1.0, BUSL-1.1. |
| Q3-2 | `licenses.exceptions` is absent or empty, or every entry's `allow` lies within the Q3-1 set. `graph.exclude` is absent or empty. |
| Q3-3 | `sources.allow-git` is absent or empty. `sources.allow-org` is absent or has only empty lists. `sources.allow-registry` is absent or exactly `["https://github.com/rust-lang/crates.io-index"]`. |
| Q3-4 | `advisories.ignore` is absent or empty. The **lead's choice:** justified ignores are allowed, so every entry must be a table with a non-empty `reason`. `advisories.db-urls` is absent. `yanked = "deny"` stays. |
| Q3-5 | A `lib_qa_003_*` test reads `ci.yml`. It finds the job block containing an uncommented line whose trimmed text starts with `- uses: EmbarkStudios/cargo-deny-action@`, and requires: no `if:` and no `continue-on-error:`; `command`, if set, is exactly `check` with no check names; no `command-arguments`; no narrowing `arguments` (`--exclude`, `--exclude-dev`, workspace narrowing). Must kill: the step deleted; commented out; `command: check bans` or any other subset; `continue-on-error: true`; `if: false`. |

**Not held to:** each dependency's actual licence (cargo-deny checks that); `[bans]`; the `unmaintained`/`unsound` levels; `confidence-threshold`; `clarify`; `private.ignore`; pinning the action by SHA; branch protection.

### LIB-CLI-001 (implemented)

The requirement says a closed stdout pipe counts as success, but no criterion or test covers it. The behaviour works today. These mutations survive: C1, delete the `BrokenPipe` arm; C2, map it to `FAILURE`; C3, map every error to `SUCCESS` silently.

| Item | Required |
|---|---|
| C-1 (lead) | A criterion: "When stdout is a closed pipe, `sacm version` exits 0 and writes nothing to stderr; any other stdout write error exits non-zero and prints a `sacm:` message on stderr." **Added.** |
| C-2, C-3, C-4 | Tests that kill C1, C2, C3 and "message dropped, still FAILURE". The verifier proposed a factored mapping function with unit tests. The lead has asked whether Linux end-to-end tests (a closed Unix socket as stdout; `/dev/full` as stdout) are an acceptable alternative, since unit-testing a binary-only crate would need public API that exists only for the test. The verifier's answer is in the next section. |

**Not held to:** clap's own `--help`/`--version` pipe behaviour; the trailing newline; behaviour when the subcommand is missing; a Windows end-to-end closed-pipe test.

### LIB-CLI-001: the verifier's ruling on the Linux end-to-end alternative

**Accepted.** It replaces C-2/C-3 and also satisfies C-4; C-1 is still required, and has been added. The verifier noted a correction to the lead's premise: a binary crate can unit-test private functions from a `#[cfg(test)]` module in `main.rs`, so no public API would have been needed. It judges the end-to-end form stronger anyway, because it also kills these mutations: `main` bypassing the mapping, an unflushed `BufWriter` losing the error, and the message going to stdout. Conditions:

- **(a) Closed stdout.** Drop the peer of a `UnixStream::pair()` before spawning. Assert `status.success()` **and** empty stderr.
- **(b) `/dev/full` as stdout, opened for writing.** Assert all of: a non-zero exit, stderr starting with `sacm: `, and no `panicked`.
- **(c) Gate.** Both tests carry `#[cfg(target_os = "linux")]`, are named `lib_cli_001_*`, are cited, and `matrix check` passes.
- **(d) Imports.** cfg-gated platform imports live inside the gated function or module, so Windows clippy stays clean.
- **(e) MSRV.** Every API used must be ≤ 1.85; all are stable since 1.63.
- **(f) Evidence.** Both tests show `ok` in the Linux CI logs: the ubuntu test job and the MSRV job. With that, the C1, C2, C3 and "message dropped" kills are accepted as reasoned, because the verifier confirmed the EPIPE and ENOSPC mechanisms on Linux and the mapping has no platform branch.
- **(g) Windows.** Windows closed-pipe behaviour was checked once by hand (exit 0, os error 232). That probe is not a recorded test.

Consequence for LIB-QA-007: these kills only show on Linux, so mutation runs must happen on Linux.

## Follow-up rows created

- LIB-QA-008: CI gate checks match whole, uncommented step lines in jobs with no `if:` or `continue-on-error:`. This is a **new class** for LIB-QA-001, which is verified and is not reopened.
- LIB-DOC-007: the MSRV stated in the README badge and CONTRIBUTING matches `rust-version`.

## Mutation summary

| Survived (counts against a row) | Killed (controls) |
|---|---|
| Q2: step commented out; `--no-run \|\| true`; `continue-on-error`; member `rust-version` literal | `clippy.toml` drift |
| Q3: the 7 licences; `exceptions`; `graph.exclude`; `allow-git`, extra registry, `allow-org`; `advisories.ignore`; deny step commented out; `command: check bans` | GPL-3.0-only allowed; `yanked = "warn"` |
| CLI: C1, C2, C3 | version label changed |
