# Verification 0007, round 4: m0-verdict-parser (D14 withdrawn, route W)

- Date: 2026-11-08
- Verdict: **PASS.** LIB-TOOL-005 and LIB-TOOL-015 both pass.
- Row verdict: LIB-TOOL-005: PASS
- Row verdict: LIB-TOOL-015: PASS
- Verifier: `sacm-verifier` (independent; read-only)
- Commit inspected: `f4d63d545a35223c3d25470a490fb90ef05b4127` (PR #14), `git diff 04d0090..f4d63d5`. Against `ee4ef31`, `xtask/src` differs only in rustdoc.
- Contract: items 3 to 7 of P-0007-D14 in the [round 3 record](0007-m0-verdict-parser-round3.md), under route W, which the maintainer chose.
- Linux evidence at that commit:
  - mutants.yml [42481198111](https://github.com/lasrod/libSACM/actions/runs/42481198111): 464 mutants, 453 caught, 0 missed, 0 timeout, 11 unviable. They are structural, and the same set as at `0dbc33b`. The set equals `cargo mutants --list`, with no exclusions.
  - ci.yml [42481198200](https://github.com/lasrod/libSACM/actions/runs/42481198200): every job succeeded, `cargo-deny` included. `mutants (PR diff)`: 54 mutants, 53 caught, 1 unviable.
- Slice brief: [docs/slices/0007-m0-verdict-parser.md](../slices/0007-m0-verdict-parser.md)

The verifier's verdicts:
- "LIB-TOOL-015: Verified at `f4d63d5` (route W, D14 withdrawn). A `- Row verdict:` line is hidden only by a code block or by an HTML block that is one closed comment. A closed inline comment is allowed but hides nothing, so a verdict-like line inside one is reported, and the record cannot pass on a rendering that GitHub and CommonMark disagree on. This is shown by the cited tests, among them the round-3 probes (a) to (d) with LF and CRLF, by a Linux mutants run with 0 missed, by output identical to round 2 on 100,000 generated records, and by a GitHub-renderer sample that found no hidden line GitHub shows in a passing record."
- "LIB-TOOL-005: Verified at `f4d63d5` (route W). A verified row passes only when its strict record holds exactly one counted `- Row verdict: <ROW-ID>: PASS` line with a well-formed id. Look-alikes inside closed inline comments are reported again, as at round 2. No candidate lies inside an inline comment in 300,000 generated records. The D12 differential test passes, mutants show 0 missed on Linux, and the real records pass."

## Checks

| Check | Result |
|---|---|
| The code is back to the round-2 behaviour | **Met.** On 100,000 generated records, the gates at `f4d63d5` and `ee4ef31` give identical output. Every round-3 probe gives the look-alike problem. |
| Tests | **Met.** <ul><li>The two hiding tests are rewritten as "reported" tests.</li><li>Probes (a), (b), (d) and the part of (c) that D9 can see are new tests, with LF and CRLF.</li><li>The controls stay.</li><li>No other test is weakened.</li></ul> |
| Docs | **Met.** The brief, README rule 5 and the CHANGELOG state the rule now in force. |
| Linux evidence | **Met.** 0 missed and 0 timeout. The triage entry matches. |
| GitHub sample (two runs of 30,000 records, 300 rendered each) | **Met.** No line that the gate hides in a passing record is shown by GitHub. The only lines GitHub showed were the LIB-TOOL-018 forms (task boxes), which D9 does not see but the gate also does not hide. |
| Candidate check | **Met.** In 300,000 generated records, no candidate lies inside a closed inline comment. |
| Real records | **Met.** |

## Gates run (by the verifier, on Windows)

All pass:
- fmt;
- clippy (`-D warnings`);
- the workspace tests (`verdict_markdown` 75, D12 1);
- rustdoc (`-D warnings`);
- `matrix check` (156 rows);
- `cargo deny check`;
- the hook tests.

## Findings

| Severity | Row | Finding | Resolution |
|---|---|---|---|
| Info | 015 | The inline clause of criterion 1 and the rustdoc read differently. No input can reach the difference (§5.2). | The criterion is reworded: a candidate can never lie inside an inline comment. |
| Info | 005 | Some forms D9 cannot see (task boxes, `[^1]:`, a leading table pipe) are not hidden lines either. | They are already in LIB-TOOL-018. |
