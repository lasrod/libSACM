# Verification 0007, round 3: m0-verdict-parser (after D14, from the Copilot review)

- Date: 2026-11-08
- Verdict: **FAIL.** LIB-TOOL-005 and LIB-TOOL-015 both fail on D14.
- Row verdict: LIB-TOOL-005: FAIL
- Row verdict: LIB-TOOL-015: FAIL
- Verifier: `sacm-verifier` (independent; read-only)
- Commit inspected: `04d009046d69507fa2426740fd305c1a973ce26b` (PR #14), `git diff ee4ef31..04d0090`.
- Context:
  - Both rows passed in [round 2](0007-m0-verdict-parser-round2.md).
  - Copilot comment 4505961274 then asked that a verdict-like line inside a closed inline comment be hidden. The lead added D14 and demoted both rows.
- Linux evidence at that commit:
  - mutants.yml [42475224874](https://github.com/lasrod/libSACM/actions/runs/42475224874): 471 mutants, 460 caught, 0 missed, 0 timeout, 11 unviable.
  - ci.yml [42475224860](https://github.com/lasrod/libSACM/actions/runs/42475224860): every job succeeded. In the PR diff: 61 mutants, 60 caught, 1 unviable.
- References used:
  - markdown-it-py 4.2.0;
  - markdown-rs 1.0;
  - GitHub's renderer, through the read-only `POST /markdown` API.

## What holds

- The Copilot record passes, and GitHub hides its line 2.
- Candidates are never inside an inline comment. 200,000 generated records confirm it.
- Boundary lines are not hidden.
- Nothing regressed: P-0007 and the round-2 verdicts still hold.
- The real records pass.

## Finding

**D14 hides look-alikes that GitHub shows (High, both rows).** Each record below, with a PASS line added, passes at `04d0090` and was refused at `ee4ef31`. In each, a reader on GitHub sees `Row verdict: LIB-TOOL-001: FAIL`.

| | Case | Example | Why the line is visible |
|---|---|---|---|
| (a) | Comment text ending in `-` | `Text <!-- a\nRow verdict: LIB-TOOL-001: FAIL\n--->\n` | CommonMark 0.31.2 reads it as a comment. GitHub's cmark-gfm and markdown-it read it as text. |
| (b) | GFM table | the comment spans a delimiter row | The look-alike becomes a table cell. |
| (c) | GFM footnote | `[^1]: Row verdict: …` | GitHub shows it in the footnotes section. |
| (d) | Image description | `![a <!-- a\n…\n-->](x.png)` | The comment becomes alt text, which is shown when the image fails to load and is read by screen readers. |

This breaks two rules: LIB-TOOL-015's requirement ("a record whose rendering is ambiguous cannot pass") and LIB-TOOL-005's strict-records criterion.

## Decision

The verifier offered two routes:
- **W:** withdraw D14.
- **N:** narrow D14 with GFM-specific conditions.

**The maintainer chose W** (2026-11-08). A closed inline comment stays allowed but hides nothing, as at round 2. Its look-alikes are reported. That is fail-safe, and it matches strict records (ADR 0008 amendment 1).

LIB-TOOL-015 criterion 1 concerns `- Row verdict:` lines, and those can never lie inside an inline comment (§5.2). So the Copilot record stays refused, and the reply on the thread gives this evidence.

## Prescription for the fix (route W)

1. **Code:** return to the `ee4ef31` behaviour, so that a closed inline comment hides nothing.
2. **Tests:**
   - The two `lib_tool_015_*` hiding tests become "look-alike reported" tests, the Copilot record included.
   - Probes (a) to (d) become `lib_tool_005_*` tests, run with LF and CRLF. They must give the look-alike problem and never pass.
   - The boundary and paragraph-ending controls stay.
3. **Docs:** the D14 text (now withdrawn), README rule 5's "Hidden lines" bullet and the CHANGELOG state the implemented rule.
4. **Evidence:** new Linux mutants.yml and PR-diff runs at the fix head, with 0 missed and 0 timeout, and the triage entry updated.
5. **Re-verification:** the verifier re-runs the GitHub sample, which must show 0 lines that the gate hides and GitHub shows, and the old-versus-new comparison.

## Status

This is LIB-TOOL-015's second FAIL. The maintainer decided the route, as above. Both rows stay `implemented`.
