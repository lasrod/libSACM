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

Reads the file (tolerant by default; `--strict` reads in strict mode), validates it, and
reports the read and validation diagnostics together, sorted.

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
- `refusal` is `null`, or, for a file that cannot be read,
  `{"code","location","locations","message"}`. `code` is `null` for an I/O error.
- Every key is always present, with `null` where a value is absent. A location is
  `{"line":L,"column":C}`.
- `"format":1` is the version of this shape. Keys may be added; a rename, a removal or a
  change of meaning moves it to 2. Codes are stable; message texts and the human format
  are not.

### `sacm roundtrip <file> [--strict] [-o <out>]`

Reads the file (tolerant by default), writes it in strict mode to memory, reads that
output back in strict mode, and compares the two documents. It prints
`<file>: round trip ok`, or each problem and then `<file>: round trip failed: …`.
With `-o`, it writes the strict output to `<out>`, only when the round trip is faithful,
so `sacm roundtrip -o` converts a file to the strict form safely.

## Exit codes

| Code | Meaning |
| --- | --- |
| 0 | The check passed. |
| 1 | The check ran and failed: an Error diagnostic, a refused strict save, or a round-trip difference. |
| 2 | The tool could not do the job: a usage error, an I/O error, or input the reader refuses. |

stdout carries the result; stderr carries only what stopped the command, such as
`sacm: case.sacm.xmi: SACM-XML-001 the input is not well-formed XML at …`.

## License

MIT. See [LICENSE](https://github.com/lasrod/libSACM/blob/main/LICENSE).
