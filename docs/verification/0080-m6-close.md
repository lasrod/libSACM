# Verification 0080 — M6 close

- Date: 2026-11-26
- Row verdict: LIB-COMPAT-003: PASS
- Row verdict: LIB-COMPAT-005: PASS
- Row verdict: SACM23-XMI-002: PASS
- Row verdict: LIB-SEC-003: PASS
- Row verdict: SACM23-CP-001: PASS
- Row verdict: SACM23-CP-002: PASS
- Row verdict: SACM23-CP-003: PASS
- Row verdict: SACM23-CP-004: PASS
- Verifier: `sacm-verifier` (independent, read-only), two rounds.
  - **Round 1, 2a7a745:** FAIL on wording only. The audit's replacement sentences for what a
    tolerant read opens, EMF names, the corpus and the Difference target were too broad,
    and three template counts were stale.
  - **Round 2, 7155bc0:** PASS for all eight rows. Its four wording items were then applied
    with its text.
- Evidence: CI runs 45635259952 and 45638311875; CLI probes of every refusal named (an XMI
  2.4.1 or other-namespace wrapper, a loose non-package root, UTF-16 without a byte order
  mark, ISO-8859-2, SACM 2.4, a DOCTYPE, a repeated `xmi:id`, Difference-target ids);
  `cargo mutants --list` against `.cargo/mutants.toml`.
- Slice brief: [docs/slices/0080-m6-close.md](../slices/0080-m6-close.md)
- Audit: the matrix-steward's M6 close audit (git-ignored `target/m6-close-audit.md`).
  Finding 1 was a D-RT break, fixed as LIB-COMPAT-020 in slice 0079
  ([record](0079-proxy-of-typed-root.md)).

## Result

No behaviour changes. M6's verified rows, the conformance statement, the READMEs and the
crate docs now claim what the code does: a tolerant read opens a well-formed file whose
roots are all SACM packages unless it is one of the listed refusals; the EMF names read
are named; the corpus claim covers its five files; the statement names the Difference-target
id limit, which SACM23-XMI-031 (M7) will remove. The compliance points depend on the
attribute limit. Two dead mutants exclusions point at their mutants again and the triage is
de-duplicated. LIB-SEC-008 is listed under Breaking. Four rows were filed for M7
(SACM23-XMI-031 planned; LIB-EDIT-022, LIB-CLI-006, LIB-COMPAT-021 candidates) and the
follow-ups were triaged. The eight rows keep their existing records; this one records the
re-check.

## Findings

| Blocking / non-blocking | Row | Finding | Resolution |
|---|---|---|---|
| Blocking (wording) | SACM23-CP-001 | The tolerant-open sentence in the READMEs, `lib.rs` and `xmi.rs` left out refusals. | The verifier's text (round 1, round 2). |
| Blocking (wording) | SACM23-XMI-002 | The EMF clause claimed every name in EMF's namespace is read. | Names the `XMI` element and six attributes. |
| Blocking (wording) | LIB-COMPAT-005 | The corpus claim read as covering every third-party file. | Five files; file 6 opens; 10, 11, 14, 15 do not. |
| Blocking (wording) | SACM23-CP-001..004 | The Difference-target limit omitted the target's own attributes. | "On or inside". |
| Blocking (wording) | LIB-API-017, LIB-API-018 | Stale counts and claims. | The verifier's text. |
| Non-blocking | several | Stray and duplicate notes; the brief's log of the omitted follow-up #10. | Applied. |
