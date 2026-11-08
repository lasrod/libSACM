# Verification 0007, round 1: m0-verdict-parser

- Date: 2026-11-08
- Verdict: **FAIL.** LIB-TOOL-005 and LIB-TOOL-015 both fail.
- Row verdict: LIB-TOOL-005: FAIL
- Row verdict: LIB-TOOL-015: FAIL
- Verifier: `sacm-verifier` (independent; read-only)
- Commit inspected: `123c71ed9847d42737187bacda0dbc9134b0a8bd` (draft PR #14), `git diff main...123c71e`
- Linux evidence at that commit:
  - ci.yml [42453353263](https://github.com/lasrod/libSACM/actions/runs/42453353263): 27 mutants, 19 caught, 0 missed, 0 timeout, 8 unviable.
  - mutants.yml [42453353258](https://github.com/lasrod/libSACM/actions/runs/42453353258): 447 mutants, 429 caught, 0 missed, 0 timeout, 18 unviable (structural; this matches the triage section).
- Method: the verifier compared the gate against three CommonMark references (markdown-it-py 4.2.0, comrak 0.55.0, markdown-rs 1.0.0) on about 50,000 generated records, using probes and hand mutations.
- Slice brief: [docs/slices/0007-m0-verdict-parser.md](../slices/0007-m0-verdict-parser.md)

## What holds

- **P-005b:**
  - T7 (P1, P1b, P6 and P8 give FAIL; P9 gives no-verdict), T8, item 3 and F5 are met. All three references agree.
  - P-005b item 7 is only partly met (F6).
- **LIB-TOOL-015:** the seeds P2 and P7 are met.
- **D1 and D4:** met, and `cargo deny` passes.
- **D7:** sound.
- **Copilot comment 4505191110:** fixed.
- **No test was weakened.**
- **Real records:** all 25 give the same result from the gate and from the three references.

## Findings

| Severity | Rows | Finding |
|---|---|---|
| High | both | **F1:** pulldown-cmark 0.13.6 ends a type-1 HTML block only at the matching lowercase end tag (`firstpass.rs:1257`, `2181-2209`). CommonMark §4.6 ends it at any of `</pre>`, `</script>`, `</style>`, `</textarea>`, case-insensitive. Example: `"{PASS}\n\n<pre>\n</PRE>\n{FAIL}\n"` passes the row, and a reader sees the FAIL. The reverse case gives a false FAIL. ADR 0008's claim "exactly when a CommonMark renderer would hide it" is false. |
| Medium | 005 | **F2 (new class):** candidates are split on LF and CRLF only. CommonMark and the parser also end a line at a lone CR, so `"{PASS}\n\ntext\r{FAIL}\n"` passes. A leading U+FEFF also hides a FAIL. |
| Medium | 015 | **F3:** D6 over-hides, and over-hiding gives a false PASS. It ignores three cases: `--!>` closes a comment; a `<!--` in an attribute value opens none; a `<!--` in raw-text elements (`style`, `script`, `textarea`) opens none. |
| Medium | both | **F4 (new class, criterion level):** a reader sees two kinds of verdict text that the gate ignores by design. One is an indented or nested `- Row verdict:` item. The other is text in an HTML block, such as `<div>\n{FAIL}\n</div>`, which a test pins as passing. |
| Low | both | **F5:** two hand mutants survive: M1 (`end <= opener` replaced by `<`) and M15 (only the first trailing tab replaced). |
| Low | both | **F6:** README rule 5, ADR 0008, the brief, the CHANGELOG and one test comment overclaim. |

## Prescription P-0007 (complete; the verifier's full text is in the slice log)

- **P-1: type-1 end condition.** Work around it, switch parser, or refuse HTML (see P-6(b)).
- **P-2: differential evidence.** A repeatable differential check against an independent CommonMark implementation over generated records, with 0 unadjudicated disagreements over at least 20,000 records.
- **P-3: the line model.** Use CommonMark line endings, or report a lone CR as a problem.
- **P-4: leading BOM.** Strip it or refuse it.
- **P-5: D6.** Either (R) refuse an unclosed `<!--` inside HTML (recommended), or (M) model the HTML tokenizer's comment rules.
- **P-6: visible look-alikes.** A recorded decision, plus tests, on each of:
  - (a) indented or nested verdict items;
  - (b) candidates inside HTML.
- **P-7: test gaps.** Tests that kill M1 and M15.
- **P-8: documents.** State the rules actually implemented.
- **P-9: mutation evidence.** New Linux evidence. The verifier re-runs its hand mutants.

## Status

This is LIB-TOOL-005's **third** FAIL (rounds 1 and 2 of slice 0004, and this one). P-5 and P-6 need maintainer decisions. Both rows stay `implemented`.

## Process note

During its run, the verifier once ran `taskkill //F //IM cargo.exe` to stop its own scratch mutation run. That would have killed any other `cargo.exe` running on the machine at the same moment.
