---
slice: 0074b
name: compliance-claim
state: in-progress
step: verification
rows: [SACM23-CP-001, SACM23-CP-002, SACM23-CP-003, SACM23-CP-004]
mode: pr
base: main (58283c5)
---

# Slice 0074b — The compliance claim

## Scope

libSACM claims the four model compliance points of SACM 2.3 clause 2: Assurance Case
(mandatory), Argumentation, Artifact and Terminology. Each point has a unit document, a test
that it reads, validates and round-trips byte for byte, and a test that it holds every class
and property of its packages. The conformance statement says what the claim covers and what
its limits are. The README and the crate docs say it.

Left out: the SACM UML Profile point (D-SCOPE).

## Basis

- SACM 2.3 clause 2, §2.2 to §2.5.
- XMI 2.5.1 §2.2.1 and clause 8.
- M6 decision Q1 (the maintainer, 2026-11-19); ADR 0043.

## Design

- The design brief, `target/slice-0074-design.md` (git-ignored):
  - section 2: the rows, the CORE dependency list, the completeness walk, and the four unit
    documents;
  - section 5: the conformance statement and the README and crate wording;
  - section 6: the CHANGELOG entry.
- No API change.

## Log

- 2026-11-22 design: every row in the four points' dependency closure (152 rows) is verified.
  The four CP rows are reworded per section 2 and are analyzed.
- CI: the `SACM XML Schema` job also runs `cargo test -p xtask --test compliance`. The unit
  documents under `crates/sacm/tests/data/sacm23/cp/` become strict outputs through the
  existing loop.
- 2026-11-22 tests: the builder wrote the four unit documents (strict outputs of the design's sources, with the cross-document hrefs that slice 0073b reads), four `compliance_points.rs` tests and four `compliance.rs` walks. All pass on today's behaviour, which is expected, since the claim needs no new behaviour: no diagnostic, byte-for-byte round trips, schema-valid, every class and property seen (41 classes, 61 properties). Hand mutations of the unit documents fail them. The open work is the docs (acceptance 4 and CP-001 item 5). Snapshot red-0074b.tar.
- 2026-11-22 implementation: the builder wrote the conformance statement, the README and crate wording, and the CHANGELOG entry. It checked each fact against the code and corrected the brief's text in six places:
  - the tolerant namespace route is narrower (ADR 0041 D4);
  - a non-SACM root is refused, not skipped;
  - an untyped contained object under an abstract type is schema-valid but strictly refused;
  - LIB-COMPAT-007 is the IDREF check itself;
  - the same document always writes the same bytes (SACM23-XMI-007);
  - only the Assurance Case unit holds the whole model.

  It also added the namespace-binding limit, and ADR 0043's S6 and S7 to the schema departures.
  - The lead's ruling: the design's dependency lists broke two of the maintainer's rules, which LIB-API-011's test enforces (2026-11-15: every point depends directly on SACM23-XMI-027; 2026-11-16: LIB-COMPAT-004, a Tolerated row, is in no point's closure). The rules win: XMI-027 is added to CP-001, and LIB-COMPAT-004 is dropped from CP-002 to CP-004. The statement still names LIB-COMPAT-004 as a limit.
- 2026-11-22 verify round 1 (9355c7b): FAIL for all four rows, on one wording defect. The statement said a strict read accepts every schema-valid document, but the generated schema accepts more than a strict read does (ADR 0043 S4, S8). The verifier listed the cases. Everything else passes: unit documents, tests, the closure, CI, README and CHANGELOG.
  - Fix round, docs only:
    - the builder applies the verifier's statement text (Import, what the schema does not check, Encoding, comments and processing instructions, the other points' wording), the crate claim paragraph and two catalog entries;
    - the lead corrects the CP-004 article, CP-002's literal wording and D-STRICT, adds LIB-RT-003 to CP-002 to CP-004, and logs the no-proxy follow-up.
