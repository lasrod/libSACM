# Verification 0065 — ExpressionLangString and its rules

- Date: 2026-11-18
- Row verdict: SACM23-BASE-009: PASS
- Row verdict: SACM23-VAL-009: PASS
- Row verdict: SACM23-VAL-017: PASS
- Verifier: `sacm-verifier` (independent; read-only), two rounds
- Commits:
  - 796dabf, round 1: VAL-017 PASS; BASE-009 FAIL on test gaps (13 surviving mutants) and
    false public docs; VAL-009 held by its dependency on BASE-009. CI run
    [44177737827](https://github.com/lasrod/libSACM/actions/runs/44177737827) was green.
  - e53c69a, round 2: PASS on all three rows. CI run
    [44191859776](https://github.com/lasrod/libSACM/actions/runs/44191859776). The PR-diff
    mutation job does not block (ADR 0006 amendment 6).
- Slice brief: [docs/slices/0065-expression-lang-string.md](../slices/0065-expression-lang-string.md)
- Decision: ADR 0038 and ADR 0030 amendment 4, accepted by the maintainer on 2026-11-18
  as written.

## Result

- **ExpressionLangString (§8.4)** is a form of `LangString`: a name, or a value of a
  `content`, `key` or `location`, may link one ExpressionElement. It is read, written
  (`xmi:type="sacm:ExpressionLangString"` plus `expression="id"`), compared, copied and
  serialized as JSON.
- **The derived `ReferenceEnd::Expression`** equals the links of the element's name and
  texts after every operation; `referrers` lists it; the reference edits refuse it as
  `EditError::DerivedEnd`.
- **Edits** check an ExpressionLangString's `expression` after the operation's own ends,
  and refuse one with no expression (`TooFewReferences`).
- **Nonconforming input** (an `expression` on a plain name or value, a `value` inside an
  ExpressionLangString name) keeps the whole part with SACM-XMI-003.
- **VAL-009:** a link beside a non-empty content is a Warning (SACM-EXPR-001).
- **VAL-017:** a concrete Expression must reference concrete elements.
- **The reworded verified rows are true:** XMI-012, BASE-004, BASE-001, LIB-API-015,
  LIB-EDIT-009, VAL-022, LIB-API-017, XMI-006, LIB-API-011 (round 0); LIB-EDIT-001, -008,
  -012, XMI-007 and LIB-RT-001 (round 1).

Evidence beyond CI: every hand mutant of rounds 1 and 2 is killed. The verifier re-ran the
12 round-1 survivors itself on e53c69a.

## Findings

| Severity | Row | Finding | Resolution |
|---|---|---|---|
| blocking | SACM23-BASE-009 | Untested: the form's place in `Ord` (M01), a stale derived end after a remove or an asset create (M40, M41, M41c), the asset creates' name check (M38c) and the check phase order (M39b). | An independent oracle run after every operation, and the cases added. |
| blocking | SACM23-BASE-009 | Untested: an `expression` element in a plain name or value (M69, roots.rs:4058), a `value` inside an ExpressionLangString name (M22b), repeated JSON keys (M28), the derived end written as an element (M34), the JSON `expecting` message. | Cases added. |
| blocking | SACM23-BASE-009 | Public rustdoc (edit module, `SetContent`, `SetKey`, `SetLocation`, `xmi::read`, `xmi::write`, `ReferenceEnd::bounds`) and four verified rows did not mention ExpressionLangString. | Reworded with the verifier's text. |
| non-blocking | SACM23-BASE-009 | Stray backticks in link targets, two dead filters, the SACM-REF-002 row, CHANGELOG wording, the code lists. | Fixed. |

## Logged

Two items in docs/follow-ups.md:
- a JSON array LangString is now refused;
- `reference_tokens` reserves an id named by a preserved `expression` attribute.
