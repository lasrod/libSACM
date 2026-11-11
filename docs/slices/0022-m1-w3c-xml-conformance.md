---
slice: 0022
name: m1-w3c-xml-conformance
state: done
step: verified
rows: [LIB-SEC-006]
mode: pr
---

# Slice 0022 — W3C XML conformance: not-well-formed input is refused

## Scope

Fetch the W3C XML Conformance Test Suite, hash-pinned and not committed. Run every
not-well-formed (XML 1.0) and namespace-not-well-formed case that the reader can meet
through `xmi::read`. Each one must be refused; any the parser accepts is refused by a new
check or listed with its reason. This closes the open class that slice 0021 left (ADR 0005
B1). The known first case is `xmlns:xmlns="urn:x"` (Namespaces 1.0 §3).

## Basis

XML 1.0 5th ed. §1.2; Namespaces in XML 1.0 3rd ed. §8; the W3C XML Conformance Test
Suite; ADR 0005 B1; slice 0021 verification.

## Design

No public library API. In `xtask::fetch`:

```rust
pub const XMLCONF: Artifact;                 // xmlts20130923.tar.gz, w3.org URL, SHA-256 pin
pub const XMLCONF_DEST: &str = "third_party/xmlconf";
pub fn fetch_suite(dest: &Path,
    download: &mut dyn FnMut(&str, &Path) -> Result<(), String>,
    extract: &mut dyn FnMut(&Path, &Path) -> Result<(), String>) -> Result<Outcome, String>;
pub fn tar_extract(archive: &Path, dir: &Path) -> Result<(), String>; // `tar -xzf <name>`, cwd = dir
```

`ARTIFACTS` is unchanged, because a verified LIB-TOOL-020 test pins it. `fetch-spec` fetches
the OMG sources, then the suite; `fetch-spec --only xmlconf` fetches only the suite.

The suite test (`crates/sacm/tests/w3c_xml_conformance.rs`) finds the suite at
`$SACM_XMLCONF_DIR`, or else at `<repo>/third_party/xmlconf/xmlconf`. cargo-mutants does not copy
git-ignored files, so CI sets the variable to an absolute path. It parses the catalogs with
roxmltree. The `xmlns:xmlns` check goes in `input.rs` `start_tag`, after `check_name` and before
the undeclaring check.

## Log

- 2026-11-11 choose: LIB-SEC-006, by the maintainer's decision in slice 0021.
- 2026-11-11 research: the suite 20130923 (SHA-256 9b61db9f...6fc1f) has 1,017 applicable
  not-wf cases. `read` refuses 1,016; roxmltree accepts only rmt-ns10-032
  (`xmlns:xmlns`). One-off evidence, not gated: the 719 DOCTYPE cases with the DOCTYPE
  stripped hide no body-level leniency, and the 31 UTF-16 OASIS cases transcoded to
  UTF-8 are all refused. The maintainer approved fetch-only use (2026-11-11). Lead
  decisions: fail under CI and skip loudly locally; `--only xmlconf`; exact counts;
  an over-strictness guard; "within the suite".
- 2026-11-11 tests and implementation: 28 tests; the `xmlns:xmlns` check; `fetch-spec --only xmlconf`; `fetch_suite_from` keeps the xtask tests offline; the local gate passes with the suite present. The CI fetch and cache wait for PR #29 (the same workflow files).
- 2026-11-11 CI: every job that runs the crate's tests fetches the suite (`--only xmlconf`), caches it by its hash, and sets `SACM_XMLCONF_DIR`.
- 2026-11-11 Copilot round: the SKIPPED notice is written to stderr directly; the over-strictness guard rejects panics; a failed extraction removes the marker (with a retry test); the input.rs unit tests run under a 5 s deadline (a timeout on `start_tag -> Ok(Some(0))`, the LIB-QA-012 residual).
- 2026-11-11 PR #30 merged before verification. Full mutants run 43031814423: 846 caught, 0 missed.
- 2026-11-11 verification round 1: FAIL, with 5 findings:
  - a case-insensitive check survives every test (`xmlns:XMLNS` is conforming);
  - `ran 1017 cases` was captured, so it never showed in CI logs;
  - the tar-naming assertion matched the temp directory's name, and the spawn-failure path was not tested;
  - re-extracting after a fresh download was not tested;
  - the docs overstated what LIB-SEC-005's sweep guards.

  Fixes go on branch `slice/0022-verification-fixes`.
- 2026-11-11 PR #31: the fixes (f79bc9f) and the Copilot fix (8aba433), which drops a temp-path check.
- 2026-11-11 verification round 2: PASS; LIB-SEC-006 verified.
