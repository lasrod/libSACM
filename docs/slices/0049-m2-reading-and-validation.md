---
slice: 0049
name: m2-reading-and-validation
state: done
step: close
rows: [LIB-COMPAT-008, SACM23-XMI-026, LIB-COMPAT-010, SACM23-VAL-019, SACM23-VAL-021, LIB-QA-018]
mode: pr
base: main
---

# Slice 0049 — Reading the C++ shapes, xsi:nil, and two validation rules

## Scope

The M2 reading and validation rows:
- **LIB-COMPAT-008:** documents written by the assurance-forge C++ library load without
  loss: its legacy name and text shapes.
- **SACM23-XMI-026:** `xsi:nil` on a value element.
- **LIB-COMPAT-010:** a tolerant load infers a missing type attribute.
- **SACM23-VAL-019:** an `asCited` Assertion must be a citation of an Assertion.
- **SACM23-VAL-021:** an implementation constraint is only allowed on an abstract element.
- **LIB-QA-018 (ADR 0006 amendment 4):** a push that changes only `docs/` skips the CI jobs
  that can only fail on code.

Left out: preserving unknown content (LIB-COMPAT-003) and the strict save
(SACM23-XMI-008), which close M2 in the next slice.

## Basis

formal/23-05-08 §8.2, §8.6 (and spec defect D15), §11.8; XMI 2.5.1 §7.6.3, §7.8.6, §7.15,
§9.4.1 and rule 2b (spec defect D16); XML Schema 1.0 Part 1 §3.3.4. ADR 0022 amendment 1
(accepted). The assurance-forge reader and its legacy serializer, for the shapes.

## Design

No new types. New codes `SACM-XMI-012` and `SACM-CITE-002`; SACM-ABS-001 widened.
LIB-COMPAT-012 (M6) takes the legacy shapes that invent or merge elements.

## Log

- 2026-11-15 choose: the M2 rows on reading and validation, independent of preservation.
- 2026-11-15 the maintainer adopted ADR 0006 amendment 4 (less waiting); its CI part,
  LIB-QA-018, joins this slice. The lead wrote the `ci.yml` change.
- 2026-11-15 research: the five rows analyzed. The maintainer chose: tolerant reads of the
  one-to-one C++ shapes, refused in strict mode (SACM-XMI-012); `xsi:nil` reads as omitted;
  LIB-COMPAT-010 becomes a SACM-XMI-005 refusal; codes SACM-XMI-012 and SACM-CITE-002, and a
  widened SACM-ABS-001. Spec defects D15 and D16 recorded. New row LIB-COMPAT-012 (M6).
  LIB-QA-018's acceptance is written by the lead. All six rows `analyzed`.
- 2026-11-15 tests: two test engineers in parallel (reader rows; validation rows and
  LIB-QA-018), 59 tests. Red confirmed (missing `Code::XMI_LEGACY_SHAPE`,
  `XMI_MISSING_TYPE`, `AS_CITED_INVALID`; today's `Unsupported` at the same locations; missing
  findings). Tests that pinned the replaced refusals were updated or removed (BASE-004,
  BASE-005 and XMI-012 to be re-checked; a full mutants run is due because tests were
  removed). A LIB-QA-018 test found that a rename into `docs/` read as docs-only; the lead
  added `--no-renames`. Rows `tests-failing`; LIB-QA-018 `implemented`.
- 2026-11-15 implementation: done (`ReadError::LegacyShape`, `ReadError::MissingType`, three new
  codes; a strict LegacyShape comes after every other refusal). 39 older tests and fixtures
  contradicted the new rules (mostly a constraint on a concrete element, now SACM23-VAL-021);
  the test engineers made the holders abstract or fixed the inputs, keeping each test's point.
  Verified rows to re-check: SACM23-XMI-010, -012, -016, -021, SACM23-ARG-006, -013,
  SACM23-BASE-004, -005, SACM23-VAL-002, -003, -004, -005, -007, -010, -011, -012, -016,
  -018, LIB-EDIT-008, LIB-CLI-002, -003, LIB-API-006. Rows `implemented`. Committed locally
  for a PC restart.
- 2026-11-15 verification (CI on f9a0e3b): green except the PR-diff mutants, 13 missed in new
  code. Tests added: 12 killed, plus one extra `holds_beside` mutant; `legacy_shape` `<` → `<=`
  is equivalent and excluded with a triage row. The stale `deleted_ids` exclusion is removed
  (slice 0048 follow-up). A full mutants run (43693981251) was dispatched because tests were
  replaced.
- 2026-11-15 verification round 1: PASS for SACM23-XMI-026, LIB-COMPAT-010, SACM23-VAL-019
  and -021; FAIL for LIB-COMPAT-008 (seven hand mutations survived; stale refusal claims in
  verified rows). Fixes: controls C1-C7 in legacy_shapes.rs, each killing its mutation; the
  stale texts in SACM23-BASE-004/005/006, SACM23-XMI-012/014/016, SACM23-ARG-008/010 and
  LIB-COMPAT-001 corrected; three doc follow-ups fixed, three logged. Copilot found a hole in
  LIB-QA-018 (a docs-only push after a failed or cancelled code run skipped the code checks);
  `changes` now also requires CI success on the previous commit, with tests.
- 2026-11-15 verification round 2: LIB-COMPAT-008 PASS once four more stale texts (SACM23-XMI-012, -014, -016, BASE-004) and LIB-COMPAT-001 were fixed (wording only, read by the lead as the verifier required); the full mutants run 43699001243 is clean, so the re-checked rows stay verified. Five library rows `verified` (record 0049). LIB-QA-018 waits for this docs-only commit's CI run.
- 2026-11-15 close: the docs-only push 1b84f84 (run 43704277285) skipped the code-only jobs with all required checks green; LIB-QA-018 `verified`. Slice done.
