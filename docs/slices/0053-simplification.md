---
slice: 0053
name: simplification
state: in-progress
step: implementation
rows: [LIB-COMPAT-001, LIB-COMPAT-003, LIB-COMPAT-007, LIB-API-011, SACM23-VAL-006, SACM23-XMI-008, LIB-API-006]
mode: pr
base: main
---

# Slice 0053 — Simplification

## Scope

The maintainer's decisions of 2026-11-16, taken after an independent review:
- **Tolerant read opens any well-formed SACM file.** What a strict read refuses is kept as
  preserved content with a Warning. That includes conforming content not read yet
  (SACM-XMI-014). Strict read is unchanged.
- **A strict save checks schema validity only:** preserved content, a non-NCName id, or a
  reference to no written id. It never runs `validate`.
- **The serde JSON shape follows semver until 1.0.**
- **Process:**
  - three roles: designer, builder, and an independent verifier;
  - one fix round, then ask;
  - C++ only when stuck (ADR 0006 amendment 5, CLAUDE.md rule 6).
- **Roadmap:**
  - real-file tests;
  - move and copy edits, `referrers`, an undo decision, `has_errors` and a first release,
    all in M3, before M4 and M5;
  - preserved-content extensions deferred.

Left out: hiding the ids of names and texts; shorter docs.

## Basis

- SACM 2.3 clause 2 (export "XMI documents that conform with the SACM XML Schema").
- XMI 2.5.1 §7.8.2 ("schemas that ignore multiplicities") and rule 4i.
- XML Schema 1.0 Part 1 §3.3.4 (ID/IDREF) and Part 2 §3.3.8 (ID is an NCName).
- [ADR 0035](../adr/0035-tolerant-read-and-schema-only-save.md).

## Design

The design brief is `target/slice-0053-simplification-brief.md` (git-ignored).
- **Public API:**
  - `Document::has_preserved_content()`;
  - `Code::XMI_UNREAD_CONTENT` (SACM-XMI-014);
  - `Code::XMI_EXTERNAL_HREF` (SACM-XMI-007);
  - `Difference::DocumentPreservedContent`.
- **Model:** a document holds preserved content outside its elements.

## Log

- 2026-11-16 choose: the simplification slice, right after slice 0052, as the maintainer
  decided. The design brief is done; the maintainer chose Option B.
- 2026-11-16 docs (lead):
  - ADR 0035, plus amendments to ADRs 0006 (amendment 5), 0015, 0021, 0022, 0030 and 0033;
  - D-PRESERVE, D-STRICT, D-ID and D-ENUM-EXACT;
  - CLAUDE.md rule 6;
  - the seven in-scope rows rewritten;
  - the roadmap: M3 is now "Artifact model and a first release" (20 rows). It gains LIB-COMPAT-015, LIB-EDIT-017, LIB-EDIT-018 and LIB-API-016, and LIB-EDIT-011 and LIB-QA-005 move into it. LIB-COMPAT-013, LIB-COMPAT-014 and LIB-EDIT-016 are deferred.
  The tests and the role restructure are in progress.
- 2026-11-16 tests: the old roles' step 1. 47 tests are red, and the lead confirmed them red for the right reason. About 200 refusal tests now read strictly, and 19 tests are renamed and 12 replaced, all cited. The eight qualifications the test engineer pinned are accepted.
- 2026-11-16 roles: `sacm-designer`, `sacm-builder` and the verifier's docs review are in place (ADR 0006 amendment 5), and the builder may use `target/scratch/**`. The old five role files are removed at this slice's close.
- 2026-11-16 implementation: `sacm-builder` step 2 is done; the workspace tests (2,335) are green. The lead accepted the following:
  - a TaggedValue `content` plus `value` is kept whole, with one MULT-001;
  - a kept bad literal counts as an occurrence for every part;
  - the ID-002 and MULT-001 severity cells stay as the verified tests pin them, with the tolerant Warning given in the row text.
  Mutants: 172 on the diff of `roots.rs`, `refusal.rs` and `output.rs`. Five unit tests were added for the misses; the one left is a timeout, an infinite loop in `Kept::rename`.
- 2026-11-16 matrix (lead): 45 rows reworded so that no sentence states a both-modes refusal or a validating strict save. LIB-COMPAT-001 now says that a refusal a read row states without naming a mode is a strict read's. LIB-TOOL-003/007 follow the new roles. The in-scope rows keep `verified` on their old records until this slice's record replaces them. The gate forbids a verified row from depending on an unverified one.
- 2026-11-16 roles: the five old role files and their write scopes are removed. The hook tests use the new roles, and four were renamed.
- 2026-11-16 verification round 1: FAIL. The findings:
  - two defects:
    - a write with no root beside document-level content was not `NoRoot`;
    - the wrapper's `xmi:id` was never checked;
  - four test gaps, the same ones CI's mutants run missed;
  - a lost default-mode test;
  - wording in the matrix, the docs and `Cargo.toml`;
  - a `Kept::rename` mutant that timed out and failed CI (prescription D4).
- 2026-11-16 fix round (the only one): `sacm-builder` fixed every blocking and non-blocking item.
  - The rename loop is now bounded.
  - The two `check_preserved_ids` mutants are excluded as equivalent, and the stale `set_preserved` exclusion is removed.
  - The lead reworded the matrix and ADR 0035 and cited the six new tests.
  - The lead's record of test changes since the confirmed red:
    - the gates were removed (`unread_content.rs`, the catalog lists);
    - `serde.rs` `CODES` gained the two live codes;
    - one comment was fixed in `round_trip.rs`;
    - the hook tests use the new roles;
    - this round's new tests were added.
    No step-1 assertion was weakened.
