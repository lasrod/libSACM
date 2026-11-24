# Verification 0075a — The EMF dialect and `sacm convert`

- Date: 2026-11-24
- Row verdict: LIB-COMPAT-002: PASS
- Row verdict: LIB-CLI-004: PASS
- Verifier: `sacm-verifier` (independent, read-only), three rounds.
  - **Round 1, 4e67af3:** FAIL for LIB-COMPAT-002. LIB-CLI-004 met its acceptance but was held on its dependency.
  - **Round 2, 00c938c:** FAIL, on tests only.
  - **Round 3, 4ea12e3:** PASS for both rows.

  The maintainer approved round 3 (2026-11-24) after reviewing PR #104.
- Slice brief: [docs/slices/0075a-emf-dialect.md](../slices/0075a-emf-dialect.md)
- Decisions: [ADR 0044](../adr/0044-the-emf-dialect-and-foreign-subclasses.md) D1 and D2, with D1 narrowed in round 1; ADR 0023 amendment 2; D-STRICT; slice 0075 Q1 and Q2 (the maintainer, 2026-11-23).

## Result

- **LIB-COMPAT-002.** A tolerant read reads what EMF writes:
  - the XMI names in `http://www.omg.org/XMI` are read as XMI 2.5.1's, with one SACM-XMI-015 Warning;
  - EMF fragment paths in reference ends, same-document `href`s and `expression`s are read as the ids they name, with one SACM-XMI-017 Warning.

  A strict read refuses the dialect and resolves no path. Names in `http://www.omg.org/XMI` inside the content of an `xmi:Extension`, or of a Difference's `target`, are kept as written in both modes: XMI.xsd leaves that content open. Both saves write XMI 2.5.1.
- **LIB-CLI-004.** `sacm convert <IN> -o <OUT>` writes a readable file as strict SACM 2.3 only when the strict save succeeds and its output reads back unchanged. Otherwise it exits 1 and leaves `<OUT>` untouched.
- **Corpus.** `sacm convert` converts 2 of the 15 corpus files, exits 1 on 9 and exits 2 on 4, as the design predicted. File 7's 35 paths are all read.
- **Tests.** There are 41 `emf_dialect` tests and 7 `convert` tests. Every hand mutant of the new decision logic is killed, apart from three that are equivalent (logged).

## Findings

| Blocking / non-blocking | Row | Finding | Resolution |
|---|---|---|---|
| Blocking | LIB-COMPAT-002 | A strict read refused schema-valid SACM 2.3 that carried EMF names inside an `xmi:Extension` or a Difference's `target`. A tolerant read gave DuplicateId. | Open content is skipped by the scan and the alias. ADR 0044 D1 and the row are narrowed. Controls C1 to C3, and two fixtures that pass xmllint (round 1). |
| Blocking | LIB-COMPAT-002 | 12 surviving hand mutants in the path and alias code. | Controls C4 to C15 (round 1). C4 and C5 reached the wrong code, so F1 and F2 replace them (round 3). |
| Blocking | LIB-COMPAT-002 | 7 untested decisions in the skip code and the path order. | F1 to F7 (round 3). |
| Blocking | LIB-COMPAT-002 | Stale rustdoc in `xmi.rs` about which `XMI` elements are refused. | The verifier's six passages (round 1). |
| Non-blocking | LIB-CLI-004 | The exit-1 help said "would lose content". | Reworded with the verifier's text. |
| Non-blocking | LIB-COMPAT-002 | The `sacm-expert` reference had no EMF entry. | Two bullets added. |
| Non-blocking | LIB-COMPAT-002 | 3 equivalent mutants; EMF path proxies are not resolved; the `xmi-namespace-root` template wording; an EMF-namespace Difference opens nothing; the PR-diff survivor at `roots.rs:4462`, from before the slice. | `docs/follow-ups.md`. |

**Lesson.** A namespace rule that says "anywhere" must be checked against the places
XMI.xsd leaves open (`xsd:any`) before it is written down.
