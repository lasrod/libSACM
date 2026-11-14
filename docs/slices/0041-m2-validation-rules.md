---
slice: 0041
name: m2-validation-rules
state: in-progress
step: verification
rows: [SACM23-VAL-002, SACM23-VAL-011, SACM23-VAL-012, LIB-DIAG-002]
mode: pr
base: main
---

# Slice 0041 — Validation rules for gid, descriptions and package homogeneity

## Scope

Three validation rules on content the reader now holds, plus a catalog check:
- **SACM23-VAL-002:** two elements with the same non-empty `gid` (SACM-ID-003).
- **SACM23-VAL-011:** a ModelElement with more than one description (SACM-MULT-001,
  Warning; D-DESC-MULT).
- **SACM23-VAL-012:** an ArgumentPackage that nests a plain ArgumentPackage and also holds
  other argumentation elements (SACM-PKG-001; D-PKG-HOMOG).
- **LIB-DIAG-002:** every code the crate can emit is in `docs/sacm/diagnostics.md`, and
  every live code there exists in the crate; a test checks both directions.

## Basis

SACM 2.3 §8.2, §8.6, §11.4; D-SEV, D-DESC-MULT, D-PKG-HOMOG, D-VAL-LAYER; ADR 0018, ADR
0025.

## Design

No new public API is expected: `validate` gains findings, and `Code` gains constants for
SACM-ID-003 and SACM-PKG-001.

## Log

- 2026-11-14 choose: the validation rows that ADR 0025's content unblocks, and the catalog
  check.
- 2026-11-14 decide: duplicate gid is an Error (the maintainer); D-SEV gains the indicative rule; D-GID-EMPTY recorded; homogeneity checks plain owners only.
