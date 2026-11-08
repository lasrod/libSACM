# Verification 0007, round 2: m0-verdict-parser (after fix round 1, strict records)

- Date: 2026-11-08
- Verdict: **PASS.** LIB-TOOL-005 and LIB-TOOL-015 both pass.
- Row verdict: LIB-TOOL-005: PASS
- Row verdict: LIB-TOOL-015: PASS
- Verifier: `sacm-verifier` (independent; read-only)
- Commit inspected: `0dbc33b35f0a940c98c92b38b88ba8f9446db054` (PR #14), `git diff 27753f8..0dbc33b`
- Contract: P-0007 from the [round 1 record](0007-m0-verdict-parser.md), checked item by item. The maintainer chose strict records (ADR 0008 amendment 1), with the differential check inside this fix.
- Linux evidence at that commit:
  - mutants.yml [42469915549](https://github.com/lasrod/libSACM/actions/runs/42469915549): 464 mutants, 453 caught, 0 missed, 0 timeout, 11 unviable. All 11 are structural, and the set equals `cargo mutants --list`, with no exclusions.
  - ci.yml [42469915514](https://github.com/lasrod/libSACM/actions/runs/42469915514): `mutants (PR diff)` reports 54 mutants, 53 caught, 1 unviable (structural). The `cargo-deny` job failed on infrastructure: GitHub returned HTTP 504 for the cargo-deny release asset. `cargo deny check` passes locally (lead and verifier), and the job is re-run before the PR is marked ready.
- Slice brief: [docs/slices/0007-m0-verdict-parser.md](../slices/0007-m0-verdict-parser.md)

The verifier's verdicts:
- "LIB-TOOL-015: Verified at `0dbc33b`: a `- Row verdict:` line is hidden only by a code block or by an HTML block that is one closed comment, and all other raw HTML is reported at each line that holds it, once per citing row, so the record cannot pass. Allowed are inline closed comments and `<br>`, `<ul>`, `</ul>`, `<li>`, `</li>` after other text on their line. Every round-1 probe (F1, F3, F4 b) is refused. This is shown by the 31 cited tests, a Linux mutants run with 0 missed, and the verifier's runs against comrak, markdown-rs and markdown-it, which found no disagreement."
- "LIB-TOOL-005: Verified at `0dbc33b`: a verified row passes only when its record is strict and holds exactly one counted `- Row verdict: <ROW-ID>: PASS` line, with a well-formed id. Code blocks are decided by pulldown-cmark with the D7 tab normalisation. Lone CRs, a leading byte-order mark and verdict-like lines, as README rule 5 defines them, are reported problems. The D12 differential test finds 0 disagreements on 11 seeds at 100,000 records, and the verifier's 49,886 strict records give the same verdicts as comrak, markdown-rs and markdown-it."

## P-0007, item by item

| Item | Result |
|---|---|
| P-1 type-1 end | **Met** (D8 route, now fail-safe). Every F1 probe gives the raw-HTML problem. |
| P-2 differential | **Met.** <ul><li>D12, re-run on 5 further seeds of 100,000 records each: 0 disagreements.</li><li>Every adjudication was checked against comrak and markdown-it.</li><li>The verifier's own generator produced 49,886 strict records. On them the gate disagreed 0 times with comrak, markdown-rs and markdown-it, and 0 times with comrak running GFM.</li></ul> |
| P-3 lone CR, P-4 BOM | **Met** (refusals, D10). |
| P-5 D6 | **Met** (route R, by D8). Every F3 probe is refused. |
| P-6 look-alikes, raw HTML | **Met as decided** (D8, D9). |
| P-7 | **Met.** M15 has a test. M1's code is gone, and its successor N2 is killed. |
| P-8 documents | **Met.** README rule 5, ADR 0008 and the CHANGELOG state the implemented rules. |
| P-9 mutation evidence | **Met.** Linux shows 0 missed. All hand mutants are killed except N26, which is equivalent. |

## Rows verified

| Row | Title | Evidence (tests) |
|---|---|---|
| LIB-TOOL-005 | Verification records carry a per-row verdict the gate reads | 59 cited tests, among them `xtask/tests/verdict_markdown.rs`, `xtask/tests/verdict_differential.rs` (D12) and `xtask/tests/gate_hardening.rs`. |
| LIB-TOOL-015 | Verdict lines hidden by Markdown do not count | 31 cited `lib_tool_015_*` tests in `xtask/tests/verdict_markdown.rs`. |

**No test was weakened.** All the old record literals are still present. The verifier ran all 602 old test records through the old gate and the new one. None went from not passing to passing. 74 went from passing to a problem, which are the intended refusals.

## Gates run (by the verifier, on Windows)

All pass: fmt, clippy (`-D warnings`), the workspace tests, `cargo test -p xtask`, rustdoc (`-D warnings`), `matrix check` (155 rows), `cargo deny check`, and the hook tests (39).

## Findings

| Severity | Row | Finding | Disposition |
|---|---|---|---|
| Low | 005 | **The rest of F4.** Some verdict text is neither counted nor reported, because its prefix is altered: no colon, different case, emphasis, a task box, a heading, an escape, or unusual whitespace and control characters. The row's criteria define the look-alike rule exactly, so this does not block. | Follow-up row LIB-TOOL-018. |
| Info | 005 | N26, swapping the `Malformed` and `LookAlike` order, is equivalent: a line is either a candidate or a look-alike, never both. | None. |
| Info | 005 | D12 skips adjudicated records (about 2%) instead of comparing them. | Noted in LIB-TOOL-018's source. |
