# Verification 0021, round 5 — m1-interchange-roots

- Date: 2026-11-11
- Row verdict: SACM23-XMI-001: PASS
- Row verdict: LIB-SEC-005: PASS
- Row verdict: LIB-API-009: PASS
- Row verdict: LIB-SEC-002: PASS
- Re-checked: LIB-API-009 (items 1 and 3 reworded) and LIB-SEC-002 stay verified. Their seven changed tests moved to inputs that are still refused, and none was weakened.
- Verifier: `sacm-verifier` (independent; read-only), at full depth (library rows). Rounds 3 to 5 were authorised by the maintainer after the second FAIL.
- Commit: `56bf1f1`. Evidence: ci.yml 43010285529 (15/15) and mutants.yml 43010285526, both success. Full run: 838 caught, 0 missed, 0 timeout, 60 unviable, 1 excluded as equivalent (`xmi.rs:245:9`, `ReadOutput::diagnostics`, which is empty in this release). PR diff: 192 caught, 0 missed.
- Slice brief: [docs/slices/0021-m1-interchange-roots.md](../slices/0021-m1-interchange-roots.md)

## Result

- **The reader:**
  - `xmi::read` reads AssuranceCasePackage and ArgumentPackage roots, bare or in `xmi:XMI`.
  - It refuses non-package roots with SACM-XMI-001, and everything it cannot read yet as `Unsupported`.
  - It returns no partial document.
  - Its departures from C++ are in ADR 0016, which the maintainer accepted, including the amended D1 and D3.
- **The XML checks:** LIB-SEC-005 refuses a closed list of inputs that roxmltree accepts against XML 1.0 or Namespaces 1.0. The maintainer narrowed the row to this list. Finding the rest is LIB-SEC-006 (W3C XML Conformance Test Suite), and the public docs say so.
- **Every defect stays caught:**
  - Every defect from rounds 1 to 3, re-inserted by hand, is killed by tests named for the row.
  - Tree-oracle sweeps found no disagreement: about 385k accepted inputs in round 2, about 9M in round 3 and 16M in round 4. Round 5 changed only docs and tests, so the round-4 sweep still applies.
  - The permanent test T7 checks 12,000 generated inputs against the parser's tree.
- **Sources:** the quotes from formal/23-05-08, XMI 2.5.1 and the C++ reader were checked against the pinned texts and the code.

## Findings

| Round | Severity | Row | Finding | Resolution |
|---|---|---|---|---|
| 1 | High | LIB-SEC-005 | The post-parse scan ended a comment inside `<!--`, so `<!-->…-->` let a later surrogate reference through, repaired to U+FFFD. | Each construct now ends after its opening delimiter. Tests T1 to T3. |
| 1 | Medium | LIB-API-009 | The same defect made well-formed input `Malformed`. | The same fix. T1 checks that the result is `Unsupported`. |
| 1 | Medium | SACM23-XMI-001 | Blocked by LIB-SEC-005, and the amended ADR 0016 D1 and D3 needed sign-off. | The maintainer confirmed the amendment. |
| 1 | Low | — | The `check_text` doc; `xmi:documentation` casing; the D10 citation; the LIB-API-009 wording; the triage line. | Fixed. |
| 2 | High | LIB-SEC-005 | The PI target ended at white space or `?`, so `<?xml>?>` read Ok. | The target now ends at the XML Name. Tests T4 and T5. |
| 2 | Medium | LIB-SEC-005 | `<?pi>x?>` was accepted, against XML [16]. | Refused, by the maintainer's decision. Test T6, and the T7 sweep. |
| 3 | High | LIB-SEC-005 | An empty prefix was read as none, so `<:ArgumentPackage …></ArgumentPackage>` read Ok. | Refused at the name. Test T9. |
| 3 | Medium | LIB-SEC-005 | A colon in a PI target was accepted, against Namespaces 1.0 §7. | Refused. Test T8. The row was narrowed to a closed list, and LIB-SEC-006 was added. |
| 3 | Medium | LIB-SEC-005 | The [4]/[4a] range edges were unpinned. | Test T10. |
| 4 | Medium | LIB-SEC-005 | The public docs read as if the refused list were complete. | They now name LIB-SEC-006 for the rest. |
| 4 | Low | LIB-SEC-005 | Hand mutants H13, H11 and X1 survived; the triage line and two citations were out of date. | Killed by new cases; fixed. |

Logged for LIB-SEC-006: roxmltree accepts `xmlns:xmlns="urn:x"` (Namespaces 1.0 §3).
