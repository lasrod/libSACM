# Verification 0020 — m1-xmi-spec-fetch

- Date: 2026-11-11
- Row verdict: LIB-TOOL-020: PASS
- Verifier: `sacm-verifier` (independent; read-only), one round (tooling row)
- Commit: `7c53e8f`. Evidence: ci.yml 42960595241, success (11 checks). The PR-diff mutants job found no mutants: the change is a constant entry and doc strings, and no test was removed or weakened.
- Slice brief: [docs/slices/0020-m1-xmi-spec-fetch.md](../slices/0020-m1-xmi-spec-fetch.md)

## Result

- **Item 1:** the XMI 2.5.1 entry is last, with the pinned file, URL and hash. The SACM entries are byte-for-byte unchanged, and two traced tests pin both.
- **Item 2:** `fetch_into` has no per-file path. A second run downloaded nothing, and a tampered XMI file was refused and left in place. The file is git-ignored, and git tracks no OMG file.
- **Item 3:** ADR 0003, CONTRIBUTING.md, the `fetch` module docs and the help text name XMI 2.5.1. A traced test checks the help.
- **Item 4:** a live `cargo xtask fetch-spec --dest target/verify-020` fetched all three files with their pins (`8004a2ee…3012`, `4bc12020…eb1e`, `2bf97c67…0c1b`). The XMI title page reads "OMG Document Number: formal/2015-06-07".

## Findings

| Severity | Row | Finding | Resolution |
|---|---|---|---|
| Low | LIB-TOOL-020 | README.md, docs/README.md and ADR 0003 Consequences still described the fetch as SACM-only. | Fixed at close. |
