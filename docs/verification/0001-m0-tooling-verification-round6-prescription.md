# Verification 0001: LIB-TOOL-001 prescription for fix round 4

- Date: 2026-11-07
- Kind: **pre-verification prescription**, not a verdict. Written by `sacm-verifier` before any work started, at the maintainer's request. It is the complete list of what LIB-TOOL-001 must show to pass round 6.
- Inspected: HEAD `0ac00fc`, clean tree.
- Lead's decision on item 11: **option (b)**. The criterion wording was narrowed, and the follow-up row LIB-TOOL-011 was opened.

## Design (verifier-approved)

```rust
// xtask library
pub fn run(args: &[&str], root: &Path, out: &mut dyn std::io::Write) -> Result<(), String>;
// cmd_check / cmd_render / cmd_next / cmd_status / cmd_fetch take `out: &mut dyn std::io::Write`
// and turn write failures into Err (no panics, no ignored write results).
```

**`main`** may only:
- decode the arguments with `args_os`, refusing a non-Unicode argument with `xtask: argument is not valid Unicode: …`, and never converting lossily;
- call `xtask::run(&args, &repo_root(), &mut stdout.lock())`;
- map `Ok` to `SUCCESS`, and `Err(m)` to `eprintln!("xtask: {m}")` followed by `FAILURE`.

`main` names no command and matches on no arguments. `run` does not print the `xtask:` prefix and does not exit the process.

The usage text lives in the library, once. The current error messages are kept. Fix the `lib.rs`/`main.rs` docs that say the binary "only parses arguments". `run` needs a doc comment with `# Errors`. Per-problem lines from `cmd_check` may stay on stderr; that is LIB-TOOL-010's concern.

## Required items (from the verifier, verbatim in substance)

"Fixture tree" means a temporary tree like `consistent_tree` in `xtask/tests/matrix.rs`. Fixture source text stays on one line with `\n` escapes.

0. **Structure.** As above. This is checked by inspection.
1. **`matrix check` arm.**
   - On a consistent tree: `Ok`, and `out` contains `functionality matrix OK: 1 rows`.
   - With an uncited traced test planted under `crates/demo/tests/planted.rs`: exactly `Err("functionality matrix: 1 problem(s)")`, and `out` is empty.
   - The rendered page is byte-identical afterwards.
   - Kills R28e, R28f, R28g, R32 (arm runs status/next) and R33 (arm uses `repo_root()`).
2. **`matrix render` arm.**
   - Page pre-set to `"stale"`: `Ok`. The file equals `render(&load(toml))` byte for byte, `out` contains `wrote docs/matrix/functionality-matrix.md`, and a following `check` is `Ok`.
   - Empty tree: `Err` containing `functionality-matrix.toml`, and no page is created.
   - `RENDERED_PATH` is a directory: `Err` starting with `write `.
   - Kills R34–R39.
3. **`matrix next` arms.** The fixture rows, in file order:
   - an M1 open row X (ready);
   - an M0 verified row V;
   - an M0 open row B that depends on V (ready);
   - an M0 open row C that depends on B (blocked);
   - an M0 candidate row K;
   - an M0 open row D (ready).

   Assertions:
   - `next` lists exactly B, D, X in that order, with the header line spelled out by hand.
   - `-n 1` lists only B, and the header says `next 1`.
   - `--limit 2` lists B and D.
   - A tree with no ready rows: `Ok` with the `no ready rows` line.
   - Empty tree: `Err` containing `functionality-matrix.toml`.
   - `-n notanumber`: `Err("invalid row count: notanumber")`.
   - `["matrix","next","-n"]`: `Err` with `unrecognized arguments`.
   - `out` is empty in every `Err` case.
   - Kills R40–R47b.
4. **`matrix status` arm.**
   - Two-milestone fixture: `Ok`, with exactly 2 lines. Line *i* starts with milestone *i*'s id and contains its title. Afterwards no rendered page exists.
   - Empty tree: `Err` containing `functionality-matrix.toml`.
   - Kills R48–R52.
