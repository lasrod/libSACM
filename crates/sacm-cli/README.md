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

### `sacm validate <file> [--strict] [--format human|json]`

Reads the file (tolerant by default), validates it, and reports the read and validation
diagnostics together, sorted. `--strict` reads in strict mode, where a recognised
variation such as `xsi:type` is an Error, not a Warning.

```console
$ sacm validate case.sacm.xmi
error SACM-MULT-001 [SACM23-VAL-018, clause 8.6]: Claim "c1" has no name (name:LangString[1])
case.sacm.xmi: invalid (1 error, 0 warnings, 0 info)
```

`--format json` prints one object on one line:

```json
{"format":1,"file":"case.sacm.xmi","mode":"tolerant","result":"invalid","refusal":null,"diagnostics":[{"code":"SACM-MULT-001","severity":"error","requirement":"SACM23-VAL-018","clause":"8.6","location":null,"affected":["c1"],"message":"Claim \"c1\" has no name (name:LangString[1])"}]}
```

- `result` is `valid`, `invalid` or `unreadable`.
- Each diagnostic has the keys `code`, `severity` (`"error"`, `"warning"` or `"info"`),
  `requirement` (the matrix row), `clause`, `location`, `affected` (element ids) and
  `message`. `clause` is a clause number in the standard that the `requirement` row
  cites: SACM 2.3, or XMI 2.5.1 for SACM-ID-* and SACM-XMI-*.
- `refusal` is `null`, or, for a file that cannot be read,
  `{"code","location","locations","message"}`. `location` is where the refusal is (for
  SACM-ID-001, the first repeat); `locations` is every place it names. `code` is `null`
  for an I/O error and for input this version cannot read yet (`Unsupported`). An I/O
  error also has `"location":null` and `"locations":[]`.
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

## Exit codes

| Code | Meaning |
| --- | --- |
| 0 | The check passed. |
| 1 | The check ran and failed: for `validate`, an Error diagnostic; for `roundtrip`, a refused strict save, output that does not read back, or a difference. |
| 2 | The tool could not do the job: a usage error, an I/O error, or input the reader refuses. |

stdout carries the result; stderr carries only what stopped the command, such as
`sacm: case.sacm.xmi: SACM-XML-001 the input is not well-formed XML at …`.

## License

MIT. See [LICENSE](https://github.com/lasrod/libSACM/blob/main/LICENSE).
