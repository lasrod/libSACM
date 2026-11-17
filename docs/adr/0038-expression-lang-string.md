# ADR 0038 — ExpressionLangString

- Status: proposed. It records the maintainer's M4 decisions Q4 to Q7 (2026-11-17); its further details (the XMI shape, `DerivedEnd`, refusing an ExpressionLangString with no expression in an edit) await approval.
- Row: SACM23-BASE-009, SACM23-VAL-009
- Related: ADR 0012, ADR 0030 amendment 4, spec defects D6 and D20

## Context

formal/23-05-08 §8.4 defines ExpressionLangString, a LangString that may point to an ExpressionElement. The design brief is `target/slice-0065-design.md` (git-ignored).

## Decision

- **D1:** the private form on `LangString`, with the four methods; `Eq`, `Ord` and `Hash` include the form.
- **D2:** the XMI shape. A `name`, or a `value` of a `content`, `key` or `location`, typed `xmi:type="sacm:ExpressionLangString"`, holds its `expression` as a reference: an `expression="id"` attribute, or an `<expression xmi:idref="id"/>` or `<expression href="#id"/>` element. `xmi::write` writes the attribute. This follows the model's reference rather than the text's "(composition)" (D6). Under rule 4i the end is a single IDREF, optional in the schema.
- **D3:** `ReferenceEnd::Expression` is derived. The reference edits refuse it as `DerivedEnd`. Creates and sets check its ids, and refuse an ExpressionLangString with no expression, because D-VAL-LAYER makes required ends structural.
- **D4:** on delete, scrub makes the text plain; reject refuses.
- **D5:** a missing expression is SACM-MULT-001 (D20), and SACM-EXPR-001 fires only on a present, non-empty content.
- **D6:** the JSON, in ADR 0030 amendment 4.
- **Alternatives rejected:**
  - a `Text` enum, which breaks the API;
  - a separate text-referrer type;
  - a nested composite ExpressionElement, which is the PDF's "(composition)" and contradicts §8.4's own "a reference to an ExpressionElement in the TerminologyPackage".
