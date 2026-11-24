# sacm-cli

Command-line tool for OMG SACM 2.3 assurance cases, built on the
[`sacm`](https://crates.io/crates/sacm) library.

> **Status: early development.** What the library reads and checks is tracked in the
> [functionality matrix](https://github.com/lasrod/libSACM/blob/main/docs/matrix/functionality-matrix.md).

```console
$ cargo install sacm-cli
$ sacm version
sacm 0.1.0 (SACM 2.3)
```

## Commands

### `sacm validate <file> [--strict | --tolerant] [--format human|json]`

Reads the file strictly, the default, validates it, and reports the read and validation
diagnostics together, sorted. A file is valid only when a strict read accepts it and no
diagnostic is an Error; a recognised variation such as `xsi:type` is an Error. `--strict`
names the default. Giving both flags is a usage error.

- A file that a strict read refuses but a tolerant read opens is `invalid` (exit 1). The
  refusal is printed on stdout, then
  `<file>: invalid (refused by a strict read; --tolerant opens it)`.
- A strict refusal of conforming content this version does not read yet (its message
  ends `(a limit of this version; …)`) is `invalid` too: the check could not confirm the
  file, which does not mean that the file breaks SACM 2.3.
- `--tolerant` reads in tolerant mode, which also opens a file that a strict read refuses
  for a bad value or for content this version does not read yet, and reports what a
  strict read refuses as Warnings. It still refuses a DOCTYPE, malformed XML, a root
  outside SACM or in SACM 2.4, an XMI element in an XMI namespace it does not read, a
  repeated `xmi:id`, an encoding it cannot decode and input over the read limits (exit 2).
  Its
  summary says `readable`, never `valid`, and `invalid` when a diagnostic is an Error.

```console
$ sacm validate case.sacm.xmi
error SACM-MULT-001 [SACM23-VAL-018, clause 8.6]: Claim "c1" has no name (name:LangString[1])
case.sacm.xmi: invalid (1 error, 0 warnings, 0 info)
```

`--format json` prints one object on one line:

```json
{"format":1,"file":"case.sacm.xmi","mode":"strict","result":"invalid","refusal":null,"diagnostics":[{"code":"SACM-MULT-001","severity":"error","requirement":"SACM23-VAL-018","clause":"8.6","location":null,"affected":["c1"],"message":"Claim \"c1\" has no name (name:LangString[1])"}]}
```

- `mode` is `strict` or `tolerant`.
- `result` is `valid` or `invalid` in strict mode, `readable` or `invalid` with
  `--tolerant`, and `unreadable` for a file neither a strict nor a tolerant read opens.
- Each diagnostic has the keys `code`, `severity` (`"error"`, `"warning"` or `"info"`),
  `requirement` (the matrix row), `clause`, `location`, `affected` (element ids) and
  `message`. `clause` is a clause number in the standard that the `requirement` row
  cites: SACM 2.3, or XMI 2.5.1 for SACM-ID-* and SACM-XMI-*.
- `refusal` is `null`, or `{"code","location","locations","message"}`: the strict
  refusal of a file only a tolerant read opens (with `"result":"invalid"` and
  `"diagnostics":[]`), or the refusal of an unreadable file, in the mode asked for.
  `location` is where the refusal is (for SACM-ID-001, the first repeat); `locations` is
  every place it names. `code` is `null` for an I/O error and for an `Unsupported`
  refusal: input this version cannot read yet, or a defect such as an attribute on
  `xmi:XMI` that is neither SACM 2.3 nor XMI 2.5.1. An I/O error also has
  `"location":null` and `"locations":[]`.
- Every key is always present, with `null` where a value is absent. A location is
  `{"line":L,"column":C}`.
- `"format":1` is the version of this shape. Keys may be added; a rename, a removal or a
  change of meaning moves it to 2. Codes are stable; message texts and the human format
  are not.

In JSON mode every result, including an unreadable file, is the JSON object on stdout,
and stderr is empty. Only a usage error (exit 2) goes to stderr, as text, with no JSON.

### `sacm roundtrip <file> [--strict] [-o <out>]`

Reads the file (tolerant by default), writes it in strict mode to memory, reads that
output back in strict mode, and compares the two documents. It prints
`<file>: round trip ok`, or each problem and then `<file>: round trip failed: …`.
Read diagnostics are not reported or checked; run `sacm validate` for them. `--strict`
changes only their severity, so in this release it does not change the result.
With `-o`, it writes the strict output to `<out>`, only when the round trip is faithful,
so `sacm roundtrip -o` converts a file to the strict form safely.

### `sacm convert <in> -o <out>`

Converts a readable SACM file to strict SACM 2.3, and refuses when that would lose
content. It reads `<in>` tolerantly, so it opens what a tolerant read opens, such as a file
in an older SACM revision's namespace or in the dialect EMF writes. It writes the document
in strict mode to memory, reads that back strictly, compares the two, and writes `<out>`
only when the strict save succeeds, the output reads back with no Error and nothing differs.

It prints every read Warning and Error first, one line each: what the conversion
normalised, or why it refuses. Then `<in>: converted to <out>`, or each problem and
`<in>: not converted: …`. Content that SACM 2.3 lacks, which a tolerant read keeps as
preserved content, is refused, not discarded. `-o` is required; there is no `--strict` and
no `--format`. `convert` reports no conformance; that is `validate`'s job.

```console
$ sacm convert emf-dialect.sacm.xmi -o brake.sacm.xmi
warning SACM-XMI-008 [LIB-COMPAT-004] at line 3, column 1: the AssuranceCasePackage …
warning SACM-XMI-015 [LIB-COMPAT-002, clause 7.5.3] at line 3, column 37: the name "xmi:version" …
…
warning SACM-XMI-017 [LIB-COMPAT-002, clause 7.10.2] at line 13, column 70: the document names …
emf-dialect.sacm.xmi: converted to brake.sacm.xmi
```

## Exit codes

| Code | Meaning |
| --- | --- |
| 0 | The check passed, or `convert` wrote its output. |
| 1 | The check ran and failed: for `validate`, an Error diagnostic, or a strict refusal of a file a tolerant read opens; for `roundtrip` and `convert`, a refused strict save, output that does not read back, or a difference (`convert` then neither creates nor changes `<out>`). |
| 2 | The tool could not do the job: a usage error, an I/O error such as an output that cannot be written, or input it cannot read: for `validate`, input neither a strict nor a tolerant read opens; for `roundtrip` and `convert`, input the reader refuses. |

stdout carries the result; stderr carries only what stopped the command, such as
`sacm: case.sacm.xmi: SACM-XML-001 the input is not well-formed XML at …`.

## License

MIT. See [LICENSE](https://github.com/lasrod/libSACM/blob/main/LICENSE).
