# sacm-cli

Command-line tool for OMG SACM 2.3 assurance cases, built on the
[`sacm`](https://crates.io/crates/sacm) library.

> **Status: pre-1.0.** What the library reads and checks is tracked in the
> [functionality matrix](https://github.com/lasrod/libSACM/blob/main/docs/matrix/functionality-matrix.md).

```console
$ cargo install sacm-cli
$ sacm version
sacm 0.4.0 (SACM 2.3)
```

## Commands

| Command | What it does |
|---|---|
| `sacm version` | Prints the tool version and the SACM revision. |
| `sacm validate <file> [--strict \| --tolerant] [--format human\|json]` | Checks that a file conforms to SACM 2.3. |
| `sacm roundtrip <file> [--strict] [-o <out>]` | Checks that a file survives a strict save and reload unchanged. |
| `sacm convert <in> -o <out>` | Converts a readable file to strict SACM 2.3, or refuses if that would lose content. |

## Exit codes

| Code | Meaning | `validate` | `roundtrip` and `convert` |
|---|---|---|---|
| 0 | Passed | `valid`, or `readable` with `--tolerant` | The round trip is faithful; `convert` wrote `<out>`. |
| 1 | The check ran and failed | An Error diagnostic, or a strict refusal of a file a tolerant read opens | A refused strict save, output that does not read back, or a difference. `convert` neither creates nor changes `<out>`. |
| 2 | The tool could not do the job | A usage error, an I/O error, or input neither mode opens | A usage error, an I/O error such as an output that cannot be written, or input the reader refuses |

- **stdout** carries the result.
- **stderr** carries only what stopped the command, such as
  `sacm: case.sacm.xmi: SACM-XML-001 the input is not well-formed XML at …`.

## `sacm validate`

Reads the file, validates it, and reports the read and validation diagnostics together,
sorted.

```console
$ sacm validate case.sacm.xmi
error SACM-MULT-001 [SACM23-VAL-018, clause 8.6]: Claim "c1" has no name (name:LangString[1])
case.sacm.xmi: invalid (1 error, 0 warnings, 0 info)
```

| | Strict (the default; `--strict` names it) | `--tolerant` |
|---|---|---|
| Passes as | `valid`: a strict read accepts the file and no diagnostic is an Error | `readable`, never `valid` |
| Fails as | `invalid` | `invalid`, when a diagnostic is an Error |
| What a strict read refuses | `invalid` (exit 1) if a tolerant read opens the file | Opened, and reported as Warnings |

- **A recognised variation is an Error** in strict mode, such as `xsi:type`.
- **A strict refusal is printed on stdout,** then
  `<file>: invalid (refused by a strict read; --tolerant opens it)`.
- **A limit of this version is `invalid` too.** A refusal whose message ends
  `(a limit of this version)` means the check could not confirm the file, not that the
  file breaks SACM 2.3.
- **`--tolerant` still refuses** (exit 2) a DOCTYPE, malformed XML, a root outside SACM or
  in SACM 2.4, an XMI element in an XMI namespace it does not read, a repeated `xmi:id`,
  an encoding it cannot decode and input over the read limits.
- **Both flags together** are a usage error.

### JSON output

`--format json` prints one object on one line:

```json
{"format":1,"file":"case.sacm.xmi","mode":"strict","result":"invalid","refusal":null,"diagnostics":[{"code":"SACM-MULT-001","severity":"error","requirement":"SACM23-VAL-018","clause":"8.6","location":null,"affected":["c1"],"message":"Claim \"c1\" has no name (name:LangString[1])"}]}
```

| Key | Value |
|---|---|
| `format` | `1`, the version of this shape. |
| `file` | The file, as given on the command line. |
| `mode` | `"strict"` or `"tolerant"`. |
| `result` | `"valid"` or `"invalid"` in strict mode; `"readable"` or `"invalid"` with `--tolerant`; `"unreadable"` for a file neither mode opens. |
| `refusal` | `null`, or a refusal (below). |
| `diagnostics` | An array of diagnostics (below). |

A diagnostic:

| Key | Value |
|---|---|
| `code` | The stable code, such as `"SACM-MULT-001"`. |
| `severity` | `"error"`, `"warning"` or `"info"`. |
| `requirement` | The matrix row. |
| `clause` | A clause number in the standard the row cites: SACM 2.3, or XMI 2.5.1 for SACM-ID-* and SACM-XMI-*. |
| `location` | Where it is in the file. |
| `affected` | The element ids. |
| `message` | The text. |

A refusal is one of two things, each in the mode asked for:

- **The strict refusal of a file only a tolerant read opens,** with `"result":"invalid"`
  and `"diagnostics":[]`.
- **The refusal of an unreadable file.**

| Key | Value |
|---|---|
| `code` | The refusal's code; `null` for an I/O error. |
| `location` | Where the refusal is; for SACM-ID-001, the first repeat. `null` for an I/O error. |
| `locations` | Every place it names. `[]` for an I/O error. |
| `message` | The text. |

The contract (ADR 0023):

- **Every key is always present,** with `null` where a value is absent.
- **A location** is `{"line":L,"column":C}`.
- **Stable:** the keys and the codes. Keys may be added; a rename, a removal or a change
  of meaning moves `format` to 2.
- **Not stable:** message texts and the human format.
- **stdout only.** Every result, an unreadable file included, is the JSON object on
  stdout, and stderr is empty. Only a usage error (exit 2) goes to stderr, as text.

## `sacm roundtrip`

It prints `<file>: round trip ok`, or each problem and then
`<file>: round trip failed: …`.

1. Reads the file, in tolerant mode unless `--strict` is given.
2. Writes it in strict mode to memory.
3. Reads that output back in strict mode.
4. Compares the two documents.

- **Read diagnostics are not reported or checked.** Run `sacm validate` for them.
- **`--strict`** reads the input in strict mode; input a strict read refuses exits 2.
- **`-o <out>`** writes the strict output, only when the round trip is faithful. So
  `sacm roundtrip -o` converts a file to the strict form safely.

## `sacm convert`

Converts a readable SACM file to strict SACM 2.3, and refuses when that would lose content.

```console
$ sacm convert emf-dialect.sacm.xmi -o brake.sacm.xmi
warning SACM-XMI-008 [LIB-COMPAT-004] at line 3, column 1: the AssuranceCasePackage …
warning SACM-XMI-015 [LIB-COMPAT-002, clause 7.5.3] at line 3, column 37: the name "xmi:version" …
…
warning SACM-XMI-017 [LIB-COMPAT-002, clause 7.10.2] at line 13, column 70: the document names …
emf-dialect.sacm.xmi: converted to brake.sacm.xmi
```

- **Input.** Anything a tolerant read opens, such as a file in an older SACM revision's
  namespace or in the dialect EMF writes.
- **Output.** `<out>` is written only when the strict save succeeds, the output reads back
  with no Error and nothing differs.
- **Report.** Every read Warning and Error first, one line each: what the conversion
  normalised, or why it refuses. Then `<in>: converted to <out>`, or each problem and
  `<in>: not converted: …`.
- **Nothing discarded.** Content that SACM 2.3 lacks, which a tolerant read keeps as
  preserved content, is refused.
- **Options.** `-o` is required; there is no `--strict` and no `--format`.
- **No conformance verdict.** That is `validate`'s job.

## License

MIT. See [LICENSE](https://github.com/lasrod/libSACM/blob/main/LICENSE).
