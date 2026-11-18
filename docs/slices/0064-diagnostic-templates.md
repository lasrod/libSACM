---
slice: 0064
name: diagnostic-templates
state: in-progress
step: verification
rows: [LIB-API-018]
mode: pr
base: main (3025637)
---

# Slice 0064 — Every diagnostic carries its template and arguments

## Scope

The diagnostics half of issue #82, and M4's last row:
- every `Diagnostic` the crate emits gets a template id and named arguments, and its message is rendered from them;
- the English stays byte-identical to 0.1.0;
- with `serde`, a diagnostic writes `template` and `arguments` after `message`, and 0.1.0 JSON still reads.

The CLI's JSON is not changed (ADR 0023).

## Basis

There are no SACM clauses: this is library API. Its basis:
- ADR 0030 amendment 3 (accepted 2026-11-18): Templates, R8, R9, R11, Records and Order;
- the maintainer's decisions Q8, Q9 and Q11 of 2026-11-17;
- the design brief, `target/slice-0064-design.md` (git-ignored).

## Design

- `Diagnostic::template() -> &'static str` and `Diagnostic::arguments() -> &[Argument]` are added. A private `details: Option<Details>` holds them.
- The crate-private `emit` replaces `create`.
- There are 102 diagnostic templates, with `Carrier::Diagnostic`: 30 from validation and 72 from the reader. Refusals and diagnostics together have 204.
- Order keys 8 and 9 are the template and the arguments.
- Serde is hand-written, with R9 defaults and R11 checks.
- No ADR change.

## Log

- 2026-11-18 design: the designer refreshed the 0063 design for 0065's five diagnostic forms. LIB-API-018 is analyzed. LIB-API-017 acceptance 5 and LIB-DIAG-001 are reworded, with notes on LIB-API-006 and LIB-DIAG-001.
- 2026-11-18 tests: the builder wrote 18 tests (cited). Red confirmed by the lead:
  - gated tests fail with E0599 (`template`/`arguments` missing);
  - the xtask count (0 of 102) and construction (28 `[built-without-template]`) tests fail;
  - the message proof `lib_api_018_each_diagnostic_template_input_gives_its_message` is
    ungated and passes on today's code for all 102 templates.
  - Snapshot red-0064.tar. Accepted gap: acceptance 6's "Text before Count" and prefix
    order cannot be reached through valid records of one template (R11 fixes the names, kinds
    and count); it is the `Argument` order 0063 verified.
- 2026-11-18 implementation: three tests were red on test mechanics only, because `serde_json::Value`
  sorts keys and R11 refuses arguments given out of order. The lead ruled option (b): rewrite how
  the tests build their input, keeping their key order, with no dependency feature change. The tests:
  `lib_api_006_each_key_of_every_object_of_the_golden_files_is_required_or_has_its_default` (a
  verified row; mechanics only), and the two `serde_diagnostics.rs` tests
  `lib_api_018_diagnostics_0_1_0_json_reads_…` and `lib_api_018_the_invalid_write_error_s_…`.
- 2026-11-18 implemented: the builder's 26 hand mutants are all killed; the three tests were fixed under
  ruling (b), mechanics only; the full gate is green, and messages-0.1.0.txt is unchanged.
