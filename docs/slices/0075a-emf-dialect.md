---
slice: 0075a
name: emf-dialect
state: in-progress
step: verification
rows: [LIB-COMPAT-002, LIB-CLI-004]
mode: pr
base: main (d3c21f0)
---

# Slice 0075a — The EMF dialect and `sacm convert`

## Scope

- **LIB-COMPAT-002.** A tolerant read reads what EMF writes. It takes the XMI names in
  `http://www.omg.org/XMI` and resolves EMF fragment paths to ids, each reported once per
  document. A strict read refuses the dialect.
- **LIB-CLI-004.** `sacm convert` turns a readable file into strict SACM 2.3, and refuses
  when that would lose content.

Left out, to slice 0075b: foreign subclasses (LIB-COMPAT-017) and the corpus claim
(LIB-COMPAT-005). The two ATL corpus files stay refused (LIB-COMPAT-014, deferred).

## Basis

- XMI 2.5.1 §7.3.1, §7.5.3, §7.6.1, §7.10.2, rule 4i; SACM 2.3 §2.4.
- M6 decision Q6 (the maintainer, 2026-11-19); slice 0075 Q1 and Q2 (2026-11-23).

## Design

- ADR 0044 (accepted), ADR 0023 amendment 2, and D-STRICT amended.
- The design brief, `target/slice-0075-design.md` (git-ignored):
  - LIB-COMPAT-002: rules R1 to R6 and tests 1 to 15;
  - LIB-CLI-004: tests 1 to 7;
  - Public API; Check order; Templates; Golden samples G1, G2, J1 and J2;
  - Corpus.
- Additive API: `Code::XMI_FRAGMENT_PATH` and the new templates. The SACM-XMI-015 label
  changes.

## Log

- 2026-11-23 design: LIB-COMPAT-002 and LIB-CLI-004 are reworded and analyzed.
  - Q10's deferrals are applied: LIB-COMPAT-009, -011 and -012 are deferred, and
    LIB-COMPAT-017 is promoted to planned.
  - ADR 0044 is filed as accepted, and ADR 0023 amendment 2 is added.
- 2026-11-23 tests: the builder wrote 15 `emf_dialect.rs` tests and 7 `convert.rs` tests, with the fixtures G1, G2, J1 and J2. The lead confirmed the red: ungated, 13 of 15 and 7 of 7 fail. Tests 10 and 11 pass today; they are guards against the resolver over-reaching, and are accepted. Gated, only `Code::XMI_FRAGMENT_PATH` is missing. Snapshot red-0075a.tar.
  - The CI schema loop now also covers crates/sacm/tests/data/compat, so G2 is schema-checked.
  - A new row, LIB-SEC-007 (planned): the builder measured about 214 s to read one element with 100,000 vendor attributes strictly (debug build), so keeping attributes looks quadratic. It is a slice of its own, after 0075a.
- 2026-11-23 implementation: R1 to R6, `Code::XMI_FRAGMENT_PATH`, the templates and `sacm convert` are built. 29 hand mutations were all killed; one survivor was killed by a new unit test.
  - **One step-1 test changed, with the lead's approval:** the strict half of `lib_compat_002_path_resolution_and_the_namespace_scan_are_linear`. Its 100,000 attributes now sit 10 per element, not all on one element. Size, assertion and deadline are unchanged.
    - **Why:** roxmltree 0.21.2's duplicate-attribute check is quadratic per element, so the old input timed the parser, not this row's scan. LIB-SEC-007's cause is that parser check, and its notes say so.
  - **The `Tolerance` refusal kind is removed;** EMF's namespace was its last entry and is now `Tolerated`. Five xtask tests of that check are removed, so LIB-API-011 no longer cites them. Because tests are removed, `mutants.yml` is dispatched on the head.
  - Stale wording now reflects the read EMF dialect in LIB-COMPAT-003, -004, -016, LIB-API-011, LIB-API-017 and SACM23-XMI-001. No test was renamed.
- 2026-11-24 verification round 1 FAIL (LIB-COMPAT-002); LIB-CLI-004 met its acceptance but waits on LIB-COMPAT-002. The fix round:
  - **B1, a regression:** names in `http://www.omg.org/XMI` inside an `xmi:Extension`, or inside a Difference's `target` (open in XMI.xsd), are now kept as written in both modes. They are not ids, and they give no SACM-XMI-015. ADR 0044 D1 and the row are narrowed to match.
  - **B2:** controls C4 to C15 were added.
  - **B3:** six stale rustdoc passages in `xmi.rs` were replaced.
  - **Also:** the CLI help wording, the sacm-expert reference bullets, and the non-blocking findings in `docs/follow-ups.md`. 19 new tests are cited. The builder chose that an EMF-namespace Difference opens nothing; that is logged as a follow-up.
