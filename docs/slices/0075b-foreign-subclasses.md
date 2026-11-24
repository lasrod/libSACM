---
slice: 0075b
name: foreign-subclasses
state: in-progress
step: implementation
rows: [LIB-COMPAT-017, LIB-COMPAT-005]
mode: pr
base: main (6b8411c)
---

# Slice 0075b — Foreign subclasses and the corpus claim

## Scope

- **LIB-COMPAT-017.** An application declares a foreign class (a GSN profile's Goal, say)
  a subclass of a SACM 2.3 class. A tolerant read then reads elements of that class as the
  SACM class and keeps the class, which a tolerant save writes back. Strict mode ignores
  the declarations.
- **LIB-COMPAT-005.** What the fetched corpus shows: five files from three tool families
  read, check and write back without loss. This is interoperability evidence, not
  conformance. D-CORPUS closes Q-INTEROP.

Left out: LIB-COMPAT-019, the tolerant write of an undeclared kept foreign type (filed in
this slice, its own slice next).

## Basis

- XMI 2.5.1 §7.8.6, §8.2.1 rule 4f, §7.11.1 Table 7.1, Annex B.2; SACM 2.3 §2.4, §8.12.
- ADR 0044 D3 and D4; M6 decisions Q7 and Q10 (the maintainer, 2026-11-19); slice 0075b
  Q1 (2026-11-24: (a), a root keeps a declared subclass of its own class).

## Design

- ADR 0044 D3 gains the root sentence (Q1). D-CORPUS added to `decisions.md`.
- The design brief, `target/slice-0075b-design.md` (git-ignored), written against 0075a's
  first commit 4e67af3. Main now has 0075a's fix rounds (open content); where they meet,
  main holds.
  - Public API: `ReadOptions::with_subclass`, `ReadOptions::subclass`,
    `model::Subclass<'a>`, `SacmElement::subclass()`; template
    `SACM-XMI-003/subclass-type-kept`.
  - Golden samples G3 to G6 and J3; tests 1 to 12 (`crates/sacm/tests/subclasses.rs`) and
    1 to 3 (`xtask/tests/corpus.rs`); the corpus.md section.
  - Test 6 is named `..._declarations_never_apply_to_a_sacm_namespace_and_a_root_keeps_only_its_own_subclass`
    (Q1 = (a)); tests 6(e) and 10(d) keep outcome (a).

## Log

- 2026-11-25 design applied: both rows `analyzed`, LIB-COMPAT-019 filed (planned, M6),
  LIB-COMPAT-015's source reworded, Q-INTEROP closed by D-CORPUS.
- 2026-11-25 step 1 done (builder): `subclasses.rs` tests 1 to 12, `corpus.rs` tests 1
  to 3, `subclass_cases` in `diagnostic_templates.rs`, fixtures G3 to G6 and J3. Red
  confirmed by the lead: every failure is a compile error on the missing API
  (`ReadOptions::with_subclass`/`subclass`, `model::Subclass`, `SacmElement::subclass`).
  Tests cited, both rows `tests-failing`. Builder step 2 started.
