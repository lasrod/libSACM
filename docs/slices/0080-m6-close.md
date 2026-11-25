---
slice: 0080
name: m6-close
state: done
step: done
rows: [LIB-COMPAT-003, LIB-COMPAT-005, SACM23-XMI-002, LIB-SEC-003, SACM23-CP-001, SACM23-CP-002, SACM23-CP-003, SACM23-CP-004]
mode: pr
base: main (d71d139)
---

# Slice 0080 — M6 close

## Scope

The milestone close of M6 (skill step 7): apply the matrix-steward audit
(`target/m6-close-audit.md`, git-ignored), triage `docs/follow-ups.md`, and present the
summary. Finding 1 (a D-RT break) became LIB-COMPAT-020 and was fixed in slice 0079. No
behaviour changes here: wording, matrix links, docs, the mutants config and new rows.

- **Statement** (finding 2): `docs/sacm/compliance-points.md` names the Difference-target
  limit; new row SACM23-XMI-031 (M7, planned) for the behaviour.
- **Verified rows reworded** to match the code: LIB-COMPAT-003, LIB-COMPAT-005,
  LIB-EDIT-017, LIB-EDIT-018, SACM23-XMI-002, LIB-SEC-003, LIB-COMPAT-002 (acceptance),
  LIB-API-017, LIB-API-018, LIB-SEC-006, SACM23-XMI-019; notes on LIB-API-006, the four
  compliance points and the re-check notes of slices 0077 and 0078.
- **README and crate docs**: a tolerant read opens a file unless it has a DOCTYPE, repeats
  an `xmi:id`, uses another encoding or passes the limits; README status (M6 done); crate
  README "Planned (M7)"; `corpus.md` wording.
- **Sequencing**: SACM23-CP-002/003/004 depend on LIB-SEC-007; LIB-SEC-008's depends_on.
- **Mutants**: two dead exclusions repointed (`document.rs:1944:41`, `1951:71`), duplicate
  triage rows removed, the `refusal.rs:207:22` row re-argued.
- **CHANGELOG**: LIB-SEC-008 moved to Breaking.
- **New rows (M7)**: SACM23-XMI-031 (planned); candidates LIB-EDIT-022, LIB-CLI-006,
  LIB-COMPAT-021.
- **Follow-ups**: three items closed (latin1 decided; Difference target and EMF path
  proxies became rows), two added from verifications 0075b and 0076.

## Basis

Quality slice: acceptance by the lead. Rows in the header carry the main wording changes.

## Log

- 2026-11-25 started from main d71d139 after #109 (M6 22/22 verified); audit applied by the
  lead; `lib.rs` crate doc by the builder.
- Not applied: audit item 12's last bullet (follow-up #10, CLAUDE.md and the sacm-expert
  skill naming XML Schema Part 2), which needs the maintainer's OK; it stays in
  docs/follow-ups.md. LIB-API-018 says 111 diagnostic templates, not the audit's 110,
  because slice 0079 added `SACM-XMI-003/proxy-of-typed-root`.
- 2026-11-25 verify round 1 at 2a7a745: FAIL on wording only (the audit's tolerant-open,
  EMF, corpus and Difference-target sentences were too broad; stale template counts).
  Fix round: the verifier's exact text, lead (matrix, READMEs, corpus.md, statement,
  architecture.md) and builder (lib.rs, xmi.rs).
- 2026-11-26 verify round 2 at 7155bc0: PASS for all eight rows; its four wording items
  applied (corpus file 6, the `Mode` docs, architecture.md, a stray space). Record
  `docs/verification/0080-m6-close.md`. M6 closed.