5. **`fetch-spec` arms (no network).**
   - Plant junk bytes G1 at `<tree>/third_party/sacm-2.3/<ARTIFACTS[0].file_name>` and junk bytes G2 at `<tree>/custom/<ARTIFACTS[0].file_name>`. The first artifact then fails on its wrong-hash check before `curl` is ever called; say so in a comment.
   - `run(["fetch-spec"])`: `Err` containing `exists but its SHA-256 is` and `sha256_hex(G1)`.
   - `run(["fetch-spec","--dest","custom"])`: `Err` containing `sha256_hex(G2)` and not `sha256_hex(G1)`.
   - Both planted files are byte-identical afterwards.
   - Kills R53–R57. Run the fetch mutants offline.
6. **`help`, unrecognized and near-miss arguments.**
   - `help`: `Ok`, and `out` starts with `usage: cargo xtask <command>`.
   - `["frobnicate","--now"]`: `Err` starting with `unrecognized arguments: frobnicate --now` and containing the usage.
   - `["matrix"]`, `["matrix","check","--extra"]` and `["fetch-spec","--dest"]`: each `Err` with `unrecognized arguments`.
   - `out` is empty in every `Err` case.
   - Kills R58, R58b, R58c.
7. **Write failures become errors.** With a writer whose every `write` fails: check (consistent tree), render, next, status and help each return `Err`, with no panic and no `Ok`. Kills R59 and R60.
8. **Arguments that are not valid Unicode.** A binary test: `OsStr::from_bytes(b"\xff")` under `#[cfg(unix)]`, `OsString::from_wide(&[0xD800])` under `#[cfg(windows)]`. Assert a non-zero exit, stderr starting with `xtask: `, and no `panicked`. Kills R61.
9. **Keep the three existing binary tests unchanged.** They cover R28, R28b–d and R28h–k. The help test also kills R62 (`main` passes `io::sink()`).
10. **Existing `cmd_check` tests.** If the signatures gain a writer, adapt the tests mechanically and keep every assertion. Changing `cmd_fetch` from `println!` to a writer is acceptable if all 9 LIB-TOOL-002 tests stay green; note it in the record. LIB-TOOL-002 is not reopened.
11. **Criterion 7 overclaim.** Resolved by (b): the wording now states that multi-line string literals and block comments are not recognised, and LIB-TOOL-011 tracks the scanner limits.

**PASS condition:**
- All gates are green.
- Items 1–9 are present and cited.
- Item 11 is resolved.
- In a fresh scratch copy (mtimes reset, fresh target directory), R28e/f/g and R32–R62 are all killed. The binary-level mutations (R28*, R61, R62) are killed by `cli.rs`.

## Out of scope (the verifier will not hold LIB-TOOL-001 to these)

**Belongs to other rows:**
- LIB-TOOL-010: problem lines (R30/R31) and problem order.
- LIB-TOOL-009: area-code format.
- LIB-TOOL-005: verdict parsing in verification records.
- LIB-TOOL-006: cited paths confined to the repository.
- LIB-TOOL-007: traced tests under `src/`.
- LIB-QA-001: how `ci.yml` invokes the gate.
- LIB-TOOL-002: `fetch_into` semantics, the fetch success labels, and the `curl` invocation.
- Cargo's handling of the alias exit code.

**Accepted Lows:**
- R23b.
- The `--help`, `-h` and no-argument forms of help.
- The default `next` limit.
- The exact counts printed by `status`.
- Whether `main` passes the right root.
- A failing stderr.
- An absolute or `..` path given to `--dest`.

**Different classes, noted for follow-up rather than held against this row:** `#[ignore]`/skipped tests counting as cited tests; `#[test_case]` attributes and Python `async def test_` not being recognised. All of these are in LIB-TOOL-011.

**Verifier's commitment:** in round 6, anything raised outside items 0–11 and the exclusions above is labelled **new class**, with the reason. A regression in criteria 1–9 caused by the refactor counts as in scope.
