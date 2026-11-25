---
slice: 0078
name: repeated-namespace-declaration
state: in-progress
step: verification
rows: [LIB-SEC-008]
mode: pr
base: main (8fd8216)
---

# Slice 0078 — A repeated namespace declaration is malformed XML

## Scope

- **LIB-SEC-008.** roxmltree 0.21.2 accepts a start tag that declares the default
  namespace (`xmlns`) or the prefix `xml` twice, which XML 1.0 §3.1 (Unique Att Spec)
  forbids, so both modes read such a file today. The post-parse checks of LIB-SEC-005
  refuse it as `Malformed` (SACM-XML-001), with the existing `duplicate-attribute` or
  `duplicate-prefix` template, at the repeat.

Left out: nothing. No public API, no new template.

## Basis

XML 1.0 (Fifth Edition) §3.1, §1.2; Namespaces in XML 1.0 (Third Edition) §3, §6.3, §8;
LIB-SEC-002, LIB-SEC-005; D-STRICT.

## Design

- Design brief: `target/slice-0078-design.md` (git-ignored). The refusal goes in
  `input.rs` `start_tag` (`check_unrepaired`), not the pre-parse scan, so the earliest
  offending construct still wins and the scan refuses only limits.

## Log

- 2026-11-25 started from main 8fd8216; LIB-SEC-008 `analyzed`; re-check notes on
  LIB-SEC-002, -005, -006, -007. Builder step 1 started.
- 2026-11-25 step 1 done: six `xml_wellformedness.rs` tests. Red confirmed by the lead at
  runtime: tests 1, 2, 4 and 5 fail on today's behaviour (a repeated `xmlns` or
  `xmlns:xml` reads, or a later error is reported instead); tests 3 and 6 pass as guards.
  Snapshot `target/red-0078-tests.tar`, row `tests-failing`.
- 2026-11-25 step 2 done (builder): built as designed; gate green (3263 workspace, 497
  xtask; the W3C suite's 1017 cases and counts unchanged; corpus not skipped). Lead
  rulings: the `check_unrepaired` doc sentence the builder added is kept; the mutant
  "repeat check before the `xmlns:xmlns` check" is equivalent (at most one of the two can
  fire at an attribute); the unit test's constant-time case is enlarged from 100,000
  to 300,000 declarations (correct code 0.74 s, the linear-search mutant 349 s, against a
  5 s deadline).
