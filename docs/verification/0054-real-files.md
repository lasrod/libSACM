# Verification 0054 — Real files and a strict `sacm validate`

- Date: 2026-11-16
- Row verdict: LIB-COMPAT-015: PASS
- Row verdict: LIB-API-016: PASS
- Row verdict: LIB-CLI-002: PASS
- Verifier: `sacm-verifier` (independent; read-only), one round
- Commit: 76a2a23. Evidence: CI run
  [43876697250](https://github.com/lasrod/libSACM/actions/runs/43876697250), with all checks
  green. On Linux, the corpus step fetched the 15 files from the cache and passed the 4 corpus
  tests. PR-diff mutants: 18, all caught; 0 missed, 0 timed out, 0 unviable.
- Slice brief: [docs/slices/0054-real-files.md](../slices/0054-real-files.md)

## Result

- **The corpus.** `cargo xtask fetch-corpus` fetches 15 SACM files written by other tools,
  each pinned by SHA-256. The verifier re-hashed every file.
  - The corpus test skips visibly when the files are absent. It fails when the corpus is
    configured but a file is missing, and on any hash mismatch.
  - Its report is deterministic and equals the one in `docs/compat/corpus.md`.
  - Every refusal, Error and loss in the report maps to a row or a note.
  - Every repository listed is public.
- **`sacm validate`** reads strictly by default (ADR 0023 amendment 1).
  - A strict refusal of a file a tolerant read opens exits 1 on stdout.
  - A file neither mode opens exits 2 on stderr.
  - `--tolerant` never reports `valid`.
  - The real files behave the same way: kitchen-blender exits 1, mobstr and the ISO-8859-1
    file exit 2, and kitchen-blender with `--tolerant` is `readable`.
- **`ReadOutput::has_errors()`** agrees with `diagnostic::has_errors`. The verifier
  confirmed that only a strict read reports Errors.

Evidence beyond CI:
- hand mutations of the exit choice, the summary word, the re-read mode and the flag conflict,
  all killed. One is equivalent: dropping the strict-mode guard on the re-read, which is only
  an optimisation;
- the red snapshot compared with the final tree: step 2 only removed the gates and added one
  test;
- the SACM 2.3 §2.4 quotation checked against the PDF.

## Findings

| Severity | Row | Finding | Resolution |
|---|---|---|---|
| non-blocking | LIB-CLI-002 | The `--tolerant` help and the README said it opens "any well-formed file". | Reworded with the verifier's text: it lists what a tolerant read still refuses. |
| non-blocking | LIB-CLI-002 | The subcommand doc said "strictly" even with `--tolerant`, "0 when the file is valid", and left out I/O errors from exit 2. | Reworded. |
| non-blocking | LIB-CLI-002 | The README's `code: null` sentence left out a defect refusal, and one line was not reflowed. | Reworded and reflowed. |
| non-blocking | LIB-CLI-002 | `invalid` for content this version does not read yet could be read as "breaks SACM 2.3". | The README says it means the check could not confirm the file. |

## Logged

- The corpus steps run on all three operating systems, not on Linux only as the design said,
  because every step of the `test` job carries the standard condition (LIB-QA-018). All three
  pass.
