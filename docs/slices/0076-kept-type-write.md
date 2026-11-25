---
slice: 0076
name: kept-type-write
state: in-progress
step: verification
rows: [LIB-COMPAT-019]
mode: pr
base: main (dab82db)
---

# Slice 0076 — A kept type attribute writes back readable, complete XMI

## Scope

- **LIB-COMPAT-019.** A tolerant save of an element that keeps a foreign `xmi:type` or
  `xsi:type` (not SACM's, not a declared subclass) writes well-formed XMI. Every prefix
  the kept value names is bound, there is at most one `xmi:type` per element, also after
  an edit moves the element to or from the roots, and the output reads back as the same
  document.
- The slice-0075b follow-up is in scope: an element typed in both XMI namespaces, read
  without declarations, may be written with two `xmi:type`s.

Left out: LIB-SEC-007, which is slice 0077. The only other ready M6 row, it has an
unrelated cause.

## Basis

XML 1.0 (Fifth Edition) §3.1 WFC: Unique Att Spec; Namespaces in XML 1.0 §5; D-PRESERVE;
D-ROOT-NAME; ADR 0044.

## Design

- Design brief: `target/slice-0076-design.md` (git-ignored). Rules V (a kept type value
  keeps its prefix binding), A/D (an attribute in both XMI namespaces: XMI 2.5.1's is read,
  for `id`, `type`, `idref`, `version`, `uuid`, `label`), B (an element keeping only an EMF
  `type` is written with its own `xmi:type`), C (edits refuse to make a child of an element
  keeping `xmi:type` or `xsi:type`).
- Public API: `EditError::KeptRootType { element, attribute, value }` (SACM-CMD-005), and
  its JSON record. No new diagnostic template.
- ADR 0033 amendment 2 (accepted: the maintainer, 2026-11-25, Q1 = refuse the edit); ADR
  0044 amendment 1 (accepted, lead ruling); D-ROOT-NAME extended.

## Log

- 2026-11-25 started from main dab82db; designer running.
- 2026-11-25 design applied: LIB-COMPAT-019 `analyzed`, widened to the six attributes in
  both XMI namespaces (lead); Q1 answered (a). Builder step 1 started.
- 2026-11-25 step 1 done: `kept_types.rs` T1 to T6 (red: compile error on the missing
  `EditError::KeptRootType`; on a copy without it, each fails at runtime on today's
  behaviour), the `edit-errors.json` sample, `EDIT_ERROR_COUNT` 24. Red confirmed by the
  lead, snapshot `target/red-0076-tests.tar`, row `tests-failing`.
  - Lead-approved changes to existing tests: `serde.rs` gains a `KeptRootType` sample in
    `edit_error_samples()` (the golden is generated from it);
    `lib_edit_017_move_element_and_its_refusals_round_trip_as_their_golden_json` compares
    the four refusals just before the new last entry, not the last four.
  - Lead ruling, rule V: a value with an empty prefix or empty local part (`:Bundle`) is no
    QName and gets no binding; new T1 case (k).
- 2026-11-25 step 2 (builder): V, A/D, B, C built; three tests added for survivors. Rule V
  broke the verified `lib_compat_017_a_root_reads_a_subclass_only_when_its_types_agree`
  (c), which passed only because the `xsi:type` value's prefix was unbound. Lead rulings:
  - B widened: a root keeping `xsi:type` and no XMI 2.5.1 type is written with its own
    `xmi:type`. This changes T1(b)'s confirmed golden (start tag gains
    `xmi:type="sacm:ArgumentPackage"`); approved.
  - Rule V also binds an XMI metadata type value (a wrapper's `xmi:type="p:Model"`), with
    a test.
  - The stale message and comment of `lib_edit_017_move_element_and_its_refusals_...` are
    reworded.
- 2026-11-25 rulings applied (builder): B covers `xsi:type`; metadata type values bind;
  tests `a_metadata_type_value_keeps_the_binding_of_its_prefix` and
  `a_type_values_binding_counts_against_the_size_limit` added. Lead gate green (3240
  workspace, 497 xtask, corpus not skipped). Row `implemented`.
