---
slice: 0033
name: m1-no-xmi-version
state: in-progress
step: verification
rows: [SACM23-XMI-024, SACM23-XMI-003, SACM23-XMI-007, SACM23-XMI-013]
mode: pr
base: slice/0032-m1-adr-review-fixes (PR #43, merged); PR #44 targets main
---

# Slice 0033 — No xmi:version on export

## Scope

`xmi::write` stops writing `xmi:version="2.0"`. The XMI 2.5.1 namespace declaration
identifies the version. The reader keeps ignoring the attribute where it does today. The
golden files, the writer tests and the docs that state `xmi:version="2.0"` change with it.
Reading is unchanged.

## Basis

XMI 2.5.1 §7.5.3 and production 1a; ADR 0017 amendment 3 (the maintainer, 2026-11-13, on
the ADR review); D-XMI-VERSION.

## Log

- 2026-11-13 choose: the first M1 row after the ADR review. It changes the golden files
  that later writer slices build on, so it goes first.
