# Verification 0074b — The compliance claim

- Date: 2026-11-23
- Row verdict: SACM23-CP-001: PASS
- Row verdict: SACM23-CP-002: PASS
- Row verdict: SACM23-CP-003: PASS
- Row verdict: SACM23-CP-004: PASS
- Verifier: `sacm-verifier` (independent, read-only), four rounds.
  - **Round 1, 9355c7b:** FAIL for all four rows, on the conformance statement's wording.
  - **Round 2, a69bc49:** FAIL, on wording.
  - **Round 3, fe32553:** FAIL, on wording.
  - **Round 4, 168c6bb:** PASS for all four rows.

  The maintainer approved rounds 3 and 4 (2026-11-23). CI run
  [45173773997](https://github.com/lasrod/libSACM/actions/runs/45173773997) is on round 4.
- Slice brief: [docs/slices/0074b-compliance-claim.md](../slices/0074b-compliance-claim.md)
- Decisions:
  - M6 decision Q1 (the maintainer, 2026-11-19);
  - [ADR 0043](../adr/0043-the-sacm-xml-schema.md);
  - the maintainer's 2026-11-15 and 2026-11-16 rules on the points' dependencies (SACM23-XMI-027 direct; no Tolerated row in a closure).

## Result

libSACM claims the four model compliance points of SACM 2.3 clause 2: Assurance Case
(mandatory), Argumentation, Artifact and Terminology.

- **The closure.** Every one of the 154 rows the four points depend on is verified. No deferred, candidate or tolerated row is in it.
- **Four unit documents,** one per point. Together they hold every concrete class and property of the model (41 classes, 61 properties), each point its own packages. Each one:
  - reads strictly with no diagnostic and validates with none;
  - is its own strict output, byte for byte;
  - is valid against the generated SACM XML Schema in CI.

  Hand mutations of the documents and of the completeness walk are caught.
- **The conformance statement** (`docs/sacm/compliance-points.md`) was probed case by case with hand-written documents: about 80 in the last round, checked with xmllint and the `sacm` CLI in both modes. It says:
  - what import, export, round trip and constraints mean;
  - how the schema departs from the literal XMI rules;
  - its limits.

  Content the generated schema accepts but that does not conform where it stands is SACM-XMI-003. The statement points to the diagnostics catalog for every case, rather than listing them as complete.
- **README, crate README, crate docs and CHANGELOG** state the claim and no more.

## Findings

| Blocking / non-blocking | Row | Finding | Resolution |
|---|---|---|---|
| Blocking | all four | The statement said a strict read takes every schema-valid document; the generated schema accepts more (ADR 0043 S4, S8). | The limits name the cases (round 1). |
| Blocking | all four | That list was incomplete: roots and parts with links, wrong or shared proxies, `sacm:Claim` under `xmi:XMI`. | It points to the catalog, and gives examples (round 2). |
| Blocking | all four | A nested `xmi:XMI` is refused, not kept. | Named; the SACM-XMI-003 outcome is worded generally, per the catalog (round 3, with the maintainer's approval). |
| Non-blocking | all four | §2.5 wording, comments and processing instructions, CP-004's article, CP-002's literal, the crate paragraph, D-STRICT, SACM-XMI-003 and -009 catalog entries, LIB-RT-003 dependency. | Fixed with the verifier's text. |
| Non-blocking | CP-001 to -004 | The unit documents hold no proxy; `latin1` encoding label refused. | `docs/follow-ups.md`. |

**Lesson.** A claim that "everything valid reads" needs a general rule and a pointer to the
catalog. An enumerated list goes stale with each probe.
