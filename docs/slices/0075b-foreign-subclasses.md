---
slice: 0075b
name: foreign-subclasses
state: done
step: done
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
  open, check and write back without loss. This is interoperability evidence, not
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
- 2026-11-25 step 2 done (builder), commit 93fad0d, draft PR #105, CI 45487617250. Lead
  gate green (3226 workspace, 497 xtask, corpus not skipped). Both rows `implemented`.
  - Four tests added for surviving hand mutants: `a_root_reads_a_subclass_only_when_its_types_agree`,
    `types_with_one_name_in_two_namespaces_disagree`,
    `a_type_names_a_subclass_only_by_a_prefix_bound_to_its_namespace`,
    `a_subclass_counts_its_namespace_text_against_the_size_limit`.
  - Pre-existing test changed: `xtask/tests/message_templates.rs` `DIAGNOSTIC_TEMPLATES`
    109 → 110 (the design's LIB-API-018 count); lead accepted.
  - Deviations from the design, lead accepted: S2's type agreement also applies to a
    root (else two `xmi:type`s); S5's `xml:` prefix case cannot occur (roxmltree does
    not resolve it in a value), so it is kept as an attribute; the `xmlns` and
    empty-namespace guards were unreachable (the input layer refuses both) and removed.
- 2026-11-25 verify round 1: LIB-COMPAT-017 FAIL (B1: a declared subclass on an element
  typed in both XMI namespaces writes two `xmi:type`s, unreadable); LIB-COMPAT-005 PASS
  (waits on -017). Fix round: guard + test
  `lib_compat_017_a_type_in_both_xmi_namespaces_reads_as_without_declarations`, rustdoc
  and requirement wording, corpus `kept_whole` counts, CHANGELOG and `Preserved` wording.
- 2026-11-25 verify round 2 at 0845d61: PASS for both rows. Record
  `docs/verification/0075b-foreign-subclasses.md`; both rows `verified`.
