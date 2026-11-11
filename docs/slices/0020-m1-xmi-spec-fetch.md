---
slice: 0020
name: m1-xmi-spec-fetch
state: in-progress
step: verification
rows: [LIB-TOOL-020]
mode: pr
---

# Slice 0020 — XMI 2.5.1 is fetched with the SACM sources

## Scope

`cargo xtask fetch-spec` also fetches XMI 2.5.1 (formal/15-06-07), hash-pinned like the
two SACM sources, so the XMI rows (starting with SACM23-XMI-001) can quote it and the
verifier can check the quotes. It goes into the same git-ignored `third_party/sacm-2.3/`
directory. Moving or renaming that directory would change every agent and doc for no
gain. Out of scope: MOF and XML Schema texts, and testing the download against a local
HTTP server (LIB-TOOL-012).

## Basis

Tooling row: acceptance by the lead. ADR 0003 source priority (3) "XMI/MOF rules it
references". Verification 0016 and 0018 checked XMI quotes against an unpinned download.

## Design

No public library API. `xtask::fetch::ARTIFACTS` gains a third entry:

- file `XMI-2.5.1-formal-15-06-07.pdf`, from `https://www.omg.org/spec/XMI/2.5.1/PDF`;
- SHA-256 `2bf97c670f128affc9ed5dc202ef748101afd0eee8ee662e8763da6092c80c1b` (461,759
  bytes, PDF 1.5, title page "OMG Document Number: formal/2015-06-07"; two downloads on
  2026-11-11 gave the same hash).

## Log

- 2026-11-11 choose: LIB-TOOL-020 alone, before SACM23-XMI-001, which will quote XMI.
- 2026-11-11 tests: 3 tests (2 red, 1 guard); implemented; gate passes.
