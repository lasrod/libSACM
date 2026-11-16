---
slice: 0054
name: real-files
state: done
step: done
rows: [LIB-COMPAT-015, LIB-CLI-002, LIB-API-016]
mode: pr
base: main
---

# Slice 0054 — Real files and a strict `sacm validate`

## Scope

- **Real files:** `cargo xtask fetch-corpus` fetches SACM files written by real tools, pinned
  by SHA-256 and never committed. A corpus test reads each file in both modes, writes it
  back tolerantly, and reports what it finds. Each refusal or loss it finds becomes a row,
  or is fixed.
- **`sacm validate` reads strictly by default** (the maintainer, 2026-11-16).
  - `--tolerant` opens any file, and its summary says "readable", not "valid".
  - A file passes only when a strict read accepts it and `validate` reports no Error.
- **`ReadOutput::has_errors()`.**

Left out: fixing what the corpus finds beyond small fixes; each larger gap becomes its own
row.

## Basis

- Quality and tooling rows: acceptance by the lead.
- For `validate`, D-STRICT: tolerant acceptance is never shown as conformance.
- SACM 2.3 clause 2 (conformance of XMI documents).

## Design

The design brief is `target/slice-0054-design.md` (git-ignored). It needs no new ADR, only
ADR 0023 amendment 1.
- **`ReadOutput::has_errors()`:** it delegates to `diagnostic::has_errors`.
- **`cargo xtask fetch-corpus`:** 15 files from 9 repositories and 6 tools, kept in
  `CORPUS` in `xtask/src/fetch.rs`.
- **The corpus test:** `xtask/tests/corpus.rs`. Its report goes in `docs/compat/corpus.md`.
- **CI:** the Linux test job fetches the corpus and runs the corpus test.
- **`sacm validate`:** strict by default, with `--tolerant`.
  - Exit 1 when only the strict read refuses the file.
  - Exit 2 when neither mode opens it.

## Log

- 2026-11-16 choose: LIB-COMPAT-015 first in the reordered M3. LIB-CLI-002 carries the
  maintainer's strict-default decision. LIB-API-016 is the read-side query that decision
  uses. No other ready row is related, so the slice has three rows.
- 2026-11-16 design: done by `sacm-designer`.
  - No public SACM 2.3 file comes from another tool. The 13 files that don't come from Assurance Forge are SACM 2.0 to 2.2 or EMF dialects, and they are refused today.
  - The maintainer keeps LIB-COMPAT-004 and LIB-COMPAT-002 in M6.
  - The lead adopted the designer's recommendations:
    - JSON `format` stays 1;
    - the corpus test runs in CI on Linux only;
    - files with no licence are fetch-only;
    - an exit-1 refusal does not print the tolerant read's diagnostics;
    - a candidate row, LIB-COMPAT-016 (ISO-8859-1);
    - the Assurance Forge writer's findings are notes in `corpus.md`.
  - LIB-CLI-002 keeps `verified` on its old record until this slice's record replaces it, because LIB-CLI-003 depends on it.
- 2026-11-16 tests: `sacm-builder` step 1 wrote 22 new or renamed tests, three fixtures and the goldens. Ten CLI tests and the usage test are red for the right reason, confirmed by the lead. The tests that use API not built yet are gated `cfg(slice_0054_api)`. Four CLI tests pass by design: they cover behaviour that is the same in both modes, or they check goldens against the library. The red snapshot is `red-0054.tar` in the session scratchpad. The lead's CI steps use the `test` job's standard condition (LIB-QA-018), so the corpus runs on all three operating systems.
- 2026-11-16 implementation: `sacm-builder` step 2 is done.
  - The step-1 tests pass unchanged. Cargo-mutants caught 16 of 16. One hand mutation survived, the JSON `mode` of a tolerant refusal, and it is now killed by a new test.
  - The lead added the ADR 0023 and ADR 0035 notes, the LIB-CLI-003 wording and the CHANGELOG flag line.
- 2026-11-16 verification: PASS for all three rows in round 1 (record: docs/verification/0054-real-files.md). The four wording findings were fixed directly with the verifier's text. Closed.
