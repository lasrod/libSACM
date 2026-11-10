---
slice: 0015
name: m1-value-types
state: done
step: close
rows: [SACM23-BASE-001, SACM23-ARG-003]
mode: pr
---

# Slice 0015 — LangString and AssertionDeclaration

## Scope

The first two SACM value types:
- **SACM23-BASE-001:** `LangString`, an optional language tag and optional content, preserved exactly.
- **SACM23-ARG-003:** the `AssertionDeclaration` enumeration, with its exact literals and its default.

Out of scope: XMI reading and writing, and editing. Those rows build on these types.

## Basis

formal/23-05-08 clauses 8 and 11; ptc/22-03-13 `Base::LangString` and
`Argumentation::AssertionDeclaration`; the assurance-forge C++ `model/lang_string.h`
and `model/argumentation.h`.

## Design

ADR 0012 (accepted) and ADR 0011 amendment 2 (accepted). In `sacm::model`, re-exported flat:
- **`LangString`** (`model/lang_string.rs`): private `Option<String>` fields; `Default`, `new(content)`, `with_lang`, `lang()`/`content() -> Option<&str>`, `set_lang`/`set_content(Option<String>)`. Users can build it.
- **`AssertionDeclaration`** (`model/argumentation.rs`): `#[non_exhaustive]`, `#[default] Asserted`, `ALL`, `as_str`, `Display`, `FromStr`.
- **`ParseAssertionDeclarationError`**: a `#[non_exhaustive]` struct with `input()`.

## Log

- 2026-11-10 brief written. The researcher is next.
- 2026-11-10 research: the text and the model agree on both types. BASE-001 is narrowed to the value type; XMI, comparison and editing go to BASE-002/003, LIB-RT-001 and LIB-EDIT-001/008. ARG-003 keeps `as_str`/`FromStr`; the XML attribute goes to ARG-002, and unknown literals to VAL-006. New row SACM23-XMI-011 (string values survive XML exactly). XMI-004 now says "every SACMElement". ARG-003 depends on LIB-API-004, and BASE-001 has no dependency. Both rows are `analyzed`.
- 2026-11-10 the maintainer chose to keep absent, empty and non-empty distinct in LangString, departing from C++ (ADR 0012). The architect is next.
- Logged for later rows: whether VAL-006 keeps the raw invalid literal; whether a `name` with absent or empty content is a missing name (BASE-003); asCited derivation as a later warning-level VAL row; the §11.11 "assertionDeclared" typo for specification-defects.md.
- 2026-11-10 design: the architect follows C++ `lang_string.h` and `argumentation.h/.cpp`. ADR 0012 records the maintainer's decision. The maintainer approved ADR 0011 amendment 2 (`Parse<Type>Error` structs). The acceptance names the API. The test engineer is next.
- 2026-11-10 tests: 46 tests (27 in `crates/sacm/tests/lang_string.rs`, 19 in `crates/sacm/tests/assertion_declaration.rs`). They are red: the items are unresolved. Both rows are `tests-failing`. The parse error quotes its input with `{:?}`, as `InsertError` does (confirmed by the lead). Review items: `#[non_exhaustive]`, private fields, and no `id()`/`kind()` on LangString. The implementer is next.
- 2026-11-10 implementation: `LangString`, `AssertionDeclaration` and `ParseAssertionDeclarationError` in `sacm::model`. All 46 tests pass. The local mutation run caught 17 of 17. Both rows are `implemented`, and the full gate passes.
- 2026-11-10 docs review: 2 blocking findings (the `asCited` doc; the `model` summary implies elements hold these values), plus clarity items and the stale root README. These go to the implementer.
- 2026-11-10 verification round 1 at `1b73f09` ([record](../verification/0015-m1-value-types.md)): both rows FAIL. The narrowing left three obligations without a row (F1–F3), and F4 is the `asCited` doc. Fix round 1 (of 2): the lead adds F1 to XMI-011, F2 to XMI-004 and F3 to ARG-002/ARG-007. The implementer makes the doc fixes.
- 2026-11-10 fix round 1: F1–F3 are in XMI-011, XMI-004, ARG-002 and ARG-007. The implementer made the `asCited` doc, the docs-review items and the root README Status, and `Display` now uses `write_str`. The full gate passes.
- 2026-11-10 Copilot: nothing depended on XMI-011, so the compliance points could be verified without it. LIB-RT-002, which all four CP rows depend on, now depends on XMI-011.
- 2026-11-10 verification round 2 at `702d40a` ([record](../verification/0015-m1-value-types-round2.md)): both rows PASS, and both are now `verified`. One Low finding: `Display` ignored width. It is fixed with a width test (red first) and a return to `f.pad`. The slice is done.
