---
slice: 0035
name: m1-load-and-save-modes
state: in-progress
step: verification
rows: [LIB-COMPAT-001, LIB-COMPAT-007, SACM23-XMI-002, SACM23-XMI-001, SACM23-XMI-024, LIB-API-010]
mode: pr
base: slice/0034-m1-atomic-edits-and-results (PR #45, merged); the PR targets main
---

# Slice 0035 — Load and save modes

## Scope

`xmi::read` takes an explicit mode: strict or tolerant. `xmi::write` takes a save mode, and
strict save refuses a document that `validate()` reports an Error for. This is the "load and
save modes" ADR that the 2026-11-13 ADR review asked for. It takes over ADR 0013 amendment
1's strict-save rule, and settles three things:
- the semver stance, since a new validation rule can make a default save fail;
- the reporting rule: every affected element, or the first;
- how strict import treats `xsi:type` and `xmi:version`.

Which tolerant variations exist today is set by research. Tolerant reading of particular
shapes stays with its own rows (LIB-COMPAT-003, -008, -010).

## Basis

D-STRICT, D-PRESERVE, D-SEV; SACM 2.3 clause 2; XMI 2.5.1 §2.2.2; ADR 0013 amendment 1;
ADR 0015, 0017 and 0021.

## Design

ADR 0022 (accepted). The new `xmi::Mode { Strict, Tolerant }` has no default of its own.
- **Reading.** `ReadOptions` defaults to tolerant, with `with_mode()`/`mode()`, and
  `ReadOutput::mode()` reports the mode used. Two new read findings: SACM-XMI-010 for
  `xsi:type` (Error strict, Warning tolerant) and SACM-XMI-011 for `xmi:version` (Info).
- **Writing.** `WriteOptions` defaults to strict, with `with_mode()`/`mode()`. A strict write
  validates and refuses with `WriteError::Invalid { diagnostics }`, holding every Error.

## Log

- 2026-11-13 choose: the next M1 rows. They unblock the most later rows, and the ADR review
  asked for this ADR before them.
- 2026-11-13 design: ADR 0022 accepted by the maintainer; SACM-XMI-011 is Info in both modes;
  SACM23-VAL-020 moves to M1 as the next slice. SACM23-XMI-002's acceptance is reworded and
  re-checked here.
