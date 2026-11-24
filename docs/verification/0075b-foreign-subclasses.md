# Verification 0075b — Foreign subclasses and the corpus claim

- Date: 2026-11-25
- Row verdict: LIB-COMPAT-017: PASS
- Row verdict: LIB-COMPAT-005: PASS
- Verifier: `sacm-verifier` (independent, read-only), two rounds.
  - **Round 1, 93fad0d:** FAIL for LIB-COMPAT-017 (one blocking finding). LIB-COMPAT-005
    met its acceptance but was held on its dependency.
  - **Round 2, 0845d61:** PASS for both rows, against the round-1 prescription.
- Evidence: CI runs 45487617250 (round 1) and 45493833428 (round 2); the PR-diff mutants
  shards had not finished, so both rounds hand-mutated the new decision logic.
- Slice brief: [docs/slices/0075b-foreign-subclasses.md](../slices/0075b-foreign-subclasses.md)
- Decisions: [ADR 0044](../adr/0044-the-emf-dialect-and-foreign-subclasses.md) D3 and D4;
  D-STRICT, D-TERM, D-ROOT-NAME, D-CORPUS; slice 0075b Q1 (the maintainer, 2026-11-24).

## Result

- **LIB-COMPAT-017.** With `ReadOptions::with_subclass`, a tolerant read reads an element
  typed with a declared foreign class as its SACM 2.3 class, keeps the class
  (`SacmElement::subclass()`) with one SACM-XMI-003 `subclass-type-kept` Warning, and a
  tolerant write writes it back as `xmi:type`. A strict write refuses it (SACM-XMI-006),
  and a strict read ignores the declarations. No declaration applies to a proxy, to a
  SACM namespace, or to an element typed in both XMI namespaces. A root keeps a declared
  subclass of its own class. The goldens G3→G4 and G5→G6 round-trip.
- **LIB-COMPAT-005.** Five corpus files from ACEditor, Epsilon and wrwei/SACM open, check
  and write back without loss; files 12 and 13 read their GSN-typed elements (46 and 24)
  as their declared SACM classes. `docs/compat/corpus.md` states the claim; every fact in
  it is asserted, including the kept-whole counts. This is interoperability evidence, not
  conformance.
- **Tests.** 18 `subclasses.rs` tests and 3 `lib_compat_005` corpus tests. Every hand
  mutation of the new decision logic was killed (39 by the builder, about 30 by the
  verifier in round 1, 19 in round 2).

## Findings

| Blocking / non-blocking | Row | Finding | Resolution |
|---|---|---|---|
| Blocking | LIB-COMPAT-017 | A declared subclass on an element typed in both XMI namespaces (EMF's and XMI 2.5.1's) was written with two `xmi:type`s, so the tolerant save did not open again. | Such an element reads as without declarations; the requirement and rustdoc say so. Control `lib_compat_017_a_type_in_both_xmi_namespaces_reads_as_without_declarations`, plus one test for a too-broad guard (round 2). |
| Non-blocking | LIB-COMPAT-005 | `corpus.md` claimed the tests check every fact, but the kept-whole counts of files 7, 8 and 9 were not asserted. | `Claimed.kept_whole` asserted for all five files. |
| Non-blocking | LIB-COMPAT-005 | The CHANGELOG said the files "read"; the Epsilon files are opened and kept. | "open, check and write back without loss". |
| Non-blocking | LIB-COMPAT-017 | A garbled sentence in the `Preserved` doc. | The verifier's text. |
| Non-blocking | LIB-COMPAT-017 | `with_subclass` accepts a name that is not an NCName; it can match an equally odd type and writes back unchanged. | Logged here; no loss. |
| Non-blocking | LIB-COMPAT-002 | Without declarations, an element typed in both XMI namespaces may still be written with two `xmi:type`s; a root's kept foreign type loses its binding. | `docs/follow-ups.md`; LIB-COMPAT-019. |
