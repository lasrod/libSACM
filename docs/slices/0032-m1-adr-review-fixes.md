---
slice: 0032
name: m1-adr-review-fixes
state: done
step: verified
rows: [LIB-DOC-008]
mode: pr
base: slice/0031-m1-semantic-comparison (PR #42)
---

# Slice 0032 — ADR review fixes

## Scope

The documentation findings of the 2026-11-13 ADR review
([published report](https://claude.ai/artifact/CnpW7Q9zbDbNS9HNXGxQQX)). The maintainer asked
for every recommendation to be applied. The easy ones are done here: the ADRs and
decisions describe the current design. The behaviour findings are new matrix rows, which the
loop takes in turn:
- SACM23-XMI-024 (no `xmi:version` on export, M1);
- LIB-API-011 (interim refusals listed and gated, M2);
- SACM23-VAL-020 (an `xmi:id` is an NCName, M2);
- LIB-DIAG-003 (users can build diagnostics, M7).

There are also notes on SACM23-XMI-020, LIB-COMPAT-001/007 and SACM23-VAL-006, and new
dependencies for the compliance points and LIB-QA-005.

## Basis

Quality row: acceptance by the lead. ADR 0001; CLAUDE.md rule 6.

## Log

- 2026-11-13 choose: the maintainer's request to apply every recommendation of the ADR
  review, with the easy ones as one task and the rest as matrix rows.
- 2026-11-13 verification: round 1 FAIL (seven Medium findings, all the lead's: the two
  meanings of `Unsupported`, the interim list, lost C++ differences, ADR 0011, the
  compliance-point links, D- entries). Fixed in 943780e. Round 2 PASS (record 0032).
