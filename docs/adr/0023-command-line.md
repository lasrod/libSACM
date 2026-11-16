# 0023. The `sacm` command line

- Status: accepted (the maintainer, 2026-11-13). Amended the same day (the JSON-mode stream
  rule, and `roundtrip --strict`), and the amendment confirmed by the maintainer.
- Date: 2026-11-13
- Matrix rows: LIB-CLI-002, LIB-CLI-003

## Context

`sacm` gives scripts and CI a way to check SACM files. The assurance-forge C++ library has
`sacm_cli` with `validate` and `roundtrip` (`libs/sacm/tools/sacm_cli.cpp`).

## Decision

- **Exit codes.**
  - 0: the check passed.
  - 1: the check ran and failed: an Error diagnostic, a refused strict save, or a round-trip
    difference.
  - 2: the tool could not do the job: a usage error, an I/O error, or input that `read`
    refuses. Amendment 1 narrows this for `validate` to input that neither mode reads.
- **Streams.** stdout carries the result. stderr carries only what stopped the command.
- **`sacm validate <file> [--strict] [--format human|json]`.**
  - It reads the file (tolerant by default; since amendment 1, strictly by default, and
    tolerantly with `--tolerant`) and runs `validation::validate`.
  - It reports the read and validation diagnostics together, sorted.
  - Human format: one `Display` line per diagnostic, then
    `<file>: valid|invalid (E errors, W warnings, I info)`.
- **`--format json`.** One object per run, ending with a newline:
  `{"format":1,"file":…,"mode":…,"result":"valid"|"invalid"|"unreadable","refusal":…,"diagnostics":[…]}`.
  - Each diagnostic is `{code, severity, requirement, clause, location, affected, message}`,
    with every key always present and `null` where a value is absent.
  - Strings are escaped per RFC 8259.
  - `"format": 1` is the version. Keys may be added. A rename, a removal or a change of
    meaning moves it to 2.
  - Codes are stable; message texts and the human format are not.
  - Shapes: a location is `{"line":L,"column":C}` or `null`. The refusal is
    `{"code","location","locations","message"}`, with `message` the `ReadError` `Display`
    and `code` `null` when the refusal has none. `mode` is `"tolerant"` or `"strict"`, and
    `diagnostics` is `[]` when the file is unreadable. The output is compact: one line, then
    a newline. Escapes use the short forms for LF, CR, TAB, BS and FF, and lowercase
    `\u00XX` otherwise.
  - In JSON mode every result, including an unreadable file, is the JSON object on stdout,
    and stderr is empty. Only a usage error (exit 2) goes to stderr, as text, with no JSON.
  - `roundtrip` does not check read diagnostics (that is `validate`'s job), so its `--strict`
    changes only their severity and, in this release, not the result.
- **JSON writer.** It is hand-written in the CLI (about 30 lines). There is no serde
  dependency at run time; `serde_json` is a dev-dependency only, for tests.
- **`sacm roundtrip <file> [--strict] [-o <out>]`.**
  - Steps: it reads the file (tolerant by default), writes it strictly to memory, reads that
    output back strictly, and runs `compare::semantic_compare`.
  - Output: each `Difference` on one line, then `<file>: round trip ok` or
    `<file>: round trip failed: …`.
  - `-o` writes the strict output only when the round trip is faithful, so
    `roundtrip -o` is a safe converter to the strict form.

**Differences from C++, one line each:**
- The flag is `--format json`, not `--json`.
- The JSON is a versioned object, not a bare array, and adds `clause`, `location` and
  `affected`.
- An unreadable file exits 2; C++ exits 1.
- C++'s JSON escaper misses most control characters; this one follows RFC 8259.
- `roundtrip` does not fail on read diagnostics (that is `validate`'s job) and has no
  `--compat`; C++ fails on load Errors.

## Consequences

- Scripts can tell "invalid" (1) from "could not check" (2).
- A later JSON format moves `"format"` to 2.

## Amendment 1 (the maintainer, 2026-11-16; LIB-CLI-002, slice 0054)

`sacm validate` is a conformance check, so it reads strictly by default (D-STRICT:
tolerant acceptance is never presented as conformance).

- **Flags.** `--tolerant` reads in tolerant mode. `--strict` stays and names the default.
  Giving both is a usage error.
- **Pass.** A file passes (exit 0) only when a strict read accepts it and no read or
  validation diagnostic is an Error.
- **A strict refusal of a file that a tolerant read opens** is a failed check:
  - exit 1, result `invalid`;
  - human output, on stdout: the refusal as `<file>: <code|error> <ReadError>`, then
    `<file>: invalid (refused by a strict read; --tolerant opens it)`;
  - JSON: the refusal object, with `result:"invalid"` and `diagnostics:[]`.
- **Exit 2** stays for usage errors, I/O errors, and input that neither mode reads. The CLI
  decides by reading the same bytes tolerantly.
- **With `--tolerant`,** a file with no Error is `readable`, never `valid`.
- **JSON `"format"` stays 1,** because no release has shipped format 1.
- **`roundtrip`** keeps its tolerant default: it converts a file to the strict form and
  does not report conformance.
