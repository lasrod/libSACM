---
slice: 0077
name: attribute-limit
state: done
step: done
rows: [LIB-SEC-007]
mode: pr
base: main (fd4025c)
---

# Slice 0077 — An element's attributes are limited

## Scope

- **LIB-SEC-007.** roxmltree 0.21.2 checks each attribute of an element against the earlier
  ones, so one element with many attributes reads in quadratic time (100,000 took about
  214 s debug). A fifth read limit, at most 256 attributes on one element (namespace
  declarations not counted), refuses such an element before parsing, so a read at the
  defaults is linear in its input.

Left out: a parser fork (ADR 0041 amendment 2 D9); raising the default (when roxmltree
ships a linear check).

## Basis

Quality row: no OMG clause. LIB-SEC-002, LIB-SEC-003; ADR 0041 amendment 2 (the
maintainer, 2026-11-25: a limit, not a fork; default 256).

## Design

- Design brief: `target/slice-0077-design.md` (git-ignored).
- Public API (additive): `Limits::with_max_attributes`, `Limits::max_attributes`,
  `Limit::Attributes`; template `SACM-SEC-003/attributes` (`max`).
- Upstream: roxmltree issue 153 already covers it (closed with a 65,535 cap), so no new
  issue was filed.

## Log

- 2026-11-25 started from main fd4025c; ADR 0041 amendment 2 filed accepted, D-STRICT
  amended, LIB-SEC-007 reworded and `analyzed`. Builder step 1 started.
- 2026-11-25 step 1 done: `attribute_limit.rs` tests 1 to 9 and the `message_templates.rs`
  case (`REFUSAL_TEMPLATES` 103). Red confirmed by the lead: compile errors only on the
  missing API; with an API-only stub, tests 2 to 6, 8 and 9 fail at runtime (test 6: a
  Claim of 100,000 attributes did not read within 5 s; measured 128 s and 270 s today);
  tests 1 and 7 are guards. Snapshot `target/red-0077-tests.tar`, row `tests-failing`.
  Lead fixed test 2's acceptance: an `xmi:XMI` holding no package is `NoPackage`.
- 2026-11-25 step 2 done (builder): built as designed; 19 hand mutants killed, no tests
  added beyond the two unit tests of the design. Count bumps forced by the new template,
  lead-approved: `template.rs` unit test 212 -> 213, `xtask/tests/message_templates.rs`
  `REFUSAL_TEMPLATES` 102 -> 103. Lead finished the CHANGELOG Added sentence and the
  compliance-points Size bullet. Row `implemented`.
- 2026-11-25 verify round 1 at e4de6a1: FAIL. Repeated namespace declarations (`xmlns`,
  `xmlns:xml`), which roxmltree 0.21.2 does not refuse and the slice left uncounted, still
  make a read at the defaults quadratic (a strict read of 40,000 repeated `xmlns=""` over
  40,000 Claims: 3.7 s release, no diagnostic). Lead rulings for the one fix round: a
  declaration that repeats a prefix in the same start tag counts (O(1) per declaration);
  controls K1 to K6 in a new test; the verifier's wording W1 to W7 and three more wording
  items. New row LIB-SEC-008 (planned, M6): refuse a repeated declaration as malformed XML,
  its own small slice next.
- 2026-11-25 verify round 2 at 3406ef4: PASS. Record `docs/verification/0077-attribute-limit.md`;
  LIB-SEC-007 verified; LIB-SEC-008's requirement reworded with the verifier's text.
