---
slice: 0036
name: m1-ids-docs-cli
state: in-progress
step: verification
rows: [SACM23-VAL-020, SACM23-VAL-001, LIB-DOC-001, LIB-CLI-002, LIB-CLI-003, SACM23-XMI-001, SACM23-XMI-016, SACM23-XMI-010, SACM23-XMI-002, SACM23-XMI-018, LIB-COMPAT-001, SACM23-XMI-004, SACM23-XMI-007, SACM23-XMI-013, SACM23-VAL-018, LIB-COMPAT-007]
mode: pr
base: main
---

# Slice 0036 — Ids, the crate example, and the CLI

## Scope

Five M1 rows, plus one priority follow-up:
- **SACM23-VAL-020:** an id that is not an NCName is a validation Error (SACM-ID-002), so a
  strict save writes only schema-conforming ids. An empty id read from a file gets its code.
- **SACM23-VAL-001:** a duplicate `xmi:id` is reported as SACM-ID-001, naming every element
  involved; the edit API already refuses duplicates (SACM-CMD-004).
- **LIB-DOC-001:** the crate root explains SACM plainly, links the matrix, and shows a
  runnable load → inspect → validate → edit → save doctest; the README mirrors it.
- **LIB-CLI-002:** `sacm validate <file>`, with `--strict` and `--format json`, and exit codes
  0, 1 or 2.
- **LIB-CLI-003:** `sacm roundtrip <file>`, which loads, saves strictly, reloads and reports
  semantic differences; `-o` writes the output.
- **The follow-up:** locating each read diagnostic must not rescan the source (Copilot, #46),
  so reads with many `xsi:type` stay linear.

## Basis

XMI 2.5.1 §7.6.1 (`xsd:ID`); D-ID; ADR 0019 E6, 0020 and 0022; SACM-ID-001 and SACM-ID-002.

## Design

ADR 0015 amendment 2 and ADR 0023, both accepted.
- **Ids.** `ReadError::DuplicateId { id, locations }` (SACM-ID-001, new `Code::ID_DUPLICATE`) and
  `ReadError::InvalidId { id, location }` (SACM-ID-002). A new validation rule reports
  non-NCName ids as SACM-ID-002. Padded ids are `Unsupported` until SACM23-XMI-025.
- **Locations.** Read diagnostics are located in one pass, with an advancing `Locator` in
  `xmi/input.rs` (the Copilot follow-up from #46).
- **The CLI.** `sacm validate` and `sacm roundtrip`, with exit codes 0, 1 and 2, and a JSON
  object versioned by `"format":1` and written by hand; serde_json is a dev-dependency only.
- **Docs.** The crate root gets the load → inspect → edit → validate → save example.

## Log

- 2026-11-13 choose: the remaining M1 rows that are ready, grouped per ADR 0006 amendment 3.
  SACM23-VAL-020 was moved to M1 by the maintainer.
- 2026-11-13 design: researcher and architect in parallel; ADR 0015 amendment 2 and ADR 0023
  accepted by the maintainer in one batch. New row SACM23-XMI-025 (padded ids, M2).
